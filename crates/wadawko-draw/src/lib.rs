//! # wadawko-draw
//!
//! Camada de desenho 2D do WaDAWko, independente de backend (docs/planejamento/gpui.md §3.1, §5).
//!
//! - [`Canvas2D`]: o trait contra o qual a notação (`wadawko-notation`), a esteira e os
//!   bonecos pintam. Quads, linhas, caminhos, texto, glifos por codepoint, recorte e
//!   transformação afim.
//! - [`FontBook`]: fontes em memória com medição de texto/glifos e contornos — sem backend,
//!   para o layout da partitura saber larguras e âncoras antes de pintar.
//! - [`smufl`]: metadados SMuFL tipados (âncoras `stemUpSE` etc.) e codepoints da Bravura.
//! - Backends: [`SvgCanvas`] (string SVG), [`SkiaCanvas`] (raster/PNG via tiny-skia) e
//!   [`Recording`] (lista de comandos, reproduzível em qualquer outro canvas).
//!   O backend GPUI mora no app (este crate não depende de GPUI); veja [`StateStack`].
//!
//! Convenções: px lógicos `f32`, y para baixo; texto ancorado na linha de base; preenchimento
//! non-zero; borda de [`Quad`] para dentro.

mod canvas;
mod error;
mod font;
mod geom;
mod path;
mod recording;
mod skia;
pub mod smufl;
mod style;
mod svg;

pub use canvas::{Canvas2D, StateStack};
pub use error::{DrawError, Result};
pub use font::{FontBook, FontId, FontInfo, FontMetrics, GlyphMetrics, PlacedGlyph, SYNTHETIC_ITALIC_SKEW, TextLayout, TextMetrics};
pub use geom::{Color, Point, Rect, Transform, pt, rect};
pub use path::{Path, PathBuilder, PathEl};
pub use recording::{DrawCmd, Recording};
pub use skia::SkiaCanvas;
pub use smufl::{SmuflFont, SmuflGlyph, SmuflMetadata, StandardFonts};
pub use style::{FontWeight, LineCap, LineJoin, Quad, Stroke, TextAlign, TextStyle};
pub use svg::{SvgCanvas, SvgOptions};

/// Reexport do `tiny_skia` (para quem precisa do `Pixmap` de [`SkiaCanvas::pixmap`]).
pub use tiny_skia;
