//! Manifesto estático do banco de samples — porte item a item de
//! `src/lib/audio.ts` (`SAMPLE_FILES`, `TUNING`, `TAIL`, `EQ`, `makeLayers`).

use std::fmt;
use std::str::FromStr;

/// Os 8 instrumentos base (`InstrumentId` do web app).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BaseInstrument {
    Shime,
    Chu,
    Hira,
    Nagado,
    Onagado,
    Okedo,
    Odaiko,
    Kane,
}

impl BaseInstrument {
    pub const COUNT: usize = 8;
    pub const ALL: [BaseInstrument; 8] =
        [BaseInstrument::Shime, BaseInstrument::Chu, BaseInstrument::Hira, BaseInstrument::Nagado, BaseInstrument::Onagado, BaseInstrument::Okedo, BaseInstrument::Odaiko, BaseInstrument::Kane];

    /// id usado no JSON das músicas ("shime", "chu", …).
    pub const fn as_str(self) -> &'static str {
        match self {
            BaseInstrument::Shime => "shime",
            BaseInstrument::Chu => "chu",
            BaseInstrument::Hira => "hira",
            BaseInstrument::Nagado => "nagado",
            BaseInstrument::Onagado => "onagado",
            BaseInstrument::Okedo => "okedo",
            BaseInstrument::Odaiko => "odaiko",
            BaseInstrument::Kane => "kane",
        }
    }

    /// índice estável 0..8 (ordem de [`BaseInstrument::ALL`]).
    pub const fn index(self) -> usize {
        self as usize
    }

    /// dados estáticos do instrumento.
    pub fn manifest(self) -> &'static InstrumentManifest {
        &MANIFEST[self.index()]
    }
}

/// id de instrumento desconhecido.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownInstrument(pub String);

impl fmt::Display for UnknownInstrument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "instrumento base desconhecido: {:?}", self.0)
    }
}
impl std::error::Error for UnknownInstrument {}

impl FromStr for BaseInstrument {
    type Err = UnknownInstrument;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        BaseInstrument::ALL.into_iter().find(|i| i.as_str() == s).ok_or_else(|| UnknownInstrument(s.to_string()))
    }
}

impl fmt::Display for BaseInstrument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Zona do tambor: couro (`head`) ou borda (`rim`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Zone {
    Head,
    Rim,
}

impl Zone {
    pub const ALL: [Zone; 2] = [Zone::Head, Zone::Rim];

    pub const fn as_str(self) -> &'static str {
        match self {
            Zone::Head => "head",
            Zone::Rim => "rim",
        }
    }
    pub const fn index(self) -> usize {
        self as usize
    }
    pub const fn is_rim(self) -> bool {
        matches!(self, Zone::Rim)
    }
}

impl FromStr for Zone {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "head" => Ok(Zone::Head),
            "rim" => Ok(Zone::Rim),
            _ => Err(format!("zona desconhecida: {s:?}")),
        }
    }
}

/// Camadas de dinâmica (suave → forte); cada camada = nomes dos round robins
/// (sem extensão; arquivo = `<nome>.ogg`).
pub type Layers = &'static [&'static [&'static str]];

/// `TUNING`: equilíbrio entre instrumentos (ganho linear e playbackRate por zona).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    pub head_gain: f32,
    pub rim_gain: f32,
    pub rim_rate: f32,
    pub head_rate: f32,
}

impl Tuning {
    pub const fn gain(&self, zone: Zone) -> f32 {
        match zone {
            Zone::Head => self.head_gain,
            Zone::Rim => self.rim_gain,
        }
    }
    pub const fn rate(&self, zone: Zone) -> f32 {
        match zone {
            Zone::Head => self.head_rate,
            Zone::Rim => self.rim_rate,
        }
    }
}

/// `TAIL`: cauda máxima (s) por zona e duração (s) do abafamento no choke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tail {
    pub head: f32,
    pub rim: f32,
    pub choke: f32,
}

