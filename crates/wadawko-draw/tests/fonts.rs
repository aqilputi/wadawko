//! Medição de fontes/glifos e metadados SMuFL contra os arquivos reais de `assets/fonts/`.

use wadawko_draw::smufl::{self, FAMILY_BRAVURA, FAMILY_ZEN_KAKU, glyphs};
use wadawko_draw::{FontWeight, SmuflFont, StandardFonts, TextAlign, TextStyle, pt};

fn fonts() -> StandardFonts {
    StandardFonts::load(smufl::dev_fonts_dir()).expect("fontes em assets/fonts/")
}

#[test]
fn standard_fonts_load_and_resolve() {
    let f = fonts();
    assert_eq!(f.book.len(), 5);
    assert_eq!(f.book.resolve(FAMILY_ZEN_KAKU, FontWeight::REGULAR, false), Some(f.zen_regular));
    assert_eq!(f.book.resolve(FAMILY_ZEN_KAKU, FontWeight::MEDIUM, false), Some(f.zen_medium));
    assert_eq!(f.book.resolve(FAMILY_ZEN_KAKU, FontWeight::BOLD, false), Some(f.zen_bold));
    // sem itálico na família: cai na face reta de peso mais próximo
    assert_eq!(f.book.resolve(FAMILY_ZEN_KAKU, FontWeight(800), true), Some(f.zen_bold));
    assert_eq!(f.book.resolve("zen kaku gothic new", FontWeight::REGULAR, false), Some(f.zen_regular));
    assert_eq!(f.book.resolve(FAMILY_BRAVURA, FontWeight::REGULAR, false), Some(f.bravura.font));
    assert_eq!(f.book.resolve("Nao Existe", FontWeight::REGULAR, false), None);
    assert_eq!(f.book.info(f.zen_bold).unwrap().weight, FontWeight::BOLD);
    assert_eq!(f.book.info(f.bravura.font).unwrap().units_per_em, 1000);
}

#[test]
fn bravura_metadata_is_typed() {
    let f = fonts();
    let md = &f.bravura.metadata;
    assert_eq!(md.font_name, "Bravura");
    assert!(!md.font_version.is_empty());
    let d = f.bravura.defaults();
    assert!((d.stem_thickness - 0.12).abs() < 1e-6);
    assert!((d.staff_line_thickness - 0.13).abs() < 1e-6);
    assert!((d.beam_thickness - 0.5).abs() < 1e-6);
    assert!(!d.text_font_family.is_empty());

    let a = f.bravura.anchors("noteheadBlack").expect("âncoras de noteheadBlack");
    let se = a.stem_up_se.expect("stemUpSE");
    let nw = a.stem_down_nw.expect("stemDownNW");
    assert!((se.x - 1.18).abs() < 1e-4 && (se.y - 0.168).abs() < 1e-4, "{se:?}");
    assert!((nw.x - 0.0).abs() < 1e-4 && (nw.y + 0.168).abs() < 1e-4, "{nw:?}");
    // conversão para a tela: y para baixo
    let p = se.to_px(10.0);
    assert!((p.x - 11.8).abs() < 1e-3 && (p.y + 1.68).abs() < 1e-3);
    // bandeirola: âncora de encaixe na haste
    assert!(f.bravura.anchors("flag8thUp").unwrap().stem_up_nw.is_some());
    assert!(f.bravura.anchors("noteheadXBlack").unwrap().stem_up_se.is_some());
}

#[test]
fn notehead_black_measurement_matches_metadata() {
    let f = fonts();
    let g = glyphs::NOTEHEAD_BLACK;
    assert_eq!(g.codepoint, '\u{E0A4}');
    let bb = f.bravura.metadata.bbox(g.name).unwrap();
    assert!((bb.width() - 1.18).abs() < 1e-4 && (bb.height() - 1.0).abs() < 1e-4);

    // caixa em unidades da fonte (y para cima) vs. metadata em staff spaces (em/4)
    let upem = f.book.info(f.bravura.font).unwrap().units_per_em as f32;
    let sp = upem / 4.0;
    let [x0, y0, x1, y1] = f.book.glyph_bbox_units(f.bravura.font, g.codepoint).unwrap();
    let tol = 0.01 * sp;
    assert!((x0 - bb.sw.x * sp).abs() <= tol, "x_min {x0}");
    assert!((y0 - bb.sw.y * sp).abs() <= tol, "y_min {y0}");
    assert!((x1 - bb.ne.x * sp).abs() <= tol, "x_max {x1}");
    assert!((y1 - bb.ne.y * sp).abs() <= tol, "y_max {y1}");

    // em px: staff space de 10 px → fonte de 40 px; cabeça ≈ 11.8 × 10 px centrada na linha
    let size = SmuflFont::font_size(10.0);
    assert_eq!(size, 40.0);
    let m = f.book.glyph_metrics(f.bravura.font, g.codepoint, size).unwrap();
    let want = f.bravura.bbox_px(g.name, 10.0).unwrap();
    for (got, want) in [(m.bbox.x, want.x), (m.bbox.y, want.y), (m.bbox.w, want.w), (m.bbox.h, want.h)] {
        assert!((got - want).abs() < 0.1, "{:?} vs {:?}", m.bbox, want);
    }
    assert!((m.bbox.w - 11.8).abs() < 0.1 && (m.bbox.h - 10.0).abs() < 0.1);
    assert!((m.bbox.y + 5.0).abs() < 0.1, "topo 0.5 sp acima da linha de base");
    let adv = f.bravura.advance_px(g.name, 10.0).unwrap();
    assert!((m.advance - adv).abs() < 0.1, "avanço {} vs metadata {}", m.advance, adv);

    // o contorno existe e cabe na caixa
    let outline = f.book.glyph_outline(f.bravura.font, g.codepoint, size).unwrap();
    let ob = outline.bounds().unwrap();
    assert!(ob.w > 11.0 && ob.w < 12.5 && ob.h > 9.5 && ob.h < 10.5, "{ob:?}");
}

