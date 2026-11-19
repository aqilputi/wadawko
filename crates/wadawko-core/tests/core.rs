//! Testes de tabela escritos à mão (os casos pedidos na Fase 1 do
//! docs/planejamento/gpui.md). A conferência exaustiva contra o TS está em `golden.rs`.

use wadawko_core::model::{Dynamic, DynamicMark, Hairpin, Measure, Section, Song, Zone};
use wadawko_core::ops::{CellPos, cells, kit, measures, people, sections, tracks};
use wadawko_core::phrase::{parse_words, resolve_cell, split_word};
use wadawko_core::timeline::{Region, dynamics_curve, main_mul, main_tracks, play_order, swing_warp};
use wadawko_core::vocab::{Hand, VOCABULARY, display_syllable, syllable};
use wadawko_core::{demo, kyai};

use Hand::{L, R};
use Zone::{Head, Rim};

// ───────────────────────────── vocabulário e frases ─────────────────────────────

/// (sílaba, golpes [(offset, zona, vel, mão)])
type Row = (&'static str, &'static [(f64, Zone, f64, Hand)]);

const TABLE: &[Row] = &[
    ("DON", &[(0.0, Head, 1.0, R)]),
    ("don", &[(0.0, Head, 0.7, R)]),
    ("KON", &[(0.0, Head, 1.0, L)]),
    ("kon", &[(0.0, Head, 0.7, L)]),
    ("doko", &[(0.0, Head, 0.7, R), (0.5, Head, 0.6, L)]),
    ("doron", &[(0.0, Head, 1.0, R), (0.0, Head, 1.0, L)]),
    ("do", &[(0.0, Head, 0.55, R)]),
    ("ko", &[(0.0, Head, 0.5, L)]),
    ("tsu", &[(0.0, Head, 0.05, R)]),
    ("ku", &[(0.0, Head, 0.05, L)]),
    ("tsuku", &[(0.0, Head, 0.05, R), (0.5, Head, 0.045, L)]),
    ("KA", &[(0.0, Rim, 1.0, R)]),
    ("ka", &[(0.0, Rim, 0.7, R)]),
    ("ra", &[(0.0, Rim, 0.7, L)]),
    ("kara", &[(0.0, Rim, 0.65, R), (0.5, Rim, 0.55, L)]),
    ("karan", &[(0.0, Rim, 1.0, R), (0.0, Rim, 1.0, L)]),
    ("ten", &[(0.0, Head, 0.7, R)]),
    ("teke", &[(0.0, Head, 0.6, R), (0.5, Head, 0.5, L)]),
    ("su", &[]),
];

#[test]
fn tabela_de_silabas() {
    // o vocabulário do TS tem 19 entradas (o planejamento fala em "20 sílabas")
    assert_eq!(VOCABULARY.len(), TABLE.len());
    for (name, hits) in TABLE {
        let entry = syllable(name).unwrap_or_else(|| panic!("{name} fora do vocabulário"));
        assert_eq!(entry.hits.len(), hits.len(), "{name}");

        // cada sílaba sozinha se resolve nela mesma, sem alongamento nem acento genérico
        let split = split_word(name).unwrap();
        assert_eq!(split.len(), 1, "{name}");
        assert_eq!((split[0].syl, split[0].stretch, split[0].accent), (*name, 1, false), "{name}");

        let cell = resolve_cell(name).unwrap();
        assert!(!cell.sfz);
        assert_eq!(cell.parts.len(), 1);
        assert_eq!(cell.hits.len(), hits.len(), "{name}");
        for (got, &(offset, zone, vel, hand)) in cell.hits.iter().zip(*hits) {
            assert_eq!((got.offset, got.zone, got.vel, got.hand), (offset, zone, vel, hand), "{name}");
            assert_eq!(got.accent, vel >= 1.0, "{name}: acento só nos golpes de força máxima");
            assert!(!got.sfz);
        }
    }
    assert!(syllable("Don").is_none());
    assert_eq!(display_syllable("su"), "·");
    assert_eq!(display_syllable("don"), "don");
}

fn syls(word: &str) -> Vec<(&'static str, u32, bool)> {
    split_word(word).unwrap().into_iter().map(|s| (s.syl, s.stretch, s.accent)).collect()
}

#[test]
fn dooon_alongamento() {
    assert_eq!(syls("dooon"), [("don", 3, false)]);
    assert_eq!(syls("DOOON"), [("DON", 3, false)]);
    assert_eq!(syls("doooko"), [("doko", 3, false)]);
    let cell = resolve_cell("dooon").unwrap();
    assert_eq!(cell.hits.len(), 1);
    assert_eq!((cell.hits[0].offset, cell.hits[0].vel), (0.0, 0.7));
    // o peso do alongamento divide a célula: "dooonka" = don (3/4) + ka (1/4)
    assert_eq!(syls("dooonka"), [("don", 3, false), ("ka", 1, false)]);
    let offsets: Vec<f64> = resolve_cell("dooonka").unwrap().hits.iter().map(|h| h.offset).collect();
    assert_eq!(offsets, [0.0, 0.75]);
}