impl Tail {
    /// cauda máxima (s) da zona.
    pub const fn max(&self, zone: Zone) -> f32 {
        match zone {
            Zone::Head => self.head,
            Zone::Rim => self.rim,
        }
    }
}

/// Tipo de filtro das bandas de `EQ` (subconjunto de `BiquadFilterType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FilterKind {
    Highpass,
    Lowshelf,
    Peaking,
}

/// Uma banda de EQ `[tipo, freq, ganho dB, Q]`. No web app cada banda é um
/// `Tone.Filter` com `rolloff: -12` (um biquad), em série na ordem da lista.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EqBand {
    pub kind: FilterKind,
    pub freq: f32,
    pub gain_db: f32,
    pub q: f32,
}

/// Tudo o que `audio.ts` sabe estaticamente de um instrumento base.
#[derive(Clone, Copy, Debug)]
pub struct InstrumentManifest {
    pub instrument: BaseInstrument,
    pub head: Layers,
    pub rim: Layers,
    pub tuning: Tuning,
    pub tail: Tail,
    pub eq_head: &'static [EqBand],
    pub eq_rim: &'static [EqBand],
}

impl InstrumentManifest {
    pub const fn layers(&self, zone: Zone) -> Layers {
        match zone {
            Zone::Head => self.head,
            Zone::Rim => self.rim,
        }
    }
    pub const fn eq(&self, zone: Zone) -> &'static [EqBand] {
        match zone {
            Zone::Head => self.eq_head,
            Zone::Rim => self.eq_rim,
        }
    }
}

// ───────────────────────────── seleção de camada ─────────────────────────────

/// `maxVel` da camada `i` de `n` (`makeLayers`): o range de velocity é dividido
/// igualmente; a última camada vai até 1.01.
pub fn layer_max_vel(i: usize, n: usize) -> f32 {
    if i + 1 >= n { 1.01 } else { (i + 1) as f32 / n as f32 }
}

/// Velocity efetiva para a escolha da camada (`playLayered`): a dinâmica mexe
/// no ganho E na camada — `min(1, vel · √dyn)`.
pub fn layer_velocity(vel: f32, dynamic: f32) -> f32 {
    (vel * dynamic.sqrt()).min(1.0)
}

/// Índice da camada para uma velocity efetiva: a primeira com
/// `layer_vel <= maxVel`, senão a última (inclui NaN, como no TS).
/// `n_layers == 0` devolve 0.
pub fn select_layer(n_layers: usize, layer_vel: f32) -> usize {
    (0..n_layers).find(|&i| layer_vel <= layer_max_vel(i, n_layers)).unwrap_or(n_layers.saturating_sub(1))
}

// ───────────────────────────── tabelas ─────────────────────────────

/// Borda de chū — reutilizada por hira, odaiko, nagadō e ō-nagadō
/// (só shime, okedō e kane têm borda própria).
const CHU_RIM: Layers = &[&["chu-rim-0-0", "chu-rim-0-1"], &["chu-rim-1-0", "chu-rim-1-1", "chu-rim-1-2"], &["chu-rim-2-0", "chu-rim-2-1"]];

const fn tuning(head_gain: f32, rim_gain: f32, rim_rate: f32, head_rate: f32) -> Tuning {
    Tuning { head_gain, rim_gain, rim_rate, head_rate }
}
const fn tail(head: f32, rim: f32, choke: f32) -> Tail {
    Tail { head, rim, choke }
}
const fn hp(freq: f32, gain_db: f32, q: f32) -> EqBand {
    EqBand { kind: FilterKind::Highpass, freq, gain_db, q }
}
const fn ls(freq: f32, gain_db: f32, q: f32) -> EqBand {
    EqBand { kind: FilterKind::Lowshelf, freq, gain_db, q }
}
const fn pk(freq: f32, gain_db: f32, q: f32) -> EqBand {
    EqBand { kind: FilterKind::Peaking, freq, gain_db, q }
}

