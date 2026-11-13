//! DSP básico do engine: biquads (fórmulas do `BiquadFilterNode` do WebAudio),
//! compressor do master (porte do `DynamicsCompressorNode`), soft clip
//! (`softClipCurve` de `audio.ts`), limiter com lookahead do export
//! (`lookaheadLimit`) e o PRNG sem alocação da humanização.

use std::f64::consts::PI;

// ───────────────────────────── PRNG ─────────────────────────────

/// xorshift32: PRNG mínimo, sem alocação, usável dentro do callback.
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u32);

impl Rng {
    /// Semeia a partir de um u64 qualquer (splitmix); nunca fica em 0.
    pub fn new(seed: u64) -> Self {
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        Rng(((z as u32) ^ ((z >> 32) as u32)).max(1))
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// uniforme em [0, 1)
    #[inline]
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// uniforme em [0, 1) em f64 (o `Math.random()` das contas do kiai)
    #[inline]
    pub fn unit64(&mut self) -> f64 {
        self.unit() as f64
    }

    /// uniforme em [-1, 1)
    #[inline]
    pub fn bipolar(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }

    /// `jitter(amt)` de `audio.ts`: 1 ± amt
    #[inline]
    pub fn jitter(&mut self, amt: f32) -> f32 {
        1.0 + self.bipolar() * amt
    }
}

// ───────────────────────────── conversões ─────────────────────────────

#[inline]
pub fn db_to_lin(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

#[inline]
pub fn lin_to_db(lin: f64) -> f64 {
    if lin <= 0.0 { -1000.0 } else { 20.0 * lin.log10() }
}

// ───────────────────────────── biquads ─────────────────────────────

/// Tipos de filtro usados pelo app (subconjunto de `BiquadFilterType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterType {
    Lowpass,
    Highpass,
    Bandpass,
    Lowshelf,
    Peaking,
}

/// Coeficientes normalizados (a0 = 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Biquad {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Default for Biquad {
    fn default() -> Self {
        Biquad::IDENTITY
    }
}

impl Biquad {
    pub const IDENTITY: Biquad = Biquad { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 };

    /// Coeficientes como o `BiquadFilterNode` calcula. Atenção às convenções
    /// do WebAudio (que o `Tone.Filter` herda): em lowpass/highpass o `Q` é
    /// ressonância em **dB**; em bandpass/peaking é Q linear; o lowshelf ignora
    /// o Q (inclinação S = 1).
    pub fn new(kind: FilterType, freq: f64, gain_db: f64, q: f64, sr: f64) -> Biquad {
        let w0 = 2.0 * PI * (freq / sr).clamp(1e-6, 0.4999);
        let (sin, cos) = w0.sin_cos();
        let a = 10f64.powf(gain_db / 40.0);
        let (b0, b1, b2, a0, a1, a2) = match kind {
            FilterType::Lowpass => {
                let alpha = sin / (2.0 * 10f64.powf(q / 20.0));
                ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
            }
            FilterType::Highpass => {
                let alpha = sin / (2.0 * 10f64.powf(q / 20.0));
                ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
            }
            FilterType::Bandpass => {
                let alpha = sin / (2.0 * q.max(1e-4));
                (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
            }
            FilterType::Peaking => {
                let alpha = sin / (2.0 * q.max(1e-4));
                (1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a)
            }
            FilterType::Lowshelf => {
                let alpha = 0.5 * sin * 2f64.sqrt(); // S = 1
                let k2 = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cos + k2),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - k2),
                    (a + 1.0) + (a - 1.0) * cos + k2,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - k2,
                )
            }
        };
        Biquad { b0: (b0 / a0) as f32, b1: (b1 / a0) as f32, b2: (b2 / a0) as f32, a1: (a1 / a0) as f32, a2: (a2 / a0) as f32 }
    }

    /// Resposta em magnitude (dB) na frequência `freq`.
    pub fn response_db(&self, freq: f64, sr: f64) -> f64 {
        let w = 2.0 * PI * freq / sr;
        let (s1, c1) = w.sin_cos();
        let (s2, c2) = (2.0 * w).sin_cos();
        let (b0, b1, b2, a1, a2) = (self.b0 as f64, self.b1 as f64, self.b2 as f64, self.a1 as f64, self.a2 as f64);
        let (nr, ni) = (b0 + b1 * c1 + b2 * c2, -(b1 * s1 + b2 * s2));
        let (dr, di) = (1.0 + a1 * c1 + a2 * c2, -(a1 * s1 + a2 * s2));
        10.0 * ((nr * nr + ni * ni) / (dr * dr + di * di)).log10()
    }
}

