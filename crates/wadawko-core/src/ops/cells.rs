//! Células: ler/escrever notas e kiai, preencher sequências e frases.
//!
//! `pen` = pessoa ativa (a "caneta", `store.activePersonId`): ao escrever numa
//! linha COMPARTILHADA (2+ pessoas) a nota nova ganha esse dono, se ele for uma
//! das pessoas da linha; célula apagada perde o dono. Em linha não
//! compartilhada o `pen` é ignorado.

use crate::model::Song;
use crate::ops::CellPos;
use crate::ops::measures::col_to_pos;
use crate::ops::people::assign_cell_owner;

/// escreve em `row[ci]`, crescendo a linha com `None` se preciso (como array do JS)
pub(crate) fn put(row: &mut Vec<Option<String>>, ci: usize, value: Option<String>) {
    if ci >= row.len() {
        row.resize(ci + 1, None);
    }
    row[ci] = value;
}

/// Carimba o dono da célula ao escrever numa track compartilhada (`stampOwner`).
fn stamp_owner(song: &mut Song, track_id: &str, mi: usize, ci: usize, syl: Option<&str>, pen: Option<&str>) {
    let Some(track) = song.track(track_id) else { return };
    if !track.is_shared() {
        return;
    }
    match (syl, pen) {
        (None, _) => assign_cell_owner(song, track_id, mi, ci, None),
        (Some(_), Some(pen)) if !pen.is_empty() && track.person_ids().iter().any(|p| p == pen) => {
            assign_cell_owner(song, track_id, mi, ci, Some(pen));
        }
        _ => {}
    }
}

/// Valor da célula (`None` = silêncio ou posição inexistente).
pub fn get_cell<'a>(song: &'a Song, track_id: &str, mi: usize, ci: usize) -> Option<&'a str> {
    song.measures.get(mi)?.cell(track_id, ci)
}

/// Escreve (ou apaga, com `None`) uma célula. Não valida o texto: use
/// `phrase::parse_words`/`split_word` antes se quiser recusar valor inválido.
pub fn set_cell(song: &mut Song, track_id: &str, mi: usize, ci: usize, syl: Option<&str>, pen: Option<&str>) {
    let Some(meas) = song.measures.get_mut(mi) else { return };
    let n = meas.cell_count();
    let row = meas.cells.entry(track_id.to_string()).or_insert_with(|| vec![None; n]);
    put(row, ci, syl.map(str::to_string));
    stamp_owner(song, track_id, mi, ci, syl, pen);
}

/// `get_cell` por coluna global.
pub fn get_cell_col<'a>(song: &'a Song, track_id: &str, global_col: usize) -> Option<&'a str> {
    let p = col_to_pos(song, global_col)?;
    get_cell(song, track_id, p.mi, p.ci)
}

/// `set_cell` por coluna global (coluna inexistente = nada).
pub fn set_cell_col(song: &mut Song, track_id: &str, global_col: usize, syl: Option<&str>, pen: Option<&str>) {
    if let Some(p) = col_to_pos(song, global_col) {
        set_cell(song, track_id, p.mi, p.ci, syl, pen);
    }
}

// ───────────────────────────── kiai (sub-linha de gritos por instrumento) ─────────────────────────────

pub fn get_kyai<'a>(song: &'a Song, track_id: &str, mi: usize, ci: usize) -> Option<&'a str> {
    song.measures.get(mi)?.kyai_at(track_id, ci)
}

/// Escreve/apaga UMA sílaba de kiai. Linha toda vazia some do JSON (e o mapa
/// `kyai` do compasso também, se ficar vazio).
pub fn set_kyai(song: &mut Song, track_id: &str, mi: usize, ci: usize, syl: Option<&str>) {
    let Some(meas) = song.measures.get_mut(mi) else { return };
    let n = meas.cell_count();
    let kyai = meas.kyai.get_or_insert_default();
    let row = kyai.entry(track_id.to_string()).or_insert_with(|| vec![None; n]);
    put(row, ci, syl.map(str::to_string));
    if row.iter().all(|x| x.as_deref().is_none_or(str::is_empty)) {
        kyai.remove(track_id);
    }
    if kyai.is_empty() {
        meas.kyai = None;
    }
}

