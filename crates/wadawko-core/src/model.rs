//! Modelo de dados da música (`src/lib/types.ts`), no MESMO formato JSON do app
//! web: chaves em camelCase, opcionais ausentes não são serializados, números
//! inteiros saem sem ".0" (como `JSON.stringify`) e chaves desconhecidas fazem
//! round-trip via `extra`.
//!
//! Convenções:
//! - tudo que é opcional no TS é `Option<…>` aqui (a presença/ausência da chave
//!   é preservada); para leitura confortável use os acessores (`Song::sections()`,
//!   `Track::person_ids()`, …) que devolvem fatia vazia quando ausente;
//! - `cells[trackId]` tem `beats * subdivision` entradas; `None` = silêncio;
//! - `version` é validado na leitura: só `1` é aceito (como `readSong`).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::LazyLock;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// Chaves desconhecidas de um objeto, preservadas no round-trip.
pub type Extra = BTreeMap<String, Value>;

/// Linhas paralelas à grade: `trackId → [valor por célula]` (`None` = vazio).
pub type CellRows = BTreeMap<String, Vec<Option<String>>>;

// ───────────────────────────── números no formato do JS ─────────────────────────────

/// Serializa `f64` como o `JSON.stringify`: valor inteiro sai sem parte
/// fracionária (`100`, não `100.0`); NaN/∞ viram `null`.
pub(crate) fn js_num<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
    const SAFE: f64 = 9_007_199_254_740_992.0; // 2^53
    if v.is_finite() && v.fract() == 0.0 && v.abs() < SAFE { s.serialize_i64(*v as i64) } else { s.serialize_f64(*v) }
}

pub(crate) fn js_num_opt<S: Serializer>(v: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
    match v {
        Some(x) => js_num(x, s),
        None => s.serialize_none(),
    }
}

fn version_one<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let v = Value::deserialize(d)?;
    if v.as_f64() == Some(1.0) { Ok(1) } else { Err(serde::de::Error::custom(format!("versão de música não suportada: {v} (esperado 1)"))) }
}

// ───────────────────────────── instrumentos ─────────────────────────────

/// Voz/sample base (`InstrumentId` do TS).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstrumentId {
    #[default]
    Shime,
    Chu,
    Hira,
    Nagado,
    Onagado,
    Okedo,
    Odaiko,
    Kane,
}

impl InstrumentId {
    /// Os 8 instrumentos base, na ordem do TS (`Object.keys(INSTRUMENTS)`).
    pub const ALL: [InstrumentId; 8] =
        [InstrumentId::Shime, InstrumentId::Chu, InstrumentId::Hira, InstrumentId::Nagado, InstrumentId::Onagado, InstrumentId::Okedo, InstrumentId::Odaiko, InstrumentId::Kane];

    /// id textual (o mesmo do JSON e dos nomes de sample): `"shime"`, `"chu"`, …
    pub fn as_str(self) -> &'static str {
        match self {
            InstrumentId::Shime => "shime",
            InstrumentId::Chu => "chu",
            InstrumentId::Hira => "hira",
            InstrumentId::Nagado => "nagado",
            InstrumentId::Onagado => "onagado",
            InstrumentId::Okedo => "okedo",
            InstrumentId::Odaiko => "odaiko",
            InstrumentId::Kane => "kane",
        }
    }

    /// Nome de exibição (`INSTRUMENTS[id]` do TS).
    pub fn label(self) -> &'static str {
        match self {
            InstrumentId::Shime => "Shime-daiko",
            InstrumentId::Chu => "Chū-daiko",
            InstrumentId::Hira => "Hira-daiko pequeno",
            InstrumentId::Nagado => "Nagadō-daiko",
            InstrumentId::Onagado => "Ō-nagadō",
            InstrumentId::Okedo => "Okedō-daiko",
            InstrumentId::Odaiko => "Ōdaiko",
            InstrumentId::Kane => "Kane",
        }
    }

    pub fn parse(s: &str) -> Option<InstrumentId> {
        Self::ALL.into_iter().find(|i| i.as_str() == s)
    }
}

impl fmt::Display for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Tipo do filtro de timbre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToneType {
    Lowpass,
    Highpass,
    Bandpass,
}

