//! A [`Surface`] de verdade: pinta na janela do GPUI (`paint_quad`, `paint_path`,
//! `shape_line`). Guarda entre frames o que é caro de refazer ([`PaintCache`]):
//! linhas de texto modeladas e contornos de glifo já triangulados.

use std::collections::HashMap;

use gpui_kit::{App, BorderStyle, Bounds, ContentMask, FillOptions, FillRule, Hsla, PathBuilder, PathStyle, Pixels, Rgba, ShapedLine, SharedString, TextAlign as GTextAlign, TextRun, Window, point, px, quad, size};
use wadawko_draw::{Color, FontBook, FontId, Path, PathEl, Point, Rect, TextAlign, TextStyle};

use super::canvas::Surface;

pub fn hsla(c: Color) -> Hsla {
    Rgba { r: c.r, g: c.g, b: c.b, a: c.a }.into()
}

pub fn color(h: Hsla) -> Color {
    let c = Rgba::from(h);
    Color::new(c.r, c.g, c.b, c.a)
}

fn bounds(r: Rect) -> Bounds<Pixels> {
    Bounds::new(point(px(r.x), px(r.y)), size(px(r.w), px(r.h)))
}

/// Como os glifos SMuFL (Bravura) são pintados.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphMode {
    /// pelo text system do GPUI (atlas de glifos: o mais barato por frame)
    Text,
    /// contorno do `FontBook` triangulado (non-zero), com cache por (glifo, tamanho)
    Outline,
}

impl GlyphMode {
    /// Padrão: contorno. `WADAWKO_SCORE_GLYPHS=text` troca (para comparar os dois).
    pub fn from_env() -> Self {
        match std::env::var("WADAWKO_SCORE_GLYPHS").as_deref() {
            Ok("text") => GlyphMode::Text,
            _ => GlyphMode::Outline,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct LineKey {
    text: SharedString,
    family: SharedString,
    size_bits: u32,
    weight: u16,
    rgba: [u8; 4],
}

/// Cache de pintura da partitura, dono = a view (vive entre frames).
#[derive(Default)]
pub struct PaintCache {
    lines: HashMap<LineKey, ShapedLine>,
    /// (fonte, codepoint, tamanho) → contorno triangulado com a origem do glifo em (0, 0)
    outlines: HashMap<(u32, char, u32), Option<gpui_kit::Path<Pixels>>>,
}

impl PaintCache {
    pub fn clear(&mut self) {
        self.lines.clear();
        self.outlines.clear();
    }

    /// Não deixa o cache crescer sem limite (zoom contínuo gera tamanhos novos).
    pub fn trim(&mut self) {
        if self.lines.len() > 6000 {
            self.lines.clear();
        }
        if self.outlines.len() > 600 {
            self.outlines.clear();
        }
    }

    pub fn len(&self) -> (usize, usize) {
        (self.lines.len(), self.outlines.len())
    }
}

pub struct WindowSurface<'a> {
    pub window: &'a mut Window,
    pub cx: &'a mut App,
    pub cache: &'a mut PaintCache,
    pub fonts: &'a FontBook,
    pub glyphs: GlyphMode,
}

/// Triangula um caminho (coordenadas da janela, px lógicos) com a regra non-zero.
fn tessellate(path: &Path, scale_factor: f32) -> Option<gpui_kit::Path<Pixels>> {
    // tolerância de ~1/8 de pixel físico
    let opts = FillOptions::default().with_fill_rule(FillRule::NonZero).with_tolerance(0.125 / scale_factor.max(0.5));
    let mut b = PathBuilder::fill().with_style(PathStyle::Fill(opts));
    let p = |q: Point| point(px(q.x), px(q.y));
    let mut open = false;
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(a) => {
                b.move_to(p(a));
                open = true;
            }
            // o lyon exige um move_to antes de qualquer segmento
            _ if !open => continue,
            PathEl::LineTo(a) => b.line_to(p(a)),
            PathEl::QuadTo(c, a) => b.curve_to(p(a), p(c)),
            PathEl::CubicTo(c1, c2, a) => b.cubic_bezier_to(p(a), p(c1), p(c2)),
            PathEl::Close => {
                b.close();
                open = false;
            }
        }
    }
    let built = b.build().ok()?;
    (!built.vertices.is_empty()).then_some(built)
}

