//! Layout determinístico, contagens (cabeças = golpes, borda = ×, beams por tempo),
//! culling/highlight/playhead, paginação e snapshot SVG.

use std::sync::LazyLock;

use wadawko_core::model::PROJECT_KEY_PREFIX;
use wadawko_core::phrase::resolve_cell;
use wadawko_core::vocab::Zone;
use wadawko_core::{InstrumentId, Measure, Section, Song, Track, demo};
use wadawko_draw::smufl::{dev_fonts_dir, glyphs};
use wadawko_draw::{Color, DrawCmd, Recording, StandardFonts, rect};
use wadawko_notation::{EngraveOptions, HitKey, LayoutMode, PageSpec, Score, Theme, engrave, to_png, to_svg, to_svg_pages};

const BACKUP: &str = include_str!("../../wadawko-core/tests/fixtures/backup-localstorage-2026-10-02.json");

static FONTS: LazyLock<StandardFonts> = LazyLock::new(|| StandardFonts::load(dev_fonts_dir()).expect("fontes em assets/fonts"));

fn fixture_songs() -> Vec<Song> {
    let backup: serde_json::Map<String, serde_json::Value> = serde_json::from_str(BACKUP).unwrap();
    let songs: Vec<Song> =
        backup.iter().filter(|(k, _)| k.starts_with(PROJECT_KEY_PREFIX)).map(|(_, raw)| Song::from_json(raw.as_str().unwrap()).unwrap()).collect();
    assert_eq!(songs.len(), 4);
    songs
}

fn all_songs() -> Vec<Song> {
    let mut songs = fixture_songs();
    songs.push(demo::demo_song());
    songs
}

/// (linha, célula global, golpe, é borda) de todos os golpes da música, pelo core
fn expected_hits(song: &Song) -> Vec<(String, usize, usize, bool)> {
    let mut out = Vec::new();
    let mut base = 0;
    for meas in &song.measures {
        for track in &song.tracks {
            for ci in 0..meas.cell_count() {
                if let Some(res) = meas.cell(&track.id, ci).filter(|v| !v.is_empty()).and_then(resolve_cell) {
                    for (hi, h) in res.hits.iter().enumerate() {
                        out.push((track.id.clone(), base + ci, hi, h.zone == Zone::Rim));
                    }
                }
            }
        }
        base += meas.cell_count();
    }
    out
}

fn head_glyphs(score: &Score) -> (usize, usize) {
    let (mut normal, mut cross) = (0, 0);
    for cmd in score.measures.iter().flat_map(|m| &m.cmds) {
        if let DrawCmd::Glyph { codepoint, .. } = cmd {
            if *codepoint == glyphs::NOTEHEAD_BLACK.codepoint {
                normal += 1;
            } else if *codepoint == glyphs::NOTEHEAD_X_BLACK.codepoint {
                cross += 1;
            }
        }
    }
    (normal, cross)
}

fn one_measure(cells: &[&str]) -> Song {
    let mut m = Measure::new(2, 2, ["t"]);
    m.cells.insert("t".into(), cells.iter().map(|c| (!c.is_empty()).then(|| c.to_string())).collect());
    Song { title: "Um".into(), tracks: vec![Track::new("t", "Chu", InstrumentId::Chu)], measures: vec![m], ..Default::default() }
}

#[test]
fn layout_deterministico_sem_panics() {
    for song in all_songs() {
        for opts in [
            EngraveOptions::default(),
            EngraveOptions::default().with_zoom(0.5).with_theme(Theme::dark()),
            EngraveOptions::default().with_zoom(2.25),
            EngraveOptions::a4_landscape(),
            EngraveOptions::default().with_mode(LayoutMode::Paged(PageSpec::a4_portrait())),
        ] {
            let a = engrave(&song, &opts, &FONTS);
            let b = engrave(&song, &opts, &FONTS);
            assert_eq!(a, b, "{}: layout não determinístico", song.title);
            assert_eq!(a.measures.len(), song.measures.len());
            assert_eq!(a.total_cells(), song.total_cells());
            // todo compasso está em exatamente um sistema, na ordem
            let covered: Vec<usize> = a.systems.iter().flat_map(|s| s.measures.clone()).collect();
            assert_eq!(covered, (0..song.measures.len()).collect::<Vec<_>>());
            for (pi, page) in a.pages.iter().enumerate() {
                for sys in &a.systems[page.systems.clone()] {
                    assert_eq!(sys.page, pi);
                    for m in &a.measures[sys.measures.clone()] {
                        assert!(m.bounds.x.is_finite() && m.bounds.w > 0.0);
                        if matches!(opts.mode, LayoutMode::Paged(_)) {
                            assert!(m.bounds.right() <= page.width + 0.5, "{}: compasso {} vaza da página", song.title, m.index);
                        }
                    }
                }
            }
            // pinta tudo sem panic
            let mut rec = Recording::new();
            for p in 0..a.page_count() {
                a.paint_page(&mut rec, p, None);
            }
            assert!(!rec.is_empty());
        }
    }
}

