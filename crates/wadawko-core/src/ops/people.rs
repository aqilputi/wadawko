//! Pessoas (agrupamento de instrumentos): CRUD, dono de célula, audibilidade
//! por mudo/solo do submix e trilhos de agrupamento.

use crate::ids::{IdKind, new_id};
use crate::model::{Measure, PERSON_COLORS, Person, Song, Track};
use crate::ops::CellPos;
use crate::ops::cells::put;

pub fn person_by_id<'a>(song: &'a Song, id: Option<&str>) -> Option<&'a Person> {
    song.person(id.filter(|s| !s.is_empty())?)
}

pub fn person_color<'a>(song: &'a Song, id: Option<&str>) -> Option<&'a str> {
    person_by_id(song, id)?.color.as_deref()
}

/// true se o instrumento é compartilhado (2+ pessoas).
pub fn is_shared(track: &Track) -> bool {
    track.is_shared()
}

/// Dono efetivo de uma célula, dado o compasso (é o `ownerOf` de `lane.ts`/
/// `score.ts` e o `kyaiSeed` sem o fallback pra linha):
/// - track sem pessoa → `None`
/// - track com 1 pessoa → sempre ela (ignora `cellOwners`)
/// - compartilhada → `cellOwners[trackId][ci]`; fallback `personIds[0]` se ausente/stale
pub fn owner_in_measure<'a>(track: &'a Track, meas: Option<&'a Measure>, ci: usize) -> Option<&'a str> {
    let ids = track.person_ids();
    match ids {
        [] => None,
        [only] => Some(only),
        [first, ..] => {
            let raw = meas.and_then(|m| m.raw_owner(&track.id, ci)).filter(|r| !r.is_empty());
            Some(raw.filter(|r| ids.iter().any(|id| id == r)).unwrap_or(first))
        }
    }
}

/// Dono efetivo da célula (mi, ci) da linha `track` (`cellOwnerOf`).
pub fn cell_owner_of<'a>(song: &'a Song, track: &'a Track, mi: usize, ci: usize) -> Option<&'a str> {
    owner_in_measure(track, song.measures.get(mi), ci)
}

/// Como `cell_owner_of`, buscando a linha pelo id (`cellOwner`).
pub fn cell_owner<'a>(song: &'a Song, track_id: &str, mi: usize, ci: usize) -> Option<&'a str> {
    cell_owner_of(song, song.track(track_id)?, mi, ci)
}

/// Pessoa audível segundo mudo/solo do submix: `false` = silenciada (notas dela
/// ficam transparentes nas views e não soam). `None`/pessoa inexistente →
/// sempre audível.
pub fn person_audible(song: &Song, id: Option<&str>) -> bool {
    let Some(p) = person_by_id(song, id) else { return true };
    if p.is_muted() {
        return false;
    }
    let any_solo = song.people().iter().any(Person::is_solo);
    !any_solo || p.is_solo()
}

/// Grava o dono de uma célula (só faz sentido em track compartilhada).
pub fn assign_cell_owner(song: &mut Song, track_id: &str, mi: usize, ci: usize, person_id: Option<&str>) {
    let Some(meas) = song.measures.get_mut(mi) else { return };
    let n = meas.cell_count();
    let row = meas.cell_owners.get_or_insert_default().entry(track_id.to_string()).or_insert_with(|| vec![None; n]);
    put(row, ci, person_id.map(str::to_string));
}

/// Reatribui o dono de várias células (modo seleção). Itens: `(trackId, posição)`.
pub fn set_cells_owner<S: AsRef<str>>(song: &mut Song, cells: &[(S, CellPos)], person_id: Option<&str>) {
    for (track_id, pos) in cells {
        assign_cell_owner(song, track_id.as_ref(), pos.mi, pos.ci, person_id);
    }
}

/// Cria uma pessoa (nome default `"Pessoa N"`, cor do ciclo `PERSON_COLORS`) e
/// retorna o id. UI: se não houver caneta ativa, o app web ativa a pessoa nova.
pub fn add_person(song: &mut Song, name: Option<&str>) -> String {
    let people = song.people.get_or_insert_default();
    let n = people.len();
    let id = new_id(IdKind::Person, "person");
    people.push(Person {
        id: id.clone(),
        name: name.map_or_else(|| format!("Pessoa {}", n + 1), str::to_string),
        color: Some(PERSON_COLORS[n % PERSON_COLORS.len()].to_string()),
        ..Default::default()
    });
    id
}

pub fn rename_person(song: &mut Song, id: &str, name: &str) {
    if let Some(p) = song.person_mut(id) {
        p.name = name.to_string();
    }
}

pub fn set_person_color(song: &mut Song, id: &str, color: &str) {
    if let Some(p) = song.person_mut(id) {
        p.color = Some(color.to_string());
    }
}

/// Remove a pessoa: limpa `personIds` e `cellOwners`; NÃO apaga tracks nem
/// notas. UI: se ela era a caneta ativa, passe pra `song.people()[0]`.
pub fn delete_person(song: &mut Song, id: &str) {
    let Some(people) = song.people.as_mut() else { return };
    people.retain(|p| p.id != id);
    for t in &mut song.tracks {
        if let Some(ids) = t.person_ids.as_mut() {
            ids.retain(|pid| pid != id);
        }
    }
    for meas in &mut song.measures {
        for row in meas.cell_owners.iter_mut().flat_map(|m| m.values_mut()) {
            for slot in row.iter_mut() {
                if slot.as_deref() == Some(id) {
                    *slot = None;
                }
            }
        }
    }
}

/// Liga/desliga a pertença de um instrumento a uma pessoa. Se a linha deixa de
/// ser compartilhada (≤ 1 pessoa), os `cellOwners` dela são apagados.
pub fn toggle_track_person(song: &mut Song, track_id: &str, person_id: &str) {
    let Some(track) = song.track_mut(track_id) else { return };
    let ids = track.person_ids.get_or_insert_default();
    match ids.iter().position(|p| p == person_id) {
        Some(at) => {
            ids.remove(at);
        }
        None => ids.push(person_id.to_string()),
    }
    // deixou de ser compartilhada → cellOwners perde o sentido; limpa
    if ids.len() <= 1 {
        for meas in &mut song.measures {
            if let Some(owners) = meas.cell_owners.as_mut() {
                owners.remove(track_id);
            }
        }
    }
}

/// Trilho de agrupamento de uma pessoa.
#[derive(Clone, Debug, PartialEq)]
pub struct Rail<'a> {
    pub person: &'a Person,
    /// paralelo a `song.tracks`: a pessoa toca essa linha?
    pub rows: Vec<bool>,
}

/// Trilhos de agrupamento: para cada pessoa (na ordem de `song.people`), quais
/// linhas ela toca. Suporta sobreposição e não-contiguidade. Pessoas sem
/// nenhuma linha ficam de fora.
pub fn rails_for_tracks(song: &Song) -> Vec<Rail<'_>> {
    song.people()
        .iter()
        .map(|person| Rail { person, rows: song.tracks.iter().map(|t| t.person_ids().contains(&person.id)).collect() })
        .filter(|r| r.rows.iter().any(|&b| b))
        .collect()
}
