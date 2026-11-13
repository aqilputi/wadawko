//! Instrumentos e compilação dos golpes — porte de src/lib/audio.ts:
//! SAMPLE_FILES (camadas de dinâmica × round robin), TUNING, TAIL, EQ,
//! velGain, humanização, sforzando, herança de dinâmica, InstrumentDefs
//! (builtins, presets e os customizados de cada música), volume/pan por linha.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;

use crate::audio::{BandKind, Biquad, EqSet, Ev, EvKind, MAX_BANDS, PPQ, SampleData, Schedule};
use crate::model::{self, Song};

// ───────────────────────────── tabelas (audio.ts) ─────────────────────────────

type Layers = &'static [&'static [&'static str]];

const CHU_RIM: Layers = &[&["chu-rim-0-0", "chu-rim-0-1"], &["chu-rim-1-0", "chu-rim-1-1", "chu-rim-1-2"], &["chu-rim-2-0", "chu-rim-2-1"]];

fn sample_files(base: &str) -> (Layers, Layers) {
    match base {
        "chu" => (&[&["chu-head-0-0", "chu-head-0-1"], &["chu-head-1-0", "chu-head-1-1"]], CHU_RIM),
        "hira" => (&[&["hira-head-0-0", "hira-head-0-1", "hira-head-0-2"], &["hira-head-1-0", "hira-head-1-1", "hira-head-1-2"]], CHU_RIM),
        "shime" => (
            &[
                &["shime-head-0-0", "shime-head-0-1", "shime-head-0-2", "shime-head-0-3"],
                &["shime-head-1-0", "shime-head-1-1", "shime-head-1-2", "shime-head-1-3"],
                &["shime-head-2-0", "shime-head-2-1", "shime-head-2-2", "shime-head-2-3"],
            ],
            &[&["shime-rim-0-0", "shime-rim-0-1"], &["shime-rim-1-0"], &["shime-rim-2-0", "shime-rim-2-1"]],
        ),
        "odaiko" => (&[&["odaiko-head-0-0", "odaiko-head-0-1"], &["odaiko-head-1-0"], &["odaiko-head-2-0"], &["odaiko-head-3-0"]], CHU_RIM),
        "nagado" => (&[&["nagado-head-0-0", "nagado-head-0-1"], &["nagado-head-1-0", "nagado-head-1-1", "nagado-head-1-2"]], CHU_RIM),
        "onagado" => (&[&["onagado-head-0-0", "onagado-head-0-1"], &["onagado-head-1-0", "onagado-head-1-1"], &["onagado-head-2-0"]], CHU_RIM),
        "okedo" => (
            &[
                &["okedo-head-0-0", "okedo-head-0-1", "okedo-head-0-2", "okedo-head-0-3"],
                &["okedo-head-1-0", "okedo-head-1-1", "okedo-head-1-2", "okedo-head-1-3"],
                &["okedo-head-2-0", "okedo-head-2-1", "okedo-head-2-2"],
            ],
            &[&["okedo-rim-0-0", "okedo-rim-0-1", "okedo-rim-0-2", "okedo-rim-0-3"]],
        ),
        "kane" => (&[&["kane-head-0-0", "kane-head-0-1", "kane-head-0-2", "kane-head-0-3"]], &[&["kane-rim-0-0", "kane-rim-0-1", "kane-rim-0-2", "kane-rim-0-3"]]),
        _ => sample_files("chu"),
    }
}

/// (headGain, rimGain, rimRate, headRate)
fn tuning(base: &str) -> (f32, f32, f32, f32) {
    match base {
        "shime" => (1.2, 0.9, 1.0, 1.0),
        "hira" => (1.1, 0.95, 1.12, 1.0),
        "odaiko" => (1.0, 0.85, 0.9, 1.0),
        "nagado" => (0.9, 1.0, 1.0, 1.0),
        "onagado" => (0.95, 0.95, 0.94, 1.0),
        "okedo" => (1.8, 1.3, 1.0, 1.0),
        "kane" => (0.8, 0.75, 1.0, 1.0),
        _ => (1.0, 1.0, 1.0, 1.0),
    }
}

/// cauda máxima (s) couro/borda e duração do choke
fn tail(base: &str) -> (f32, f32, f32) {
    match base {
        "shime" => (0.45, 0.35, 0.04),
        "hira" => (0.7, 0.5, 0.06),
        "nagado" => (1.8, 0.7, 0.1),
        "onagado" => (2.2, 0.7, 0.12),
        "okedo" => (0.8, 0.4, 0.06),
        "odaiko" => (2.8, 0.8, 0.14),
        "kane" => (1.2, 0.3, 0.05),
        _ => (1.3, 0.7, 0.09),
    }
}

type Band = (BandKind, f32, f32, f32);
use BandKind::{Highpass as HP, Lowshelf as LS, Peaking as PK};

fn eq_bands(base: &str, rim: bool) -> &'static [Band] {
    match (base, rim) {
        ("shime", false) => &[(HP, 180.0, 0.0, 0.7), (PK, 3200.0, 3.0, 1.0)],
        ("shime", true) => &[(HP, 450.0, 0.0, 0.7)],
        ("nagado", false) => &[(HP, 60.0, 0.0, 0.7), (LS, 120.0, -3.0, 0.7), (PK, 300.0, -4.0, 1.2), (PK, 2500.0, 3.0, 1.0)],
        ("onagado", false) => &[(HP, 50.0, 0.0, 0.7), (LS, 110.0, -2.0, 0.7), (PK, 260.0, -4.0, 1.2), (PK, 2200.0, 2.5, 1.0)],
        ("onagado", true) | ("odaiko", true) => &[(HP, 350.0, 0.0, 0.7)],
        ("hira", false) => &[(HP, 110.0, 0.0, 0.7), (PK, 380.0, -3.0, 1.2), (PK, 3000.0, 2.5, 1.0)],
        ("hira", true) => &[(HP, 400.0, 0.0, 0.7), (PK, 3800.0, 2.0, 1.0)],
        ("okedo", false) => &[(HP, 110.0, 0.0, 0.7), (PK, 420.0, -3.0, 1.2), (PK, 3000.0, 3.0, 1.0)],
        ("okedo", true) => &[(HP, 400.0, 0.0, 0.7)],
        ("odaiko", false) => &[(HP, 40.0, 0.0, 0.7), (PK, 230.0, -4.0, 1.0), (PK, 1800.0, 2.0, 1.0)],
        ("kane", false) => &[(HP, 600.0, 0.0, 0.7)],
        ("kane", true) => &[(HP, 700.0, 0.0, 0.7)],
        (_, false) => &[(HP, 70.0, 0.0, 0.7), (LS, 130.0, -3.0, 0.7), (PK, 350.0, -4.0, 1.2), (PK, 2800.0, 3.0, 1.0)],
        (_, true) => &[(HP, 350.0, 0.0, 0.7), (PK, 3500.0, 2.0, 1.0)],
    }
}

