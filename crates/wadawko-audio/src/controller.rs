//! Lado da GUI: `Controller` compila a música, manda comandos pelo ring buffer
//! SPSC, lê os eventos de volta, recolhe o lixo e pede previews. Não depende
//! de dispositivo de áudio (o [`crate::backend::Audio`] é um `Controller` + stream).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use wadawko_core::kyai::{kyai_note, kyai_profile, kyai_voice_or_default};
use wadawko_core::ops::kit::resolve_instrument;
use wadawko_core::ops::people::person_audible;
use wadawko_core::phrase::resolve_cell;
use wadawko_core::timeline::swing_warp;
use wadawko_core::{InstrumentDef, Song, Track, Zone};
use wadawko_samples::{RoundRobin, SampleBank};

use crate::dsp::Rng;
use crate::engine::{Clock, Command, Engine, EngineConfig, EngineEvent, Garbage, PlayCmd, PreviewSeq, SampleSet, Trainer};
use crate::fue::fue_total_secs;
use crate::kyai_voice::kyai_grains;
use crate::schedule::{CompileOptions, Compiler, EventKind, PREVIEW_KYAI_SLOT, PREVIEW_SLOT, Schedule, channel_key, db_gain, hit_event, sfz_dyn, track_audible, zone_chain};

/// Treino (`opts.trainer` do TS): começa em `start_pct × tempo` e sobe
/// `step × tempo` a cada volta, até o andamento da música.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrainerOptions {
    pub start_pct: f64,
    pub step: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayOptions {
    /// célula-fonte global de onde começar (ignorada quando o schedule tem região)
    pub start_cell: usize,
    pub looping: bool,
    /// 1 compasso (4 cliques) antes de começar
    pub count_in: bool,
    /// só vale com região + loop, como no web app
    pub trainer: Option<TrainerOptions>,
}

pub struct Controller {
    tx: rtrb::Producer<Command>,
    events: rtrb::Consumer<EngineEvent>,
    garbage: rtrb::Consumer<Garbage>,
    clock: Arc<Clock>,
    compiler: Compiler,
    schedule: Arc<Schedule>,
    preview_rr: HashMap<String, RoundRobin>,
    rng: Rng,
    errors_seen: u64,
    fue_until: Option<Instant>,
}

/// Cria o par engine (vai pro thread de áudio) + controller (fica na GUI).
pub fn engine_pair(bank: &SampleBank, samples: Arc<SampleSet>, cfg: EngineConfig) -> (Engine, Controller) {
    let (tx, rx) = rtrb::RingBuffer::new(4096);
    let (ev_tx, ev_rx) = rtrb::RingBuffer::new(8192);
    let (g_tx, g_rx) = rtrb::RingBuffer::new(256);
    let clock = Clock::new(cfg.sample_rate, 100.0);
    let mut engine = Engine::with_clock(cfg, samples, clock.clone());
    engine.attach(rx, ev_tx, g_tx);
    let ctl = Controller {
        tx,
        events: ev_rx,
        garbage: g_rx,
        clock,
        compiler: Compiler::new(bank, cfg.sample_rate),
        schedule: Arc::new(Schedule::empty()),
        preview_rr: HashMap::new(),
        rng: Rng::new(cfg.seed ^ 0xC0FFEE),
        errors_seen: 0,
        fue_until: None,
    };
    (engine, ctl)
}

impl Controller {
    pub fn clock(&self) -> &Arc<Clock> {
        &self.clock
    }

    pub fn sample_rate(&self) -> u32 {
        self.clock.sample_rate
    }

    /// último schedule enviado (tem o `TimeMap` para cursor/seek)
    pub fn schedule(&self) -> &Arc<Schedule> {
        &self.schedule
    }

    /// Solta, fora do callback, os schedules/sequências que o engine devolveu.
    pub fn collect_garbage(&mut self) {
        while self.garbage.pop().is_ok() {}
    }

    /// Envia um comando cru. `false` = fila cheia (o engine parou de consumir?).
    pub fn send(&mut self, cmd: Command) -> bool {
        self.collect_garbage();
        self.tx.push(cmd).is_ok()
    }

    /// Eventos vindos do thread de áudio desde a última chamada (chame a cada frame da UI).
    pub fn poll_events(&mut self) -> Vec<EngineEvent> {
        self.collect_garbage();
        let mut out = Vec::new();
        while let Ok(e) = self.events.pop() {
            out.push(e);
        }
        let errors = self.clock.stream_errors.load(Ordering::Relaxed);
        if errors > self.errors_seen {
            out.push(EngineEvent::Xrun { count: (errors - self.errors_seen) as u32 });
            self.errors_seen = errors;
        }
        out
    }

    // ───────────── música ─────────────

    /// Compila a música e manda pro engine (a posição do playback é mantida).
    /// Chame a cada edição (com debounce) e ao mudar região/mudo/solo/kit.
    pub fn set_song(&mut self, song: &Song, opts: &CompileOptions) -> Arc<Schedule> {
        let schedule = Arc::new(self.compiler.compile(song, opts));
        self.set_schedule(schedule.clone());
        schedule
    }

