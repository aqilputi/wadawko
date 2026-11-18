//! Render offline: determinismo (semente fixa → hash estável), duração, pico,
//! ausência de NaN/clipping, mudo/solo, progresso e cancelamento.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use common::{bank, hash, peak, rms, yagyo};
use wadawko_audio::{Cancelled, MasterMode, Region, RenderOptions, Rendered, SampleSet, render, render_with};
use wadawko_core::demo::demo_song;
use wadawko_core::model::Person;
use wadawko_core::ops::cells::set_kyai;

fn opts() -> RenderOptions {
    RenderOptions { seed: 2026, ..Default::default() }
}

fn assert_sane(label: &str, r: &Rendered) {
    assert!(r.samples.iter().all(|s| s.is_finite()), "{label}: NaN/inf");
    // teto do limiter: −1,5 dBFS (0,8414); nunca clipa
    assert!(r.peak() <= 0.8415, "{label}: pico {} acima do teto", r.peak());
    assert!(r.rms() > 0.01, "{label}: silêncio? rms {}", r.rms());
}

#[test]
fn demo_render_is_deterministic_and_sane() {
    let song = demo_song();
    let a = render(&song, bank(), &opts());
    assert_sane("demo", &a);
    // lead 0,1 s + 4 compassos × 4 tempos × 0,6 s + 3 s de cauda
    assert_eq!(a.sample_rate, 48_000);
    assert_eq!(a.frames(), ((0.1f64 + 9.6 + 3.0) * 48_000.0).ceil() as usize);
    assert_eq!(a.hits, 67);
    // silêncio antes do 1º tempo (0,1 s − 6 ms de folga da humanização)
    assert_eq!(peak(&a.samples[..(0.093 * 48_000.0) as usize * 2]), 0.0);
    assert!(peak(&a.samples[(0.094 * 48_000.0) as usize * 2..(0.2 * 48_000.0) as usize * 2]) > 0.05, "o 1º DON entra em 0,1 s");
    // a cauda morre antes do fim do arquivo
    assert!(peak(&a.samples[a.samples.len() - 9600..]) < 1e-3);
    // o limiter normalizou até o teto
    let lim = a.limiter.unwrap();
    assert!((a.peak() - 0.8414).abs() < 0.01, "pico {}", a.peak());
    assert!(lim.pre_gain > 1.0 && lim.pre_gain <= 4.0 && lim.min_gain < 1.0, "{lim:?}");

    // mesma semente → bit a bit o mesmo áudio (hash estável); outra semente → outra humanização
    let b = render(&song, bank(), &opts());
    assert_eq!(hash(&a.samples), hash(&b.samples));
    let c = render(&song, bank(), &RenderOptions { seed: 2027, ..opts() });
    assert_ne!(hash(&a.samples), hash(&c.samples));
    assert!((c.rms() / a.rms() - 1.0).abs() < 0.1, "mesma música, outra variação");
    println!("demo: hash {:016x} pico {:.4} rms {:.4} pre {:.3} min {:.3}", hash(&a.samples), a.peak(), a.rms(), lim.pre_gain, lim.min_gain);

    // regressão numérica (tolerante a diferenças de libm entre plataformas)
    assert!((a.rms() - GOLDEN_DEMO_RMS).abs() < 0.003, "rms {} ≠ {GOLDEN_DEMO_RMS}", a.rms());
    assert!((lim.peak_in - GOLDEN_DEMO_PEAK_IN).abs() < 0.01, "pico antes do limiter {} ≠ {GOLDEN_DEMO_PEAK_IN}", lim.peak_in);
}

/// valores do render da demo com semente 2026 (48 kHz, export limpo)
const GOLDEN_DEMO_RMS: f32 = 0.1594;
const GOLDEN_DEMO_PEAK_IN: f32 = 0.416;

#[test]
fn clean_export_vs_live_master_and_no_limiter() {
    let song = demo_song();
    let raw = render(&song, bank(), &RenderOptions { limiter_ceiling_db: None, ..opts() });
    assert!(raw.limiter.is_none());
    let clean = render(&song, bank(), &opts());
    // o limiter aplica o ganho de normalização e só segura os picos (no máx. ~5 dB)
    let lim = clean.limiter.unwrap();
    let ratio = clean.rms() / (raw.rms() * lim.pre_gain);
    assert!(ratio > 0.6 && ratio <= 1.0, "{ratio}");
    assert!((lim.peak_in - raw.peak()).abs() < 1e-6);
    assert!(lim.min_gain >= 0.5617, "nunca mais que 5 dB de limitação: {}", lim.min_gain);
    // cadeia ao vivo (compressor + soft clip): nunca passa de 0,98, e sai 6 ms atrasada (lookahead)
    let live = render(&song, bank(), &RenderOptions { master: MasterMode::Live, limiter_ceiling_db: None, ..opts() });
    assert!(live.samples.iter().all(|s| s.is_finite()));
    assert!(live.peak() < 0.98 && live.rms() > 0.01, "pico {}", live.peak());
    assert_eq!(peak(&live.samples[..(0.099 * 48_000.0) as usize * 2]), 0.0);
    // o compressor aperta a dinâmica: fator de crista menor que no export limpo
    assert!(live.peak() / live.rms() < raw.peak() / raw.rms());
}