fn translated(mut path: gpui_kit::Path<Pixels>, dx: f32, dy: f32) -> gpui_kit::Path<Pixels> {
    let d = point(px(dx), px(dy));
    for v in &mut path.vertices {
        v.xy_position = v.xy_position + d;
    }
    path.bounds.origin = path.bounds.origin + d;
    path
}

impl WindowSurface<'_> {
    fn masked(&mut self, clip: Option<Rect>, f: impl FnOnce(&mut Window, &mut App)) {
        match clip {
            Some(c) => self.window.with_content_mask(Some(ContentMask { bounds: bounds(c) }), |w| f(w, self.cx)),
            None => f(self.window, self.cx),
        }
    }

    fn shape(&mut self, text: &str, family: &str, size_px: f32, weight: u16, color: Color) -> ShapedLine {
        let key = LineKey { text: SharedString::from(text.to_string()), family: SharedString::from(family.to_string()), size_bits: size_px.to_bits(), weight, rgba: color.to_rgba8() };
        if let Some(line) = self.cache.lines.get(&key) {
            return line.clone();
        }
        let mut font = gpui_kit::font(key.family.clone());
        font.weight = gpui_kit::FontWeight(weight as f32);
        let run = TextRun { len: key.text.len(), font, color: hsla(color), background_color: None, underline: None, strikethrough: None };
        let line = self.window.text_system().shape_line(key.text.clone(), px(size_px), &[run], None);
        self.cache.lines.insert(key, line.clone());
        line
    }

    /// Pinta a linha com a linha de base em `origin.y` (o GPUI ancora no topo da linha).
    fn paint_line(&mut self, line: ShapedLine, origin: Point, align: TextAlign, clip: Option<Rect>) {
        let w = f32::from(line.width);
        let x = match align {
            TextAlign::Left => origin.x,
            TextAlign::Center => origin.x - w * 0.5,
            TextAlign::Right => origin.x - w,
        };
        let (asc, desc) = (line.ascent, line.descent);
        // altura de linha = ascent + descent ⇒ a linha de base fica a `ascent` do topo
        let top = point(px(x), px(origin.y) - asc);
        self.masked(clip, |window, cx| {
            let _ = line.paint(top, asc + desc, GTextAlign::Left, None, window, cx);
        });
    }
}

impl Surface for WindowSurface<'_> {
    fn quad(&mut self, r: Rect, radius: f32, fill: Option<Color>, border: Option<(f32, Color)>, clip: Option<Rect>) {
        let bg = fill.map(hsla).unwrap_or_else(gpui_kit::transparent_black);
        let (bw, bc) = border.map(|(w, c)| (w, hsla(c))).unwrap_or((0.0, gpui_kit::transparent_black()));
        self.masked(clip, |window, _| window.paint_quad(quad(bounds(r), px(radius), bg, px(bw), bc, BorderStyle::Solid)));
    }

    fn fill_path(&mut self, path: &Path, color: Color, clip: Option<Rect>) {
        let Some(p) = tessellate(path, self.window.scale_factor()) else { return };
        self.masked(clip, |window, _| window.paint_path(p, hsla(color)));
    }

    fn text(&mut self, text: &str, origin: Point, style: &TextStyle, color: Color, clip: Option<Rect>) {
        let line = self.shape(text, &style.family, style.size, style.weight.0, color);
        self.paint_line(line, origin, style.align, clip);
    }

    fn glyph(&mut self, font: FontId, codepoint: char, origin: Point, size_px: f32, color: Color, clip: Option<Rect>) {
        if self.glyphs == GlyphMode::Text
            && let Some(family) = self.fonts.info(font).map(|i| i.family.clone())
        {
            let mut buf = [0u8; 4];
            let line = self.shape(codepoint.encode_utf8(&mut buf), &family, size_px, 400, color);
            self.paint_line(line, origin, TextAlign::Left, clip);
            return;
        }
        let key = (font.0, codepoint, size_px.to_bits());
        if !self.cache.outlines.contains_key(&key) {
            let sf = self.window.scale_factor();
            let built = self.fonts.glyph_outline(font, codepoint, size_px).and_then(|o| tessellate(&o, sf));
            self.cache.outlines.insert(key, built);
        }
        let Some(Some(base)) = self.cache.outlines.get(&key) else { return };
        let p = translated(base.clone(), origin.x, origin.y);
        self.masked(clip, |window, _| window.paint_path(p, hsla(color)));
    }
}
