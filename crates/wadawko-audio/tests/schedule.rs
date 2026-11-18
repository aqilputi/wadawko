//! Compilação do schedule (sem dispositivo de áudio): ordem, ticks,
//! ritornello, swing, mudo/solo, dinâmica, roteamento, kiai — sobre a demo e
//! as músicas de `crates/wadawko-core/tests/fixtures/`.

mod common;

use common::{bank, big_song, fixtures, yagyo};
use wadawko_audio::schedule::{PREVIEW_KYAI_SLOT, channel_key, zone_chain};
use wadawko_audio::{BAR_TICKS, CompileOptions, Compiler, EventKind, HitEv, PPQ, Region, Schedule, compile, sfz_dyn, vel_gain};
use wadawko_core::demo::demo_song;
use wadawko_core::model::{DynamicMark, Person, Section};
use wadawko_core::ops::cells::{set_cell, set_kyai};
use wadawko_core::phrase::resolve_cell;
use wadawko_core::timeline::{play_order, swing_warp};
use wadawko_core::{Dynamic, Hairpin, Song, Zone};

const SR: u32 = 48_000;

fn live(song: &Song) -> Schedule {
    compile(song, bank(), SR, &CompileOptions::default())
}

fn export(song: &Song) -> Schedule {
    compile(song, bank(), SR, &CompileOptions { skip_inaudible: true, ..Default::default() })
}

fn hits_of(s: &Schedule, track: u16) -> Vec<(f64, HitEv)> {
    s.hits().filter(|(_, h)| h.track == track).map(|(t, h)| (t, *h)).collect()
}

/// contagem independente: golpes de todas as células na ordem de execução
fn expected_hits(song: &Song, region: Option<Region>) -> usize {
    play_order(song, region)
        .iter()
        .map(|&mi| {
            let m = &song.measures[mi];
            song.tracks.iter().map(|t| (0..m.cell_count()).filter_map(|ci| m.cell(&t.id, ci)).filter_map(resolve_cell).map(|r| r.hits.len()).sum::<usize>()).sum::<usize>()
        })
        .sum()
}

fn assert_well_formed(label: &str, song: &Song, s: &Schedule) {
    assert!(s.events.windows(2).all(|w| w[0].tick <= w[1].tick), "{label}: eventos fora de ordem");
    assert_eq!(s.len_ticks, s.time.order.len() as f64 * BAR_TICKS, "{label}");
    for e in &s.events {
        assert!(e.tick >= 0.0 && e.tick < s.len_ticks, "{label}: tick {} fora de [0, {})", e.tick, s.len_ticks);
        match e.kind {
            EventKind::Hit(h) => {
                assert!(h.gain.is_finite() && h.gain > 0.0 && h.rate > 0.0 && h.dur > 0.0, "{label}: {h:?}");
                assert!(h.fade >= 0.0 && h.fade <= h.dur / 2.0 + 1e-6);
                let ch = &s.channels[(h.strip / 2) as usize];
                assert!(ch.is_used() && !ch.kyai && ch.track == h.track, "{label}: canal errado");
                assert_eq!(ch.track_id, song.tracks[h.track as usize].id);
                assert!((ch.bus as usize) < s.buses.len());
                assert!(h.strip / 2 < PREVIEW_KYAI_SLOT);
            }
            EventKind::Grain(g) => assert!(s.channels[(g.strip / 2) as usize].kyai && g.dur > 0.0 && g.gain.is_finite()),
            EventKind::Noise(n) => assert!(s.channels[(n.strip / 2) as usize].kyai && n.dur > 0.0),
            EventKind::Click { .. } => {}
        }
    }
}