#[test]
fn dokon_backtracking() {
    // doko+n não fecha → recua pra do+kon
    assert_eq!(syls("dokon"), [("do", 1, false), ("kon", 1, false)]);
    let cell = resolve_cell("dokon").unwrap();
    let hits: Vec<_> = cell.hits.iter().map(|h| (h.offset, h.vel, h.hand)).collect();
    assert_eq!(hits, [(0.0, 0.55, R), (0.5, 0.7, L)]);
    assert_eq!(syls("dokodoko"), [("doko", 1, false), ("doko", 1, false)]);
    let offsets: Vec<f64> = resolve_cell("dokodoko").unwrap().hits.iter().map(|h| h.offset).collect();
    assert_eq!(offsets, [0.0, 0.25, 0.5, 0.75]);
    assert!(split_word("dok").is_none());
    assert!(split_word("xyz").is_none());
    assert!(resolve_cell("dok").is_none());
    assert_eq!(split_word("").unwrap(), []);
}

#[test]
fn doko_em_caixa_alta_e_acento() {
    // DOKO não existe em maiúsculas no vocabulário → doko com acento genérico (vel ×1.5, teto 1)
    assert_eq!(syls("DOKO"), [("doko", 1, true)]);
    let cell = resolve_cell("DOKO").unwrap();
    let hits: Vec<_> = cell.hits.iter().map(|h| (h.offset, h.vel, h.accent)).collect();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0], (0.0, 1.0, true));
    assert_eq!(hits[1].0, 0.5);
    assert!((hits[1].1 - 0.9).abs() < 1e-12 && hits[1].2);
    // DON existe em maiúsculas: é a variante forte, não acento genérico
    assert_eq!(syls("DON"), [("DON", 1, false)]);
    // caixa mista segue a variante minúscula sem acento
    assert_eq!(syls("Don"), [("don", 1, false)]);
    assert_eq!(syls("DONdoko"), [("DON", 1, false), ("doko", 1, false)]);
}

#[test]
fn don_com_sforzando() {
    let cell = resolve_cell("don!").unwrap();
    assert!(cell.sfz);
    assert_eq!(cell.hits.len(), 1);
    let h = cell.hits[0];
    assert_eq!((h.vel, h.accent, h.sfz), (1.0, true, true));
    // só o 1º golpe da nota é forçado
    let cell = resolve_cell("doko!").unwrap();
    assert_eq!(cell.hits.iter().map(|h| (h.vel, h.sfz)).collect::<Vec<_>>(), [(1.0, true), (0.6, false)]);
    // pausa com "!" não tem golpe pra forçar
    let cell = resolve_cell("su!").unwrap();
    assert!(cell.sfz && cell.hits.is_empty());
    assert!(resolve_cell("!").is_none());
}

#[test]
fn parse_words_valida_o_texto() {
    assert_eq!(parse_words("don doko, DON;tsuku | don!").unwrap(), ["don", "doko", "DON", "tsuku", "don!"]);
    assert_eq!(parse_words("  ").unwrap(), Vec::<String>::new());
    assert!(parse_words("don blah").is_none());
}

// ───────────────────────────── timeline ─────────────────────────────

fn empty_song(n_measures: usize) -> Song {
    let mut song = Song { title: "t".into(), ..Default::default() };
    song.tracks.push(wadawko_core::Track::new("a", "A", wadawko_core::InstrumentId::Chu));
    song.tracks.push(wadawko_core::Track::new("b", "B", wadawko_core::InstrumentId::Shime));
    song.measures = (0..n_measures).map(|_| Measure::new(4, 2, ["a", "b"])).collect();
    song
}

fn section(id: &str, start: usize, repeat: Option<u32>) -> Section {
    Section { id: id.into(), name: id.to_uppercase(), start, repeat, ..Default::default() }
}