/// Filtro de timbre aplicado sobre o sample base.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentTone {
    #[serde(rename = "type")]
    pub kind: ToneType,
    /// frequência de corte em Hz
    #[serde(serialize_with = "js_num")]
    pub cutoff: f64,
    /// ressonância base do filtro
    #[serde(serialize_with = "js_num")]
    pub q: f64,
}

/// Ajustes de som de uma zona (couro ou borda).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneParams {
    /// transposição em semitons (0 = base) → playbackRate = 2^(pitch/12)
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "js_num_opt")]
    pub pitch: Option<f64>,
    /// trim de volume em dB (0 = base)
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "js_num_opt")]
    pub gain: Option<f64>,
    /// filtro de timbre (ausente = sem filtro)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<InstrumentTone>,
    /// 0..1 → realce de ressonância/overtones (soma no Q do filtro)
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "js_num_opt")]
    pub overtones: Option<f64>,
    /// 0..1 → encurta (<0.5) ou alonga (>0.5) o sample; 0.5 = natural
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "js_num_opt")]
    pub decay: Option<f64>,
}

/// Zona do tambor: couro (pele) ou borda (fuchi).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Zone {
    Head,
    Rim,
}

/// Definição de instrumento: um sample base + ajustes de som SEPARADOS por zona.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentDef {
    pub id: String,
    pub name: String,
    /// voz/sample base
    pub base: InstrumentId,
    /// def padrão (constante no código). Defs duplicadas pelo app gravam `false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builtin: Option<bool>,
    /// ajustes do couro (pele)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<ZoneParams>,
    /// ajustes da borda (fuchi/rim)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rim: Option<ZoneParams>,
    /// cor do instrumento (`#rrggbb`); as linhas sem cor própria herdam dela
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl InstrumentDef {
    pub fn is_builtin(&self) -> bool {
        self.builtin == Some(true)
    }

    pub fn zone(&self, zone: Zone) -> Option<&ZoneParams> {
        match zone {
            Zone::Head => self.head.as_ref(),
            Zone::Rim => self.rim.as_ref(),
        }
    }

    pub fn zone_mut(&mut self, zone: Zone) -> &mut Option<ZoneParams> {
        match zone {
            Zone::Head => &mut self.head,
            Zone::Rim => &mut self.rim,
        }
    }
}

fn zone(pitch: Option<f64>, gain: Option<f64>, tone: Option<(ToneType, f64, f64)>, overtones: Option<f64>, decay: Option<f64>) -> Option<ZoneParams> {
    Some(ZoneParams { pitch, gain, tone: tone.map(|(kind, cutoff, q)| InstrumentTone { kind, cutoff, q }), overtones, decay })
}

/// Defs padrão sempre disponíveis: as 8 base (id = id do sample) + 5 presets.
/// Equivale a `BUILTIN_INSTRUMENTS` do TS (mesma ordem).
pub static BUILTIN_INSTRUMENTS: LazyLock<Vec<InstrumentDef>> = LazyLock::new(|| {
    let mut v: Vec<InstrumentDef> =
        InstrumentId::ALL.into_iter().map(|base| InstrumentDef { id: base.as_str().into(), name: base.label().into(), base, builtin: Some(true), ..Default::default() }).collect();
    let preset = |id: &str, name: &str, base, head, rim| InstrumentDef { id: id.into(), name: name.into(), base, builtin: Some(true), head, rim, color: None, extra: Extra::new() };
    v.push(preset(
        "preset-shime-agudo",
        "Shime agudo",
        InstrumentId::Shime,
        zone(Some(7.0), None, Some((ToneType::Highpass, 900.0, 1.0)), None, Some(0.4)),
        zone(Some(7.0), Some(-2.0), None, None, None),
    ));
    v.push(preset("preset-chu-grave", "Chū grave", InstrumentId::Chu, zone(Some(-5.0), Some(1.0), None, None, Some(0.7)), zone(Some(-3.0), None, None, None, None)));
    v.push(preset(
        "preset-odaiko-profundo",
        "Ōdaiko profundo",
        InstrumentId::Odaiko,
        zone(Some(-7.0), Some(2.0), Some((ToneType::Lowpass, 600.0, 1.0)), None, Some(0.85)),
        zone(Some(-4.0), Some(-2.0), None, None, None),
    ));
    v.push(preset(
        "preset-nagado-anelado",
        "Nagadō anelado",
        InstrumentId::Nagado,
        zone(None, Some(8.0), Some((ToneType::Bandpass, 320.0, 1.2)), Some(0.4), None),
        zone(None, None, Some((ToneType::Bandpass, 2200.0, 2.0)), Some(0.5), None),
    ));
    v.push(preset(
        "preset-okedo-brilhante",
        "Okedō brilhante",
        InstrumentId::Okedo,
        zone(Some(3.0), Some(3.0), Some((ToneType::Highpass, 380.0, 1.2)), None, Some(0.45)),
        zone(Some(5.0), None, None, Some(0.5), None),
    ));
    v
});