#[test]
fn musica_vazia_e_degenerada_nao_quebram() {
    let empty = Song { tracks: vec![], measures: vec![], ..Default::default() };
    let s = engrave(&empty, &EngraveOptions::default(), &FONTS);
    assert_eq!(s.hit_count(), 0);
    assert!(s.x_at(0, 0.0).is_none());
    let mut no_measures = demo::demo_song();
    no_measures.measures.clear();
    engrave(&no_measures, &EngraveOptions::a4_landscape(), &FONTS);
    let mut odd = one_measure(&["xyz", "dooon", "DOKODOKODOKODOKO", "!"]);
    odd.measures[0].subdivision = 3; // células a menos que beats*sub
    odd.sections = Some(vec![Section { id: "s".into(), name: "Fora".into(), start: 9, repeat: Some(4), extra: Default::default() }]);
    let s = engrave(&odd, &EngraveOptions::default(), &FONTS);
    s.paint_page(&mut Recording::new(), 0, None);
}

#[test]
fn cabecas_igual_golpes() {
    let demo = demo::demo_song();
    let score = engrave(&demo, &EngraveOptions::default(), &FONTS);
    let expected = expected_hits(&demo);
    let (normal, cross) = head_glyphs(&score);
    assert_eq!(normal + cross, expected.len(), "demo: uma cabeça por golpe");
    assert_eq!(cross, expected.iter().filter(|h| h.3).count());
    assert_eq!(cross, 2, "demo tem 2 ka");

    for song in fixture_songs() {
        let score = engrave(&song, &EngraveOptions::default(), &FONTS);
        let expected = expected_hits(&song);
        assert_eq!(score.hit_count(), expected.len(), "{}: toda chave de golpe existe", song.title);
        let (normal, cross) = head_glyphs(&score);
        let gfx: usize = score.measures.iter().map(|m| m.hits.len()).sum();
        assert_eq!(normal + cross, gfx);
        // as músicas do backup não têm rulo (>4 golpes por célula): cabeça = golpe
        assert_eq!(gfx, expected.len(), "{}", song.title);
    }
}

#[test]
fn borda_usa_cabeca_x_e_chaves_batem_com_o_core() {
    for song in all_songs() {
        let score = engrave(&song, &EngraveOptions::default(), &FONTS);
        for (track, cell, hit, rim) in expected_hits(&song) {
            let key = HitKey::new(track, cell, hit);
            let (meas, gfx) = score.hit(&key).unwrap_or_else(|| panic!("{}: sem golpe {key}", song.title));
            assert_eq!(gfx.rim, rim, "{key}");
            let want = if rim { glyphs::NOTEHEAD_X_BLACK } else { glyphs::NOTEHEAD_BLACK };
            match &meas.cmds[gfx.cmds[0] as usize] {
                DrawCmd::Glyph { codepoint, origin, .. } => {
                    assert_eq!(*codepoint, want.codepoint, "{key}");
                    // sem Formatter: x = x do compasso + (célula + offset)·largura
                    let res = resolve_cell(song.measures[meas.index].cell(&key.track_id, gfx.cell).unwrap()).unwrap();
                    let x = meas.notes_x + (gfx.cell as f32 + res.hits[hit].offset as f32) * meas.cell_width;
                    assert!((origin.x - x).abs() < 0.01, "{key}: x {} ≠ {x}", origin.x);
                }
                other => panic!("{key}: 1º comando não é cabeça: {other:?}"),
            }
        }
    }
}