/// Estado de um biquad (forma direta II transposta, em f64).
#[derive(Clone, Copy, Debug, Default)]
pub struct BiquadState {
    z1: f64,
    z2: f64,
}

impl BiquadState {
    pub const ZERO: BiquadState = BiquadState { z1: 0.0, z2: 0.0 };

    #[inline]
    pub fn tick(&mut self, c: &Biquad, x: f64) -> f64 {
        let y = c.b0 as f64 * x + self.z1;
        self.z1 = c.b1 as f64 * x - c.a1 as f64 * y + self.z2;
        self.z2 = c.b2 as f64 * x - c.a2 as f64 * y;
        y
    }

    pub fn reset(&mut self) {
        *self = BiquadState::default();
    }
}

/// Máximo de biquads em série por zona: até 4 bandas de EQ + filtro de timbre.
pub const MAX_BANDS: usize = 6;

/// Cadeia de biquads em série (EQ do instrumento + filtro de timbre da zona).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilterChain {
    pub bands: [Biquad; MAX_BANDS],
    pub n: u8,
}

impl Default for FilterChain {
    fn default() -> Self {
        FilterChain { bands: [Biquad::IDENTITY; MAX_BANDS], n: 0 }
    }
}

impl FilterChain {
    pub fn push(&mut self, b: Biquad) {
        if (self.n as usize) < MAX_BANDS {
            self.bands[self.n as usize] = b;
            self.n += 1;
        }
    }

    pub fn bands(&self) -> &[Biquad] {
        &self.bands[..self.n as usize]
    }

    /// resposta total (dB) em `freq`
    pub fn response_db(&self, freq: f64, sr: f64) -> f64 {
        self.bands().iter().map(|b| b.response_db(freq, sr)).sum()
    }
}

// ───────────────────────────── soft clip ─────────────────────────────

/// `softClipCurve` de `audio.ts`: linear até 0,9 (≈ −1 dBFS) e satura em tanh
/// até 0,98. A entrada é limitada a ±1 antes, como o `WaveShaperNode` faz
/// (fora da curva vale o valor da ponta).
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-1.0, 1.0);
    let a = x.abs();
    if a < 0.9 { x } else { x.signum() * (0.9 + 0.08 * ((a - 0.9) / 0.08).tanh()) }
}

// ───────────────────────────── compressor ─────────────────────────────

/// Parâmetros do compressor (os do `Tone.Compressor` do master).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompressorParams {
    pub threshold_db: f64,
    pub knee_db: f64,
    pub ratio: f64,
    pub attack: f64,
    pub release: f64,
    /// atraso de lookahead (s); o WebAudio usa 6 ms fixos
    pub pre_delay: f64,
}

impl CompressorParams {
    /// `addMasterLimiter` de `audio.ts`: −8 dB, 4:1, ataque 2 ms, release 150 ms, joelho 6 dB.
    pub const MASTER: CompressorParams = CompressorParams { threshold_db: -8.0, knee_db: 6.0, ratio: 4.0, attack: 0.002, release: 0.15, pre_delay: 0.006 };
}

/// Curva estática do `DynamicsCompressorNode` (joelho exponencial ACIMA do
/// threshold, depois reta de `ratio`), com o ganho de compensação automático.
#[derive(Clone, Copy, Debug)]
pub struct CompressorCurve {
    linear_threshold: f64,
    knee_threshold: f64,
    knee_threshold_db: f64,
    yknee_threshold_db: f64,
    ratio: f64,
    k: f64,
    /// ganho de compensação: `(1 / saturate(1))^0.6`
    pub makeup: f64,
}

