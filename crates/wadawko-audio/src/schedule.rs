//! `Schedule`: a música compilada FORA do callback — porte de `Engine.schedule`
//! e da montagem de `renderOffline` (`audio.ts`). Tudo que depende do modelo
//! (ordem com ritornellos, swing, dinâmica, sfz, mainMul, camada + round robin,
//! cauda, choke, roteamento por linha/dono/pessoa, EQ, kiai, metrônomo) já sai
//! resolvido em eventos ordenados por tick (PPQ 960). O thread de áudio só
//! humaniza (ganho/pitch/tempo), dispara vozes e mixa.

use std::collections::HashMap;

use wadawko_core::kyai::{kyai_events, kyai_lag, kyai_note, kyai_profile, kyai_seed, kyai_voice_or_default};
use wadawko_core::ops::kit::resolve_instrument;
use wadawko_core::ops::measures::bar_beat_starts;
use wadawko_core::ops::people::{cell_owner_of, person_audible};
use wadawko_core::phrase::resolve_cell;
use wadawko_core::timeline::{Region, dynamics_curve, main_mul, play_order, swing_warp};
use wadawko_core::{Dynamic, InstrumentDef, InstrumentId, Song, ToneType, Track, Zone};
use wadawko_samples::{BaseInstrument, FilterKind, RoundRobin, SampleBank, Zone as SZone};

use crate::dsp::{Biquad, FilterChain, FilterType, Rng};
use crate::kyai_voice::kyai_grains;

/// Resolução do transport (ticks por semínima).
pub const PPQ: f64 = 960.0;
/// Ticks de um compasso de 4 tempos (4/4). Cada compasso ocupa `beats` tempos
/// no transport — use [`TimeMap::bar_tick`] para o início de um compasso tocado;
/// esta constante só vale para músicas inteiramente em 4/4.
pub const BAR_TICKS: f64 = 4.0 * PPQ;

/// Máximo de canais (linha, linha×dono, kiai). Os 2 últimos são reservados aos previews.
pub const MAX_CHANNELS: usize = 128;
/// Cada canal tem 2 "strips" (couro, borda), cada um com a sua cadeia de filtros.
pub const MAX_STRIPS: usize = MAX_CHANNELS * 2;
/// Barramentos: 0 = master direto; 1.. = pessoas.
pub const MAX_BUSES: usize = 32;
/// Canal dos previews de instrumento/frase (`previewChannel` do TS, −6 dB → master).
pub const PREVIEW_SLOT: u16 = (MAX_CHANNELS - 1) as u16;
/// Canal reserva do preview de kiai (quando a linha ainda não tem canal compilado).
pub const PREVIEW_KYAI_SLOT: u16 = (MAX_CHANNELS - 2) as u16;
const USER_SLOTS: usize = MAX_CHANNELS - 2;

// ───────────────────────────── eventos ─────────────────────────────

/// Golpe de tambor já resolvido (sample, afinação, ganho sem humanização, cauda).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitEv {
    /// índice do sample no banco (`SampleId`)
    pub sample: u16,
    /// playbackRate (TUNING × pitch da zona), sem o jitter
    pub rate: f32,
    /// ganho linear = TUNING × ganho da zona × dinâmica × `velGain(vel)`, sem o jitter
    pub gain: f32,
    /// duração efetiva (s): natural, encurtada pelo decay e limitada pelo `TAIL`
    pub dur: f32,
    /// rampa final (s); 0 = o sample toca até o fim natural
    pub fade: f32,
    /// duração (s) do abafamento que este golpe aplica nos anteriores
    pub choke: f32,
    /// strip de destino = canal × 2 + zona
    pub strip: u16,
    pub rim: bool,
    /// aplica a humanização (ganho ±12 %, pitch ±1 %, tempo ±6 ms) ao disparar
    pub humanize: bool,
    /// para o `HitFired`: índice da linha, célula-FONTE global e golpe dentro da célula
    pub track: u16,
    pub cell: u32,
    pub hit: u8,
    /// emite `HitFired` (golpes da música sim; previews não)
    pub notify: bool,
}