#[test]
fn rulo_registra_todos_os_golpes_na_mesma_nota() {
    let song = one_measure(&["dokodokodoko", "", "don", ""]);
    let score = engrave(&song, &EngraveOptions::default(), &FONTS);
    assert_eq!(score.hit_count(), 7);
    assert_eq!(head_glyphs(&score), (2, 0));
    let first = score.hit(&HitKey::new("t", 0, 0)).unwrap().1;
    assert_eq!(score.hit(&HitKey::new("t", 0, 5)).unwrap().1, first);
}

#[test]
fn beams_agrupam_por_tempo() {
    let demo = demo::demo_song();
    let score = engrave(&demo, &EngraveOptions::default(), &FONTS);
    // shime, compasso 1: 8 colcheias → 4 beams de 2 notas, um por tempo
    let m = &score.measures[0];
    let shime: Vec<_> = m.beams.iter().filter(|b| b.track == 0).collect();
    assert_eq!(shime.iter().map(|b| (b.beat, b.level, b.notes)).collect::<Vec<_>>(), vec![(0, 1, 2), (1, 1, 2), (2, 1, 2), (3, 1, 2)]);
    // chū, compasso 1: só o tempo 3 (doko don) tem beam: principal de 3 + secundário de 2
    let chu: Vec<_> = m.beams.iter().filter(|b| b.track == 1).map(|b| (b.beat, b.level, b.notes)).collect();
    assert_eq!(chu, vec![(2, 1, 3), (2, 2, 2)]);
    // nenhum beam cruza a fronteira do tempo, em nenhuma música; todos horizontais
    for song in all_songs() {
        let score = engrave(&song, &EngraveOptions::default(), &FONTS);
        for (mi, m) in score.measures.iter().enumerate() {
            let sub = song.measures[mi].subdivision.max(1) as f32;
            for b in &m.beams {
                let lo = m.notes_x + b.beat as f32 * sub * m.cell_width;
                let hi = lo + sub * m.cell_width;
                assert!(b.rect.x >= lo - 0.5 && b.rect.right() <= hi + 0.5, "{}: beam fora do tempo no compasso {mi}", song.title);
                assert!(b.rect.h > 0.0 && b.rect.w > 0.0);
            }
        }
    }
}

#[test]
fn pausas_acento_sfz_e_repeticao_aparecem() {
    let has = |score: &Score, g: wadawko_draw::SmuflGlyph| {
        score.measures.iter().flat_map(|m| &m.cmds).filter(|c| matches!(c, DrawCmd::Glyph { codepoint, .. } if *codepoint == g.codepoint)).count()
    };
    let demo = demo::demo_song();
    let score = engrave(&demo, &EngraveOptions::default(), &FONTS);
    assert!(has(&score, glyphs::REST_8TH) > 0 && has(&score, glyphs::REST_QUARTER) > 0);
    assert_eq!(has(&score, glyphs::ACCENT_ABOVE), 12, "12 DON na demo");

    let big = fixture_songs().into_iter().find(|s| s.measures.len() == 131).unwrap();
    let score = engrave(&big, &EngraveOptions::default(), &FONTS);
    assert_eq!(has(&score, glyphs::DYNAMIC_SFZ), 9, "9 DON! na música grande");
    assert!(has(&score, glyphs::DYNAMIC_FF) > 0 && has(&score, glyphs::DYNAMIC_MP) > 0);
    let texts: Vec<&str> =
        score.measures.iter().flat_map(|m| &m.cmds).filter_map(|c| if let DrawCmd::Text { text, .. } = c { Some(text.as_str()) } else { None }).collect();
    assert_eq!(texts.iter().filter(|t| **t == "×2").count(), 2, "duas seções com ritornello");
    assert!(texts.contains(&"solo1") && texts.contains(&"Seção 18"));
}

