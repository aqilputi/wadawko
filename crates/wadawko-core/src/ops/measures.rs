//! Compassos: criar/remover, subdivisão (reamostrando), dinâmicas e a
//! conversão coluna global ↔ (compasso, célula).

use crate::model::{CellRows, Dynamic, DynamicMark, Hairpin, Measure, Song, TempoMark};
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
    let reference = |song: &Song, i: Option<usize>| i.and_then(|i| song.measures.get(i)).or(song.measures.last()).map_or((4, 2), |m| (m.beats, m.subdivision));
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
        if let Some(fx) = &meas.fx {
            meas.fx = Some(resample(fx, old_n, new_n));
        }
        meas.cells = resample(&meas.cells, old_n, new_n);
        meas.subdivision = sub;
        meas.cell_owners = None;
    }
}

/// Menor e maior número de tempos por compasso aceitos pela UI.
pub const MIN_BEATS: usize = 1;
pub const MAX_BEATS: usize = 16;

/// Muda a fórmula do compasso `mi` para `beats`/4 (ex.: 4 → 5 = 5/4). As notas
/// ficam nas MESMAS células: aumentar acrescenta tempos vazios no fim; diminuir
/// corta os últimos tempos (as notas deles somem — o desfazer recupera).
/// `cells`, `cellOwners` e `kyai` são ajustados juntos. Retorna `false` se nada
/// mudou (compasso inexistente ou mesma fórmula).
pub fn set_measure_beats(song: &mut Song, mi: usize, beats: usize) -> bool {
    let beats = beats.clamp(MIN_BEATS, MAX_BEATS);
    let Some(meas) = song.measures.get_mut(mi) else { return false };
    if meas.beats == beats {
        return false;
    }
    let new_n = beats * meas.subdivision.max(1);
    let fit = |rows: &mut CellRows| {
        for row in rows.values_mut() {
            row.resize(new_n, None);
        }
    };
    fit(&mut meas.cells);
    if let Some(owners) = meas.cell_owners.as_mut() {
        fit(owners);
    }
    if let Some(kyai) = meas.kyai.as_mut() {
        fit(kyai);
    }
    if let Some(fx) = meas.fx.as_mut() {
        fit(fx);
    }
    meas.beats = beats;
    true
}

/// Início de cada compasso TOCADO, em tempos, para a ordem de execução `order`
/// (compassos-fonte, saída de `play_order`): `starts[u]` = soma dos tempos dos
/// compassos tocados antes de `u`. Tem `order.len() + 1` entradas — a última é a
/// duração total. Com compassos de fórmulas diferentes (4/4, 5/4, 3/4…) é o
/// que posiciona cada um no tempo; um compasso-fonte inexistente conta 4.
pub fn bar_beat_starts(song: &Song, order: &[usize]) -> Vec<f64> {
    let mut starts = Vec::with_capacity(order.len() + 1);
    let mut acc = 0.0;
    for &mi in order {
        starts.push(acc);
        acc += song.measures.get(mi).map_or(4, |m| m.beats) as f64;
    }
    starts.push(acc);
    starts
}

// ───────────────────────────── andamento (rallentando / accelerando) ─────────────────────────────

/// Limites aceitos para o BPM alvo e para a duração (em compassos) de uma mudança.
pub const MIN_TEMPO: f64 = 20.0;
pub const MAX_TEMPO: f64 = 400.0;
pub const MAX_TEMPO_SPAN: usize = 64;

/// Põe (ou tira, com `None`) a mudança de andamento que começa no compasso `mi`.
/// O alvo é preso em [`MIN_TEMPO`, `MAX_TEMPO`] e a duração em `0..=MAX_TEMPO_SPAN`.
/// Retorna `false` se nada mudou.
pub fn set_tempo_mark(song: &mut Song, mi: usize, mark: Option<TempoMark>) -> bool {
    let Some(meas) = song.measures.get_mut(mi) else { return false };
    let mark = mark.map(|m| TempoMark { target: m.target.clamp(MIN_TEMPO, MAX_TEMPO).round(), span: m.span.min(MAX_TEMPO_SPAN) });
    if meas.tempo == mark {
        return false;
    }
    meas.tempo = mark;
    true
}

