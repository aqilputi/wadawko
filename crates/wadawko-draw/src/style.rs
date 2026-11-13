//! Estilos de pintura: traço, quad e texto.

use std::borrow::Cow;

use crate::geom::{Color, Rect};

/// Terminação de linha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

/// Junção de segmentos.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// Estilo de traço (contorno), centrado no caminho.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub cap: LineCap,
    pub join: LineJoin,
}

impl Default for Stroke {
    fn default() -> Self {
        Stroke::new(1.0)
    }
}

impl Stroke {
    pub const fn new(width: f32) -> Self {
        Stroke { width, cap: LineCap::Butt, join: LineJoin::Miter }
    }
    pub const fn cap(mut self, cap: LineCap) -> Self {
        self.cap = cap;
        self
    }
    pub const fn join(mut self, join: LineJoin) -> Self {
        self.join = join;
        self
    }
    /// Traço com pontas e junções arredondadas.
    pub const fn round(width: f32) -> Self {
        Stroke { width, cap: LineCap::Round, join: LineJoin::Round }
    }
}

/// Retângulo com raio uniforme, preenchimento e borda opcionais — o equivalente direto do
/// `paint_quad(quad(bounds, raio, fundo, largura_borda, cor_borda, ..))` do GPUI.
/// A borda é desenhada **para dentro** do retângulo (como no GPUI/CSS `border-box`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub rect: Rect,
    pub radius: f32,
    pub fill: Option<Color>,
    pub border_width: f32,
    pub border_color: Option<Color>,
}

impl Quad {
    /// Retângulo preenchido.
    pub const fn fill(rect: Rect, color: Color) -> Self {
        Quad { rect, radius: 0.0, fill: Some(color), border_width: 0.0, border_color: None }
    }
    /// Só contorno (borda interna).
    pub const fn outline(rect: Rect, width: f32, color: Color) -> Self {
        Quad { rect, radius: 0.0, fill: None, border_width: width, border_color: Some(color) }
    }
    pub const fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }
    pub const fn border(mut self, width: f32, color: Color) -> Self {
        self.border_width = width;
        self.border_color = Some(color);
        self
    }
    /// Borda efetiva (largura > 0 e cor definida).
    pub fn border_parts(&self) -> Option<(f32, Color)> {
        match self.border_color {
            Some(c) if self.border_width > 0.0 => Some((self.border_width, c)),
            _ => None,
        }
    }
}

/// Alinhamento horizontal do texto em relação ao ponto de origem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// A origem é o início (esquerda) da linha.
    #[default]
    Left,
    /// A origem é o centro da linha.
    Center,
    /// A origem é o fim (direita) da linha.
    Right,
}

/// Peso de fonte no padrão CSS/OpenType (100–900).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontWeight(pub u16);

impl FontWeight {
    pub const LIGHT: FontWeight = FontWeight(300);
    pub const REGULAR: FontWeight = FontWeight(400);
    pub const MEDIUM: FontWeight = FontWeight(500);
    pub const SEMIBOLD: FontWeight = FontWeight(600);
    pub const BOLD: FontWeight = FontWeight(700);
}

impl Default for FontWeight {
    fn default() -> Self {
        FontWeight::REGULAR
    }
}

/// Estilo de uma linha de texto. O texto é sempre de **uma linha**, ancorado na linha de base.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// Família (ex.: `"Zen Kaku Gothic New"`), resolvida pelo [`crate::FontBook`] ou pelo backend.
    pub family: Cow<'static, str>,
    /// Tamanho em px (altura do quadrado em).
    pub size: f32,
    pub weight: FontWeight,
    pub italic: bool,
    pub align: TextAlign,
}

impl TextStyle {
    pub fn new(family: impl Into<Cow<'static, str>>, size: f32) -> Self {
        TextStyle {
            family: family.into(),
            size,
            weight: FontWeight::REGULAR,
            italic: false,
            align: TextAlign::Left,
        }
    }
    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }
    pub fn bold(self) -> Self {
        self.weight(FontWeight::BOLD)
    }
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}
