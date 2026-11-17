//! Fontes sem backend: registro ([`FontBook`]), métricas, medição de texto e contornos de
//! glifos via `ttf-parser`.
//!
//! Limitações assumidas (suficientes para UI/sílabas/partitura): texto de uma linha, sem
//! kerning/ligaduras/bidi; cada caractere vira um glifo pelo `cmap`. Caracteres ausentes na
//! fonte pedida caem nas outras fontes do livro (ordem de registro).

use std::borrow::Cow;
use std::path::Path as FsPath;

use ttf_parser::{Face, GlyphId};

use crate::error::{DrawError, Result};
use crate::geom::{Point, Rect, Transform};
use crate::path::{Path, PathBuilder, PathEl};
use crate::style::{FontWeight, TextAlign, TextStyle};

/// Inclinação do itálico sintético (quando a família não tem face itálica).
pub const SYNTHETIC_ITALIC_SKEW: f32 = 0.2;

/// Identificador de uma fonte registrada num [`FontBook`] (índice de registro).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FontId(pub u32);

/// Dados de identificação de uma face registrada.
#[derive(Clone, Debug, PartialEq)]
pub struct FontInfo {
    pub family: String,
    pub weight: FontWeight,
    pub italic: bool,
    pub units_per_em: u16,
}

/// Métricas verticais de uma fonte num tamanho (px). `descent` é **positivo** para baixo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontMetrics {
    pub size: f32,
    pub ascent: f32,
    pub descent: f32,
    pub line_gap: f32,
    pub cap_height: f32,
    pub x_height: f32,
    pub units_per_em: u16,
}

impl FontMetrics {
    /// Altura de linha natural: ascent + descent + line gap.
    pub fn line_height(&self) -> f32 {
        self.ascent + self.descent + self.line_gap
    }
}

/// Métricas de um glifo num tamanho. `bbox` é relativo à origem do glifo (linha de base),
/// com **y para baixo** — some `origin` para obter a caixa na tela.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphMetrics {
    pub advance: f32,
    pub bbox: Rect,
}

/// Resultado de [`FontBook::measure_text`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMetrics {
    /// Soma dos avanços.
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    pub line_height: f32,
}

impl TextMetrics {
    /// Caixa da linha dado o ponto de origem (linha de base) e o alinhamento.
    pub fn bounds(&self, origin: Point, align: TextAlign) -> Rect {
        let x = origin.x - align_offset(align, self.width);
        Rect::new(x, origin.y - self.ascent, self.width, self.ascent + self.descent)
    }
}

/// Um glifo posicionado por [`FontBook::layout_text`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedGlyph {
    /// Fonte efetiva (pode ser uma de fallback).
    pub font: FontId,
    pub glyph_id: u16,
    pub ch: char,
    /// Deslocamento x a partir do início da linha (antes do alinhamento).
    pub x: f32,
    pub advance: f32,
}

/// Linha de texto posicionada.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayout {
    /// Fonte primária resolvida para o estilo.
    pub font: FontId,
    pub glyphs: Vec<PlacedGlyph>,
    pub width: f32,
}

pub(crate) fn align_offset(align: TextAlign, width: f32) -> f32 {
    match align {
        TextAlign::Left => 0.0,
        TextAlign::Center => width * 0.5,
        TextAlign::Right => width,
    }
}

struct FontEntry {
    data: Cow<'static, [u8]>,
    index: u32,
    info: FontInfo,
    ascender: i16,
    descender: i16,
    line_gap: i16,
    cap_height: i16,
    x_height: i16,
}

/// Registro de fontes (bytes TTF/OTF em memória) com medição e contornos.
///
/// O mesmo `FontBook` alimenta os backends SVG/tiny-skia e a notação; o backend GPUI pode
/// registrar os mesmos bytes no `TextSystem` via [`FontBook::data`].
#[derive(Default)]
pub struct FontBook {
    fonts: Vec<FontEntry>,
}

impl std::fmt::Debug for FontBook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.fonts.iter().map(|e| &e.info)).finish()
    }
}