// ───────────────────────────── pessoas ─────────────────────────────

/// Pessoa que toca um ou mais instrumentos (agrupamento de tracks).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub id: String,
    pub name: String,
    /// cor do grupo (hex); default = ciclo de `PERSON_COLORS` ao criar
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// mudo do submix da pessoa
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted: Option<bool>,
    /// solo do submix da pessoa
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solo: Option<bool>,
    /// volume do barramento da pessoa em dB (default 0)
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "js_num_opt")]
    pub volume: Option<f64>,
    /// voz dos kiai da pessoa: "m" (masculina) ou "f" (feminina); sem valor = sorteada pelo id
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Person {
    pub fn is_muted(&self) -> bool {
        self.muted == Some(true)
    }
    pub fn is_solo(&self) -> bool {
        self.solo == Some(true)
    }
}

/// Paleta de cores das pessoas — ciclada ao criar.
pub const PERSON_COLORS: [&str; 8] = [
    "#e8663d", // laranja-terracota
    "#3d7ee8", // azul
    "#2fae6b", // verde
    "#b154d6", // roxo
    "#d6a319", // âmbar
    "#d64577", // rosa
    "#1ba7a7", // teal
    "#8a6d3b", // marrom
];

// ───────────────────────────── linhas ─────────────────────────────

/// Ajustes da voz de kiai de uma linha.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KyaiVoice {
    /// transposição em semitons (soma ao tom da pessoa)
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "js_num_opt")]
    pub pitch: Option<f64>,
    /// 0..1 — quanto cada grito varia (tom, contorno, tempo, duração)
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "js_num_opt")]
    pub vary: Option<f64>,
    /// volume dos gritos em dB (default 0)
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "js_num_opt")]
    pub gain: Option<f64>,
    /// voz das gravações: "m" ou "f" (vale para a linha toda; a da pessoa tem prioridade)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
}

/// Linha (instrumento) da grade.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub name: String,
    /// instrumento base / fallback se a def custom for deletada
    pub instrument: InstrumentId,
    /// ref à `InstrumentDef` (builtin id ou custom id)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrument_id: Option<String>,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub solo: bool,
    /// volume do canal em dB (default -6, ver `Track::DEFAULT_VOLUME_DB`)
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "js_num_opt")]
    pub volume: Option<f64>,
    /// panorama -1 (esq) .. 1 (dir), default 0
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "js_num_opt")]
    pub pan: Option<f64>,
    /// pessoas que tocam este instrumento; ausente/[] = sem pessoa. ≥2 = compartilhado
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub person_ids: Option<Vec<String>>,
    /// voz dos kiai desta linha (soma à voz própria de cada pessoa)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kyai: Option<KyaiVoice>,
    /// cor da linha (hex `#rrggbb`); ausente = cor padrão do instrumento base
    /// ([`default_track_color`]). Campo opcional: músicas antigas abrem igual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Paleta de cores das linhas (12), escolhidas para contrastar entre si sobre
/// o cinza escuro e o claro do app. Uma linha sem `color` usa a cor do seu
/// instrumento base ([`default_track_color`]).
pub const TRACK_COLORS: [&str; 12] = [
    "#e8603f", // coral
    "#f08a2e", // laranja
    "#e6c33f", // amarelo
    "#8dc152", // lima
    "#2fae6b", // verde
    "#2cb7a4", // teal
    "#3d9be9", // azul
    "#6f7ce8", // índigo
    "#ab63d9", // roxo (o #a35ad6 do plano dava 4,38 de contraste com o texto escuro)
    "#e0569b", // magenta
    "#b08968", // castanho
    "#8d99a6", // ardósia
];