impl CompressorCurve {
    pub fn new(p: &CompressorParams) -> Self {
        let linear_threshold = db_to_lin(p.threshold_db);
        let knee_threshold_db = p.threshold_db + p.knee_db;
        let knee_threshold = db_to_lin(knee_threshold_db);
        let mut c = CompressorCurve { linear_threshold, knee_threshold, knee_threshold_db, yknee_threshold_db: 0.0, ratio: p.ratio, k: 5.0, makeup: 1.0 };
        // k tal que a inclinação no fim do joelho seja 1/ratio (busca do Chromium)
        let desired = 1.0 / p.ratio;
        let (mut min_k, mut max_k) = (0.1f64, 10_000f64);
        for _ in 0..15 {
            if c.slope_at(knee_threshold) < desired {
                max_k = c.k;
            } else {
                min_k = c.k;
            }
            c.k = (min_k * max_k).sqrt();
        }
        c.yknee_threshold_db = lin_to_db(c.knee_curve(knee_threshold));
        c.makeup = (1.0 / c.saturate(1.0)).powf(0.6);
        c
    }

    fn knee_curve(&self, x: f64) -> f64 {
        if x < self.linear_threshold { x } else { self.linear_threshold + (1.0 - (-self.k * (x - self.linear_threshold)).exp()) / self.k }
    }

    fn slope_at(&self, x: f64) -> f64 {
        if x < self.linear_threshold {
            return 1.0;
        }
        let x2 = x * 1.001;
        (lin_to_db(self.knee_curve(x2)) - lin_to_db(self.knee_curve(x))) / (lin_to_db(x2) - lin_to_db(x))
    }

    /// saída (linear) da curva para uma entrada (linear, ≥ 0), sem compensação
    pub fn saturate(&self, x: f64) -> f64 {
        if x < self.knee_threshold { self.knee_curve(x) } else { db_to_lin(self.yknee_threshold_db + (lin_to_db(x) - self.knee_threshold_db) / self.ratio) }
    }
}

/// Compressor estéreo ligado — porte do `DynamicsCompressorKernel` do Chromium
/// (detector com release saturado, envelope adaptativo em divisões de 32
/// frames, lookahead de 6 ms, ganho de compensação). Não aloca em `process`.
pub struct Compressor {
    curve: CompressorCurve,
    master_gain: f64,
    attack_frames: f64,
    sat_release_frames: f64,
    k: [f64; 5],
    detector_average: f64,
    compressor_gain: f64,
    max_attack_diff_db: f64,
    // divisão corrente (32 frames)
    div_left: u32,
    envelope_rate: f64,
    scaled_desired_gain: f64,
    // lookahead
    delay: Box<[[f32; 2]]>,
    delay_pos: usize,
    /// redução de ganho atual (dB ≤ 0), para medidor
    pub reduction_db: f32,
}

impl Compressor {
    pub fn new(p: &CompressorParams, sr: f64) -> Self {
        let curve = CompressorCurve::new(p);
        let release_frames = sr * p.release;
        // zonas de release adaptativo (constantes do Chromium)
        let (y1, y2, y3, y4) = (release_frames * 0.09, release_frames * 0.16, release_frames * 0.42, release_frames * 0.98);
        let k = [
            0.9999999999999998 * y1 + 1.8432219684323923e-16 * y2 - 1.9373394351676423e-16 * y3 + 8.824516011816245e-18 * y4,
            -1.5788320352845888 * y1 + 2.3305837032074286 * y2 - 0.9141194204840429 * y3 + 0.1623677525612032 * y4,
            0.5334142869106424 * y1 - 1.272736789213631 * y2 + 0.9258856042207512 * y3 - 0.18656310191776226 * y4,
            0.08783463138207234 * y1 - 0.1694162967925622 * y2 + 0.08588057951595272 * y3 - 0.00429891410546283 * y4,
            -0.042416883008123074 * y1 + 0.1115693827987602 * y2 - 0.09764676325265872 * y3 + 0.028494263462021576 * y4,
        ];
        let delay_frames = ((p.pre_delay * sr).round() as usize).max(1);
        Compressor {
            curve,
            master_gain: curve.makeup,
            attack_frames: p.attack.max(0.001) * sr,
            sat_release_frames: 0.0025 * sr,
            k,
            // começa "aberto" (o WebAudio parte de 0 e afunda os primeiros ms do contexto)
            detector_average: 1.0,
            compressor_gain: 1.0,
            max_attack_diff_db: -1.0,
            div_left: 0,
            envelope_rate: 1.0,
            scaled_desired_gain: 1.0,
            delay: vec![[0.0; 2]; delay_frames].into_boxed_slice(),
            delay_pos: 0,
            reduction_db: 0.0,
        }
    }