/// velocity → ganho (velGain de audio.ts)
fn vel_gain(vel: f32) -> f32 {
    let at = |v: f32| 0.865 * (v / 0.7).powf(1.3);
    if vel >= 0.5 { at(vel) } else { at(0.5) * (0.1 + 0.9 * (vel / 0.5)) }
}

pub fn dyn_level(d: Option<&str>) -> f32 {
    match d {
        Some("pp") => 0.25,
        Some("p") => 0.5,
        Some("mp") => 0.71,
        Some("f") => 2.0,
        Some("ff") => 3.2,
        _ => 1.0,
    }
}

fn sfz_dyn(d: f32) -> f32 {
    (3.2 * 1.15f32).min(2.0f32.max(d * 2.0))
}

// ───────────────────────────── InstrumentDef ─────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct Zone {
    pub pitch: f32,
    pub gain_db: f32,
    pub decay: f32,
    pub tone: Option<(BandKind, f32, f32)>,
}

#[derive(Clone, Debug)]
pub struct InstDef {
    pub id: String,
    pub base: String,
    pub head: Zone,
    pub rim: Zone,
}

fn zone(pitch: f32, gain_db: f32, decay: f32, tone: Option<(BandKind, f32, f32)>) -> Zone {
    Zone { pitch, gain_db, decay, tone }
}

pub fn parse_zone(v: &serde_json::Value) -> Zone {
    let tone = v["tone"].as_object().map(|t| {
        let kind = match t.get("type").and_then(|x| x.as_str()).unwrap_or("lowpass") {
            "highpass" => BandKind::Highpass,
            "bandpass" => BandKind::Bandpass,
            _ => BandKind::Lowpass,
        };
        let cutoff = t.get("cutoff").and_then(|x| x.as_f64()).unwrap_or(1000.0) as f32;
        let q = t.get("q").and_then(|x| x.as_f64()).unwrap_or(1.0) as f32;
        let over = v["overtones"].as_f64().unwrap_or(0.0) as f32;
        (kind, cutoff, (q + over * 8.0).max(0.1))
    });
    Zone {
        pitch: v["pitch"].as_f64().unwrap_or(0.0) as f32,
        gain_db: v["gain"].as_f64().unwrap_or(0.0) as f32,
        decay: v["decay"].as_f64().unwrap_or(0.5) as f32,
        tone,
    }
}

