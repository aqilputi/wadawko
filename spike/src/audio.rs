//! Motor de áudio em tempo real (porte do essencial de src/lib/audio.ts).
//!
//! Regras dentro do callback do cpal: sem alocação, sem locks, sem log.
//! A GUI compila um `Schedule` (golpes já com sample, camada, round robin,
//! ganho, pan, cauda, choke e EQ resolvidos) e manda por ring buffer SPSC;
//! schedules velhos voltam pela fila de "lixo" e são liberados na GUI.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

use anyhow::{Context as _, Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub const PPQ: u64 = 960;
const MAX_VOICES: usize = 64;
pub const MAX_BANDS: usize = 5;

// ───────────────────────────── EQ (biquads RBJ) ─────────────────────────────

#[derive(Clone, Copy, Default, Debug)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum BandKind {
    Highpass,
    Lowpass,
    Bandpass,
    Lowshelf,
    Peaking,
}

impl Biquad {
    /// Fórmulas do Audio EQ Cookbook (as mesmas do BiquadFilterNode).
    pub fn new(kind: BandKind, freq: f32, gain_db: f32, q: f32, sr: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * (freq / sr).min(0.49);
        let (sin, cos) = w0.sin_cos();
        let q = q.max(0.05);
        let alpha = sin / (2.0 * q);
        let a = 10f32.powf(gain_db / 40.0);
        let (b0, b1, b2, a0, a1, a2) = match kind {
            BandKind::Highpass => ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            BandKind::Lowpass => ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            BandKind::Bandpass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            BandKind::Peaking => (1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a),
            BandKind::Lowshelf => {
                // shelf com S = 1 (o lowshelf do WebAudio ignora Q)
                let sa = 2.0 * a.sqrt() * (sin / 2.0 * 2f32.sqrt());
                (
                    a * ((a + 1.0) - (a - 1.0) * cos + sa),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - sa),
                    (a + 1.0) + (a - 1.0) * cos + sa,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - sa,
                )
            }
        };
        Biquad { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 }
    }
}

/// Cadeia de até MAX_BANDS biquads (por instrumento × zona).
#[derive(Clone, Copy, Default, Debug)]
pub struct EqSet {
    pub bands: [Biquad; MAX_BANDS],
    pub n: usize,
}

#[derive(Clone, Copy, Default)]
struct BqState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

// ───────────────────────────── eventos ─────────────────────────────

#[derive(Clone, Copy, Debug)]
pub enum EvKind {
    /// Golpe de tambor já resolvido.
    Sample {
        sample: u16,
        /// playback rate relativo ao sample (afinação × jitter)
        rate: f32,
        gain_l: f32,
        gain_r: f32,
        /// duração máxima (s) e fade final (s)
        tail: f32,
        fade: f32,
        /// choke: linha (grupo), zona e duração do abafamento (s)
        group: u16,
        rim: bool,
        choke: f32,
        eq: u16,
    },
    /// Clique de metrônomo sintético.
    Click { accent: bool },
}

#[derive(Clone, Copy, Debug)]
pub struct Ev {
    pub tick: u64,
    pub kind: EvKind,
}

pub struct Schedule {
    pub events: Vec<Ev>,
    pub len_ticks: u64,
    pub eq: Vec<EqSet>,
}

pub enum Cmd {
    Play,
    Stop,
    Seek(u64),
    SetBpm(f32),
    SetSchedule(Arc<Schedule>),
    SetGain(f32),
    SetLoop(bool),
    ResetCounters,
}

/// Estado publicado pelo thread de áudio (lock-free).
pub struct Shared {
    pub pos_tick: AtomicU64,
    pub stamp_ns: AtomicU64,
    pub latency_ns: AtomicU64,
    pub bpm_bits: AtomicU32,
    pub playing: AtomicBool,
    pub late_callbacks: AtomicU64,
    pub stream_errors: AtomicU64,
    pub callbacks: AtomicU64,
    pub buffer_frames: AtomicU32,
    pub max_process_ns: AtomicU64,
    pub sample_rate: u32,
    pub epoch: Instant,
}

impl Shared {
    pub fn bpm(&self) -> f32 {
        f32::from_bits(self.bpm_bits.load(Ordering::Relaxed))
    }

