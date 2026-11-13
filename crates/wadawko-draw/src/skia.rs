//! Backend raster (tiny-skia): pinta num `Pixmap` RGBA e exporta PNG.
//! Texto e glifos são rasterizados a partir dos contornos da fonte (via [`FontBook`]), então
//! o resultado não depende de fontes instaladas no sistema.

use std::path::Path as FsPath;
use std::rc::Rc;

use tiny_skia as ts;

use crate::canvas::Canvas2D;
use crate::error::{DrawError, Result};
use crate::font::{FontBook, FontId};
use crate::geom::{Color, Point, Rect, Transform};
use crate::path::{Path, PathEl};
use crate::style::{LineCap, LineJoin, Quad, Stroke, TextStyle};

#[derive(Clone)]
struct SkState {
    transform: Transform,
    mask: Option<Rc<ts::Mask>>,
}

/// Canvas raster. Coordenadas do usuário em px lógicos; `scale` (device pixel ratio)
/// multiplica a resolução do bitmap.
pub struct SkiaCanvas<'a> {
    pixmap: ts::Pixmap,
    fonts: &'a FontBook,
    cur: SkState,
    stack: Vec<SkState>,
}

impl<'a> SkiaCanvas<'a> {
    /// Bitmap transparente de `width × height` px (escala 1).
    pub fn new(width: u32, height: u32, fonts: &'a FontBook) -> Result<Self> {
        Self::with_scale(width, height, 1.0, fonts)
    }

    /// Bitmap de `ceil(width·scale) × ceil(height·scale)` px físicos; o desenho continua em
    /// coordenadas lógicas `width × height`.
    pub fn with_scale(width: u32, height: u32, scale: f32, fonts: &'a FontBook) -> Result<Self> {
        let pw = (width as f32 * scale).ceil() as u32;
        let ph = (height as f32 * scale).ceil() as u32;
        let pixmap = ts::Pixmap::new(pw, ph).ok_or(DrawError::InvalidSize { width: pw, height: ph })?;
        Ok(SkiaCanvas {
            pixmap,
            fonts,
            cur: SkState { transform: Transform::scale(scale, scale), mask: None },
            stack: Vec::new(),
        })
    }

    /// Preenche o bitmap inteiro (ignora recorte e transformação).
    pub fn clear(&mut self, color: Color) {
        self.pixmap.fill(to_color(color));
    }

    /// Largura/altura em px físicos.
    pub fn pixel_size(&self) -> (u32, u32) {
        (self.pixmap.width(), self.pixmap.height())
    }
    pub fn pixmap(&self) -> &ts::Pixmap {
        &self.pixmap
    }
    pub fn into_pixmap(self) -> ts::Pixmap {
        self.pixmap
    }
    /// Bytes RGBA8 **pré-multiplicados**, linha a linha.
    pub fn data(&self) -> &[u8] {
        self.pixmap.data()
    }
    /// Pixel (px físicos) como RGBA8 não pré-multiplicado; `None` fora do bitmap.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        let c = self.pixmap.pixel(x, y)?.demultiply();
        Some([c.red(), c.green(), c.blue(), c.alpha()])
    }

    pub fn encode_png(&self) -> Result<Vec<u8>> {
        self.pixmap.encode_png().map_err(|e| DrawError::Png(e.to_string()))
    }
    pub fn save_png(&self, path: impl AsRef<FsPath>) -> Result<()> {
        let path = path.as_ref();
        let bytes = self.encode_png()?;
        std::fs::write(path, bytes).map_err(|source| DrawError::Io { path: path.to_path_buf(), source })
    }

    fn fill_ts(&mut self, path: &Path, color: Color) {
        let Some(p) = to_ts_path(path) else { return };
        let paint = paint_for(color);
        self.pixmap.fill_path(
            &p,
            &paint,
            ts::FillRule::Winding,
            to_ts_transform(&self.cur.transform),
            self.cur.mask.as_deref(),
        );
    }

    fn stroke_ts(&mut self, path: &Path, stroke: &Stroke, color: Color) {
        if stroke.width <= 0.0 {
            return;
        }
        let Some(p) = to_ts_path(path) else { return };
        let paint = paint_for(color);
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
        self.pixmap.stroke_path(
            &p,
            &paint,
            &st,
            to_ts_transform(&self.cur.transform),
            self.cur.mask.as_deref(),
        );
    }
}

fn to_color(c: Color) -> ts::Color {
    let q = |v: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
    ts::Color::from_rgba(q(c.r), q(c.g), q(c.b), q(c.a)).unwrap_or(ts::Color::TRANSPARENT)
}

fn paint_for(color: Color) -> ts::Paint<'static> {
    let mut paint = ts::Paint::default();
    paint.set_color(to_color(color));
    paint.anti_alias = true;
    paint
}

fn to_ts_transform(t: &Transform) -> ts::Transform {
    ts::Transform::from_row(t.a, t.b, t.c, t.d, t.e, t.f)
}

fn to_ts_path(path: &Path) -> Option<ts::Path> {
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
    b.finish()
}

impl Canvas2D for SkiaCanvas<'_> {
    fn save(&mut self) {
        self.stack.push(self.cur.clone());
    }

    fn restore(&mut self) {
        if let Some(s) = self.stack.pop() {
            self.cur = s;
        }
    }

    fn transform(&mut self, t: Transform) {
        self.cur.transform = self.cur.transform.pre_concat(&t);
    }

    fn clip_rect(&mut self, rect: Rect) {
        let ts_t = to_ts_transform(&self.cur.transform);
        // retângulo degenerado: recorta tudo
        let path = if rect.is_empty() { None } else { to_ts_path(&Path::rect(rect)) };
        let mut mask = match &self.cur.mask {
            Some(m) => (**m).clone(),
            None => {
                let Some(mut m) = ts::Mask::new(self.pixmap.width(), self.pixmap.height()) else { return };
                if let Some(p) = &path {
                    m.fill_path(p, ts::FillRule::Winding, true, ts_t);
                }
                self.cur.mask = Some(Rc::new(m));
                return;
            }
        };
        match &path {
            Some(p) => mask.intersect_path(p, ts::FillRule::Winding, true, ts_t),
            None => mask.clear(),
        }
        self.cur.mask = Some(Rc::new(mask));
    }

    fn quad(&mut self, q: &Quad) {
        if let Some(fill) = q.fill {
            self.fill_ts(&Path::rounded_rect(q.rect, q.radius), fill);
        }
        if let Some((bw, bc)) = q.border_parts() {
            let inner = q.rect.inset(bw * 0.5);
            let path = Path::rounded_rect(inner, (q.radius - bw * 0.5).max(0.0));
            self.stroke_ts(&path, &Stroke::new(bw), bc);
        }
    }

    fn fill_path(&mut self, path: &Path, color: Color) {
        self.fill_ts(path, color);
    }

    fn stroke_path(&mut self, path: &Path, stroke: &Stroke, color: Color) {
        self.stroke_ts(path, stroke, color);
    }

    fn text(&mut self, text: &str, origin: Point, style: &TextStyle, color: Color) {
        // família não registrada: nada é pintado (sem fonte não há contorno)
        if let Some(path) = self.fonts.text_outline(text, style, origin) {
            self.fill_ts(&path, color);
        }
    }

    fn glyph(&mut self, font: FontId, codepoint: char, origin: Point, size: f32, color: Color) {
        if let Some(outline) = self.fonts.glyph_outline(font, codepoint, size) {
            self.fill_ts(&outline.translated(origin.x, origin.y), color);
        }
    }
}
