//! # wadawko-notation
//!
//! Engraving da partitura de taiko do WaDAWko (PLANEJAMENTO-gpui.md §5): substituto do
//! VexFlow de `src/lib/score.ts`, escrito contra [`wadawko_draw::Canvas2D`] — o mesmo código
//! pinta na tela (GPUI), em SVG e em PNG.
//!
//! Convenção: pauta de 1 linha por tambor; cabeça acima da linha = mão direita, abaixo =
//! esquerda; cabeça × = borda (fuchi); `>` = acento; sílaba kuchi shōga embaixo; kiai em
//! itálico vermelho; cor do dono nas linhas compartilhadas.
//!
//! Duas etapas:
//!
//! 1. [`engrave`]`(&Song, &EngraveOptions, &StandardFonts) -> `[`Score`] — layout puro
//!    (páginas → sistemas → compassos → comandos de desenho) e o mapa
//!    [`HitKey`] → elementos gráficos do golpe.
//! 2. [`Score::paint`] / [`Score::paint_lit`] / [`Score::paint_highlights`] — pintura com
//!    culling por compasso visível; acender golpes só troca a cor dos comandos deles.
//!
//! ```no_run
//! use wadawko_draw::{StandardFonts, smufl::dev_fonts_dir, rect};
//! use wadawko_notation::{EngraveOptions, HitKey, engrave};
//!
//! let fonts = StandardFonts::load(dev_fonts_dir()).unwrap();
//! let song = wadawko_core::demo::demo_song();
//! let score = engrave(&song, &EngraveOptions::default(), &fonts);
//! let playhead = score.x_at(1, 0.5).unwrap(); // meio do 2º compasso
//! let scroll = score.scroll_x_for(playhead.x, 800.0);
//! let lit = [(HitKey::new("chu1", 0, 0), score.theme.accent)];
//! # let mut canvas = wadawko_draw::Recording::new();
//! score.paint_lit(&mut canvas, 0, Some(rect(scroll, 0.0, 800.0, 600.0)), &lit);
//! ```

mod engrave;
mod export;
mod options;
pub mod rhythm;
mod score;

pub use engrave::engrave;
pub use export::{to_png, to_png_pages, to_svg, to_svg_pages};
pub use options::{EngraveOptions, LayoutMode, PageSpec, Theme, parse_hex_color};
pub use score::{BeamInfo, CellRef, HitGfx, HitKey, Lit, MeasureLayout, PageLayout, PlayheadPos, Score, SystemLayout};