#[test]
fn play_order_com_ritornello_e_regiao() {
    let mut song = empty_song(6);
    assert_eq!(play_order(&song, None), [0, 1, 2, 3, 4, 5]);

    // compasso 0 antes da 1ª seção; A = 1..3 tocando 2×; B = 3..6 (gravadas fora de ordem)
    song.sections = Some(vec![section("b", 3, None), section("a", 1, Some(2))]);
    assert_eq!(play_order(&song, None), [0, 1, 2, 1, 2, 3, 4, 5]);

    // região = só a seção A: o ritornello continua valendo
    assert_eq!(play_order(&song, Some(Region { start_bar: 1, end_bar: 3 })), [1, 2, 1, 2]);
    // região cortando as seções
    assert_eq!(play_order(&song, Some(Region { start_bar: 2, end_bar: 5 })), [2, 2, 3, 4]);
    assert_eq!(play_order(&song, Some(Region { start_bar: 0, end_bar: 1 })), [0]);
    // região além do fim é limitada à música
    assert_eq!(play_order(&song, Some(Region { start_bar: 4, end_bar: 99 })), [4, 5]);
    // repeat 0 conta como 1
    song.sections.as_mut().unwrap()[1].repeat = Some(0);
    assert_eq!(play_order(&song, None), [0, 1, 2, 3, 4, 5]);

    // região a partir das seções selecionadas
    let region = sections::selected_region(&song, &["b".to_string()]).unwrap();
    assert_eq!(region, Region { start_bar: 3, end_bar: 6 });
    assert_eq!(sections::selected_region(&song, &[]), None);
}

#[test]
fn dynamics_curve_heranca_e_hairpin() {
    let mut song = empty_song(7);
    song.measures[0].dynamic = Some(Dynamic::P);
    song.measures[1].hairpin = Some(Hairpin::Cresc);
    song.measures[2].hairpin = Some(Hairpin::Cresc);
    song.measures[3].dynamic = Some(Dynamic::Ff);
    song.measures[5].hairpin = Some(Hairpin::Dim);
    let order = play_order(&song, None);
    let c = dynamics_curve(&song, &order, None);
    assert_eq!(c.len(), 7);

    assert_eq!(c.at(0, 0.0), 0.5); // p
    assert_eq!(c.at(0, 0.9), 0.5);
    // crescendo de 2 compassos: de p (0.5) até a próxima marcação, ff (3.2)
    assert_eq!(c.at(1, 0.0), 0.5);
    assert!((c.at(1, 0.5) - (0.5 + 2.7 * 0.25)).abs() < 1e-12);
    assert!((c.at(2, 0.0) - (0.5 + 2.7 * 0.5)).abs() < 1e-12);
    assert!((c.at(2, 0.5) - (0.5 + 2.7 * 0.75)).abs() < 1e-12);
    assert_eq!(c.at(3, 0.0), 3.2); // ff
    assert_eq!(c.at(4, 0.3), 3.2); // herdado
    // diminuendo sem marcação depois: desce um degrau (ff → f) e o alvo persiste
    assert_eq!(c.at(5, 0.0), 3.2);
    assert!((c.at(5, 0.5) - 2.6).abs() < 1e-12);
    assert_eq!(c.at(6, 0.0), 2.0);
    assert_eq!(c.start_level(6), 2.0);
    // fora da ordem → mf
    assert_eq!(c.at(99, 0.0), 1.0);

    // tocando só um trecho: herda a última marcação ANTES dele
    let part = play_order(&song, Some(Region { start_bar: 4, end_bar: 6 }));
    let c = dynamics_curve(&song, &part, None);
    assert_eq!(c.at(0, 0.0), 3.2);
    assert!((c.at(1, 0.5) - 2.6).abs() < 1e-12);

    // sem marcação nenhuma: tudo mf
    let plain = empty_song(2);
    let c = dynamics_curve(&plain, &[0, 1], None);
    assert_eq!((c.at(0, 0.0), c.at(1, 0.5)), (1.0, 1.0));
}

#[test]
fn dynamics_curve_override_por_instrumento() {
    let mut song = empty_song(5);
    song.measures[0].dynamic = Some(Dynamic::P);
    song.measures[2].hairpin = Some(Hairpin::Cresc);
    song.measures[3].dynamic = Some(Dynamic::Mf);
    // "a": f no compasso 1 e diminuendo próprio no 2 (no lugar do crescendo geral)
    song.measures[1].track_dynamics = Some([("a".to_string(), DynamicMark { dynamic: Some(Dynamic::F), hairpin: None })].into());
    song.measures[2].track_dynamics = Some([("a".to_string(), DynamicMark { dynamic: None, hairpin: Some(Hairpin::Dim) })].into());
    let order = play_order(&song, None);

    let general = dynamics_curve(&song, &order, None);
    assert_eq!([general.at(0, 0.0), general.at(1, 0.0), general.at(2, 0.0), general.at(3, 0.0)], [0.5, 0.5, 0.5, 1.0]);
    assert!((general.at(2, 0.5) - 0.75).abs() < 1e-12); // cresc p → mf

    let a = dynamics_curve(&song, &order, Some("a"));
    assert_eq!(a.at(0, 0.0), 0.5); // segue a geral onde não marca nada
    assert_eq!(a.at(1, 0.0), 2.0); // f próprio
    assert_eq!(a.at(2, 0.0), 2.0);
    assert!((a.at(2, 0.5) - 1.5).abs() < 1e-12); // dim próprio: f → mf (próxima marcação geral)
    assert_eq!(a.at(3, 0.0), 1.0);

    // "b" não marca nada: igual à geral
    assert_eq!(dynamics_curve(&song, &order, Some("b")), general);

    assert!(measures::track_has_dynamics(&song, "a"));
    assert!(!measures::track_has_dynamics(&song, "b"));
}

