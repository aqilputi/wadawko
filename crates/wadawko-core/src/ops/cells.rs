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

// ───────────────────────────── efeitos por célula (`crate::fx`) ─────────────────────────────

pub fn get_fx<'a>(song: &'a Song, track_id: &str, mi: usize, ci: usize) -> Option<&'a str> {
    song.measures.get(mi)?.fx_at(track_id, ci)
}

/// Grava (normalizado: maiúsculas, ordem `V R F D X H`) ou apaga os efeitos de
/// UMA célula. Linha toda vazia some do JSON (e o mapa `fx`, se ficar vazio).
/// Texto inválido não grava e devolve o erro.
pub fn set_fx(song: &mut Song, track_id: &str, mi: usize, ci: usize, fx: Option<&str>) -> Result<(), crate::fx::FxError> {
    let norm = match fx {
        Some(s) => crate::fx::normalize(s)?,
        None => None,
    };
    let Some(meas) = song.measures.get_mut(mi) else { return Ok(()) };
    let n = meas.cell_count();
    if ci >= n {
        return Ok(());
    }
    let map = meas.fx.get_or_insert_default();
    let row = map.entry(track_id.to_string()).or_insert_with(|| vec![None; n]);
    row.resize(n, None);
    row[ci] = norm;
    if row.iter().all(Option::is_none) {
        map.remove(track_id);
    }
    if map.is_empty() {
        meas.fx = None;
    }
    Ok(())
}

/// A linha tem algum efeito em algum compasso?
pub fn track_has_fx(song: &Song, track_id: &str) -> bool {
    song.measures.iter().any(|m| m.fx.as_ref().and_then(|k| k.get(track_id)).is_some_and(|row| row.iter().any(Option::is_some)))
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

// ───────────────────────────── preencher padrões (ux-pesquisa #2) ─────────────────────────────

/// Padrão para preencher uma sequência de células.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillPattern {
    /// uma a cada `n` células (a partir da 1ª)
    Every(usize),
    /// contratempo: o meio de cada tempo (`sub` = células por tempo)
    Offbeat(usize),
    /// euclidiano: `k` golpes espalhados em cada bloco de `n` células
    Euclid(usize, usize),
    /// oroshi: golpes cada vez mais próximos até o fim (gaps 4, 3, 2, 2, 1…)
    Oroshi,
}

/// Quais posições `0..len` recebem golpe.
pub fn fill_mask(p: FillPattern, len: usize) -> Vec<bool> {
    match p {
        FillPattern::Every(n) => (0..len).map(|i| i % n.max(1) == 0).collect(),
        FillPattern::Offbeat(sub) => {
            let sub = sub.max(1);
            (0..len).map(|i| sub >= 2 && i % sub == sub / 2).collect()
        }
        FillPattern::Euclid(k, n) => {
            let n = n.max(1);
            let k = k.min(n);
            (0..len).map(|i| (i % n) * k % n < k).collect()
        }
        FillPattern::Oroshi => {
            let mut m = vec![false; len];
            let gaps = [4usize, 3, 2, 2];
            let (mut i, mut g) = (0usize, 0usize);
            while i < len {
                m[i] = true;
                i += gaps.get(g).copied().unwrap_or(1);
                g += 1;
            }
            m
        }
    }
}

/// Preenche as células globais `g0..=g1` da linha com `syl` onde o padrão manda;
/// só nas vazias, a menos que `overwrite`. Devolve quantas escreveu. Num oroshi
/// a última célula vira acento (`DON` em maiúsculas).
pub fn fill_cells(song: &mut Song, track_id: &str, g0: usize, g1: usize, p: FillPattern, syl: &str, overwrite: bool) -> usize {
    use crate::ops::measures::col_to_pos;
    let len = g1.saturating_sub(g0) + 1;
    let mask = fill_mask(p, len);
    let mut n = 0;
    for (i, on) in mask.into_iter().enumerate() {
        if !on {
            continue;
        }
        let Some(pos) = col_to_pos(song, g0 + i) else { continue };
        if !overwrite && get_cell(song, track_id, pos.mi, pos.ci).is_some_and(|s| !s.is_empty()) {
            continue;
        }
        let last = p == FillPattern::Oroshi && i + 1 >= len.saturating_sub(1);
        let s = if last { syl.to_uppercase() } else { syl.to_string() };
        set_cell(song, track_id, pos.mi, pos.ci, Some(&s), None);
        n += 1;
    }
    n
}

#[cfg(test)]
mod fill_tests {
    use super::*;

    #[test]
    fn mascaras() {
        let b = |m: Vec<bool>| m.iter().map(|&x| if x { 'x' } else { '.' }).collect::<String>();
        assert_eq!(b(fill_mask(FillPattern::Every(2), 8)), "x.x.x.x.");
        assert_eq!(b(fill_mask(FillPattern::Every(4), 8)), "x...x...");
        assert_eq!(b(fill_mask(FillPattern::Offbeat(2), 8)), ".x.x.x.x");
        assert_eq!(b(fill_mask(FillPattern::Euclid(3, 8), 8)), "x..x..x.");
        assert_eq!(fill_mask(FillPattern::Euclid(5, 8), 8).iter().filter(|x| **x).count(), 5);
        assert_eq!(b(fill_mask(FillPattern::Oroshi, 16)), "x...x..x.x.xxxxx");
    }

    #[test]
    fn preenche_so_vazias() {
        let mut s = crate::templates::template_song("vazia").unwrap();
        let t = s.tracks[0].id.clone();
        set_cell(&mut s, &t, 0, 2, Some("ka"), None);
        let n = fill_cells(&mut s, &t, 0, 7, FillPattern::Every(2), "don", false);
        assert_eq!(n, 3, "a célula 2 já tinha ka");
        assert_eq!(get_cell(&s, &t, 0, 2), Some("ka"));
        assert_eq!(get_cell(&s, &t, 0, 4), Some("don"));
        // atravessa compassos (células globais)
        let n2 = fill_cells(&mut s, &t, 6, 11, FillPattern::Every(2), "don", true);
        assert_eq!(n2, 3);
        assert_eq!(get_cell(&s, &t, 1, 2), Some("don"));
    }
}