/// Cor padrão (determinística) de uma linha pelo instrumento base.
pub fn default_track_color(base: InstrumentId) -> &'static str {
    match base {
        InstrumentId::Shime => TRACK_COLORS[2],    // amarelo
        InstrumentId::Chu => TRACK_COLORS[0],      // coral
        InstrumentId::Nagado => TRACK_COLORS[1],   // laranja
        InstrumentId::Onagado => TRACK_COLORS[10], // castanho
        InstrumentId::Okedo => TRACK_COLORS[5],    // teal
        InstrumentId::Odaiko => TRACK_COLORS[8],   // roxo
        InstrumentId::Hira => TRACK_COLORS[4],     // verde
        InstrumentId::Kane => TRACK_COLORS[6],     // azul
    }
}

impl Track {
    /// volume do canal quando `volume` está ausente (dB)
    pub const DEFAULT_VOLUME_DB: f64 = -6.0;

    /// Cor efetiva da linha: a própria, ou a padrão do instrumento base.
    pub fn color_or_default(&self) -> &str {
        self.color.as_deref().unwrap_or_else(|| default_track_color(self.instrument))
    }

    pub fn new(id: impl Into<String>, name: impl Into<String>, instrument: InstrumentId) -> Self {
        Track { id: id.into(), name: name.into(), instrument, ..Default::default() }
    }

    /// pessoas da linha (vazio quando ausente)
    pub fn person_ids(&self) -> &[String] {
        self.person_ids.as_deref().unwrap_or(&[])
    }

    /// true se o instrumento é compartilhado (2+ pessoas) — `isShared` do TS
    pub fn is_shared(&self) -> bool {
        self.person_ids().len() >= 2
    }

    /// id da def efetiva: `instrumentId ?? instrument`
    pub fn def_id(&self) -> &str {
        self.instrument_id.as_deref().unwrap_or(self.instrument.as_str())
    }
}

// ───────────────────────────── compassos ─────────────────────────────

/// Marcação de dinâmica.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dynamic {
    Pp,
    P,
    Mp,
    Mf,
    F,
    Ff,
}

impl Dynamic {
    /// do mais fraco ao mais forte (ordem das chaves de `DYNAMIC_LEVELS`)
    pub const ALL: [Dynamic; 6] = [Dynamic::Pp, Dynamic::P, Dynamic::Mp, Dynamic::Mf, Dynamic::F, Dynamic::Ff];

    /// Multiplicador de ganho (`DYNAMIC_LEVELS`); mf = 1 (neutro). Degraus de
    /// ~3–6 dB (pp −12, p −6, mp −3, f +6, ff +10).
    pub fn level(self) -> f64 {
        match self {
            Dynamic::Pp => 0.25,
            Dynamic::P => 0.5,
            Dynamic::Mp => 0.71,
            Dynamic::Mf => 1.0,
            Dynamic::F => 2.0,
            Dynamic::Ff => 3.2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Dynamic::Pp => "pp",
            Dynamic::P => "p",
            Dynamic::Mp => "mp",
            Dynamic::Mf => "mf",
            Dynamic::F => "f",
            Dynamic::Ff => "ff",
        }
    }

    pub fn parse(s: &str) -> Option<Dynamic> {
        Self::ALL.into_iter().find(|d| d.as_str() == s)
    }

    /// posição em `ALL` (0 = pp … 5 = ff)
    pub fn index(self) -> usize {
        self as usize
    }
}

/// Crescendo/diminuendo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Hairpin {
    Cresc,
    Dim,
}

/// Marcação de dinâmica de um instrumento num compasso (`trackDynamics[trackId]`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicMark {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<Dynamic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hairpin: Option<Hairpin>,
}

fn one() -> usize {
    1
}

/// Mudança de andamento a partir do INÍCIO do compasso: o andamento vai, em linha
/// reta, do valor atual até `target` BPM ao FIM de `span` compassos tocados
/// (`span = 0` = troca imediata, "a tempo"). Alvo abaixo do atual = rallentando;
/// acima = accelerando. Depois, o andamento fica no alvo até a próxima marcação.
/// `target` é absoluto (o BPM que se ouve com o andamento da música sem treino).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TempoMark {
    #[serde(serialize_with = "js_num")]
    pub target: f64,
    #[serde(default = "one")]
    pub span: usize,
}

