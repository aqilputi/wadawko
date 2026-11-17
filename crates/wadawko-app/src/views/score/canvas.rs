//! `GpuiCanvas`: o backend GPUI do trait [`Canvas2D`] (docs/planejamento/gpui.md §5).
//!
//! O GPUI não tem transformação nem pilha de estado na API de pintura, então tudo é
//! resolvido aqui na CPU com um [`StateStack`]: cada comando chega em coordenadas do
//! usuário e sai em coordenadas da janela para uma [`Surface`] — a fronteira fina com
//! o GPUI (`surface.rs`), que só sabe pintar quad, caminho preenchido, linha de texto
//! e glifo. A separação existe para testar as decisões sem janela (ver os testes).
//!
//! Decisões:
//! - transformação alinhada aos eixos → quads diretos (`paint_quad`, que encaixa no
//!   pixel do dispositivo: linhas de pauta e hastes ficam nítidas); o resto vira caminho;
//! - traço = contorno calculado na CPU (o mesmo stroker do tiny-skia do backend de
//!   referência) e preenchido com a regra non-zero;
//! - texto reto e sem itálico → `shape_line` do GPUI; texto girado ou em itálico
//!   (a família não tem face itálica: o itálico é sintético) → contorno do `FontBook`;
//! - recorte = caixa envolvente em coordenadas da janela, entregue como content mask.

use wadawko_draw::tiny_skia as ts;
use wadawko_draw::{Canvas2D, Color, FontBook, FontId, LineCap, LineJoin, Path, PathBuilder, PathEl, Point, Quad, Rect, StateStack, Stroke, TextStyle, Transform, pt, rect};

/// O que o backend precisa saber pintar. Tudo em coordenadas da janela (px lógicos);
/// `clip` é o recorte corrente (`None` = sem recorte além do que o elemento já tem).
pub trait Surface {
    fn quad(&mut self, rect: Rect, radius: f32, fill: Option<Color>, border: Option<(f32, Color)>, clip: Option<Rect>);
    /// Preenche com a regra non-zero.
    fn fill_path(&mut self, path: &Path, color: Color, clip: Option<Rect>);
    /// Linha de texto reta; `origin` na linha de base (o `align` do estilo diz o que ele é).
    fn text(&mut self, text: &str, origin: Point, style: &TextStyle, color: Color, clip: Option<Rect>);
    /// Glifo por codepoint, com a origem do glifo (linha de base) em `origin`.
    fn glyph(&mut self, font: FontId, codepoint: char, origin: Point, size: f32, color: Color, clip: Option<Rect>);
}

pub struct GpuiCanvas<'a, S: Surface> {
    surface: &'a mut S,
    fonts: &'a FontBook,
    state: StateStack,
}

impl<'a, S: Surface> GpuiCanvas<'a, S> {
    /// `base` leva o (0, 0) do desenho para a origem do elemento na janela.
    pub fn new(surface: &'a mut S, fonts: &'a FontBook, base: Transform) -> Self {
        GpuiCanvas { surface, fonts, state: StateStack::with_base(base) }
    }

    fn clip(&self) -> Option<Rect> {
        self.state.clip_bounds()
    }

    /// Escala uniforme e positiva sem rotação: texto e glifos podem ir direto ao GPUI.
    fn upright_scale(&self) -> Option<f32> {
        let t = self.state.transform();
        (t.is_axis_aligned() && t.a > 0.0 && (t.a - t.d).abs() <= 1e-4 * t.a.abs()).then_some(t.a)
    }

    fn fill_device(&mut self, path: &Path, color: Color) {
        if path.is_empty() || color.a <= 0.0 {
            return;
        }
        let dev = path.transformed(&self.state.transform());
        if let (Some(clip), Some(b)) = (self.clip(), dev.bounds())
            && !clip.intersects(&b)
        {
            return;
        }
        let clip = self.clip();
        self.surface.fill_path(&dev, color, clip);
    }
}