#[test]
fn destaque_main_e_swing() {
    let mut song = demo::demo_song();
    assert_eq!(main_mul(&song, "chu1"), 1.0);
    tracks::toggle_main_track(&mut song, "chu1");
    assert_eq!(main_tracks(&song), ["chu1"]);
    assert_eq!((main_mul(&song, "chu1"), main_mul(&song, "shime")), (2.0, 0.5));
    assert!(tracks::is_main_track(&song, "chu1"));
    // formato antigo
    song.main_track_ids = None;
    song.main_track_id = Some("odaiko".into());
    assert_eq!(main_tracks(&song), ["odaiko"]);
    tracks::toggle_main_track(&mut song, "odaiko");
    assert_eq!((song.main_track_id.clone(), song.main_track_ids.clone()), (None, None));

    assert_eq!(swing_warp(0.5, 0.0), 0.5);
    assert!((swing_warp(0.5, 1.0) - 2.0 / 3.0).abs() < 1e-12);
    assert_eq!(swing_warp(0.0, 1.0), 0.0);
    assert!((swing_warp(1.0, 1.0) - 1.0).abs() < 1e-12);
}

// ───────────────────────────── operações ─────────────────────────────

fn row(song: &Song, track: &str, mi: usize) -> Vec<String> {
    song.measures[mi].cells[track].iter().map(|c| c.clone().unwrap_or_else(|| "_".into())).collect()
}

#[test]
fn set_subdivision_reamostra() {
    let original = demo::demo_song();
    let mut song = original.clone();
    cells::fill_kyai(&mut song, "chu1", 0, 2, &["so", "re!"]);
    let with_kyai = song.clone();

    // 2 → 4: cada célula vai pro dobro do índice
    measures::set_subdivision(&mut song, 4);
    assert!(song.measures.iter().all(|m| m.subdivision == 4 && m.cell_count() == 16 && m.cells.values().all(|r| r.len() == 16)));
    assert_eq!(row(&song, "chu1", 0), ["DON", "_", "_", "_", "don", "_", "_", "_", "doko", "_", "don", "_", "_", "_", "_", "_"]);
    assert_eq!(song.measures[0].kyai_at("chu1", 4), Some("so"));
    assert_eq!(song.measures[0].kyai_at("chu1", 6), Some("re!"));
    assert_eq!(song.measures[0].kyai.as_ref().unwrap()["chu1"].len(), 16);

    // 4 → 2 volta ao original
    measures::set_subdivision(&mut song, 2);
    assert_eq!(song, with_kyai);

    // 2 → 1: colisão empurra pra próxima célula livre; o que não cabe some
    measures::set_subdivision(&mut song, 1);
    assert_eq!(row(&song, "chu1", 0), ["DON", "don", "doko", "don"]);
    assert_eq!(row(&song, "shime", 0), ["ten", "ten", "ten", "ten"]);
    assert_eq!(row(&song, "odaiko", 1), ["_", "_", "DON", "_"]);

    // mesma subdivisão: nada muda (nem os donos)
    let mut shared = original.clone();
    let p1 = people::add_person(&mut shared, None);
    let p2 = people::add_person(&mut shared, None);
    people::toggle_track_person(&mut shared, "chu1", &p1);
    people::toggle_track_person(&mut shared, "chu1", &p2);
    cells::set_cell(&mut shared, "chu1", 0, 1, Some("ka"), Some(&p2));
    assert_eq!(people::cell_owner(&shared, "chu1", 0, 1), Some(p2.as_str()));
    let before = shared.clone();
    measures::set_subdivision(&mut shared, 2);
    assert_eq!(shared, before);
    // mudando o nº de células, os donos são descartados (fallback = 1ª pessoa)
    measures::set_subdivision(&mut shared, 4);
    assert!(shared.measures.iter().all(|m| m.cell_owners.is_none()));
    assert_eq!(people::cell_owner(&shared, "chu1", 0, 2), Some(p1.as_str()));
}

