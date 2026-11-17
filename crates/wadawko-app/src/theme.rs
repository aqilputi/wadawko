//! Tokens de cor do app — sistema de design "estilo DAW" (docs/planejamento/daw.md §2.2).
//!
//! Cinzas neutros em níveis, na ordem moldura (mais escura) → painéis → área de
//! trabalho (mais clara); UMA cor quente de "ligado" (`accent`), azul para
//! seleção/borda/display (`accent2`); tudo o mais é cinza.

use gpui_kit::{Hsla, rgb};

#[derive(Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    /// moldura: barra de cima, nav rail, painel direito, abas/contexto, status bar
    pub chrome: Hsla,
    /// faixa de cabeçalho de painel (MÚSICAS, INSPECTOR, grupo do mixer)
    pub header: Hsla,
    /// "tela" do transporte (posição, BPM): campo escuro com texto `accent2`
    pub display: Hsla,
    /// área de trabalho: fundo da grade, páginas, esteira
    pub bg: Hsla,
    /// cartões, popovers, cabeçalho de seção, strips do mixer, paleta
    pub paper: Hsla,
    /// controles em repouso: botão, chip, campo, segmento inativo
    pub paper_raise: Hsla,
    /// hover / pressionado
    pub paper_hover: Hsla,
    /// célula vazia da grade
    pub cell_empty: Hsla,
    /// célula com nota
    pub cell_filled: Hsla,
    /// texto principal (não é branco puro)
    pub ink: Hsla,
    /// texto secundário, rótulos, ícones
    pub ink_soft: Hsla,
    /// legendas, desabilitado, contagem "e & a"
    pub ink_faint: Hsla,
    /// hairlines: bordas de painel, divisores
    pub line: Hsla,
    /// borda de controle, trilho de slider
    pub line_strong: Hsla,
    /// linha de início de tempo na grade — ESCURECE a célula
    pub line_beat: Hsla,
    /// barra de compasso na grade (2 px)
    pub line_bar: Hsla,
    /// ação principal e estado "ligado": play, loop, cursor, nota de couro, kiai
    pub accent: Hsla,
    pub accent_wash: Hsla,
    /// seleção, nota de borda (fuchi), solo, faixa de loop, texto do display
    pub accent2: Hsla,
    pub accent2_wash: Hsla,
    /// texto sobre `accent`/`accent2`/cor de trilha preenchidos
    pub on_accent: Hsla,
    /// linha do playhead
    pub playhead: Hsla,
    /// mudo ligado, aviso, medidor meio
    pub warn: Hsla,
    /// clip, erro de gravação, excluir
    pub danger: Hsla,
    /// medidor até −18 dB
    pub meter_lo: Hsla,
    /// medidor −18..−6 dB
    pub meter_mid: Hsla,
    /// medidor −6..0 dB
    pub meter_hi: Hsla,
}

impl Theme {
    pub fn light() -> Self {
        // o plano pedia #b8860b, mas o "M" branco sobre ele dava 3,25: mais escuro (4,95)
        let warn: Hsla = rgb(0x946808).into();
        // o plano pedia #d9503c, mas com texto branco dá 4,07 (< 4,5): um tom mais escuro
        let accent: Hsla = rgb(0xc94531).into();
        Theme {
            dark: false,
            chrome: rgb(0xf6f6f6).into(),
            header: rgb(0xe9e9e9).into(),
            display: rgb(0xe3e3e3).into(),
            bg: rgb(0xececec).into(),
            paper: rgb(0xffffff).into(),
            paper_raise: rgb(0xefefef).into(),
            paper_hover: rgb(0xe2e2e2).into(),
            cell_empty: rgb(0xe6e6e6).into(),
            cell_filled: rgb(0xffffff).into(),
            ink: rgb(0x1f1f1f).into(),
            ink_soft: rgb(0x5a5a5a).into(),
            ink_faint: rgb(0x9a9a9a).into(),
            line: rgb(0xd9d9d9).into(),
            line_strong: rgb(0xbdbdbd).into(),
            line_beat: rgb(0xd2d2d2).into(),
            line_bar: rgb(0x9a9a9a).into(),
            accent,
            accent_wash: rgb(0xfbe3de).into(),
            accent2: rgb(0x2f6fb0).into(),
            accent2_wash: rgb(0xdfeafa).into(),
            on_accent: rgb(0xffffff).into(),
            playhead: rgb(0x1f1f1f).into(),
            warn,
            danger: rgb(0xd32f2f).into(),
            meter_lo: rgb(0x2e9e52).into(),
            meter_mid: warn,
            meter_hi: accent,
        }
    }

    /// Tema padrão (bancos sem tema salvo abrem escuros).
    pub fn dark() -> Self {
        let warn: Hsla = rgb(0xe2b53e).into();
        let accent: Hsla = rgb(0xee6b5a).into();
        Theme {
            dark: true,
            chrome: rgb(0x1c1c1c).into(),
            header: rgb(0x161616).into(),
            display: rgb(0x0f0f0f).into(),
            bg: rgb(0x262626).into(),
            paper: rgb(0x303030).into(),
            paper_raise: rgb(0x3b3b3b).into(),
            paper_hover: rgb(0x474747).into(),
            cell_empty: rgb(0x2b2b2b).into(),
            cell_filled: rgb(0x363636).into(),
            ink: rgb(0xe6e6e6).into(),
            ink_soft: rgb(0xa6a6a6).into(),
            ink_faint: rgb(0x707070).into(),
            line: rgb(0x3a3a3a).into(),
            line_strong: rgb(0x525252).into(),
            line_beat: rgb(0x1a1a1a).into(),
            line_bar: rgb(0x0f0f0f).into(),
            accent,
            accent_wash: rgb(0x4a2c27).into(),
            accent2: rgb(0x5a9be0).into(),
            accent2_wash: rgb(0x253a52).into(),
            // texto ESCURO sobre o botão aceso (lâmpada), como nas DAWs
            on_accent: rgb(0x141414).into(),
            playhead: rgb(0xf4f4f4).into(),
            warn,
            danger: rgb(0xff4a4a).into(),
            meter_lo: rgb(0x7fc53a).into(),
            meter_mid: warn,
            meter_hi: accent,
        }
    }

