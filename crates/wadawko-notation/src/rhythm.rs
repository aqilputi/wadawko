//! Células de um compasso → eventos rítmicos (notas, pausas, beams, quiálteras).
//!
//! Porte de `measureToNotes` (`score.ts`), com três melhorias possíveis porque aqui cada
//! golpe tem posição exata (`célula + offset`) em vez de passar por um Formatter:
//!
//! - golpes simultâneos (`doron`, `karan`) viram um acorde (duas cabeças, uma haste);
//! - sílaba que começa no meio da célula (`suko`) ganha a pausa antes dela;
//! - pausas de células vazias vizinhas se fundem dentro do tempo (2 colcheias → 1 semínima)
//!   e um compasso todo vazio vira uma pausa de compasso.

use wadawko_core::Measure;
use wadawko_core::phrase::{CellResolution, resolve_cell, strip_sfz};
use wadawko_core::vocab::{Hand, Zone};

/// sílabas de 2 golpes quebradas por golpe na letra: doko → do + ko
const SPLIT: [(&str, [&str; 2]); 4] = [("doko", ["do", "ko"]), ("kara", ["ka", "ra"]), ("tsuku", ["tsu", "ku"]), ("teke", ["te", "ke"])];

/// Um golpe dentro de uma nota (acorde = mais de um).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteHit {
    /// índice do golpe dentro da célula (o `hitIndex` da chave)
    pub index: usize,
    /// borda (fuchi) → cabeça ×
    pub rim: bool,
    /// mão esquerda → abaixo da linha
    pub left: bool,
    pub accent: bool,
    pub sfz: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EventKind {
    Note {
        hits: Vec<NoteHit>,
        /// rulo: nota única que representa N > 4 golpes (0 = nota normal)
        roll: usize,
    },
    Rest {
        /// pausa de compasso inteiro (centralizada)
        whole: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// célula de origem dentro do compasso
    pub cell: usize,
    /// posição em células desde o início do compasso
    pub pos: f32,
    /// tempo (semínima) a que pertence
    pub beat: usize,
    /// nº de bandeirolas/beams da figura: 0 = semínima, 1 = colcheia, 2 = semicolcheia…
    pub flags: u8,
    pub kind: EventKind,
    /// sílaba embaixo
    pub label: Option<String>,
}

impl Event {
    pub fn is_note(&self) -> bool {
        matches!(self.kind, EventKind::Note { .. })
    }
}

/// Notas consecutivas do mesmo tempo ligadas por beam (índices em `events`, inclusivos).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeamGroup {
    pub first: usize,
    pub last: usize,
}

/// Quiáltera de 3 dentro de uma célula (índices em `events`, inclusivos).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tuplet {
    pub first: usize,
    pub last: usize,
    pub cell: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackRhythm {
    pub events: Vec<Event>,
    pub beams: Vec<BeamGroup>,
    pub tuplets: Vec<Tuplet>,
    /// por célula: palavra escrita (sem `!`) quando a célula soa — para a letra de reserva
    pub words: Vec<Option<String>>,
}

/// rótulos por golpe de uma célula: parte de 2 golpes usa o SPLIT (doko → do, ko)
fn hit_labels(res: &CellResolution) -> Vec<Option<&'static str>> {
    let mut labels = Vec::with_capacity(res.hits.len());
    for part in &res.parts {
        if part.hits.len() == 2 {
            match SPLIT.iter().find(|(k, _)| *k == part.syl) {
                Some((_, [a, b])) => labels.extend([Some(*a), Some(*b)]),
                None => labels.extend([Some(part.syl), None]),
            }
        } else if !part.hits.is_empty() {
            labels.push(Some(part.syl));
            labels.extend(std::iter::repeat_n(None, part.hits.len() - 1));
        }
    }
    labels
}

fn log2_floor(n: usize) -> u8 {
    (usize::BITS - 1 - n.max(1).leading_zeros()) as u8
}

