//! Teste de fumaça do backend cpal: abre o dispositivo padrão, toca a demo em
//! loop por alguns segundos e mostra callbacks, xruns, tempo de DSP e latência.
//!
//! `cargo run --release -p wadawko-audio --example smoke -- [segundos] [volume 0..1] [buffer]`
//! (o volume padrão é 0 = MUDO: o engine roda inteiro, mas a saída é silêncio).

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use wadawko_audio::{Audio, AudioConfig, CompileOptions, EngineEvent, PlayOptions};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let secs: f64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(4.0);
    let volume: f32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(0.0);
    let buffer: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(256);
    let bank = wadawko_samples::SampleBank::load_embedded()?;
    let song = wadawko_core::demo::demo_song();
    let mut audio = Audio::start(&bank, AudioConfig { buffer_frames: buffer, master_gain: volume, ..Default::default() })?;
    let schedule = audio.set_song(&song, &CompileOptions::default());
    audio.set_metronome(true);
    audio.play(PlayOptions { looping: true, ..Default::default() });
    let clock = audio.clock().clone();
    println!("dispositivo: {} Hz, {} canais, buffer pedido {buffer}, volume {volume}", clock.sample_rate, audio.channels);
    let t0 = Instant::now();
    let (mut hits, mut wraps, mut xruns, mut last_print) = (0usize, 0usize, 0u32, 0u64);
    let mut ticks = Vec::new();
    while t0.elapsed().as_secs_f64() < secs {
        std::thread::sleep(Duration::from_millis(16));
        for ev in audio.poll_events() {
            match ev {
                EngineEvent::HitFired { .. } => hits += 1,
                EngineEvent::LoopWrapped { .. } => wraps += 1,
                EngineEvent::Xrun { count } => xruns += count,
                _ => {}
            }
        }
        let tick = clock.audible_tick(0.0);
        ticks.push((t0.elapsed().as_secs_f64(), tick));
        let s = t0.elapsed().as_secs();
        if s > last_print {
            last_print = s;
            println!("  t={s}s tick audível {tick:8.1} célula {:?} vozes {}", schedule.time.cell_at(tick), clock.active_voices.load(Ordering::Relaxed));
        }
    }
    audio.stop();
    std::thread::sleep(Duration::from_millis(100));
    // o relógio audível anda no ritmo do BPM (100 BPM = 1600 ticks/s)
    let (a, b) = (ticks[ticks.len() / 4], ticks[ticks.len() - 1]);
    let rate = (b.1 - a.1).rem_euclid(schedule.len_ticks) / (b.0 - a.0);
    println!(
        "callbacks {} · buffer real {} frames · atrasados {} · erros de stream {} · xruns reportados {} · DSP máx {:.3} ms · latência informada {:.1} ms (+{:.1} ms fixos do engine)",
        clock.callbacks.load(Ordering::Relaxed),
        clock.buffer_frames.load(Ordering::Relaxed),
        clock.late_callbacks.load(Ordering::Relaxed),
        clock.stream_errors.load(Ordering::Relaxed),
        xruns,
        clock.max_process_ns.load(Ordering::Relaxed) as f64 / 1e6,
        clock.latency_ns.load(Ordering::Relaxed) as f64 / 1e6,
        clock.fixed_delay_ns.load(Ordering::Relaxed) as f64 / 1e6,
    );
    println!("golpes {hits} · voltas {wraps} · relógio audível {rate:.0} ticks/s (esperado 1600)");
    Ok(())
}
