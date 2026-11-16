//! Engine de áudio em tempo real (docs/planejamento/gpui.md §3.2). O MESMO
//! `process()` roda no callback do cpal e no render offline.
//!
//! Regras do `process()`/`drain_commands()`: sem alocação, sem locks, sem log,
//! sem syscalls e sem soltar a última referência de um `Arc` (schedules e
//! sequências antigas voltam pela fila de "lixo"). Tudo é preallocado em
//! [`Engine::new`].
//!
//! Grafo (igual ao do web app): voz → strip (canal × zona: EQ + timbre) →
//! canal (volume/pan/mudo) → barramento da pessoa (ou master) → master
//! (−6 dB → compressor → soft clip). Como no Tone (`Tone.Channel` tem
//! `channelCount: 1`), canais e barramentos somam em mono antes do pan.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

use wadawko_samples::SampleBank;

use crate::dsp::{Biquad, BiquadState, Compressor, CompressorParams, FilterChain, MAX_BANDS, Rng, soft_clip};
use crate::fue::Fue;
use crate::schedule::{Event, EventKind, GrainEv, HitEv, MAX_BUSES, MAX_CHANNELS, MAX_STRIPS, NoiseEv, PPQ, PREVIEW_KYAI_SLOT, PREVIEW_SLOT, Schedule};

/// Pool fixo de vozes (golpes, grãos de kiai, cliques); estourou, rouba a mais antiga.
pub const MAX_VOICES: usize = 512;
/// Bloco interno de processamento (frames).
pub const BLOCK: usize = 128;
/// Antecedência fixa (s) com que todo evento é disparado, para a humanização
/// de tempo (±6 ms) poder adiantar um golpe. O relógio audível já desconta.
pub const HUMANIZE_LEAD: f64 = 0.006;
const HUMANIZE_TIME: f64 = 0.006;
const HUMANIZE_GAIN: f32 = 0.12;
const HUMANIZE_RATE: f32 = 0.01;
const NONE: u32 = u32::MAX;

// ───────────────────────────── samples ─────────────────────────────

/// Um sample já somado em mono (`(L+R)/2`, o downmix que o canal do Tone faz).
pub struct MonoSample {
    pub data: Vec<f32>,
    pub sr: u32,
}

/// Samples do banco prontos para o engine. Construa uma vez por banco e
/// compartilhe (`Arc`) entre o engine ao vivo e os renders offline.
pub struct SampleSet {
    pub samples: Vec<MonoSample>,
}

impl SampleSet {
    pub fn from_bank(bank: &SampleBank) -> Arc<SampleSet> {
        let samples = bank
            .samples()
            .iter()
            .map(|s| MonoSample { data: s.frames.as_chunks::<2>().0.iter().map(|f| (f[0] + f[1]) * 0.5).collect(), sr: s.sr })
            .collect();
        Arc::new(SampleSet { samples })
    }
}

// ───────────────────────────── comandos e eventos ─────────────────────────────

/// Cadeia do master.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MasterMode {
    /// ao vivo: −6 dB → compressor → soft clip (`addMasterLimiter`)
    Live,
    /// export limpo: só os −6 dB; o teto vem do `lookahead_limit` depois
    Clean,
}

/// Treino: a cada volta do loop o BPM sobe `step × base` até o andamento base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trainer {
    pub base_bpm: f32,
    pub step: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayCmd {
    /// posição inicial (ticks)
    pub from_tick: f64,
    pub bpm: f32,
    pub looping: bool,
    /// 1 compasso (4 tempos) de cliques antes de começar
    pub count_in: bool,
    pub trainer: Option<Trainer>,
}

/// Sequência em tempo real (frames), fora do transport: loop de preview de frase.
pub struct PreviewSeq {
    /// (frame, evento), em ordem de frame
    pub events: Vec<(u32, EventKind)>,
    pub len_frames: u32,
    pub looping: bool,
    /// atraso antes do 1º evento (frames)
    pub start_delay: u32,
}

pub enum Command {
    /// troca a música compilada (reposiciona por busca binária no tick atual)
    SetSchedule(Arc<Schedule>),
    Play(PlayCmd),
    /// para e volta ao início (as caudas continuam soando, como no web app)
    Stop,
    Pause,
    Resume,
    /// reposiciona o transport (seek ao vivo), em ticks
    Seek(f64),
    SetBpm(f32),
    SetLoop(bool),
    SetMetronome(bool),
    SetTrainer(Option<Trainer>),
    /// mix de um canal: volume linear, pan −1..1, mudo (mudo/solo já resolvidos)
    SetChannelMix { slot: u16, gain: f32, pan: f32, mute: bool },
    /// destino de um canal (0 = master)
    SetChannelBus { slot: u16, bus: u16 },
    /// mix do barramento de uma pessoa
    SetBusMix { slot: u16, gain: f32, mute: bool },
    /// filtros de um strip (previews)
    SetStrip { strip: u16, chain: FilterChain },
    /// volume geral de saída (depois do master), linear
    SetMasterGain(f32),
    SetMasterMode(MasterMode),
    /// dispara um evento avulso daqui a `delay` s (previews)
    Trigger { kind: EventKind, delay: f32 },
    PlaySeq(Arc<PreviewSeq>),
    /// para a sequência e mata a cauda do canal de preview
    StopSeq,
    PlayFue,
    /// abafa tudo que está soando (30 ms)
    Panic,
    ResetCounters,
}

/// O que volta do thread de áudio. `frame` = contador absoluto de frames de
/// saída em que o evento SOA; compare com [`Clock::audible_frame`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EngineEvent {
    /// golpe disparado: linha, célula-fonte global, golpe dentro da célula (a hitKey do TS)
    HitFired { track: u16, cell: u32, hit: u8, frame: u64 },
    /// o loop deu a volta (`wraps` = voltas completas)
    LoopWrapped { wraps: u32, frame: u64 },
    /// o treino mudou o andamento
    BpmChanged { bpm: f32 },
    /// chegou ao fim sem loop
    Ended { frame: u64 },
    /// callbacks atrasados / erros de stream desde o último aviso
    Xrun { count: u32 },
}