/// Manifesto na ordem de [`BaseInstrument::ALL`].
pub static MANIFEST: [InstrumentManifest; BaseInstrument::COUNT] = [
    InstrumentManifest {
        instrument: BaseInstrument::Shime,
        head: &[
            &["shime-head-0-0", "shime-head-0-1", "shime-head-0-2", "shime-head-0-3"],
            &["shime-head-1-0", "shime-head-1-1", "shime-head-1-2", "shime-head-1-3"],
            &["shime-head-2-0", "shime-head-2-1", "shime-head-2-2", "shime-head-2-3"],
        ],
        rim: &[&["shime-rim-0-0", "shime-rim-0-1"], &["shime-rim-1-0"], &["shime-rim-2-0", "shime-rim-2-1"]],
        tuning: tuning(1.2, 0.9, 1.0, 1.0),
        tail: tail(0.45, 0.35, 0.04),
        eq_head: &[hp(180.0, 0.0, 0.7), pk(3200.0, 3.0, 1.0)],
        eq_rim: &[hp(450.0, 0.0, 0.7)],
    },
    InstrumentManifest {
        instrument: BaseInstrument::Chu,
        head: &[&["chu-head-0-0", "chu-head-0-1"], &["chu-head-1-0", "chu-head-1-1"]],
        rim: CHU_RIM,
        tuning: tuning(1.0, 1.0, 1.0, 1.0),
        tail: tail(1.3, 0.7, 0.09),
        eq_head: &[hp(70.0, 0.0, 0.7), ls(130.0, -3.0, 0.7), pk(350.0, -4.0, 1.2), pk(2800.0, 3.0, 1.0)],
        eq_rim: &[hp(350.0, 0.0, 0.7), pk(3500.0, 2.0, 1.0)],
    },
    InstrumentManifest {
        instrument: BaseInstrument::Hira,
        head: &[&["hira-head-0-0", "hira-head-0-1", "hira-head-0-2"], &["hira-head-1-0", "hira-head-1-1", "hira-head-1-2"]],
        rim: CHU_RIM,
        tuning: tuning(1.1, 0.95, 1.12, 1.0),
        tail: tail(0.7, 0.5, 0.06),
        eq_head: &[hp(110.0, 0.0, 0.7), pk(380.0, -3.0, 1.2), pk(3000.0, 2.5, 1.0)],
        eq_rim: &[hp(400.0, 0.0, 0.7), pk(3800.0, 2.0, 1.0)],
    },
    InstrumentManifest {
        instrument: BaseInstrument::Nagado,
        head: &[&["nagado-head-0-0", "nagado-head-0-1"], &["nagado-head-1-0", "nagado-head-1-1", "nagado-head-1-2"]],
        rim: CHU_RIM,
        tuning: tuning(0.9, 1.0, 1.0, 1.0),
        tail: tail(1.8, 0.7, 0.1),
        eq_head: &[hp(60.0, 0.0, 0.7), ls(120.0, -3.0, 0.7), pk(300.0, -4.0, 1.2), pk(2500.0, 3.0, 1.0)],
        eq_rim: &[hp(350.0, 0.0, 0.7), pk(3500.0, 2.0, 1.0)],
    },
    InstrumentManifest {
        instrument: BaseInstrument::Onagado,
        head: &[&["onagado-head-0-0", "onagado-head-0-1"], &["onagado-head-1-0", "onagado-head-1-1"], &["onagado-head-2-0"]],
        rim: CHU_RIM,
        tuning: tuning(0.95, 0.95, 0.94, 1.0),
        tail: tail(2.2, 0.7, 0.12),
        eq_head: &[hp(50.0, 0.0, 0.7), ls(110.0, -2.0, 0.7), pk(260.0, -4.0, 1.2), pk(2200.0, 2.5, 1.0)],
        eq_rim: &[hp(350.0, 0.0, 0.7)],
    },
    InstrumentManifest {
        instrument: BaseInstrument::Okedo,
        head: &[
            &["okedo-head-0-0", "okedo-head-0-1", "okedo-head-0-2", "okedo-head-0-3"],
            &["okedo-head-1-0", "okedo-head-1-1", "okedo-head-1-2", "okedo-head-1-3"],
            &["okedo-head-2-0", "okedo-head-2-1", "okedo-head-2-2"],
        ],
        rim: &[&["okedo-rim-0-0", "okedo-rim-0-1", "okedo-rim-0-2", "okedo-rim-0-3"]],
        tuning: tuning(1.8, 1.3, 1.0, 1.0),
        tail: tail(0.8, 0.4, 0.06),
        eq_head: &[hp(110.0, 0.0, 0.7), pk(420.0, -3.0, 1.2), pk(3000.0, 3.0, 1.0)],
        eq_rim: &[hp(400.0, 0.0, 0.7)],
    },
    InstrumentManifest {
        instrument: BaseInstrument::Odaiko,
        head: &[&["odaiko-head-0-0", "odaiko-head-0-1"], &["odaiko-head-1-0"], &["odaiko-head-2-0"], &["odaiko-head-3-0"]],
        rim: CHU_RIM,
        tuning: tuning(1.0, 0.85, 0.9, 1.0),
        tail: tail(2.8, 0.8, 0.14),
        eq_head: &[hp(40.0, 0.0, 0.7), pk(230.0, -4.0, 1.0), pk(1800.0, 2.0, 1.0)],
        eq_rim: &[hp(350.0, 0.0, 0.7)],
    },
    InstrumentManifest {
        instrument: BaseInstrument::Kane,
        head: &[&["kane-head-0-0", "kane-head-0-1", "kane-head-0-2", "kane-head-0-3"]],
        rim: &[&["kane-rim-0-0", "kane-rim-0-1", "kane-rim-0-2", "kane-rim-0-3"]],
        tuning: tuning(0.8, 0.75, 1.0, 1.0),
        tail: tail(1.2, 0.3, 0.05),
        eq_head: &[hp(600.0, 0.0, 0.7)],
        eq_rim: &[hp(700.0, 0.0, 0.7)],
    },
];