impl FontBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra uma face a partir de bytes (`Vec<u8>` ou `&'static [u8]` de `include_bytes!`).
    /// Família, peso e itálico são lidos da própria fonte.
    pub fn add(&mut self, data: impl Into<Cow<'static, [u8]>>) -> Result<FontId> {
        self.add_indexed(data, 0)
    }

    /// Como [`add`](Self::add), escolhendo a face dentro de uma coleção (.ttc).
    pub fn add_indexed(&mut self, data: impl Into<Cow<'static, [u8]>>, index: u32) -> Result<FontId> {
        let data = data.into();
        let entry = {
            let face = Face::parse(&data, index).map_err(|e| DrawError::FontParse(e.to_string()))?;
            let info = FontInfo {
                family: family_name(&face).unwrap_or_else(|| "unknown".to_owned()),
                weight: FontWeight(face.weight().to_number()),
                italic: face.is_italic(),
                units_per_em: face.units_per_em(),
            };
            let ascender = face.ascender();
            let descender = face.descender();
            let line_gap = face.line_gap();
            let cap_height = face.capital_height().unwrap_or((ascender as f32 * 0.7) as i16);
            let x_height = face.x_height().unwrap_or((ascender as f32 * 0.5) as i16);
            (info, ascender, descender, line_gap, cap_height, x_height)
        };
        let (info, ascender, descender, line_gap, cap_height, x_height) = entry;
        self.fonts.push(FontEntry { data, index, info, ascender, descender, line_gap, cap_height, x_height });
        Ok(FontId(self.fonts.len() as u32 - 1))
    }

    /// Lê e registra um arquivo .ttf/.otf.
    pub fn load_file(&mut self, path: impl AsRef<FsPath>) -> Result<FontId> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| DrawError::Io { path: path.to_path_buf(), source })?;
        self.add(bytes)
    }

    /// Sobrescreve o nome de família usado por [`resolve`](Self::resolve).
    pub fn set_family(&mut self, id: FontId, family: impl Into<String>) {
        if let Some(e) = self.fonts.get_mut(id.0 as usize) {
            e.info.family = family.into();
        }
    }

    pub fn len(&self) -> usize {
        self.fonts.len()
    }
    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }
    /// Todos os ids, na ordem de registro.
    pub fn ids(&self) -> impl Iterator<Item = FontId> + '_ {
        (0..self.fonts.len() as u32).map(FontId)
    }
    pub fn info(&self, id: FontId) -> Option<&FontInfo> {
        self.entry(id).map(|e| &e.info)
    }
    /// Bytes da fonte (para registrar no backend, ex.: `cx.text_system().add_fonts`).
    pub fn data(&self, id: FontId) -> Option<&[u8]> {
        self.entry(id).map(|e| &*e.data)
    }

    /// Face que melhor atende família (sem diferenciar maiúsculas) + peso + itálico.
    /// Prioridade: mesma inclinação, depois peso mais próximo. `None` se a família não existe.
    pub fn resolve(&self, family: &str, weight: FontWeight, italic: bool) -> Option<FontId> {
        self.fonts
            .iter()
            .enumerate()
            .filter(|(_, e)| e.info.family.eq_ignore_ascii_case(family))
            .min_by_key(|(_, e)| {
                let slant = if e.info.italic == italic { 0u32 } else { 10_000 };
                slant + (e.info.weight.0 as i32 - weight.0 as i32).unsigned_abs()
            })
            .map(|(i, _)| FontId(i as u32))
    }

    /// [`resolve`](Self::resolve) a partir de um [`TextStyle`].
    pub fn resolve_style(&self, style: &TextStyle) -> Option<FontId> {
        self.resolve(&style.family, style.weight, style.italic)
    }

    /// Métricas verticais no tamanho dado.
    pub fn metrics(&self, id: FontId, size: f32) -> Option<FontMetrics> {
        let e = self.entry(id)?;
        let k = size / e.info.units_per_em as f32;
        Some(FontMetrics {
            size,
            ascent: e.ascender as f32 * k,
            descent: -(e.descender as f32) * k,
            line_gap: e.line_gap as f32 * k,
            cap_height: e.cap_height as f32 * k,
            x_height: e.x_height as f32 * k,
            units_per_em: e.info.units_per_em,
        })
    }

    pub fn has_glyph(&self, id: FontId, ch: char) -> bool {
        self.face(id).and_then(|f| f.glyph_index(ch)).is_some()
    }

    /// Avanço horizontal do glifo (px).
    pub fn glyph_advance(&self, id: FontId, ch: char, size: f32) -> Option<f32> {
        let face = self.face(id)?;
        let gid = face.glyph_index(ch)?;
        Some(face.glyph_hor_advance(gid)? as f32 * size / face.units_per_em() as f32)
    }

    /// Caixa do glifo em **unidades da fonte, y para cima** (`[x_min, y_min, x_max, y_max]`),
    /// como no arquivo — comparável direto com `glyphBBoxes` do SMuFL (× em/4).
    pub fn glyph_bbox_units(&self, id: FontId, ch: char) -> Option<[f32; 4]> {
        let face = self.face(id)?;
        let gid = face.glyph_index(ch)?;
        let b = face.glyph_bounding_box(gid)?;
        Some([b.x_min as f32, b.y_min as f32, b.x_max as f32, b.y_max as f32])
    }

    /// Avanço + caixa do glifo em px (y para baixo, relativo à origem do glifo).
    /// Glifos sem contorno (espaço) têm caixa vazia.
    pub fn glyph_metrics(&self, id: FontId, ch: char, size: f32) -> Option<GlyphMetrics> {
        let face = self.face(id)?;
        let gid = face.glyph_index(ch)?;
        let k = size / face.units_per_em() as f32;
        let advance = face.glyph_hor_advance(gid).unwrap_or(0) as f32 * k;
        let bbox = match face.glyph_bounding_box(gid) {
            Some(b) => Rect::from_ltrb(b.x_min as f32 * k, -(b.y_max as f32) * k, b.x_max as f32 * k, -(b.y_min as f32) * k),
            None => Rect::default(),
        };
        Some(GlyphMetrics { advance, bbox })
    }

    /// Contorno do glifo como [`Path`] em px (y para baixo), com a origem do glifo em (0, 0).
    /// `None` se a fonte não tem o caractere; caminho vazio para glifos sem contorno.
    pub fn glyph_outline(&self, id: FontId, ch: char, size: f32) -> Option<Path> {
        let face = self.face(id)?;
        let gid = face.glyph_index(ch)?;
        let mut sink = OutlineSink::new(size / face.units_per_em() as f32, 0.0, 0.0);
        face.outline_glyph(gid, &mut sink);
        Some(sink.finish())
    }

    /// Posiciona os glifos de uma linha (sem alinhamento). `None` se a família não resolve.
    pub fn layout_text(&self, text: &str, style: &TextStyle) -> Option<TextLayout> {
        let font = self.resolve_style(style)?;
        Some(self.layout_with(text, font, style.size, |_, _, _, _| {}))
    }

    /// Mede uma linha de texto. `None` se a família do estilo não está registrada.
    pub fn measure_text(&self, text: &str, style: &TextStyle) -> Option<TextMetrics> {
        let layout = self.layout_text(text, style)?;
        let m = self.metrics(layout.font, style.size)?;
        Some(TextMetrics { width: layout.width, ascent: m.ascent, descent: m.descent, line_height: m.line_height() })
    }

    /// Largura (soma dos avanços) de uma linha numa fonte específica.
    pub fn text_width(&self, text: &str, font: FontId, size: f32) -> f32 {
        self.layout_with(text, font, size, |_, _, _, _| {}).width
    }

    /// Contorno de uma linha de texto já alinhada em `origin` (linha de base), em px.
    /// Aplica itálico sintético se o estilo pede itálico e a face resolvida não é itálica.
    /// `None` se a família não resolve.
    pub fn text_outline(&self, text: &str, style: &TextStyle, origin: Point) -> Option<Path> {
        let font = self.resolve_style(style)?;
        let mut els: Vec<PathEl> = Vec::new();
        let layout = self.layout_with(text, font, style.size, |face, gid, x, k| {
            let mut sink = OutlineSink::new(k, x, 0.0);
            face.outline_glyph(gid, &mut sink);
            els.extend_from_slice(sink.finish().elements());
        });
        let mut t = Transform::translate(origin.x - align_offset(style.align, layout.width), origin.y);
        if style.italic && !self.info(font).map(|i| i.italic).unwrap_or(false) {
            t = t.pre_concat(&Transform::skew_x(-SYNTHETIC_ITALIC_SKEW));
        }
        Some(Path::from_elements(els).transformed(&t))
    }

    // ---- internos ----

    fn entry(&self, id: FontId) -> Option<&FontEntry> {
        self.fonts.get(id.0 as usize)
    }

    fn face(&self, id: FontId) -> Option<Face<'_>> {
        let e = self.entry(id)?;
        Face::parse(&e.data, e.index).ok()
    }

    /// Layout simples por cmap + avanço, com fallback por caractere. `visit(face, glifo, x, escala)`
    /// é chamado para cada glifo com a face que efetivamente o contém.
    fn layout_with(&self, text: &str, font: FontId, size: f32, mut visit: impl FnMut(&Face<'_>, GlyphId, f32, f32)) -> TextLayout {
        let mut faces: Vec<Option<Option<Face<'_>>>> = (0..self.fonts.len()).map(|_| None).collect();
        let mut glyphs = Vec::with_capacity(text.len());
        let mut x = 0.0f32;
        for ch in text.chars() {
            if ch.is_control() {
                continue;
            }
            // primária primeiro, depois as demais na ordem de registro
            let order = std::iter::once(font.0 as usize).chain((0..self.fonts.len()).filter(|i| *i != font.0 as usize));
            let mut found: Option<(usize, GlyphId)> = None;
            for i in order {
                if i >= self.fonts.len() {
                    continue;
                }
                let slot = faces[i].get_or_insert_with(|| self.face(FontId(i as u32)));
                if let Some(face) = slot
                    && let Some(gid) = face.glyph_index(ch)
                {
                    found = Some((i, gid));
                    break;
                }
            }
            // sem glifo em nenhuma fonte: .notdef da primária
            let (i, gid) = found.unwrap_or((font.0 as usize, GlyphId(0)));
            let Some(Some(Some(face))) = faces.get(i) else { continue };
            let k = size / face.units_per_em() as f32;
            let advance = face.glyph_hor_advance(gid).unwrap_or(0) as f32 * k;
            visit(face, gid, x, k);
            glyphs.push(PlacedGlyph { font: FontId(i as u32), glyph_id: gid.0, ch, x, advance });
            x += advance;
        }
        TextLayout { font, glyphs, width: x }
    }
}

