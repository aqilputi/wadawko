//! Backends SVG (snapshot), tiny-skia (pixels) e Recording (replay).

use wadawko_draw::smufl::{self, FAMILY_ZEN_KAKU, glyphs};
use wadawko_draw::{
    Canvas2D, Color, DrawCmd, LineCap, Path, Quad, Recording, SkiaCanvas, SmuflFont, StandardFonts,
    StateStack, Stroke, SvgCanvas, SvgOptions, TextAlign, TextStyle, Transform, pt, rect,
};

const INK: Color = Color::rgb(0x1f1a17);
const RED: Color = Color::rgb(0xc8102e);
const PAPER: Color = Color::rgb(0xf6f1e7);

fn fonts() -> StandardFonts {
    StandardFonts::load(smufl::dev_fonts_dir()).expect("fontes em assets/fonts/")
}

/// Cena sem fontes: quads, linha, caminhos, texto nativo, recorte e transformação.
fn shapes_scene(c: &mut dyn Canvas2D) {
    c.fill_rect(rect(0.0, 0.0, 120.0, 80.0), PAPER);
    c.quad(&Quad::fill(rect(10.0, 10.0, 40.0, 20.0), RED.with_alpha(0.5)).radius(4.0).border(2.0, INK));
    c.line(pt(10.0, 40.5), pt(110.0, 40.5), &Stroke::new(1.0), INK);
    let mut pb = Path::builder();
    pb.move_to(pt(60.0, 10.0))
        .line_to(pt(80.0, 10.0))
        .quad_to(pt(90.0, 20.0), pt(80.0, 30.0))
        .cubic_to(pt(75.0, 35.0), pt(65.0, 35.0), pt(60.0, 30.0))
        .close();
    let path = pb.build();
    c.fill_path(&path, INK);
    c.stroke_path(&path, &Stroke::round(1.5), RED);
    c.text("Don & <ka>", pt(60.0, 60.0), &TextStyle::new("Zen Kaku Gothic New", 12.0).bold().italic().align(TextAlign::Center), RED);
    c.save();
    c.translate(10.0, 50.0);
    c.clip_rect(rect(0.0, 0.0, 20.0, 20.0));
    c.scale(2.0, 2.0);
    c.fill_circle(pt(5.0, 5.0), 5.0, INK);
    c.restore();
    c.stroke_rect(rect(100.0, 60.0, 10.0, 10.0), 1.0, INK);
}

#[test]
fn svg_snapshot_shapes() {
    let mut c = SvgCanvas::new(120.0, 80.0);
    shapes_scene(&mut c);
    let svg = c.finish();
    let expected = include_str!("snapshots/shapes.svg");
    assert_eq!(svg, expected, "\n--- obtido ---\n{svg}");
}

#[test]
fn svg_glyphs_as_paths_and_native() {
    let f = fonts();
    let size = SmuflFont::font_size(10.0);

    // padrão: contorno em <defs>, instanciado com <use> (definido uma única vez)
    let mut c = SvgCanvas::with_fonts(100.0, 40.0, &f.book);
    c.glyph(f.bravura.font, glyphs::NOTEHEAD_BLACK.codepoint, pt(10.0, 20.0), size, INK);
    c.glyph(f.bravura.font, glyphs::NOTEHEAD_BLACK.codepoint, pt(40.0, 20.0), size, RED);
    let svg = c.finish();
    let id = format!("g{}_E0A4", f.bravura.font.0);
    assert_eq!(svg.matches(&format!("<path id=\"{id}\"")).count(), 1, "{svg}");
    assert_eq!(svg.matches(&format!("<use xlink:href=\"#{id}\"")).count(), 2, "{svg}");
    assert!(svg.contains("fill=\"#1f1a17\" transform=\"matrix(0.04 0 0 0.04 10 20)\""), "{svg}");
    assert!(svg.contains("fill=\"#c8102e\" transform=\"matrix(0.04 0 0 0.04 40 20)\""), "{svg}");
    assert!(!svg.contains("<text"));

    // glifo nativo: <text> com a família da fonte e o codepoint como referência numérica
    let opts = SvgOptions { glyphs_as_paths: false, ..SvgOptions::default() };
    let mut c = SvgCanvas::with_options(100.0, 40.0, Some(&f.book), opts);
    c.glyph(f.bravura.font, glyphs::NOTEHEAD_BLACK.codepoint, pt(10.0, 20.0), size, INK);
    let svg = c.finish();
    assert!(
        svg.contains("<text x=\"10\" y=\"20\" font-family=\"Bravura\" font-size=\"40\" fill=\"#1f1a17\">&#xE0A4;</text>"),
        "{svg}"
    );

    // texto como contorno: sem <text>, com <path>
    let opts = SvgOptions { text_as_paths: true, background: Some(PAPER), ..SvgOptions::default() };
    let mut c = SvgCanvas::with_options(100.0, 40.0, Some(&f.book), opts);
    c.text("ドン", pt(10.0, 30.0), &TextStyle::new(FAMILY_ZEN_KAKU, 16.0), INK);
    let svg = c.finish();
    assert!(!svg.contains("<text") && svg.contains("<path d=\"M"), "{svg}");
    assert!(svg.contains("<rect width=\"100\" height=\"40\" fill=\"#f6f1e7\"/>"));
}