    pub fn set_schedule(&mut self, schedule: Arc<Schedule>) {
        self.schedule = schedule.clone();
        self.send(Command::SetSchedule(schedule));
    }

    /// Reaplica volume/pan/mudo/solo de linhas e pessoas SEM recompilar
    /// (`applyMix`): 2 camadas, canal da linha e barramento da pessoa.
    pub fn apply_mix(&mut self, song: &Song) {
        let schedule = self.schedule.clone();
        for (slot, ch) in schedule.channels.iter().enumerate() {
            if !ch.is_used() {
                continue;
            }
            let Some(track) = song.track(&ch.track_id) else { continue };
            let gain = if ch.kyai { db_gain(kyai_voice_or_default(track.kyai.as_ref()).2) } else { db_gain(track.volume.unwrap_or(Track::DEFAULT_VOLUME_DB)) };
            self.send(Command::SetChannelMix { slot: slot as u16, gain, pan: track.pan.unwrap_or(0.0) as f32, mute: !track_audible(song, track) });
        }
        for (slot, bus) in schedule.buses.iter().enumerate().skip(1) {
            if bus.person_id.is_empty() {
                continue;
            }
            let Some(person) = song.person(&bus.person_id) else { continue };
            self.send(Command::SetBusMix { slot: slot as u16, gain: db_gain(person.volume.unwrap_or(0.0)), mute: !person_audible(song, Some(&bus.person_id)) });
        }
    }

    // ───────────── transport ─────────────

    /// Começa a tocar o schedule atual. Devolve o BPM inicial (≠ do tempo da
    /// música quando o treino está valendo).
    pub fn play(&mut self, opts: PlayOptions) -> f32 {
        let s = self.schedule.clone();
        let base = s.tempo;
        let trainer = opts.trainer.filter(|_| s.has_region && opts.looping);
        let bpm = match trainer {
            Some(t) => (base * t.start_pct).round().max(20.0),
            None => base,
        } as f32;
        let from_tick = if s.has_region { 0.0 } else { s.time.cell_tick(opts.start_cell) };
        self.send(Command::Play(PlayCmd {
            from_tick,
            bpm,
            looping: opts.looping,
            count_in: opts.count_in,
            trainer: trainer.map(|t| Trainer { base_bpm: base as f32, step: t.step as f32 }),
        }));
        bpm
    }

    pub fn stop(&mut self) {
        self.send(Command::Stop);
    }

    pub fn pause(&mut self) {
        self.send(Command::Pause);
    }

    pub fn resume(&mut self) {
        self.send(Command::Resume);
    }

    /// seek ao vivo para uma célula-fonte global
    pub fn seek_cell(&mut self, global_cell: usize) {
        let tick = self.schedule.time.cell_tick(global_cell);
        self.send(Command::Seek(tick));
    }

    pub fn seek_tick(&mut self, tick: f64) {
        self.send(Command::Seek(tick));
    }

    pub fn set_bpm(&mut self, bpm: f32) {
        self.send(Command::SetBpm(bpm));
    }

    pub fn set_loop(&mut self, looping: bool) {
        self.send(Command::SetLoop(looping));
    }

    pub fn set_metronome(&mut self, on: bool) {
        self.send(Command::SetMetronome(on));
    }

    /// volume geral de saída (linear; 0 = mudo)
    pub fn set_master_gain(&mut self, gain: f32) {
        self.send(Command::SetMasterGain(gain));
    }

    /// abafa tudo que está soando
    pub fn panic(&mut self) {
        self.send(Command::Panic);
    }

    // ───────────── previews ─────────────

    fn preview_strips(&mut self, def: &InstrumentDef) {
        let sr = self.compiler.sample_rate();
        for (z, zone) in [Zone::Head, Zone::Rim].into_iter().enumerate() {
            self.send(Command::SetStrip { strip: PREVIEW_SLOT * 2 + z as u16, chain: zone_chain(def, zone, sr) });
        }
    }

    /// Preview do conteúdo de uma célula ao escrever (`Engine.preview`): toca
    /// pelo canal da própria linha (volume, pan, mudo e barramento dela).
    pub fn preview_cell(&mut self, song: &Song, track: &Track, syl: &str) {
        let Some(resolved) = resolve_cell(syl) else { return };
        let def = resolve_instrument(song, track);
        let key = channel_key(track, track.person_ids().first().map(String::as_str));
        let slot = match self.schedule.channel_slot(&key) {
            Some(s) => s,
            None => {
                self.preview_strips(def);
                PREVIEW_SLOT
            }
        };
        for hit in &resolved.hits {
            let dynv = if hit.sfz { sfz_dyn(1.0) } else { 1.0 };
            let strip = slot * 2 + (hit.zone == Zone::Rim) as u16;
            let ev = hit_event(self.compiler.bank(), def, hit.zone, hit.vel, dynv, self.preview_rr.entry(key.clone()).or_default(), strip);
            self.send(Command::Trigger { kind: EventKind::Hit(ev), delay: (hit.offset * 0.2) as f32 });
        }
    }