pub fn track_has_kyai(song: &Song, track_id: &str) -> bool {
    song.measures.iter().any(|m| m.kyai.as_ref().and_then(|k| k.get(track_id)).is_some_and(|row| row.iter().any(|x| x.as_deref().is_some_and(|s| !s.is_empty()))))
}

// ───────────────────────────── preenchimento ─────────────────────────────

/// Resultado de um preenchimento.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FillResult {
    /// todas as células tocadas (escritas ou limpas) — p/ flash visual
    pub written: Vec<CellPos>,
    /// primeira célula DEPOIS da inserção — p/ mover o foco (`None` = fim da música)
    pub next: Option<CellPos>,
}

/// (m, c) com `c` possivelmente além do compasso → próxima posição válida.
fn normalize_pos(song: &Song, mut m: usize, mut c: usize) -> Option<CellPos> {
    while m < song.measures.len() && c >= song.measures[m].cell_count() {
        m += 1;
        c = 0;
    }
    (m < song.measures.len()).then_some(CellPos { mi: m, ci: c })
}

/// Preenche células consecutivas (atravessando compassos) a partir de (mi, ci),
/// um valor por célula. Para no fim da música (não cria compassos).
pub fn fill_sequential<S: AsRef<str>>(song: &mut Song, track_id: &str, mi: usize, ci: usize, syls: &[S], pen: Option<&str>) -> FillResult {
    let mut written = Vec::new();
    let (mut m, mut c) = (mi, ci);
    for syl in syls {
        let Some(pos) = normalize_pos(song, m, c) else { break };
        (m, c) = (pos.mi, pos.ci);
        set_cell(song, track_id, m, c, Some(syl.as_ref()), pen);
        written.push(pos);
        c += 1;
    }
    FillResult { written, next: normalize_pos(song, m, c) }
}

/// Escreve uma palavra POR TEMPO a partir de (mi, ci), sobrescrevendo o span
/// inteiro de cada tempo (célula da palavra + subdivisões restantes limpas).
/// Ancora no início do tempo da célula focada. É o modo das frases do
/// catálogo: "don doko don don" = 4 tempos.
pub fn fill_phrase_beats<S: AsRef<str>>(song: &mut Song, track_id: &str, mi: usize, ci: usize, words: &[S], pen: Option<&str>) -> FillResult {
    let mut written = Vec::new();
    let mut m = mi;
    // frase é coisa de tempo: ancora no início do tempo da célula focada
    let sub = song.measures.get(mi).map_or(2, |x| x.subdivision);
    let mut c = ci.checked_div(sub).map_or(ci, |q| q * sub);
    for word in words {
        let Some(pos) = normalize_pos(song, m, c) else { break };
        (m, c) = (pos.mi, pos.ci);
        let (n, msub) = (song.measures[m].cell_count(), song.measures[m].subdivision);
        set_cell(song, track_id, m, c, Some(word.as_ref()), pen);
        written.push(pos);
        let mut k = 1;
        while k < msub && c + k < n {
            set_cell(song, track_id, m, c + k, None, pen);
            written.push(CellPos { mi: m, ci: c + k });
            k += 1;
        }
        c += msub;
    }
    FillResult { written, next: normalize_pos(song, m, c) }
}

/// Sílabas de kiai em células consecutivas (atravessando compassos) a partir
/// de (mi, ci). As sílabas vêm de `kyai::split_kyai`.
pub fn fill_kyai<S: AsRef<str>>(song: &mut Song, track_id: &str, mi: usize, ci: usize, syls: &[S]) -> FillResult {
    let mut written = Vec::new();
    let (mut m, mut c) = (mi, ci);
    for syl in syls {
        let Some(pos) = normalize_pos(song, m, c) else { break };
        (m, c) = (pos.mi, pos.ci);
        set_kyai(song, track_id, m, c, Some(syl.as_ref()));
        written.push(pos);
        c += 1;
    }
    FillResult { written, next: normalize_pos(song, m, c) }
}
