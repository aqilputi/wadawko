//! Render offline (PLANEJAMENTO-gpui.md §3.5): o MESMO engine do playback, sem
//! dispositivo, processando em blocos até o fim da música + cauda — porte de
//! `renderOffline` (`audio.ts`), sem as janelas de 2 s (aqui o custo já é linear).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use wadawko_core::Song;
use wadawko_core::timeline::Region;
use wadawko_samples::SampleBank;

use crate::dsp::{LimitStats, lookahead_limit};
use crate::engine::{Command, Engine, EngineConfig, HUMANIZE_LEAD, MasterMode, PlayCmd, SampleSet};
use crate::schedule::{CompileOptions, compile};

#[derive(Clone, Debug, PartialEq)]
pub struct RenderOptions {
    /// taxa do arquivo (os samples são 48 kHz; a web usa 44,1 kHz por padrão)
    pub sample_rate: u32,
    /// semente da humanização e da variação dos kiai: mesma semente = mesmo áudio
    pub seed: u64,
    /// silêncio antes do 1º tempo (s) — `lead` do TS
    pub lead_secs: f64,
    /// cauda para a ressonância final (s)
    pub tail_secs: f64,
    /// `Clean` (padrão, `MIX.cleanExport`): só −6 dB + limiter com lookahead.
    /// `Live`: a cadeia do playback (compressor + soft clip).
    pub master: MasterMode,
    /// teto do `lookahead_limit` em dBFS; `None` = sem limiter
    pub limiter_ceiling_db: Option<f32>,
    pub metronome: bool,
    /// renderiza só uma região de compassos
    pub region: Option<Region>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions { sample_rate: 48_000, seed: 1, lead_secs: 0.1, tail_secs: 3.0, master: MasterMode::Clean, limiter_ceiling_db: Some(-1.5), metronome: false, region: None }
    }
}

/// Áudio renderizado.
#[derive(Clone, Debug, PartialEq)]
pub struct Rendered {
    pub sample_rate: u32,
    /// estéreo intercalado (L, R, L, R…)
    pub samples: Vec<f32>,
    /// o que o limiter fez (None = desligado)
    pub limiter: Option<LimitStats>,
    /// golpes disparados
    pub hits: usize,
}

impl Rendered {
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }
    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / self.sample_rate as f64
    }
    pub fn peak(&self) -> f32 {
        self.samples.iter().fold(0.0, |a, s| a.max(s.abs()))
    }
    pub fn rms(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        (self.samples.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / self.samples.len() as f64).sqrt() as f32
    }
}

/// O render foi cancelado pelo chamador.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("render cancelado")
    }
}
impl std::error::Error for Cancelled {}

/// Renderiza a música inteira (ritornellos, dinâmica, swing, kiai e o mix
/// atual; quem está mudo nem é agendado). Bloqueia até terminar.
pub fn render(song: &Song, bank: &SampleBank, opts: &RenderOptions) -> Rendered {
    render_with(song, bank, &SampleSet::from_bank(bank), opts, &mut |_| {}, &AtomicBool::new(false)).expect("sem cancelamento")
}

/// Como [`render`], com progresso (0..1, chamado a cada ~0,7 s de áudio) e
/// cancelamento (`cancel` é lido a cada bloco). `samples` pode ser o mesmo
/// `SampleSet` do engine ao vivo.
pub fn render_with(
    song: &Song,
    bank: &SampleBank,
    samples: &Arc<SampleSet>,
    opts: &RenderOptions,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
) -> Result<Rendered, Cancelled> {
    let sr = opts.sample_rate.max(8_000);
    let schedule = Arc::new(compile(song, bank, sr, &CompileOptions { region: opts.region, skip_inaudible: true, seed: opts.seed, bpm: None }));
    let hits = schedule.hits().count();
    let bpm = song.tempo.clamp(10.0, 999.0);
    let total = ((opts.lead_secs.max(0.0) + schedule.seconds(bpm) + opts.tail_secs.max(0.0)) * sr as f64).ceil() as usize;
    let mut out = vec![0f32; total * 2];
    let mut eng = Engine::new(EngineConfig { sample_rate: sr, seed: opts.seed, master: opts.master, metronome: opts.metronome }, samples.clone());
    eng.handle(Command::SetSchedule(schedule));
    eng.handle(Command::Play(PlayCmd { from_tick: 0.0, bpm: bpm as f32, looping: false, count_in: false, trainer: None }));
    // o engine já dispara tudo com 6 ms de antecedência; o resto do lead é silêncio
    let mut done = (((opts.lead_secs - HUMANIZE_LEAD).max(0.0)) * sr as f64).round() as usize;
    let mut block = 0usize;
    while done < total {
        if cancel.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        let n = (total - done).min(512);
        eng.process(&mut out[done * 2..(done + n) * 2], 2);
        done += n;
        block += 1;
        if block.is_multiple_of(64) {
            progress(done as f32 / total as f32 * 0.95);
        }
    }
    let limiter = opts.limiter_ceiling_db.map(|c| lookahead_limit(&mut out, sr, c));
    progress(1.0);
    Ok(Rendered { sample_rate: sr, samples: out, limiter, hits })
}
