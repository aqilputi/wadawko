//! Export da partitura: SVG contínuo, PNG paginado (A4 paisagem) e uma página
//! HTML de impressão com as páginas A4 em SVG (o navegador imprime/gera o PDF).
//! Tudo puro (música → bytes) para rodar fora da thread da UI.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use wadawko_core::Song;
use wadawko_draw::StandardFonts;
use wadawko_notation::{EngraveOptions, Theme, to_png_pages, to_svg, to_svg_pages};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportKind {
    /// um SVG só, contínuo (a música inteira numa linha)
    Svg,
    /// um PNG por página A4 paisagem (2× = ~190 dpi)
    Png,
    /// HTML com as páginas A4 paisagem em SVG, pronto para Ctrl+P → impressora/PDF
    Print,
}

impl ExportKind {
    pub fn ext(self) -> &'static str {
        match self {
            ExportKind::Svg => "svg",
            ExportKind::Png => "png",
            ExportKind::Print => "html",
        }
    }
}

/// Escala dos PNGs (px físicos por px lógico de 96 dpi).
const PNG_SCALE: f32 = 2.0;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Página de impressão: cada SVG ocupa uma folha A4 paisagem.
pub fn print_html(title: &str, pages: &[String]) -> String {
    let mut out = String::with_capacity(pages.iter().map(String::len).sum::<usize>() + 1024);
    out.push_str("<!doctype html>\n<html lang=\"pt-BR\">\n<head>\n<meta charset=\"utf-8\">\n<title>");
    out.push_str(&esc(title));
    out.push_str(
        "</title>\n<style>\n@page { size: A4 landscape; margin: 0; }\nhtml, body { margin: 0; padding: 0; background: #8a8f9c; }\n\
         svg { display: block; width: 297mm; height: 210mm; margin: 8mm auto; background: #fff; box-shadow: 0 2px 12px rgba(0,0,0,.35); break-after: page; page-break-after: always; }\n\
         svg:last-of-type { break-after: auto; page-break-after: auto; }\n\
         @media print { html, body { background: none; } svg { margin: 0; box-shadow: none; } }\n</style>\n</head>\n<body>\n",
    );
    for svg in pages {
        // só o elemento <svg> (sem declaração XML, se houver)
        out.push_str(svg.find("<svg").map_or(svg.as_str(), |i| &svg[i..]));
    }
    out.push_str("</body>\n</html>\n");
    out
}

/// Arquivos do export: (nome relativo ao caminho escolhido, bytes). `None` = o próprio caminho;
/// `Some(n)` = página `n` (1…) de um export com várias páginas.
pub fn render(song: &Song, kind: ExportKind, fonts: &StandardFonts) -> Result<Vec<(Option<usize>, Vec<u8>)>> {
    // sempre no tema claro: é papel
    Ok(match kind {
        ExportKind::Svg => vec![(None, to_svg(song, &EngraveOptions::screen(Theme::light()), fonts).into_bytes())],
        ExportKind::Png => {
            let pages = to_png_pages(song, &EngraveOptions::a4_landscape(), fonts, PNG_SCALE).map_err(|e| anyhow::anyhow!("{e}"))?;
            if pages.len() == 1 { pages.into_iter().map(|p| (None, p)).collect() } else { pages.into_iter().enumerate().map(|(i, p)| (Some(i + 1), p)).collect() }
        }
        ExportKind::Print => {
            let pages = to_svg_pages(song, &EngraveOptions::a4_landscape(), fonts);
            vec![(None, print_html(&format!("{} — partitura", song.title), &pages).into_bytes())]
        }
    })
}

/// `musica.png` + página 2 → `musica-02.png`.
pub fn page_path(path: &Path, page: Option<usize>) -> PathBuf {
    let Some(n) = page else { return path.to_path_buf() };
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let name = match path.extension() {
        Some(ext) => format!("{stem}-{n:02}.{}", ext.to_string_lossy()),
        None => format!("{stem}-{n:02}"),
    };
    path.with_file_name(name)
}

/// Gera e grava; devolve os arquivos escritos.
pub fn write(song: &Song, kind: ExportKind, fonts: &StandardFonts, path: &Path) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for (page, bytes) in render(song, kind, fonts)? {
        let p = page_path(path, page);
        std::fs::write(&p, bytes).with_context(|| format!("gravar {}", p.display()))?;
        written.push(p);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fonts() -> &'static StandardFonts {
        crate::views::score::fonts()
    }

    #[test]
    fn page_paths() {
        assert_eq!(page_path(Path::new("/a/b.png"), None), PathBuf::from("/a/b.png"));
        assert_eq!(page_path(Path::new("/a/b.png"), Some(2)), PathBuf::from("/a/b-02.png"));
        assert_eq!(page_path(Path::new("b"), Some(11)), PathBuf::from("b-11"));
    }

    #[test]
    fn svg_is_one_continuous_file_with_outlined_text() {
        let song = wadawko_core::demo::demo_song();
        let files = render(&song, ExportKind::Svg, fonts()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, None);
        let svg = String::from_utf8(files[0].1.clone()).unwrap();
        assert!(svg.starts_with("<svg") && svg.trim_end().ends_with("</svg>"));
        assert!(svg.contains("<path"));
    }

    #[test]
    fn png_pages_are_a4_landscape_at_2x() {
        let song = wadawko_core::demo::demo_song();
        let files = render(&song, ExportKind::Png, fonts()).unwrap();
        assert!(!files.is_empty());
        for (i, (page, bytes)) in files.iter().enumerate() {
            assert_eq!(*page, if files.len() == 1 { None } else { Some(i + 1) });
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
            // IHDR: largura e altura em big-endian
            let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
            let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
            assert_eq!((w, h), (2246, 1588), "A4 paisagem (1122,52 × 793,70 px) × 2");
        }
    }

    #[test]
    fn print_html_has_one_svg_per_page_and_escapes_the_title() {
        let html = print_html("A <b> & C", &["<?xml version=\"1.0\"?>\n<svg a=\"1\"></svg>\n".into(), "<svg b=\"2\"></svg>\n".into()]);
        assert!(html.contains("<title>A &lt;b&gt; &amp; C</title>"));
        assert_eq!(html.matches("<svg").count(), 2);
        assert!(!html.contains("<?xml"));
        assert!(html.contains("size: A4 landscape"));
        let song = wadawko_core::demo::demo_song();
        let files = render(&song, ExportKind::Print, fonts()).unwrap();
        let html = String::from_utf8(files[0].1.clone()).unwrap();
        assert!(html.matches("<svg").count() >= 1 && html.ends_with("</html>\n"));
    }

    #[test]
    fn write_puts_files_on_disk() {
        let dir = std::env::temp_dir().join(format!("wadawko-score-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let song = wadawko_core::demo::demo_song();
        let out = write(&song, ExportKind::Svg, fonts(), &dir.join("x.svg")).unwrap();
        assert_eq!(out, vec![dir.join("x.svg")]);
        assert!(std::fs::metadata(&out[0]).unwrap().len() > 1000);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
