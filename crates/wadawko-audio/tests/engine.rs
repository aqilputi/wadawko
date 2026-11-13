//! Engine + Controller sem dispositivo: o teste faz o papel do callback
//! (`drain_commands` + `process`) e confere transport, eventos, mixer e previews.

mod common;

use std::sync::Arc;

use common::{bank, peak, rms};
use wadawko_audio::schedule::{PREVIEW_SLOT, hit_event};
use wadawko_audio::{
    Command, CompileOptions, Controller, Engine, EngineConfig, EngineEvent, EventKind, MasterMode, PPQ, PlayOptions, Region, SampleSet, TrainerOptions, engine_pair,
};
use wadawko_core::demo::demo_song;
use wadawko_core::model::{BUILTIN_INSTRUMENTS, Person, Section};
use wadawko_core::ops::cells::set_cell;
use wadawko_core::{Measure, Song, Zone};
use wadawko_samples::RoundRobin;

const SR: u32 = 48_000;

fn samples() -> Arc<SampleSet> {
    static S: std::sync::OnceLock<Arc<SampleSet>> = std::sync::OnceLock::new();
    S.get_or_init(|| SampleSet::from_bank(bank())).clone()
}

fn pair(master: MasterMode) -> (Engine, Controller) {
    engine_pair(bank(), samples(), EngineConfig { sample_rate: SR, seed: 7, master, metronome: false })
}

/// roda o "callback" por `secs` segundos em blocos de 256 frames
fn pump(eng: &mut Engine, secs: f64) -> Vec<f32> {
    let frames = (secs * SR as f64) as usize;
    let mut out = vec![0f32; frames * 2];
    for chunk in out.chunks_mut(512) {
        eng.drain_commands();
        eng.publish_clock(0);
        eng.process(chunk, 2);
    }
    out
}

fn hits(events: &[EngineEvent]) -> Vec<(u16, u32, u8, u64)> {
    events.iter().filter_map(|e| if let EngineEvent::HitFired { track, cell, hit, frame } = *e { Some((track, cell, hit, frame)) } else { None }).collect()
}

/// música de 1 linha de chū com as células dadas no 1º compasso
fn one_track(cells: &[(usize, &str)], bars: usize) -> Song {
    let mut song = demo_song();
    song.tracks.retain(|t| t.id == "chu1");
    song.measures = (0..bars).map(|_| Measure::new(4, 2, ["chu1"])).collect();
    for &(ci, syl) in cells {
        set_cell(&mut song, "chu1", 0, ci, Some(syl), None);
    }
    song
}

#[test]
fn plays_to_the_end_and_reports_every_hit() {
    let (mut eng, mut ctl) = pair(MasterMode::Live);
    let song = demo_song();
    let schedule = ctl.set_song(&song, &CompileOptions::default());
    assert_eq!(ctl.play(PlayOptions::default()), 100.0);
    let out = pump(&mut eng, 11.0);
    let events = ctl.poll_events();
    let fired = hits(&events);
    assert_eq!(fired.len(), 67, "um HitFired por golpe");
    // mesma ordem e mesmas chaves do schedule; o frame cresce com o tick
    let expect: Vec<(u16, u32, u8)> = schedule.hits().map(|(_, h)| (h.track, h.cell, h.hit)).collect();
    assert_eq!(fired.iter().map(|h| (h.0, h.1, h.2)).collect::<Vec<_>>(), expect);
    assert!(fired.windows(2).all(|w| w[0].3 <= w[1].3));
    // 1º golpe: antecedência da humanização (6 ms) + lookahead do compressor (6 ms)
    assert_eq!(fired[0].3, 288 + 288);
    // tick → frame: 100 BPM = 28800 frames por tempo
    let (t5, _) = schedule.hits().nth(5).unwrap();
    assert!((fired[5].3 as f64 - (576.0 + t5 / PPQ * 28_800.0)).abs() <= 1.0);
    let ended: Vec<_> = events.iter().filter(|e| matches!(e, EngineEvent::Ended { .. })).collect();
    assert_eq!(ended.len(), 1);
    assert!(!eng.is_playing() && eng.pos_tick() == 0.0);
    assert!(!ctl.clock().snapshot().playing);
    // áudio: soa, sem NaN, dentro do teto do soft clip
    assert!(out.iter().all(|s| s.is_finite()));
    assert!(rms(&out) > 0.01 && peak(&out) < 0.98, "rms {} pico {}", rms(&out), peak(&out));
    // depois do fim, só a cauda: 4 s depois está em silêncio
    let tail = pump(&mut eng, 4.0);
    assert!(peak(&tail[tail.len() - 9600..]) < 1e-4);
    assert_eq!(eng.active_voices(), 0);
}