    /// Posição audível agora (ticks): extrapola do último callback e desconta a latência de saída.
    pub fn audible_tick(&self, len_ticks: u64, extra_offset_ms: f32) -> f64 {
        let pos = f64::from_bits(self.pos_tick.load(Ordering::Acquire));
        if !self.playing.load(Ordering::Relaxed) {
            return pos;
        }
        let stamp = self.stamp_ns.load(Ordering::Acquire);
        let now = self.epoch.elapsed().as_nanos() as u64;
        let buf_ns = self.buffer_frames.load(Ordering::Relaxed) as f64 * 1e9 / self.sample_rate as f64;
        let elapsed = (now.saturating_sub(stamp) as f64).min(buf_ns * 2.0);
        let lat = self.latency_ns.load(Ordering::Relaxed) as f64 + extra_offset_ms as f64 * 1e6;
        let secs = (elapsed - lat) / 1e9;
        let t = pos + secs * self.bpm() as f64 / 60.0 * PPQ as f64;
        let len = len_ticks.max(1) as f64;
        ((t % len) + len) % len
    }
}

// ───────────────────────────── samples ─────────────────────────────

pub struct SampleData {
    /// estéreo intercalado
    pub frames: Vec<[f32; 2]>,
    pub sr: u32,
}

pub fn load_sample(path: &Path) -> Result<SampleData> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(path).with_context(|| format!("abrir {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("ogg");
    let probed = symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())?;
    let mut format = probed.format;
    let track = format.default_track().ok_or_else(|| anyhow!("sem trilha"))?;
    let track_id = track.id;
    let sr = track.codec_params.sample_rate.unwrap_or(48_000);
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut frames = Vec::new();
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = decoder.decode(&packet)?;
        let spec = *decoded.spec();
        let ch = spec.channels.count();
        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buf.copy_interleaved_ref(decoded);
        for fr in buf.samples().chunks(ch) {
            frames.push(if ch >= 2 { [fr[0], fr[1]] } else { [fr[0], fr[0]] });
        }
    }
    Ok(SampleData { frames, sr })
}

// ───────────────────────────── motor ─────────────────────────────

#[derive(Clone, Copy, Default)]
struct Voice {
    active: bool,
    age: u64,
    sample: u16,
    pos: f64,
    rate: f64,
    gain_l: f32,
    gain_r: f32,
    /// frames restantes até o fim (cauda máxima)
    left: u32,
    fade_frames: u32,
    /// fade forçado (choke/stop): frames restantes / total
    choke_left: u32,
    choke_len: u32,
    group: u16,
    rim: bool,
    eq: u16,
    bq: [[BqState; 2]; MAX_BANDS],
    click: bool,
    click_phase: f32,
    click_freq: f32,
}

pub struct Engine {
    rx: Option<rtrb::Consumer<Cmd>>,
    garbage: Option<rtrb::Producer<Arc<Schedule>>>,
    shared: Arc<Shared>,
    schedule: Arc<Schedule>,
    bank: Arc<Vec<SampleData>>,
    voices: [Voice; MAX_VOICES],
    next_idx: usize,
    pos_tick: f64,
    bpm: f32,
    playing: bool,
    looping: bool,
    gain: f32,
    sr: f32,
    channels: usize,
    age: u64,
    last_cb: Option<Instant>,
}

impl Engine {
    fn new(shared: Arc<Shared>, schedule: Arc<Schedule>, bank: Arc<Vec<SampleData>>, bpm: f32, sr: u32, channels: usize) -> Self {
        Engine {
            rx: None,
            garbage: None,
            shared,
            schedule,
            bank,
            voices: [Voice::default(); MAX_VOICES],
            next_idx: 0,
            pos_tick: 0.0,
            bpm,
            playing: false,
            looping: true,
            gain: 1.0,
            sr: sr as f32,
            channels,
            age: 0,
            last_cb: None,
        }
    }

    fn seek(&mut self, tick: f64) {
        self.pos_tick = tick;
        let t = tick.ceil() as u64;
        self.next_idx = self.schedule.events.partition_point(|e| e.tick < t);
    }

    fn fade_all(&mut self, secs: f32) {
        let fade = ((self.sr * secs) as u32).max(1);
        for v in &mut self.voices {
            if v.active && !v.click && (v.choke_len == 0 || v.choke_left > fade) {
                v.choke_left = fade;
                v.choke_len = fade;
            }
        }
    }

