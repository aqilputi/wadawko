//! Ícones do app (SVG próprios, traço de 2 px em 24×24, em `assets/icons/`),
//! embutidos no binário e servidos ao gpui como assets (`icons/<nome>.svg`).
//! O gpui rasteriza o SVG como máscara e pinta com a cor do texto do elemento,
//! então `ui::icon(nome).text_color(...)` colore o ícone.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

/// (nome, bytes do SVG)
pub const ICONS: &[(&str, &[u8])] = &[
    ("close", include_bytes!("../assets/icons/close.svg")),
    ("copy", include_bytes!("../assets/icons/copy.svg")),
    ("countin", include_bytes!("../assets/icons/countin.svg")),
    ("countin-loop", include_bytes!("../assets/icons/countin-loop.svg")),
    ("download", include_bytes!("../assets/icons/download.svg")),
    ("drum", include_bytes!("../assets/icons/drum.svg")),
    ("gear", include_bytes!("../assets/icons/gear.svg")),
    ("grid", include_bytes!("../assets/icons/grid.svg")),
    ("group", include_bytes!("../assets/icons/group.svg")),
    ("loop", include_bytes!("../assets/icons/loop.svg")),
    ("menu", include_bytes!("../assets/icons/menu.svg")),
    ("metronome", include_bytes!("../assets/icons/metronome.svg")),
    ("minus", include_bytes!("../assets/icons/minus.svg")),
    ("mixer", include_bytes!("../assets/icons/mixer.svg")),
    ("moon", include_bytes!("../assets/icons/moon.svg")),
    ("paste", include_bytes!("../assets/icons/paste.svg")),
    ("pencil", include_bytes!("../assets/icons/pencil.svg")),
    ("people", include_bytes!("../assets/icons/people.svg")),
    ("play", include_bytes!("../assets/icons/play.svg")),
    ("plus", include_bytes!("../assets/icons/plus.svg")),
    ("stop", include_bytes!("../assets/icons/stop.svg")),
    ("sun", include_bytes!("../assets/icons/sun.svg")),
    ("swing", include_bytes!("../assets/icons/swing.svg")),
    ("tap", include_bytes!("../assets/icons/tap.svg")),
    ("to-start", include_bytes!("../assets/icons/to-start.svg")),
    ("trainer", include_bytes!("../assets/icons/trainer.svg")),
    ("upload", include_bytes!("../assets/icons/upload.svg")),
    ("user", include_bytes!("../assets/icons/user.svg")),
    ("zoom-in", include_bytes!("../assets/icons/zoom-in.svg")),
    ("zoom-out", include_bytes!("../assets/icons/zoom-out.svg")),
];

/// Fonte de assets do app: só os ícones.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let name = path.strip_prefix("icons/").and_then(|p| p.strip_suffix(".svg"));
        Ok(name.and_then(|n| ICONS.iter().find(|(k, _)| *k == n)).map(|(_, b)| Cow::Borrowed(*b)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(if path.trim_end_matches('/') == "icons" { ICONS.iter().map(|(n, _)| SharedString::from(format!("icons/{n}.svg"))).collect() } else { Vec::new() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todos_os_icones_carregam() {
        for (n, b) in ICONS {
            let s = std::str::from_utf8(b).unwrap();
            assert!(s.starts_with("<svg") && s.contains("viewBox=\"0 0 24 24\""), "{n}");
            assert!(Assets.load(&format!("icons/{n}.svg")).unwrap().is_some());
        }
        assert!(Assets.load("icons/nao-existe.svg").unwrap().is_none());
    }
}
