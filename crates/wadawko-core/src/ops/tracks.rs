//! Linhas (tracks): adicionar por InstrumentDef, remover, renomear, trocar de
//! instrumento, reordenar e destaque "main".

use crate::ids::{IdKind, new_id};
use crate::model::{BUILTIN_INSTRUMENTS, Song, Track};
use crate::ops::kit::instrument_by_id;
use crate::timeline::main_tracks;

/// Adiciona uma linha a partir de uma `InstrumentDef` (builtin id ou custom
/// id; id desconhecido cai no primeiro builtin, shime). O nome é o da def, com
/// sufixo " N" se já houver linhas dela. Todos os compassos ganham a linha
/// vazia. Retorna o id da linha (`<base>-<tempo36>-<n>`).
pub fn add_track(song: &mut Song, def_id: &str) -> String {
    let def = instrument_by_id(song, Some(def_id)).unwrap_or(&BUILTIN_INSTRUMENTS[0]).clone();
    let id = new_id(IdKind::Track, def.base.as_str());
    let count = song.tracks.iter().filter(|t| t.def_id() == def.id).count();
    song.tracks.push(Track {
        id: id.clone(),
        name: if count > 0 { format!("{} {}", def.name, count + 1) } else { def.name.clone() },
        instrument: def.base,
        instrument_id: Some(def.id.clone()),
        ..Default::default()
    });
    for meas in &mut song.measures {
        meas.cells.insert(id.clone(), vec![None; meas.cell_count()]);
    }
    id
}

/// Remove a linha e tudo dela nos compassos (notas, donos, kiai, dinâmicas
/// próprias) e do destaque. A coreografia/pessoas não são afetadas.
pub fn remove_track(song: &mut Song, track_id: &str) {
    song.tracks.retain(|t| t.id != track_id);
    let main = main_tracks(song);
    set_main_tracks(song, main);
    for meas in &mut song.measures {
        meas.cells.remove(track_id);
        if let Some(owners) = meas.cell_owners.as_mut() {
            owners.remove(track_id);
        }
        if let Some(kyai) = meas.kyai.as_mut() {
            kyai.remove(track_id);
        }
        if let Some(td) = meas.track_dynamics.as_mut() {
            td.remove(track_id);
            if td.is_empty() {
                meas.track_dynamics = None;
            }
        }
    }
}

/// grava a lista de destaque (vazia = remove o campo); migra o formato antigo
fn set_main_tracks(song: &mut Song, ids: Vec<String>) {
    song.main_track_id = None;
    song.main_track_ids = (!ids.is_empty()).then_some(ids);
}

/// Liga/desliga o "main" de um instrumento (vários podem estar em destaque).
pub fn toggle_main_track(song: &mut Song, track_id: &str) {
    let mut cur = main_tracks(song);
    match cur.iter().position(|id| id == track_id) {
        Some(_) => cur.retain(|id| id != track_id),
        None => cur.push(track_id.to_string()),
    }
    set_main_tracks(song, cur);
}

pub fn is_main_track(song: &Song, track_id: &str) -> bool {
    main_tracks(song).iter().any(|id| id == track_id)
}

pub fn rename_track(song: &mut Song, track_id: &str, name: &str) {
    if let Some(t) = song.track_mut(track_id) {
        t.name = name.to_string();
    }
}

/// Troca o instrumento de uma linha (mantém as notas); atualiza base +
/// `instrumentId`. Def desconhecida = nada.
pub fn set_track_instrument(song: &mut Song, track_id: &str, def_id: &str) {
    let Some(def) = instrument_by_id(song, Some(def_id)) else { return };
    let (base, id) = (def.base, def.id.clone());
    if let Some(t) = song.track_mut(track_id) {
        t.instrument = base;
        t.instrument_id = Some(id);
    }
}

/// Move a linha na ordem (reflete em grade/partitura/esteira).
/// `dir` = -1 (cima) / +1 (baixo); fora dos limites = nada.
pub fn move_track(song: &mut Song, track_id: &str, dir: i32) {
    let Some(i) = song.track_index(track_id) else { return };
    let j = i as i64 + dir as i64;
    if j < 0 || j >= song.tracks.len() as i64 {
        return;
    }
    let x = song.tracks.remove(i);
    song.tracks.insert(j as usize, x);
}
