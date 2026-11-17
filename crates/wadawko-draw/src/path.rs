//! Caminhos vetoriais (move/line/quad/cubic/close) e seu construtor.

use crate::geom::{Point, Rect, Transform, pt};

/// Constante para aproximar um quarto de círculo com uma cúbica.
const KAPPA: f32 = 0.552_284_8;

/// Elemento de um [`Path`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathEl {
    MoveTo(Point),
    LineTo(Point),
    /// Bézier quadrática: (controle, fim).
    QuadTo(Point, Point),
    /// Bézier cúbica: (controle 1, controle 2, fim).
    CubicTo(Point, Point, Point),
    Close,
}

/// Caminho imutável. O preenchimento usa a regra **non-zero** (a mesma dos contornos de fonte).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Path {
    els: Vec<PathEl>,
}

impl Path {
    pub fn builder() -> PathBuilder {
        PathBuilder::new()
    }
    pub fn from_elements(els: Vec<PathEl>) -> Self {
        Self { els }
    }
    pub fn elements(&self) -> &[PathEl] {
        &self.els
    }
    pub fn is_empty(&self) -> bool {
        self.els.is_empty()
    }

    pub fn line(a: Point, b: Point) -> Path {
        let mut p = PathBuilder::new();
        p.move_to(a).line_to(b);
        p.build()
    }
    pub fn rect(r: Rect) -> Path {
        let mut p = PathBuilder::new();
        p.rect(r);
        p.build()
    }
    pub fn rounded_rect(r: Rect, radius: f32) -> Path {
        let mut p = PathBuilder::new();
        p.rounded_rect(r, radius);
        p.build()
    }
    pub fn circle(center: Point, radius: f32) -> Path {
        Self::ellipse(center, radius, radius)
    }
    pub fn ellipse(center: Point, rx: f32, ry: f32) -> Path {
        let mut p = PathBuilder::new();
        p.ellipse(center, rx, ry);
        p.build()
    }
    /// Polígono fechado.
    pub fn polygon(points: &[Point]) -> Path {
        let mut p = PathBuilder::new();
        if let Some((first, rest)) = points.split_first() {
            p.move_to(*first);
            for q in rest {
                p.line_to(*q);
            }
            p.close();
        }
        p.build()
    }

    /// Cópia com todos os pontos transformados.
    pub fn transformed(&self, t: &Transform) -> Path {
        if t.is_identity() {
            return self.clone();
        }
        let els = self
            .els
            .iter()
            .map(|el| match *el {
                PathEl::MoveTo(p) => PathEl::MoveTo(t.apply(p)),
                PathEl::LineTo(p) => PathEl::LineTo(t.apply(p)),
                PathEl::QuadTo(c, p) => PathEl::QuadTo(t.apply(c), t.apply(p)),
                PathEl::CubicTo(c1, c2, p) => PathEl::CubicTo(t.apply(c1), t.apply(c2), t.apply(p)),
                PathEl::Close => PathEl::Close,
            })
            .collect();
        Path { els }
    }
    pub fn translated(&self, dx: f32, dy: f32) -> Path {
        self.transformed(&Transform::translate(dx, dy))
    }

    /// Acrescenta os elementos de outro caminho (subcaminhos independentes).
    pub fn extend(&mut self, other: &Path) {
        self.els.extend_from_slice(&other.els);
    }

    /// Caixa envolvente dos pontos (inclui pontos de controle: é conservadora, nunca menor
    /// que a forma). `None` para caminho vazio.
    pub fn bounds(&self) -> Option<Rect> {
        let mut b: Option<(f32, f32, f32, f32)> = None;
        let mut add = |p: Point| {
            b = Some(match b {
                None => (p.x, p.y, p.x, p.y),
                Some((l, t, r, bo)) => (l.min(p.x), t.min(p.y), r.max(p.x), bo.max(p.y)),
            });
        };
        for el in &self.els {
            match *el {
                PathEl::MoveTo(p) | PathEl::LineTo(p) => add(p),
                PathEl::QuadTo(c, p) => {
                    add(c);
                    add(p)
                }
                PathEl::CubicTo(c1, c2, p) => {
                    add(c1);
                    add(c2);
                    add(p)
                }
                PathEl::Close => {}
            }
        }
        b.map(|(l, t, r, bo)| Rect::from_ltrb(l, t, r, bo))
    }