#[test]
fn loop_wraps_seamlessly() {
    let (mut eng, mut ctl) = pair(MasterMode::Clean);
    let song = one_track(&[(0, "DON"), (4, "don")], 1);
    ctl.set_song(&song, &CompileOptions::default());
    ctl.play(PlayOptions { looping: true, ..Default::default() });
    pump(&mut eng, 2.4 * 3.0 + 0.5);
    let events = ctl.poll_events();
    let wraps: Vec<(u32, u64)> = events.iter().filter_map(|e| if let EngineEvent::LoopWrapped { wraps, frame } = *e { Some((wraps, frame)) } else { None }).collect();
    assert_eq!(wraps.iter().map(|w| w.0).collect::<Vec<_>>(), vec![1, 2, 3]);
    // 1 compasso a 100 BPM = 2,4 s = 115200 frames; a volta é exata no frame
    assert_eq!(wraps[0].1, 115_200 + 288);
    assert_eq!(wraps[1].1 - wraps[0].1, 115_200);
    let fired = hits(&events);
    assert_eq!(fired.len(), 7);
    // o DON de cada volta soa exatamente 1 compasso depois do anterior
    let dons: Vec<u64> = fired.iter().filter(|h| h.1 == 0).map(|h| h.3).collect();
    assert!(dons.windows(2).all(|w| w[1] - w[0] == 115_200), "{dons:?}");
    assert!(eng.is_playing());
    assert_eq!(ctl.clock().snapshot().wraps, 3);
    // desligar o loop ao vivo: termina no fim desta volta
    ctl.set_loop(false);
    pump(&mut eng, 2.5);
    assert!(ctl.poll_events().iter().any(|e| matches!(e, EngineEvent::Ended { .. })));
}

#[test]
fn seek_stop_pause_and_live_bpm() {
    let (mut eng, mut ctl) = pair(MasterMode::Clean);
    let song = demo_song();
    let schedule = ctl.set_song(&song, &CompileOptions::default());
    // começa da célula 8 (2º compasso)
    ctl.play(PlayOptions { start_cell: 8, ..Default::default() });
    pump(&mut eng, 0.5);
    let fired = hits(&ctl.poll_events());
    assert!(!fired.is_empty() && fired.iter().all(|h| h.1 >= 8));
    // seek ao vivo pra célula 24 (4º compasso)
    ctl.seek_cell(24);
    pump(&mut eng, 0.3);
    assert!(hits(&ctl.poll_events()).iter().all(|h| h.1 >= 24));
    let pos = eng.pos_tick();
    assert!(pos > schedule.time.cell_tick(24) && pos < schedule.time.cell_tick(24) + PPQ);
    // pause mantém a posição; resume continua
    ctl.pause();
    pump(&mut eng, 0.2);
    let held = eng.pos_tick();
    pump(&mut eng, 0.2);
    assert_eq!(eng.pos_tick(), held);
    assert!(!eng.is_playing());
    ctl.resume();
    // BPM ao vivo: a 200 BPM o transport anda 2× mais ticks por segundo
    ctl.set_bpm(200.0);
    pump(&mut eng, 0.25);
    assert!((eng.pos_tick() - held - 0.25 * 200.0 / 60.0 * PPQ).abs() < 30.0, "{} {}", eng.pos_tick(), held);
    assert_eq!(ctl.clock().bpm(), 200.0);
    // stop volta ao início
    ctl.stop();
    pump(&mut eng, 0.1);
    assert!(!eng.is_playing() && eng.pos_tick() == 0.0);
    assert!(hits(&ctl.poll_events()).iter().all(|h| h.1 >= 24));
}

