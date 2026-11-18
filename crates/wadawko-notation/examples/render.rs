//! Gera PNGs/SVGs de exemplo da partitura: `cargo run -p wadawko-notation --example render -- <dir>`.

use std::path::PathBuf;

use wadawko_core::model::{DynamicMark, PROJECT_KEY_PREFIX};
use wadawko_core::{Dynamic, Hairpin, InstrumentId, Measure, Person, Section, Song, Track};
use wadawko_draw::smufl::dev_fonts_dir;
use wadawko_draw::{StandardFonts, rect};
use wadawko_notation::{EngraveOptions, HitKey, Theme, engrave, to_svg, to_svg_pages};

const BACKUP: &str = include_str!("../../wadawko-core/tests/fixtures/backup-localstorage-2026-10-02.json");

/// Música sintética que exercita tudo: pessoas, kiai, sfz, acorde, quiáltera, rulo, repetição, dinâmicas.
pub fn kitchen_sink() -> Song {
    let row = |cells: &[&str]| cells.iter().map(|s| (!s.is_empty()).then(|| s.to_string())).collect::<Vec<_>>();
    let mut chu = Track::new("chu", "Chū (dupla)", InstrumentId::Chu);
    chu.person_ids = Some(vec!["ana".into(), "bia".into()]);
    let mut shime = Track::new("shime", "Shime", InstrumentId::Shime);
    shime.person_ids = Some(vec!["caio".into()]);
    let person = |id: &str, name: &str, color: &str| Person { id: id.into(), name: name.into(), color: Some(color.into()), muted: None, solo: None, volume: None, extra: Default::default() };
    let mut m1 = Measure::new(4, 2, ["shime", "chu"]);
    m1.cells.insert("shime".into(), row(&["ten", "teke", "ten", "teke", "tsuku", "ten", "suko", "ten"]));
    m1.cells.insert("chu".into(), row(&["DON!", "", "doron", "ka", "dondondon", "kara", "dokodoko", "su"]));
    m1.cell_owners = Some([("chu".to_string(), row(&["ana", "ana", "ana", "ana", "bia", "bia", "bia", "bia"]))].into());
    m1.kyai = Some([("chu".to_string(), row(&["", "", "", "", "", "", "sore!", ""]))].into());
    m1.dynamic = Some(Dynamic::Mf);
    m1.hairpin = Some(Hairpin::Cresc);
    let mut m2 = Measure::new(4, 4, ["shime", "chu"]);
    m2.cells.insert("shime".into(), row(&["ten", "ten", "ten", "ten", "ten", "", "ten", "", "ten", "", "", "ten", "teke", "ten", "ten", "ten"]));
    m2.cells.insert("chu".into(), row(&["don", "", "", "", "do", "ko", "do", "ko", "DON", "", "ka", "", "", "", "", ""]));
    m2.hairpin = Some(Hairpin::Cresc);
    m2.kyai = Some([("shime".to_string(), row(&["", "", "", "", "", "", "", "", "ha", "", "", "", "", "", "", ""]))].into());
    let mut m3 = Measure::new(3, 2, ["shime", "chu"]);
    m3.cells.insert("shime".into(), row(&["", "", "", "", "", ""]));
    m3.cells.insert("chu".into(), row(&["dokodokodoko", "", "KARAN", "dokodon", "dondoko", "don"]));
    m3.dynamic = Some(Dynamic::Ff);
    m3.track_dynamics = Some([("chu".to_string(), DynamicMark { dynamic: Some(Dynamic::P), hairpin: Some(Hairpin::Dim) })].into());
    let mut m4 = Measure::new(2, 1, ["shime", "chu"]);
    m4.cells.insert("shime".into(), row(&["ten", "ten"]));
    m4.cells.insert("chu".into(), row(&["DON", ""]));
    m4.hairpin = Some(Hairpin::Dim);
    let sec = |id: &str, name: &str, start: usize, repeat: Option<u32>| Section { id: id.into(), name: name.into(), start, repeat, ..Default::default() };
    Song {
        title: "Tudo junto".into(),
        tempo: 120.0,
        tracks: vec![shime, chu],
        measures: vec![m1, m2, m3, m4],
        sections: Some(vec![sec("a", "Abertura", 0, Some(3)), sec("b", "Final", 2, Some(2))]),
        people: Some(vec![person("ana", "Ana", "#2f9e44"), person("bia", "Bia", "#1c7ed6"), person("caio", "Caio", "#e8590c")]),
        ..Default::default()
    }
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "target/notation".into()));
    std::fs::create_dir_all(&out).unwrap();
    let fonts = StandardFonts::load(dev_fonts_dir()).unwrap();
    let save = |name: &str, bytes: Vec<u8>| std::fs::write(out.join(name), bytes).unwrap();

    // demo: contínua, com 2 golpes acesos, e SVG
    let demo = wadawko_core::demo::demo_song();
    let score = engrave(&demo, &EngraveOptions::default(), &fonts);
    let (w, h) = score.size();
    let lit = [(HitKey::new("chu1", 4, 0), score.theme.accent), (HitKey::new("shime", 4, 0), score.theme.accent)];
    save("demo.png", score.region_png(0, rect(0.0, 0.0, w, h), &fonts, 1.0, &lit).unwrap());
    save("demo-zoom.png", score.region_png(0, rect(0.0, 0.0, 760.0, h), &fonts, 2.0, &[]).unwrap());
    std::fs::write(out.join("demo.svg"), to_svg(&demo, &EngraveOptions::default(), &fonts)).unwrap();

    // tudo junto: claro e escuro
    let sink = kitchen_sink();
    for (name, theme) in [("sink-light.png", Theme::light()), ("sink-dark.png", Theme::dark())] {
        let score = engrave(&sink, &EngraveOptions::screen(theme), &fonts);
        save(name, score.page_png(0, &fonts, 1.5).unwrap());
    }
    let score = engrave(&sink, &EngraveOptions::a4_landscape(), &fonts);
    save("sink-a4.png", score.page_png(0, &fonts, 1.5).unwrap());

    // músicas reais do backup
    let backup: serde_json::Map<String, serde_json::Value> = serde_json::from_str(BACKUP).unwrap();
    for (key, raw) in backup.iter().filter(|(k, _)| k.starts_with(PROJECT_KEY_PREFIX)) {
        let song = Song::from_json(raw.as_str().unwrap()).unwrap();
        if song.measures.len() < 10 {
            continue;
        }
        let tag = key.rsplit(':').next().unwrap();
        let score = engrave(&song, &EngraveOptions::default(), &fonts);
        let (w, h) = score.size();
        println!("{tag}: {} compassos, contínua {w:.0}×{h:.0}, {} golpes", song.measures.len(), score.hit_count());
        save(&format!("{tag}-inicio.png"), score.region_png(0, rect(0.0, 0.0, 1500.0, h), &fonts, 1.0, &[]).unwrap());
        let mid = score.x_at(song.measures.len() / 2, 0.0).unwrap().x;
        save(&format!("{tag}-meio.png"), score.region_png(0, rect(mid - 100.0, 0.0, 1500.0, h), &fonts, 1.0, &[]).unwrap());
        save(&format!("{tag}-fim.png"), score.region_png(0, rect(w - 1200.0, 0.0, 1200.0, h), &fonts, 1.0, &[]).unwrap());
        let paged = engrave(&song, &EngraveOptions::a4_landscape(), &fonts);
        println!("{tag}: A4 paisagem = {} páginas", paged.page_count());
        save(&format!("{tag}-a4-p1.png"), paged.page_png(0, &fonts, 1.5).unwrap());
        save(&format!("{tag}-a4-p2.png"), paged.page_png(1.min(paged.page_count() - 1), &fonts, 1.5).unwrap());
        let pages = to_svg_pages(&song, &EngraveOptions::a4_landscape(), &fonts);
        std::fs::write(out.join(format!("{tag}-a4-p1.svg")), &pages[0]).unwrap();
    }
}
