//! Modelo mínimo do WaDAWko (subconjunto de src/lib/types.ts) + leitura
//! SOMENTE LEITURA do data/wadawko.db + compilação do schedule de áudio.

use std::path::Path;

use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::audio::PPQ;
use crate::sound::{InstDef, parse_zone};

// ───────────────────────────── vocabulário (src/lib/vocabulary.ts) ─────────────────────────────

#[derive(Clone, Copy)]
pub struct Hit {
    pub offset: f32,
    pub rim: bool,
    pub vel: f32,
}

pub fn vocab(syl: &str) -> Option<&'static [Hit]> {
    Some(match syl {
        "DON" => &[Hit { offset: 0.0, rim: false, vel: 1.0 }],
        "don" => &[Hit { offset: 0.0, rim: false, vel: 0.7 }],
        "KON" => &[Hit { offset: 0.0, rim: false, vel: 1.0 }],
        "kon" => &[Hit { offset: 0.0, rim: false, vel: 0.7 }],
        "doko" => &[Hit { offset: 0.0, rim: false, vel: 0.7 }, Hit { offset: 0.5, rim: false, vel: 0.6 }],
        "doron" => &[Hit { offset: 0.0, rim: false, vel: 1.0 }, Hit { offset: 0.0, rim: false, vel: 1.0 }],
        "do" => &[Hit { offset: 0.0, rim: false, vel: 0.55 }],
        "ko" => &[Hit { offset: 0.0, rim: false, vel: 0.5 }],
        "tsu" => &[Hit { offset: 0.0, rim: false, vel: 0.05 }],
        "ku" => &[Hit { offset: 0.0, rim: false, vel: 0.05 }],
        "tsuku" => &[Hit { offset: 0.0, rim: false, vel: 0.05 }, Hit { offset: 0.5, rim: false, vel: 0.045 }],
        "KA" => &[Hit { offset: 0.0, rim: true, vel: 1.0 }],
        "ka" => &[Hit { offset: 0.0, rim: true, vel: 0.7 }],
        "ra" => &[Hit { offset: 0.0, rim: true, vel: 0.7 }],
        "kara" => &[Hit { offset: 0.0, rim: true, vel: 0.65 }, Hit { offset: 0.5, rim: true, vel: 0.55 }],
        "karan" => &[Hit { offset: 0.0, rim: true, vel: 1.0 }, Hit { offset: 0.0, rim: true, vel: 1.0 }],
        "ten" => &[Hit { offset: 0.0, rim: false, vel: 0.7 }],
        "teke" => &[Hit { offset: 0.0, rim: false, vel: 0.6 }, Hit { offset: 0.5, rim: false, vel: 0.5 }],
        "su" => &[],
        _ => return None,
    })
}

pub const PALETTE: &[(&str, &[&str])] = &[
    ("couro", &["DON", "don", "KON", "kon", "doron", "do", "ko", "doko", "tsu", "ku", "tsuku", "ten", "teke"]),
    ("fuchi", &["KA", "ka", "ra", "kara", "karan"]),
    ("pausa", &["su"]),
];

const KEYS: &[&str] = &[
    "DON", "don", "KON", "kon", "doko", "doron", "do", "ko", "tsu", "ku", "tsuku", "KA", "ka", "ra", "kara", "karan",
    "ten", "teke", "su",
];

/// Golpes de uma célula (subconjunto de phrase.ts::resolveCell): sílaba conhecida,
/// palavra colada ("dokodoko", "dondon") quebrada por backtracking, ou desconhecida
/// (1 golpe no couro). Os golpes se distribuem dentro da célula.
pub fn resolve(word: &str) -> Vec<Hit> {
    let w = word.trim_end_matches('!');
    let accent = word.ends_with('!');
    if let Some(h) = vocab(w) {
        return h.iter().map(|h| Hit { vel: if accent { 1.0 } else { h.vel }, ..*h }).collect();
    }
    fn split<'a>(s: &str, out: &mut Vec<&'static str>) -> bool {
        if s.is_empty() {
            return true;
        }
        // palavras mais longas primeiro
        let mut keys: Vec<&'static str> = KEYS.to_vec();
        keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
        for k in keys {
            let lower_ok = s.len() >= k.len() && s[..k.len()].eq_ignore_ascii_case(k);
            if lower_ok {
                out.push(k);
                if split(&s[k.len()..], out) {
                    return true;
                }
                out.pop();
            }
        }
        false
    }
    let mut parts = Vec::new();
    if split(w, &mut parts) && !parts.is_empty() {
        let upper = w.chars().next().is_some_and(|c| c.is_ascii_uppercase());
        let n = parts.len() as f32;
        let mut hits = Vec::new();
        for (i, p) in parts.iter().enumerate() {
            for h in vocab(p).unwrap_or(&[]) {
                hits.push(Hit {
                    offset: (i as f32 + h.offset) / n,
                    rim: h.rim,
                    vel: if upper || accent { 1.0 } else { h.vel },
                });
            }
        }
        return hits;
    }
    vec![Hit { offset: 0.0, rim: false, vel: 0.7 }]
}