#[test]
fn count_in_plays_four_clicks_before_starting() {
    let (mut eng, mut ctl) = pair(MasterMode::Clean);
    let song = one_track(&[(0, "DON")], 1);
    ctl.set_song(&song, &CompileOptions::default());
    ctl.play(PlayOptions { count_in: true, ..Default::default() });
    // 4 tempos a 100 BPM = 2,4 s de contagem
    let pre = pump(&mut eng, 2.3);
    assert!(hits(&ctl.poll_events()).is_empty(), "nada toca durante a contagem");
    let snap = ctl.clock().snapshot();
    assert!(snap.playing && snap.pos_tick == 0.0 && snap.preroll_frames > 0);
    assert_eq!(ctl.clock().audible_tick(0.0), 0.0);
    // 4 cliques: energia nos 80 ms depois de cada tempo, silêncio entre eles
    for i in 0..4 {
        let at = (i as f64 * 0.6 * SR as f64) as usize + 288;
        assert!(peak(&pre[at * 2..(at + 2400) * 2]) > 0.05, "clique {i}");
        assert!(peak(&pre[(at + 6000) * 2..(at + 20_000) * 2]) < 1e-5, "silêncio depois do clique {i}");
    }
    // o acento (C6, velocity 1) é mais forte que os demais (G5, 0.6)
    assert!(peak(&pre[288 * 2..2688 * 2]) > 1.4 * peak(&pre[(28_800 + 288) * 2..(28_800 + 2688) * 2]));
    pump(&mut eng, 0.3);
    let fired = hits(&ctl.poll_events());
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].3, 115_200 + 288, "o 1º tempo cai exatamente 4 tempos depois do 1º clique");
    // parar no meio da contagem cancela os cliques que faltam
    ctl.stop();
    pump(&mut eng, 1.5);
    ctl.play(PlayOptions { count_in: true, ..Default::default() });
    pump(&mut eng, 0.3);
    ctl.stop();
    pump(&mut eng, 0.3);
    let after = pump(&mut eng, 2.0);
    assert!(peak(&after[9600..]) < 1e-5);
}

#[test]
fn trainer_raises_bpm_each_lap() {
    let (mut eng, mut ctl) = pair(MasterMode::Clean);
    let mut song = one_track(&[(0, "don")], 2);
    song.tempo = 240.0;
    let trainer = Some(TrainerOptions { start_pct: 0.5, step: 0.25 });
    // sem região o treino não vale (como no web app)
    ctl.set_song(&song, &CompileOptions::default());
    assert_eq!(ctl.play(PlayOptions { looping: true, trainer, ..Default::default() }), 240.0);
    ctl.stop();
    ctl.set_song(&song, &CompileOptions { region: Some(Region { start_bar: 0, end_bar: 1 }), ..Default::default() });
    assert_eq!(ctl.play(PlayOptions { looping: true, trainer, ..Default::default() }), 120.0);
    // voltas de 1 compasso: 2 s a 120, 1,33 s a 180, 1 s a 240…
    pump(&mut eng, 6.5);
    let events = ctl.poll_events();
    let bpms: Vec<f32> = events.iter().filter_map(|e| if let EngineEvent::BpmChanged { bpm } = *e { Some(bpm) } else { None }).collect();
    assert_eq!(bpms, vec![180.0, 240.0], "sobe 25 % do tempo base por volta e para no tempo base");
    assert_eq!(eng.bpm(), 240.0);
    let wraps: Vec<u64> = events.iter().filter_map(|e| if let EngineEvent::LoopWrapped { frame, .. } = *e { Some(frame) } else { None }).collect();
    assert_eq!(wraps[0], 96_000 + 288);
    assert_eq!(wraps[1] - wraps[0], 64_000);
    assert_eq!(wraps[2] - wraps[1], 48_000);
}

#[test]
fn metronome_toggles_live() {
    let (mut eng, mut ctl) = pair(MasterMode::Clean);
    let song = one_track(&[], 2);
    ctl.set_song(&song, &CompileOptions::default());
    ctl.play(PlayOptions { looping: true, ..Default::default() });
    assert_eq!(peak(&pump(&mut eng, 2.0)), 0.0, "música vazia e metrônomo desligado");
    ctl.set_metronome(true);
    let on = pump(&mut eng, 2.4);
    assert!(peak(&on) > 0.05);
    ctl.set_metronome(false);
    pump(&mut eng, 0.2);
    assert_eq!(peak(&pump(&mut eng, 2.0)), 0.0);
}