    /// Dados no formato do atributo `d` do SVG.
    pub fn to_svg_data(&self) -> String {
        use crate::svg::num;
        let mut s = String::with_capacity(self.els.len() * 12);
        for el in &self.els {
            if !s.is_empty() {
                s.push(' ');
            }
            match *el {
                PathEl::MoveTo(p) => s.push_str(&format!("M{} {}", num(p.x), num(p.y))),
                PathEl::LineTo(p) => s.push_str(&format!("L{} {}", num(p.x), num(p.y))),
                PathEl::QuadTo(c, p) => s.push_str(&format!("Q{} {} {} {}", num(c.x), num(c.y), num(p.x), num(p.y))),
                PathEl::CubicTo(c1, c2, p) => s.push_str(&format!("C{} {} {} {} {} {}", num(c1.x), num(c1.y), num(c2.x), num(c2.y), num(p.x), num(p.y))),
                PathEl::Close => s.push('Z'),
            }
        }
        s
    }
}

/// Construtor de [`Path`] (encadeável).
#[derive(Clone, Debug, Default)]
pub struct PathBuilder {
    els: Vec<PathEl>,
}

impl PathBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn move_to(&mut self, p: Point) -> &mut Self {
        self.els.push(PathEl::MoveTo(p));
        self
    }
    pub fn line_to(&mut self, p: Point) -> &mut Self {
        self.els.push(PathEl::LineTo(p));
        self
    }
    pub fn quad_to(&mut self, ctrl: Point, p: Point) -> &mut Self {
        self.els.push(PathEl::QuadTo(ctrl, p));
        self
    }
    pub fn cubic_to(&mut self, c1: Point, c2: Point, p: Point) -> &mut Self {
        self.els.push(PathEl::CubicTo(c1, c2, p));
        self
    }
    pub fn close(&mut self) -> &mut Self {
        self.els.push(PathEl::Close);
        self
    }
    /// Subcaminho retangular fechado (sentido horário na tela).
    pub fn rect(&mut self, r: Rect) -> &mut Self {
        self.move_to(pt(r.left(), r.top())).line_to(pt(r.right(), r.top())).line_to(pt(r.right(), r.bottom())).line_to(pt(r.left(), r.bottom())).close()
    }
    /// Subcaminho de retângulo com cantos arredondados (raio limitado a metade do menor lado).
    pub fn rounded_rect(&mut self, r: Rect, radius: f32) -> &mut Self {
        let rad = radius.min(r.w * 0.5).min(r.h * 0.5).max(0.0);
        if rad <= 0.0 {
            return self.rect(r);
        }
        let k = rad * (1.0 - KAPPA);
        let (l, t, rr, b) = (r.left(), r.top(), r.right(), r.bottom());
        self.move_to(pt(l + rad, t))
            .line_to(pt(rr - rad, t))
            .cubic_to(pt(rr - k, t), pt(rr, t + k), pt(rr, t + rad))
            .line_to(pt(rr, b - rad))
            .cubic_to(pt(rr, b - k), pt(rr - k, b), pt(rr - rad, b))
            .line_to(pt(l + rad, b))
            .cubic_to(pt(l + k, b), pt(l, b - k), pt(l, b - rad))
            .line_to(pt(l, t + rad))
            .cubic_to(pt(l, t + k), pt(l + k, t), pt(l + rad, t))
            .close()
    }
    /// Subcaminho de elipse (4 cúbicas).
    pub fn ellipse(&mut self, c: Point, rx: f32, ry: f32) -> &mut Self {
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        self.move_to(pt(c.x + rx, c.y))
            .cubic_to(pt(c.x + rx, c.y + ky), pt(c.x + kx, c.y + ry), pt(c.x, c.y + ry))
            .cubic_to(pt(c.x - kx, c.y + ry), pt(c.x - rx, c.y + ky), pt(c.x - rx, c.y))
            .cubic_to(pt(c.x - rx, c.y - ky), pt(c.x - kx, c.y - ry), pt(c.x, c.y - ry))
            .cubic_to(pt(c.x + kx, c.y - ry), pt(c.x + rx, c.y - ky), pt(c.x + rx, c.y))
            .close()
    }
    pub fn circle(&mut self, c: Point, r: f32) -> &mut Self {
        self.ellipse(c, r, r)
    }
    pub fn is_empty(&self) -> bool {
        self.els.is_empty()
    }
    /// Finaliza (o construtor fica vazio e pode ser reutilizado).
    pub fn build(&mut self) -> Path {
        Path { els: std::mem::take(&mut self.els) }
    }
}