/// Grão de letra do animalese (kiai).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrainEv {
    pub sample: u16,
    /// início (s) e duração máxima (s) dentro do buffer
    pub start: f32,
    pub src_len: f32,
    pub rate: f32,
    pub gain: f32,
    /// envelope: ataque, release e duração total (s)
    pub att: f32,
    pub rel: f32,
    pub dur: f32,
    /// atraso (s, tempo real) em relação ao tick do evento
    pub delay: f32,
    pub strip: u16,
}

/// Ruído filtrado das consoantes sibilantes do kiai.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoiseEv {
    pub filter: Biquad,
    pub gain: f32,
    pub att: f32,
    pub dur: f32,
    pub delay: f32,
    pub strip: u16,
    pub seed: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EventKind {
    Hit(HitEv),
    Grain(GrainEv),
    Noise(NoiseEv),
    /// clique do metrônomo (só soa com o metrônomo ligado no engine)
    Click { accent: bool },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub tick: f64,
    pub kind: EventKind,
}

// ───────────────────────────── canais ─────────────────────────────

/// Canal do mixer (um `Tone.Channel` do web app): linha, linha×dono ou kiai.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelDef {
    /// `channelKey` do TS (`linha@pessoa|master`, `linha::dono`, sufixo `#kyai`); vazio = slot livre
    pub key: String,
    pub track_id: String,
    pub track: u16,
    pub kyai: bool,
    /// barramento de destino (0 = master)
    pub bus: u16,
    /// volume linear, pan (−1..1) e mudo (mudo/solo de linha já resolvidos)
    pub gain: f32,
    pub pan: f32,
    pub mute: bool,
    /// EQ do instrumento + filtro de timbre, por zona (couro, borda)
    pub chains: [FilterChain; 2],
}

impl ChannelDef {
    fn unused() -> Self {
        ChannelDef { key: String::new(), track_id: String::new(), track: 0, kyai: false, bus: 0, gain: 0.0, pan: 0.0, mute: true, chains: [FilterChain::default(); 2] }
    }
    pub fn is_used(&self) -> bool {
        !self.key.is_empty()
    }
}

/// Barramento (submix) de uma pessoa.
#[derive(Clone, Debug, PartialEq)]
pub struct BusDef {
    /// id da pessoa; vazio = master (slot 0) ou slot livre
    pub person_id: String,
    pub gain: f32,
    pub mute: bool,
}

// ───────────────────────────── mapa de tempo ─────────────────────────────

/// Relação entre ticks do transport e a música como escrita (compassos-fonte,
/// células). É o que a UI usa para cursor, playhead dobrado e seek.
#[derive(Clone, Debug, PartialEq)]
pub struct TimeMap {
    /// compasso tocado → compasso-fonte (saída de `play_order`)
    pub order: Vec<usize>,
    /// índice global da 1ª célula de cada compasso-fonte
    pub cell_start: Vec<usize>,
    /// (beats, subdivision) de cada compasso-fonte
    pub shape: Vec<(usize, usize)>,
    pub swing: f64,
    /// início de cada compasso TOCADO, em ticks (`order.len() + 1` entradas: a última
    /// é a duração total) — compassos de fórmulas diferentes têm durações diferentes
    pub bar_start: Vec<f64>,
    /// início de cada compasso-FONTE na música escrita, em ticks (para o playhead dobrado)
    pub src_start: Vec<f64>,
}

impl TimeMap {
    pub fn new(song: &Song, order: Vec<usize>) -> Self {
        let mut cell_start = Vec::with_capacity(song.measures.len());
        let mut acc = 0;
        for m in &song.measures {
            cell_start.push(acc);
            acc += m.cell_count();
        }
        let bar_start = bar_beat_starts(song, &order).into_iter().map(|b| b * PPQ).collect();
        let all: Vec<usize> = (0..song.measures.len()).collect();
        let src_start = bar_beat_starts(song, &all).into_iter().map(|b| b * PPQ).collect();
        TimeMap { order, cell_start, shape: song.measures.iter().map(|m| (m.beats, m.subdivision.max(1))).collect(), swing: song.swing(), bar_start, src_start }
    }

    pub fn len_ticks(&self) -> f64 {
        self.bar_start.last().copied().unwrap_or(0.0)
    }

    /// Tick do início do compasso tocado `u` (`u == order.len()` = fim da execução).
    pub fn bar_tick(&self, u: usize) -> f64 {
        self.bar_start.get(u).copied().unwrap_or_else(|| self.len_ticks())
    }