fn stereo_rms(out: &[f32]) -> (f32, f32) {
    let l: Vec<f32> = out.iter().step_by(2).copied().collect();
    let r: Vec<f32> = out.iter().skip(1).step_by(2).copied().collect();
    (rms(&l), rms(&r))
}

#[test]
fn mixer_volume_pan_mute_and_person_bus() {
    let play = |song: &Song| {
        let (mut eng, mut ctl) = pair(MasterMode::Clean);
        ctl.set_song(song, &CompileOptions::default());
        ctl.play(PlayOptions::default());
        (stereo_rms(&pump(&mut eng, 2.0)), eng, ctl)
    };
    let mut song = one_track(&[(0, "DON"), (2, "DON"), (4, "DON"), (6, "DON")], 1);
    let ((l0, r0), ..) = play(&song);
    assert!(l0 > 0.005 && (l0 / r0 - 1.0).abs() < 1e-3, "pan central: L = R ({l0} {r0})");
    // volume: −6 dB (default) → −12 dB = metade (a humanização usa a mesma semente)
    song.tracks[0].volume = Some(-12.0);
    let ((l1, _), ..) = play(&song);
    assert!((l1 / l0 - 0.501).abs() < 0.01, "{}", l1 / l0);
    song.tracks[0].volume = None;
    // pan todo à esquerda: R mudo e L com +3 dB (potência constante)
    song.tracks[0].pan = Some(-1.0);
    let ((l2, r2), ..) = play(&song);
    assert!(r2 < 1e-6 && (l2 / l0 - std::f32::consts::SQRT_2).abs() < 0.01, "{l2} {r2}");
    // com pessoa: o barramento (Tone.Channel mono) centraliza e tira 3 dB — igual ao web app
    song.people = Some(vec![Person { id: "ana".into(), name: "Ana".into(), ..Default::default() }]);
    song.tracks[0].person_ids = Some(vec!["ana".into()]);
    let ((l3, r3), mut eng, mut ctl) = play(&song);
    assert!((l3 / r3 - 1.0).abs() < 1e-3 && (l3 / l0 - 0.5).abs() < 0.01, "{l3} {r3} {l0}");
    // mudo da pessoa ao vivo, sem recompilar (applyMix)
    song.people.as_mut().unwrap()[0].muted = Some(true);
    ctl.apply_mix(&song);
    ctl.play(PlayOptions::default());
    pump(&mut eng, 0.1);
    assert!(peak(&pump(&mut eng, 1.5)) < 1e-6);
    // desmuta a pessoa e muta a linha: continua mudo (2 camadas); solo de outra linha idem
    song.people.as_mut().unwrap()[0].muted = None;
    song.tracks[0].muted = true;
    ctl.apply_mix(&song);
    ctl.play(PlayOptions::default());
    pump(&mut eng, 0.1);
    assert!(peak(&pump(&mut eng, 1.5)) < 1e-6);
    song.tracks[0].muted = false;
    ctl.apply_mix(&song);
    ctl.play(PlayOptions::default());
    assert!(peak(&pump(&mut eng, 1.5)) > 0.01);
    // volume geral em zero = silêncio total
    ctl.set_master_gain(0.0);
    ctl.play(PlayOptions::default());
    pump(&mut eng, 0.1);
    assert_eq!(peak(&pump(&mut eng, 1.0)), 0.0);
}

/// dispara golpes avulsos (sem humanização) e devolve o áudio
fn trigger_render(hits: &[(Zone, f32, f32)], secs: f64) -> Vec<f32> {
    let (mut eng, mut ctl) = pair(MasterMode::Clean);
    let def = BUILTIN_INSTRUMENTS.iter().find(|d| d.id == "odaiko").unwrap();
    let mut rr = RoundRobin::new();
    for &(zone, gain_mul, delay) in hits {
        let mut ev = hit_event(bank(), def, zone, 1.0, 1.0, &mut rr, PREVIEW_SLOT * 2 + (zone == Zone::Rim) as u16);
        ev.humanize = false;
        ev.gain *= gain_mul;
        ctl.send(Command::Trigger { kind: EventKind::Hit(ev), delay });
    }
    pump(&mut eng, secs)
}

