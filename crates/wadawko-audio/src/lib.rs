//! # wadawko-audio
//!
//! Engine de áudio do WaDAWko (docs/planejamento/gpui.md §3.2–3.5) — porte de
//! `src/lib/audio.ts` (Tone.js) e da síntese de `src/lib/kyai.ts`.
//!
//! | módulo | o que tem |
//! |---|---|
//! | [`schedule`] | `Compiler`/`compile`: `Song` → `Schedule` (eventos em ticks, canais, barramentos, `TimeMap`) |
//! | [`engine`] | `Engine` de tempo real (`process()` sem alocação), `Command`, `EngineEvent`, `Clock` |
//! | [`controller`] | `Controller` (lado da GUI): compila/envia, transport, mix, previews, eventos |
//! | [`backend`] | `Audio`: `Controller` + stream do cpal |
//! | [`offline`] | `render`: o mesmo engine sem dispositivo, com `lookahead_limit` |
//! | [`dsp`] | biquads (convenções do WebAudio), compressor, soft clip, limiter, PRNG |
//! | [`kyai_synth`] | kiai sintetizado (animalese com timbre de grito) |
//! | [`kyai_voice`] | kiai com gravações reais (masc/fem) |
//!
//! ## Uso pela UI
//!
//! ```no_run
//! use wadawko_audio::{Audio, AudioConfig, CompileOptions, PlayOptions};
//! let bank = wadawko_samples::SampleBank::load_embedded().unwrap();
//! let song = wadawko_core::demo::demo_song();
//! let mut audio = Audio::start(&bank, AudioConfig::default()).unwrap();
//! let schedule = audio.set_song(&song, &CompileOptions::default()); // a cada edição
//! audio.play(PlayOptions { looping: true, ..Default::default() });
//! // a cada frame da UI:
//! let tick = audio.clock().audible_tick(0.0);       // playhead
//! let cell = schedule.time.cell_at(tick);           // cursor (célula-fonte)
//! for ev in audio.poll_events() { /* HitFired, LoopWrapped, Ended, Xrun… */ }
//! audio.stop();
//! ```

pub mod backend;
pub mod controller;
pub mod dsp;
pub mod engine;
mod fue;
pub mod kyai_synth;
pub mod kyai_voice;
pub mod metronome;
pub mod offline;
pub mod schedule;

pub use backend::{Audio, AudioConfig, device_rate};
pub use controller::{Controller, PlayOptions, TrainerOptions, engine_pair};
pub use engine::{BLOCK, Clock, ClockSnapshot, Command, Engine, EngineConfig, EngineEvent, HUMANIZE_LEAD, MAX_VOICES, MasterMode, Meters, PlayCmd, PreviewSeq, SampleSet, Trainer};
pub use fue::fue_total_secs;
pub use metronome::{ClickLevel, ClickSound, CountVoice, Metronome};
pub use offline::{Cancelled, RenderOptions, Rendered, render, render_with};
pub use schedule::{
    BAR_TICKS, BusDef, ChannelDef, CompileOptions, Compiler, Event, EventKind, GrainEv, HitEv, MAX_BUSES, MAX_CHANNELS, NoiseEv, PPQ, PREVIEW_SLOT, Schedule, TempoCurve, TimeMap, compile, sfz_dyn,
    vel_gain,
};
pub use wadawko_core::timeline::Region;
