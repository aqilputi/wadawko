//! `~/.config/wadawko/settings.json`: preferências do teclado e da sincronia.

use std::path::PathBuf;

use anyhow::{Context as _, Result};

use super::keymap::user_keymap_path;

pub fn settings_path() -> PathBuf {
    user_keymap_path().with_file_name("settings.json")
}

/// settings.json: { "vim": bool, "av_offset_ms": f32 }
#[derive(Clone, Copy)]
pub struct Settings {
    pub vim: bool,
    /// atraso extra do visual em relação ao som (ms); positivo = visual mais tarde
    pub av_offset_ms: f32,
}

pub fn load_settings() -> Settings {
    let v = std::fs::read_to_string(settings_path()).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    Settings {
        vim: v.as_ref().and_then(|v| v["vim"].as_bool()).unwrap_or(true),
        av_offset_ms: v.as_ref().and_then(|v| v["av_offset_ms"].as_f64()).unwrap_or(0.0) as f32,
    }
}

pub fn save_settings(s: Settings) -> Result<()> {
    let p = settings_path();
    std::fs::create_dir_all(p.parent().context("sem pasta")?)?;
    std::fs::write(&p, format!("{{ \"vim\": {}, \"av_offset_ms\": {} }}\n", s.vim, s.av_offset_ms))?;
    Ok(())
}