/// Nome (sem extensão) do sample das letras do animalese (vozes dos kiai).
pub const ANIMALESE: &str = "animalese";

/// Dados estáticos do `animalese.ogg` (de `src/lib/kyai.ts`): 26 letras (a–z)
/// de [`animalese::LETTER_SEC`] s cada, em sequência.
pub mod animalese {
    /// duração (s) de cada letra no arquivo.
    pub const LETTER_SEC: f32 = 0.15;

    /// início audível (s, dentro da fatia da letra) e força relativa.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Letter {
        pub start: f32,
        pub gain: f32,
    }

    /// `LETTER[l]` do TS; `None` para letras que o kiai não usa.
    pub fn letter(l: char) -> Option<Letter> {
        let (start, gain) = match l {
            'a' => (0.0, 1.0),
            'b' => (0.0, 0.8),
            'd' => (0.02, 1.6),
            'e' => (0.0, 1.5),
            'g' => (0.01, 2.0),
            'i' => (0.008, 1.8),
            'j' => (0.02, 2.5),
            'k' => (0.03, 1.6),
            'm' => (0.0, 3.0),
            'n' => (0.0, 2.5),
            'o' => (0.0, 0.8),
            'p' => (0.02, 1.3),
            'u' => (0.0, 3.5),
            'w' => (0.02, 1.4),
            _ => return None,
        };
        Some(Letter { start, gain })
    }

    /// offset (s) do início da fatia da letra no arquivo (`idx * LETTER_SEC`).
    pub fn slice_offset(l: char) -> Option<f32> {
        l.is_ascii_lowercase().then(|| (l as u8 - b'a') as f32 * LETTER_SEC)
    }
}