    /// compasso tocado em `tick` (None fora da execução)
    pub fn bar_at(&self, tick: f64) -> Option<usize> {
        if tick < 0.0 || tick >= self.len_ticks() {
            return None;
        }
        // último compasso que começa em ou antes de `tick`
        let u = self.bar_start.partition_point(|&s| s <= tick).saturating_sub(1);
        (u < self.order.len()).then_some(u)
    }

    /// Posição dobrada pro compasso-FONTE, em ticks (`transportSeconds` do TS):
    /// numa repetição o playhead volta pro início da seção.
    pub fn fold_tick(&self, tick: f64) -> f64 {
        match self.bar_at(tick) {
            Some(u) => self.src_start.get(self.order[u]).copied().unwrap_or(0.0) + (tick - self.bar_tick(u)),
            None => tick,
        }
    }

    /// `transportSeconds`: posição dobrada, em segundos no andamento `bpm`.
    pub fn folded_seconds(&self, tick: f64, bpm: f64) -> f64 {
        self.fold_tick(tick) / PPQ * 60.0 / bpm
    }

    /// Célula-fonte global sob o cursor em `tick` (o `onCursor` do TS; com
    /// swing, a célula do contratempo começa no ponto deformado).
    pub fn cell_at(&self, tick: f64) -> Option<usize> {
        let u = self.bar_at(tick)?;
        let mi = self.order[u];
        let (beats, sub) = *self.shape.get(mi)?;
        if beats == 0 {
            return None;
        }
        let q = (tick - self.bar_tick(u)) / PPQ;
        let beat = (q.floor() as usize).min(beats - 1);
        let f = if q >= beats as f64 { 1.0 } else { q - beat as f64 };
        let mut k = 0;
        for i in 1..sub {
            if swing_warp(i as f64 / sub as f64, self.swing) <= f + 1e-9 {
                k = i;
            }
        }
        Some(self.cell_start[mi] + beat * sub + k)
    }

    /// Tick do início de uma célula-fonte global (`cellPosition` do TS): com
    /// ritornellos, a 1ª vez que o compasso toca; compasso fora da região → 0.
    /// Como no TS, a fração NÃO passa pelo swing.
    pub fn cell_tick(&self, global_cell: usize) -> f64 {
        for (mi, &(beats, sub)) in self.shape.iter().enumerate() {
            let start = self.cell_start[mi];
            if global_cell < start + beats * sub {
                let ci = global_cell - start;
                let u = self.order.iter().position(|&m| m == mi).unwrap_or(0);
                return self.bar_tick(u) + ((ci / sub) as f64 + (ci % sub) as f64 / sub as f64) * PPQ;
            }
        }
        0.0
    }
}

// ───────────────────────────── schedule ─────────────────────────────

/// Música compilada. Imutável; vai pro thread de áudio dentro de um `Arc`.
#[derive(Clone, Debug, PartialEq)]
pub struct Schedule {
    /// eventos ordenados por tick
    pub events: Vec<Event>,
    /// duração de uma passada (nº de compassos tocados × 4 tempos)
    pub len_ticks: f64,
    /// canais por slot (estável entre compilações do mesmo `Compiler`)
    pub channels: Vec<ChannelDef>,
    /// barramentos por slot (0 = master)
    pub buses: Vec<BusDef>,
    pub time: TimeMap,
    /// andamento base da música (BPM)
    pub tempo: f64,
    /// mapa de andamento (rallentando/accelerando) em ticks
    pub curve: TempoCurve,
    /// compilado com região (o play começa do início dela)
    pub has_region: bool,
}

/// Mapa de andamento em ticks: trechos contíguos em que o BPM varia linearmente
/// com o tick. O engine multiplica o seu BPM (o da música, ou o do treino) por
/// [`TempoCurve::factor_at`] — sem marcações o fator é sempre 1.
#[derive(Clone, Debug, PartialEq)]
pub struct TempoCurve {
    /// (tick inicial, tick final, bpm inicial, bpm final)
    pub segments: Vec<(f64, f64, f64, f64)>,
    /// andamento de referência (o da música): fator = bpm / base
    pub base: f64,
}