/// Compasso. `cells[trackId]` tem `beats * subdivision` entradas.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measure {
    pub beats: usize,
    pub subdivision: usize,
    pub cells: CellRows,
    /// paralelo a `cells`; personId dono de cada célula. Só usado em track compartilhada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell_owners: Option<CellRows>,
    /// marcação de dinâmica no início do compasso; vale até a próxima
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<Dynamic>,
    /// crescendo/diminuendo ao longo do compasso (encadeia até a próxima marcação)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hairpin: Option<Hairpin>,
    /// dinâmica por instrumento (trackId): tem prioridade sobre a geral neste compasso
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track_dynamics: Option<BTreeMap<String, DynamicMark>>,
    /// kiai/kakegoe por linha, paralelo a `cells`: UMA sílaba por célula ("so", "re!")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kyai: Option<CellRows>,
    /// efeitos por célula, paralelo a `cells` (`"R4< D+1"`, ver `crate::fx`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fx: Option<CellRows>,
    /// mudança de andamento que começa neste compasso (rallentando/accelerando/a tempo)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tempo: Option<TempoMark>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Measure {
    /// Compasso vazio com uma linha de silêncio para cada `track_ids`.
    pub fn new<'a>(beats: usize, subdivision: usize, track_ids: impl IntoIterator<Item = &'a str>) -> Self {
        let n = beats * subdivision;
        Measure { beats, subdivision, cells: track_ids.into_iter().map(|id| (id.to_string(), vec![None; n])).collect(), ..Default::default() }
    }

    /// `cellsPerMeasure` do TS: `beats * subdivision`.
    pub fn cell_count(&self) -> usize {
        self.beats * self.subdivision
    }

    /// valor da célula (`None` = silêncio, linha ausente ou fora do compasso)
    pub fn cell(&self, track_id: &str, ci: usize) -> Option<&str> {
        self.cells.get(track_id)?.get(ci)?.as_deref()
    }

    /// sílaba de kiai da célula
    pub fn kyai_at(&self, track_id: &str, ci: usize) -> Option<&str> {
        self.kyai.as_ref()?.get(track_id)?.get(ci)?.as_deref()
    }

    /// texto de efeitos da célula (`"R4<"`), se houver
    pub fn fx_at(&self, track_id: &str, ci: usize) -> Option<&str> {
        self.fx.as_ref()?.get(track_id)?.get(ci)?.as_deref().filter(|s| !s.is_empty())
    }

    /// dono CRU gravado em `cellOwners` (sem fallback; ver `ops::people::cell_owner_of`)
    pub fn raw_owner(&self, track_id: &str, ci: usize) -> Option<&str> {
        self.cell_owners.as_ref()?.get(track_id)?.get(ci)?.as_deref()
    }

    /// marcação própria do instrumento neste compasso
    pub fn track_dynamic(&self, track_id: &str) -> Option<&DynamicMark> {
        self.track_dynamics.as_ref()?.get(track_id)
    }
}

// ───────────────────────────── seções ─────────────────────────────

/// Divisão nomeada da música: começa no compasso `start`, vai até a próxima.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: String,
    pub name: String,
    pub start: usize,
    /// quantas vezes a seção toca (ritornello); default 1
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<u32>,
    /// cor da seção (`#rrggbb`); sem cor herda a do grupo
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// grupo (`SectionGroup.id`) a que a seção pertence
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Section {
    /// vezes que a seção toca: `max(1, repeat ?? 1)`
    pub fn times(&self) -> u32 {
        self.repeat.unwrap_or(1).max(1)
    }
}

/// Grupo de seções (organização, cor e região — NÃO muda a ordem tocada;
/// `docs/planejamento/tracker.md` §3.5). As seções do grupo são contíguas.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionGroup {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// recolhido no painel de seções (estado de UI salvo no arquivo)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub collapsed: bool,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Paleta de cores de estrutura (seções e grupos): mais suaves que as das linhas.
pub const SECTION_COLORS: [&str; 8] = ["#d9775f", "#d9a441", "#8fb86a", "#4fb3a0", "#5e9fd8", "#8c86d6", "#c97aa8", "#9a8f84"];

// ───────────────────────────── trechos de ensaio ─────────────────────────────

