//! Export da partitura: SVG (contínuo ou paginado A4 paisagem) e PNG (tiny-skia).

use wadawko_core::Song;
use wadawko_draw::{Canvas2D, Rect, Result, SkiaCanvas, StandardFonts, SvgCanvas, SvgOptions};

use crate::engrave::engrave;
use crate::options::{EngraveOptions, LayoutMode, PageSpec};
use crate::score::{Lit, Score};

impl Score {
    /// SVG de uma página. `text_as_paths = true` converte os textos em contornos (arquivo
    /// maior, mas independente das fontes instaladas — o certo para imprimir).
    pub fn page_svg(&self, page: usize, fonts: &StandardFonts, text_as_paths: bool) -> String {
        let (w, h) = self.page_size(page);
        let opts = SvgOptions { text_as_paths, glyphs_as_paths: true, background: self.theme.paper };
        let mut canvas = SvgCanvas::with_options(w, h, Some(&fonts.book), opts);
        self.paint_page(&mut canvas, page, None);
        canvas.finish()
    }

    /// PNG de uma página inteira; `scale` = pixels físicos por px lógico.
    pub fn page_png(&self, page: usize, fonts: &StandardFonts, scale: f32) -> Result<Vec<u8>> {
        let (w, h) = self.page_size(page);
        self.region_png(page, Rect::new(0.0, 0.0, w, h), fonts, scale, &[])
    }

    /// PNG de um recorte da página (o resto é descartado pelo culling), com golpes acesos
    /// opcionais — útil para miniaturas e para conferir o highlight.
    pub fn region_png(&self, page: usize, region: Rect, fonts: &StandardFonts, scale: f32, lit: &[Lit]) -> Result<Vec<u8>> {
        let mut canvas = SkiaCanvas::with_scale(region.w.ceil().max(1.0) as u32, region.h.ceil().max(1.0) as u32, scale, &fonts.book)?;
        if let Some(bg) = self.theme.paper {
            canvas.clear(bg);
        }
        canvas.save();
        canvas.translate(-region.x, -region.y);
        self.paint_lit(&mut canvas, page, Some(region), lit);
        canvas.restore();
        canvas.encode_png()
    }
}

/// SVG de página única contínua (um sistema só, largura = música inteira).
pub fn to_svg(song: &Song, opts: &EngraveOptions, fonts: &StandardFonts) -> String {
    let opts = EngraveOptions { mode: LayoutMode::Continuous, ..opts.clone() };
    engrave(song, &opts, fonts).page_svg(0, fonts, true)
}

/// SVG paginado, uma string por página. Se `opts.mode` não for paginado, usa A4 paisagem.
pub fn to_svg_pages(song: &Song, opts: &EngraveOptions, fonts: &StandardFonts) -> Vec<String> {
    let score = engrave(song, &paged(opts), fonts);
    (0..score.page_count()).map(|p| score.page_svg(p, fonts, true)).collect()
}

/// PNG da partitura contínua inteira. Cuidado com músicas longas: a imagem tem a largura
/// da música (use [`Score::region_png`] para recortes ou [`to_png_pages`]).
pub fn to_png(song: &Song, opts: &EngraveOptions, fonts: &StandardFonts, scale: f32) -> Result<Vec<u8>> {
    let opts = EngraveOptions { mode: LayoutMode::Continuous, ..opts.clone() };
    engrave(song, &opts, fonts).page_png(0, fonts, scale)
}

/// PNG paginado (A4 paisagem se `opts.mode` não for paginado), um por página.
pub fn to_png_pages(song: &Song, opts: &EngraveOptions, fonts: &StandardFonts, scale: f32) -> Result<Vec<Vec<u8>>> {
    let score = engrave(song, &paged(opts), fonts);
    (0..score.page_count()).map(|p| score.page_png(p, fonts, scale)).collect()
}

fn paged(opts: &EngraveOptions) -> EngraveOptions {
    match opts.mode {
        LayoutMode::Paged(_) => opts.clone(),
        LayoutMode::Continuous => EngraveOptions { mode: LayoutMode::Paged(PageSpec::a4_landscape()), ..opts.clone() },
    }
}
