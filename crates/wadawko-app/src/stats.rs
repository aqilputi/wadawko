//! Medições (modo --bench e HUD): intervalo entre frames (o que o olho vê), custo de CPU
//! do paint da grade e do palco, e saúde do áudio.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use wadawko_audio::Clock;

/// Amostras guardadas no máximo (≈ 5 min a 60 fps); acima disso descarta as
/// mais antigas em bloco. O máximo é mantido à parte (O(1) para o HUD).
const HIST_CAP: usize = 20_000;

#[derive(Default)]
pub struct Hist {
    ms: Vec<f32>,
    max: f32,
}

impl Hist {
    pub fn record(&mut self, d: Duration) {
        let v = d.as_secs_f32() * 1000.0;
        if self.ms.len() >= HIST_CAP {
            self.ms.drain(..HIST_CAP / 2);
        }
        self.ms.push(v);
        self.max = self.max.max(v);
    }
    fn pct_of(v: &[f32], p: f32) -> f32 {
        if v.is_empty() {
            return 0.0;
        }
        let mut s = v.to_vec();
        s.sort_by(|a, b| a.total_cmp(b));
        s[((s.len() - 1) as f32 * p).round() as usize]
    }
    pub fn pct(&self, p: f32) -> f32 {
        Self::pct_of(&self.ms, p)
    }
    /// Percentil só das últimas `n` amostras (HUD ao vivo).
    pub fn recent_pct(&self, n: usize, p: f32) -> f32 {
        let start = self.ms.len().saturating_sub(n);
        Self::pct_of(&self.ms[start..], p)
    }
    pub fn len(&self) -> usize {
        self.ms.len()
    }
    pub fn max(&self) -> f32 {
        self.max
    }
    pub fn over(&self, limit: f32) -> usize {
        self.ms.iter().filter(|&&m| m > limit).count()
    }
}

#[derive(Default)]
pub struct Stats {
    /// Intervalo entre frames enquanto algo anima.
    pub frames: Hist,
    last_frame: Option<Instant>,
    pub grid_paint: Hist,
}

impl Stats {
    /// Descarta o aquecimento (shaders, cache de glifos, abertura do stream).
    pub fn reset(&mut self) {
        *self = Stats::default();
    }

    pub fn frame(&mut self, active: bool) {
        let now = Instant::now();
        if active {
            if let Some(prev) = self.last_frame {
                let d = now.duration_since(prev);
                if d < Duration::from_millis(250) {
                    self.frames.record(d);
                }
            }
            self.last_frame = Some(now);
        } else {
            self.last_frame = None;
        }
    }

    /// Linha curta p/ a status bar.
    pub fn compact(&self, a: Option<&Clock>) -> String {
        let Some(a) = a else { return "sem áudio".to_string() };
        let p95 = self.frames.recent_pct(240, 0.95);
        let fps = if p95 > 0.0 { 1000.0 / self.frames.recent_pct(240, 0.5) } else { 0.0 };
        format!(
            "{:.0} fps · grade {:.1} ms · áudio {} · atrasos {}",
            fps,
            self.grid_paint.recent_pct(240, 0.95),
            a.buffer_frames.load(Ordering::Relaxed),
            a.late_callbacks.load(Ordering::Relaxed)
        )
    }

    pub fn report(&self, a: Option<&Clock>, secs: f32) -> String {
        let fallback = Clock::new(48_000, 120.0);
        let a = a.unwrap_or(&fallback);
        let f = &self.frames;
        format!(
            "=== wadawko bench ({secs:.1} s) ===\n\
             frames medidos      {}\n\
             fps médio           {:.1}\n\
             intervalo frame ms  p50 {:.2} · p95 {:.2} · p99 {:.2} · max {:.2}\n\
             frames > 8.3 ms     {} ({:.1}%)\n\
             frames > 16.7 ms    {} ({:.1}%)\n\
             paint grade ms      n {} · p50 {:.3} · p95 {:.3} · max {:.3}\n\
             áudio               {} Hz · buffer {} · latência {:.1} ms · callbacks {} · atrasados {} · erros {} · DSP max {:.3} ms",
            f.len(),
            f.len() as f32 / secs,
            f.pct(0.5),
            f.pct(0.95),
            f.pct(0.99),
            f.max(),
            f.over(8.34),
            100.0 * f.over(8.34) as f32 / f.len().max(1) as f32,
            f.over(16.7),
            100.0 * f.over(16.7) as f32 / f.len().max(1) as f32,
            self.grid_paint.len(),
            self.grid_paint.pct(0.5),
            self.grid_paint.pct(0.95),
            self.grid_paint.max(),
            a.sample_rate,
            a.buffer_frames.load(Ordering::Relaxed),
            a.latency_ns.load(Ordering::Relaxed) as f64 / 1e6,
            a.callbacks.load(Ordering::Relaxed),
            a.late_callbacks.load(Ordering::Relaxed),
            a.stream_errors.load(Ordering::Relaxed),
            a.max_process_ns.load(Ordering::Relaxed) as f64 / 1e6,
        )
    }
}