/// Contorno de um traço como caminho preenchível (non-zero), em coordenadas do usuário.
pub fn stroke_outline(path: &Path, stroke: &Stroke) -> Option<Path> {
    // (NaN também cai aqui)
    if stroke.width.is_nan() || stroke.width <= 0.0 || path.is_empty() {
        return None;
    }
    let mut b = ts::PathBuilder::new();
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => b.move_to(p.x, p.y),
            PathEl::LineTo(p) => b.line_to(p.x, p.y),
            PathEl::QuadTo(c, p) => b.quad_to(c.x, c.y, p.x, p.y),
            PathEl::CubicTo(c1, c2, p) => b.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y),
            PathEl::Close => b.close(),
        }
    }
    let st = ts::Stroke {
        width: stroke.width,
        line_cap: match stroke.cap {
            LineCap::Butt => ts::LineCap::Butt,
            LineCap::Round => ts::LineCap::Round,
            LineCap::Square => ts::LineCap::Square,
        },
        line_join: match stroke.join {
            LineJoin::Miter => ts::LineJoin::Miter,
            LineJoin::Round => ts::LineJoin::Round,
            LineJoin::Bevel => ts::LineJoin::Bevel,
        },
        ..ts::Stroke::default()
    };
    // resolução 4: curvas das pontas redondas suaves mesmo em telas 2×
    let outline = b.finish()?.stroke(&st, 4.0)?;
    let mut out = PathBuilder::new();
    let p = |q: ts::Point| pt(q.x, q.y);
    for seg in outline.segments() {
        match seg {
            ts::PathSegment::MoveTo(a) => out.move_to(p(a)),
            ts::PathSegment::LineTo(a) => out.line_to(p(a)),
            ts::PathSegment::QuadTo(c, a) => out.quad_to(p(c), p(a)),
            ts::PathSegment::CubicTo(c1, c2, a) => out.cubic_to(p(c1), p(c2), p(a)),
            ts::PathSegment::Close => out.close(),
        };
    }
    Some(out.build())
}