    fn handle_cmds(&mut self) {
        let Some(mut rx) = self.rx.take() else { return };
        while let Ok(cmd) = rx.pop() {
            match cmd {
                Cmd::ResetCounters => {
                    self.shared.late_callbacks.store(0, Ordering::Relaxed);
                    self.shared.max_process_ns.store(0, Ordering::Relaxed);
                }
                Cmd::Play => self.playing = true,
                Cmd::Stop => {
                    self.playing = false;
                    self.fade_all(0.03);
                }
                Cmd::Seek(t) => {
                    self.fade_all(0.03);
                    self.seek(t as f64);
                }
                Cmd::SetBpm(b) => self.bpm = b.clamp(20.0, 400.0),
                Cmd::SetGain(g) => self.gain = g,
                Cmd::SetLoop(l) => self.looping = l,
                Cmd::SetSchedule(s) => {
                    let old = std::mem::replace(&mut self.schedule, s);
                    if let Some(g) = self.garbage.as_mut() {
                        let _ = g.push(old);
                    }
                    let p = self.pos_tick;
                    self.seek(p);
                }
            }
        }
        self.rx = Some(rx);
    }

    fn trigger(&mut self, ev: &Ev) {
        // choke: couro abafa couro+borda da mesma linha; borda abafa só borda
        if let EvKind::Sample { group, rim, choke, .. } = ev.kind {
            let fade = ((choke * self.sr) as u32).max(1);
            for v in &mut self.voices {
                if v.active && !v.click && v.group == group && (!rim || v.rim) && (v.choke_len == 0 || v.choke_left > fade) {
                    v.choke_left = fade;
                    v.choke_len = fade;
                }
            }
        }
        // voz livre, senão rouba a mais antiga
        let mut idx = 0;
        let mut oldest = u64::MAX;
        for (i, v) in self.voices.iter().enumerate() {
            if !v.active {
                idx = i;
                break;
            }
            if v.age < oldest {
                oldest = v.age;
                idx = i;
            }
        }
        self.age += 1;
        let sr = self.sr;
        let v = &mut self.voices[idx];
        *v = Voice { active: true, age: self.age, ..Voice::default() };
        match ev.kind {
            EvKind::Sample { sample, rate, gain_l, gain_r, tail, fade, group, rim, eq, .. } => {
                let Some(data) = self.bank.get(sample as usize) else {
                    v.active = false;
                    return;
                };
                v.sample = sample;
                v.rate = rate as f64 * data.sr as f64 / sr as f64;
                v.gain_l = gain_l;
                v.gain_r = gain_r;
                let natural = (data.frames.len() as f64 / v.rate) as u32;
                v.left = natural.min((tail * sr) as u32).max(1);
                // só faz fade se a cauda foi encurtada
                if v.left + ((0.01 * sr) as u32) < natural {
                    v.fade_frames = ((fade * sr) as u32).min(v.left / 2).max(1);
                }
                v.group = group;
                v.rim = rim;
                v.eq = eq;
            }
            EvKind::Click { accent } => {
                v.click = true;
                v.click_freq = if accent { 1760.0 } else { 1320.0 };
                v.left = (sr * 0.03) as u32;
                v.gain_l = if accent { 0.35 } else { 0.22 };
                v.gain_r = v.gain_l;
            }
        }
    }