#[test]
fn mute_solo_people_and_region_shape_the_render() {
    let mut song = demo_song();
    let full = render(&song, bank(), &RenderOptions { limiter_ceiling_db: None, ..opts() });
    // tudo mudo → silêncio digital
    for t in &mut song.tracks {
        t.muted = true;
    }
    let silent = render(&song, bank(), &RenderOptions { limiter_ceiling_db: None, ..opts() });
    assert_eq!((silent.hits, silent.peak()), (0, 0.0));
    assert_eq!(silent.frames(), full.frames());
    // solo do ōdaiko: 5 golpes, menos energia que a música inteira
    for t in &mut song.tracks {
        t.muted = false;
    }
    song.tracks[2].solo = true;
    let solo = render(&song, bank(), &RenderOptions { limiter_ceiling_db: None, ..opts() });
    assert_eq!(solo.hits, 5);
    assert!(solo.rms() > 0.001 && solo.rms() < full.rms());
    song.tracks[2].solo = false;
    // pessoa muda leva junto as linhas dela
    song.people = Some(vec![Person { id: "ana".into(), name: "Ana".into(), muted: Some(true), ..Default::default() }]);
    song.tracks[0].person_ids = Some(vec!["ana".into()]);
    song.tracks[1].person_ids = Some(vec!["ana".into()]);
    assert_eq!(render(&song, bank(), &opts()).hits, 5);
    // região: só os compassos 1..3 → 2 compassos de duração
    let song = demo_song();
    let part = render(&song, bank(), &RenderOptions { region: Some(Region { start_bar: 1, end_bar: 3 }), ..opts() });
    assert_eq!(part.frames(), ((0.1f64 + 4.8 + 3.0) * 48_000.0).ceil() as usize);
    assert_sane("região", &part);
    // metrônomo no render (opcional): soma os cliques
    let click = render(&song, bank(), &RenderOptions { metronome: true, limiter_ceiling_db: None, ..opts() });
    assert_ne!(hash(&click.samples), hash(&full.samples));
}

#[test]
fn other_sample_rates_and_kyai() {
    let mut song = demo_song();
    for (ci, syl) in ["so", "re", "so", "re!"].into_iter().enumerate() {
        set_kyai(&mut song, "chu1", 1, ci, Some(syl));
    }
    let with_kyai = render(&song, bank(), &RenderOptions { limiter_ceiling_db: None, ..opts() });
    let without = render(&demo_song(), bank(), &RenderOptions { limiter_ceiling_db: None, ..opts() });
    assert_eq!(with_kyai.frames(), without.frames());
    // o kiai é voz de fundo: muda o áudio do 2º compasso, sem dominar
    let bar2 = |r: &Rendered| r.samples[(2.5 * 48_000.0) as usize * 2..(3.7 * 48_000.0) as usize * 2].to_vec();
    let diff: Vec<f32> = bar2(&with_kyai).iter().zip(bar2(&without)).map(|(a, b)| a - b).collect();
    assert!(rms(&diff) > 0.002 && rms(&diff) < rms(&bar2(&without)), "kiai rms {} vs tambores {}", rms(&diff), rms(&bar2(&without)));
    assert!(with_kyai.samples.iter().all(|s| s.is_finite()));
    // fora do compasso do kiai nada muda (mesma semente)
    assert_eq!(hash(&with_kyai.samples[..(2.4 * 48_000.0) as usize * 2]), hash(&without.samples[..(2.4 * 48_000.0) as usize * 2]));

    // 44,1 kHz (o padrão do export da web): mesma duração e nível equivalente
    let r44 = render(&demo_song(), bank(), &RenderOptions { sample_rate: 44_100, ..opts() });
    let r48 = render(&demo_song(), bank(), &opts());
    assert_sane("44.1k", &r44);
    assert_eq!(r44.frames(), ((0.1f64 + 9.6 + 3.0) * 44_100.0).ceil() as usize);
    assert!((r44.rms() / r48.rms() - 1.0).abs() < 0.03, "{} vs {}", r44.rms(), r48.rms());
}

#[test]
fn fixture_yagyo_renders() {
    let song = yagyo();
    let r = render(&song, bank(), &opts());
    assert_sane("yagyo", &r);
    // 22 compassos tocados (18 + ritornello ×3 de 2 compassos) a 123 BPM
    let secs = 0.1 + 22.0 * 4.0 * 60.0 / 123.0 + 3.0;
    assert_eq!(r.frames(), (secs * 48_000.0f64).ceil() as usize);
    assert!(r.hits > 100);
    assert_eq!(hash(&r.samples), hash(&render(&song, bank(), &opts()).samples), "determinístico");
    println!("yagyo: {:.2} s, {} golpes, pico {:.4}, rms {:.4}, hash {:016x}", r.seconds(), r.hits, r.peak(), r.rms(), hash(&r.samples));
}

#[test]
fn progress_and_cancel() {
    let song = yagyo();
    let samples = SampleSet::from_bank(bank());
    let mut seen = Vec::new();
    let r = render_with(&song, bank(), &samples, &opts(), &mut |p| seen.push(p), &AtomicBool::new(false)).unwrap();
    assert!(seen.len() > 10 && seen.windows(2).all(|w| w[0] <= w[1]) && *seen.last().unwrap() == 1.0, "{seen:?}");
    assert!(seen[0] > 0.0 && seen[0] < 0.1);
    assert_eq!(hash(&r.samples), hash(&render(&song, bank(), &opts()).samples), "render e render_with dão o mesmo áudio");
    // cancelamento no meio: para na hora
    let cancel = Arc::new(AtomicBool::new(false));
    let c2 = cancel.clone();
    let mut calls = 0;
    let res = render_with(
        &song,
        bank(),
        &samples,
        &opts(),
        &mut |_| {
            calls += 1;
            if calls == 3 {
                c2.store(true, Ordering::Relaxed);
            }
        },
        &cancel,
    );
    assert_eq!(res, Err(Cancelled));
    assert_eq!(calls, 3);
}
