//! Backend cpal: abre o dispositivo padrão e roda o [`Engine`] no callback.

use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering;
use std::time::Instant;

use anyhow::{Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use wadawko_samples::SampleBank;

use crate::controller::{Controller, engine_pair};
use crate::engine::{EngineConfig, MasterMode, SampleSet};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioConfig {
    /// tamanho de buffer pedido (frames); 256 ≈ 5 ms a 48 kHz
    pub buffer_frames: u32,
    /// semente da humanização
    pub seed: u64,
    /// volume de saída inicial (0 = mudo)
    pub master_gain: f32,
    /// define `PIPEWIRE_LATENCY=<buffer>/<taxa>` se ainda não estiver definido:
    /// sem isso o grafo do PipeWire roda com quantum 1024 (~21 ms a mais)
    pub pipewire_latency: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        AudioConfig { buffer_frames: 256, seed: 0x0007_A1C0, master_gain: 1.0, pipewire_latency: true }
    }
}

/// Engine rodando num dispositivo de saída. É um [`Controller`] (via `Deref`)
/// que mantém o stream vivo; soltar o `Audio` fecha o dispositivo.
pub struct Audio {
    ctl: Controller,
    /// canais do dispositivo
    pub channels: usize,
    _stream: cpal::Stream,
}

impl Deref for Audio {
    type Target = Controller;
    fn deref(&self) -> &Controller {
        &self.ctl
    }
}

impl DerefMut for Audio {
    fn deref_mut(&mut self) -> &mut Controller {
        &mut self.ctl
    }
}

/// Taxa do dispositivo de saída padrão (48 kHz se não houver dispositivo).
pub fn device_rate() -> u32 {
    cpal::default_host().default_output_device().and_then(|d| d.default_output_config().ok()).map(|c| c.sample_rate()).unwrap_or(48_000)
}

impl Audio {
    /// Abre o dispositivo padrão (f32, taxa nativa) e começa a rodar o engine
    /// (parado, sem música). Deve ser chamado cedo: `pipewire_latency` mexe em
    /// variável de ambiente do processo.
    pub fn start(bank: &SampleBank, cfg: AudioConfig) -> Result<Audio> {
        if cfg.pipewire_latency && std::env::var_os("PIPEWIRE_LATENCY").is_none() {
            // SAFETY: feito na inicialização, antes de o backend de áudio criar threads
            unsafe { std::env::set_var("PIPEWIRE_LATENCY", format!("{}/48000", cfg.buffer_frames)) };
        }
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or_else(|| anyhow!("sem dispositivo de saída de áudio"))?;
        let def = device.default_output_config()?;
        let sr = def.sample_rate();
        let channels = def.channels() as usize;
        let config = cpal::StreamConfig { channels: def.channels(), sample_rate: sr, buffer_size: cpal::BufferSize::Fixed(cfg.buffer_frames) };

        let samples = SampleSet::from_bank(bank);
        let (mut eng, mut ctl) = engine_pair(bank, samples, EngineConfig { sample_rate: sr, seed: cfg.seed, master: MasterMode::Live, metronome: false });
        ctl.set_master_gain(cfg.master_gain);
        let clock = ctl.clock().clone();
        clock.buffer_frames.store(cfg.buffer_frames, Ordering::Relaxed);
        let err_clock = clock.clone();
        let mut last_cb: Option<Instant> = None;
        let stream = device.build_output_stream::<f32, _, _>(
            config,
            move |out: &mut [f32], info: &cpal::OutputCallbackInfo| {
                let t0 = Instant::now();
                let frames = (out.len() / channels) as u32;
                if let Some(prev) = last_cb {
                    let gap = t0.duration_since(prev).as_secs_f64();
                    if gap > frames as f64 / sr as f64 * 1.5 && eng.is_playing() {
                        clock.late_callbacks.fetch_add(1, Ordering::Relaxed);
                    }
                }
                last_cb = Some(t0);
                eng.drain_commands();
                let ts = info.timestamp();
                clock.latency_ns.store(ts.playback.duration_since(ts.callback).as_nanos() as u64, Ordering::Relaxed);
                clock.buffer_frames.store(frames, Ordering::Relaxed);
                clock.callbacks.fetch_add(1, Ordering::Relaxed);
                eng.publish_clock(t0.duration_since(clock.epoch).as_nanos() as u64);
                eng.process(out, channels);
                clock.max_process_ns.fetch_max(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
            },
            move |_err| {
                err_clock.stream_errors.fetch_add(1, Ordering::Relaxed);
            },
            None,
        )?;
        stream.play()?;
        Ok(Audio { ctl, channels, _stream: stream })
    }
}
