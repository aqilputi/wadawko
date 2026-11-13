use gpui_kit::{Hsla, rgb};

/// Tokens de src/app.css.
#[derive(Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    pub bg: Hsla,
    pub paper: Hsla,
    pub paper_raise: Hsla,
    pub paper_hover: Hsla,
    pub cell_empty: Hsla,
    pub ink: Hsla,
    pub ink_soft: Hsla,
    pub ink_faint: Hsla,
    pub line: Hsla,
    pub line_strong: Hsla,
    pub accent: Hsla,
    pub accent_wash: Hsla,
    pub accent2: Hsla,
    pub accent2_wash: Hsla,
    pub on_accent: Hsla,
}

impl Theme {
    pub fn light() -> Self {
        Theme {
            dark: false,
            bg: rgb(0xeceff5).into(),
            paper: rgb(0xf9fafc).into(),
            paper_raise: rgb(0xeef1f7).into(),
            paper_hover: rgb(0xe4e9f3).into(),
            cell_empty: rgb(0xe8ecf4).into(),
            ink: rgb(0x232733).into(),
            ink_soft: rgb(0x5c6577).into(),
            ink_faint: rgb(0x97a0b4).into(),
            line: rgb(0xdde2ec).into(),
            line_strong: rgb(0xc6cede).into(),
            accent: rgb(0xe26374).into(),
            accent_wash: rgb(0xfbe7ea).into(),
            accent2: rgb(0x6f93cf).into(),
            accent2_wash: rgb(0xe7eefb).into(),
            on_accent: rgb(0xffffff).into(),
        }
    }

    pub fn dark() -> Self {
        Theme {
            dark: true,
            bg: rgb(0x12141b).into(),
            paper: rgb(0x1d212c).into(),
            paper_raise: rgb(0x262b39).into(),
            paper_hover: rgb(0x2f3545).into(),
            cell_empty: rgb(0x222735).into(),
            ink: rgb(0xe8ebf2).into(),
            ink_soft: rgb(0x9aa4ba).into(),
            ink_faint: rgb(0x6b7590).into(),
            line: rgb(0x2c3242).into(),
            line_strong: rgb(0x3a4256).into(),
            accent: rgb(0xec7c8b).into(),
            accent_wash: rgb(0x3a2830).into(),
            accent2: rgb(0x86a9e0).into(),
            accent2_wash: rgb(0x22304a).into(),
            on_accent: rgb(0xffffff).into(),
        }
    }

    pub fn alpha(c: Hsla, a: f32) -> Hsla {
        Hsla { a, ..c }
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
}