    /// Toca UMA batida de uma zona de uma def (`previewInstrument`, editor de Kit).
    pub fn preview_instrument(&mut self, def: &InstrumentDef, zone: Zone) {
        self.preview_strips(def);
        let strip = PREVIEW_SLOT * 2 + (zone == Zone::Rim) as u16;
        let ev = hit_event(self.compiler.bank(), def, zone, 1.0, 1.0, self.preview_rr.entry(format!("def:{}", def.id)).or_default(), strip);
        self.send(Command::Trigger { kind: EventKind::Hit(ev), delay: 0.02 });
    }

    /// Loop de preview de uma frase (`startPhraseLoop`, hover no catálogo):
    /// `unit_sec` = duração de cada palavra. Toca até `stop_phrase_loop`.
    pub fn start_phrase_loop(&mut self, def: &InstrumentDef, words: &[&str], unit_sec: f64, swing: f64) {
        self.stop_phrase_loop();
        if words.is_empty() || unit_sec <= 0.0 {
            return;
        }
        self.preview_strips(def);
        let sr = self.compiler.sample_rate();
        let total = words.len() as f64 * unit_sec;
        // 4 passadas por volta da sequência, para o round robin variar
        const PASSES: usize = 4;
        let mut events = Vec::new();
        let mut rr = RoundRobin::new();
        for pass in 0..PASSES {
            for (i, word) in words.iter().enumerate() {
                let Some(resolved) = resolve_cell(word) else { continue };
                for hit in &resolved.hits {
                    let t = pass as f64 * total + (i as f64 + swing_warp(hit.offset, swing)) * unit_sec;
                    let strip = PREVIEW_SLOT * 2 + (hit.zone == Zone::Rim) as u16;
                    events.push(((t * sr) as u32, EventKind::Hit(hit_event(self.compiler.bank(), def, hit.zone, hit.vel, 1.0, &mut rr, strip))));
                }
            }
        }
        events.sort_by_key(|e| e.0);
        let seq = PreviewSeq { events, len_frames: (PASSES as f64 * total * sr) as u32, looping: true, start_delay: (0.08 * sr) as u32 };
        self.send(Command::PlaySeq(Arc::new(seq)));
    }

    pub fn stop_phrase_loop(&mut self) {
        self.send(Command::StopSeq);
    }

    /// Preview de um kiai ao escrever (`previewKyai`): as sílabas tocam uma por
    /// `unit_sec` (o passo da célula), com a voz do dono (`seed_id` = pessoa)
    /// e a entonação da palavra.
    pub fn preview_kyai(&mut self, track: &Track, syls: &[&str], unit_sec: f64, seed_id: Option<&str>) {
        if syls.is_empty() {
            return;
        }
        let first = track.person_ids().first().map(String::as_str);
        let owner = if track.is_shared() { seed_id.or(first) } else { None };
        let key = format!("{}#kyai", channel_key(track, owner));
        let slot = self.schedule.channel_slot(&key).unwrap_or(PREVIEW_KYAI_SLOT);
        // como no TS, o preview desmuta o canal e aplica o volume da voz
        self.send(Command::SetChannelMix { slot, gain: db_gain(kyai_voice_or_default(track.kyai.as_ref()).2), pan: track.pan.unwrap_or(0.0) as f32, mute: false });
        let prof = kyai_profile(seed_id.or(first).unwrap_or(&track.id), track);
        let sr = self.compiler.sample_rate();
        let animalese = self.compiler.bank().animalese_id().0;
        let shout = |s: &str| s.ends_with('!');
        // agrupa em palavras como na música (quebra depois de cada "!")
        let mut start = 0;
        let mut grains = Vec::new();
        for (i, syl) in syls.iter().enumerate() {
            if i > 0 && shout(syls[i - 1]) {
                start = i;
            }
            let mut end = i;
            while end < syls.len() - 1 && !shout(syls[end]) {
                end += 1;
            }
            let gap = if i < syls.len() - 1 { unit_sec } else { f64::INFINITY };
            let rng = &mut self.rng;
            let note = kyai_note(syl, i - start, end - start + 1, shout(syl), gap, &prof, || rng.unit64());
            kyai_grains(&note, &prof, animalese, slot * 2, 0.03 + i as f64 * unit_sec, sr, rng, &mut grains);
        }
        for kind in grains {
            self.send(Command::Trigger { kind, delay: 0.0 });
        }
    }

    /// Easter egg: "Sakura Sakura" no fue. Devolve a duração em ms (0 se já está tocando).
    pub fn play_fue(&mut self) -> u64 {
        let now = Instant::now();
        if self.fue_until.is_some_and(|t| now < t) {
            return 0;
        }
        let total = fue_total_secs();
        self.fue_until = Some(now + Duration::from_secs_f64(total + 1.0));
        self.send(Command::PlayFue);
        (total * 1000.0) as u64
    }
}