#[test]
fn demo_order_ticks_and_counts() {
    let song = demo_song();
    let s = live(&song);
    assert_well_formed("demo", &song, &s);
    assert_eq!(s.time.order, vec![0, 1, 2, 3]);
    assert_eq!(s.len_ticks, 4.0 * 4.0 * PPQ);
    assert_eq!(s.tempo, 100.0);
    assert!((s.seconds(100.0) - 9.6).abs() < 1e-9);
    assert_eq!(s.hits().count(), expected_hits(&song, None));
    // shime 8+9+8+10, chū 5+7+9+6 ("su" é pausa), ōdaiko 1+1+1+2
    assert_eq!(hits_of(&s, 0).len(), 35);
    assert_eq!(hits_of(&s, 1).len(), 27);
    assert_eq!(hits_of(&s, 2).len(), 5);
    // metrônomo: 4 cliques por compasso, acento no tempo 1
    let clicks: Vec<(f64, bool)> = s.events.iter().filter_map(|e| if let EventKind::Click { beat, sub: 0 } = e.kind { Some((e.tick, beat == 0)) } else { None }).collect();
    assert_eq!(clicks.len(), 16);
    assert!(clicks.iter().all(|&(t, a)| t % PPQ == 0.0 && a == (t % BAR_TICKS == 0.0)));
    // chū, compasso 1: DON · don · doko don → colcheias de 480 ticks; "doko" = 2 golpes na célula
    let chu = hits_of(&s, 1);
    let ticks: Vec<f64> = chu.iter().take(5).map(|(t, _)| *t).collect();
    assert_eq!(ticks, vec![0.0, 960.0, 1920.0, 2160.0, 2400.0]);
    assert_eq!(chu.iter().take(5).map(|(_, h)| (h.cell, h.hit)).collect::<Vec<_>>(), vec![(0, 0), (2, 0), (4, 0), (4, 1), (5, 0)]);
    // 2º compasso: "ka ka" na borda (strip ímpar), célula global 8+4
    let ka = chu.iter().find(|(_, h)| h.rim).unwrap();
    assert_eq!((ka.0, ka.1.cell, ka.1.strip % 2), (BAR_TICKS + 1920.0, 12, 1));
    // ganho: TUNING × dinâmica (mf = 1) × velGain(vel); DON = 1.0, don = 0.7
    assert!((chu[0].1.gain as f64 - vel_gain(1.0)).abs() < 1e-6);
    assert!((chu[1].1.gain as f64 - vel_gain(0.7)).abs() < 1e-6);
    assert!((vel_gain(0.7) - 0.865).abs() < 1e-12);
    assert!((hits_of(&s, 0)[0].1.gain as f64 - 1.2 * 0.865).abs() < 1e-6, "shime headGain 1.2");
}

#[test]
fn vel_gain_and_sfz_formulas() {
    // ghost (vel 0.05) ≈ −18 dB do don; piso de 10 % em vel 0
    let ghost_db = 20.0 * (vel_gain(0.05) / vel_gain(0.7)).log10();
    assert!((-21.0..-15.0).contains(&ghost_db), "{ghost_db}");
    assert!((vel_gain(0.0) - 0.1 * vel_gain(0.5)).abs() < 1e-12);
    assert!((20.0 * (vel_gain(1.0) / vel_gain(0.7)).log10() - 4.03).abs() < 0.05, "DON ≈ +4 dB");
    // sfz: nunca abaixo de f (2.0), dobro da dinâmica, teto ff × 1.15
    assert_eq!(sfz_dyn(0.25), 2.0);
    assert_eq!(sfz_dyn(1.0), 2.0);
    assert_eq!(sfz_dyn(1.5), 3.0);
    assert!((sfz_dyn(3.2) - 3.68).abs() < 1e-12);
}

#[test]
fn layers_round_robin_and_tail() {
    let mut song = demo_song();
    song.measures.truncate(1);
    // 8 "don" seguidos no chū: mesma camada, round robin alternando 2 samples
    for ci in 0..8 {
        set_cell(&mut song, "chu1", 0, ci, Some("don"), None);
    }
    let s = live(&song);
    let chu = hits_of(&s, 1);
    let ids: Vec<u16> = chu.iter().map(|(_, h)| h.sample).collect();
    assert_eq!(ids[0], ids[2]);
    assert_ne!(ids[0], ids[1]);
    assert_eq!(ids[1], ids[3]);
    // vel·√dyn escolhe a camada: don (0.7) em mf → camada forte do chū (2 camadas, corte em 0.5);
    // em pp (0.7·√0.25 = 0.35) → camada suave
    let name = |id: u16| wadawko_samples::SampleId(id).name();
    assert!(name(ids[0]).starts_with("chu-head-1-"), "{}", name(ids[0]));
    song.measures[0].dynamic = Some(Dynamic::Pp);
    let pp = live(&song);
    assert!(name(hits_of(&pp, 1)[0].1.sample).starts_with("chu-head-0-"));
    // TAIL: a cauda do chū é limitada a 1,3 s (fade longo de 0,25 s); o shime a 0,45 s
    let h = chu[0].1;
    let natural = bank().sample(wadawko_samples::SampleId(h.sample)).duration_secs();
    if natural > 1.31 {
        assert_eq!((h.dur, h.fade), (1.3, 0.25));
    } else {
        assert_eq!(h.fade, 0.0);
    }
    assert!((h.choke - 0.09).abs() < 1e-6);
    let shime = hits_of(&s, 0)[0].1;
    assert!(shime.dur <= 0.45 + 1e-6 && (shime.choke - 0.04).abs() < 1e-6);
}