pub(crate) enum Garbage {
    Schedule(#[allow(dead_code)] Arc<Schedule>),
    Seq(#[allow(dead_code)] Arc<PreviewSeq>),
}

// ───────────────────────────── relógio ─────────────────────────────

/// Estado publicado pelo thread de áudio em atômicos (sem locks) — é daqui
/// que a UI tira o playhead.
pub struct Clock {
    seq: AtomicU64,
    pos_bits: AtomicU64,
    stamp_ns: AtomicU64,
    frames: AtomicU64,
    len_bits: AtomicU64,
    preroll: AtomicU64,
    bpm_bits: AtomicU32,
    wraps: AtomicU32,
    playing: AtomicBool,
    looping: AtomicBool,
    /// latência de saída informada pelo dispositivo (ns)
    pub latency_ns: AtomicU64,
    /// atraso fixo do engine (antecedência da humanização + lookahead do compressor), em ns
    pub fixed_delay_ns: AtomicU64,
    pub callbacks: AtomicU64,
    pub late_callbacks: AtomicU64,
    pub stream_errors: AtomicU64,
    pub max_process_ns: AtomicU64,
    pub buffer_frames: AtomicU32,
    /// vozes ativas no último bloco
    pub active_voices: AtomicU32,
    pub sample_rate: u32,
    pub epoch: Instant,
}

/// Leitura coerente do relógio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockSnapshot {
    /// posição do transport (ticks) no início do último bloco
    pub pos_tick: f64,
    /// instante (ns desde `epoch`) dessa posição
    pub stamp_ns: u64,
    /// frames de saída já processados
    pub frames: u64,
    pub len_ticks: f64,
    /// frames de count-in que faltam
    pub preroll_frames: u64,
    pub bpm: f32,
    pub wraps: u32,
    pub playing: bool,
    pub looping: bool,
}

impl Clock {
    pub fn new(sample_rate: u32, bpm: f32) -> Arc<Clock> {
        Arc::new(Clock {
            seq: AtomicU64::new(0),
            pos_bits: AtomicU64::new(0f64.to_bits()),
            stamp_ns: AtomicU64::new(0),
            frames: AtomicU64::new(0),
            len_bits: AtomicU64::new(0f64.to_bits()),
            preroll: AtomicU64::new(0),
            bpm_bits: AtomicU32::new(bpm.to_bits()),
            wraps: AtomicU32::new(0),
            playing: AtomicBool::new(false),
            looping: AtomicBool::new(false),
            latency_ns: AtomicU64::new(0),
            fixed_delay_ns: AtomicU64::new(0),
            callbacks: AtomicU64::new(0),
            late_callbacks: AtomicU64::new(0),
            stream_errors: AtomicU64::new(0),
            max_process_ns: AtomicU64::new(0),
            buffer_frames: AtomicU32::new(0),
            active_voices: AtomicU32::new(0),
            sample_rate,
            epoch: Instant::now(),
        })
    }

    pub fn now_ns(&self) -> u64 {
        self.epoch.elapsed().as_nanos() as u64
    }

    /// Leitura coerente (seqlock: repete se o callback estava escrevendo).
    pub fn snapshot(&self) -> ClockSnapshot {
        loop {
            let s0 = self.seq.load(Ordering::Acquire);
            if s0 & 1 == 0 {
                let snap = ClockSnapshot {
                    pos_tick: f64::from_bits(self.pos_bits.load(Ordering::Relaxed)),
                    stamp_ns: self.stamp_ns.load(Ordering::Relaxed),
                    frames: self.frames.load(Ordering::Relaxed),
                    len_ticks: f64::from_bits(self.len_bits.load(Ordering::Relaxed)),
                    preroll_frames: self.preroll.load(Ordering::Relaxed),
                    bpm: f32::from_bits(self.bpm_bits.load(Ordering::Relaxed)),
                    wraps: self.wraps.load(Ordering::Relaxed),
                    playing: self.playing.load(Ordering::Relaxed),
                    looping: self.looping.load(Ordering::Relaxed),
                };
                if self.seq.load(Ordering::Acquire) == s0 {
                    return snap;
                }
            }
            std::hint::spin_loop();
        }
    }

    pub fn bpm(&self) -> f32 {
        f32::from_bits(self.bpm_bits.load(Ordering::Relaxed))
    }

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    /// tempo (ns) decorrido desde o último bloco, limitado para não disparar se o callback travar
    fn elapsed_ns(&self, s: &ClockSnapshot) -> f64 {
        let buf_ns = self.buffer_frames.load(Ordering::Relaxed) as f64 * 1e9 / self.sample_rate as f64;
        (self.now_ns().saturating_sub(s.stamp_ns) as f64).min((buf_ns * 2.0).max(20e6))
    }

    /// Posição AUDÍVEL agora, em ticks: extrapola do último callback e desconta
    /// a latência de saída, o atraso fixo do engine e `extra_offset_ms`
    /// (ajuste de sincronia do usuário). Parado ou no count-in, é a posição do
    /// transport.
    pub fn audible_tick(&self, extra_offset_ms: f32) -> f64 {
        let s = self.snapshot();
        if !s.playing || s.preroll_frames > 0 {
            return s.pos_tick;
        }
        let lat = self.latency_ns.load(Ordering::Relaxed) as f64 + self.fixed_delay_ns.load(Ordering::Relaxed) as f64 + extra_offset_ms as f64 * 1e6;
        let secs = (self.elapsed_ns(&s) - lat) / 1e9;
        let t = s.pos_tick + secs * s.bpm as f64 / 60.0 * PPQ;
        if s.looping && s.len_ticks > 0.0 {
            if t < 0.0 { if s.wraps > 0 { t + s.len_ticks } else { 0.0 } } else { t % s.len_ticks }
        } else {
            t.clamp(0.0, s.len_ticks.max(0.0))
        }
    }

    /// Frame de saída audível agora (mesma base do `frame` dos [`EngineEvent`]).
    pub fn audible_frame(&self, extra_offset_ms: f32) -> f64 {
        let s = self.snapshot();
        let lat = self.latency_ns.load(Ordering::Relaxed) as f64 + extra_offset_ms as f64 * 1e6;
        s.frames as f64 + (self.elapsed_ns(&s) - lat) / 1e9 * self.sample_rate as f64
    }
}

// ───────────────────────────── vozes ─────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum VKind {
    Sample,
    Noise,
    Click,
}