    /// atraso (frames) introduzido pelo lookahead
    pub fn latency_frames(&self) -> usize {
        self.delay.len()
    }

    pub fn curve(&self) -> &CompressorCurve {
        &self.curve
    }

    fn begin_division(&mut self) {
        let desired = self.detector_average;
        let scaled = (desired.clamp(0.0, 1.0)).asin() / (PI / 2.0);
        self.scaled_desired_gain = scaled;
        let releasing = scaled > self.compressor_gain;
        let mut diff_db = lin_to_db(self.compressor_gain / scaled.max(1e-12));
        if !diff_db.is_finite() {
            diff_db = if releasing { -1.0 } else { 1.0 };
        }
        if releasing {
            self.max_attack_diff_db = -1.0;
            let x = 0.25 * (diff_db.clamp(-12.0, 0.0) + 12.0);
            let (x2, x3) = (x * x, x * x * x);
            let release_frames = self.k[0] + self.k[1] * x + self.k[2] * x2 + self.k[3] * x3 + self.k[4] * x2 * x2;
            self.envelope_rate = db_to_lin(5.0 / release_frames.max(1.0));
        } else {
            if self.max_attack_diff_db == -1.0 || self.max_attack_diff_db < diff_db {
                self.max_attack_diff_db = diff_db;
            }
            let eff = self.max_attack_diff_db.max(0.5);
            self.envelope_rate = 1.0 - (0.25 / eff).powf(1.0 / self.attack_frames);
        }
        self.div_left = 32;
    }

    /// Processa um frame estéreo (devolve o frame atrasado pelo lookahead).
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        if self.div_left == 0 {
            self.begin_division();
        }
        self.div_left -= 1;
        let [dl, dr] = std::mem::replace(&mut self.delay[self.delay_pos], [l, r]);
        self.delay_pos += 1;
        if self.delay_pos == self.delay.len() {
            self.delay_pos = 0;
        }
        // detector sobre a entrada NÃO atrasada
        let abs_in = l.abs().max(r.abs()) as f64;
        let attenuation = if abs_in <= 0.0001 { 1.0 } else { self.curve.saturate(abs_in) / abs_in };
        let attenuation_db = (-lin_to_db(attenuation)).max(2.0);
        let sat_release_rate = db_to_lin(attenuation_db / self.sat_release_frames) - 1.0;
        let rate = if attenuation > self.detector_average { sat_release_rate } else { 1.0 };
        self.detector_average = (self.detector_average + (attenuation - self.detector_average) * rate).min(1.0);
        if !self.detector_average.is_finite() {
            self.detector_average = 1.0;
        }
        if self.envelope_rate < 1.0 {
            self.compressor_gain += (self.scaled_desired_gain - self.compressor_gain) * self.envelope_rate;
        } else {
            self.compressor_gain = (self.compressor_gain * self.envelope_rate).min(1.0);
        }
        if !self.compressor_gain.is_finite() {
            self.compressor_gain = 1.0;
        }
        let post = (PI / 2.0 * self.compressor_gain).sin();
        self.reduction_db = lin_to_db(post) as f32;
        let g = (self.master_gain * post) as f32;
        (dl * g, dr * g)
    }
}

// ───────────────────────────── limiter do export ─────────────────────────────

/// Resultado de [`lookahead_limit`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LimitStats {
    /// pico antes do limiter
    pub peak_in: f32,
    /// ganho de normalização aplicado (≤ 4 = +12 dB)
    pub pre_gain: f32,
    /// menor ganho do limiter (1 = não atuou)
    pub min_gain: f32,
}

