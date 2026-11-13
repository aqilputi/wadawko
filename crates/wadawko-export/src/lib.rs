//! # wadawko-export
//!
//! Export de áudio do WaDAWko (PLANEJAMENTO-gpui.md §3.5) — porte de
//! `src/lib/exportAudio.ts`: render offline com o MESMO engine do playback
//! ([`wadawko_audio::offline`]), normalização de pico em 0,98 como a web, e
//! gravação em WAV (16/24-bit) ou MP3 192 kbps (via `ffmpeg`/libmp3lame, com o
//! título nos metadados). Roda em thread com progresso e cancelamento
//! ([`spawn_export`]).
//!
//! ```no_run
//! let bank = wadawko_samples::SampleBank::load_embedded().unwrap();
//! let song = wadawko_core::demo::demo_song();
//! let job = wadawko_export::spawn_export(song, bank, "renshu.mp3".into(), Default::default());
//! while !job.is_finished() {
//!     println!("{} {:.0} %", job.stage().label(), job.progress() * 100.0); // ou job.cancel()
//!     std::thread::sleep(std::time::Duration::from_millis(100));
//! }
//! let summary = job.join().unwrap();
//! ```

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::thread::JoinHandle;

use anyhow::{Context, Result, anyhow, bail};
use wadawko_audio::{RenderOptions, SampleSet, render_with};
use wadawko_core::Song;
use wadawko_core::model::PROJECT_KEY_PREFIX;
use wadawko_samples::SampleBank;

/// Teto da normalização de pico do export (o mesmo do `encodeMp3` da web).
pub const PEAK_CEILING: f32 = 0.98;
/// Bitrate do MP3 (como a web).
pub const MP3_KBPS: u32 = 192;

// ───────────────────────────── formato ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitDepth {
    Pcm16,
    Pcm24,
}

impl BitDepth {
    pub fn bits(self) -> u16 {
        match self {
            BitDepth::Pcm16 => 16,
            BitDepth::Pcm24 => 24,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioFormat {
    Wav(BitDepth),
    Mp3 { kbps: u32 },
}

impl AudioFormat {
    /// Formato pela extensão do arquivo: `.wav` (16-bit) ou `.mp3` (192 kbps).
    pub fn from_path(path: &Path) -> Result<AudioFormat> {
        match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
            Some("wav") => Ok(AudioFormat::Wav(BitDepth::Pcm16)),
            Some("mp3") => Ok(AudioFormat::Mp3 { kbps: MP3_KBPS }),
            other => bail!("extensão .{} não suportada (use .wav ou .mp3)", other.unwrap_or("")),
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            AudioFormat::Wav(_) => "wav",
            AudioFormat::Mp3 { .. } => "mp3",
        }
    }
}

/// Garante a extensão do formato (diálogos de arquivo podem devolver o nome sem ela).
pub fn with_extension(path: PathBuf, format: AudioFormat) -> PathBuf {
    let ext = format.extension();
    if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(ext)) { path } else { PathBuf::from(format!("{}.{ext}", path.display())) }
}

// ───────────────────────────── PCM ─────────────────────────────

/// Só abaixa se clipar — não infla músicas baixas (`encodeMp3` da web).
/// Devolve o ganho aplicado (1 = não mexeu).
pub fn normalize_peak(samples: &mut [f32], ceiling: f32) -> f32 {
    let peak = samples.iter().fold(0f32, |a, s| a.max(s.abs()));
    if peak <= ceiling {
        return 1.0;
    }
    let gain = ceiling / peak;
    for s in samples.iter_mut() {
        *s *= gain;
    }
    gain
}

/// float [-1, 1] → PCM inteiro, como o `toInt16` da web (negativos × 2^(n−1), positivos × 2^(n−1) − 1).
fn quantize(v: f32, bits: u16) -> i32 {
    let v = if v.is_finite() { v.clamp(-1.0, 1.0) as f64 } else { 0.0 };
    let full = (1i64 << (bits - 1)) as f64;
    (if v < 0.0 { v * full } else { v * (full - 1.0) }) as i32
}

/// Codifica estéreo intercalado em WAV (RIFF, PCM inteiro, 2 canais).
pub fn encode_wav(samples: &[f32], sample_rate: u32, depth: BitDepth) -> Vec<u8> {
    let bytes = (depth.bits() / 8) as usize;
    let data_len = (samples.len() * bytes) as u32;
    let mut w = Vec::with_capacity(44 + data_len as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data_len).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&sample_rate.to_le_bytes());
    w.extend_from_slice(&(sample_rate * 2 * bytes as u32).to_le_bytes());
    w.extend_from_slice(&(2 * bytes as u16).to_le_bytes());
    w.extend_from_slice(&depth.bits().to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        w.extend_from_slice(&quantize(*s, depth.bits()).to_le_bytes()[..bytes]);
    }
    w
}