#[test]
fn choke_head_mutes_head_and_rim_but_rim_only_rim() {
    let window = |out: &[f32]| rms(&out[(0.5 * SR as f64) as usize * 2..(1.2 * SR as f64) as usize * 2]);
    // ōdaiko: cauda longa (TAIL 2,8 s); choke de 140 ms
    let alone = window(&trigger_render(&[(Zone::Head, 1.0, 0.0)], 1.5));
    assert!(alone > 0.002, "{alone}");
    // um 2º golpe de couro (quase mudo) 0,3 s depois abafa a ressonância do 1º
    let choked = window(&trigger_render(&[(Zone::Head, 1.0, 0.0), (Zone::Head, 1e-6, 0.3)], 1.5));
    assert!(choked < alone * 0.01, "couro abafa couro: {choked} vs {alone}");
    // um golpe de borda NÃO abafa o couro
    let rim_after = window(&trigger_render(&[(Zone::Head, 1.0, 0.0), (Zone::Rim, 1e-6, 0.3)], 1.5));
    assert!((rim_after / alone - 1.0).abs() < 0.01, "borda não abafa couro: {rim_after} vs {alone}");
    // couro abafa a borda anterior; borda abafa borda
    let w = |out: &[f32]| rms(&out[(0.5 * SR as f64) as usize * 2..(0.75 * SR as f64) as usize * 2]);
    let rim_alone = w(&trigger_render(&[(Zone::Rim, 1.0, 0.0)], 1.0));
    assert!(rim_alone > 1e-4, "{rim_alone}");
    assert!(w(&trigger_render(&[(Zone::Rim, 1.0, 0.0), (Zone::Head, 1e-6, 0.3)], 1.0)) < rim_alone * 0.01);
    assert!(w(&trigger_render(&[(Zone::Rim, 1.0, 0.0), (Zone::Rim, 1e-6, 0.3)], 1.0)) < rim_alone * 0.01);
    // o abafamento é uma rampa (não um corte seco): no meio dos 140 ms ainda há som
    let out = trigger_render(&[(Zone::Head, 1.0, 0.0), (Zone::Head, 1e-6, 0.3)], 1.0);
    let mid = (0.3 + 0.006 + 0.05) * SR as f64;
    assert!(peak(&out[mid as usize * 2..(mid as usize + 480) * 2]) > 1e-3);
    // TAIL: sozinho, o ōdaiko some depois de 2,8 s (fade de 250 ms no fim)
    let long = trigger_render(&[(Zone::Head, 1.0, 0.0)], 3.2);
    assert!(peak(&long[(2.5 * SR as f64) as usize * 2..(2.6 * SR as f64) as usize * 2]) > 1e-4);
    assert!(peak(&long[(2.85 * SR as f64) as usize * 2..]) < 1e-5);
}

#[test]
fn humanization_varies_gain_pitch_and_time_within_bounds() {
    // o mesmo golpe 200× com humanização: o instante do pico e o nível variam, dentro dos limites
    let def = BUILTIN_INSTRUMENTS.iter().find(|d| d.id == "shime").unwrap();
    let (mut eng, mut ctl) = pair(MasterMode::Clean);
    let (mut first_frames, mut peaks) = (Vec::new(), Vec::new());
    for _ in 0..200 {
        let mut rr = RoundRobin::new();
        let ev = hit_event(bank(), def, Zone::Head, 0.7, 1.0, &mut rr, PREVIEW_SLOT * 2);
        ctl.send(Command::Trigger { kind: EventKind::Hit(ev), delay: 0.0 });
        let out = pump(&mut eng, 0.6);
        first_frames.push(out.iter().position(|s| s.abs() > 1e-6).unwrap() / 2);
        peaks.push(peak(&out));
    }
    let (lo, hi) = (*first_frames.iter().min().unwrap(), *first_frames.iter().max().unwrap());
    // antecedência de 6 ms ± 6 ms → entre 0 e 12 ms (576 frames), bem espalhado
    assert!(hi <= 580 && hi - lo > 400, "micro-timing {lo}..{hi}");
    let (pl, ph) = peaks.iter().fold((f32::MAX, 0f32), |(a, b), p| (a.min(*p), b.max(*p)));
    assert!(ph / pl > 1.15 && ph / pl < 1.36, "ganho ±12 % (e um pouco do pitch): {}", ph / pl);
}

