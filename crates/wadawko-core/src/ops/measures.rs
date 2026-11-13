//! Compassos: criar/remover, subdivisão (reamostrando), dinâmicas e a
//! conversão coluna global ↔ (compasso, célula).

use crate::model::{CellRows, Dynamic, DynamicMark, Hairpin, Measure, Song};
use crate::ops::CellPos;
use crate::ops::sections::section_ranges;

/// Compasso vazio com uma linha de silêncio por track da música (`makeMeasure`).
pub fn make_measure(song: &Song, beats: usize, subdivision: usize) -> Measure {
    Measure::new(beats, subdivision, song.tracks.iter().map(|t| t.id.as_str()))
}

/// Adiciona um compasso vazio (mesma fórmula do vizinho): no fim da seção
/// `section_id`, ou no fim da música se `None`/seção inexistente.
///
/// No app, sem seção explícita o alvo é a última seção selecionada:
/// `add_measure(song, target_section(song, &sel).as_ref().map(|r| r.id.as_str()))`.
pub fn add_measure(song: &mut Song, section_id: Option<&str>) {
    let target = section_id.and_then(|id| section_ranges(song).into_iter().find(|r| r.id == id));
    let reference = |song: &Song, i: Option<usize>| {
        i.and_then(|i| song.measures.get(i)).or(song.measures.last()).map_or((4, 2), |m| (m.beats, m.subdivision))
    };
    match target {
        Some(t) => {
            let (beats, sub) = reference(song, t.end_excl.checked_sub(1));
            let at = t.end_excl;
            let fresh = make_measure(song, beats, sub);
            let ins = at.min(song.measures.len());
            song.measures.insert(ins, fresh);
            for s in song.sections.iter_mut().flatten() {
                if s.start >= at {
                    s.start += 1;
                }
            }
        }
        None => {
            let (beats, sub) = reference(song, None);
            let fresh = make_measure(song, beats, sub);
            song.measures.push(fresh);
        }
    }
}

/// Remove o último compasso da seção `section_id` (a seção mantém ao menos 1)
/// ou, se `None`/inexistente, o último da música (que mantém ao menos 1).
pub fn remove_measure(song: &mut Song, section_id: Option<&str>) {
    let target = section_id.and_then(|id| section_ranges(song).into_iter().find(|r| r.id == id));
    match target {
        Some(t) => {
            if t.len() <= 1 {
                return; // seção não pode ficar sem compasso
            }
            let at = t.end_excl - 1;
            if at < song.measures.len() {
                song.measures.remove(at);
            }
            for s in song.sections.iter_mut().flatten() {
                if s.start > at {
                    s.start -= 1;
                }
            }
        }
        None => {
            if song.measures.len() > 1 {
                song.measures.pop();
            }
        }
    }
}

fn resample(rows: &CellRows, old_n: usize, new_n: usize) -> CellRows {
    rows.iter()
        .map(|(track_id, cells)| {
            let mut arr: Vec<Option<String>> = vec![None; new_n];
            for (i, syl) in cells.iter().enumerate() {
                let Some(syl) = syl.as_ref().filter(|s| !s.is_empty()) else { continue };
                // old_n == 0: no JS a conta dá NaN (i = 0, nota some) ou ∞ (vai pro fim)
                if new_n == 0 || (old_n == 0 && i == 0) {
                    continue;
                }
                // Math.round((i * newN) / oldN), limitado ao fim; ocupado → próxima livre
                let mut idx = (((i * new_n) as f64 / old_n as f64) + 0.5).floor() as usize;
                idx = idx.min(new_n - 1);
                while idx < new_n && arr[idx].is_some() {
                    idx += 1;
                }
                if idx < new_n {
                    arr[idx] = Some(syl.clone());
                }
            }
            (track_id.clone(), arr)
        })
        .collect()
}

