//! Instrumentos (Kit): paleta = builtins + defs custom da música; resolução
//! da def efetiva de uma linha; CRUD das defs custom.

use crate::ids::{IdKind, new_id};
use crate::model::{BUILTIN_INSTRUMENTS, InstrumentDef, InstrumentId, InstrumentTone, Song, Track, Zone, ZoneParams};

/// Paleta = builtins (constantes) + custom da música, nessa ordem.
pub fn palette_instruments(song: &Song) -> Vec<&InstrumentDef> {
    BUILTIN_INSTRUMENTS.iter().chain(song.instruments()).collect()
}

/// Def por id (builtin primeiro, depois custom). `None`/vazio/desconhecido → `None`.
pub fn instrument_by_id<'a>(song: &'a Song, id: Option<&str>) -> Option<&'a InstrumentDef> {
    let id = id.filter(|s| !s.is_empty())?;
    BUILTIN_INSTRUMENTS.iter().chain(song.instruments()).find(|d| d.id == id)
}

/// Def efetiva de uma linha: por `instrumentId`; fallback = builtin do
/// instrumento base (nunca falha).
pub fn resolve_instrument<'a>(song: &'a Song, track: &Track) -> &'a InstrumentDef {
    instrument_by_id(song, track.instrument_id.as_deref()).or_else(|| BUILTIN_INSTRUMENTS.iter().find(|d| d.base == track.instrument)).unwrap_or(&BUILTIN_INSTRUMENTS[0])
}

/// Nome a mostrar nos rótulos da linha.
pub fn instrument_label<'a>(song: &'a Song, track: &Track) -> &'a str {
    &resolve_instrument(song, track).name
}

fn new_instrument_id() -> String {
    new_id(IdKind::Instrument, "inst")
}

/// Cria uma def custom sobre o sample `base` (nome default `"<Instrumento>
/// custom"`, zonas neutras: pitch 0, gain 0, overtones 0, decay 0.5). Retorna o id.
pub fn add_instrument(song: &mut Song, base: InstrumentId, name: Option<&str>) -> String {
    let zone = || Some(ZoneParams { pitch: Some(0.0), gain: Some(0.0), tone: None, overtones: Some(0.0), decay: Some(0.5) });
    let id = new_instrument_id();
    song.instruments.get_or_insert_default().push(InstrumentDef {
        id: id.clone(),
        name: name.map_or_else(|| format!("{} custom", base.label()), str::to_string),
        base,
        builtin: None,
        head: zone(),
        rim: zone(),
        ..Default::default()
    });
    id
}

/// Def custom mutável (builtins não são editáveis → `None`).
pub fn custom_instrument_mut<'a>(song: &'a mut Song, id: &str) -> Option<&'a mut InstrumentDef> {
    song.instruments.as_mut()?.iter_mut().find(|d| d.id == id)
}

/// Edita uma def custom (`updateInstrument` com patch vira closure).
/// Retorna `false` se a def não existe/é builtin.
pub fn update_instrument(song: &mut Song, id: &str, patch: impl FnOnce(&mut InstrumentDef)) -> bool {
    match custom_instrument_mut(song, id) {
        Some(def) => {
            patch(def);
            true
        }
        None => false,
    }
}

/// Patch parcial de uma zona: `None` = não mexe. `tone: Some(None)` remove o filtro.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ZonePatch {
    pub pitch: Option<f64>,
    pub gain: Option<f64>,
    pub tone: Option<Option<InstrumentTone>>,
    pub overtones: Option<f64>,
    pub decay: Option<f64>,
}

/// Merge de params de uma zona (couro/borda) de um instrumento custom.
pub fn update_instrument_zone(song: &mut Song, id: &str, zone: Zone, patch: ZonePatch) {
    let Some(def) = custom_instrument_mut(song, id) else { return };
    let z = def.zone_mut(zone).get_or_insert_default();
    if let Some(v) = patch.pitch {
        z.pitch = Some(v);
    }
    if let Some(v) = patch.gain {
        z.gain = Some(v);
    }
    if let Some(v) = patch.tone {
        z.tone = v;
    }
    if let Some(v) = patch.overtones {
        z.overtones = Some(v);
    }
    if let Some(v) = patch.decay {
        z.decay = Some(v);
    }
}

pub fn rename_instrument(song: &mut Song, id: &str, name: &str) {
    update_instrument(song, id, |d| d.name = name.to_string());
}

/// Duplica uma def (builtin ou custom) como custom `"<nome> cópia"`
/// (`builtin: false`). Retorna o id da cópia.
pub fn duplicate_instrument(song: &mut Song, id: &str) -> Option<String> {
    let mut copy = instrument_by_id(song, Some(id))?.clone();
    copy.id = new_instrument_id();
    copy.name = format!("{} cópia", copy.name);
    copy.builtin = Some(false);
    let new_id = copy.id.clone();
    song.instruments.get_or_insert_default().push(copy);
    Some(new_id)
}

/// Remove a def custom; linhas que a usavam voltam ao som base (não morrem).
pub fn delete_instrument(song: &mut Song, id: &str) {
    let Some(defs) = song.instruments.as_mut() else { return };
    defs.retain(|d| d.id != id);
    for t in &mut song.tracks {
        if t.instrument_id.as_deref() == Some(id) {
            t.instrument_id = None;
        }
    }
}