/// Os 8 instrumentos base + os 5 presets de types.ts.
pub fn builtin_defs() -> Vec<InstDef> {
    let mut v: Vec<InstDef> = model::INSTRUMENTS
        .iter()
        .map(|b| InstDef { id: b.to_string(), base: b.to_string(), head: zone(0.0, 0.0, 0.5, None), rim: zone(0.0, 0.0, 0.5, None) })
        .collect();
    let p = |id: &str, base: &str, head: Zone, rim: Zone| InstDef { id: id.into(), base: base.into(), head, rim };
    v.push(p("preset-shime-agudo", "shime", zone(7.0, 0.0, 0.4, Some((BandKind::Highpass, 900.0, 1.0))), zone(7.0, -2.0, 0.5, None)));
    v.push(p("preset-chu-grave", "chu", zone(-5.0, 1.0, 0.7, None), zone(-3.0, 0.0, 0.5, None)));
    v.push(p("preset-odaiko-profundo", "odaiko", zone(-7.0, 2.0, 0.85, Some((BandKind::Lowpass, 600.0, 1.0))), zone(-4.0, -2.0, 0.5, None)));
    v.push(p(
        "preset-nagado-anelado",
        "nagado",
        zone(0.0, 8.0, 0.5, Some((BandKind::Bandpass, 320.0, 1.2 + 0.4 * 8.0))),
        zone(0.0, 0.0, 0.5, Some((BandKind::Bandpass, 2200.0, 2.0 + 0.5 * 8.0))),
    ));
    v.push(p("preset-okedo-brilhante", "okedo", zone(3.0, 3.0, 0.45, Some((BandKind::Highpass, 380.0, 1.2))), zone(5.0, 0.0, 0.5, None)));
    v
}

// ───────────────────────────── banco de samples ─────────────────────────────

pub struct Bank {
    pub samples: Arc<Vec<SampleData>>,
    index: HashMap<&'static str, u16>,
}

pub fn load_bank(dir: &Path) -> Result<Bank> {
    let mut names: Vec<&'static str> = Vec::new();
    for base in model::INSTRUMENTS {
        let (h, r) = sample_files(base);
        for layer in h.iter().chain(r.iter()) {
            for n in layer.iter() {
                if !names.contains(n) {
                    names.push(n);
                }
            }
        }
    }
    let mut samples = Vec::with_capacity(names.len());
    let mut index = HashMap::new();
    for (i, n) in names.iter().enumerate() {
        samples.push(crate::audio::load_sample(&dir.join(format!("{n}.ogg")))?);
        index.insert(*n, i as u16);
    }
    Ok(Bank { samples: Arc::new(samples), index })
}

impl Bank {
    /// duração natural (s) de um sample
    fn secs(&self, idx: u16) -> f32 {
        let s = &self.samples[idx as usize];
        s.frames.len() as f32 / s.sr as f32
    }
}

// ───────────────────────────── compilação ─────────────────────────────

struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
    fn jitter(&mut self, amt: f32) -> f32 {
        1.0 + self.next() * amt
    }
}