    pub fn render(&mut self, out: &mut [f32]) {
        let frames = out.len() / self.channels;
        let ticks_per_frame = self.bpm as f64 / 60.0 * PPQ as f64 / self.sr as f64;
        let len = self.schedule.len_ticks.max(1) as f64;
        for f in 0..frames {
            if self.playing {
                loop {
                    let Some(ev) = self.schedule.events.get(self.next_idx).copied() else { break };
                    if (ev.tick as f64) > self.pos_tick {
                        break;
                    }
                    self.trigger(&ev);
                    self.next_idx += 1;
                }
                self.pos_tick += ticks_per_frame;
                if self.pos_tick >= len {
                    self.pos_tick -= len;
                    self.next_idx = 0;
                    if !self.looping {
                        self.playing = false;
                        self.pos_tick = 0.0;
                    }
                }
            }
            let (mut l, mut r) = (0.0f32, 0.0f32);
            let sched = &self.schedule;
            for v in &mut self.voices {
                if !v.active {
                    continue;
                }
                if v.click {
                    let env = v.left as f32 / (self.sr * 0.03);
                    let s = (v.click_phase * std::f32::consts::TAU).sin() * env * env;
                    l += s * v.gain_l;
                    r += s * v.gain_r;
                    v.click_phase = (v.click_phase + v.click_freq / self.sr).fract();
                    v.left = v.left.saturating_sub(1);
                    if v.left == 0 {
                        v.active = false;
                    }
                    continue;
                }
                let data = &self.bank[v.sample as usize].frames;
                let i = v.pos as usize;
                if i + 1 >= data.len() || v.left == 0 {
                    v.active = false;
                    continue;
                }
                let fr = (v.pos - i as f64) as f32;
                let mut x = [data[i][0] + (data[i + 1][0] - data[i][0]) * fr, data[i][1] + (data[i + 1][1] - data[i][1]) * fr];
                if let Some(eq) = sched.eq.get(v.eq as usize) {
                    for b in 0..eq.n {
                        let c = &eq.bands[b];
                        for (ch, xc) in x.iter_mut().enumerate() {
                            let s = &mut v.bq[b][ch];
                            let y = c.b0 * *xc + c.b1 * s.x1 + c.b2 * s.x2 - c.a1 * s.y1 - c.a2 * s.y2;
                            s.x2 = s.x1;
                            s.x1 = *xc;
                            s.y2 = s.y1;
                            s.y1 = y;
                            *xc = y;
                        }
                    }
                }
                let mut env = 1.0;
                if v.fade_frames > 0 && v.left <= v.fade_frames {
                    env *= v.left as f32 / v.fade_frames as f32;
                }
                if v.choke_len > 0 {
                    env *= v.choke_left as f32 / v.choke_len as f32;
                    v.choke_left = v.choke_left.saturating_sub(1);
                    if v.choke_left == 0 {
                        v.active = false;
                    }
                }
                l += x[0] * v.gain_l * env;
                r += x[1] * v.gain_r * env;
                v.pos += v.rate;
                v.left -= 1;
            }
            // master: −6 dB + soft clip (no lugar do compressor+waveshaper do Tone)
            let g = self.gain * 0.5;
            let (l, r) = ((l * g).tanh(), (r * g).tanh());
            if self.channels == 1 {
                out[f] = (l + r) * 0.5;
            } else {
                let base = f * self.channels;
                out[base] = l;
                out[base + 1] = r;
                for c in 2..self.channels {
                    out[base + c] = 0.0;
                }
            }
        }
    }
}

pub struct Audio {
    pub tx: rtrb::Producer<Cmd>,
    pub garbage: rtrb::Consumer<Arc<Schedule>>,
    pub shared: Arc<Shared>,
    _stream: cpal::Stream,
}

impl Audio {
    pub fn collect_garbage(&mut self) {
        while self.garbage.pop().is_ok() {}
    }

    pub fn send(&mut self, cmd: Cmd) {
        let _ = self.tx.push(cmd);
    }
}

fn new_shared(sr: u32, bpm: f32, buffer_frames: u32) -> Arc<Shared> {
    Arc::new(Shared {
        pos_tick: AtomicU64::new(0f64.to_bits()),
        stamp_ns: AtomicU64::new(0),
        latency_ns: AtomicU64::new(0),
        bpm_bits: AtomicU32::new(bpm.to_bits()),
        playing: AtomicBool::new(false),
        late_callbacks: AtomicU64::new(0),
        stream_errors: AtomicU64::new(0),
        callbacks: AtomicU64::new(0),
        buffer_frames: AtomicU32::new(buffer_frames),
        max_process_ns: AtomicU64::new(0),
        sample_rate: sr,
        epoch: Instant::now(),
    })
}

/// Taxa do dispositivo padrão (o EQ é compilado antes de abrir o stream).
pub fn device_rate() -> u32 {
    cpal::default_host()
        .default_output_device()
        .and_then(|d| d.default_output_config().ok())
        .map(|c| c.sample_rate())
        .unwrap_or(48_000)
}