#[test]
fn clonar_secao() {
    let mut song = demo::demo_song();
    let a = sections::append_section(&mut song, None); // envolve os 4 compassos existentes
    let b = sections::append_section(&mut song, Some("B")); // ganha um compasso novo
    assert_eq!(song.measures.len(), 5);
    assert_eq!(song.sections().iter().map(|s| (s.name.as_str(), s.start)).collect::<Vec<_>>(), [("Seção 1", 0), ("B", 4)]);
    sections::set_section_repeat(&mut song, &a, 3);
    measures::set_measure_dynamic(&mut song, 1, Some(Dynamic::F), None);
    cells::set_cell(&mut song, "chu1", 4, 0, Some("karan"), None);

    let copy = sections::clone_section(&mut song, &a).expect("seção existe");
    assert_ne!(copy, a);
    assert_eq!(song.measures.len(), 9);
    // a cópia entra logo depois da original, com nome e ritornello; B é empurrada
    let secs: Vec<_> = song.sections().iter().map(|s| (s.id.as_str(), s.name.as_str(), s.start, s.repeat)).collect();
    assert_eq!(secs, [(a.as_str(), "Seção 1", 0, Some(3)), (copy.as_str(), "Seção 1 (cópia)", 4, Some(3)), (b.as_str(), "B", 8, None)]);
    for k in 0..4 {
        assert_eq!(song.measures[4 + k], song.measures[k], "compasso {k} copiado com tudo");
    }
    assert_eq!(song.measures[5].dynamic, Some(Dynamic::F));
    assert_eq!(song.measures[8].cell("chu1", 0), Some("karan"));
    // a cópia é independente
    cells::set_cell(&mut song, "chu1", 4, 0, Some("su"), None);
    assert_eq!(song.measures[0].cell("chu1", 0), Some("DON"));

    assert_eq!(sections::clone_section(&mut song, "não-existe"), None);
    assert_eq!(play_order(&song, None).len(), 4 * 3 + 4 * 3 + 1);
}

#[test]
fn mover_secao() {
    let mut song = demo::demo_song();
    let a = sections::append_section(&mut song, Some("A"));
    let b = sections::append_section(&mut song, Some("B"));
    let c = sections::append_section(&mut song, Some("C"));
    measures::add_measure(&mut song, Some(&b)); // B com 2 compassos
    sections::set_section_repeat(&mut song, &b, 2);
    cells::set_cell(&mut song, "odaiko", 4, 0, Some("don"), None); // marca B
    cells::set_cell(&mut song, "odaiko", 6, 0, Some("ka"), None); // marca C
    let ranges = |song: &Song| sections::section_ranges(song).into_iter().map(|r| (r.name, r.start, r.end_excl)).collect::<Vec<_>>();
    assert_eq!(ranges(&song), [("A".into(), 0, 4), ("B".into(), 4, 6), ("C".into(), 6, 7)]);
    let a_first = song.measures[0].clone();

    // C vai pro começo, levando o compasso dela
    sections::move_section(&mut song, &c, 0);
    assert_eq!(ranges(&song), [("C".into(), 0, 1), ("A".into(), 1, 5), ("B".into(), 5, 7)]);
    assert_eq!(song.measures[0].cell("odaiko", 0), Some("ka"));
    assert_eq!(song.measures[1], a_first);
    assert_eq!(song.measures[5].cell("odaiko", 0), Some("don"));
    assert_eq!(song.section(&b).unwrap().repeat, Some(2));
    assert_eq!(song.measures.len(), 7);

    // índice além do fim = última posição
    sections::move_section(&mut song, &c, 99);
    assert_eq!(ranges(&song), [("A".into(), 0, 4), ("B".into(), 4, 6), ("C".into(), 6, 7)]);
    // id desconhecido = nada
    let before = song.clone();
    sections::move_section(&mut song, "x", 0);
    assert_eq!(song, before);

    // remover leva os compassos junto; +/− compasso dentro da seção
    sections::remove_section(&mut song, &a);
    assert_eq!(ranges(&song), [("B".into(), 0, 2), ("C".into(), 2, 3)]);
    measures::remove_measure(&mut song, Some(&b));
    measures::remove_measure(&mut song, Some(&b)); // a seção mantém ao menos 1
    assert_eq!(ranges(&song), [("B".into(), 0, 1), ("C".into(), 1, 2)]);
    assert_eq!(sections::section_of_measure(&song, 1).unwrap().id, c);
    assert_eq!(sections::target_section(&song, &[b.clone(), c.clone()]).unwrap().id, c);
}