impl TempoCurve {
    pub fn constant(base: f64) -> Self {
        TempoCurve { segments: Vec::new(), base }
    }

    /// Do mapa em tempos do core (`ops::measures::tempo_segments`).
    pub fn from_segments(segs: &[wadawko_core::ops::measures::TempoSegment], base: f64) -> Self {
        let constant = segs.iter().all(|s| s.bpm0 == base && s.bpm1 == base);
        let segments = if constant { Vec::new() } else { segs.iter().map(|s| (s.start * PPQ, s.end * PPQ, s.bpm0, s.bpm1)).collect() };
        TempoCurve { segments, base }
    }

    /// BPM (relativo ao andamento da música) no tick `t` (sem alocar: busca binária).
    pub fn bpm_at(&self, t: f64) -> f64 {
        if self.segments.is_empty() {
            return self.base;
        }
        let i = self.segments.partition_point(|s| s.0 <= t).saturating_sub(1);
        let (t0, t1, b0, b1) = self.segments[i];
        if t1 <= t0 {
            return b1;
        }
        b0 + (b1 - b0) * ((t - t0) / (t1 - t0)).clamp(0.0, 1.0)
    }

    /// Maior fator do mapa (1 sem marcações): limite seguro para cortes por distância.
    pub fn max_factor(&self) -> f64 {
        if self.base <= 0.0 {
            return 1.0;
        }
        self.segments.iter().fold(1.0f64, |m, s| m.max(s.2 / self.base).max(s.3 / self.base))
    }

    /// Fator a aplicar ao BPM do engine no tick `t`.
    pub fn factor_at(&self, t: f64) -> f64 {
        if self.base <= 0.0 { 1.0 } else { self.bpm_at(t) / self.base }
    }

    /// Segundos de `0` a `t` com o engine em `bpm` (integral exata: numa rampa
    /// linear em ticks o tempo decorrido é logarítmico).
    pub fn seconds_to(&self, t: f64, bpm: f64) -> f64 {
        let scale = bpm / self.base.max(1e-9);
        if self.segments.is_empty() {
            return t / PPQ * 60.0 / bpm.max(1e-9);
        }
        let mut secs = 0.0;
        for &(t0, t1, b0, b1) in &self.segments {
            if t <= t0 {
                break;
            }
            let end = t.min(t1);
            let (a0, a1) = (b0 * scale, b0 * scale + (b1 - b0) * scale * ((end - t0) / (t1 - t0).max(1e-9)));
            let dt = end - t0;
            // ∫ dτ / (PPQ·bpm(τ)/60), bpm linear em τ
            secs += if (a1 - a0).abs() < 1e-9 { dt / PPQ * 60.0 / a0 } else { dt / PPQ * 60.0 * (a1 / a0).ln() / (a1 - a0) };
        }
        if let Some(&(_, t1, _, b1)) = self.segments.last()
            && t > t1
        {
            secs += (t - t1) / PPQ * 60.0 / (b1 * scale);
        }
        secs
    }
}

impl Schedule {
    pub fn empty() -> Self {
        Schedule {
            events: Vec::new(),
            len_ticks: 0.0,
            channels: Vec::new(),
            buses: vec![BusDef { person_id: String::new(), gain: 1.0, mute: false }],
            time: TimeMap { order: Vec::new(), cell_start: Vec::new(), shape: Vec::new(), swing: 0.0, bar_start: vec![0.0], src_start: vec![0.0] },
            tempo: 100.0,
            curve: TempoCurve::constant(100.0),
            has_region: false,
        }
    }

    pub fn channel_slot(&self, key: &str) -> Option<u16> {
        self.channels.iter().position(|c| c.key == key).map(|i| i as u16)
    }

    pub fn hits(&self) -> impl Iterator<Item = (f64, &HitEv)> {
        self.events.iter().filter_map(|e| if let EventKind::Hit(h) = &e.kind { Some((e.tick, h)) } else { None })
    }

    /// duração de uma passada em segundos com o engine em `bpm` (respeita o mapa
    /// de andamento: rallentando alonga, accelerando encurta)
    pub fn seconds(&self, bpm: f64) -> f64 {
        self.curve.seconds_to(self.len_ticks, bpm)
    }
}