#[test]
fn ritornello_and_region() {
    let mut song = demo_song();
    song.sections = Some(vec![
        Section { id: "a".into(), name: "A".into(), start: 0, repeat: Some(2), ..Default::default() },
        Section { id: "b".into(), name: "B".into(), start: 2, repeat: None, ..Default::default() },
    ]);
    let s = live(&song);
    assert_well_formed("ritornello", &song, &s);
    assert_eq!(s.time.order, vec![0, 1, 0, 1, 2, 3]);
    assert_eq!(s.len_ticks, 6.0 * BAR_TICKS);
    assert_eq!(s.hits().count(), expected_hits(&song, None));
    // a repetição toca as MESMAS células-fonte, 2 compassos depois
    let chu = hits_of(&s, 1);
    let first: Vec<_> = chu.iter().filter(|(t, _)| *t < 2.0 * BAR_TICKS).map(|(t, h)| (*t, h.cell, h.hit)).collect();
    let second: Vec<_> = chu.iter().filter(|(t, _)| (2.0 * BAR_TICKS..4.0 * BAR_TICKS).contains(t)).map(|(t, h)| (*t - 2.0 * BAR_TICKS, h.cell, h.hit)).collect();
    assert_eq!(first.len(), 12);
    assert_eq!(first, second);
    // playhead dobrado: no 3º compasso tocado estamos de volta ao compasso-fonte 0
    assert_eq!(s.time.fold_tick(2.0 * BAR_TICKS + 100.0), 100.0);
    assert_eq!(s.time.fold_tick(4.0 * BAR_TICKS + 100.0), 2.0 * BAR_TICKS + 100.0);
    assert_eq!(s.time.cell_at(2.0 * BAR_TICKS + 480.0), Some(1));
    assert_eq!(s.time.cell_at(5.0 * BAR_TICKS + 3.5 * PPQ), Some(31));
    assert_eq!(s.time.cell_at(6.0 * BAR_TICKS), None);
    // seek: 1ª vez que o compasso toca
    assert_eq!(s.time.cell_tick(0), 0.0);
    assert_eq!(s.time.cell_tick(9), BAR_TICKS + 480.0);
    assert_eq!(s.time.cell_tick(16), 4.0 * BAR_TICKS);
    assert!((s.time.folded_seconds(2.0 * BAR_TICKS + 960.0, 100.0) - 0.6).abs() < 1e-12);

    // região = só a seção B (compassos 2..4), sem repetição
    let r = compile(&song, bank(), SR, &CompileOptions { region: Some(Region { start_bar: 2, end_bar: 4 }), ..Default::default() });
    assert!(r.has_region && !s.has_region);
    assert_eq!(r.time.order, vec![2, 3]);
    assert_eq!(r.hits().count(), expected_hits(&song, Some(Region { start_bar: 2, end_bar: 4 })));
    assert_eq!(r.hits().next().unwrap().1.cell, 16, "células continuam sendo as da fonte");
    // região na seção A: o ritornello continua valendo
    let a = compile(&song, bank(), SR, &CompileOptions { region: Some(Region { start_bar: 0, end_bar: 2 }), ..Default::default() });
    assert_eq!(a.time.order, vec![0, 1, 0, 1]);
}