#[derive(Clone, Copy)]
struct Voice {
    on: bool,
    kind: VKind,
    age: u64,
    /// frames até começar a soar
    delay: u32,
    strip: u16,
    rim: bool,
    /// golpe de tambor (entra no choke); grãos de kiai e cliques não
    drum: bool,
    sample: u16,
    pos: f64,
    inc: f64,
    src_end: f64,
    gain: f32,
    /// envelope em frames: ataque até `att`, patamar, rampa de `rel_start` a `dur`
    t: u32,
    att: u32,
    rel_start: u32,
    dur: u32,
    /// choke/abafamento: frames até começar, duração e progresso
    choke_in: u32,
    choke_len: u32,
    choke_pos: u32,
    // ruído
    filter: Biquad,
    fstate: BiquadState,
    noise: u32,
    // clique
    phase: f32,
    freq: f32,
    env: f32,
}

impl Voice {
    const OFF: Voice = Voice {
        on: false,
        kind: VKind::Sample,
        age: 0,
        delay: 0,
        strip: 0,
        rim: false,
        drum: false,
        sample: 0,
        pos: 0.0,
        inc: 1.0,
        src_end: 0.0,
        gain: 0.0,
        t: 0,
        att: 0,
        rel_start: NONE,
        dur: NONE,
        choke_in: NONE,
        choke_len: 1,
        choke_pos: 0,
        filter: Biquad::IDENTITY,
        fstate: BiquadState::ZERO,
        noise: 1,
        phase: 0.0,
        freq: 0.0,
        env: 0.0,
    };

    /// frames que faltam até a voz acabar sozinha (aprox.; NONE = cauda natural longa)
    fn frames_left(&self) -> u64 {
        let by_dur = if self.dur == NONE { u64::MAX } else { self.dur.saturating_sub(self.t) as u64 };
        let by_src = if self.kind == VKind::Sample && self.inc > 0.0 { ((self.src_end - self.pos) / self.inc).max(0.0) as u64 } else { u64::MAX };
        (self.delay as u64).saturating_add(by_dur.min(by_src))
    }

    /// agenda um abafamento (se ainda não houver um mais cedo)
    fn choke(&mut self, in_frames: u32, len: u32) {
        let len = len.max(1);
        if self.choke_in != NONE && (self.choke_in as u64 + (self.choke_len - self.choke_pos) as u64) <= in_frames as u64 + len as u64 {
            return;
        }
        self.choke_in = in_frames;
        self.choke_len = len;
        self.choke_pos = 0;
    }

    /// ganho do envelope × choke para o frame atual; avança o choke. `false` = a voz acabou.
    #[inline]
    fn envelope(&mut self) -> (f32, bool) {
        let mut e = 1.0f32;
        if self.t < self.att {
            e = self.t as f32 / self.att as f32;
        }
        if self.t >= self.rel_start {
            e *= self.dur.saturating_sub(self.t) as f32 / (self.dur - self.rel_start).max(1) as f32;
        }
        let mut alive = true;
        if self.choke_in != NONE {
            if self.choke_in > 0 {
                self.choke_in -= 1;
            } else {
                e *= 1.0 - self.choke_pos as f32 / self.choke_len as f32;
                self.choke_pos += 1;
                alive = self.choke_pos < self.choke_len;
            }
        }
        (e, alive)
    }
}

#[derive(Clone, Copy)]
struct Strip {
    chain: FilterChain,
    state: [BiquadState; MAX_BANDS],
    /// frames de ressonância dos filtros ainda a processar depois da última voz
    tail: u32,
    dirty: bool,
}

#[derive(Clone, Copy)]
struct Chan {
    cur: [f32; 2],
    target: [f32; 2],
    bus: u16,
}

#[derive(Clone, Copy)]
struct Bus {
    cur: f32,
    target: f32,
}

fn pan_gains(gain: f32, pan: f32, mute: bool) -> [f32; 2] {
    if mute {
        return [0.0, 0.0];
    }
    // StereoPanner com entrada mono: potência constante
    let x = (pan.clamp(-1.0, 1.0) + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
    [gain * x.cos(), gain * x.sin()]
}

// ───────────────────────────── engine ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineConfig {
    pub sample_rate: u32,
    /// semente da humanização (mesma semente + mesmos comandos = mesmo áudio)
    pub seed: u64,
    pub master: MasterMode,
    pub metronome: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig { sample_rate: 48_000, seed: 0x0007_A1C0, master: MasterMode::Live, metronome: false }
    }
}

pub struct Engine {
    sr: f64,
    samples: Arc<SampleSet>,
    clock: Arc<Clock>,
    cmd_rx: Option<rtrb::Consumer<Command>>,
    ev_tx: Option<rtrb::Producer<EngineEvent>>,
    garbage_tx: Option<rtrb::Producer<Garbage>>,
    // transport
    schedule: Arc<Schedule>,
    next_idx: usize,
    playing: bool,
    looping: bool,
    pos: f64,
    bpm: f32,
    preroll: u64,
    metronome: bool,
    trainer: Option<Trainer>,
    wraps: u32,
    // sequência de preview
    seq: Option<Arc<PreviewSeq>>,
    seq_pos: i64,
    seq_idx: usize,
    // vozes e mixer
    voices: Box<[Voice]>,
    age: u64,
    rng: Rng,
    strips: Box<[Strip]>,
    strip_buf: Box<[f32]>,
    chans: Box<[Chan]>,
    buses: [Bus; MAX_BUSES],
    bus_buf: Box<[f32]>,
    bus_dirty: [bool; MAX_BUSES],
    master_l: [f32; BLOCK],
    master_r: [f32; BLOCK],
    master_mode: MasterMode,
    comp: Compressor,
    out_gain: f32,
    out_gain_target: f32,
    fue: Fue,
    frames: u64,
    lead: u32,
    strip_tail: u32,
    // clique do metrônomo (Tone.Synth: triângulo, ataque 1 ms, decay 50 ms, release 20 ms)
    click_att: u32,
    click_rel: u32,
    click_k1: f32,
    click_k2: f32,
    click_end: u32,
    late_reported: u64,
}

