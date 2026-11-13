//! Easter egg 🌸 — "Sakura Sakura" em fue (shinobue) sintetizado, porte de
//! `playFue` (`audio.ts`): triângulo + vibrato + sopro de ruído rosa filtrado.
//! Sintetizador monofônico de estado fixo; roda dentro do callback sem alocar.

use crate::dsp::{Biquad, BiquadState, FilterType, Rng};

/// segundos por semínima
const Q: f64 = 0.42;
/// atraso antes da 1ª nota (s)
const START: f64 = 0.08;

const A5: f32 = 880.0;
const B5: f32 = 987.766_6;
const C6: f32 = 1_046.502_3;
const F5: f32 = 698.456_5;
const E5: f32 = 659.255_1;

/// [nota, duração em semínimas] — frase de abertura de Sakura Sakura
const MELODY: [(f32, f64); 15] =
    [(A5, 1.0), (A5, 1.0), (B5, 2.0), (A5, 1.0), (A5, 1.0), (B5, 2.0), (A5, 1.0), (B5, 1.0), (C6, 1.0), (B5, 1.0), (A5, 1.0), (B5, 0.5), (A5, 0.5), (F5, 1.0), (E5, 3.0)];

/// duração total da melodia (s)
pub fn fue_total_secs() -> f64 {
    MELODY.iter().map(|(_, b)| b * Q).sum()
}

pub(crate) struct Fue {
    sr: f64,
    active: bool,
    /// frames desde o disparo
    t: u64,
    note: usize,
    note_start: f64,
    gate: bool,
    attacking: bool,
    env: f32,
    freq: f32,
    target: f32,
    phase: f32,
    lfo: f32,
    rng: Rng,
    pink: [f32; 3],
    bp: Biquad,
    bp_state: BiquadState,
    // constantes por taxa
    att_step: f32,
    k_decay: f32,
    k_release: f32,
    k_glide: f32,
}

impl Fue {
    pub fn new(sr: f64) -> Self {
        // curvas "exponential" do Tone: setTargetAtTime com τ = ln(t+1)/ln(200)
        let tc = |t: f64| (t + 1.0).ln() / 200f64.ln();
        let k = |tau: f64| (-1.0 / (tau * sr)).exp() as f32;
        Fue {
            sr,
            active: false,
            t: 0,
            note: 0,
            note_start: START,
            gate: false,
            attacking: false,
            env: 0.0,
            freq: A5,
            target: A5,
            phase: 0.0,
            lfo: 0.0,
            rng: Rng::new(0xF0E),
            pink: [0.0; 3],
            bp: Biquad::new(FilterType::Bandpass, 2600.0, 0.0, 1.0, sr),
            bp_state: BiquadState::default(),
            att_step: (1.0 / (0.07 * sr)) as f32,
            k_decay: k(tc(0.12)),
            k_release: k(tc(0.35)),
            k_glide: k(0.02 / 3.0),
        }
    }

    /// (re)começa a melodia; ignorado se já está tocando (como o `fuePlaying` do TS)
    pub fn start(&mut self) {
        if self.active {
            return;
        }
        self.active = true;
        self.t = 0;
        self.note = 0;
        self.note_start = START;
        self.gate = false;
        self.attacking = false;
        self.env = 0.0;
        self.freq = MELODY[0].0;
        self.target = MELODY[0].0;
        self.bp_state.reset();
    }

    pub fn stop(&mut self) {
        self.active = false;
    }

    /// soma `n` frames (mono) em `l` e `r`
    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        if !self.active {
            return;
        }
        let total = fue_total_secs();
        let sr = self.sr as f32;
        for (l, r) in l.iter_mut().zip(r.iter_mut()) {
            let t = self.t as f64 / self.sr;
            // sequenciador: ataque no início de cada nota, release a 92 % da duração
            if self.note < MELODY.len() {
                let (freq, beats) = MELODY[self.note];
                if !self.gate && t >= self.note_start {
                    self.gate = true;
                    self.attacking = true;
                    self.target = freq;
                } else if self.gate && t >= self.note_start + beats * Q * 0.92 {
                    self.gate = false;
                    self.attacking = false;
                    self.note += 1;
                    self.note_start += beats * Q;
                }
            }
            // envelope ADSR (ataque linear; decay e release exponenciais)
            if self.attacking {
                self.env += self.att_step;
                if self.env >= 1.0 {
                    self.env = 1.0;
                    self.attacking = false;
                }
            } else if self.gate {
                self.env = 0.8 + (self.env - 0.8) * self.k_decay;
            } else {
                self.env *= self.k_release;
            }
            // triângulo com portamento e vibrato (5,2 Hz, ±0,8 %)
            self.freq = self.target + (self.freq - self.target) * self.k_glide;
            self.lfo = (self.lfo + 5.2 / sr).fract();
            let vib = 1.0 + 0.008 * (self.lfo * std::f32::consts::TAU).sin();
            self.phase = (self.phase + self.freq * vib / sr).fract();
            let p = self.phase;
            let tri = if p < 0.25 { 4.0 * p } else if p < 0.75 { 2.0 - 4.0 * p } else { 4.0 * p - 4.0 };
            // sopro: ruído rosa em bandpass, bem baixo, acompanha a melodia
            let w = self.rng.bipolar();
            self.pink[0] = 0.99765 * self.pink[0] + w * 0.099_046;
            self.pink[1] = 0.963 * self.pink[1] + w * 0.296_516_4;
            self.pink[2] = 0.57 * self.pink[2] + w * 1.052_691_3;
            let pink = (self.pink[0] + self.pink[1] + self.pink[2] + w * 0.1848) * 0.25;
            let ts = t - START;
            let bg = if ts < 0.0 {
                0.0
            } else if ts < 0.15 {
                0.035 * ts / 0.15
            } else if ts < total - 0.3 {
                0.035
            } else if ts < total + 0.3 {
                0.035 * (total + 0.3 - ts) / 0.6
            } else {
                0.0
            };
            let breath = self.bp_state.tick(&self.bp, pink as f64) as f32 * bg as f32;
            let s = (tri * self.env + breath) * 0.4;
            *l += s;
            *r += s;
            self.t += 1;
            if ts > total + 1.0 {
                self.active = false;
                break;
            }
        }
    }
}