#[test]
fn culling_highlight_e_playhead() {
    let big = fixture_songs().into_iter().find(|s| s.measures.len() == 131).unwrap();
    let score = engrave(&big, &EngraveOptions::default(), &FONTS);
    let (w, h) = score.size();
    let mut full = Recording::new();
    score.paint_page(&mut full, 0, None);
    let mut part = Recording::new();
    score.paint(&mut part, rect(w / 2.0, 0.0, 1200.0, h));
    assert!(part.len() * 10 < full.len(), "culling: {} de {}", part.len(), full.len());
    let visible = score.visible_measures(&score.systems[0], Some(rect(w / 2.0, 0.0, 1200.0, h)));
    assert!(visible.len() < 8 && !visible.is_empty());

    // highlight: mesma quantidade de comandos, só as cores do golpe mudam
    let mi = visible.start + 1;
    let hit = score.measures[mi].hits[0].clone();
    let key = score.hit_key(&score.track_ids[hit.track], mi, hit.cell, hit.hit).unwrap();
    let red = Color::rgb(0xff0000);
    let mut lit = Recording::new();
    score.paint_lit(&mut lit, 0, Some(rect(w / 2.0, 0.0, 1200.0, h)), &[(key.clone(), red)]);
    assert_eq!(lit.len(), part.len());
    let changed = lit.cmds.iter().zip(&part.cmds).filter(|(a, b)| a != b).count();
    assert_eq!(changed, hit.cmds.len());
    let mut overlay = Recording::new();
    score.paint_highlights(&mut overlay, 0, None, &[(key.clone(), red)]);
    assert_eq!(overlay.len(), hit.cmds.len());
    assert!(overlay.cmds.iter().all(|c| matches!(c, DrawCmd::Glyph { color, .. } | DrawCmd::Text { color, .. } if *color == red) || matches!(c, DrawCmd::Quad(q) if q.fill == Some(red))));
    // hit-test devolve a mesma chave
    assert_eq!(score.hit_at(0, hit.head.center(), 1.0), Some(key));
    let cell = score.cell_at(0, hit.head.center()).unwrap();
    assert_eq!((cell.track, cell.measure, cell.cell), (hit.track, mi, hit.cell));

    // playhead: cresce com o tempo e bate com a célula global
    let mut last = f32::MIN;
    for m in 0..big.measures.len() {
        for f in [0.0, 0.5, 1.0] {
            let p = score.x_at(m, f).unwrap();
            assert!(p.x >= last && p.bottom > p.top);
            last = p.x;
        }
    }
    let m5 = &score.measures[5];
    let by_cell = score.x_at_cell(m5.cell_base as f32 + m5.cells as f32 / 2.0).unwrap();
    assert!((by_cell.x - score.x_at(5, 0.5).unwrap().x).abs() < 0.01);
    assert_eq!(score.scroll_x_for(0.0, 800.0), 0.0);
    assert_eq!(score.scroll_x_for(w, 800.0), w - 800.0);
    assert_eq!(score.scroll_x_for(5000.0, 800.0), 5000.0 - 320.0);
}

#[test]
fn export_svg_e_png() {
    let demo = demo::demo_song();
    let svg = to_svg(&demo, &EngraveOptions::default(), &FONTS);
    assert!(svg.starts_with("<svg") && svg.trim_end().ends_with("</svg>"));
    let png = to_png(&demo, &EngraveOptions::default(), &FONTS, 1.0).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");

    let big = fixture_songs().into_iter().find(|s| s.measures.len() == 131).unwrap();
    let pages = to_svg_pages(&big, &EngraveOptions::default(), &FONTS);
    let score = engrave(&big, &EngraveOptions::a4_landscape(), &FONTS);
    assert!(pages.len() > 5);
    assert_eq!(score.page_size(0), (1122.52, 793.70));
    for p in &pages {
        assert!(p.contains("width=\"1122.52\"") && p.contains("height=\"793.7\""), "página A4 paisagem a 96 dpi");
    }
    // sistemas justificados: todos (menos talvez o último) terminam na margem direita
    let spec = PageSpec::a4_landscape();
    for sys in &score.systems[..score.systems.len() - 1] {
        let right = score.measures[sys.measures.end - 1].bounds.right();
        assert!((right - (spec.width - spec.margin)).abs() < 0.5, "sistema não justificado: {right}");
    }
}

#[test]
fn snapshot_svg_de_um_compasso() {
    let song = one_measure(&["DON", "", "doko", "ka"]);
    let opts = EngraveOptions { measure_numbers: false, show_labels: false, ..Default::default() };
    let svg = engrave(&song, &opts, &FONTS).page_svg(0, &FONTS, false);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/um-compasso.svg");
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() || !path.exists() {
        std::fs::write(&path, &svg).unwrap();
    }
    let want = std::fs::read_to_string(&path).unwrap();
    assert_eq!(svg, want, "snapshot mudou; rode com UPDATE_SNAPSHOTS=1 se for intencional");
}