impl Engine {
    /// Engine sem filas (render offline, testes): comande com [`Engine::handle`].
    pub fn new(cfg: EngineConfig, samples: Arc<SampleSet>) -> Engine {
        let clock = Clock::new(cfg.sample_rate, 100.0);
        Engine::with_clock(cfg, samples, clock)
    }

    pub(crate) fn with_clock(cfg: EngineConfig, samples: Arc<SampleSet>, clock: Arc<Clock>) -> Engine {
        let sr = cfg.sample_rate as f64;
        let tc = |t: f64| (t + 1.0).ln() / 200f64.ln();
        let mut chans = vec![Chan { cur: [0.0; 2], target: [0.0; 2], bus: 0 }; MAX_CHANNELS].into_boxed_slice();
        // canal de preview: −6 dB → master (`previewChannel`); reserva do kiai: 0 dB
        let g = pan_gains(0.501_187_2, 0.0, false);
        chans[PREVIEW_SLOT as usize] = Chan { cur: g, target: g, bus: 0 };
        let g = pan_gains(1.0, 0.0, false);
        chans[PREVIEW_KYAI_SLOT as usize] = Chan { cur: g, target: g, bus: 0 };
        let mut eng = Engine {
            sr,
            samples,
            clock,
            cmd_rx: None,
            ev_tx: None,
            garbage_tx: None,
            schedule: Arc::new(Schedule::empty()),
            next_idx: 0,
            playing: false,
            looping: false,
            pos: 0.0,
            bpm: 100.0,
            preroll: 0,
            metronome: cfg.metronome,
            trainer: None,
            wraps: 0,
            seq: None,
            seq_pos: 0,
            seq_idx: 0,
            voices: vec![Voice::OFF; MAX_VOICES].into_boxed_slice(),
            age: 0,
            rng: Rng::new(cfg.seed),
            strips: vec![Strip { chain: FilterChain::default(), state: [BiquadState::ZERO; MAX_BANDS], tail: 0, dirty: false }; MAX_STRIPS].into_boxed_slice(),
            strip_buf: vec![0.0; MAX_STRIPS * BLOCK].into_boxed_slice(),
            chans,
            buses: [Bus { cur: 1.0, target: 1.0 }; MAX_BUSES],
            bus_buf: vec![0.0; MAX_BUSES * BLOCK].into_boxed_slice(),
            bus_dirty: [false; MAX_BUSES],
            master_l: [0.0; BLOCK],
            master_r: [0.0; BLOCK],
            master_mode: cfg.master,
            comp: Compressor::new(&CompressorParams::MASTER, sr),
            out_gain: 1.0,
            out_gain_target: 1.0,
            fue: Fue::new(sr),
            frames: 0,
            lead: (HUMANIZE_LEAD * sr).round() as u32,
            strip_tail: (0.2 * sr) as u32,
            click_att: ((0.001 * sr) as u32).max(1),
            click_rel: (0.03 * sr) as u32,
            click_k1: (-1.0 / (tc(0.05) * sr)).exp() as f32,
            click_k2: (-1.0 / (tc(0.02) * sr)).exp() as f32,
            click_end: (0.08 * sr) as u32,
            late_reported: 0,
        };
        eng.publish_fixed_delay();
        eng
    }

    pub(crate) fn attach(&mut self, cmd_rx: rtrb::Consumer<Command>, ev_tx: rtrb::Producer<EngineEvent>, garbage_tx: rtrb::Producer<Garbage>) {
        self.cmd_rx = Some(cmd_rx);
        self.ev_tx = Some(ev_tx);
        self.garbage_tx = Some(garbage_tx);
    }

    pub fn sample_rate(&self) -> u32 {
        self.sr as u32
    }

    pub fn clock(&self) -> &Arc<Clock> {
        &self.clock
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn pos_tick(&self) -> f64 {
        self.pos
    }

    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    /// frames de saída já processados
    pub fn frames(&self) -> u64 {
        self.frames
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.on).count()
    }

    /// atraso (frames) entre o evento ser mixado e sair do master
    fn master_delay(&self) -> u32 {
        if self.master_mode == MasterMode::Live { self.comp.latency_frames() as u32 } else { 0 }
    }

    fn publish_fixed_delay(&mut self) {
        let frames = (self.lead + self.master_delay()) as f64;
        self.clock.fixed_delay_ns.store((frames / self.sr * 1e9) as u64, Ordering::Relaxed);
    }

    #[inline]
    fn emit(&mut self, ev: EngineEvent) {
        if let Some(tx) = self.ev_tx.as_mut() {
            let _ = tx.push(ev);
        }
    }

    // ───────────── comandos ─────────────

    /// Esvazia a fila de comandos (chame no início de cada callback).
    pub fn drain_commands(&mut self) {
        let Some(mut rx) = self.cmd_rx.take() else { return };
        // só aceita comando se houver onde devolver o lixo que ele pode gerar
        while self.garbage_tx.as_ref().is_none_or(|g| g.slots() >= 2) {
            match rx.pop() {
                Ok(cmd) => self.handle(cmd),
                Err(_) => break,
            }
        }
        self.cmd_rx = Some(rx);
    }

    fn retire(&mut self, g: Garbage) {
        match self.garbage_tx.as_mut() {
            Some(tx) => {
                if let Err(rtrb::PushError::Full(g)) = tx.push(g) {
                    // nunca deveria acontecer (drain_commands confere o espaço);
                    // vazar é melhor que desalocar dentro do callback
                    std::mem::forget(g);
                }
            }
            // engine offline: não há thread de tempo real a proteger
            None => drop(g),
        }
    }

    fn reposition(&mut self) {
        let pos = self.pos;
        self.next_idx = self.schedule.events.partition_point(|e| e.tick < pos);
    }