#[test]
fn swing_moves_the_offbeat() {
    let mut song = demo_song();
    let straight = live(&song);
    song.swing = Some(1.0);
    let swung = live(&song);
    assert_well_formed("swing", &song, &swung);
    let a = hits_of(&straight, 0);
    let b = hits_of(&swung, 0);
    assert_eq!(a.len(), b.len());
    // shime: colcheias; o contratempo vai de 1/2 para 2/3 do tempo
    assert_eq!((a[0].0, a[1].0, a[2].0), (0.0, 480.0, 960.0));
    assert_eq!((b[0].0, b[2].0), (0.0, 960.0));
    assert!((b[1].0 - 640.0).abs() < 1e-9);
    // "doko" no tempo 3 do chū (célula 4): golpes em 0 e 1/4 do tempo → 0 e 1/3·… com warp
    let chu = hits_of(&swung, 1);
    assert_eq!(chu[2].0, 1920.0);
    assert!((chu[3].0 - (1920.0 + swing_warp(0.25, 1.0) * PPQ)).abs() < 1e-9);
    assert!((swing_warp(0.25, 1.0) - 1.0 / 3.0).abs() < 1e-12);
    // meio swing
    song.swing = Some(0.5);
    assert!((hits_of(&live(&song), 0)[1].0 - 560.0).abs() < 1e-9);
    // o cursor acompanha: a célula do contratempo só começa no ponto deformado
    assert_eq!(swung.time.cell_at(500.0), Some(0));
    assert_eq!(swung.time.cell_at(641.0), Some(1));
    assert_eq!(straight.time.cell_at(500.0), Some(1));
}

#[test]
fn mute_and_solo_two_layers() {
    let mut song = demo_song();
    let all = export(&song).hits().count();
    assert_eq!(all, 67);
    // mudo de linha: no export nem é agendado; ao vivo é agendado e o canal fica mudo
    song.tracks[0].muted = true;
    let e = export(&song);
    assert_eq!(e.hits().count(), 67 - 35);
    assert!(e.hits().all(|(_, h)| h.track != 0));
    let l = live(&song);
    assert_eq!(l.hits().count(), 67);
    let key = channel_key(&song.tracks[0], None);
    assert!(l.channels[l.channel_slot(&key).unwrap() as usize].mute);
    assert!(!l.channels[l.channel_slot(&channel_key(&song.tracks[1], None)).unwrap() as usize].mute);
    // solo: só a linha em solo soa (mudo ganha do solo na mesma linha)
    song.tracks[0].muted = false;
    song.tracks[2].solo = true;
    assert_eq!(export(&song).hits().count(), 5);
    song.tracks[2].muted = true;
    assert_eq!(export(&song).hits().count(), 0);
    song.tracks[2].muted = false;
    song.tracks[2].solo = false;

    // 2ª camada: pessoas. chū pertence à Ana; shime é compartilhado entre Ana e Bia.
    song.people = Some(vec![Person { id: "ana".into(), name: "Ana".into(), volume: Some(-6.0), ..Default::default() }, Person { id: "bia".into(), name: "Bia".into(), ..Default::default() }]);
    song.tracks[1].person_ids = Some(vec!["ana".into()]);
    song.tracks[0].person_ids = Some(vec!["ana".into(), "bia".into()]);
    // as 4 primeiras células do shime de cada compasso são da Bia; o resto cai no fallback (Ana)
    for m in &mut song.measures {
        let mut owners = vec![None; 8];
        for o in owners.iter_mut().take(4) {
            *o = Some("bia".to_string());
        }
        m.cell_owners.get_or_insert_default().insert("shime".into(), owners);
    }
    let l = live(&song);
    assert_well_formed("pessoas", &song, &l);
    let bus_of = |s: &Schedule, key: &str| s.buses[s.channels[s.channel_slot(key).unwrap() as usize].bus as usize].clone();
    assert_eq!(bus_of(&l, "chu1@ana").person_id, "ana");
    assert!((bus_of(&l, "chu1@ana").gain - 0.501).abs() < 1e-3, "volume do barramento −6 dB");
    assert_eq!(bus_of(&l, "shime::ana").person_id, "ana");
    assert_eq!(bus_of(&l, "shime::bia").person_id, "bia");
    assert_eq!(l.channels[l.channel_slot("odaiko@master").unwrap() as usize].bus, 0);
    // golpes do shime: canal por dono da célula
    let (ana_slot, bia_slot) = (l.channel_slot("shime::ana").unwrap(), l.channel_slot("shime::bia").unwrap());
    let shime = hits_of(&l, 0);
    assert!(shime.iter().all(|(t, h)| (h.strip / 2 == bia_slot) == (*t % BAR_TICKS < 1920.0)));
    assert!(shime.iter().any(|(_, h)| h.strip / 2 == ana_slot));
    // mudo da Bia: ao vivo o barramento dela fica mudo; no export os golpes dela somem
    song.people.as_mut().unwrap()[1].muted = Some(true);
    assert!(bus_of(&live(&song), "shime::bia").mute);
    assert!(!bus_of(&live(&song), "shime::ana").mute);
    let e = export(&song);
    assert!(e.channel_slot("shime::bia").is_none());
    assert_eq!(hits_of(&e, 0).len(), shime.iter().filter(|(_, h)| h.strip / 2 == ana_slot).count());
    // solo da Bia (desmutada): Ana some (chū inteiro e as células dela no shime); ōdaiko (sem pessoa) continua
    song.people.as_mut().unwrap()[1].muted = None;
    song.people.as_mut().unwrap()[1].solo = Some(true);
    let e = export(&song);
    assert_eq!(hits_of(&e, 1).len(), 0);
    assert_eq!(hits_of(&e, 2).len(), 5);
    assert!(hits_of(&e, 0).iter().all(|(t, _)| *t % BAR_TICKS < 1920.0));
    assert!(bus_of(&live(&song), "chu1@ana").mute);
}