#[test]
fn preencher_celulas_e_frases() {
    let mut song = demo::demo_song();
    // sequência atravessa o compasso
    let r = cells::fill_sequential(&mut song, "odaiko", 0, 6, &["don", "ka", "ra"], None);
    assert_eq!(r.written, [CellPos::new(0, 6), CellPos::new(0, 7), CellPos::new(1, 0)]);
    assert_eq!(r.next, Some(CellPos::new(1, 1)));
    assert_eq!(cells::get_cell(&song, "odaiko", 1, 0), Some("ra"));
    // frase: 1 palavra por TEMPO, ancorada no início do tempo, limpando o resto do tempo
    let words = parse_words("don doko don don").unwrap();
    let r = cells::fill_phrase_beats(&mut song, "chu1", 3, 1, &words, None);
    assert_eq!(row(&song, "chu1", 3), ["don", "_", "doko", "_", "don", "_", "don", "_"]);
    assert_eq!(r.written.len(), 8);
    assert_eq!(r.next, None); // acabou a música
    // para no fim da música sem criar compasso
    let r = cells::fill_sequential(&mut song, "chu1", 3, 7, &["ka", "ka"], None);
    assert_eq!((r.written.len(), song.measures.len()), (1, 4));
    // coluna global
    assert_eq!(measures::col_to_pos(&song, 17), Some(CellPos::new(2, 1)));
    assert_eq!(measures::pos_to_col(&song, CellPos::new(2, 1)), Some(17));
    assert_eq!(measures::col_to_pos(&song, 32), None);
    assert_eq!(measures::total_cols(&song), 32);
    measures::ensure_cols(&mut song, 40, None);
    assert_eq!(song.measures.len(), 5);
}

#[test]
fn pessoas_donos_e_kit() {
    let mut song = demo::demo_song();
    let ana = people::add_person(&mut song, Some("Ana"));
    let bia = people::add_person(&mut song, None);
    assert_eq!(song.person(&bia).unwrap().name, "Pessoa 2");
    assert_eq!(song.person(&ana).unwrap().color.as_deref(), Some("#e8663d"));
    people::toggle_track_person(&mut song, "chu1", &ana);
    assert_eq!(people::cell_owner(&song, "chu1", 0, 0), Some(ana.as_str())); // 1 pessoa = sempre ela
    people::toggle_track_person(&mut song, "chu1", &bia);
    assert!(song.track("chu1").unwrap().is_shared());
    // caneta carimba o dono na linha compartilhada
    cells::set_cell(&mut song, "chu1", 0, 1, Some("ka"), Some(&bia));
    assert_eq!(people::cell_owner(&song, "chu1", 0, 1), Some(bia.as_str()));
    assert_eq!(people::cell_owner(&song, "chu1", 0, 0), Some(ana.as_str())); // fallback personIds[0]
    assert_eq!(people::cell_owner(&song, "shime", 0, 0), None);
    let rails = people::rails_for_tracks(&song);
    assert_eq!(rails.iter().map(|r| (r.person.id.as_str(), r.rows.clone())).collect::<Vec<_>>(), [(ana.as_str(), vec![false, true, false]), (bia.as_str(), vec![false, true, false])]);
    // mudo/solo
    assert!(people::person_audible(&song, Some(&ana)) && people::person_audible(&song, None));
    song.person_mut(&bia).unwrap().solo = Some(true);
    assert!(!people::person_audible(&song, Some(&ana)) && people::person_audible(&song, Some(&bia)));
    song.person_mut(&bia).unwrap().muted = Some(true);
    assert!(!people::person_audible(&song, Some(&bia)));
    // apagar a pessoa limpa pertenças e donos
    people::delete_person(&mut song, &bia);
    assert_eq!(song.track("chu1").unwrap().person_ids(), std::slice::from_ref(&ana));
    assert_eq!(song.measures[0].raw_owner("chu1", 1), None);

    // kit: def efetiva com fallback
    let track = song.track("chu1").unwrap().clone();
    assert_eq!(kit::resolve_instrument(&song, &track).id, "chu");
    let custom = kit::add_instrument(&mut song, wadawko_core::InstrumentId::Okedo, None);
    assert_eq!(kit::instrument_by_id(&song, Some(&custom)).unwrap().name, "Okedō-daiko custom");
    tracks::set_track_instrument(&mut song, "chu1", &custom);
    let track = song.track("chu1").unwrap().clone();
    assert_eq!((track.instrument, kit::instrument_label(&song, &track)), (wadawko_core::InstrumentId::Okedo, "Okedō-daiko custom"));
    kit::delete_instrument(&mut song, &custom);
    let track = song.track("chu1").unwrap().clone();
    assert_eq!(kit::resolve_instrument(&song, &track).id, "okedo"); // volta ao som base
    assert_eq!(kit::palette_instruments(&song).len(), 13);
    let line = tracks::add_track(&mut song, "preset-chu-grave");
    assert!(line.starts_with("chu-"));
    assert_eq!(song.track(&line).unwrap().name, "Chū grave");
    assert!(song.measures.iter().all(|m| m.cells[&line].len() == m.cell_count()));
}