    /// Aplica um comando (é o que `drain_commands` chama; use direto no engine offline).
    pub fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::SetSchedule(s) => {
                for (slot, def) in s.channels.iter().enumerate().take(MAX_CHANNELS - 2) {
                    if !def.is_used() {
                        continue;
                    }
                    let fresh = !self.schedule.channels.get(slot).is_some_and(|c| c.is_used());
                    let ch = &mut self.chans[slot];
                    ch.target = pan_gains(def.gain, def.pan, def.mute);
                    ch.bus = def.bus.min(MAX_BUSES as u16 - 1);
                    if fresh {
                        ch.cur = ch.target;
                    }
                    self.strips[slot * 2].chain = def.chains[0];
                    self.strips[slot * 2 + 1].chain = def.chains[1];
                }
                for (slot, def) in s.buses.iter().enumerate().take(MAX_BUSES).skip(1) {
                    self.buses[slot].target = if def.mute { 0.0 } else { def.gain };
                }
                let old = std::mem::replace(&mut self.schedule, s);
                self.retire(Garbage::Schedule(old));
                let len = self.schedule.len_ticks;
                if self.playing && len > 0.0 && self.pos >= len {
                    self.pos %= len;
                }
                self.reposition();
            }
            Command::Play(p) => {
                self.bpm = p.bpm.clamp(10.0, 999.0);
                self.looping = p.looping;
                self.trainer = p.trainer;
                self.wraps = 0;
                self.pos = p.from_tick.clamp(0.0, self.schedule.len_ticks.max(0.0));
                self.reposition();
                self.preroll = 0;
                if self.schedule.len_ticks <= 0.0 {
                    self.playing = false;
                    let frame = self.frames;
                    self.emit(EngineEvent::Ended { frame });
                    return;
                }
                self.playing = true;
                if p.count_in {
                    // 1 compasso (4 tempos) de cliques antes de começar
                    let spb = 60.0 / self.bpm as f64 * self.sr;
                    for i in 0..4 {
                        self.trigger_click(i == 0, self.lead + (i as f64 * spb) as u32);
                    }
                    self.preroll = (4.0 * spb) as u64;
                }
            }
            Command::Stop => {
                self.playing = false;
                self.pos = 0.0;
                self.next_idx = 0;
                self.preroll = 0;
                // cliques do count-in que ainda não soaram
                for v in self.voices.iter_mut() {
                    if v.on && v.kind == VKind::Click && v.delay > 0 {
                        v.on = false;
                    }
                }
            }
            Command::Pause => self.playing = false,
            Command::Resume => self.playing = self.schedule.len_ticks > 0.0,
            Command::Seek(t) => {
                self.pos = t.clamp(0.0, self.schedule.len_ticks.max(0.0));
                self.reposition();
            }
            Command::SetBpm(b) => self.bpm = b.clamp(10.0, 999.0),
            Command::SetLoop(l) => self.looping = l,
            Command::SetMetronome(m) => self.metronome = m,
            Command::SetTrainer(t) => self.trainer = t,
            Command::SetChannelMix { slot, gain, pan, mute } => {
                if let Some(ch) = self.chans.get_mut(slot as usize) {
                    ch.target = pan_gains(gain, pan, mute);
                }
            }
            Command::SetChannelBus { slot, bus } => {
                if let Some(ch) = self.chans.get_mut(slot as usize) {
                    ch.bus = bus.min(MAX_BUSES as u16 - 1);
                }
            }
            Command::SetBusMix { slot, gain, mute } => {
                if let Some(b) = self.buses.get_mut(slot as usize) {
                    b.target = if mute { 0.0 } else { gain };
                }
            }
            Command::SetStrip { strip, chain } => {
                if let Some(s) = self.strips.get_mut(strip as usize) {
                    s.chain = chain;
                }
            }
            Command::SetMasterGain(g) => self.out_gain_target = g.max(0.0),
            Command::SetMasterMode(m) => {
                self.master_mode = m;
                self.publish_fixed_delay();
            }
            Command::Trigger { kind, delay } => {
                let d = (delay.max(0.0) as f64 * self.sr) as u32;
                self.fire(kind, d, false);
            }
            Command::PlaySeq(seq) => {
                self.seq_pos = -(seq.start_delay as i64);
                self.seq_idx = 0;
                if let Some(old) = self.seq.replace(seq) {
                    self.retire(Garbage::Seq(old));
                }
            }
            Command::StopSeq => {
                if let Some(old) = self.seq.take() {
                    self.retire(Garbage::Seq(old));
                }
                // mata a cauda: o que já soa no canal de preview some em 10 ms
                let fade = (0.01 * self.sr) as u32;
                for v in self.voices.iter_mut() {
                    if v.on && v.strip / 2 == PREVIEW_SLOT {
                        if v.delay > 0 { v.on = false } else { v.choke(0, fade) }
                    }
                }
            }
            Command::PlayFue => self.fue.start(),
            Command::Panic => {
                let fade = (0.03 * self.sr) as u32;
                for v in self.voices.iter_mut() {
                    if v.on {
                        if v.delay > 0 { v.on = false } else { v.choke(0, fade) }
                    }
                }
                self.fue.stop();
            }
            Command::ResetCounters => {
                self.clock.late_callbacks.store(0, Ordering::Relaxed);
                self.clock.max_process_ns.store(0, Ordering::Relaxed);
                self.clock.callbacks.store(0, Ordering::Relaxed);
                self.late_reported = 0;
            }
        }
    }

    // ───────────── disparo de vozes ─────────────

    fn alloc(&mut self) -> usize {
        let mut idx = 0;
        let mut oldest = u64::MAX;
        for (i, v) in self.voices.iter().enumerate() {
            if !v.on {
                idx = i;
                break;
            }
            if v.age < oldest {
                oldest = v.age;
                idx = i;
            }
        }
        self.age += 1;
        self.voices[idx] = Voice { on: true, age: self.age, ..Voice::OFF };
        idx
    }

    /// Dispara um evento `offset` frames à frente (mais a antecedência fixa).
    fn fire(&mut self, kind: EventKind, offset: u32, from_transport: bool) {
        let at = self.lead + offset;
        match kind {
            EventKind::Hit(h) => {
                self.trigger_hit(&h, at);
                if h.notify && from_transport {
                    let frame = self.frames + at as u64 + self.master_delay() as u64;
                    self.emit(EngineEvent::HitFired { track: h.track, cell: h.cell, hit: h.hit, frame });
                }
            }
            EventKind::Grain(g) => self.trigger_grain(&g, at),
            EventKind::Noise(n) => self.trigger_noise(&n, at),
            EventKind::Click { accent } => {
                if self.metronome || !from_transport {
                    self.trigger_click(accent, at);
                }
            }
        }
    }

    fn trigger_hit(&mut self, h: &HitEv, at: u32) {
        let Some(sample) = self.samples.samples.get(h.sample as usize) else { return };
        let (len, ratio) = (sample.data.len(), sample.sr as f64 / self.sr);
        if len < 2 || h.strip as usize >= MAX_STRIPS {
            return;
        }
        // humanização: ganho ±12 %, pitch ±1 % (~17 cents), micro-timing ±6 ms
        let (jg, jr, jt) = if h.humanize {
            (self.rng.jitter(HUMANIZE_GAIN), self.rng.jitter(HUMANIZE_RATE), self.rng.bipolar() as f64 * HUMANIZE_TIME * self.sr)
        } else {
            (1.0, 1.0, 0.0)
        };
        // choke: couro abafa couro e borda anteriores do mesmo canal (a pele
        // parou de vibrar); borda só abafa borda
        let fade = ((h.choke as f64 * self.sr) as u32).max(1);
        let channel = h.strip / 2;
        for v in self.voices.iter_mut() {
            if v.on && v.drum && v.strip / 2 == channel && (!h.rim || v.rim) && v.frames_left() > at as u64 + fade as u64 {
                v.choke(at, fade);
            }
        }
        let i = self.alloc();
        let sr = self.sr;
        let v = &mut self.voices[i];
        v.kind = VKind::Sample;
        v.drum = true;
        v.delay = (at as f64 + jt).max(0.0) as u32;
        v.strip = h.strip;
        v.rim = h.rim;
        v.sample = h.sample;
        v.inc = (h.rate * jr) as f64 * ratio;
        v.src_end = len as f64;
        v.gain = h.gain * jg;
        if h.fade > 0.0 {
            // cauda encurtada (decay/TAIL): corta por rampa de ganho
            v.dur = ((h.dur as f64 * sr) as u32).max(1);
            v.rel_start = v.dur.saturating_sub((h.fade as f64 * sr) as u32);
        }
    }

    fn trigger_grain(&mut self, g: &GrainEv, at: u32) {
        let Some(sample) = self.samples.samples.get(g.sample as usize) else { return };
        let (len, ssr) = (sample.data.len() as f64, sample.sr as f64);
        if g.strip as usize >= MAX_STRIPS {
            return;
        }
        let start = g.start as f64 * ssr;
        if start >= len - 1.0 {
            return;
        }
        let i = self.alloc();
        let sr = self.sr;
        let v = &mut self.voices[i];
        v.kind = VKind::Sample;
        v.delay = at + (g.delay.max(0.0) as f64 * sr) as u32;
        v.strip = g.strip;
        v.sample = g.sample;
        v.pos = start;
        v.inc = g.rate as f64 * ssr / sr;
        v.src_end = (start + g.src_len as f64 * ssr).min(len);
        v.gain = g.gain;
        v.dur = ((g.dur as f64 * sr) as u32).max(2);
        v.att = ((g.att as f64 * sr) as u32).min(v.dur / 2);
        v.rel_start = v.dur.saturating_sub((g.rel as f64 * sr) as u32).max(v.att);
    }

    fn trigger_noise(&mut self, n: &NoiseEv, at: u32) {
        if n.strip as usize >= MAX_STRIPS {
            return;
        }
        let i = self.alloc();
        let sr = self.sr;
        let v = &mut self.voices[i];
        v.kind = VKind::Noise;
        v.delay = at + (n.delay.max(0.0) as f64 * sr) as u32;
        v.strip = n.strip;
        v.gain = n.gain;
        v.filter = n.filter;
        v.noise = n.seed | 1;
        v.dur = ((n.dur as f64 * sr) as u32).max(2);
        v.att = ((n.att as f64 * sr) as u32).clamp(1, v.dur - 1);
        v.rel_start = v.att;
    }

    fn trigger_click(&mut self, accent: bool, at: u32) {
        let i = self.alloc();
        let v = &mut self.voices[i];
        v.kind = VKind::Click;
        v.delay = at;
        // C6 no acento, G5 nos demais; velocity 1 / 0.6; volume −8 dB
        v.freq = if accent { 1_046.502_3 } else { 783.990_9 };
        v.gain = if accent { 1.0 } else { 0.6 };
        v.env = 0.0;
        v.dur = self.click_end;
    }

    // ───────────── transport ─────────────

    /// Avança o transport até `n` frames, disparando os eventos do trecho.
    /// Devolve quantos frames avançou (encurta o bloco na volta do loop/fim).
    fn advance(&mut self, n: usize) -> usize {
        if self.preroll > 0 {
            let k = (self.preroll as usize).min(n);
            self.preroll -= k as u64;
            return k;
        }
        let len = self.schedule.len_ticks;
        if len <= 0.0 {
            // schedule esvaziou durante o playback
            self.playing = false;
            self.pos = 0.0;
            let frame = self.frames;
            self.emit(EngineEvent::Ended { frame });
            return n;
        }
        let tpf = self.bpm as f64 / 60.0 * PPQ / self.sr;
        let mut n = n;
        let mut end = self.pos + n as f64 * tpf;
        // tolerância de 1 milionésimo de frame: o acúmulo de ponto flutuante não
        // pode atrasar a volta do loop em 1 frame
        let eps = tpf * 1e-6;
        if end + eps >= len {
            n = ((((len - self.pos) / tpf - 1e-6).ceil().max(1.0)) as usize).min(n);
            end = self.pos + n as f64 * tpf;
            if end + eps >= len {
                end = end.max(len);
            }
        }
        while let Some(ev) = self.schedule.events.get(self.next_idx).copied() {
            if ev.tick >= end {
                break;
            }
            let offset = (((ev.tick - self.pos) / tpf).max(0.0) as u32).min(n as u32 - 1);
            self.fire(ev.kind, offset, true);
            self.next_idx += 1;
        }
        self.pos = end;
        if self.pos >= len {
            let frame = self.frames + n as u64 + (self.lead + self.master_delay()) as u64;
            if self.looping {
                self.pos = (self.pos - len).max(0.0);
                self.next_idx = 0;
                self.wraps += 1;
                let wraps = self.wraps;
                self.emit(EngineEvent::LoopWrapped { wraps, frame });
                // treino: a cada volta do loop, sobe o BPM até o tempo base
                if let Some(t) = self.trainer {
                    let next = (self.bpm + t.base_bpm * t.step).round().min(t.base_bpm);
                    if next != self.bpm {
                        self.bpm = next;
                        self.emit(EngineEvent::BpmChanged { bpm: next });
                    }
                }
            } else {
                self.playing = false;
                self.pos = 0.0;
                self.next_idx = 0;
                self.emit(EngineEvent::Ended { frame });
            }
        }
        n
    }

    fn advance_seq(&mut self, n: usize) {
        let Some(seq) = self.seq.take() else { return };
        let mut remaining = n as i64;
        let mut base = 0i64;
        let len = seq.len_frames as i64;
        let mut done = false;
        loop {
            while let Some(&(frame, kind)) = seq.events.get(self.seq_idx) {
                if frame as i64 >= self.seq_pos + remaining {
                    break;
                }
                let offset = (base + (frame as i64 - self.seq_pos).max(0)) as u32;
                self.fire(kind, offset, false);
                self.seq_idx += 1;
            }
            if len > 0 && self.seq_pos + remaining >= len {
                if !seq.looping {
                    done = true;
                    break;
                }
                let used = len - self.seq_pos;
                base += used;
                remaining -= used;
                self.seq_pos = 0;
                self.seq_idx = 0;
            } else {
                self.seq_pos += remaining;
                break;
            }
        }
        if done || len <= 0 {
            self.retire(Garbage::Seq(seq));
        } else {
            self.seq = Some(seq);
        }
    }

    // ───────────── áudio ─────────────

    /// Publica o relógio (posição no início do bloco + instante). Chame antes
    /// de `process` em cada callback; offline não precisa.
    pub fn publish_clock(&mut self, stamp_ns: u64) {
        let c = &self.clock;
        c.seq.fetch_add(1, Ordering::AcqRel);
        c.pos_bits.store(self.pos.to_bits(), Ordering::Relaxed);
        c.stamp_ns.store(stamp_ns, Ordering::Relaxed);
        c.frames.store(self.frames, Ordering::Relaxed);
        c.len_bits.store(self.schedule.len_ticks.to_bits(), Ordering::Relaxed);
        c.preroll.store(self.preroll, Ordering::Relaxed);
        c.bpm_bits.store(self.bpm.to_bits(), Ordering::Relaxed);
        c.wraps.store(self.wraps, Ordering::Relaxed);
        c.playing.store(self.playing, Ordering::Relaxed);
        c.looping.store(self.looping, Ordering::Relaxed);
        c.seq.fetch_add(1, Ordering::AcqRel);
        // xruns contados pelo backend viram evento
        let late = c.late_callbacks.load(Ordering::Relaxed);
        if late > self.late_reported {
            let count = (late - self.late_reported) as u32;
            self.late_reported = late;
            self.emit(EngineEvent::Xrun { count });
        }
    }

    /// Gera `out.len() / channels` frames intercalados (1 canal = mono; 2+ =
    /// estéreo nos dois primeiros, silêncio nos demais).
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let total = out.len() / channels;
        let mut done = 0;
        while done < total {
            let mut n = (total - done).min(BLOCK);
            if self.playing {
                n = self.advance(n);
            }
            if self.seq.is_some() {
                self.advance_seq(n);
            }
            self.render_block(n, &mut out[done * channels..(done + n) * channels], channels);
            done += n;
            self.frames += n as u64;
        }
    }

    fn render_block(&mut self, n: usize, out: &mut [f32], channels: usize) {
        let Engine {
            voices,
            samples,
            strips,
            strip_buf,
            chans,
            buses,
            bus_buf,
            bus_dirty,
            master_l,
            master_r,
            comp,
            fue,
            click_att,
            click_rel,
            click_k1,
            click_k2,
            sr,
            clock,
            strip_tail,
            master_mode,
            out_gain,
            out_gain_target,
            ..
        } = self;
        let sr = *sr as f32;
        master_l[..n].fill(0.0);
        master_r[..n].fill(0.0);
        let mut active = 0u32;

        // 1) vozes → strips (mono); cliques → master
        for v in voices.iter_mut() {
            if !v.on {
                continue;
            }
            active += 1;
            let f0 = (v.delay as usize).min(n);
            v.delay -= f0 as u32;
            if f0 > 0 && v.choke_in != NONE {
                // o abafamento agendado corre mesmo com a voz ainda em espera
                let wait = (v.choke_in as usize).min(f0);
                v.choke_in -= wait as u32;
                v.choke_pos += (f0 - wait) as u32;
                if v.choke_pos >= v.choke_len {
                    v.on = false;
                    continue;
                }
            }
            if f0 == n {
                continue;
            }
            if v.kind == VKind::Click {
                for f in f0..n {
                    if v.t < *click_att {
                        v.env = v.gain * (v.t + 1) as f32 / *click_att as f32;
                    } else if v.t < *click_rel {
                        v.env *= *click_k1;
                    } else {
                        v.env *= *click_k2;
                    }
                    let p = v.phase;
                    let tri = if p < 0.25 { 4.0 * p } else if p < 0.75 { 2.0 - 4.0 * p } else { 4.0 * p - 4.0 };
                    let s = tri * v.env * 0.398_107_2;
                    master_l[f] += s;
                    master_r[f] += s;
                    v.phase = (v.phase + v.freq / sr).fract();
                    v.t += 1;
                    if v.t >= v.dur {
                        v.on = false;
                        break;
                    }
                }
                continue;
            }
            let s = v.strip as usize;
            let buf = &mut strip_buf[s * BLOCK..s * BLOCK + n];
            if !strips[s].dirty {
                buf.fill(0.0);
                strips[s].dirty = true;
            }
            match v.kind {
                VKind::Sample => {
                    let data = &samples.samples[v.sample as usize].data[..];
                    let last = data.len() - 1;
                    for x in &mut buf[f0..] {
                        if v.pos >= v.src_end || v.t >= v.dur {
                            v.on = false;
                            break;
                        }
                        let i = v.pos as usize;
                        if i > last {
                            v.on = false;
                            break;
                        }
                        // interpolação cúbica (Catmull-Rom) com bordas repetidas
                        let fr = (v.pos - i as f64) as f32;
                        let (x0, x1, x2, x3) = (data[i.saturating_sub(1)], data[i], data[(i + 1).min(last)], data[(i + 2).min(last)]);
                        let c1 = 0.5 * (x2 - x0);
                        let c2 = x0 - 2.5 * x1 + 2.0 * x2 - 0.5 * x3;
                        let c3 = 0.5 * (x3 - x0) + 1.5 * (x1 - x2);
                        let y = ((c3 * fr + c2) * fr + c1) * fr + x1;
                        let (e, alive) = v.envelope();
                        *x += y * v.gain * e;
                        v.pos += v.inc;
                        v.t += 1;
                        if !alive {
                            v.on = false;
                            break;
                        }
                    }
                }
                VKind::Noise => {
                    for x in &mut buf[f0..] {
                        if v.t >= v.dur {
                            v.on = false;
                            break;
                        }
                        let mut r = v.noise;
                        r ^= r << 13;
                        r ^= r >> 17;
                        r ^= r << 5;
                        v.noise = r;
                        let w = (r >> 8) as f32 * (2.0 / 16_777_216.0) - 1.0;
                        let y = v.fstate.tick(&v.filter, w as f64) as f32;
                        let (e, alive) = v.envelope();
                        *x += y * v.gain * e;
                        v.t += 1;
                        if !alive {
                            v.on = false;
                            break;
                        }
                    }
                }
                VKind::Click => {}
            }
        }
        clock.active_voices.store(active, Ordering::Relaxed);

        // 2) strips: filtros → canal (volume/pan/mudo, rampa de 1 bloco) → barramento ou master
        bus_dirty.fill(false);
        let inv_n = 1.0 / n as f32;
        for (s, st) in strips.iter_mut().enumerate() {
            if !st.dirty && st.tail == 0 {
                continue;
            }
            let buf = &mut strip_buf[s * BLOCK..s * BLOCK + n];
            if !st.dirty {
                buf.fill(0.0);
            }
            for b in 0..st.chain.n as usize {
                let c = st.chain.bands[b];
                let state = &mut st.state[b];
                for x in buf.iter_mut() {
                    *x = state.tick(&c, *x as f64) as f32;
                }
            }
            if st.dirty {
                st.tail = *strip_tail;
                st.dirty = false;
            } else {
                st.tail = st.tail.saturating_sub(n as u32);
                if st.tail == 0 {
                    st.state = [BiquadState::ZERO; MAX_BANDS];
                }
            }
            let ch = &chans[s / 2];
            if ch.cur == [0.0, 0.0] && ch.target == [0.0, 0.0] {
                continue;
            }
            if ch.bus == 0 {
                let (gl, gr) = (ch.cur[0], ch.cur[1]);
                let (dl, dr) = ((ch.target[0] - gl) * inv_n, (ch.target[1] - gr) * inv_n);
                for (f, x) in buf.iter().enumerate() {
                    master_l[f] += x * (gl + dl * f as f32);
                    master_r[f] += x * (gr + dr * f as f32);
                }
            } else {
                // o barramento (Tone.Channel) soma a entrada em mono
                let b = ch.bus as usize;
                let bb = &mut bus_buf[b * BLOCK..b * BLOCK + n];
                if !bus_dirty[b] {
                    bb.fill(0.0);
                    bus_dirty[b] = true;
                }
                let g = (ch.cur[0] + ch.cur[1]) * 0.5;
                let d = ((ch.target[0] + ch.target[1]) * 0.5 - g) * inv_n;
                for (f, x) in buf.iter().enumerate() {
                    bb[f] += x * (g + d * f as f32);
                }
            }
        }
        for ch in chans.iter_mut() {
            ch.cur = ch.target;
        }

        // 3) barramentos das pessoas → master (pan central: −3 dB por lado)
        for (b, bus) in buses.iter_mut().enumerate().skip(1) {
            if bus_dirty[b] && (bus.cur != 0.0 || bus.target != 0.0) {
                let bb = &bus_buf[b * BLOCK..b * BLOCK + n];
                let g = bus.cur * std::f32::consts::FRAC_1_SQRT_2;
                let d = (bus.target * std::f32::consts::FRAC_1_SQRT_2 - g) * inv_n;
                for (f, x) in bb.iter().enumerate() {
                    let y = x * (g + d * f as f32);
                    master_l[f] += y;
                    master_r[f] += y;
                }
            }
            bus.cur = bus.target;
        }

        // 4) easter egg
        fue.render(&mut master_l[..n], &mut master_r[..n]);

        // 5) master: headroom (−6 dB) → compressor → soft clip; depois o volume de saída
        let live = *master_mode == MasterMode::Live;
        let g0 = *out_gain;
        let dg = (*out_gain_target - g0) * inv_n;
        *out_gain = *out_gain_target;
        for f in 0..n {
            let (mut l, mut r) = (master_l[f] * 0.5, master_r[f] * 0.5);
            if !l.is_finite() || !r.is_finite() {
                (l, r) = (0.0, 0.0);
            }
            if live {
                (l, r) = comp.tick(l, r);
                l = soft_clip(l);
                r = soft_clip(r);
            }
            let g = g0 + dg * f as f32;
            (l, r) = (l * g, r * g);
            let o = &mut out[f * channels..(f + 1) * channels];
            if channels == 1 {
                o[0] = (l + r) * 0.5;
            } else {
                o[0] = l;
                o[1] = r;
                o[2..].fill(0.0);
            }
        }
    }
}

/// Evento avulso com tick (útil em testes e para montar `Schedule`s à mão).
pub fn event(tick: f64, kind: EventKind) -> Event {
    Event { tick, kind }
}