#[test]
fn dynamics_inheritance_hairpins_override_sfz_main() {
    let mut song = demo_song();
    // chū: "don" em todas as células pra comparar ganhos
    for mi in 0..4 {
        for ci in 0..8 {
            set_cell(&mut song, "chu1", mi, ci, Some("don"), None);
        }
    }
    let base = vel_gain(0.7);
    let gains = |s: &Schedule, track: u16| -> Vec<f64> { hits_of(s, track).iter().map(|(_, h)| h.gain as f64 / base).collect() };
    // marcação herda até a próxima: p no compasso 0, f no 2
    song.measures[0].dynamic = Some(Dynamic::P);
    song.measures[2].dynamic = Some(Dynamic::F);
    let g = gains(&live(&song), 1);
    assert!(g[..16].iter().all(|x| (x - 0.5).abs() < 1e-6), "p vale nos compassos 0 e 1: {:?}", &g[..16]);
    assert!(g[16..].iter().all(|x| (x - 2.0).abs() < 1e-6), "f vale nos compassos 2 e 3");
    // crescendo no compasso 1: rampa de p (0.5) até a próxima marcação (f = 2.0), linear no compasso
    song.measures[1].hairpin = Some(Hairpin::Cresc);
    let g = gains(&live(&song), 1);
    for ci in 0..8 {
        let expect = 0.5 + 1.5 * ci as f64 / 8.0;
        assert!((g[8 + ci] - expect).abs() < 1e-6, "célula {ci}: {} ≠ {expect}", g[8 + ci]);
    }
    // override por instrumento: o shime marca ff no compasso 0 (e herda a própria marcação)
    song.measures[0].track_dynamics.get_or_insert_default().insert("shime".into(), DynamicMark { dynamic: Some(Dynamic::Ff), hairpin: None });
    let s = live(&song);
    let shime_base = 1.2 * vel_gain(0.7);
    assert!((hits_of(&s, 0)[0].1.gain as f64 / shime_base - 3.2).abs() < 1e-6);
    assert!((gains(&s, 1)[0] - 0.5).abs() < 1e-6, "o chū segue a geral");
    // sfz: 1º golpe em vel 1 e dinâmica sfzDyn(d0); num trecho p estoura pra f
    set_cell(&mut song, "chu1", 0, 0, Some("don!"), None);
    let h = hits_of(&live(&song), 1)[0].1;
    assert!((h.gain as f64 - sfz_dyn(0.5) * vel_gain(1.0)).abs() < 1e-6);
    set_cell(&mut song, "chu1", 0, 0, Some("don"), None);
    // main: destaque ×2 (f/mf), os demais ×0.5 (p/mf)
    song.main_track_ids = Some(vec!["chu1".into()]);
    let s = live(&song);
    assert!((gains(&s, 1)[0] - 0.5 * 2.0).abs() < 1e-6);
    assert!((hits_of(&s, 2)[0].1.gain as f64 / vel_gain(1.0) - 0.5 * 0.5).abs() < 1e-6, "ōdaiko em p × 0.5");
}