/// Muda a subdivisão de TODOS os compassos, reamostrando notas e kiai (a nota
/// vai pra célula proporcional mais próxima; se ocupada, pra próxima livre; se
/// não couber, some). Onde o nº de células muda, `cellOwners` é descartado
/// (índices mudaram; o dono volta ao fallback `personIds[0]` até reatribuir).
pub fn set_subdivision(song: &mut Song, sub: usize) {
    for meas in &mut song.measures {
        let old_n = meas.cell_count();
        let new_n = meas.beats * sub;
        if new_n == old_n {
            meas.subdivision = sub;
            continue;
        }
        if let Some(kyai) = &meas.kyai {
            meas.kyai = Some(resample(kyai, old_n, new_n));
        }
        meas.cells = resample(&meas.cells, old_n, new_n);
        meas.subdivision = sub;
        meas.cell_owners = None;
    }
}

// ───────────────────────────── dinâmicas ─────────────────────────────

/// remove a entrada do instrumento se ficou vazia (não suja o JSON)
fn prune_dyn(meas: &mut Measure, track_id: &str) {
    let Some(td) = meas.track_dynamics.as_mut() else { return };
    if td.get(track_id).is_some_and(|e| e.dynamic.is_none() && e.hairpin.is_none()) {
        td.remove(track_id);
    }
    if td.is_empty() {
        meas.track_dynamics = None;
    }
}

/// Marcação de dinâmica (pp..ff) no início do compasso; `None` remove.
/// `track_id` = só aquele instrumento (`trackDynamics`); `None` = geral.
pub fn set_measure_dynamic(song: &mut Song, mi: usize, dynamic: Option<Dynamic>, track_id: Option<&str>) {
    let Some(meas) = song.measures.get_mut(mi) else { return };
    match track_id.filter(|id| !id.is_empty()) {
        None => meas.dynamic = dynamic,
        Some(id) => {
            let exists = meas.track_dynamic(id).is_some();
            if !exists && dynamic.is_none() {
                return;
            }
            meas.track_dynamics.get_or_insert_default().entry(id.to_string()).or_default().dynamic = dynamic;
            prune_dyn(meas, id);
        }
    }
}

/// Alterna crescendo/diminuendo no compasso (mesmo valor = remove).
pub fn toggle_measure_hairpin(song: &mut Song, mi: usize, hairpin: Hairpin, track_id: Option<&str>) {
    let Some(meas) = song.measures.get_mut(mi) else { return };
    let toggle = |cur: &mut Option<Hairpin>| *cur = if *cur == Some(hairpin) { None } else { Some(hairpin) };
    match track_id.filter(|id| !id.is_empty()) {
        None => toggle(&mut meas.hairpin),
        Some(id) => {
            let mark: &mut DynamicMark = meas.track_dynamics.get_or_insert_default().entry(id.to_string()).or_default();
            toggle(&mut mark.hairpin);
            prune_dyn(meas, id);
        }
    }
}

/// O instrumento tem alguma marcação própria de dinâmica?
pub fn track_has_dynamics(song: &Song, track_id: &str) -> bool {
    song.measures.iter().any(|m| m.track_dynamic(track_id).is_some())
}

// ───────────────────────────── colunas globais ─────────────────────────────

/// Coluna global (0..total-1) → posição (compasso, célula).
pub fn col_to_pos(song: &Song, global_col: usize) -> Option<CellPos> {
    let mut acc = 0;
    for (mi, m) in song.measures.iter().enumerate() {
        let n = m.cell_count();
        if global_col < acc + n {
            return Some(CellPos { mi, ci: global_col - acc });
        }
        acc += n;
    }
    None
}

/// Posição → coluna global (inverso de `col_to_pos`; a `hitKey` usa essa coluna).
pub fn pos_to_col(song: &Song, pos: CellPos) -> Option<usize> {
    let m = song.measures.get(pos.mi)?;
    (pos.ci < m.cell_count()).then(|| song.measures[..pos.mi].iter().map(Measure::cell_count).sum::<usize>() + pos.ci)
}

/// Total de colunas (células) da música.
pub fn total_cols(song: &Song) -> usize {
    song.total_cells()
}

/// Garante pelo menos `min_cols` colunas, criando compassos (no máx. 64) com
/// `add_measure(song, section_id)`.
pub fn ensure_cols(song: &mut Song, min_cols: usize, section_id: Option<&str>) {
    let mut guard = 0;
    while total_cols(song) < min_cols && guard < 64 {
        guard += 1;
        add_measure(song, section_id);
    }
}
