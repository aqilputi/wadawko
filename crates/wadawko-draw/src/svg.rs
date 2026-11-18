//! Backend SVG: acumula elementos e devolve o documento como `String`.
//!
//! - Texto vira `<text>` (depende da fonte instalada/embutida no visualizador) ou, com
//!   [`SvgOptions::text_as_paths`], contornos `<path>`.
//! - Glifos (`Canvas2D::glyph`) viram, por padrão, contornos: cada glifo é definido uma vez em
//!   `<defs>` e instanciado com `<use>` — o SVG não depende da Bravura instalada e fica compacto.
//! - Recorte: cada `clip_rect` abre um `<g clip-path>` fechado no `restore` correspondente.

use std::collections::HashSet;
use std::fmt::Write as _;

use crate::canvas::Canvas2D;
use crate::font::{FontBook, FontId};
use crate::geom::{Color, Point, Rect, Transform};
use crate::path::Path;
use crate::style::{LineCap, LineJoin, Quad, Stroke, TextAlign, TextStyle};

/// Formata um número para SVG: até 3 casas decimais, sem zeros à direita.
pub fn num(v: f32) -> String {
    if !v.is_finite() {
        return "0".to_owned();
    }
    let r = (v as f64 * 1000.0).round() / 1000.0;
    if r == 0.0 {
        return "0".to_owned();
    }
    let mut s = format!("{r:.3}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

/// Como o número acima, mas com mais precisão (fatores de escala pequenos em `matrix()`).
fn num6(v: f32) -> String {
    if !v.is_finite() {
        return "0".to_owned();
    }
    let r = (v as f64 * 1e6).round() / 1e6;
    if r == 0.0 {
        return "0".to_owned();
    }
    let mut s = format!("{r:.6}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

fn escape_xml(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // área de uso privado (SMuFL) e controles: referência numérica
            c if ('\u{E000}'..='\u{F8FF}').contains(&c) || (c.is_control() && c != '\t') => {
                let _ = write!(out, "&#x{:X};", c as u32);
            }
            c => out.push(c),
        }
    }
}

/// Opções do [`SvgCanvas`].
#[derive(Clone, Debug, PartialEq)]
pub struct SvgOptions {
    /// `Canvas2D::text` como contornos (`<path>`) em vez de `<text>`. Exige `FontBook` com a
    /// família; se não resolver, cai em `<text>`. Padrão: `false`.
    pub text_as_paths: bool,
    /// `Canvas2D::glyph` como contornos (`<defs>` + `<use>`) em vez de `<text>` com o
    /// codepoint. Exige `FontBook`. Padrão: `true`.
    pub glyphs_as_paths: bool,
    /// Cor de fundo (um `<rect>` cobrindo tudo). Padrão: sem fundo.
    pub background: Option<Color>,
}

impl Default for SvgOptions {
    fn default() -> Self {
        SvgOptions { text_as_paths: false, glyphs_as_paths: true, background: None }
    }
}

struct SvgState {
    transform: Transform,
    /// Quantos `<g clip-path>` foram abertos neste nível de save.
    open_groups: u32,
}

/// Canvas que gera SVG.
pub struct SvgCanvas<'a> {
    width: f32,
    height: f32,
    fonts: Option<&'a FontBook>,
    opts: SvgOptions,
    defs: String,
    body: String,
    cur: SvgState,
    stack: Vec<SvgState>,
    next_clip: u32,
    glyph_defs: HashSet<(FontId, char)>,
}

impl<'a> SvgCanvas<'a> {
    /// Canvas sem fontes: texto e glifos saem como `<text>`.
    pub fn new(width: f32, height: f32) -> Self {
        Self::build(width, height, None, SvgOptions::default())
    }
    /// Canvas com fontes (permite contornos de glifos/texto e resolve a família dos glifos).
    pub fn with_fonts(width: f32, height: f32, fonts: &'a FontBook) -> Self {
        Self::build(width, height, Some(fonts), SvgOptions::default())
    }
    pub fn with_options(width: f32, height: f32, fonts: Option<&'a FontBook>, opts: SvgOptions) -> Self {
        Self::build(width, height, fonts, opts)
    }

    fn build(width: f32, height: f32, fonts: Option<&'a FontBook>, opts: SvgOptions) -> Self {
        SvgCanvas {
            width,
            height,
            fonts,
            opts,
            defs: String::new(),
            body: String::new(),
            cur: SvgState { transform: Transform::IDENTITY, open_groups: 0 },
            stack: Vec::new(),
            next_clip: 0,
            glyph_defs: HashSet::new(),
        }
    }

    /// Fecha grupos pendentes e devolve o documento SVG completo.
    pub fn finish(mut self) -> String {
        while !self.stack.is_empty() {
            self.restore();
        }
        self.close_groups();
        let (w, h) = (num(self.width), num(self.height));
        let mut out = String::with_capacity(self.body.len() + self.defs.len() + 256);
        let _ = writeln!(out, "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">");
        if !self.defs.is_empty() {
            out.push_str("<defs>\n");
            out.push_str(&self.defs);
            out.push_str("</defs>\n");
        }
        if let Some(bg) = self.opts.background {
            let _ = writeln!(out, "<rect width=\"{w}\" height=\"{h}\"{}/>", paint_attr("fill", bg));
        }
        out.push_str(&self.body);
        out.push_str("</svg>\n");
        out
    }

    fn close_groups(&mut self) {
        for _ in 0..self.cur.open_groups {
            self.body.push_str("</g>\n");
        }
        self.cur.open_groups = 0;
    }

    fn transform_attr(&self) -> String {
        transform_attr(&self.cur.transform)
    }

    fn push_path(&mut self, path: &Path, paint: &str) {
        if path.is_empty() {
            return;
        }
        let _ = writeln!(self.body, "<path d=\"{}\"{}{}/>", path.to_svg_data(), paint, self.transform_attr());
    }

    #[allow(clippy::too_many_arguments)]
    fn push_text(&mut self, content: &str, origin: Point, family: &str, size: f32, weight: u16, italic: bool, align: TextAlign, color: Color) {
        let mut s = String::new();
        let _ = write!(s, "<text x=\"{}\" y=\"{}\" font-family=\"", num(origin.x), num(origin.y));
        escape_xml(family, &mut s);
        let _ = write!(s, "\" font-size=\"{}\"", num(size));
        if weight != 400 {
            let _ = write!(s, " font-weight=\"{weight}\"");
        }
        if italic {
            s.push_str(" font-style=\"italic\"");
        }
        match align {
            TextAlign::Left => {}
            TextAlign::Center => s.push_str(" text-anchor=\"middle\""),
            TextAlign::Right => s.push_str(" text-anchor=\"end\""),
        }
        s.push_str(&paint_attr("fill", color));
        s.push_str(&self.transform_attr());
        s.push('>');
        escape_xml(content, &mut s);
        s.push_str("</text>\n");
        self.body.push_str(&s);
    }
}

fn transform_attr(t: &Transform) -> String {
    if t.is_identity() {
        String::new()
    } else if t.is_translation() {
        format!(" transform=\"translate({} {})\"", num(t.e), num(t.f))
    } else {
        format!(" transform=\"matrix({} {} {} {} {} {})\"", num6(t.a), num6(t.b), num6(t.c), num6(t.d), num(t.e), num(t.f))
    }
}

/// ` fill="#rrggbb"` (+ `fill-opacity` quando há alfa); `attr` é `fill` ou `stroke`.
fn paint_attr(attr: &str, c: Color) -> String {
    if c.is_opaque() { format!(" {attr}=\"{}\"", c.hex_rgb()) } else { format!(" {attr}=\"{}\" {attr}-opacity=\"{}\"", c.hex_rgb(), num(c.a.clamp(0.0, 1.0))) }
}

fn stroke_attrs(stroke: &Stroke, color: Color) -> String {
    let mut s = paint_attr("stroke", color);
    let _ = write!(s, " stroke-width=\"{}\"", num(stroke.width));
    match stroke.cap {
        LineCap::Butt => {}
        LineCap::Round => s.push_str(" stroke-linecap=\"round\""),
        LineCap::Square => s.push_str(" stroke-linecap=\"square\""),
    }
    match stroke.join {
        LineJoin::Miter => {}
        LineJoin::Round => s.push_str(" stroke-linejoin=\"round\""),
        LineJoin::Bevel => s.push_str(" stroke-linejoin=\"bevel\""),
    }
    s
}

impl Canvas2D for SvgCanvas<'_> {
    fn save(&mut self) {
        let transform = self.cur.transform;
        let prev = std::mem::replace(&mut self.cur, SvgState { transform, open_groups: 0 });
        self.stack.push(prev);
    }

    fn restore(&mut self) {
        if let Some(prev) = self.stack.pop() {
            self.close_groups();
            self.cur = prev;
        }
    }

    fn transform(&mut self, t: Transform) {
        self.cur.transform = self.cur.transform.pre_concat(&t);
    }

    fn clip_rect(&mut self, rect: Rect) {
        let id = self.next_clip;
        self.next_clip += 1;
        let _ = writeln!(
            self.defs,
            "<clipPath id=\"clip{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/></clipPath>",
            num(rect.x),
            num(rect.y),
            num(rect.w),
            num(rect.h),
            self.transform_attr()
        );
        let _ = writeln!(self.body, "<g clip-path=\"url(#clip{id})\">");
        self.cur.open_groups += 1;
    }

    fn quad(&mut self, q: &Quad) {
        let border = q.border_parts();
        if q.fill.is_none() && border.is_none() {
            return;
        }
        // o traço do SVG é centrado: encolhe meio traço para a borda ficar para dentro
        let (r, radius) = match border {
            Some((bw, _)) => (q.rect.inset(bw * 0.5), (q.radius - bw * 0.5).max(0.0)),
            None => (q.rect, q.radius),
        };
        let mut s = String::new();
        let _ = write!(s, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"", num(r.x), num(r.y), num(r.w), num(r.h));
        if radius > 0.0 {
            let _ = write!(s, " rx=\"{}\"", num(radius.min(r.w * 0.5).min(r.h * 0.5)));
        }
        match q.fill {
            Some(c) => s.push_str(&paint_attr("fill", c)),
            None => s.push_str(" fill=\"none\""),
        }
        if let Some((bw, bc)) = border {
            s.push_str(&paint_attr("stroke", bc));
            let _ = write!(s, " stroke-width=\"{}\"", num(bw));
        }
        s.push_str(&self.transform_attr());
        s.push_str("/>\n");
        self.body.push_str(&s);
    }

    fn fill_path(&mut self, path: &Path, color: Color) {
        self.push_path(path, &paint_attr("fill", color));
    }

    fn stroke_path(&mut self, path: &Path, stroke: &Stroke, color: Color) {
        let paint = format!(" fill=\"none\"{}", stroke_attrs(stroke, color));
        self.push_path(path, &paint);
    }

    fn line(&mut self, from: Point, to: Point, stroke: &Stroke, color: Color) {
        let _ = writeln!(self.body, "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"{}{}/>", num(from.x), num(from.y), num(to.x), num(to.y), stroke_attrs(stroke, color), self.transform_attr());
    }

    fn text(&mut self, text: &str, origin: Point, style: &TextStyle, color: Color) {
        if text.is_empty() {
            return;
        }
        if self.opts.text_as_paths
            && let Some(path) = self.fonts.and_then(|f| f.text_outline(text, style, origin))
        {
            self.push_path(&path, &paint_attr("fill", color));
            return;
        }
        self.push_text(text, origin, &style.family, style.size, style.weight.0, style.italic, style.align, color);
    }

    fn glyph(&mut self, font: FontId, codepoint: char, origin: Point, size: f32, color: Color) {
        let fonts = self.fonts;
        if self.opts.glyphs_as_paths
            && let Some(upem) = fonts.and_then(|f| f.info(font)).map(|i| i.units_per_em as f32)
        {
            let id = format!("g{}_{:X}", font.0, codepoint as u32);
            if !self.glyph_defs.contains(&(font, codepoint)) {
                // contorno em unidades da fonte (y já invertido); o <use> aplica a escala
                let Some(outline) = fonts.and_then(|f| f.glyph_outline(font, codepoint, upem)) else {
                    return; // a fonte não tem o glifo
                };
                let _ = writeln!(self.defs, "<path id=\"{id}\" d=\"{}\"/>", outline.to_svg_data());
                self.glyph_defs.insert((font, codepoint));
            }
            let k = size / upem;
            let t = self.cur.transform.pre_concat(&Transform::translate(origin.x, origin.y)).pre_concat(&Transform::scale(k, k));
            let _ = writeln!(self.body, "<use xlink:href=\"#{id}\"{}{}/>", paint_attr("fill", color), transform_attr(&t));
            return;
        }
        let (family, weight, italic) = match fonts.and_then(|f| f.info(font)) {
            Some(i) => (i.family.clone(), i.weight.0, i.italic),
            None => (format!("font{}", font.0), 400, false),
        };
        let mut buf = [0u8; 4];
        self.push_text(codepoint.encode_utf8(&mut buf), origin, &family, size, weight, italic, TextAlign::Left, color);
    }
}