#[test]
fn previews_cell_instrument_phrase_kyai_and_fue() {
    let (mut eng, mut ctl) = pair(MasterMode::Live);
    let song = demo_song();
    ctl.set_song(&song, &CompileOptions::default());
    // célula: toca pelo canal da linha, sem transport e sem HitFired
    ctl.preview_cell(&song, &song.tracks[1], "doko");
    let out = pump(&mut eng, 0.8);
    assert!(peak(&out) > 0.02 && !eng.is_playing());
    assert!(ctl.poll_events().is_empty());
    // 2 golpes: o 2º a hit.offset × 0,2 s = 100 ms (± humanização)
    let late = &out[(0.1 * SR as f64) as usize * 2..(0.2 * SR as f64) as usize * 2];
    assert!(peak(late) > 0.01);
    // sílaba inválida não toca
    ctl.preview_cell(&song, &song.tracks[1], "xyz");
    pump(&mut eng, 2.0);
    assert!(peak(&pump(&mut eng, 0.5)) < 1e-4);
    // zona de uma def do kit (canal de preview)
    let def = BUILTIN_INSTRUMENTS.iter().find(|d| d.id == "preset-okedo-brilhante").unwrap();
    ctl.preview_instrument(def, Zone::Rim);
    assert!(peak(&pump(&mut eng, 0.5)) > 0.01);
    pump(&mut eng, 2.0);
    // loop de frase: continua tocando volta após volta até parar
    ctl.start_phrase_loop(def, &["don", "doko", "don", "kara"], 0.15, 0.0);
    pump(&mut eng, 0.3);
    for lap in 0..6 {
        assert!(peak(&pump(&mut eng, 0.6)) > 0.01, "volta {lap}");
    }
    ctl.stop_phrase_loop();
    pump(&mut eng, 0.05);
    assert!(peak(&pump(&mut eng, 0.5)) < 1e-4, "parar mata a cauda");
    // kiai: voz da linha; 3 sílabas a cada 0,25 s
    ctl.preview_kyai(&song.tracks[1], &["so", "re", "ya!"], 0.25, None);
    let voice = pump(&mut eng, 1.2);
    for i in 0..3 {
        let at = ((0.03 + 0.012 + i as f64 * 0.25) * SR as f64) as usize;
        assert!(peak(&voice[(at + 2400) * 2..(at + 7200) * 2]) > 0.003, "sílaba {i}");
    }
    assert!(peak(&pump(&mut eng, 0.5)) < 1e-4);
    // easter egg: ~7,6 s de melodia; a 2ª chamada enquanto toca devolve 0
    let ms = ctl.play_fue();
    assert_eq!(ms, 7560);
    assert_eq!(ctl.play_fue(), 0);
    let fue = pump(&mut eng, 8.0);
    assert!(rms(&fue[..SR as usize * 2 * 7]) > 0.02);
    assert!(fue.iter().all(|s| s.is_finite()));
    assert!(peak(&pump(&mut eng, 1.5)[SR as usize..]) < 1e-4, "termina sozinho");
    assert!(ctl.poll_events().is_empty());
}

