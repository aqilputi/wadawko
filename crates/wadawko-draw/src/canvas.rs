//! O trait [`Canvas2D`] e a pilha de estado reutilizável pelos backends.

use crate::font::FontId;
use crate::geom::{Color, Point, Rect, Transform};
use crate::path::Path;
use crate::style::{Quad, Stroke, TextStyle};

/// Superfície de desenho 2D independente de backend.
///
/// Modelo de estado igual ao do canvas HTML: uma transformação corrente (CTM) e um recorte
/// corrente, empilhados por [`save`](Canvas2D::save)/[`restore`](Canvas2D::restore).
/// Coordenadas em px lógicos, y para baixo. O trait é *object-safe* (`&mut dyn Canvas2D`).
///
/// Um backend só precisa implementar os 9 métodos obrigatórios; o resto tem implementação
/// padrão em cima deles.
pub trait Canvas2D {
    /// Empilha transformação + recorte correntes.
    fn save(&mut self);
    /// Restaura o estado empilhado pelo último `save` (no-op se a pilha estiver vazia).
    fn restore(&mut self);
    /// Compõe `t` com a CTM: desenhos seguintes passam por `t` e depois pela CTM anterior.
    fn transform(&mut self, t: Transform);
    /// Intersecta o recorte corrente com `rect` (em coordenadas do usuário, sob a CTM atual).
    fn clip_rect(&mut self, rect: Rect);

    /// Retângulo com raio/preenchimento/borda (GPUI: `paint_quad`).
    fn quad(&mut self, quad: &Quad);
    /// Preenche um caminho (regra non-zero).
    fn fill_path(&mut self, path: &Path, color: Color);
    /// Contorna um caminho.
    fn stroke_path(&mut self, path: &Path, stroke: &Stroke, color: Color);
    /// Uma linha de texto. `origin` é o ponto na **linha de base**; o `align` do estilo diz se
    /// ele é o início, o centro ou o fim da linha.
    fn text(&mut self, text: &str, origin: Point, style: &TextStyle, color: Color);
    /// Um glifo de fonte por codepoint (ex.: SMuFL `U+E0A4`), com a origem do glifo
    /// (linha de base, x = 0 do glifo) em `origin` e o em com `size` px.
    fn glyph(&mut self, font: FontId, codepoint: char, origin: Point, size: f32, color: Color);

    // ---- métodos derivados ----

    fn translate(&mut self, dx: f32, dy: f32) {
        self.transform(Transform::translate(dx, dy));
    }
    fn scale(&mut self, sx: f32, sy: f32) {
        self.transform(Transform::scale(sx, sy));
    }
    fn rotate(&mut self, rad: f32) {
        self.transform(Transform::rotate(rad));
    }
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.quad(&Quad::fill(rect, color));
    }
    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        self.quad(&Quad::fill(rect, color).radius(radius));
    }
    /// Contorno de retângulo com a borda para dentro.
    fn stroke_rect(&mut self, rect: Rect, width: f32, color: Color) {
        self.quad(&Quad::outline(rect, width, color));
    }
    fn line(&mut self, from: Point, to: Point, stroke: &Stroke, color: Color) {
        self.stroke_path(&Path::line(from, to), stroke, color);
    }
    fn fill_circle(&mut self, center: Point, radius: f32, color: Color) {
        self.fill_path(&Path::circle(center, radius), color);
    }
    fn stroke_circle(&mut self, center: Point, radius: f32, stroke: &Stroke, color: Color) {
        self.stroke_path(&Path::circle(center, radius), stroke, color);
    }
}

#[derive(Clone, Copy, Debug)]
struct State {
    transform: Transform,
    clip: Option<Rect>,
}

/// Pilha de estado (CTM + recorte) pronta para backends que precisam resolver tudo na CPU.
///
/// Pensada para o backend GPUI: mantenha um `StateStack`, mapeie pontos com
/// [`map_point`](StateStack::map_point)/[`map_rect`](StateStack::map_rect) antes de chamar
/// `paint_quad`/`paint_path`, e use [`clip_bounds`](StateStack::clip_bounds) como
/// `ContentMask` (o recorte é guardado como caixa envolvente em coordenadas de dispositivo).
#[derive(Clone, Debug)]
pub struct StateStack {
    cur: State,
    stack: Vec<State>,
}

impl Default for StateStack {
    fn default() -> Self {
        Self::new()
    }
}

impl StateStack {
    pub fn new() -> Self {
        Self::with_base(Transform::IDENTITY)
    }
    /// Começa com uma transformação base (ex.: origem do elemento na janela).
    pub fn with_base(base: Transform) -> Self {
        StateStack { cur: State { transform: base, clip: None }, stack: Vec::new() }
    }
    pub fn save(&mut self) {
        self.stack.push(self.cur);
    }
    /// `false` se não havia nada para restaurar.
    pub fn restore(&mut self) -> bool {
        match self.stack.pop() {
            Some(s) => {
                self.cur = s;
                true
            }
            None => false,
        }
    }
    pub fn concat(&mut self, t: Transform) {
        self.cur.transform = self.cur.transform.pre_concat(&t);
    }
    /// Intersecta o recorte com `rect` (coordenadas do usuário).
    pub fn clip_rect(&mut self, rect: Rect) {
        let dev = self.cur.transform.apply_rect(&rect);
        self.cur.clip = Some(match self.cur.clip {
            Some(c) => c.intersect(&dev),
            None => dev,
        });
    }
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
    /// CTM corrente (usuário → dispositivo).
    pub fn transform(&self) -> Transform {
        self.cur.transform
    }
    /// Recorte corrente em coordenadas de dispositivo (`None` = sem recorte).
    pub fn clip_bounds(&self) -> Option<Rect> {
        self.cur.clip
    }
    pub fn map_point(&self, p: Point) -> Point {
        self.cur.transform.apply(p)
    }
    /// Caixa envolvente do retângulo em coordenadas de dispositivo.
    pub fn map_rect(&self, r: &Rect) -> Rect {
        self.cur.transform.apply_rect(r)
    }
    /// Converte um comprimento (espessura, raio, tamanho de fonte) pelo fator de escala médio.
    pub fn map_len(&self, len: f32) -> f32 {
        len * self.cur.transform.mean_scale()
    }
    /// `true` se o retângulo (do usuário) está inteiramente fora do recorte — para culling.
    pub fn is_clipped_out(&self, r: &Rect) -> bool {
        match self.cur.clip {
            Some(c) => !c.intersects(&self.map_rect(r)),
            None => false,
        }
    }
}
