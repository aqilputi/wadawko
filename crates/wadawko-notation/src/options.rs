//! Opções do engraving: escala, densidade horizontal, modo de layout e tema.

use wadawko_draw::Color;

/// Cores da partitura — passadas de fora (a view GPUI monta a partir do tema do app).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// notas, hastes, beams, barras, sílabas
    pub ink: Color,
    /// textos secundários: nº de compasso, nomes de seção, rodapé
    pub ink_soft: Color,
    /// linha da pauta
    pub staff: Color,
    /// kiai (itálico vermelho)
    pub kyai: Color,
    /// cor padrão do golpe aceso (highlight)
    pub accent: Color,
    /// fundo usado só nos exports (`None` = transparente); `paint` nunca pinta fundo
    pub paper: Option<Color>,
}

impl Theme {
    pub const fn light() -> Self {
        Theme { ink: Color::rgb(0x1c1b1a), ink_soft: Color::rgb(0x6b6660), staff: Color::rgb(0x1c1b1a), kyai: Color::rgb(0xd6452b), accent: Color::rgb(0xe8590c), paper: Some(Color::WHITE) }
    }
    pub const fn dark() -> Self {
        Theme { ink: Color::rgb(0xeae6df), ink_soft: Color::rgb(0xa39d94), staff: Color::rgb(0xcfc9c0), kyai: Color::rgb(0xff7a5c), accent: Color::rgb(0xffa94d), paper: Some(Color::rgb(0x1b1a19)) }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::light()
    }
}

/// Página para o modo paginado, em px lógicos (96 dpi: 1 px = 1/96 in).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageSpec {
    pub width: f32,
    pub height: f32,
    pub margin: f32,
}

impl PageSpec {
    /// A4 paisagem (297 × 210 mm) a 96 dpi, margem de 12 mm.
    pub const fn a4_landscape() -> Self {
        PageSpec { width: 1122.52, height: 793.70, margin: 45.35 }
    }
    /// A4 retrato.
    pub const fn a4_portrait() -> Self {
        PageSpec { width: 793.70, height: 1122.52, margin: 45.35 }
    }
}

/// Como os compassos são distribuídos.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutMode {
    /// Um sistema só, rolagem horizontal contínua (tela).
    Continuous,
    /// Quebra em sistemas justificados na largura da página e pagina (impressão).
    Paged(PageSpec),
}

/// Opções de [`crate::engrave`]. Todas as medidas em px lógicos **antes** do `zoom`.
#[derive(Clone, Debug, PartialEq)]
pub struct EngraveOptions {
    /// tamanho de um staff space (cabeça de nota ≈ 1 sp de altura). Padrão 10.
    pub staff_space: f32,
    /// largura de um tempo (semínima). Padrão 110 (igual ao web).
    pub beat_width: f32,
    /// largura mínima de uma célula. Padrão 36 (igual ao web).
    pub min_cell_width: f32,
    /// escala geral (multiplica tudo: glifos, textos, larguras).
    pub zoom: f32,
    pub mode: LayoutMode,
    pub theme: Theme,
    /// número do compasso acima de cada barra
    pub measure_numbers: bool,
    /// título/andamento no topo e rodapé "título — pág." (`None` = só no modo paginado)
    pub show_title: Option<bool>,
    /// coluna com os nomes das linhas (e trilhos de pessoa) à esquerda de cada sistema
    pub show_labels: bool,
}

impl Default for EngraveOptions {
    fn default() -> Self {
        EngraveOptions {
            staff_space: 10.0,
            beat_width: 110.0,
            min_cell_width: 36.0,
            zoom: 1.0,
            mode: LayoutMode::Continuous,
            theme: Theme::light(),
            measure_numbers: true,
            show_title: None,
            show_labels: true,
        }
    }
}

impl EngraveOptions {
    /// Tela: sistema único com rolagem horizontal.
    pub fn screen(theme: Theme) -> Self {
        EngraveOptions { theme, ..Default::default() }
    }
    /// Impressão: A4 paisagem, escala reduzida (2 sistemas de 4 linhas por página).
    pub fn a4_landscape() -> Self {
        EngraveOptions { zoom: 0.65, beat_width: 86.0, mode: LayoutMode::Paged(PageSpec::a4_landscape()), ..Default::default() }
    }
    pub fn with_zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
    pub fn with_mode(mut self, mode: LayoutMode) -> Self {
        self.mode = mode;
        self
    }
    pub(crate) fn zoom(&self) -> f32 {
        if self.zoom.is_finite() && self.zoom > 0.0 { self.zoom } else { 1.0 }
    }
}

/// `#rgb` / `#rrggbb` → cor (cores de pessoa do modelo).
pub fn parse_hex_color(s: &str) -> Option<Color> {
    let h = s.trim().strip_prefix('#')?;
    let v = u32::from_str_radix(h, 16).ok()?;
    match h.len() {
        6 => Some(Color::rgb(v)),
        3 => {
            let (r, g, b) = ((v >> 8) & 0xf, (v >> 4) & 0xf, v & 0xf);
            Some(Color::rgb((r * 17) << 16 | (g * 17) << 8 | (b * 17)))
        }
        _ => None,
    }
}