pub fn write_wav(path: &Path, samples: &[f32], sample_rate: u32, depth: BitDepth) -> Result<()> {
    std::fs::write(path, encode_wav(samples, sample_rate, depth)).with_context(|| format!("gravar {}", path.display()))
}

// ───────────────────────────── MP3 (ffmpeg) ─────────────────────────────

pub fn ffmpeg_available() -> bool {
    Command::new("ffmpeg").arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Grava MP3 (libmp3lame, joint stereo) chamando o `ffmpeg`: o PCM vai por
/// pipe (sem arquivo temporário). `progress` recebe 0..1; `cancel` mata o
/// ffmpeg e apaga o arquivo parcial.
pub fn write_mp3(path: &Path, samples: &[f32], sample_rate: u32, title: &str, kbps: u32, progress: &mut dyn FnMut(f32), cancel: &AtomicBool) -> Result<()> {
    if !ffmpeg_available() {
        bail!("ffmpeg não encontrado — instale o ffmpeg ou exporte em WAV");
    }
    let mut child = Command::new("ffmpeg")
        .args(["-y", "-hide_banner", "-loglevel", "error", "-f", "s16le", "-ar"])
        .arg(sample_rate.to_string())
        .args(["-ac", "2", "-i", "pipe:0", "-codec:a", "libmp3lame", "-b:a"])
        .arg(format!("{kbps}k"))
        .args(["-id3v2_version", "3", "-metadata"])
        .arg(format!("title={title}"))
        .args(["-metadata", "comment=WaDAWko", "-f", "mp3"])
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("rodar ffmpeg")?;
    let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("ffmpeg sem stdin"))?;
    let mut buf = Vec::with_capacity(1152 * 400 * 4);
    let total = samples.len().max(1);
    let mut failed = false;
    // blocos de 400 quadros MP3 (1152 amostras), como a web cede o thread
    for (i, chunk) in samples.chunks(1152 * 400 * 2).enumerate() {
        if cancel.load(Ordering::Relaxed) {
            drop(stdin);
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(path);
            bail!("export cancelado");
        }
        buf.clear();
        for s in chunk {
            buf.extend_from_slice(&(quantize(*s, 16) as i16).to_le_bytes());
        }
        if stdin.write_all(&buf).is_err() {
            failed = true; // o ffmpeg morreu; o motivo está no stderr
            break;
        }
        progress((i * 1152 * 400 * 2) as f32 / total as f32);
    }
    drop(stdin);
    let out = child.wait_with_output().context("esperar o ffmpeg")?;
    if failed || !out.status.success() {
        let _ = std::fs::remove_file(path);
        bail!("ffmpeg falhou: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    progress(1.0);
    Ok(())
}

// ───────────────────────────── export de uma música ─────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct ExportOptions {
    /// `None` = pela extensão do arquivo (`.wav` 16-bit / `.mp3` 192 kbps)
    pub format: Option<AudioFormat>,
    /// opções do render offline (taxa, semente, cauda, limiter…)
    pub render: RenderOptions,
    /// título nos metadados do MP3 (`None` = título da música)
    pub title: Option<String>,
    /// teto da normalização de pico (`None` = não normaliza)
    pub peak_ceiling: Option<f32>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions { format: None, render: RenderOptions::default(), title: None, peak_ceiling: Some(PEAK_CEILING) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ExportStage {
    Rendering = 0,
    Encoding = 1,
    Done = 2,
}

impl ExportStage {
    /// rótulo para a barra de status (como o `onStatus` da web)
    pub fn label(self) -> &'static str {
        match self {
            ExportStage::Rendering => "renderizando…",
            ExportStage::Encoding => "codificando…",
            ExportStage::Done => "pronto",
        }
    }
}

/// O que foi gravado.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportSummary {
    pub path: PathBuf,
    pub format: AudioFormat,
    pub sample_rate: u32,
    pub seconds: f64,
    /// pico e RMS do PCM gravado (depois do limiter e da normalização)
    pub peak: f32,
    pub rms: f32,
    /// golpes renderizados
    pub hits: usize,
    /// ganho da normalização de pico (1 = não precisou)
    pub normalize_gain: f32,
    /// ganho de normalização do limiter com lookahead (None = limiter desligado)
    pub limiter_pre_gain: Option<f32>,
    pub limiter_min_gain: Option<f32>,
}

/// Renderiza e grava uma música. `progress(estágio, 0..1)`; `cancel` aborta
/// (erro "cancelado", sem deixar arquivo parcial). Bloqueia até terminar — na
/// UI use [`spawn_export`].
pub fn export_song(
    song: &Song,
    bank: &SampleBank,
    samples: &Arc<SampleSet>,
    path: &Path,
    opts: &ExportOptions,
    progress: &mut dyn FnMut(ExportStage, f32),
    cancel: &AtomicBool,
) -> Result<ExportSummary> {
    let format = match opts.format {
        Some(f) => f,
        None => AudioFormat::from_path(path)?,
    };
    if matches!(format, AudioFormat::Mp3 { .. }) && !ffmpeg_available() {
        bail!("ffmpeg não encontrado — instale o ffmpeg ou exporte em WAV");
    }
    progress(ExportStage::Rendering, 0.0);
    let mut rendered = render_with(song, bank, samples, &opts.render, &mut |p| progress(ExportStage::Rendering, p), cancel).map_err(|_| anyhow!("export cancelado"))?;
    let normalize_gain = opts.peak_ceiling.map_or(1.0, |c| normalize_peak(&mut rendered.samples, c));
    progress(ExportStage::Encoding, 0.0);
    if cancel.load(Ordering::Relaxed) {
        bail!("export cancelado");
    }
    match format {
        AudioFormat::Wav(depth) => write_wav(path, &rendered.samples, rendered.sample_rate, depth)?,
        AudioFormat::Mp3 { kbps } => {
            let title = opts.title.as_deref().unwrap_or(&song.title);
            write_mp3(path, &rendered.samples, rendered.sample_rate, title, kbps, &mut |p| progress(ExportStage::Encoding, p), cancel)?;
        }
    }
    progress(ExportStage::Done, 1.0);
    Ok(ExportSummary {
        path: path.to_path_buf(),
        format,
        sample_rate: rendered.sample_rate,
        seconds: rendered.seconds(),
        peak: rendered.peak(),
        rms: rendered.rms(),
        hits: rendered.hits,
        normalize_gain,
        limiter_pre_gain: rendered.limiter.map(|l| l.pre_gain),
        limiter_min_gain: rendered.limiter.map(|l| l.min_gain),
    })
}

/// Export rodando numa thread de fundo.
pub struct ExportJob {
    stage: Arc<AtomicU8>,
    progress: Arc<AtomicU32>,
    cancel: Arc<AtomicBool>,
    handle: JoinHandle<Result<ExportSummary>>,
}

impl ExportJob {
    pub fn stage(&self) -> ExportStage {
        match self.stage.load(Ordering::Relaxed) {
            0 => ExportStage::Rendering,
            1 => ExportStage::Encoding,
            _ => ExportStage::Done,
        }
    }

    /// progresso (0..1) do estágio atual
    pub fn progress(&self) -> f32 {
        f32::from_bits(self.progress.load(Ordering::Relaxed))
    }

    /// pede o cancelamento (o `join` devolve erro "export cancelado")
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

    pub fn join(self) -> Result<ExportSummary> {
        self.handle.join().map_err(|_| anyhow!("a thread de export entrou em pânico"))?
    }
}

/// Renderiza e grava em thread de fundo. A música é uma cópia (a UI pode
/// continuar editando); `bank` é barato de clonar.
pub fn spawn_export(song: Song, bank: SampleBank, path: PathBuf, opts: ExportOptions) -> ExportJob {
    let stage = Arc::new(AtomicU8::new(0));
    let progress = Arc::new(AtomicU32::new(0f32.to_bits()));
    let cancel = Arc::new(AtomicBool::new(false));
    let (s, p, c) = (stage.clone(), progress.clone(), cancel.clone());
    let handle = std::thread::Builder::new()
        .name("wadawko-export".into())
        .spawn(move || {
            let samples = SampleSet::from_bank(&bank);
            export_song(
                &song,
                &bank,
                &samples,
                &path,
                &opts,
                &mut |st, f| {
                    s.store(st as u8, Ordering::Relaxed);
                    p.store(f.to_bits(), Ordering::Relaxed);
                },
                &c,
            )
        })
        .expect("criar thread de export");
    ExportJob { stage, progress, cancel, handle }
}

// ───────────────────────────── arquivos de música ─────────────────────────────

/// Lê uma música de um `.taiko.json`/`.wadawko.json` (objeto `Song` v1). Também
/// aceita um backup do localStorage (dicionário `wadawko:project:<id>` → JSON):
/// aí `project` escolhe a música por título ou id (obrigatório se houver mais de uma).
pub fn load_song_file(path: &Path, project: Option<&str>) -> Result<Song> {
    let text = std::fs::read_to_string(path).with_context(|| format!("ler {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text).with_context(|| format!("{}: JSON inválido", path.display()))?;
    let is_backup = value.as_object().is_some_and(|o| !o.contains_key("measures") && o.keys().any(|k| k.starts_with(PROJECT_KEY_PREFIX)));
    if !is_backup {
        return Song::from_value(value).map_err(|e| anyhow!("{}: {e} ({})", path.display(), e.detail()));
    }
    let mut songs = Vec::new();
    for (key, raw) in value.as_object().into_iter().flatten().filter(|(k, _)| k.starts_with(PROJECT_KEY_PREFIX)) {
        let song = match raw {
            serde_json::Value::String(s) => Song::from_json(s),
            other => Song::from_value(other.clone()),
        };
        if let Ok(song) = song {
            songs.push((key[PROJECT_KEY_PREFIX.len()..].to_string(), song));
        }
    }
    let list = || songs.iter().map(|(id, s)| format!("{id} ({:?}, {} compassos)", s.title, s.measures.len())).collect::<Vec<_>>().join(", ");
    match project {
        Some(want) => {
            let by_id = songs.iter().position(|(id, _)| id == want);
            let by_title: Vec<usize> = songs.iter().enumerate().filter(|(_, (_, s))| s.title == want).map(|(i, _)| i).collect();
            match (by_id, by_title.as_slice()) {
                (Some(i), _) => Ok(songs.swap_remove(i).1),
                (None, [i]) => Ok(songs.swap_remove(*i).1),
                (None, []) => bail!("projeto {want:?} não encontrado; disponíveis: {}", list()),
                (None, _) => bail!("há mais de um projeto com o título {want:?}; use o id: {}", list()),
            }
        }
        None if songs.len() == 1 => Ok(songs.remove(0).1),
        None => bail!("o arquivo é um backup com {} projetos; escolha um com --project: {}", songs.len(), list()),
    }
}