// ───────────────────────────── fórmulas de audio.ts ─────────────────────────────

/// velocity → ganho (`velGain`): don (0.7) = 0.865; acima cresce em curva;
/// abaixo de 0.5 cai linear até um piso de 10 % (ghost notes).
pub fn vel_gain(vel: f64) -> f64 {
    let at = |v: f64| 0.865 * (v / 0.7).powf(1.3);
    if vel >= 0.5 { at(vel) } else { at(0.5) * (0.1 + 0.9 * (vel / 0.5)) }
}

/// sforzando (`sfzDyn`): ~+6 dB acima da dinâmica do trecho, nunca abaixo de f
/// e com teto um pouco acima de ff.
pub fn sfz_dyn(dynv: f64) -> f64 {
    (Dynamic::Ff.level() * 1.15).min(Dynamic::F.level().max(dynv * 2.0))
}

pub fn db_gain(db: f64) -> f32 {
    10f64.powf(db / 20.0) as f32
}

pub(crate) fn base_of(id: InstrumentId) -> BaseInstrument {
    BaseInstrument::ALL[InstrumentId::ALL.iter().position(|i| *i == id).unwrap_or(0)]
}

pub(crate) fn szone(z: Zone) -> SZone {
    match z {
        Zone::Head => SZone::Head,
        Zone::Rim => SZone::Rim,
    }
}

/// Cadeia de filtros de uma zona (`zoneDest`): filtro de timbre opcional
/// (tone + overtones no Q) seguido do EQ do instrumento base.
pub fn zone_chain(def: &InstrumentDef, zone: Zone, sr: f64) -> FilterChain {
    let mut chain = FilterChain::default();
    let p = def.zone(zone);
    if let Some(tone) = p.and_then(|p| p.tone.as_ref()) {
        let q = (tone.q + p.and_then(|p| p.overtones).unwrap_or(0.0) * 8.0).max(0.1);
        let kind = match tone.kind {
            ToneType::Lowpass => FilterType::Lowpass,
            ToneType::Highpass => FilterType::Highpass,
            ToneType::Bandpass => FilterType::Bandpass,
        };
        chain.push(Biquad::new(kind, tone.cutoff, 0.0, q, sr));
    }
    for b in base_of(def.base).manifest().eq(szone(zone)) {
        let kind = match b.kind {
            FilterKind::Highpass => FilterType::Highpass,
            FilterKind::Lowshelf => FilterType::Lowshelf,
            FilterKind::Peaking => FilterType::Peaking,
        };
        chain.push(Biquad::new(kind, b.freq as f64, b.gain_db as f64, b.q as f64, sr));
    }
    chain
}

/// Um golpe como `playLayered` o prepara (menos a humanização, que é do
/// thread de áudio): camada por `vel·√dyn`, round robin, ganho, afinação,
/// duração (decay < 0.5 encurta; `TAIL` limita) e a rampa final.
pub fn hit_event(bank: &SampleBank, def: &InstrumentDef, zone: Zone, vel: f64, dynv: f64, rr: &mut RoundRobin, strip: u16) -> HitEv {
    let base = base_of(def.base);
    let m = base.manifest();
    let z = szone(zone);
    let p = def.zone(zone);
    let pick = bank.pick(base, z, vel as f32, dynv as f32, rr);
    let rate = m.tuning.rate(z) as f64 * 2f64.powf(p.and_then(|p| p.pitch).unwrap_or(0.0) / 12.0);
    let gain = m.tuning.gain(z) as f64 * 10f64.powf(p.and_then(|p| p.gain).unwrap_or(0.0) / 20.0) * dynv * vel_gain(vel);
    let natural = pick.data.duration_secs() / if rate == 0.0 { 1.0 } else { rate };
    let decay = p.and_then(|p| p.decay).unwrap_or(0.5);
    let tail = m.tail.max(z) as f64;
    let mut dur = natural;
    if decay < 0.5 {
        dur = (natural * decay * 2.0).max(0.03);
    }
    dur = dur.min(tail);
    let fade = if dur < natural - 0.01 { (if dur < tail { 0.02 } else { 0.25f64 }).min(dur / 2.0) } else { 0.0 };
    HitEv {
        sample: pick.id.0,
        rate: rate as f32,
        gain: gain as f32,
        dur: dur as f32,
        fade: fade as f32,
        choke: m.tail.choke,
        strip,
        rim: z.is_rim(),
        humanize: true,
        track: 0,
        cell: 0,
        hit: 0,
        notify: false,
    }
}

