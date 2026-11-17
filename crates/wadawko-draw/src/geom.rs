//! Geometria básica: ponto, retângulo, transformação afim e cor.
//!
//! Convenção: pixels lógicos em `f32`, origem no canto superior esquerdo, **y para baixo**
//! (igual a GPUI, SVG e tiny-skia).

/// Ponto 2D (px lógicos, y para baixo).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// Atalho para [`Point`].
pub const fn pt(x: f32, y: f32) -> Point {
    Point { x, y }
}

impl Point {
    pub const ZERO: Point = pt(0.0, 0.0);

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        pt(self.x + dx, self.y + dy)
    }
}

impl std::ops::Add for Point {
    type Output = Point;
    fn add(self, o: Point) -> Point {
        pt(self.x + o.x, self.y + o.y)
    }
}
impl std::ops::Sub for Point {
    type Output = Point;
    fn sub(self, o: Point) -> Point {
        pt(self.x - o.x, self.y - o.y)
    }
}

/// Retângulo alinhado aos eixos: origem (canto superior esquerdo) + tamanho.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Atalho para [`Rect`].
pub const fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    /// A partir de esquerda/topo/direita/base.
    pub fn from_ltrb(l: f32, t: f32, r: f32, b: f32) -> Self {
        rect(l, t, r - l, b - t)
    }
    pub fn left(&self) -> f32 {
        self.x
    }
    pub fn top(&self) -> f32 {
        self.y
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn origin(&self) -> Point {
        pt(self.x, self.y)
    }
    pub fn center(&self) -> Point {
        pt(self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
    pub fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }
    pub fn translate(&self, dx: f32, dy: f32) -> Rect {
        rect(self.x + dx, self.y + dy, self.w, self.h)
    }
    /// Encolhe (d > 0) ou expande (d < 0) em todos os lados.
    pub fn inset(&self, d: f32) -> Rect {
        rect(self.x + d, self.y + d, (self.w - 2.0 * d).max(0.0), (self.h - 2.0 * d).max(0.0))
    }
    /// Interseção; retângulo vazio (w = h = 0) se não houver sobreposição.
    pub fn intersect(&self, o: &Rect) -> Rect {
        let l = self.left().max(o.left());
        let t = self.top().max(o.top());
        let r = self.right().min(o.right());
        let b = self.bottom().min(o.bottom());
        if r <= l || b <= t { rect(l, t, 0.0, 0.0) } else { Rect::from_ltrb(l, t, r, b) }
    }
    /// Menor retângulo que contém os dois.
    pub fn union(&self, o: &Rect) -> Rect {
        Rect::from_ltrb(self.left().min(o.left()), self.top().min(o.top()), self.right().max(o.right()), self.bottom().max(o.bottom()))
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        !self.intersect(o).is_empty()
    }
}

/// Transformação afim 2D, na ordem do `matrix(a b c d e f)` do SVG:
/// `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Transform = Transform { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    pub const fn translate(dx: f32, dy: f32) -> Self {
        Transform { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: dx, f: dy }
    }
    pub const fn scale(sx: f32, sy: f32) -> Self {
        Transform { a: sx, b: 0.0, c: 0.0, d: sy, e: 0.0, f: 0.0 }
    }
    /// Rotação em radianos (sentido horário na tela, pois y cresce para baixo).
    pub fn rotate(rad: f32) -> Self {
        let (s, c) = rad.sin_cos();
        Transform { a: c, b: s, c: -s, d: c, e: 0.0, f: 0.0 }
    }
    /// Cisalhamento horizontal: `x' = x + k·y` (usado no itálico sintético).
    pub const fn skew_x(k: f32) -> Self {
        Transform { a: 1.0, b: 0.0, c: k, d: 1.0, e: 0.0, f: 0.0 }
    }

    /// Composição `self ∘ inner`: aplica `inner` primeiro, depois `self`
    /// (é o que `Canvas2D::transform` faz com a CTM, como no canvas HTML/SVG).
    pub fn pre_concat(&self, inner: &Transform) -> Transform {
        let s = self;
        let t = inner;
        Transform { a: s.a * t.a + s.c * t.b, b: s.b * t.a + s.d * t.b, c: s.a * t.c + s.c * t.d, d: s.b * t.c + s.d * t.d, e: s.a * t.e + s.c * t.f + s.e, f: s.b * t.e + s.d * t.f + s.f }
    }

    pub fn apply(&self, p: Point) -> Point {
        pt(self.a * p.x + self.c * p.y + self.e, self.b * p.x + self.d * p.y + self.f)
    }

    /// Caixa envolvente do retângulo transformado.
    pub fn apply_rect(&self, r: &Rect) -> Rect {
        let ps = [self.apply(pt(r.left(), r.top())), self.apply(pt(r.right(), r.top())), self.apply(pt(r.right(), r.bottom())), self.apply(pt(r.left(), r.bottom()))];
        let (mut l, mut t, mut rr, mut b) = (ps[0].x, ps[0].y, ps[0].x, ps[0].y);
        for p in &ps[1..] {
            l = l.min(p.x);
            t = t.min(p.y);
            rr = rr.max(p.x);
            b = b.max(p.y);
        }
        Rect::from_ltrb(l, t, rr, b)
    }

    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }
    /// Só translação (sem escala/rotação).
    pub fn is_translation(&self) -> bool {
        self.a == 1.0 && self.b == 0.0 && self.c == 0.0 && self.d == 1.0
    }
    /// Sem rotação/cisalhamento: retângulos continuam retângulos. Backends baseados em quads
    /// (GPUI) podem pintar quads/texto diretamente quando isto é verdadeiro.
    pub fn is_axis_aligned(&self) -> bool {
        self.b == 0.0 && self.c == 0.0
    }
    /// Fator de escala médio (para converter espessuras de traço e tamanhos de fonte).
    pub fn mean_scale(&self) -> f32 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }
}

/// Cor RGBA linear-ingênua (sRGB não pré-multiplicado), componentes em `0.0..=1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
    /// `0xRRGGBB`, opaco.
    pub const fn rgb(hex: u32) -> Self {
        Self::rgba8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255)
    }
    /// `0xRRGGBBAA`.
    pub const fn rgba(hex: u32) -> Self {
        Self::rgba8((hex >> 24) as u8, (hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }
    pub const fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: a as f32 / 255.0 }
    }
    /// Mesma cor com outro alfa.
    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }
    /// Multiplica o alfa atual.
    pub fn fade(self, k: f32) -> Self {
        Self { a: self.a * k, ..self }
    }
    pub fn to_rgba8(self) -> [u8; 4] {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        [q(self.r), q(self.g), q(self.b), q(self.a)]
    }
    /// `#rrggbb` (sem alfa).
    pub fn hex_rgb(self) -> String {
        let [r, g, b, _] = self.to_rgba8();
        format!("#{r:02x}{g:02x}{b:02x}")
    }
    pub fn is_opaque(self) -> bool {
        self.a >= 1.0
    }
}
