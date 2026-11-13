//! Exportação (painel MÚSICAS / paleta / Ctrl+E · Ctrl+Shift+E · Ctrl+O):
//! JSON no formato da web (`.taiko.json`), WAV e MP3 renderizados offline com o
//! MESMO motor do playback (paridade som ao vivo × arquivo). MP3 via `ffmpeg`
//! (libmp3lame, 192 kbps, como o exportAudio.ts da web).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};

use crate::audio::{self, SampleData, Schedule};

pub const EXPORT_SR: u32 = 48_000;

/// Pasta sugerida nos diálogos: ~/Downloads (ou o equivalente localizado), senão ~.
pub fn default_dir() -> PathBuf {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    // xdg-user-dirs (ex.: XDG_DOWNLOAD_DIR="$HOME/Downloads")
    if let Ok(cfg) = std::fs::read_to_string(home.join(".config/user-dirs.dirs")) {
        for line in cfg.lines() {
            if let Some(v) = line.strip_prefix("XDG_DOWNLOAD_DIR=") {
                let p = PathBuf::from(v.trim_matches('"').replace("$HOME", &home.to_string_lossy()));
                if p.is_dir() {
                    return p;
                }
            }
        }
    }
    ["Downloads", "Transferências"].iter().map(|d| home.join(d)).find(|p| p.is_dir()).unwrap_or(home)
}

/// Render offline + normalização de pico como a web (teto 0,98).
pub fn render(bank: Arc<Vec<SampleData>>, sched: Arc<Schedule>, bpm: f32) -> Vec<[f32; 2]> {
    let mut frames = audio::render_offline(bank, sched, bpm, EXPORT_SR);
    let peak = frames.iter().flat_map(|f| f.iter()).fold(0f32, |a, s| a.max(s.abs()));
    if peak > 0.98 {
        let g = 0.98 / peak;
        for f in &mut frames {
            f[0] *= g;
            f[1] *= g;
        }
    }
    frames
}

pub fn ffmpeg_available() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok_and(|o| o.status.success())
}

/// Grava WAV (16-bit) ou MP3 (192 kbps via ffmpeg) conforme a extensão.
pub fn write_audio(path: &Path, frames: &[[f32; 2]], title: &str) -> Result<()> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "wav" => audio::write_wav(path, EXPORT_SR, frames),
        "mp3" => {
            if !ffmpeg_available() {
                bail!("ffmpeg não encontrado — instale o ffmpeg ou exporte em WAV");
            }
            let tmp = std::env::temp_dir().join(format!("wadawko-export-{}.wav", std::process::id()));
            audio::write_wav(&tmp, EXPORT_SR, frames)?;
            let out = Command::new("ffmpeg")
                .args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
                .arg(&tmp)
                .args(["-codec:a", "libmp3lame", "-b:a", "192k", "-metadata"])
                .arg(format!("title={title}"))
                .args(["-metadata", "comment=WaDAWko"])
                .arg(path)
                .output()
                .context("rodar ffmpeg")?;
            let _ = std::fs::remove_file(&tmp);
            if !out.status.success() {
                bail!("ffmpeg falhou: {}", String::from_utf8_lossy(&out.stderr).trim());
            }
            Ok(())
        }
        other => bail!("extensão .{other} não suportada (use .mp3 ou .wav)"),
    }
}

/// Garante a extensão (o diálogo pode devolver o nome sem ela).
pub fn with_ext(p: PathBuf, ext: &str) -> PathBuf {
    let ok = p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(ext));
    if ok || p.to_string_lossy().ends_with(".taiko.json") { p } else { PathBuf::from(format!("{}.{ext}", p.display())) }
}

pub fn secs(frames: &[[f32; 2]]) -> f32 {
    frames.len() as f32 / EXPORT_SR as f32
}