/// Confere nome ↔ codepoint de toda a tabela `smufl::glyphs`: a caixa medida na fonte pelo
/// codepoint tem de bater com a caixa do metadata pelo nome.
#[test]
fn glyph_table_agrees_with_font_and_metadata() {
    let f = fonts();
    let sp = 250.0; // upem 1000
    for g in glyphs::ALL {
        let bb = f.bravura.metadata.bbox(g.name).unwrap_or_else(|| panic!("{} sem bbox no metadata", g.name));
        let [x0, y0, x1, y1] = f
            .book
            .glyph_bbox_units(f.bravura.font, g.codepoint)
            .unwrap_or_else(|| panic!("{} (U+{:X}) ausente na fonte", g.name, g.codepoint as u32));
        let ok = (x0 - bb.sw.x * sp).abs() <= 3.0
            && (y0 - bb.sw.y * sp).abs() <= 3.0
            && (x1 - bb.ne.x * sp).abs() <= 3.0
            && (y1 - bb.ne.y * sp).abs() <= 3.0;
        assert!(ok, "{} U+{:X}: fonte [{x0} {y0} {x1} {y1}] vs metadata {bb:?}", g.name, g.codepoint as u32);
    }
    assert_eq!(glyphs::by_name("noteheadXBlack"), Some(glyphs::NOTEHEAD_X_BLACK));
}

#[test]
fn text_measurement() {
    let f = fonts();
    let st = TextStyle::new(FAMILY_ZEN_KAKU, 20.0);
    let m = f.book.measure_text("don", &st).unwrap();
    assert!(m.width > 20.0 && m.width < 45.0, "{m:?}");
    assert!(m.ascent > 10.0 && m.descent > 2.0 && m.line_height >= m.ascent + m.descent);

    // largura é aditiva e escala linearmente com o tamanho
    let a = f.book.measure_text("do", &st).unwrap().width;
    let b = f.book.measure_text("n", &st).unwrap().width;
    assert!((a + b - m.width).abs() < 1e-3);
    let big = f.book.measure_text("don", &st.clone().size(40.0)).unwrap();
    assert!((big.width - 2.0 * m.width).abs() < 1e-3);

    // kana: largura cheia (1 em por caractere)
    let k = f.book.measure_text("ドン", &st).unwrap();
    assert!((k.width - 40.0).abs() < 0.5, "{k:?}");

    // negrito é mais largo que regular em latim
    let bold = f.book.measure_text("don", &st.clone().bold()).unwrap();
    assert!(bold.width > m.width);

    // caixa conforme o alinhamento
    let bx = m.bounds(pt(100.0, 50.0), TextAlign::Center);
    assert!((bx.center().x - 100.0).abs() < 1e-3 && (bx.y - (50.0 - m.ascent)).abs() < 1e-3);
    let bx = m.bounds(pt(100.0, 50.0), TextAlign::Right);
    assert!((bx.right() - 100.0).abs() < 1e-3);

    // família desconhecida
    assert!(f.book.measure_text("x", &TextStyle::new("Nao Existe", 12.0)).is_none());
}

#[test]
fn layout_falls_back_per_character() {
    let f = fonts();
    // a Bravura não tem kana: o glifo vem de outra fonte do livro
    let st = TextStyle::new(FAMILY_BRAVURA, 20.0);
    let l = f.book.layout_text("\u{E0A4}ド", &st).unwrap();
    assert_eq!(l.glyphs.len(), 2);
    assert_eq!(l.glyphs[0].font, f.bravura.font);
    assert_ne!(l.glyphs[1].font, f.bravura.font);
    assert!(l.glyphs[1].glyph_id != 0);
    assert!((l.glyphs[1].x - l.glyphs[0].advance).abs() < 1e-4);
}

#[test]
fn text_outline_alignment_and_synthetic_italic() {
    let f = fonts();
    let st = TextStyle::new(FAMILY_ZEN_KAKU, 24.0);
    let w = f.book.measure_text("HH", &st).unwrap().width;
    let left = f.book.text_outline("HH", &st, pt(100.0, 50.0)).unwrap().bounds().unwrap();
    let center = f.book.text_outline("HH", &st.clone().align(TextAlign::Center), pt(100.0, 50.0)).unwrap().bounds().unwrap();
    let right = f.book.text_outline("HH", &st.clone().align(TextAlign::Right), pt(100.0, 50.0)).unwrap().bounds().unwrap();
    assert!(left.x >= 100.0 && left.right() <= 100.0 + w + 0.01);
    assert!((left.x - center.x - w / 2.0).abs() < 1e-2);
    assert!((left.x - right.x - w).abs() < 1e-2);
    assert!(left.bottom() <= 50.5 && left.y < 40.0, "apoiado na linha de base: {left:?}");

    // itálico sintético: mesma base, topo deslocado para a direita
    let ital = f.book.text_outline("HH", &st.clone().italic(), pt(100.0, 50.0)).unwrap().bounds().unwrap();
    assert!(ital.right() > left.right() + 1.0, "{ital:?} vs {left:?}");
    assert!((ital.y - left.y).abs() < 1e-3);
}