pub fn start(bank: Arc<Vec<SampleData>>, schedule: Arc<Schedule>, bpm: f32, buffer_frames: u32) -> Result<Audio> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or_else(|| anyhow!("sem dispositivo de saída"))?;
    let def = device.default_output_config()?;
    let sr = def.sample_rate();
    let channels = def.channels() as usize;
    let config = cpal::StreamConfig { channels: def.channels(), sample_rate: sr, buffer_size: cpal::BufferSize::Fixed(buffer_frames) };

    let (tx, rx) = rtrb::RingBuffer::new(256);
    let (gtx, grx) = rtrb::RingBuffer::new(64);
    let shared = new_shared(sr, bpm, buffer_frames);
    let mut eng = Engine::new(shared.clone(), schedule, bank, bpm, sr, channels);
    eng.rx = Some(rx);
    eng.garbage = Some(gtx);

    let err_shared = shared.clone();
    let sh = shared.clone();
    let stream = device.build_output_stream::<f32, _, _>(
        config,
        move |out: &mut [f32], info: &cpal::OutputCallbackInfo| {
            let t0 = Instant::now();
            let frames = (out.len() / eng.channels) as u32;
            if let Some(prev) = eng.last_cb {
                let gap = t0.duration_since(prev).as_secs_f64();
                if gap > frames as f64 / eng.sr as f64 * 1.5 && eng.playing {
                    sh.late_callbacks.fetch_add(1, Ordering::Relaxed);
                }
            }
            eng.last_cb = Some(t0);
            eng.handle_cmds();
            sh.pos_tick.store(eng.pos_tick.to_bits(), Ordering::Release);
            sh.stamp_ns.store(t0.duration_since(sh.epoch).as_nanos() as u64, Ordering::Release);
            let ts = info.timestamp();
            sh.latency_ns.store(ts.playback.duration_since(ts.callback).as_nanos() as u64, Ordering::Relaxed);
            sh.bpm_bits.store(eng.bpm.to_bits(), Ordering::Relaxed);
            sh.playing.store(eng.playing, Ordering::Relaxed);
            sh.buffer_frames.store(frames, Ordering::Relaxed);
            sh.callbacks.fetch_add(1, Ordering::Relaxed);
            eng.render(out);
            sh.max_process_ns.fetch_max(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
        },
        move |_err| {
            err_shared.stream_errors.fetch_add(1, Ordering::Relaxed);
        },
        None,
    )?;
    stream.play()?;
    Ok(Audio { tx, garbage: grx, shared, _stream: stream })
}

/// Render offline com o MESMO motor (sem dispositivo): uma passada pela música + 3 s de cauda.
pub fn render_offline(bank: Arc<Vec<SampleData>>, schedule: Arc<Schedule>, bpm: f32, sr: u32) -> Vec<[f32; 2]> {
    let len_ticks = schedule.len_ticks;
    let mut eng = Engine::new(new_shared(sr, bpm, 512), schedule, bank, bpm, sr, 2);
    eng.playing = true;
    eng.looping = false;
    let secs = len_ticks as f64 / PPQ as f64 * 60.0 / bpm as f64 + 3.0;
    let total = (secs * sr as f64) as usize;
    let mut out = Vec::with_capacity(total);
    let mut buf = [0.0f32; 1024];
    while out.len() < total {
        eng.render(&mut buf);
        out.extend(buf.chunks(2).map(|c| [c[0], c[1]]));
    }
    out.truncate(total);
    out
}

pub fn write_wav(path: &Path, sr: u32, frames: &[[f32; 2]]) -> Result<()> {
    use std::io::Write;
    let mut w = std::io::BufWriter::new(std::fs::File::create(path)?);
    let data_len = (frames.len() * 4) as u32;
    w.write_all(b"RIFF")?;
    w.write_all(&(36 + data_len).to_le_bytes())?;
    w.write_all(b"WAVEfmt ")?;
    w.write_all(&16u32.to_le_bytes())?;
    w.write_all(&1u16.to_le_bytes())?;
    w.write_all(&2u16.to_le_bytes())?;
    w.write_all(&sr.to_le_bytes())?;
    w.write_all(&(sr * 4).to_le_bytes())?;
    w.write_all(&4u16.to_le_bytes())?;
    w.write_all(&16u16.to_le_bytes())?;
    w.write_all(b"data")?;
    w.write_all(&data_len.to_le_bytes())?;
    for f in frames {
        for s in f {
            w.write_all(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
        }
    }
    Ok(())
}