impl<S: Surface> Canvas2D for GpuiCanvas<'_, S> {
    fn save(&mut self) {
        self.state.save();
    }

    fn restore(&mut self) {
        self.state.restore();
    }

    fn transform(&mut self, t: Transform) {
        self.state.concat(t);
    }

    fn clip_rect(&mut self, r: Rect) {
        self.state.clip_rect(r);
    }

    fn quad(&mut self, q: &Quad) {
        if q.rect.is_empty() || self.state.is_clipped_out(&q.rect) {
            return;
        }
        let border = q.border_parts();
        if q.fill.is_none() && border.is_none() {
            return;
        }
        let t = self.state.transform();
        if t.is_axis_aligned() {
            let k = self.state.map_len(1.0);
            let clip = self.clip();
            self.surface.quad(self.state.map_rect(&q.rect), q.radius * k, q.fill, border.map(|(w, c)| (w * k, c)), clip);
            return;
        }
        // girado/cisalhado: o retângulo deixa de ser retângulo
        if let Some(fill) = q.fill {
            self.fill_device(&Path::rounded_rect(q.rect, q.radius), fill);
        }
        if let Some((bw, bc)) = border {
            let inner = Path::rounded_rect(q.rect.inset(bw * 0.5), (q.radius - bw * 0.5).max(0.0));
            if let Some(o) = stroke_outline(&inner, &Stroke::new(bw)) {
                self.fill_device(&o, bc);
            }
        }
    }

    fn fill_path(&mut self, path: &Path, color: Color) {
        self.fill_device(path, color);
    }

    fn stroke_path(&mut self, path: &Path, stroke: &Stroke, color: Color) {
        // contorna em coordenadas do usuário e só depois transforma: a espessura
        // acompanha a escala (inclusive a não uniforme), como no canvas HTML
        if let Some(o) = stroke_outline(path, stroke) {
            self.fill_device(&o, color);
        }
    }

    fn line(&mut self, from: Point, to: Point, stroke: &Stroke, color: Color) {
        // linha horizontal/vertical de ponta reta = retângulo: vai como quad (nítido)
        let straight = from.x == to.x || from.y == to.y;
        if straight && stroke.cap != LineCap::Round && self.state.transform().is_axis_aligned() && stroke.width > 0.0 {
            let h = stroke.width * 0.5;
            let e = if stroke.cap == LineCap::Square { h } else { 0.0 };
            let r = if from.y == to.y {
                rect(from.x.min(to.x) - e, from.y - h, (to.x - from.x).abs() + 2.0 * e, stroke.width)
            } else {
                rect(from.x - h, from.y.min(to.y) - e, stroke.width, (to.y - from.y).abs() + 2.0 * e)
            };
            self.quad(&Quad::fill(r, color));
            return;
        }
        self.stroke_path(&Path::line(from, to), stroke, color);
    }

    fn text(&mut self, text: &str, origin: Point, style: &TextStyle, color: Color) {
        if text.is_empty() || color.a <= 0.0 {
            return;
        }
        match self.upright_scale() {
            Some(k) if !style.italic => {
                let clip = self.clip();
                let st = style.clone().size(style.size * k);
                self.surface.text(text, self.state.map_point(origin), &st, color, clip);
            }
            // girado ou itálico sintético: contorno do FontBook (igual ao backend tiny-skia);
            // família não registrada = nada pintado, como lá
            _ => {
                if let Some(outline) = self.fonts.text_outline(text, style, origin) {
                    self.fill_device(&outline, color);
                }
            }
        }
    }

    fn glyph(&mut self, font: FontId, codepoint: char, origin: Point, size: f32, color: Color) {
        if color.a <= 0.0 {
            return;
        }
        match self.upright_scale() {
            Some(k) => {
                let clip = self.clip();
                self.surface.glyph(font, codepoint, self.state.map_point(origin), size * k, color, clip);
            }
            None => {
                if let Some(outline) = self.fonts.glyph_outline(font, codepoint, size) {
                    self.fill_device(&outline.translated(origin.x, origin.y), color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use wadawko_draw::{DrawCmd, FontWeight, Recording, TextAlign};

    use super::*;

    /// O que chegou à superfície (coordenadas da janela).
    #[derive(Debug, Clone, PartialEq)]
    enum Op {
        Quad { rect: Rect, radius: f32, fill: Option<Color>, border: Option<(f32, Color)>, clip: Option<Rect> },
        Path { bounds: Rect, color: Color, clip: Option<Rect> },
        Text { text: String, origin: Point, size: f32, align: TextAlign, weight: FontWeight, color: Color, clip: Option<Rect> },
        Glyph { ch: char, origin: Point, size: f32, color: Color },
    }

    #[derive(Default)]
    struct Mock(Vec<Op>);

    impl Surface for Mock {
        fn quad(&mut self, rect: Rect, radius: f32, fill: Option<Color>, border: Option<(f32, Color)>, clip: Option<Rect>) {
            self.0.push(Op::Quad { rect, radius, fill, border, clip });
        }
        fn fill_path(&mut self, path: &Path, color: Color, clip: Option<Rect>) {
            self.0.push(Op::Path { bounds: path.bounds().unwrap(), color, clip });
        }
        fn text(&mut self, text: &str, origin: Point, style: &TextStyle, color: Color, clip: Option<Rect>) {
            self.0.push(Op::Text { text: text.into(), origin, size: style.size, align: style.align, weight: style.weight, color, clip });
        }
        fn glyph(&mut self, _font: FontId, codepoint: char, origin: Point, size: f32, color: Color, _clip: Option<Rect>) {
            self.0.push(Op::Glyph { ch: codepoint, origin, size, color });
        }
    }

    const RED: Color = Color::rgb(0xff0000);

    fn fonts() -> &'static wadawko_draw::StandardFonts {
        crate::views::score::fonts()
    }

    fn run(base: Transform, f: impl FnOnce(&mut dyn Canvas2D)) -> Vec<Op> {
        let mut mock = Mock::default();
        let mut c = GpuiCanvas::new(&mut mock, &fonts().book, base);
        f(&mut c);
        mock.0
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    fn near_rect(a: Rect, b: Rect) -> bool {
        near(a.x, b.x) && near(a.y, b.y) && near(a.w, b.w) && near(a.h, b.h)
    }

    #[test]
    fn axis_aligned_quads_go_straight_through_in_window_coords() {
        let ops = run(Transform::translate(100.0, 50.0), |c| {
            c.save();
            c.translate(10.0, 0.0);
            c.scale(2.0, 2.0);
            c.quad(&Quad::fill(rect(1.0, 2.0, 3.0, 4.0), RED).radius(1.5).border(0.5, Color::BLACK));
            c.restore();
            c.fill_rect(rect(0.0, 0.0, 5.0, 5.0), RED);
        });
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0], Op::Quad { rect: rect(112.0, 54.0, 6.0, 8.0), radius: 3.0, fill: Some(RED), border: Some((1.0, Color::BLACK)), clip: None });
        // o restore desfez translate+scale
        assert_eq!(ops[1], Op::Quad { rect: rect(100.0, 50.0, 5.0, 5.0), radius: 0.0, fill: Some(RED), border: None, clip: None });
    }

    #[test]
    fn rotated_quad_becomes_a_path() {
        let ops = run(Transform::IDENTITY, |c| {
            c.translate(10.0, 10.0);
            c.rotate(-FRAC_PI_2);
            c.fill_rect(rect(0.0, 0.0, 8.0, 2.0), RED);
        });
        let [Op::Path { bounds, color, .. }] = &ops[..] else { panic!("{ops:?}") };
        assert_eq!(*color, RED);
        // (x, y) → (10 + y, 10 − x): 8×2 deitado vira 2×8 em pé, para cima
        assert!(near_rect(*bounds, rect(10.0, 2.0, 2.0, 8.0)), "{bounds:?}");
    }

    #[test]
    fn straight_lines_are_quads_and_slanted_or_round_lines_are_paths() {
        let ops = run(Transform::translate(0.0, 100.0), |c| {
            c.line(pt(0.0, 10.0), pt(20.0, 10.0), &Stroke::new(2.0), RED); // pauta
            c.line(pt(5.0, 0.0), pt(5.0, 30.0), &Stroke::new(1.0).cap(LineCap::Square), RED); // haste
            c.line(pt(0.0, 0.0), pt(20.0, 6.0), &Stroke::round(2.0), RED); // hairpin
        });
        assert_eq!(ops[0], Op::Quad { rect: rect(0.0, 109.0, 20.0, 2.0), radius: 0.0, fill: Some(RED), border: None, clip: None });
        assert_eq!(ops[1], Op::Quad { rect: rect(4.5, 99.5, 1.0, 31.0), radius: 0.0, fill: Some(RED), border: None, clip: None });
        let Op::Path { bounds, .. } = &ops[2] else { panic!("{:?}", ops[2]) };
        // pontas redondas: o contorno passa 1 px (meia espessura) de cada extremo
        assert!(near_rect(*bounds, rect(-1.0, 99.0, 22.0, 8.0)) || (bounds.w > 21.5 && bounds.w < 22.5 && bounds.h > 7.5 && bounds.h < 8.5), "{bounds:?}");
    }

    #[test]
    fn stroke_outline_has_the_stroke_width() {
        let o = stroke_outline(&Path::line(pt(0.0, 0.0), pt(10.0, 0.0)), &Stroke::new(4.0)).unwrap();
        assert!(near_rect(o.bounds().unwrap(), rect(0.0, -2.0, 10.0, 4.0)));
        assert!(stroke_outline(&Path::line(pt(0.0, 0.0), pt(10.0, 0.0)), &Stroke::new(0.0)).is_none());
    }

    #[test]
    fn clip_is_device_space_survives_translate_and_is_popped_by_restore() {
        let ops = run(Transform::translate(100.0, 0.0), |c| {
            c.save();
            c.clip_rect(rect(10.0, 0.0, 50.0, 50.0));
            c.translate(-30.0, 0.0); // rolagem
            c.fill_rect(rect(45.0, 0.0, 10.0, 10.0), RED); // visível: x janela = 115
            c.fill_rect(rect(0.0, 0.0, 10.0, 10.0), RED); // x janela = 70..80: fora do recorte → descartado
            c.fill_path(&Path::circle(pt(500.0, 5.0), 2.0), RED); // fora → descartado
            c.restore();
            c.fill_rect(rect(0.0, 0.0, 10.0, 10.0), RED);
        });
        let clip = Some(rect(110.0, 0.0, 50.0, 50.0));
        assert_eq!(ops.len(), 2, "{ops:?}");
        assert_eq!(ops[0], Op::Quad { rect: rect(115.0, 0.0, 10.0, 10.0), radius: 0.0, fill: Some(RED), border: None, clip });
        assert!(matches!(&ops[1], Op::Quad { clip: None, .. }));
    }

    #[test]
    fn upright_text_and_glyphs_use_the_text_system_rotated_or_italic_use_outlines() {
        let st = TextStyle::new(wadawko_draw::smufl::FAMILY_ZEN_KAKU, 10.0).bold().align(TextAlign::Center);
        let bravura = fonts().bravura.font;
        let ops = run(Transform::translate(7.0, 3.0), |c| {
            c.scale(2.0, 2.0);
            c.text("don", pt(10.0, 20.0), &st, RED);
            c.glyph(bravura, '\u{E0A4}', pt(1.0, 2.0), 40.0, RED);
            c.text("sore!", pt(10.0, 20.0), &st.clone().italic(), RED);
            c.rotate(-FRAC_PI_2);
            c.text("Ana", pt(0.0, 0.0), &st, RED);
            c.glyph(bravura, '\u{E0A4}', pt(0.0, 0.0), 40.0, RED);
        });
        assert_eq!(ops[0], Op::Text { text: "don".into(), origin: pt(27.0, 43.0), size: 20.0, align: TextAlign::Center, weight: FontWeight::BOLD, color: RED, clip: None });
        assert_eq!(ops[1], Op::Glyph { ch: '\u{E0A4}', origin: pt(9.0, 7.0), size: 80.0, color: RED });
        assert!(matches!(ops[2], Op::Path { .. }), "itálico sintético vai por contorno");
        assert!(matches!(ops[3], Op::Path { .. }), "texto girado vai por contorno");
        // cabeça de nota girada −90°: mais alta que larga
        let Op::Path { bounds, .. } = &ops[4] else { panic!("{:?}", ops[4]) };
        assert!(bounds.h > bounds.w, "{bounds:?}");
        assert_eq!(ops.len(), 5);
    }

    /// A partitura engravada passa inteira pelo canvas: cada comando de desenho do
    /// `Recording` vira exatamente uma operação na superfície, do tipo esperado.
    #[test]
    fn engraved_score_maps_one_to_one_onto_surface_ops() {
        let song = wadawko_core::demo::demo_song();
        let score = wadawko_notation::engrave(&song, &wadawko_notation::EngraveOptions::default(), fonts());
        let mut rec = Recording::new();
        score.paint_page(&mut rec, 0, None);
        let ops = run(Transform::IDENTITY, |c| score.paint_page(c, 0, None));

        // estado corrente ao reproduzir o Recording, para saber o que esperar de cada comando
        let mut st = StateStack::new();
        let mut expected: Vec<&'static str> = Vec::new();
        for cmd in &rec.cmds {
            let upright = st.transform().is_axis_aligned();
            match cmd {
                DrawCmd::Save => st.save(),
                DrawCmd::Restore => {
                    st.restore();
                }
                DrawCmd::Transform(t) => st.concat(*t),
                DrawCmd::ClipRect(r) => st.clip_rect(*r),
                DrawCmd::Quad(_) => expected.push(if upright { "quad" } else { "path" }),
                DrawCmd::Line { from, to, stroke, .. } => expected.push(if upright && (from.x == to.x || from.y == to.y) && stroke.cap != LineCap::Round { "quad" } else { "path" }),
                DrawCmd::FillPath { .. } | DrawCmd::StrokePath { .. } => expected.push("path"),
                DrawCmd::Text { style, .. } => expected.push(if upright && !style.italic { "text" } else { "path" }),
                DrawCmd::Glyph { .. } => expected.push(if upright { "glyph" } else { "path" }),
            }
        }
        let got: Vec<&'static str> = ops
            .iter()
            .map(|o| match o {
                Op::Quad { .. } => "quad",
                Op::Path { .. } => "path",
                Op::Text { .. } => "text",
                Op::Glyph { .. } => "glyph",
            })
            .collect();
        assert_eq!(got.len(), expected.len());
        assert_eq!(got, expected);
        // a demo tem de tudo: notas (glifos), pauta (quads), sílabas (texto)
        for kind in ["quad", "text", "glyph"] {
            assert!(got.contains(&kind), "sem {kind}");
        }
        // glifos e textos saem na posição do comando (base identidade, sem transformação)
        let first_glyph = rec.cmds.iter().find_map(|c| if let DrawCmd::Glyph { origin, size, .. } = c { Some((*origin, *size)) } else { None }).unwrap();
        let got_glyph = ops.iter().find_map(|o| if let Op::Glyph { origin, size, .. } = o { Some((*origin, *size)) } else { None }).unwrap();
        assert_eq!(first_glyph, got_glyph);
    }
}