#[test]
fn instrument_defs_change_pitch_gain_decay_and_filters() {
    let mut song = demo_song();
    let plain = hits_of(&live(&song), 0)[0].1;
    song.tracks[0].instrument_id = Some("preset-shime-agudo".into());
    let s = live(&song);
    let h = hits_of(&s, 0)[0].1;
    assert!((h.rate / plain.rate - 2f32.powf(7.0 / 12.0)).abs() < 1e-5, "pitch +7 st");
    assert!(h.dur < plain.dur, "decay 0.4 encurta");
    assert_eq!(h.fade, 0.02);
    // filtro de timbre (highpass 900 Hz) entra antes do EQ do shime (highpass + peaking)
    let ch = &s.channels[(h.strip / 2) as usize];
    assert_eq!((ch.chains[0].n, ch.chains[1].n), (3, 1));
    assert!(ch.chains[0].response_db(200.0, 48_000.0) < -20.0);
    // overtones somam no Q: nagadō anelado = bandpass 320 Hz com Q 1.2 + 0.4·8
    let def = wadawko_core::model::BUILTIN_INSTRUMENTS.iter().find(|d| d.id == "preset-nagado-anelado").unwrap();
    let chain = zone_chain(def, Zone::Head, 48_000.0);
    assert_eq!(chain.n, 5);
    let bw = 320.0 / 4.4;
    assert!((chain.bands[0].response_db(320.0 + bw / 2.0, 48_000.0) + 3.0).abs() < 0.4);
    // ganho da zona em dB: +8 dB no couro
    song.tracks[1].instrument_id = Some("preset-nagado-anelado".into());
    let h2 = hits_of(&live(&song), 1)[0].1;
    assert!((h2.gain as f64 / (0.9 * vel_gain(1.0)) - 10f64.powf(8.0 / 20.0)).abs() < 1e-5);
}

#[test]
fn channel_mix_volume_pan_and_slot_stability() {
    let mut song = demo_song();
    song.tracks[1].volume = Some(-12.0);
    song.tracks[1].pan = Some(-0.5);
    let mut c = Compiler::new(bank(), SR);
    let a = c.compile(&song, &CompileOptions::default());
    let slot = a.channel_slot("chu1@master").unwrap();
    let ch = &a.channels[slot as usize];
    assert!((ch.gain - 0.2512).abs() < 1e-4 && ch.pan == -0.5 && !ch.mute);
    assert!((a.channels[a.channel_slot("shime@master").unwrap() as usize].gain - 0.5012).abs() < 1e-4, "default −6 dB");
    // recompilar depois de remover a 1ª linha: o chū continua no MESMO slot
    let shime_slot = a.channel_slot("shime@master").unwrap();
    wadawko_core::ops::tracks::remove_track(&mut song, "shime");
    let b = c.compile(&song, &CompileOptions::default());
    assert_eq!(b.channel_slot("chu1@master"), Some(slot));
    assert_eq!(b.channel_slot("shime@master"), None);
    assert!(b.hits().all(|(_, h)| h.track == 0 || h.track == 1));
    // o slot liberado só é reutilizado numa compilação seguinte
    let id = wadawko_core::ops::tracks::add_track(&mut song, "kane");
    let d = c.compile(&song, &CompileOptions::default());
    assert_eq!(d.channel_slot(&format!("{id}@master")), Some(shime_slot));
    assert_eq!(d.channel_slot("chu1@master"), Some(slot));
}