fn family_name(face: &Face<'_>) -> Option<String> {
    use ttf_parser::name_id::{FAMILY, TYPOGRAPHIC_FAMILY};
    let pick = |id: u16| face.names().into_iter().filter(|n| n.name_id == id && n.is_unicode()).find_map(|n| n.to_string());
    pick(TYPOGRAPHIC_FAMILY).or_else(|| pick(FAMILY))
}

/// Converte o contorno da fonte (unidades, y para cima) em [`Path`] (px, y para baixo).
struct OutlineSink {
    b: PathBuilder,
    k: f32,
    dx: f32,
    dy: f32,
}

impl OutlineSink {
    fn new(k: f32, dx: f32, dy: f32) -> Self {
        OutlineSink { b: PathBuilder::new(), k, dx, dy }
    }
    fn p(&self, x: f32, y: f32) -> Point {
        Point::new(self.dx + x * self.k, self.dy - y * self.k)
    }
    fn finish(mut self) -> Path {
        self.b.build()
    }
}

impl ttf_parser::OutlineBuilder for OutlineSink {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.p(x, y);
        self.b.move_to(p);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.p(x, y);
        self.b.line_to(p);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (c, p) = (self.p(x1, y1), self.p(x, y));
        self.b.quad_to(c, p);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (c1, c2, p) = (self.p(x1, y1), self.p(x2, y2), self.p(x, y));
        self.b.cubic_to(c1, c2, p);
    }
    fn close(&mut self) {
        self.b.close();
    }
}