/// Trecho do mapa de andamento: de `start` a `end` (em tempos desde o início da
/// execução) o BPM vai linearmente de `bpm0` a `bpm1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TempoSegment {
    pub start: f64,
    pub end: f64,
    pub bpm0: f64,
    pub bpm1: f64,
}

impl TempoSegment {
    pub fn bpm_at(&self, beat: f64) -> f64 {
        if self.end <= self.start {
            return self.bpm1;
        }
        let f = ((beat - self.start) / (self.end - self.start)).clamp(0.0, 1.0);
        self.bpm0 + (self.bpm1 - self.bpm0) * f
    }
}

/// Mapa de andamento da execução `order` (compassos-fonte na ordem tocada),
/// começando em `song.tempo`: trechos contíguos que cobrem `0..duração` (em tempos).
/// Uma marcação que começa no meio de uma rampa ainda em curso a interrompe no
/// BPM daquele ponto. Sem marcações: um trecho só, constante.
pub fn tempo_segments(song: &Song, order: &[usize]) -> Vec<TempoSegment> {
    tempo_segments_from(song, order, song.tempo)
}

/// Como [`tempo_segments`], mas começando em `start_bpm` — para tocar só um trecho
/// (região) com o andamento que vale no início dele.
pub fn tempo_segments_from(song: &Song, order: &[usize], start_bpm: f64) -> Vec<TempoSegment> {
    let starts = bar_beat_starts(song, order);
    let total = *starts.last().unwrap_or(&0.0);
    let mut segs: Vec<TempoSegment> = Vec::new();
    let mut cur = start_bpm;
    let mut t = 0.0; // onde o trecho constante atual começou
    for (u, &mi) in order.iter().enumerate() {
        let Some(mark) = song.measures.get(mi).and_then(|m| m.tempo.as_ref()) else { continue };
        let b0 = starts[u];
        // rampa anterior ainda em curso: corta no BPM deste ponto
        if let Some(last) = segs.last_mut()
            && last.end > b0
        {
            cur = last.bpm_at(b0);
            last.bpm1 = cur;
            last.end = b0;
            t = b0;
        }
        if b0 > t {
            segs.push(TempoSegment { start: t, end: b0, bpm0: cur, bpm1: cur });
        }
        if mark.span == 0 {
            t = b0;
        } else {
            let b1 = starts[(u + mark.span).min(order.len())];
            if b1 > b0 {
                segs.push(TempoSegment { start: b0, end: b1, bpm0: cur, bpm1: mark.target });
            }
            t = b1;
        }
        cur = mark.target;
    }
    if total > t || segs.is_empty() {
        segs.push(TempoSegment { start: t, end: total.max(t), bpm0: cur, bpm1: cur });
    }
    segs
}

/// BPM que vinha soando logo ANTES do início de cada compasso ESCRITO (ordem da
/// partitura, sem repetições; o 1º compasso parte de `song.tempo`) — o ponto de
/// partida de uma marcação ali: rotula "rall." quando o alvo é menor, "accel." quando maior.
pub fn tempo_at_measure_starts(song: &Song) -> Vec<f64> {
    let order: Vec<usize> = (0..song.measures.len()).collect();
    let segs = tempo_segments(song, &order);
    let starts = bar_beat_starts(song, &order);
    starts[..order.len()]
        .iter()
        .map(|&b| {
            if b <= 0.0 {
                return song.tempo;
            }
            // trecho que termina em b ou passa por ele (limite à esquerda)
            segs.iter().find(|s| b > s.start && b <= s.end).map_or(song.tempo, |s| s.bpm_at(b))
        })
        .collect()
}

/// Rótulo curto de uma marcação de andamento: `rall. → 90`, `accel. → 130` ou `♩ = 90`
/// (troca imediata ou alvo igual ao andamento de antes).
pub fn tempo_mark_label(mark: &TempoMark, bpm_before: f64) -> String {
    let t = mark.target.round();
    if mark.span == 0 || (t - bpm_before.round()).abs() < 0.5 {
        format!("♩ = {t:.0}")
    } else if t < bpm_before {
        format!("rall. → {t:.0}")
    } else {
        format!("accel. → {t:.0}")
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
