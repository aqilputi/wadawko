//! Backend que apenas grava os comandos — para testes e para cachear uma cena
//! (ex.: engravar a partitura uma vez e reproduzir a cada frame, trocando só cores).

use crate::canvas::Canvas2D;
use crate::font::FontId;
use crate::geom::{Color, Point, Rect, Transform};
use crate::path::Path;
use crate::style::{Quad, Stroke, TextStyle};

/// Um comando de desenho gravado (espelha os métodos de [`Canvas2D`]).
#[derive(Clone, Debug, PartialEq)]
pub enum DrawCmd {
    Save,
    Restore,
    Transform(Transform),
    ClipRect(Rect),
    Quad(Quad),
    Line { from: Point, to: Point, stroke: Stroke, color: Color },
    FillPath { path: Path, color: Color },
    StrokePath { path: Path, stroke: Stroke, color: Color },
    Text { text: String, origin: Point, style: TextStyle, color: Color },
    Glyph { font: FontId, codepoint: char, origin: Point, size: f32, color: Color },
}

impl DrawCmd {
    /// Troca a cor principal do comando (preenchimento do quad, cor do traço/texto/glifo).
    /// Devolve `false` para comandos sem cor. Serve para o "flash" de notas sem re-engravar.
    pub fn set_color(&mut self, c: Color) -> bool {
        match self {
            DrawCmd::Quad(q) => {
                if q.fill.is_some() {
                    q.fill = Some(c);
                } else {
                    q.border_color = Some(c);
                }
                true
            }
            DrawCmd::Line { color, .. }
            | DrawCmd::FillPath { color, .. }
            | DrawCmd::StrokePath { color, .. }
            | DrawCmd::Text { color, .. }
            | DrawCmd::Glyph { color, .. } => {
                *color = c;
                true
            }
            _ => false,
        }
    }

    /// Reexecuta o comando em outro canvas.
    pub fn apply(&self, canvas: &mut dyn Canvas2D) {
        match self {
            DrawCmd::Save => canvas.save(),
            DrawCmd::Restore => canvas.restore(),
            DrawCmd::Transform(t) => canvas.transform(*t),
            DrawCmd::ClipRect(r) => canvas.clip_rect(*r),
            DrawCmd::Quad(q) => canvas.quad(q),
            DrawCmd::Line { from, to, stroke, color } => canvas.line(*from, *to, stroke, *color),
            DrawCmd::FillPath { path, color } => canvas.fill_path(path, *color),
            DrawCmd::StrokePath { path, stroke, color } => canvas.stroke_path(path, stroke, *color),
            DrawCmd::Text { text, origin, style, color } => canvas.text(text, *origin, style, *color),
            DrawCmd::Glyph { font, codepoint, origin, size, color } => {
                canvas.glyph(*font, *codepoint, *origin, *size, *color)
            }
        }
    }
}

/// Lista de comandos gravados.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recording {
    pub cmds: Vec<DrawCmd>,
}

impl Recording {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.cmds.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }
    pub fn clear(&mut self) {
        self.cmds.clear();
    }
    /// Reproduz tudo em outro canvas (envolto em save/restore para não vazar estado).
    pub fn replay(&self, canvas: &mut dyn Canvas2D) {
        canvas.save();
        let mut depth = 0i32;
        for cmd in &self.cmds {
            match cmd {
                DrawCmd::Save => depth += 1,
                DrawCmd::Restore => {
                    if depth == 0 {
                        continue; // restore sem save correspondente: ignora
                    }
                    depth -= 1;
                }
                _ => {}
            }
            cmd.apply(canvas);
        }
        for _ in 0..depth {
            canvas.restore();
        }
        canvas.restore();
    }
}

impl Canvas2D for Recording {
    fn save(&mut self) {
        self.cmds.push(DrawCmd::Save);
    }
    fn restore(&mut self) {
        self.cmds.push(DrawCmd::Restore);
    }
    fn transform(&mut self, t: Transform) {
        self.cmds.push(DrawCmd::Transform(t));
    }
    fn clip_rect(&mut self, rect: Rect) {
        self.cmds.push(DrawCmd::ClipRect(rect));
    }
    fn quad(&mut self, quad: &Quad) {
        self.cmds.push(DrawCmd::Quad(*quad));
    }
    fn fill_path(&mut self, path: &Path, color: Color) {
        self.cmds.push(DrawCmd::FillPath { path: path.clone(), color });
    }
    fn stroke_path(&mut self, path: &Path, stroke: &Stroke, color: Color) {
        self.cmds.push(DrawCmd::StrokePath { path: path.clone(), stroke: *stroke, color });
    }
    fn text(&mut self, text: &str, origin: Point, style: &TextStyle, color: Color) {
        self.cmds.push(DrawCmd::Text { text: text.to_owned(), origin, style: style.clone(), color });
    }
    fn glyph(&mut self, font: FontId, codepoint: char, origin: Point, size: f32, color: Color) {
        self.cmds.push(DrawCmd::Glyph { font, codepoint, origin, size, color });
    }
    fn line(&mut self, from: Point, to: Point, stroke: &Stroke, color: Color) {
        self.cmds.push(DrawCmd::Line { from, to, stroke: *stroke, color });
    }
}