#[test]
fn kiai_silabacao_e_contorno() {
    assert_eq!(kyai::split_kyai("seiya!").unwrap(), ["sei", "ya!"]);
    assert_eq!(kyai::split_kyai("soresoresoresore! soryia!").unwrap(), ["so", "re", "so", "re", "so", "re", "so", "re!", "so", "ryia!"]);
    assert_eq!(kyai::split_kyai("HAI!").unwrap(), ["hai!"]);
    assert_eq!(kyai::split_kyai("sokka").unwrap(), ["sok", "ka"]);
    assert!(kyai::split_kyai("").is_none() && kyai::split_kyai("k").is_none() && kyai::split_kyai("so re2").is_none());
    assert!(kyai::is_shout("ya!") && !kyai::is_shout("ya"));
    assert_eq!(kyai::contour(0, 1, true), (5.0, -4.0));
    assert_eq!(kyai::contour(0, 3, false), (0.0, 1.5));
    // a voz é determinística por id e soma o pitch da linha
    let mut track = wadawko_core::Track::new("t", "T", wadawko_core::InstrumentId::Chu);
    let p = kyai::kyai_profile("person-1", &track);
    assert_eq!(p, kyai::kyai_profile("person-1", &track));
    assert_ne!(p, kyai::kyai_profile("person-2", &track));
    assert!((-4.0..5.0).contains(&p.pitch) && (0.85..1.2).contains(&p.speed) && p.vary == 0.5);
    track.kyai = Some(wadawko_core::KyaiVoice { pitch: Some(3.0), vary: Some(0.1), gain: None, voice: None });
    let q = kyai::kyai_profile("person-1", &track);
    assert!((q.pitch - p.pitch - 3.0).abs() < 1e-12 && q.vary == 0.1);
}

// ───────────────────────────── fórmula de compasso ─────────────────────────────

#[test]
fn formula_de_compasso_muda_sem_mexer_nas_notas() {
    let mut s = demo::demo_song();
    let t = s.tracks[0].id.clone();
    let sub = s.measures[0].subdivision;
    let antes: Vec<Option<String>> = s.measures[0].cells[&t].clone();
    assert_eq!(s.measures[0].beats, 4);

    // 4/4 → 5/4: um tempo vazio no fim, notas no lugar
    assert!(measures::set_measure_beats(&mut s, 0, 5));
    assert_eq!(s.measures[0].beats, 5);
    assert_eq!(s.measures[0].cell_count(), 5 * sub);
    for row in s.measures[0].cells.values() {
        assert_eq!(row.len(), 5 * sub);
    }
    assert_eq!(&s.measures[0].cells[&t][..4 * sub], &antes[..]);
    assert!(s.measures[0].cells[&t][4 * sub..].iter().all(Option::is_none));

    // 5/4 → 3/4: corta os últimos tempos
    assert!(measures::set_measure_beats(&mut s, 0, 3));
    assert_eq!(&s.measures[0].cells[&t][..], &antes[..3 * sub]);
    // kiai acompanha
    cells::set_kyai(&mut s, &t, 0, 0, Some("sa!"));
    assert!(measures::set_measure_beats(&mut s, 0, 6));
    assert_eq!(s.measures[0].kyai.as_ref().unwrap()[&t].len(), 6 * sub);

    // mesma fórmula / compasso inexistente: nada muda; limites
    assert!(!measures::set_measure_beats(&mut s, 0, 6));
    assert!(!measures::set_measure_beats(&mut s, 999, 5));
    assert!(measures::set_measure_beats(&mut s, 0, 99));
    assert_eq!(s.measures[0].beats, measures::MAX_BEATS);
    assert!(measures::set_measure_beats(&mut s, 0, 0));
    assert_eq!(s.measures[0].beats, measures::MIN_BEATS);
    // a música continua válida e faz round-trip
    assert_eq!(Song::from_json(&s.to_json()).unwrap(), s);
}

#[test]
fn inicio_dos_compassos_soma_os_tempos() {
    let mut s = demo::demo_song();
    measures::set_measure_beats(&mut s, 1, 5);
    measures::set_measure_beats(&mut s, 2, 3);
    let b: Vec<usize> = s.measures.iter().map(|m| m.beats).collect();
    assert_eq!(&b[..3], &[4, 5, 3]);
    // ordem com repetição: 0 1 2 1
    assert_eq!(measures::bar_beat_starts(&s, &[0, 1, 2, 1]), vec![0.0, 4.0, 9.0, 12.0, 17.0]);
    assert_eq!(measures::bar_beat_starts(&s, &[]), vec![0.0]);
}

