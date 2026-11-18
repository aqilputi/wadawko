//! `~/.config/wadawko/settings.json`: preferências do teclado, da sincronia e do metrônomo.

use std::path::PathBuf;

use anyhow::{Context as _, Result};
use wadawko_audio::Metronome;

use crate::views::grid::{CELL_H, CELL_H_MAX, CELL_H_MIN};

use super::keymap::user_keymap_path;

pub fn settings_path() -> PathBuf {
    user_keymap_path().with_file_name("settings.json")
}

/// settings.json: `{ "vim", "av_offset_ms", "metro_pattern", "metro_subdiv",
/// "metro_gain", "count_in_loop" }` — campos ausentes ficam no padrão.
#[derive(Clone, Copy)]
pub struct Settings {
    pub vim: bool,
    /// atraso extra do visual em relação ao som (ms); positivo = visual mais tarde
    pub av_offset_ms: f32,
    /// metrônomo: padrão de acentos, subdivisão e volume (F7)
    pub metro: Metronome,
    /// count-in a cada volta do loop (F7c)
    pub count_in_loop: bool,
    /// altura das linhas da grade em px (F16)
    pub row_h: f32,
}

pub fn load_settings() -> Settings {
    let v = std::fs::read_to_string(settings_path()).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).unwrap_or_default();
    let mut metro = Metronome::default();
    if let Some((pattern, len)) = v["metro_pattern"].as_str().and_then(Metronome::parse_pattern) {
        metro.pattern = pattern;
        metro.len = len;
    }
    metro.subdiv = v["metro_subdiv"].as_u64().map_or(1, |d| d.clamp(1, 4) as u8);
    metro.gain = v["metro_gain"].as_f64().map_or(1.0, |g| g.clamp(0.0, 2.0) as f32);
    let row_h = v["row_h"].as_f64().map_or(CELL_H, |h| (h as f32).clamp(CELL_H_MIN, CELL_H_MAX));
    Settings { vim: v["vim"].as_bool().unwrap_or(true), av_offset_ms: v["av_offset_ms"].as_f64().unwrap_or(0.0) as f32, metro, count_in_loop: v["count_in_loop"].as_bool().unwrap_or(false), row_h }
}

pub fn save_settings(s: Settings) -> Result<()> {
    let p = settings_path();
    std::fs::create_dir_all(p.parent().context("sem pasta")?)?;
    let v = serde_json::json!({
        "vim": s.vim,
        "av_offset_ms": s.av_offset_ms,
        "metro_pattern": s.metro.pattern_text(),
        "metro_subdiv": s.metro.subdiv,
        "metro_gain": s.metro.gain,
        "count_in_loop": s.count_in_loop,
        "row_h": s.row_h,
    });
    std::fs::write(&p, format!("{}\n", serde_json::to_string_pretty(&v)?))?;
    Ok(())
}