pub fn compile(song: &Song, bank: &Bank, sr: u32, metronome: bool) -> Schedule {
    let defs = {
        let mut d = builtin_defs();
        d.extend(song.instruments.iter().cloned());
        d
    };
    let resolve = |t: &model::Track| -> InstDef {
        t.instrument_id
            .as_deref()
            .and_then(|id| defs.iter().find(|d| d.id == id))
            .or_else(|| defs.iter().find(|d| d.id == t.instrument))
            .cloned()
            .unwrap_or_else(|| defs[1].clone())
    };
    let track_defs: Vec<InstDef> = song.tracks.iter().map(resolve).collect();

    // EQ por (linha, zona)
    let mut eq = Vec::new();
    for d in &track_defs {
        for rim in [false, true] {
            let z = if rim { &d.rim } else { &d.head };
            let mut set = EqSet::default();
            for &(k, f, g, q) in eq_bands(&d.base, rim) {
                if set.n < MAX_BANDS {
                    set.bands[set.n] = Biquad::new(k, f, g, q, sr as f32);
                    set.n += 1;
                }
            }
            if let Some((k, f, q)) = z.tone
                && set.n < MAX_BANDS
            {
                set.bands[set.n] = Biquad::new(k, f, 0.0, q, sr as f32);
                set.n += 1;
            }
            eq.push(set);
        }
    }

    let (order, len) = song.play_order();
    let any_solo = song.tracks.iter().any(|t| t.solo);
    let mut rr: HashMap<(usize, bool, usize), usize> = HashMap::new();
    let mut rng = Rng(0x2545_F491);
    let jitter_ticks = 0.006 * song.tempo / 60.0 * PPQ as f32;
    let mut events = Vec::new();
    let mut cur_dyn = 1.0f32; // herança: vale até a próxima marcação

    for &(start, mi) in &order {
        let m = &song.measures[mi];
        if let Some(d) = m.dynamic.as_deref() {
            cur_dyn = dyn_level(Some(d));
        }
        let cell_ticks = PPQ as f64 / m.subdivision as f64;
        for (t, row) in m.cells.iter().enumerate() {
            let tr = &song.tracks[t];
            if tr.muted || (any_solo && !tr.solo) {
                continue;
            }
            let def = &track_defs[t];
            let (hg, rg, rrate, hrate) = tuning(&def.base);
            let (tail_h, tail_r, choke) = tail(&def.base);
            let (head_layers, rim_layers) = sample_files(&def.base);
            let ch_gain = 10f32.powf(tr.volume_db / 20.0);
            let pan = tr.pan.clamp(-1.0, 1.0);
            let (pl, pr) = (((1.0 - pan) * std::f32::consts::FRAC_PI_4).cos(), ((1.0 + pan) * std::f32::consts::FRAC_PI_4).cos());
            let (pl, pr) = (pl * std::f32::consts::SQRT_2, pr * std::f32::consts::SQRT_2);
            for (k, cell) in row.iter().enumerate() {
                let Some(word) = cell else { continue };
                let sfz = word.ends_with('!');
                let (mut cstart, mut clen) = (k as f64 * cell_ticks, cell_ticks);
                if song.swing && m.subdivision == 2 {
                    let beat = (k / 2) as f64 * PPQ as f64;
                    (cstart, clen) = if k % 2 == 0 { (beat, PPQ as f64 * 2.0 / 3.0) } else { (beat + PPQ as f64 * 2.0 / 3.0, PPQ as f64 / 3.0) };
                }
                for h in model::resolve(word) {
                    let zone = if h.rim { &def.rim } else { &def.head };
                    let layers = if h.rim { rim_layers } else { head_layers };
                    let dynv = if sfz { sfz_dyn(cur_dyn) } else { cur_dyn };
                    // camada por vel·√dyn; round robin por (linha, zona, camada)
                    let lv = (h.vel * dynv.sqrt()).min(1.0);
                    let n = layers.len();
                    let li = (0..n).find(|&i| lv <= if i == n - 1 { 1.01 } else { (i + 1) as f32 / n as f32 }).unwrap_or(n - 1);
                    let c = rr.entry((t, h.rim, li)).or_insert(0);
                    let name = layers[li][*c % layers[li].len()];
                    *c += 1;
                    let Some(&sample) = bank.index.get(name) else { continue };

                    let rate = (if h.rim { rrate } else { hrate }) * 2f32.powf(zone.pitch / 12.0) * rng.jitter(0.01);
                    let level = (if h.rim { rg } else { hg }) * 10f32.powf(zone.gain_db / 20.0) * dynv * vel_gain(h.vel) * rng.jitter(0.12) * ch_gain;
                    let natural = bank.secs(sample) / rate;
                    let tail_max = if h.rim { tail_r } else { tail_h };
                    let mut dur = natural;
                    if zone.decay < 0.5 {
                        dur = (natural * zone.decay * 2.0).max(0.03);
                    }
                    let dur = dur.min(tail_max);
                    let fade = if dur < tail_max { 0.02 } else { 0.25 };
                    let tick = start as f64 + cstart + h.offset as f64 * clen + (rng.next() * jitter_ticks) as f64;
                    events.push(Ev {
                        tick: tick.max(0.0) as u64,
                        kind: EvKind::Sample {
                            sample,
                            rate,
                            gain_l: level * pl,
                            gain_r: level * pr,
                            tail: dur,
                            fade,
                            group: t as u16,
                            rim: h.rim,
                            choke,
                            eq: (t * 2 + h.rim as usize) as u16,
                        },
                    });
                }
            }
        }
        if metronome {
            for b in 0..m.beats as u64 {
                events.push(Ev { tick: start + b * PPQ, kind: EvKind::Click { accent: b == 0 } });
            }
        }
    }
    events.sort_by_key(|e| e.tick);
    Schedule { events, len_ticks: len, eq }
}