/// Funde vazios em blocos alinhados em potência de 2: devolve `(início, tamanho)`.
fn merge_runs(mergeable: &[bool], base: usize, top: bool, out: &mut Vec<(usize, usize)>) {
    let n = mergeable.len();
    if n == 0 {
        return;
    }
    if mergeable.iter().all(|&m| m) && (n.is_power_of_two() || top) {
        out.push((base, n));
        return;
    }
    if n == 1 {
        return;
    }
    if n.is_multiple_of(2) {
        merge_runs(&mergeable[..n / 2], base, false, out);
        merge_runs(&mergeable[n / 2..], base + n / 2, false, out);
    } else {
        for (i, &m) in mergeable.iter().enumerate() {
            if m {
                out.push((base + i, 1));
            }
        }
    }
}

fn grid_of(offsets: &[f64]) -> Option<usize> {
    [1usize, 2, 4, 3].into_iter().find(|&d| offsets.iter().all(|o| ((o * d as f64) - (o * d as f64).round()).abs() < 1e-4))
}

/// Converte a linha `track_id` do compasso em eventos rítmicos.
pub fn measure_rhythm(meas: &Measure, track_id: &str) -> TrackRhythm {
    let sub = meas.subdivision.max(1);
    let n_cells = meas.cell_count();
    let cell_flags = log2_floor(sub);
    let mut out = TrackRhythm { words: vec![None; n_cells], ..Default::default() };
    if n_cells == 0 {
        return out;
    }

    // resolve as células: None = vazia
    let resolved: Vec<Option<_>> = (0..n_cells)
        .map(|ci| meas.cell(track_id, ci).filter(|v| !v.is_empty()).and_then(resolve_cell).filter(|r| !r.hits.is_empty()))
        .collect();
    let is_su = |ci: usize| meas.cell(track_id, ci) == Some("su");

    if resolved.iter().all(Option::is_none) && !(0..n_cells).any(is_su) {
        out.events.push(Event { cell: 0, pos: 0.0, beat: 0, flags: 0, kind: EventKind::Rest { whole: true }, label: None });
        return out;
    }

    let n_beats = n_cells.div_ceil(sub);
    for beat in 0..n_beats {
        let lo = beat * sub;
        let hi = ((beat + 1) * sub).min(n_cells);
        // pausas fundidas deste tempo, por célula inicial
        let mergeable: Vec<bool> = (lo..hi).map(|ci| resolved[ci].is_none() && !is_su(ci)).collect();
        let mut rests = Vec::new();
        merge_runs(&mergeable, lo, true, &mut rests);

        let mut ci = lo;
        while ci < hi {
            if let Some(&(_, len)) = rests.iter().find(|(s, _)| *s == ci) {
                let flags = cell_flags.saturating_sub(log2_floor(len));
                out.events.push(Event { cell: ci, pos: ci as f32, beat, flags, kind: EventKind::Rest { whole: false }, label: None });
                ci += len;
                continue;
            }
            match &resolved[ci] {
                None => {
                    // `su`: pausa marcada, com a sílaba
                    out.events.push(Event {
                        cell: ci,
                        pos: ci as f32,
                        beat,
                        flags: cell_flags,
                        kind: EventKind::Rest { whole: false },
                        label: Some("su".into()),
                    });
                }
                Some(res) => {
                    let value = meas.cell(track_id, ci).unwrap_or_default();
                    out.words[ci] = Some(strip_sfz(value).to_string());
                    cell_events(&mut out, res, strip_sfz(value), ci, beat, cell_flags);
                }
            }
            ci += 1;
        }
    }

    // beams: sequências de 2+ notas com bandeirola, no mesmo tempo, sem pausa no meio
    let mut i = 0;
    while i < out.events.len() {
        let beamable = |e: &Event| e.is_note() && e.flags >= 1;
        if !beamable(&out.events[i]) {
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < out.events.len() && beamable(&out.events[j + 1]) && out.events[j + 1].beat == out.events[i].beat {
            j += 1;
        }
        if j > i {
            out.beams.push(BeamGroup { first: i, last: j });
        }
        i = j + 1;
    }
    out
}

fn cell_events(out: &mut TrackRhythm, res: &CellResolution, word: &str, ci: usize, beat: usize, cell_flags: u8) {
    let labels = hit_labels(res);
    // agrupa golpes simultâneos (acorde)
    let mut onsets: Vec<(f64, Vec<usize>)> = Vec::new();
    for (i, h) in res.hits.iter().enumerate() {
        match onsets.iter_mut().find(|(o, _)| (*o - h.offset).abs() < 1e-6) {
            Some((_, v)) => v.push(i),
            None => onsets.push((h.offset, vec![i])),
        }
    }
    onsets.sort_by(|a, b| a.0.total_cmp(&b.0));
    let note_hit = |i: usize| {
        let h = &res.hits[i];
        NoteHit { index: i, rim: h.zone == Zone::Rim, left: h.hand == Hand::L, accent: h.accent, sfz: h.sfz }
    };
    let note = |idx: &[usize], pos: f32, flags: u8| Event {
        cell: ci,
        pos,
        beat,
        flags,
        kind: EventKind::Note { hits: idx.iter().map(|&i| note_hit(i)).collect(), roll: 0 },
        label: idx.iter().find_map(|&i| labels.get(i).copied().flatten()).map(str::to_string),
    };
    let offsets: Vec<f64> = onsets.iter().map(|o| o.0).collect();
    let k = onsets.len();

    match grid_of(&offsets) {
        Some(d) => {
            let slot_flags = cell_flags + if d == 3 { 1 } else { log2_floor(d) };
            let mut slots: Vec<Option<&Vec<usize>>> = vec![None; d];
            for (o, idx) in &onsets {
                let s = ((o * d as f64).round() as usize).min(d - 1);
                slots[s] = Some(idx);
            }
            // nota (ou pausa) absorve os espaços vazios seguintes enquanto ficar alinhada em
            // potência de 2: dokodon = fusa fusa semicolcheia; suko = pausa + nota
            let first = out.events.len();
            let mut s = 0;
            while s < d {
                let pos = ci as f32 + s as f32 / d as f32;
                let is_note = slots[s].is_some();
                let mut len = 1;
                while s % (len * 2) == 0 && s + len * 2 <= d && slots[s + usize::from(is_note)..s + len * 2].iter().all(Option::is_none) {
                    len *= 2;
                }
                let flags = slot_flags.saturating_sub(log2_floor(len));
                match slots[s] {
                    Some(idx) => out.events.push(note(idx, pos, flags)),
                    None => out.events.push(Event { cell: ci, pos, beat, flags, kind: EventKind::Rest { whole: false }, label: None }),
                }
                s += len;
            }
            if d == 3 {
                out.tuplets.push(Tuplet { first, last: out.events.len() - 1, cell: ci });
            }
        }
        // mais de 4 ataques fora de grade simples: rulo — nota única com a palavra inteira
        None if k > 4 => {
            let all: Vec<usize> = (0..res.hits.len()).collect();
            let first = &res.hits[0];
            out.events.push(Event {
                cell: ci,
                pos: ci as f32,
                beat,
                // semínima com três traços (rulo não medido): sem bandeirola nem beam
                flags: 0,
                kind: EventKind::Note {
                    hits: vec![NoteHit { index: 0, rim: first.zone == Zone::Rim, left: first.hand == Hand::L, accent: first.accent, sfz: first.sfz }],
                    roll: all.len(),
                },
                label: Some(word.to_string()),
            });
        }
        // grade irregular (alongamentos): notas na posição exata, figura pela contagem
        None => {
            let flags = cell_flags + if k == 1 { 0 } else if k <= 3 { 1 } else { 2 };
            for (o, idx) in &onsets {
                out.events.push(note(idx, ci as f32 + *o as f32, flags));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meas(sub: usize, cells: &[&str]) -> Measure {
        let mut m = Measure::new(cells.len() / sub, sub, ["t"]);
        m.cells.insert("t".into(), cells.iter().map(|c| (!c.is_empty()).then(|| c.to_string())).collect());
        m
    }

    #[test]
    fn compasso_vazio_vira_pausa_de_compasso() {
        let r = measure_rhythm(&meas(2, &["", "", "", "", "", "", "", ""]), "t");
        assert_eq!(r.events.len(), 1);
        assert_eq!(r.events[0].kind, EventKind::Rest { whole: true });
    }

    #[test]
    fn pausas_fundem_dentro_do_tempo() {
        // DON _ | _ _ | don don | su _
        let r = measure_rhythm(&meas(2, &["DON", "", "", "", "don", "don", "su", ""]), "t");
        let shape: Vec<(bool, u8, f32)> = r.events.iter().map(|e| (e.is_note(), e.flags, e.pos)).collect();
        assert_eq!(
            shape,
            vec![(true, 1, 0.0), (false, 1, 1.0), (false, 0, 2.0), (true, 1, 4.0), (true, 1, 5.0), (false, 1, 6.0), (false, 1, 7.0)]
        );
        assert_eq!(r.events[5].label.as_deref(), Some("su"));
        assert_eq!(r.beams, vec![BeamGroup { first: 3, last: 4 }]);
    }

    #[test]
    fn doko_divide_a_celula_e_a_letra() {
        let r = measure_rhythm(&meas(2, &["doko", "don"]), "t");
        assert_eq!(r.events.len(), 3);
        assert_eq!(r.events.iter().map(|e| e.flags).collect::<Vec<_>>(), vec![2, 2, 1]);
        assert_eq!(r.events.iter().map(|e| e.label.clone().unwrap()).collect::<Vec<_>>(), vec!["do", "ko", "don"]);
        assert_eq!(r.events[1].pos, 0.5);
        assert_eq!(r.beams, vec![BeamGroup { first: 0, last: 2 }]);
    }

    #[test]
    fn suko_tem_pausa_antes() {
        let r = measure_rhythm(&meas(2, &["suko", ""]), "t");
        assert!(!r.events[0].is_note());
        assert_eq!(r.events[0].flags, 2);
        assert!(r.events[1].is_note());
        assert_eq!(r.events[1].pos, 0.5);
    }

    #[test]
    fn doron_e_acorde_e_tres_golpes_e_quialtera() {
        let r = measure_rhythm(&meas(2, &["doron", "dondondon"]), "t");
        match &r.events[0].kind {
            EventKind::Note { hits, .. } => {
                assert_eq!(hits.len(), 2);
                assert!(!hits[0].left && hits[1].left);
            }
            _ => panic!("doron deveria ser nota"),
        }
        assert_eq!(r.tuplets.len(), 1);
        assert_eq!(r.tuplets[0].last - r.tuplets[0].first, 2);
    }

    #[test]
    fn dokodon_nao_e_quialtera() {
        let r = measure_rhythm(&meas(2, &["dokodon", "dondoko"]), "t");
        assert!(r.tuplets.is_empty());
        assert_eq!(r.events.iter().map(|e| (e.is_note(), e.flags)).collect::<Vec<_>>(), vec![(true, 3), (true, 3), (true, 2), (true, 2), (true, 3), (true, 3)]);
    }

    #[test]
    fn rulo_e_nota_unica() {
        let r = measure_rhythm(&meas(2, &["dokodokodoko", ""]), "t");
        match &r.events[0].kind {
            EventKind::Note { roll, .. } => assert_eq!(*roll, 6),
            _ => panic!(),
        }
        assert_eq!(r.events[0].label.as_deref(), Some("dokodokodoko"));
    }
}