/// Porte de `lookaheadLimit` (`audio.ts`) sobre estéreo intercalado: ganho de
/// normalização (até +12 dB, deixando no máx. ~5 dB pro limiter) e limiter com
/// lookahead de 5 ms (o ganho desce ANTES do pico) e release de ~80 ms.
pub fn lookahead_limit(buf: &mut [f32], sr: u32, ceiling_db: f32) -> LimitStats {
    let n = buf.len() / 2;
    let ceiling = 10f32.powf(ceiling_db / 20.0);
    let mut peak_at = vec![0f32; n];
    let mut pk = 0f32;
    for i in 0..n {
        let v = buf[2 * i].abs().max(buf[2 * i + 1].abs());
        peak_at[i] = v;
        pk = pk.max(v);
    }
    if pk == 0.0 || n == 0 {
        return LimitStats { peak_in: 0.0, pre_gain: 1.0, min_gain: 1.0 };
    }
    let pre = (ceiling * 1.78 / pk).min(4.0);
    let look = ((0.005 * sr as f64).round() as usize).max(1);
    // ganho necessário por amostra (reaproveita peak_at) e mínimo na janela à frente
    let need = &mut peak_at;
    for v in need.iter_mut() {
        *v = (ceiling / (*v * pre).max(1e-9)).min(1.0);
    }
    let mut ahead = vec![0f32; n];
    let mut dq = vec![0u32; n];
    let (mut head, mut tail) = (0usize, 0usize);
    for i in (0..n).rev() {
        while tail > head && need[dq[tail - 1] as usize] >= need[i] {
            tail -= 1;
        }
        dq[tail] = i as u32;
        tail += 1;
        while dq[head] as usize > i + look {
            head += 1;
        }
        ahead[i] = need[dq[head] as usize];
    }
    // ataque suave (média móvel do tamanho do lookahead) + release exponencial
    let rel = 1.0 - (-1.0 / (0.08 * sr as f32)).exp();
    let mut g = 1f32;
    let mut acc = 0f32;
    let mut min_gain = 1f32;
    let mut ring = vec![0f32; look];
    for i in 0..n {
        let slot = i % look;
        if i >= look {
            acc -= ring[slot];
        }
        ring[slot] = ahead[i];
        acc += ahead[i];
        let target = ahead[i].min(acc / (i + 1).min(look) as f32);
        g = if target < g { target } else { g + (target - g) * rel };
        min_gain = min_gain.min(g);
        let k = pre * g;
        buf[2 * i] *= k;
        buf[2 * i + 1] *= k;
    }
    LimitStats { peak_in: pk, pre_gain: pre, min_gain }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f64 = 48_000.0;

    /// mede o ganho (dB) de um biquad com uma senoide, depois do transiente
    fn measured_db(b: &Biquad, freq: f64) -> f64 {
        let mut st = BiquadState::default();
        let n = 48_000;
        let mut pk = 0f64;
        for i in 0..n {
            let y = st.tick(b, (2.0 * PI * freq * i as f64 / SR).sin());
            if i > n / 2 {
                pk = pk.max(y.abs());
            }
        }
        lin_to_db(pk)
    }

    #[test]
    fn highpass_q_is_resonance_in_db() {
        // WebAudio: no corte, |H| = 10^(Q/20) → ganho em dB = Q
        let b = Biquad::new(FilterType::Highpass, 180.0, 0.0, 0.7, SR);
        assert!((b.response_db(180.0, SR) - 0.7).abs() < 0.01, "{}", b.response_db(180.0, SR));
        assert!((measured_db(&b, 180.0) - 0.7).abs() < 0.05);
        // 12 dB/oitava bem abaixo do corte, plano bem acima
        assert!((b.response_db(45.0, SR) - (-24.0)).abs() < 0.6, "{}", b.response_db(45.0, SR));
        assert!(b.response_db(5000.0, SR).abs() < 0.02);
        let lp = Biquad::new(FilterType::Lowpass, 600.0, 0.0, 1.0, SR);
        assert!((lp.response_db(600.0, SR) - 1.0).abs() < 0.01);
        assert!(lp.response_db(20.0, SR).abs() < 0.02);
    }

    #[test]
    fn peaking_and_shelf_gains() {
        let pk = Biquad::new(FilterType::Peaking, 3200.0, 3.0, 1.0, SR);
        assert!((pk.response_db(3200.0, SR) - 3.0).abs() < 1e-3);
        assert!((measured_db(&pk, 3200.0) - 3.0).abs() < 0.05);
        assert!(pk.response_db(100.0, SR).abs() < 0.05);
        let cut = Biquad::new(FilterType::Peaking, 350.0, -4.0, 1.2, SR);
        assert!((cut.response_db(350.0, SR) + 4.0).abs() < 1e-3);
        let ls = Biquad::new(FilterType::Lowshelf, 130.0, -3.0, 0.7, SR);
        assert!((ls.response_db(1.0, SR) + 3.0).abs() < 0.02, "{}", ls.response_db(1.0, SR));
        assert!((ls.response_db(130.0, SR) + 1.5).abs() < 0.05, "{}", ls.response_db(130.0, SR));
        assert!(ls.response_db(8000.0, SR).abs() < 0.02);
    }

    #[test]
    fn bandpass_unity_at_center() {
        let bp = Biquad::new(FilterType::Bandpass, 320.0, 0.0, 4.4, SR);
        assert!(bp.response_db(320.0, SR).abs() < 1e-3);
        // −3 dB nas bordas da banda (f0 / Q de largura)
        let bw = 320.0 / 4.4;
        let lo = (320.0f64 * 320.0 + bw * bw / 4.0).sqrt() - bw / 2.0;
        assert!((bp.response_db(lo, SR) + 3.0).abs() < 0.15, "{}", bp.response_db(lo, SR));
        assert!(measured_db(&bp, 3200.0) < -25.0);
    }

    #[test]
    fn chain_sums_bands() {
        let mut c = FilterChain::default();
        c.push(Biquad::new(FilterType::Peaking, 1000.0, 3.0, 1.0, SR));
        c.push(Biquad::new(FilterType::Peaking, 1000.0, -3.0, 1.0, SR));
        assert_eq!(c.n, 2);
        assert!(c.response_db(1000.0, SR).abs() < 1e-3);
        for _ in 0..10 {
            c.push(Biquad::IDENTITY);
        }
        assert_eq!(c.n as usize, MAX_BANDS);
    }

    #[test]
    fn soft_clip_curve() {
        assert_eq!(soft_clip(0.5), 0.5);
        assert_eq!(soft_clip(-0.89), -0.89);
        assert!((soft_clip(0.9) - 0.9).abs() < 1e-6);
        let top = 0.9 + 0.08 * (0.1f32 / 0.08).tanh();
        assert!((soft_clip(1.0) - top).abs() < 1e-6);
        assert!((soft_clip(7.0) - top).abs() < 1e-6, "acima de 1 vale a ponta da curva");
        assert!((soft_clip(-7.0) + top).abs() < 1e-6);
        assert!(top < 0.98);
        let mut prev = -1.0;
        for i in 0..=2000 {
            let y = soft_clip(i as f32 / 1000.0 - 1.0);
            assert!(y >= prev);
            prev = y;
        }
    }

    #[test]
    fn compressor_static_curve() {
        let c = CompressorCurve::new(&CompressorParams::MASTER);
        // abaixo do threshold (−8 dB) é linear
        assert!((c.saturate(db_to_lin(-20.0)) - db_to_lin(-20.0)).abs() < 1e-12);
        // acima do joelho (−2 dB) a inclinação é 1/4
        let (a, b) = (lin_to_db(c.saturate(db_to_lin(-2.0))), lin_to_db(c.saturate(1.0)));
        assert!(((b - a) - 2.0 / 4.0).abs() < 1e-6, "{a} {b}");
        // joelho: contínuo, entre a reta 1:1 e a reta de 4:1 partindo do threshold
        assert!(a < -2.0 && a > -8.0 + 6.0 / 4.0, "{a}");
        // ganho de compensação = (1/saturate(1))^0.6 ≈ +2,5 dB
        assert!((c.makeup - (1.0 / c.saturate(1.0)).powf(0.6)).abs() < 1e-12);
        assert!(lin_to_db(c.makeup) > 2.0 && lin_to_db(c.makeup) < 4.0, "{}", lin_to_db(c.makeup));
    }

    fn steady_peak(comp: &mut Compressor, amp: f32) -> f32 {
        let n = 96_000;
        let mut pk = 0f32;
        for i in 0..n {
            let x = amp * (2.0 * PI * 440.0 * i as f64 / SR).sin() as f32;
            let (l, r) = comp.tick(x, x);
            assert_eq!(l, r);
            if i > n * 3 / 4 {
                pk = pk.max(l.abs());
            }
        }
        pk
    }

    #[test]
    fn compressor_steady_state() {
        let p = CompressorParams::MASTER;
        // sinal baixo: só o ganho de compensação
        let mut comp = Compressor::new(&p, SR);
        let makeup = comp.curve().makeup;
        let quiet = steady_peak(&mut comp, 0.05);
        assert!((quiet as f64 / (0.05 * makeup) - 1.0).abs() < 0.01, "{quiet}");
        assert!(comp.reduction_db.abs() < 0.05);
        // 0 dBFS: o pico em regime fica perto de saturate(1)·makeup (±1,5 dB de ripple do detector)
        let mut comp = Compressor::new(&p, SR);
        let loud = steady_peak(&mut comp, 1.0);
        let expect = comp.curve().saturate(1.0) * makeup;
        let err_db = lin_to_db(loud as f64 / expect);
        assert!(err_db.abs() < 1.5, "pico {loud}, esperado ~{expect} ({err_db:+.2} dB)");
        assert!(comp.reduction_db < -3.0, "{}", comp.reduction_db);
        // mais entrada → mais saída, mas comprimida (≈ 4:1 acima do joelho)
        let mut comp = Compressor::new(&p, SR);
        let mid = steady_peak(&mut comp, 0.5);
        assert!(mid < loud);
        let slope = lin_to_db((loud / mid) as f64) / 6.0206;
        assert!(slope > 0.1 && slope < 0.6, "inclinação {slope}");
    }

    #[test]
    fn compressor_lookahead_delay() {
        let mut comp = Compressor::new(&CompressorParams::MASTER, SR);
        assert_eq!(comp.latency_frames(), 288);
        let mut first = None;
        for i in 0..1000 {
            let (l, _) = comp.tick(if i == 0 { 0.01 } else { 0.0 }, 0.0);
            if l != 0.0 && first.is_none() {
                first = Some(i);
            }
        }
        assert_eq!(first, Some(288));
    }

    #[test]
    fn limiter_holds_ceiling_without_touching_quiet_parts() {
        let sr = 48_000u32;
        let n = sr as usize;
        let mut buf = vec![0f32; n * 2];
        for i in 0..n {
            let burst = if (20_000..20_200).contains(&i) { 4.0 } else { 1.0 };
            let s = 0.2 * burst * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / sr as f32).sin();
            buf[2 * i] = s;
            buf[2 * i + 1] = -s;
        }
        let stats = lookahead_limit(&mut buf, sr, -1.5);
        let ceiling = 10f32.powf(-1.5 / 20.0);
        assert!((stats.pre_gain - ceiling * 1.78 / stats.peak_in).abs() < 1e-6);
        let peak = buf.iter().fold(0f32, |a, s| a.max(s.abs()));
        assert!(peak <= ceiling * 1.0001, "pico {peak} > teto {ceiling}");
        assert!(peak > ceiling * 0.9);
        assert!(stats.min_gain < 0.6);
        // longe do pico o ganho volta a ser só o de normalização
        let i = 2_000;
        let orig = 0.2 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / sr as f32).sin();
        assert!((buf[2 * i] - orig * stats.pre_gain).abs() < 1e-4);
    }

    #[test]
    fn limiter_gain_is_capped_and_silence_is_noop() {
        let mut buf = vec![0.001f32; 2000];
        let s = lookahead_limit(&mut buf, 48_000, -1.5);
        assert_eq!(s.pre_gain, 4.0, "no máximo +12 dB");
        assert!((buf[100] - 0.004).abs() < 1e-7);
        let mut z = vec![0f32; 64];
        assert_eq!(lookahead_limit(&mut z, 48_000, -1.5).pre_gain, 1.0);
        assert!(z.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn rng_ranges() {
        let mut r = Rng::new(42);
        let (mut lo, mut hi) = (1f32, -1f32);
        for _ in 0..10_000 {
            let j = r.jitter(0.12);
            lo = lo.min(j);
            hi = hi.max(j);
            let u = r.unit();
            assert!((0.0..1.0).contains(&u));
        }
        assert!((0.88..0.89).contains(&lo) && hi <= 1.12 && hi > 1.11, "{lo} {hi}");
        assert_eq!(Rng::new(7).next_u32(), Rng::new(7).next_u32());
        assert_ne!(Rng::new(7).next_u32(), Rng::new(8).next_u32());
    }
}