/// Ponta de um trecho de ensaio: compasso `bar` contado dentro da seção
/// `section` (id; vazio = música sem seções, conta do início). Ancorar na
/// seção faz o trecho acompanhar mover/clonar seções e acrescentar compassos.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PracticeAnchor {
    #[serde(default)]
    pub section: String,
    pub bar: usize,
}

/// Trecho de ensaio com nome (F7b: "o solo do Ōdaiko"): `start..=end`, as duas
/// pontas inclusivas.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PracticeSpot {
    pub id: String,
    pub name: String,
    pub start: PracticeAnchor,
    pub end: PracticeAnchor,
    #[serde(flatten)]
    pub extra: Extra,
}

// ───────────────────────────── música ─────────────────────────────

/// A música inteira (formato `version: 1`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    /// sempre 1; outro valor é erro de leitura
    #[serde(deserialize_with = "version_one")]
    pub version: u32,
    #[serde(default)]
    pub title: String,
    #[serde(serialize_with = "js_num")]
    pub tempo: f64,
    /// 0 = reto; 1 = swing total (colcheia atrasada pro terço, estilo matsuri/hane)
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "js_num_opt")]
    pub swing: Option<f64>,
    pub tracks: Vec<Track>,
    pub measures: Vec<Measure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sections: Option<Vec<Section>>,
    /// pessoas; a ordem define a ordem dos trilhos de agrupamento
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub people: Option<Vec<Person>>,
    /// instrumentos custom desta música (builtins ficam em `BUILTIN_INSTRUMENTS`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruments: Option<Vec<InstrumentDef>>,
    /// "main": instrumentos em destaque (+6 dB, como f); o resto recua (−6 dB, como p)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_track_ids: Option<Vec<String>>,
    /// formato antigo (um só instrumento em destaque); lido como `[mainTrackId]`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_track_id: Option<String>,
    /// trechos de ensaio com nome (F7b)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub practice: Option<Vec<PracticeSpot>>,
    /// grupos de seções (tracker §3.5)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_groups: Option<Vec<SectionGroup>>,
    /// cor dos instrumentos PRONTOS nesta música (id da def → `#rrggbb`); os
    /// custom guardam a cor na própria def (`InstrumentDef.color`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrument_colors: Option<BTreeMap<String, String>>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for Song {
    fn default() -> Self {
        Song {
            version: 1,
            title: String::new(),
            tempo: 100.0,
            swing: None,
            tracks: Vec::new(),
            measures: Vec::new(),
            sections: None,
            people: None,
            instruments: None,
            main_track_ids: None,
            main_track_id: None,
            practice: None,
            section_groups: None,
            instrument_colors: None,
            extra: Extra::new(),
        }
    }
}

/// Erro de leitura de música.
#[derive(Debug)]
pub enum SongError {
    /// o texto não é JSON
    InvalidJson(serde_json::Error),
    /// JSON válido, mas não é uma música `version: 1` (ou falta `tracks`/`measures`)
    NotASong(String),
}

impl fmt::Display for SongError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // mesmas mensagens do importSong
            SongError::InvalidJson(_) => f.write_str("JSON inválido."),
            SongError::NotASong(_) => f.write_str("Arquivo não parece ser uma música do taiko-fu (version 1)."),
        }
    }
}

impl std::error::Error for SongError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SongError::InvalidJson(e) => Some(e),
            SongError::NotASong(_) => None,
        }
    }
}

impl SongError {
    /// detalhe técnico (mensagem do serde), para log
    pub fn detail(&self) -> String {
        match self {
            SongError::InvalidJson(e) => e.to_string(),
            SongError::NotASong(d) => d.clone(),
        }
    }
}

/// Extensão nova dos arquivos exportados.
pub const FILE_EXT: &str = ".wadawko.json";
/// Extensão do app web (aceita na importação; usada por `Song::export_file_name`).
pub const LEGACY_FILE_EXT: &str = ".taiko.json";

impl Song {
    /// Lê uma música de JSON (projeto salvo ou arquivo `.taiko.json`/`.wadawko.json`).
    /// Equivale a `readSong`/`importSong`: exige `version: 1`, `tracks` e `measures`.
    pub fn from_json(src: &str) -> Result<Song, SongError> {
        let value: Value = serde_json::from_str(src).map_err(SongError::InvalidJson)?;
        Self::from_value(value)
    }