/// `channelKey` do TS: codifica o destino (barramento) na chave.
pub fn channel_key(track: &Track, owner: Option<&str>) -> String {
    let ids = track.person_ids();
    match owner {
        Some(o) if track.is_shared() => format!("{}::{o}", track.id),
        _ => format!("{}@{}", track.id, if ids.len() == 1 { ids[0].as_str() } else { "master" }),
    }
}

/// pessoa cujo barramento recebe o canal (None = master)
fn bus_person<'a>(track: &'a Track, owner: Option<&'a str>) -> Option<&'a str> {
    let ids = track.person_ids();
    if track.is_shared()
        && let Some(o) = owner
    {
        return Some(o);
    }
    if ids.len() == 1 { Some(ids[0].as_str()) } else { None }
}

/// linha audível pelo mudo/solo de LINHA (1ª camada; a 2ª é a das pessoas)
pub fn track_audible(song: &Song, track: &Track) -> bool {
    !track.muted && (!song.tracks.iter().any(|t| t.solo) || track.solo)
}

// ───────────────────────────── compilador ─────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct CompileOptions {
    /// região de compassos `[start, end)`: toca/loopa só ela
    pub region: Option<Region>,
    /// `true` (export): quem está mudo (linha ou pessoa) nem é agendado, como o
    /// `renderOffline`. `false` (ao vivo): tudo é agendado e o mudo é do mixer.
    pub skip_inaudible: bool,
    /// semente da variação dos kiai (estável entre recompilações)
    pub seed: u64,
    /// BPM usado para a duração das sílabas de kiai (default: `song.tempo`)
    pub bpm: Option<f64>,
}

impl Default for CompileOptions {
    fn default() -> Self {
        CompileOptions { region: None, skip_inaudible: false, seed: 0x5EED_7A1C, bpm: None }
    }
}

/// Compila músicas em [`Schedule`]s. Guarda a tabela de slots de canal e de
/// barramento entre compilações: a mesma chave cai sempre no mesmo slot, então
/// vozes soando e estados de filtro continuam válidos quando a UI recompila
/// durante o playback.
#[derive(Clone, Debug)]
pub struct Compiler {
    bank: SampleBank,
    sr: f64,
    chan_slots: Vec<Option<String>>,
    bus_slots: Vec<Option<String>>,
}

impl Compiler {
    pub fn new(bank: &SampleBank, sample_rate: u32) -> Self {
        Compiler { bank: bank.clone(), sr: sample_rate as f64, chan_slots: Vec::new(), bus_slots: vec![Some(String::new())] }
    }

    pub fn bank(&self) -> &SampleBank {
        &self.bank
    }

    pub fn sample_rate(&self) -> f64 {
        self.sr
    }

    fn slot_for(slots: &mut Vec<Option<String>>, used: &mut Vec<bool>, key: &str, max: usize) -> usize {
        let idx = match slots.iter().position(|s| s.as_deref() == Some(key)) {
            Some(i) => i,
            None => match slots.iter().position(Option::is_none) {
                Some(i) => {
                    slots[i] = Some(key.to_string());
                    i
                }
                None if slots.len() < max => {
                    slots.push(Some(key.to_string()));
                    slots.len() - 1
                }
                // estourou: divide o último slot (música com > 126 canais)
                None => max - 1,
            },
        };
        if used.len() < slots.len() {
            used.resize(slots.len(), false);
        }
        used[idx] = true;
        idx
    }