    pub fn alpha(c: Hsla, a: f32) -> Hsla {
        Hsla { a, ..c }
    }

    /// Todos os tokens, com nome (para os testes).
    #[cfg(test)]
    pub fn tokens(&self) -> Vec<(&'static str, Hsla)> {
        vec![
            ("chrome", self.chrome),
            ("header", self.header),
            ("display", self.display),
            ("bg", self.bg),
            ("paper", self.paper),
            ("paper_raise", self.paper_raise),
            ("paper_hover", self.paper_hover),
            ("cell_empty", self.cell_empty),
            ("cell_filled", self.cell_filled),
            ("ink", self.ink),
            ("ink_soft", self.ink_soft),
            ("ink_faint", self.ink_faint),
            ("line", self.line),
            ("line_strong", self.line_strong),
            ("line_beat", self.line_beat),
            ("line_bar", self.line_bar),
            ("accent", self.accent),
            ("accent_wash", self.accent_wash),
            ("accent2", self.accent2),
            ("accent2_wash", self.accent2_wash),
            ("on_accent", self.on_accent),
            ("playhead", self.playhead),
            ("warn", self.warn),
            ("danger", self.danger),
            ("meter_lo", self.meter_lo),
            ("meter_mid", self.meter_mid),
            ("meter_hi", self.meter_hi),
        ]
    }
}

/// `#rrggbb` (cor de pessoa do JSON) → Hsla.
pub fn parse_color(s: &str) -> Option<Hsla> {
    let h = s.trim().strip_prefix('#')?;
    let v = match h.len() {
        6 => u32::from_str_radix(h, 16).ok()?,
        3 => {
            let v = u32::from_str_radix(h, 16).ok()?;
            let (r, g, b) = ((v >> 8) & 0xf, (v >> 4) & 0xf, v & 0xf);
            (r * 17) << 16 | (g * 17) << 8 | (b * 17)
        }
        _ => return None,
    };
    Some(rgb(v).into())
}

/// Luminância relativa (WCAG 2.x) de uma cor opaca.
#[cfg(test)]
pub fn luminance(c: Hsla) -> f64 {
    let rgba = c.to_rgb();
    let lin = |v: f32| {
        let v = v as f64;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(rgba.r) + 0.7152 * lin(rgba.g) + 0.0722 * lin(rgba.b)
}

/// Razão de contraste WCAG entre duas cores (≥ 1).
#[cfg(test)]
pub fn contrast(a: Hsla, b: Hsla) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_person_colors() {
        assert!(parse_color("#e26374").is_some());
        assert!(parse_color("#fff").is_some());
        assert!(parse_color("vermelho").is_none());
        for c in wadawko_core::model::PERSON_COLORS {
            assert!(parse_color(c).is_some(), "{c}");
        }
    }

    #[test]
    fn luminance_and_contrast_match_wcag() {
        let black: Hsla = rgb(0x000000).into();
        let white: Hsla = rgb(0xffffff).into();
        assert!((luminance(black) - 0.0).abs() < 1e-6);
        assert!((luminance(white) - 1.0).abs() < 1e-6);
        assert!((contrast(black, white) - 21.0).abs() < 1e-3);
        assert!((contrast(white, black) - 21.0).abs() < 1e-3);
    }

    /// Contrastes mínimos do plano (daw.md §2.2) e nenhum token translúcido.
    #[test]
    fn themes_meet_contrast_and_are_opaque() {
        for th in [Theme::dark(), Theme::light()] {
            let name = if th.dark { "escuro" } else { "claro" };
            for (tok, c) in th.tokens() {
                assert!((c.a - 1.0).abs() < 1e-6, "{name}: {tok} tem alfa {}", c.a);
            }
            let ink_bg = contrast(th.ink, th.bg);
            let soft_paper = contrast(th.ink_soft, th.paper);
            let on_accent = contrast(th.on_accent, th.accent);
            let on_accent2 = contrast(th.on_accent, th.accent2);
            assert!(ink_bg >= 7.0, "{name}: ink/bg = {ink_bg:.2}");
            assert!(soft_paper >= 4.5, "{name}: ink_soft/paper = {soft_paper:.2}");
            assert!(on_accent >= 4.5, "{name}: on_accent/accent = {on_accent:.2}");
            assert!(on_accent2 >= 4.5, "{name}: on_accent/accent2 = {on_accent2:.2}");
            let on_warn = contrast(th.on_accent, th.warn);
            assert!(on_warn >= 4.5, "{name}: on_accent/warn = {on_warn:.2}");
            // a moldura é mais escura que a área de trabalho no tema escuro (e mais clara no claro)
            if th.dark {
                assert!(luminance(th.chrome) < luminance(th.bg) && luminance(th.bg) < luminance(th.paper));
            } else {
                assert!(luminance(th.chrome) > luminance(th.bg));
            }
        }
    }
}