#[test]
fn kyai_events_are_scheduled_and_deterministic() {
    let mut song = demo_song();
    set_kyai(&mut song, "chu1", 0, 0, Some("so"));
    set_kyai(&mut song, "chu1", 0, 1, Some("re!"));
    set_kyai(&mut song, "chu1", 3, 6, Some("sei!"));
    let s = live(&song);
    assert_well_formed("kiai", &song, &s);
    let slot = s.channel_slot("chu1@master#kyai").expect("canal de kiai");
    let ch = &s.channels[slot as usize];
    assert!(ch.kyai && ch.gain == 1.0 && ch.chains[0].n == 0);
    let voice: Vec<_> = s.events.iter().filter(|e| matches!(e.kind, EventKind::Grain(_) | EventKind::Noise(_))).collect();
    let ticks: std::collections::BTreeSet<u64> = voice.iter().map(|e| e.tick as u64).collect();
    assert_eq!(ticks.into_iter().collect::<Vec<_>>(), vec![0, 480, 3 * 3840 + 2880]);
    // "so": 1 ruído (s) + grãos da vogal "o" (letra 14 do animalese)
    let so: Vec<_> = voice.iter().filter(|e| e.tick == 0.0).collect();
    assert_eq!(so.iter().filter(|e| matches!(e.kind, EventKind::Noise(_))).count(), 1);
    let grains: Vec<_> = so.iter().filter_map(|e| if let EventKind::Grain(g) = e.kind { Some(g) } else { None }).collect();
    assert!(grains.len() >= 2);
    assert!(grains.iter().all(|g| (g.start - 14.0 * 0.15).abs() < 0.02 && g.sample == bank().animalese_id().0 && g.strip == slot * 2));
    // a sílaba cabe no intervalo até a próxima (meia colcheia a 100 BPM = 0,3 s), fora o micro-atraso
    assert!(grains.iter().all(|g| g.delay + g.dur < 0.3 + 0.018 * 0.5 + 0.006), "{grains:?}");
    // mesma semente → mesmos kiai; outra semente → variação diferente
    assert_eq!(live(&song), s);
    let other = compile(&song, bank(), SR, &CompileOptions { seed: 99, ..Default::default() });
    assert_ne!(other.events, s.events);
    assert_eq!(other.hits().count(), s.hits().count());
    // linha muda: no export o kiai dela também some
    song.tracks[1].muted = true;
    assert!(export(&song).events.iter().all(|e| !matches!(e.kind, EventKind::Grain(_) | EventKind::Noise(_))));
    // volume da voz (dB) vai pro canal de kiai
    song.tracks[1].kyai = Some(wadawko_core::KyaiVoice { pitch: None, vary: Some(0.0), gain: Some(-6.0) });
    let s = live(&song);
    assert!((s.channels[s.channel_slot("chu1@master#kyai").unwrap() as usize].gain - 0.5012).abs() < 1e-4);
}

#[test]
fn fixtures_compile() {
    let all = fixtures();
    assert_eq!(all.len(), 2);
    for (key, song) in &all {
        let s = live(song);
        assert_well_formed(key, song, &s);
        assert_eq!(s.hits().count(), expected_hits(song, None), "{key}");
        assert_eq!(s.time.order, play_order(song, None), "{key}");
        let e = export(song);
        assert_eq!(e.hits().count(), s.hits().count(), "{key}: nada mudo nas fixtures");
    }
}