// ───────────────────────────── andamento (rallentando / accelerando) ─────────────────────────────

#[test]
fn mapa_de_andamento_rall_accel_e_a_tempo() {
    use wadawko_core::TempoMark;
    let mut s = empty_song(6);
    s.tempo = 100.0;
    let all: Vec<usize> = (0..6).collect();
    // sem marcações: um trecho constante
    assert_eq!(measures::tempo_segments(&s, &all), vec![measures::TempoSegment { start: 0.0, end: 24.0, bpm0: 100.0, bpm1: 100.0 }]);
    // rall. de 100 → 80 ao longo dos compassos 2 e 3; a tempo (100) no 5
    assert!(measures::set_tempo_mark(&mut s, 1, Some(TempoMark { target: 80.0, span: 2 })));
    assert!(!measures::set_tempo_mark(&mut s, 1, Some(TempoMark { target: 80.0, span: 2 })), "igual = sem mudança");
    assert!(measures::set_tempo_mark(&mut s, 4, Some(TempoMark { target: 100.0, span: 0 })));
    let segs = measures::tempo_segments(&s, &all);
    let tuples: Vec<(f64, f64, f64, f64)> = segs.iter().map(|g| (g.start, g.end, g.bpm0, g.bpm1)).collect();
    assert_eq!(tuples, vec![(0.0, 4.0, 100.0, 100.0), (4.0, 12.0, 100.0, 80.0), (12.0, 16.0, 80.0, 80.0), (16.0, 24.0, 100.0, 100.0)]);
    assert_eq!(segs[1].bpm_at(8.0), 90.0);
    // rótulos com o andamento de antes de cada marcação
    let before = measures::tempo_at_measure_starts(&s);
    assert_eq!(before[1], 100.0);
    assert_eq!(before[2], 90.0);
    assert_eq!(before[3], 80.0);
    assert_eq!(before[4], 80.0);
    assert_eq!(measures::tempo_mark_label(s.measures[1].tempo.as_ref().unwrap(), before[1]), "rall. → 80");
    assert_eq!(measures::tempo_mark_label(s.measures[4].tempo.as_ref().unwrap(), before[4]), "♩ = 100");
    assert_eq!(measures::tempo_mark_label(&TempoMark { target: 120.0, span: 1 }, 100.0), "accel. → 120");
    // limites
    assert!(measures::set_tempo_mark(&mut s, 0, Some(TempoMark { target: 5.0, span: 999 })));
    assert_eq!(s.measures[0].tempo, Some(TempoMark { target: measures::MIN_TEMPO, span: measures::MAX_TEMPO_SPAN }));
    assert!(measures::set_tempo_mark(&mut s, 0, None));
    assert!(s.measures[0].tempo.is_none());
    // começando no meio (região): parte do andamento informado
    let segs = measures::tempo_segments_from(&s, &[3, 4, 5], 90.0);
    assert_eq!((segs[0].bpm0, segs[0].end), (90.0, 4.0));
}

#[test]
fn marcacao_no_meio_de_uma_rampa_corta_a_rampa() {
    use wadawko_core::TempoMark;
    let mut s = empty_song(4);
    s.tempo = 100.0;
    measures::set_tempo_mark(&mut s, 0, Some(TempoMark { target: 60.0, span: 4 })); // 100 → 60 em 16 tempos
    measures::set_tempo_mark(&mut s, 2, Some(TempoMark { target: 120.0, span: 1 })); // no tempo 8 (80 BPM) sobe a 120
    let segs = measures::tempo_segments(&s, &[0, 1, 2, 3]);
    let tuples: Vec<(f64, f64, f64, f64)> = segs.iter().map(|g| (g.start, g.end, g.bpm0, g.bpm1)).collect();
    assert_eq!(tuples, vec![(0.0, 8.0, 100.0, 80.0), (8.0, 12.0, 80.0, 120.0), (12.0, 16.0, 120.0, 120.0)]);
}

#[test]
fn marcacao_de_andamento_faz_round_trip_no_json() {
    use wadawko_core::TempoMark;
    let mut s = empty_song(2);
    measures::set_tempo_mark(&mut s, 1, Some(TempoMark { target: 90.0, span: 2 }));
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains(r#""tempo":{"target":90,"span":2}"#), "{json}");
    let back: Song = serde_json::from_str(&json).unwrap();
    assert_eq!(back.measures[1].tempo, s.measures[1].tempo);
    assert!(back.measures[0].tempo.is_none());
    // span ausente = 1 compasso
    let m: Measure = serde_json::from_str(r#"{"beats":4,"subdivision":2,"cells":{},"tempo":{"target":70}}"#).unwrap();
    assert_eq!(m.tempo, Some(TempoMark { target: 70.0, span: 1 }));
}