    pub fn compile(&mut self, song: &Song, opts: &CompileOptions) -> Schedule {
        let swing = song.swing();
        let bpm = opts.bpm.unwrap_or(song.tempo).max(1.0);
        let order = play_order(song, opts.region);
        let time = TimeMap::new(song, order.clone());
        let mut chan_used = vec![false; self.chan_slots.len()];
        let mut bus_used = vec![false; self.bus_slots.len()];
        bus_used[0] = true;
        let mut channels: Vec<ChannelDef> = Vec::new();
        let mut buses: Vec<BusDef> = vec![BusDef { person_id: String::new(), gain: 1.0, mute: false }];
        let mut rr: HashMap<usize, RoundRobin> = HashMap::new();
        let mut events: Vec<Event> = Vec::new();

        let audible: Vec<bool> = song.tracks.iter().map(|t| track_audible(song, t)).collect();
        let defs: Vec<&InstrumentDef> = song.tracks.iter().map(|t| resolve_instrument(song, t)).collect();
        let curves: Vec<_> = song.tracks.iter().map(|t| dynamics_curve(song, &order, Some(&t.id))).collect();
        let mains: Vec<f64> = song.tracks.iter().map(|t| main_mul(song, &t.id)).collect();

        // canal (slot) de um golpe/kiai; cria na 1ª vez
        let sr = self.sr;
        let chan_slots = &mut self.chan_slots;
        let bus_slots = &mut self.bus_slots;
        let mut channel = |ti: usize, owner: Option<&str>, kyai: bool| -> usize {
            let track = &song.tracks[ti];
            let mut key = channel_key(track, owner);
            if kyai {
                key.push_str("#kyai");
            }
            let slot = Self::slot_for(chan_slots, &mut chan_used, &key, USER_SLOTS);
            if channels.len() <= slot {
                channels.resize_with(slot + 1, ChannelDef::unused);
            }
            if !channels[slot].is_used() {
                let bus = match bus_person(track, owner) {
                    None => 0,
                    Some(pid) => {
                        let b = Self::slot_for(bus_slots, &mut bus_used, pid, MAX_BUSES);
                        if buses.len() <= b {
                            buses.resize_with(b + 1, || BusDef { person_id: String::new(), gain: 1.0, mute: false });
                        }
                        let person = song.person(pid);
                        buses[b] = BusDef {
                            person_id: pid.to_string(),
                            gain: db_gain(person.and_then(|p| p.volume).unwrap_or(0.0)),
                            // pessoa inexistente (removida) soa, como no TS
                            mute: !person_audible(song, Some(pid)),
                        };
                        b
                    }
                };
                let def = defs[ti];
                channels[slot] = ChannelDef {
                    key,
                    track_id: track.id.clone(),
                    track: ti as u16,
                    kyai,
                    bus: bus as u16,
                    gain: if kyai { db_gain(kyai_voice_or_default(track.kyai.as_ref()).2) } else { db_gain(track.volume.unwrap_or(Track::DEFAULT_VOLUME_DB)) },
                    pan: track.pan.unwrap_or(0.0).clamp(-1.0, 1.0) as f32,
                    mute: !audible[ti],
                    chains: if kyai { [FilterChain::default(); 2] } else { [zone_chain(def, Zone::Head, sr), zone_chain(def, Zone::Rim, sr)] },
                };
            }
            slot
        };

        // ao vivo, toda linha ganha os seus canais mesmo sem notas (`ensureTrack`)
        if !opts.skip_inaudible {
            for (ti, track) in song.tracks.iter().enumerate() {
                if track.is_shared() {
                    for pid in track.person_ids() {
                        channel(ti, Some(pid), false);
                    }
                } else {
                    channel(ti, None, false);
                }
            }
        }

        for (u, &mi) in order.iter().enumerate() {
            let Some(meas) = song.measures.get(mi) else { continue };
            let sub = meas.subdivision.max(1);
            for ci in 0..meas.cell_count() {
                let beat = ci / sub;
                let cell_frac = (ci % sub) as f64 / sub as f64;
                let global = time.cell_start[mi] + ci;
                for (ti, track) in song.tracks.iter().enumerate() {
                    if opts.skip_inaudible && !audible[ti] {
                        continue;
                    }
                    let Some(syl) = meas.cell(&track.id, ci).filter(|s| !s.is_empty()) else { continue };
                    let Some(resolved) = resolve_cell(syl) else { continue };
                    // instrumento compartilhado: golpe roteia pro barramento do dono da célula
                    let owner = cell_owner_of(song, track, mi, ci);
                    if opts.skip_inaudible && bus_person(track, owner).is_some_and(|p| !person_audible(song, Some(p))) {
                        continue;
                    }
                    let slot = channel(ti, owner, false);
                    for (hi, hit) in resolved.hits.iter().enumerate() {
                        // posição do golpe como fração do TEMPO, com warp de swing
                        let beat_frac = swing_warp(cell_frac + hit.offset / sub as f64, swing);
                        let tick = time.bar_tick(u) + (beat as f64 + beat_frac) * PPQ;
                        let d0 = curves[ti].at(u, (beat as f64 + beat_frac) / meas.beats.max(1) as f64);
                        let dynv = (if hit.sfz { sfz_dyn(d0) } else { d0 }) * mains[ti];
                        let strip = (slot * 2 + (hit.zone == Zone::Rim) as usize) as u16;
                        let mut ev = hit_event(&self.bank, defs[ti], hit.zone, hit.vel, dynv, rr.entry(slot).or_default(), strip);
                        ev.track = ti as u16;
                        ev.cell = global as u32;
                        ev.hit = hi as u8;
                        ev.notify = true;
                        events.push(Event { tick, kind: EventKind::Hit(ev) });
                    }
                }
            }
            // metrônomo: um clique por tempo, acento no tempo 1
            for b in 0..meas.beats {
                events.push(Event { tick: time.bar_tick(u) + b as f64 * PPQ, kind: EventKind::Click { accent: b == 0 } });
            }
        }

        // kiai: cada sílaba com a voz do dono da célula e entonação da palavra
        let animalese = self.bank.animalese_id().0;
        let spb = 60.0 / bpm;
        let mut grains = Vec::new();
        for e in kyai_events(song, &order, |f| swing_warp(f, swing)) {
            let ti = e.track_index;
            let track = &song.tracks[ti];
            if opts.skip_inaudible && !audible[ti] {
                continue;
            }
            let Some(meas) = song.measures.get(e.mi) else { continue };
            let owner = cell_owner_of(song, track, e.mi, e.ci);
            if opts.skip_inaudible && bus_person(track, owner).is_some_and(|p| !person_audible(song, Some(p))) {
                continue;
            }
            let slot = channel(ti, owner, true);
            let prof = kyai_profile(kyai_seed(track, meas, e.ci), track);
            // variação determinística por (semente, linha, compasso tocado, célula)
            let mut rng = Rng::new(opts.seed ^ ((ti as u64) << 48) ^ ((e.u as u64) << 16) ^ e.ci as u64);
            let lag = kyai_lag(prof.vary, rng.unit64());
            let note = kyai_note(&e.syl, e.idx, e.n, e.shout, e.gap_beats * spb, &prof, || rng.unit64());
            grains.clear();
            kyai_grains(&note, &prof, animalese, (slot * 2) as u16, lag, self.sr, &mut rng, &mut grains);
            let tick = e.beats * PPQ;
            events.extend(grains.iter().map(|&kind| Event { tick, kind }));
        }

        // ordem de tempo estável: no mesmo tick vale a ordem de linha (o choke depende dela)
        events.sort_by(|a, b| a.tick.total_cmp(&b.tick));

        // slots que esta música não usa ficam livres para a PRÓXIMA compilação
        for (i, s) in self.chan_slots.iter_mut().enumerate() {
            if !chan_used.get(i).copied().unwrap_or(false) {
                *s = None;
            }
        }
        for (i, s) in self.bus_slots.iter_mut().enumerate().skip(1) {
            if !bus_used.get(i).copied().unwrap_or(false) {
                *s = None;
            }
        }
        // tocando só uma região: parte do andamento que vale no início dela
        let start_bpm = match (opts.region, order.first()) {
            (Some(_), Some(&m0)) => wadawko_core::ops::measures::tempo_at_measure_starts(song).get(m0).copied().unwrap_or(song.tempo),
            _ => song.tempo,
        };
        let curve = TempoCurve::from_segments(&wadawko_core::ops::measures::tempo_segments_from(song, &order, start_bpm), song.tempo);
        Schedule { events, len_ticks: time.len_ticks(), channels, buses, time, tempo: song.tempo, curve, has_region: opts.region.is_some() }
    }
}

/// Compilação avulsa (slots novos a cada chamada) — export e testes.
pub fn compile(song: &Song, bank: &SampleBank, sample_rate: u32, opts: &CompileOptions) -> Schedule {
    Compiler::new(bank, sample_rate).compile(song, opts)
}