fn dump(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("WADAWKO_DRAW_DUMP") {
        std::fs::write(std::path::Path::new(&dir).join(name), bytes).unwrap();
    }
}

#[test]
fn skia_renders_small_scene() {
    let f = fonts();
    let mut c = SkiaCanvas::new(160, 100, &f.book).unwrap();
    assert_eq!(c.pixel(5, 5), Some([0, 0, 0, 0]), "começa transparente");
    c.clear(PAPER);
    let bg = PAPER.to_rgba8();
    assert_eq!(c.pixel(5, 5), Some(bg));

    // quad preenchido
    c.fill_rect(rect(10.0, 10.0, 30.0, 20.0), RED);
    assert_eq!(c.pixel(25, 20), Some(RED.to_rgba8()));
    assert_eq!(c.pixel(45, 20), Some(bg));

    // pauta de 1 linha + cabeça de nota + haste na âncora stemUpSE
    let sp = 10.0;
    let size = SmuflFont::font_size(sp);
    let d = f.bravura.defaults().clone();
    let (x, y) = (60.0, 50.0);
    c.line(pt(50.0, y), pt(150.0, y), &Stroke::new(d.staff_line_thickness * sp), INK);
    c.glyph(f.bravura.font, glyphs::NOTEHEAD_BLACK.codepoint, pt(x, y), size, INK);
    let se = f.bravura.anchors("noteheadBlack").unwrap().stem_up_se.unwrap().to_px(sp);
    let sw = d.stem_thickness * sp;
    c.fill_rect(rect(x + se.x - sw, y + se.y - 3.5 * sp, sw, 3.5 * sp), INK);
    // centro da cabeça é tinta; acima dela, fora da haste, é papel
    assert_eq!(c.pixel(66, 50), Some(INK.to_rgba8()));
    assert_eq!(c.pixel(62, 30), Some(bg));
    // a haste sobe ~3.5 sp a partir da âncora
    let stem_x = (x + se.x - sw * 0.5) as u32;
    let p = c.pixel(stem_x, 25).unwrap();
    assert!(p[0] < 0x80, "haste em ({stem_x}, 25): {p:?}");

    // texto (kana) via contorno, centrado
    let st = TextStyle::new(FAMILY_ZEN_KAKU, 18.0).bold().align(TextAlign::Center);
    c.text("ドン", pt(66.0, 85.0), &st, RED);
    let tb = f.book.measure_text("ドン", &st).unwrap().bounds(pt(66.0, 85.0), TextAlign::Center);
    let mut red_px = 0;
    for py in tb.y as u32..tb.bottom() as u32 {
        for px in tb.x as u32..tb.right() as u32 {
            let p = c.pixel(px, py).unwrap();
            if p[0] > 150 && p[1] < 80 {
                red_px += 1;
            }
        }
    }
    assert!(red_px > 60, "texto pintou {red_px} px vermelhos em {tb:?}");

    // recorte + transformação: círculo de raio 20 recortado a um quadrado 10×10
    c.save();
    c.translate(120.0, 10.0);
    c.clip_rect(rect(0.0, 0.0, 10.0, 10.0));
    c.fill_circle(pt(5.0, 5.0), 20.0, INK);
    c.restore();
    assert_eq!(c.pixel(125, 15), Some(INK.to_rgba8()));
    assert_eq!(c.pixel(135, 15), Some(bg), "fora do recorte");
    assert_eq!(c.pixel(125, 25), Some(bg), "fora do recorte");
    // depois do restore o recorte some
    c.fill_rect(rect(140.0, 80.0, 10.0, 10.0), INK);
    assert_eq!(c.pixel(145, 85), Some(INK.to_rgba8()));

    // a cena não está vazia e o PNG é válido
    let non_bg = c.data().as_chunks::<4>().0.iter().filter(|p| **p != bg).count();
    assert!(non_bg > 1000, "{non_bg} px diferentes do fundo");
    let png = c.encode_png().unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    dump("skia_scene.png", &png);
}