#[test]
fn big_fixture_ritornellos_dynamics_and_kit() {
    let song = big_song();
    let s = live(&song);
    // 131 compassos + 2 seções de 8 compassos repetidas
    assert_eq!(s.time.order.len(), 131 + 16);
    assert_eq!(&s.time.order[..18], &[0, 1, 2, 3, 4, 5, 6, 7, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    assert!((s.seconds(song.tempo) - 147.0 * 4.0 * 60.0 / 180.0).abs() < 1e-9);
    assert!(s.hits().count() > 2000, "{}", s.hits().count());
    // 50 marcações + 20 hairpins: o ganho relativo dos golpes cobre de pp a ff
    let rel: Vec<f64> = s.hits().map(|(_, h)| h.gain as f64).collect();
    let (lo, hi) = rel.iter().fold((f64::MAX, 0f64), |(lo, hi), g| (lo.min(*g), hi.max(*g)));
    assert!(hi / lo > 12.0, "faixa dinâmica {lo}..{hi}");
    // 4 linhas, todas com instrumento custom do kit da música
    assert_eq!(song.instruments().len(), 4);
    assert_eq!(s.channels.iter().filter(|c| c.is_used()).count(), 4);
}

#[test]
fn yagyo_swing_and_repeat() {
    let song = yagyo();
    assert_eq!(song.swing(), 1.0);
    let s = live(&song);
    assert_eq!(s.time.order.len(), 18 + 2 * 2);
    // com swing 1 nenhum golpe cai na metade exata do tempo (colcheias vão pro terço)
    let off: Vec<f64> = s.hits().map(|(t, _)| t % PPQ).filter(|f| *f != 0.0).collect();
    assert!(!off.is_empty());
    assert!(off.iter().all(|f| (f - 480.0).abs() > 1.0), "golpe reto numa música com swing");
    assert!(off.iter().any(|f| (f - 640.0).abs() < 1e-6));
}

// ───────────────────────────── fórmulas de compasso diferentes ─────────────────────────────

/// demo com os 3 primeiros compassos em 4/4, 5/4 e 3/4, uma nota no ÚLTIMO
/// tempo de cada um e só esses 3 compassos tocando
fn mixed_meters() -> Song {
    let mut s = demo_song();
    s.measures.truncate(3);
    s.sections = None;
    wadawko_core::ops::measures::set_measure_beats(&mut s, 1, 5);
    wadawko_core::ops::measures::set_measure_beats(&mut s, 2, 3);
    for m in &mut s.measures {
        for row in m.cells.values_mut() {
            row.iter_mut().for_each(|c| *c = None);
        }
        m.kyai = None;
    }
    let t = s.tracks[0].id.clone();
    for mi in 0..3 {
        let last_beat_first_cell = (s.measures[mi].beats - 1) * s.measures[mi].subdivision;
        set_cell(&mut s, &t, mi, last_beat_first_cell, Some("don"), None);
    }
    s
}

#[test]
fn compassos_de_formulas_diferentes_tem_duracoes_diferentes() {
    let song = mixed_meters();
    let s = live(&song);
    // duração = 4 + 5 + 3 tempos
    assert_eq!(s.len_ticks, 12.0 * PPQ);
    assert_eq!(s.time.len_ticks(), 12.0 * PPQ);
    assert_eq!((s.time.bar_tick(0), s.time.bar_tick(1), s.time.bar_tick(2), s.time.bar_tick(3)), (0.0, 4.0 * PPQ, 9.0 * PPQ, 12.0 * PPQ));
    // a nota do último tempo de cada compasso: tempos 3, 8 e 11 (sem sobrepor o compasso seguinte)
    let ticks: Vec<f64> = hits_of(&s, 0).iter().map(|(t, _)| *t).collect();
    assert_eq!(ticks, vec![3.0 * PPQ, 8.0 * PPQ, 11.0 * PPQ]);
    // metrônomo: 12 cliques, acento no 1º tempo de cada compasso (tempos 0, 4 e 9)
    let clicks: Vec<(f64, bool)> = s.events.iter().filter_map(|e| if let EventKind::Click { beat, sub: 0 } = e.kind { Some((e.tick / PPQ, beat == 0)) } else { None }).collect();
    assert_eq!(clicks.len(), 12);
    let accents: Vec<f64> = clicks.iter().filter(|(_, a)| *a).map(|(t, _)| *t).collect();
    assert_eq!(accents, vec![0.0, 4.0, 9.0]);
    // cursor ↔ tempo batem em todas as células, e o compasso tocado sai certo
    let total_cells: usize = song.measures.iter().map(|m| m.cell_count()).sum();
    for g in 0..total_cells {
        assert_eq!(s.time.cell_at(s.time.cell_tick(g)), Some(g), "célula {g}");
    }
    assert_eq!(s.time.bar_at(8.9 * PPQ), Some(1));
    assert_eq!(s.time.bar_at(9.0 * PPQ), Some(2));
    assert_eq!(s.time.bar_at(12.0 * PPQ), None);
}

#[test]
fn formula_diferente_com_ritornello_dobra_o_playhead() {
    let mut song = mixed_meters();
    // seção única cobrindo os 3 compassos, tocada 2×
    song.sections = Some(vec![Section { id: "s".into(), name: "S".into(), start: 0, repeat: Some(2), ..Default::default() }]);
    let s = live(&song);
    assert_eq!(s.time.order, vec![0, 1, 2, 0, 1, 2]);
    assert_eq!(s.len_ticks, 24.0 * PPQ);
    // na 2ª volta, o 1º tempo do compasso de 5/4 (tick 16) dobra para o tick 4 da música escrita
    assert_eq!(s.time.fold_tick(16.0 * PPQ), 4.0 * PPQ);
    assert_eq!(hits_of(&s, 0).len(), 6);
}