#[test]
fn schedule_swap_keeps_position_and_returns_garbage() {
    let (mut eng, mut ctl) = pair(MasterMode::Clean);
    let mut song = demo_song();
    let first = ctl.set_song(&song, &CompileOptions::default());
    ctl.play(PlayOptions { looping: true, ..Default::default() });
    pump(&mut eng, 1.0);
    let before = eng.pos_tick();
    // edição durante o playback: recompila e troca; a posição continua
    set_cell(&mut song, "odaiko", 0, 6, Some("DON"), None);
    let second = ctl.set_song(&song, &CompileOptions::default());
    pump(&mut eng, 0.5);
    assert!((eng.pos_tick() - before - 0.5 * 100.0 / 60.0 * PPQ).abs() < 20.0);
    // o schedule antigo voltou pela fila de lixo e é solto aqui, fora do "callback"
    assert_eq!(Arc::strong_count(&first), 2, "ainda na fila de lixo");
    let events = ctl.poll_events();
    assert_eq!(Arc::strong_count(&first), 1);
    assert_eq!(Arc::strong_count(&second), 3, "controller + engine + este teste");
    // a nota nova (célula 6 do ōdaiko, em 1,8 s) toca na 1ª passada; nada é repetido
    pump(&mut eng, 1.0);
    let mut all = hits(&events);
    all.extend(hits(&ctl.poll_events()));
    assert_eq!(all.iter().filter(|h| h.0 == 2 && h.1 == 6).count(), 1);
    let mut keys: Vec<_> = all.iter().map(|h| (h.0, h.1, h.2)).collect();
    let n = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), n);
    // música mais curta que a posição atual: dá a volta em vez de travar
    pump(&mut eng, 5.0);
    song.measures.truncate(1);
    song.sections = Some(vec![Section { id: "a".into(), name: "A".into(), start: 0, ..Default::default() }]);
    ctl.set_song(&song, &CompileOptions::default());
    pump(&mut eng, 0.5);
    assert!(eng.is_playing() && eng.pos_tick() < 4.0 * PPQ);
    // schedule vazio durante o playback: termina
    song.measures.clear();
    ctl.set_song(&song, &CompileOptions::default());
    pump(&mut eng, 0.1);
    assert!(!eng.is_playing());
    assert!(ctl.poll_events().iter().any(|e| matches!(e, EngineEvent::Ended { .. })));
}

#[test]
fn clock_extrapolates_audible_position() {
    let (mut eng, mut ctl) = pair(MasterMode::Live);
    ctl.set_song(&demo_song(), &CompileOptions::default());
    ctl.play(PlayOptions { looping: true, ..Default::default() });
    let clock = ctl.clock().clone();
    // 12 ms de atraso fixo: antecedência da humanização + lookahead do compressor
    assert_eq!(clock.fixed_delay_ns.load(std::sync::atomic::Ordering::Relaxed), 12_000_000);
    let mut out = vec![0f32; 512];
    for _ in 0..100 {
        eng.drain_commands();
        eng.publish_clock(clock.now_ns());
        eng.process(&mut out, 2);
    }
    let snap = clock.snapshot();
    assert!(snap.playing && snap.looping && snap.len_ticks == 16.0 * PPQ && snap.bpm == 100.0);
    // 99 blocos de 256 frames processados antes da última publicação
    assert_eq!(snap.frames, 99 * 256);
    assert!((snap.pos_tick - 99.0 * 256.0 / 48_000.0 * 100.0 / 60.0 * PPQ).abs() < 1e-6);
    // audível = posição do bloco + tempo decorrido − 12 ms (aqui o decorrido é ~0)
    let audible = clock.audible_tick(0.0);
    let lag_ticks = 0.012 * 100.0 / 60.0 * PPQ;
    assert!(audible > snap.pos_tick - lag_ticks - 1.0 && audible < snap.pos_tick + 40.0, "{audible} vs {}", snap.pos_tick);
    // o offset do usuário atrasa o playhead
    assert!(clock.audible_tick(50.0) < audible - 70.0);
    assert!(clock.audible_frame(0.0) >= snap.frames as f64);
}

#[test]
fn mono_and_multichannel_outputs() {
    let (mut eng, mut ctl) = pair(MasterMode::Clean);
    ctl.set_song(&demo_song(), &CompileOptions::default());
    ctl.play(PlayOptions::default());
    eng.drain_commands();
    let mut six = vec![1f32; 6 * 4800];
    eng.process(&mut six, 6);
    assert!(six.chunks(6).any(|f| f[0] != 0.0) && six.chunks(6).all(|f| f[0] == f[1] && f[2..] == [0.0; 4]));
    let mut mono = vec![1f32; 4800];
    eng.process(&mut mono, 1);
    assert!(peak(&mono) > 0.0);
}