#[test]
fn skia_scale_and_alpha() {
    let f = fonts();
    let mut c = SkiaCanvas::with_scale(20, 10, 2.0, &f.book).unwrap();
    assert_eq!(c.pixel_size(), (40, 20));
    c.clear(Color::WHITE);
    c.fill_rect(rect(0.0, 0.0, 10.0, 10.0), Color::BLACK.with_alpha(0.5));
    let p = c.pixel(10, 10).unwrap();
    assert!((120..=135).contains(&p[0]) && p[3] == 255, "{p:?}");
    assert_eq!(c.pixel(19, 19).unwrap()[0], p[0], "coordenadas lógicas ×2");
    assert_eq!(c.pixel(21, 10), Some([255, 255, 255, 255]));
    assert!(SkiaCanvas::new(0, 10, &f.book).is_err());
}

#[test]
fn recording_replays_identically() {
    let mut rec = Recording::new();
    shapes_scene(&mut rec);
    assert!(matches!(rec.cmds[0], DrawCmd::Quad(_)));
    assert!(rec.cmds.iter().any(|c| matches!(c, DrawCmd::Line { .. })));
    assert!(rec.cmds.iter().any(|c| matches!(c, DrawCmd::Text { text, .. } if text == "Don & <ka>")));

    // replay no SVG = desenho direto
    let mut direct = SvgCanvas::new(120.0, 80.0);
    shapes_scene(&mut direct);
    let mut replayed = SvgCanvas::new(120.0, 80.0);
    rec.replay(&mut replayed);
    assert_eq!(direct.finish(), replayed.finish());

    // replay em outra gravação preserva os comandos (entre o save/restore de proteção)
    let mut again = Recording::new();
    rec.replay(&mut again);
    assert_eq!(&again.cmds[1..again.cmds.len() - 1], &rec.cmds[..]);

    // recolorir um comando (flash de nota) sem regravar
    let i = rec.cmds.iter().position(|c| matches!(c, DrawCmd::Text { .. })).unwrap();
    assert!(rec.cmds[i].set_color(INK));
    assert!(matches!(&rec.cmds[i], DrawCmd::Text { color, .. } if *color == INK));
    assert!(!rec.cmds.iter_mut().find(|c| matches!(c, DrawCmd::Save)).unwrap().set_color(INK));
}

#[test]
fn state_stack_maps_and_clips() {
    let mut s = StateStack::with_base(Transform::translate(100.0, 50.0));
    s.save();
    s.concat(Transform::scale(2.0, 2.0));
    assert_eq!(s.map_point(pt(5.0, 5.0)), pt(110.0, 60.0));
    assert_eq!(s.map_rect(&rect(0.0, 0.0, 10.0, 5.0)), rect(100.0, 50.0, 20.0, 10.0));
    assert_eq!(s.map_len(1.5), 3.0);
    s.clip_rect(rect(0.0, 0.0, 10.0, 10.0));
    s.clip_rect(rect(5.0, 5.0, 10.0, 10.0));
    assert_eq!(s.clip_bounds(), Some(rect(110.0, 60.0, 10.0, 10.0)));
    assert!(s.is_clipped_out(&rect(20.0, 20.0, 5.0, 5.0)));
    assert!(!s.is_clipped_out(&rect(6.0, 6.0, 1.0, 1.0)));
    assert!(s.restore());
    assert_eq!(s.clip_bounds(), None);
    assert_eq!(s.transform(), Transform::translate(100.0, 50.0));
    assert!(!s.restore());

    // ordem de composição: translate depois scale ≠ scale depois translate
    let a = Transform::translate(10.0, 0.0).pre_concat(&Transform::scale(2.0, 2.0));
    let b = Transform::scale(2.0, 2.0).pre_concat(&Transform::translate(10.0, 0.0));
    assert_eq!(a.apply(pt(1.0, 1.0)), pt(12.0, 2.0));
    assert_eq!(b.apply(pt(1.0, 1.0)), pt(22.0, 2.0));
    assert!(a.is_axis_aligned() && !Transform::rotate(0.3).is_axis_aligned());
}

#[test]
fn stroke_caps_reach_svg() {
    let mut c = SvgCanvas::new(10.0, 10.0);
    c.line(pt(1.0, 1.0), pt(9.0, 9.0), &Stroke::new(2.0).cap(LineCap::Square), INK.with_alpha(0.25));
    let svg = c.finish();
    assert!(
        svg.contains("<line x1=\"1\" y1=\"1\" x2=\"9\" y2=\"9\" stroke=\"#1f1a17\" stroke-opacity=\"0.25\" stroke-width=\"2\" stroke-linecap=\"square\"/>"),
        "{svg}"
    );
}