// ───────────────────────────── música ─────────────────────────────

#[derive(Clone)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub instrument: String,
    pub instrument_id: Option<String>,
    pub muted: bool,
    pub solo: bool,
    pub volume_db: f32,
    pub pan: f32,
    /// JSON original (campos que o app nativo não edita: personIds, kyai, main…)
    pub raw: Value,
}

#[derive(Clone)]
pub struct Measure {
    pub beats: usize,
    pub subdivision: usize,
    /// [track][cell]
    pub cells: Vec<Vec<Option<String>>>,
    pub dynamic: Option<String>,
    /// JSON original (cellOwners, kyai, hairpin, trackDynamics…)
    pub raw: Value,
}

impl Measure {
    pub fn len(&self) -> usize {
        self.beats * self.subdivision
    }
    fn empty(beats: usize, sub: usize, tracks: usize) -> Self {
        Measure { beats, subdivision: sub, cells: vec![vec![None; beats * sub]; tracks], dynamic: None, raw: Value::Null }
    }
}

#[derive(Clone)]
pub struct Section {
    pub name: String,
    pub start: usize,
    pub repeat: usize,
    pub raw: Value,
}

#[derive(Clone)]
pub struct Song {
    pub title: String,
    pub tempo: f32,
    pub swing: bool,
    pub tracks: Vec<Track>,
    pub measures: Vec<Measure>,
    pub sections: Vec<Section>,
    /// InstrumentDefs customizados da música (kit)
    pub instruments: Vec<InstDef>,
    /// JSON original da música (people, mainTrackIds, instruments…): preservado no export
    pub raw: Value,
}

pub const DYNAMICS: &[&str] = &["pp", "p", "mp", "mf", "f", "ff"];

pub fn instrument_label(id: &str) -> &'static str {
    match id {
        "shime" => "Shime-daiko",
        "chu" => "Chū-daiko",
        "hira" => "Hira-daiko",
        "nagado" => "Nagadō-daiko",
        "onagado" => "Ō-nagadō",
        "okedo" => "Okedō-daiko",
        "odaiko" => "Ōdaiko",
        "kane" => "Kane",
        _ => "Taiko",
    }
}

pub const INSTRUMENTS: &[&str] = &["shime", "chu", "hira", "nagado", "onagado", "okedo", "odaiko", "kane"];

impl Song {
    pub fn new_empty(title: &str) -> Self {
        let tracks = vec![
            Track { id: "shime".into(), name: "Shime".into(), instrument: "shime".into(), instrument_id: None, muted: false, solo: false, volume_db: -12.0, pan: 0.0, raw: Value::Null },
            Track { id: "chu".into(), name: "Chū".into(), instrument: "chu".into(), instrument_id: None, muted: false, solo: false, volume_db: -6.0, pan: 0.0, raw: Value::Null },
        ];
        Song {
            title: title.into(),
            tempo: 120.0,
            swing: false,
            measures: (0..2).map(|_| Measure::empty(4, 2, tracks.len())).collect(),
            tracks,
            sections: vec![Section { name: "Seção 1".into(), start: 0, repeat: 1, raw: Value::Null }],
            instruments: Vec::new(),
            raw: Value::Null,
        }
    }