    /// Como `from_json`, a partir de um `Value` já parseado.
    pub fn from_value(value: Value) -> Result<Song, SongError> {
        serde_json::from_value(value).map_err(|e| SongError::NotASong(e.to_string()))
    }

    /// JSON compacto (o que vai para o banco: `JSON.stringify(song)`).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Song sempre serializa")
    }

    /// JSON indentado com 2 espaços (arquivo exportado: `JSON.stringify(song, null, 2)`).
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("Song sempre serializa")
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).expect("Song sempre serializa")
    }

    /// Parte do nome de arquivo derivada do título: minúsculas, espaços → `-`;
    /// título vazio vira `"musica"` (como em `exportSong`).
    pub fn slug(&self) -> String {
        let lower = self.title.to_lowercase();
        let mut out = String::with_capacity(lower.len());
        let mut in_ws = false;
        for c in lower.chars() {
            if crate::js_space(c) {
                if !in_ws {
                    out.push('-');
                }
                in_ws = true;
            } else {
                out.push(c);
                in_ws = false;
            }
        }
        if out.is_empty() { "musica".into() } else { out }
    }

    /// Nome do arquivo exportado pelo app web: `<slug>.taiko.json`.
    pub fn export_file_name(&self) -> String {
        format!("{}{}", self.slug(), LEGACY_FILE_EXT)
    }

    /// seções na ordem gravada (vazio quando ausente). Ordenadas: `ops::sections::section_ranges`.
    pub fn sections(&self) -> &[Section] {
        self.sections.as_deref().unwrap_or(&[])
    }

    pub fn people(&self) -> &[Person] {
        self.people.as_deref().unwrap_or(&[])
    }

    /// instrumentos custom da música (sem os builtins)
    pub fn instruments(&self) -> &[InstrumentDef] {
        self.instruments.as_deref().unwrap_or(&[])
    }

    /// swing efetivo (`swing ?? 0`)
    pub fn swing(&self) -> f64 {
        self.swing.unwrap_or(0.0)
    }

    pub fn track(&self, id: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: &str) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    pub fn track_index(&self, id: &str) -> Option<usize> {
        self.tracks.iter().position(|t| t.id == id)
    }

    pub fn person(&self, id: &str) -> Option<&Person> {
        self.people().iter().find(|p| p.id == id)
    }

    pub fn person_mut(&mut self, id: &str) -> Option<&mut Person> {
        self.people.as_mut()?.iter_mut().find(|p| p.id == id)
    }

    pub fn section(&self, id: &str) -> Option<&Section> {
        self.sections().iter().find(|s| s.id == id)
    }

    /// total de células (colunas) da música — `totalCols` do TS
    pub fn total_cells(&self) -> usize {
        self.measures.iter().map(Measure::cell_count).sum()
    }
}

// ───────────────────────────── workspace (chave `wadawko:workspace`) ─────────────────────────────

/// Chave do workspace no banco `kv`.
pub const WORKSPACE_KEY: &str = "wadawko:workspace";
/// Prefixo das chaves de projeto (`wadawko:project:<id>`).
pub const PROJECT_KEY_PREFIX: &str = "wadawko:project:";
/// Chave do tema (`"light"` | `"dark"`).
pub const THEME_KEY: &str = "wadawko:theme";

/// Chave `kv` de um projeto.
pub fn project_key(id: &str) -> String {
    format!("{PROJECT_KEY_PREFIX}{id}")
}

/// Entrada da lista de músicas do workspace.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeta {
    pub id: String,
    pub title: String,
    /// `Date.now()` da última gravação (ms desde a época)
    pub updated_at: i64,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Workspace multi-música (`version: 2`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub version: u32,
    pub projects: Vec<ProjectMeta>,
    pub current_id: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for Workspace {
    fn default() -> Self {
        Workspace { version: 2, projects: Vec::new(), current_id: None, extra: Extra::new() }
    }
}

impl Workspace {
    /// Como `readWorkspace`: `None` se não for JSON, não tiver `version: 2` ou `projects`.
    pub fn from_json(src: &str) -> Option<Workspace> {
        let ws: Workspace = serde_json::from_str(src).ok()?;
        (ws.version == 2).then_some(ws)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Workspace sempre serializa")
    }
}