    fn from_json(v: &Value) -> Result<Self> {
        let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
        let tracks: Vec<Track> = v["tracks"]
            .as_array()
            .context("tracks")?
            .iter()
            .map(|t| Track {
                id: s(&t["id"]),
                name: s(&t["name"]),
                instrument: s(&t["instrument"]),
                instrument_id: t["instrumentId"].as_str().map(String::from),
                muted: t["muted"].as_bool().unwrap_or(false),
                solo: t["solo"].as_bool().unwrap_or(false),
                volume_db: t["volume"].as_f64().unwrap_or(-6.0) as f32,
                pan: t["pan"].as_f64().unwrap_or(0.0) as f32,
                raw: t.clone(),
            })
            .collect();
        let measures = v["measures"]
            .as_array()
            .context("measures")?
            .iter()
            .map(|m| {
                let beats = m["beats"].as_u64().unwrap_or(4) as usize;
                let sub = m["subdivision"].as_u64().unwrap_or(2) as usize;
                let n = beats * sub;
                let cells = tracks
                    .iter()
                    .map(|t| {
                        let arr = m["cells"][&t.id].as_array();
                        (0..n)
                            .map(|i| arr.and_then(|a| a.get(i)).and_then(|c| c.as_str()).filter(|c| !c.is_empty()).map(String::from))
                            .collect()
                    })
                    .collect();
                Measure { beats, subdivision: sub, cells, dynamic: m["dynamic"].as_str().map(String::from), raw: m.clone() }
            })
            .collect::<Vec<_>>();
        let mut sections: Vec<Section> = v["sections"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|x| Section {
                        name: s(&x["name"]),
                        start: x["start"].as_u64().unwrap_or(0) as usize,
                        repeat: x["repeat"].as_u64().unwrap_or(1).max(1) as usize,
                        raw: x.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        if sections.is_empty() {
            sections.push(Section { name: "Seção 1".into(), start: 0, repeat: 1, raw: Value::Null });
        }
        sections.sort_by_key(|x| x.start);
        Ok(Song {
            title: s(&v["title"]),
            tempo: v["tempo"].as_f64().unwrap_or(120.0) as f32,
            swing: v["swing"].as_f64().unwrap_or(0.0) > 0.0,
            tracks,
            measures,
            sections,
            instruments: v["instruments"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|d| InstDef {
                            id: s(&d["id"]),
                            base: s(&d["base"]),
                            head: parse_zone(&d["head"]),
                            rim: parse_zone(&d["rim"]),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            raw: v.clone(),
        })
    }

    /// Lê uma música exportada (.taiko.json / .wadawko.json): o objeto Song v1 da web.
    pub fn from_json_str(src: &str) -> Result<Self> {
        let v: Value = serde_json::from_str(src).context("JSON inválido")?;
        anyhow::ensure!(
            v["version"].as_u64() == Some(1) && v["tracks"].is_array() && v["measures"].is_array(),
            "não parece ser uma música do WaDAWko (version 1)"
        );
        Song::from_json(&v)
    }

    /// Serializa no formato da web (Song v1), partindo do JSON original e sobrescrevendo
    /// só o que o app nativo edita — o resto (pessoas, kiai, donos, hairpins…) passa intacto.
    pub fn to_json(&self) -> Value {
        use serde_json::{Map, json};
        let obj = |v: &Value| if v.is_object() { v.clone() } else { Value::Object(Map::new()) };
        let mut out = obj(&self.raw);
        out["version"] = json!(1);
        out["title"] = json!(self.title);
        out["tempo"] = json!(self.tempo);
        if self.swing || out.get("swing").is_some() {
            out["swing"] = json!(if self.swing { 1 } else { 0 });
        }
        out["tracks"] = Value::Array(
            self.tracks
                .iter()
                .map(|t| {
                    let mut o = obj(&t.raw);
                    o["id"] = json!(t.id);
                    o["name"] = json!(t.name);
                    o["instrument"] = json!(t.instrument);
                    o["muted"] = json!(t.muted);
                    o["solo"] = json!(t.solo);
                    // a web trata volume ausente como −6 dB: só escreve se já existia ou mudou
                    if t.volume_db != -6.0 || o.get("volume").is_some() {
                        o["volume"] = json!(t.volume_db);
                    }
                    if t.pan != 0.0 || o.get("pan").is_some() {
                        o["pan"] = json!(t.pan);
                    }
                    match &t.instrument_id {
                        Some(id) => o["instrumentId"] = json!(id),
                        None => {
                            if let Some(m) = o.as_object_mut() {
                                m.remove("instrumentId");
                            }
                        }
                    }
                    o
                })
                .collect(),
        );
        out["measures"] = Value::Array(
            self.measures
                .iter()
                .map(|m| {
                    let mut o = obj(&m.raw);
                    // subdivisão mudou no app nativo: índices de célula do kiai/donos não valem mais
                    if o["subdivision"].as_u64().is_some_and(|s| s as usize != m.subdivision)
                        && let Some(map) = o.as_object_mut()
                    {
                        map.remove("cellOwners");
                        map.remove("kyai");
                    }
                    o["beats"] = json!(m.beats);
                    o["subdivision"] = json!(m.subdivision);
                    let mut cells = Map::new();
                    for (t, row) in self.tracks.iter().zip(&m.cells) {
                        cells.insert(t.id.clone(), Value::Array(row.iter().map(|c| c.as_ref().map_or(Value::Null, |w| json!(w))).collect()));
                    }
                    o["cells"] = Value::Object(cells);
                    match &m.dynamic {
                        Some(d) => o["dynamic"] = json!(d),
                        None => {
                            if let Some(map) = o.as_object_mut() {
                                map.remove("dynamic");
                            }
                        }
                    }
                    o
                })
                .collect(),
        );
        // música antiga sem seções: a "Seção 1" implícita do app nativo não vai para o arquivo
        let implicit = self.raw.get("sections").is_none()
            && self.sections.len() == 1
            && self.sections[0].raw.is_null()
            && self.sections[0].start == 0
            && self.sections[0].repeat == 1
            && self.sections[0].name == "Seção 1";
        if implicit {
            return out;
        }
        out["sections"] = Value::Array(
            self.sections
                .iter()
                .enumerate()
                .map(|(i, x)| {
                    let mut o = obj(&x.raw);
                    if !o["id"].is_string() {
                        o["id"] = json!(format!("sec-native-{i}"));
                    }
                    o["name"] = json!(x.name);
                    o["start"] = json!(x.start);
                    if x.repeat > 1 {
                        o["repeat"] = json!(x.repeat);
                    } else if let Some(map) = o.as_object_mut() {
                        map.remove("repeat");
                    }
                    o
                })
                .collect(),
        );
        out
    }

    /// Nome de arquivo como a web: "minha-musica"
    pub fn slug(&self) -> String {
        let s: String = self.title.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join("-");
        if s.is_empty() { "musica".into() } else { s }
    }

    /// Compassos [a, b) da seção `s`.
    pub fn section_range(&self, s: usize) -> (usize, usize) {
        let a = self.sections[s].start.min(self.measures.len());
        let b = self.sections.get(s + 1).map(|x| x.start).unwrap_or(self.measures.len()).min(self.measures.len());
        (a, b.max(a))
    }

    /// Índice linear de célula (compassos concatenados) — base dos movimentos vim.
    pub fn flat(&self, m: usize, k: usize) -> usize {
        self.measures[..m.min(self.measures.len())].iter().map(|x| x.len()).sum::<usize>() + k
    }

    pub fn unflat(&self, g: usize) -> (usize, usize) {
        let mut acc = 0;
        for (m, meas) in self.measures.iter().enumerate() {
            if g < acc + meas.len() {
                return (m, g - acc);
            }
            acc += meas.len();
        }
        let last = self.measures.len().saturating_sub(1);
        (last, self.measures.get(last).map(|x| x.len().saturating_sub(1)).unwrap_or(0))
    }

    pub fn total_cells(&self) -> usize {
        self.measures.iter().map(|x| x.len()).sum()
    }

    pub fn cell_at(&self, t: usize, g: usize) -> Option<&Option<String>> {
        let (m, k) = self.unflat(g);
        self.measures.get(m)?.cells.get(t)?.get(k)
    }

    pub fn cell_at_mut(&mut self, t: usize, g: usize) -> Option<&mut Option<String>> {
        if g >= self.total_cells() {
            return None;
        }
        let (m, k) = self.unflat(g);
        self.measures.get_mut(m)?.cells.get_mut(t)?.get_mut(k)
    }

    pub fn section_of(&self, m: usize) -> usize {
        (0..self.sections.len()).rev().find(|&s| self.sections[s].start <= m).unwrap_or(0)
    }

    pub fn add_track(&mut self, instrument: &str) {
        let n = self.tracks.iter().filter(|t| t.instrument == instrument).count();
        let label = instrument_label(instrument);
        self.tracks.push(Track {
            id: format!("{instrument}-{}", self.tracks.len()),
            name: if n == 0 { label.to_string() } else { format!("{label} {}", n + 1) },
            instrument: instrument.into(),
            instrument_id: None,
            muted: false,
            solo: false,
            volume_db: -6.0,
            pan: 0.0,
            raw: Value::Null,
        });
        for m in &mut self.measures {
            m.cells.push(vec![None; m.len()]);
        }
    }

    pub fn remove_track(&mut self, t: usize) {
        if self.tracks.len() <= 1 {
            return;
        }
        self.tracks.remove(t);
        for m in &mut self.measures {
            m.cells.remove(t);
        }
    }

    pub fn add_section(&mut self) {
        let start = self.measures.len();
        let (beats, sub) = self.measures.last().map(|m| (m.beats, m.subdivision)).unwrap_or((4, 2));
        for _ in 0..2 {
            self.measures.push(Measure::empty(beats, sub, self.tracks.len()));
        }
        self.sections.push(Section { name: format!("Seção {}", self.sections.len() + 1), start, repeat: 1, raw: Value::Null });
    }

    pub fn clone_section(&mut self, s: usize) {
        let (a, b) = self.section_range(s);
        let copy: Vec<Measure> = self.measures[a..b].to_vec();
        let n = copy.len();
        self.measures.splice(b..b, copy);
        for x in self.sections.iter_mut().skip(s + 1) {
            x.start += n;
        }
        let name = format!("{} (cópia)", self.sections[s].name);
        self.sections.insert(s + 1, Section { name, start: b, repeat: self.sections[s].repeat, raw: Value::Null });
    }

    pub fn remove_section(&mut self, s: usize) {
        if self.sections.len() <= 1 {
            return;
        }
        let (a, b) = self.section_range(s);
        self.measures.drain(a..b);
        self.sections.remove(s);
        for x in self.sections.iter_mut().skip(s) {
            x.start -= b - a;
        }
    }

    /// +/− compasso no fim da seção.
    pub fn add_measure(&mut self, s: usize, delta: i32) {
        let (a, b) = self.section_range(s);
        if delta > 0 {
            let (beats, sub) = self.measures.get(b.saturating_sub(1)).map(|m| (m.beats, m.subdivision)).unwrap_or((4, 2));
            self.measures.insert(b, Measure::empty(beats, sub, self.tracks.len()));
            for x in self.sections.iter_mut().skip(s + 1) {
                x.start += 1;
            }
        } else if b - a > 1 {
            self.measures.remove(b - 1);
            for x in self.sections.iter_mut().skip(s + 1) {
                x.start -= 1;
            }
        }
    }

    /// setSubdivision com reamostragem (célula i → i·new/old quando cai em célula inteira).
    pub fn set_subdivision(&mut self, sub: usize) {
        for m in &mut self.measures {
            let old = m.subdivision;
            if old == sub {
                continue;
            }
            let n = m.beats * sub;
            for row in &mut m.cells {
                let mut out = vec![None; n];
                for (i, c) in row.iter().enumerate() {
                    if c.is_none() {
                        continue;
                    }
                    let num = i * sub;
                    if num % old == 0 && num / old < n {
                        out[num / old] = c.clone();
                    }
                }
                *row = out;
            }
            m.subdivision = sub;
        }
    }

    /// Ordem de execução (desdobra ritornellos): (tick inicial, compasso).
    pub fn play_order(&self) -> (Vec<(u64, usize)>, u64) {
        let mut out = Vec::new();
        let mut tick = 0u64;
        for s in 0..self.sections.len() {
            let (a, b) = self.section_range(s);
            for _ in 0..self.sections[s].repeat {
                for m in a..b {
                    out.push((tick, m));
                    tick += self.measures[m].beats as u64 * PPQ;
                }
            }
        }
        (out, tick.max(PPQ))
    }

    pub fn first_tick_of(&self, measure: usize, cell: usize) -> u64 {
        let (order, _) = self.play_order();
        let m = &self.measures[measure.min(self.measures.len().saturating_sub(1))];
        order.iter().find(|(_, mm)| *mm == measure).map(|(t, _)| *t).unwrap_or(0) + (cell as u64 * PPQ) / m.subdivision as u64
    }

    pub fn audible(&self, t: usize) -> bool {
        let any_solo = self.tracks.iter().any(|x| x.solo);
        let tr = &self.tracks[t];
        !tr.muted && (!any_solo || tr.solo)
    }
}

// ───────────────────────────── workspace (SQLite, só leitura) ─────────────────────────────

pub struct ProjectEntry {
    pub id: String,
    pub updated_ms: i64,
    pub song: Song,
}

pub fn load_workspace(db: &Path) -> Result<(Vec<ProjectEntry>, usize)> {
    use rusqlite::{Connection, OpenFlags};
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .with_context(|| format!("abrir {}", db.display()))?;
    let get = |key: &str| -> Option<String> {
        conn.query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get::<_, String>(0)).ok()
    };
    let ws: Value = serde_json::from_str(&get("wadawko:workspace").context("sem workspace")?)?;
    let current = ws["currentId"].as_str().unwrap_or_default().to_string();
    let mut list = Vec::new();
    for p in ws["projects"].as_array().into_iter().flatten() {
        let id = p["id"].as_str().unwrap_or_default().to_string();
        let Some(raw) = get(&format!("wadawko:project:{id}")) else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&raw) else { continue };
        let Ok(mut song) = Song::from_json(&v) else { continue };
        if let Some(t) = p["title"].as_str() {
            song.title = t.to_string();
        }
        list.push(ProjectEntry { id, updated_ms: p["updatedAt"].as_i64().unwrap_or(0), song });
    }
    list.sort_by_key(|p| std::cmp::Reverse(p.updated_ms));
    let cur = list.iter().position(|p| p.id == current).unwrap_or(0);
    Ok((list, cur))
}

/// "10 de nov." a partir de epoch ms (UTC).
pub fn short_date(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    // civil_from_days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    const M: [&str; 12] = ["jan.", "fev.", "mar.", "abr.", "mai.", "jun.", "jul.", "ago.", "set.", "out.", "nov.", "dez."];
    format!("{:02} de {}", d, M[(m - 1) as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip_preserves_unknown_fields() {
        let src = r#"{"version":1,"title":"T","tempo":100,"swing":1,"people":[{"id":"p1","name":"Ana"}],
          "tracks":[{"id":"a","name":"A","instrument":"chu","muted":false,"solo":false,"personIds":["p1"],"volume":-6}],
          "measures":[{"beats":4,"subdivision":2,"cells":{"a":["don",null,"ka",null,null,null,null,"doko"]},"dynamic":"f","kyai":{"a":[null,"sore"]}}],
          "sections":[{"id":"s1","name":"Abertura","start":0,"repeat":2}]}"#;
        let mut song = Song::from_json_str(src).unwrap();
        song.measures[0].cells[0][1] = Some("kon".into());
        let out = song.to_json();
        assert_eq!(out["people"][0]["name"], "Ana");
        assert_eq!(out["tracks"][0]["personIds"][0], "p1");
        assert_eq!(out["measures"][0]["kyai"]["a"][1], "sore");
        assert_eq!(out["measures"][0]["cells"]["a"][1], "kon");
        assert_eq!(out["measures"][0]["cells"]["a"][2], "ka");
        assert_eq!(out["sections"][0]["repeat"], 2);
        assert_eq!(out["swing"], 1);
        let again = Song::from_json_str(&serde_json::to_string(&out).unwrap()).unwrap();
        assert_eq!(again.measures[0].cells[0][7].as_deref(), Some("doko"));
        assert_eq!(again.slug(), "t");
    }

    #[test]
    fn subdivision_change_drops_cell_indexed_extras() {
        let src = r#"{"version":1,"title":"T","tempo":100,"tracks":[{"id":"a","name":"A","instrument":"chu"}],
          "measures":[{"beats":4,"subdivision":2,"cells":{"a":["don",null,null,null,null,null,null,null]},"kyai":{"a":["x"]},"cellOwners":{"a":["p"]}}]}"#;
        let mut song = Song::from_json_str(src).unwrap();
        song.set_subdivision(4);
        let out = song.to_json();
        assert!(out["measures"][0].get("kyai").is_none());
        assert_eq!(out["measures"][0]["cells"]["a"].as_array().unwrap().len(), 16);
    }

    #[test]
    fn rejects_non_song_json() {
        assert!(Song::from_json_str(r#"{"version":2,"projects":[]}"#).is_err());
        assert!(Song::from_json_str("não é json").is_err());
    }
}
