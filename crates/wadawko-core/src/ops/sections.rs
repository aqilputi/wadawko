//! Seções: intervalos, criar no fim, clonar, mover (levando os compassos),
//! renomear, ritornello e remover.

use crate::ids::{IdKind, new_id};
use crate::model::{Measure, Section, Song};
use crate::ops::measures::make_measure;
use crate::timeline::Region;

/// Seção com o intervalo de compassos calculado.
///
/// Diferença de forma em relação ao TS: lá `end` é o ÚLTIMO compasso
/// (inclusivo); aqui `end_excl` é o primeiro compasso DEPOIS da seção
/// (`end_excl == end + 1`), que não fica negativo em seção vazia.
#[derive(Clone, Debug, PartialEq)]
pub struct SectionRange {
    pub id: String,
    pub name: String,
    pub start: usize,
    pub repeat: Option<u32>,
    /// primeiro compasso depois da seção (início da próxima, ou nº de compassos)
    pub end_excl: usize,
}

impl SectionRange {
    /// quantos compassos a seção tem
    pub fn len(&self) -> usize {
        self.end_excl.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// último compasso da seção (o `end` inclusivo do TS); `None` se vazia
    pub fn last(&self) -> Option<usize> {
        (self.end_excl > self.start).then(|| self.end_excl - 1)
    }

    pub fn contains(&self, mi: usize) -> bool {
        mi >= self.start && mi < self.end_excl
    }

    /// `[start, end_excl)` como região de playback
    pub fn region(&self) -> Region {
        Region { start_bar: self.start, end_bar: self.end_excl }
    }
}

/// Seções ordenadas por compasso inicial, com o fim calculado (`sectionRanges`).
pub fn section_ranges(song: &Song) -> Vec<SectionRange> {
    let mut secs: Vec<&Section> = song.sections().iter().collect();
    secs.sort_by_key(|s| s.start);
    secs.iter()
        .enumerate()
        .map(|(i, s)| SectionRange { id: s.id.clone(), name: s.name.clone(), start: s.start, repeat: s.repeat, end_excl: secs.get(i + 1).map_or(song.measures.len(), |n| n.start) })
        .collect()
}

/// Seção que contém o compasso `mi` (`None` = antes da 1ª seção ou sem seções).
pub fn section_of_measure(song: &Song, mi: usize) -> Option<SectionRange> {
    section_ranges(song).into_iter().find(|s| s.contains(mi))
}

/// Seção alvo pra +/− compasso: a que termina por último entre as selecionadas
/// (`selectedTargetSection`). `None` = nada selecionado.
pub fn target_section(song: &Song, selected: &[String]) -> Option<SectionRange> {
    let mut best: Option<SectionRange> = None;
    for r in section_ranges(song).into_iter().filter(|r| selected.contains(&r.id)) {
        if best.as_ref().is_none_or(|a| r.end_excl > a.end_excl) {
            best = Some(r);
        }
    }
    best
}

/// Intervalo de compassos que cobre as seções selecionadas (`selectedRegion`);
/// `None` = nada selecionado = música inteira.
pub fn selected_region(song: &Song, selected: &[String]) -> Option<Region> {
    let ranges: Vec<SectionRange> = section_ranges(song).into_iter().filter(|r| selected.contains(&r.id)).collect();
    let start_bar = ranges.iter().map(|r| r.start).min()?;
    let end_bar = ranges.iter().map(|r| r.end_excl).max()?;
    Some(Region { start_bar, end_bar })
}

fn new_section_id() -> String {
    new_id(IdKind::Section, "sec")
}

/// Adiciona uma seção SEMPRE no fim. A primeira envolve a música já existente
/// (start 0); as seguintes ganham um compasso novo próprio no fim.
/// Nome default: `"Seção N"`. Retorna o id da seção criada.
pub fn append_section(song: &mut Song, name: Option<&str>) -> String {
    let id = new_section_id();
    let count = song.sections().len();
    if count == 0 {
        let name = name.unwrap_or("Seção 1").to_string();
        song.sections.get_or_insert_default().push(Section { id: id.clone(), name, start: 0, ..Default::default() });
        return id;
    }
    let start = song.measures.len();
    let (beats, sub) = song.measures.last().map_or((4, 2), |m| (m.beats, m.subdivision));
    let fresh = make_measure(song, beats, sub); // bloco próprio no fim
    song.measures.push(fresh);
    let name = name.map_or_else(|| format!("Seção {}", count + 1), str::to_string);
    song.sections.get_or_insert_default().push(Section { id: id.clone(), name, start, ..Default::default() });
    id
}

/// Clona a seção `id`: copia os compassos (notas, donos, dinâmicas, kiai) e
/// insere a cópia logo depois da original, como seção nova `"<nome> (cópia)"`.
/// Retorna o id da cópia (`None` se a seção não existe).
pub fn clone_section(song: &mut Song, id: &str) -> Option<String> {
    let r = section_ranges(song).into_iter().find(|x| x.id == id)?;
    let n = song.measures.len();
    let copy: Vec<Measure> = song.measures[r.start.min(n)..r.end_excl.min(n)].to_vec();
    let at = r.end_excl;
    let copy_len = copy.len();
    let ins = at.min(n);
    song.measures.splice(ins..ins, copy);
    let sections = song.sections.get_or_insert_default();
    for s in sections.iter_mut() {
        if s.start >= at {
            s.start += copy_len;
        }
    }
    let new_id = new_section_id();
    let idx = sections.iter().position(|s| s.id == id)?;
    sections.insert(idx + 1, Section { id: new_id.clone(), name: format!("{} (cópia)", r.name), start: at, repeat: r.repeat.filter(|&x| x != 0), ..Default::default() });
    Some(new_id)
}

/// Move a seção `id` para a posição `target_index` (na ordem por compasso),
/// levando os compassos junto. Compassos antes da 1ª seção ficam fixos no topo.
/// Depois da chamada `song.sections` fica ordenado por `start`.
pub fn move_section(song: &mut Song, id: &str, target_index: usize) {
    let ranges = section_ranges(song);
    let Some(from) = ranges.iter().position(|r| r.id == id) else { return };
    let n = song.measures.len();
    let pre_len = ranges.first().map_or(0, |r| r.start).min(n);
    let mut blocks: Vec<Vec<Measure>> = ranges.iter().map(|r| song.measures[r.start.min(n)..r.end_excl.min(n).max(r.start.min(n))].to_vec()).collect();
    // metadados na ordem por compasso (preserva chaves desconhecidas da seção)
    let mut metas: Vec<Section> = ranges.iter().filter_map(|r| song.section(&r.id).cloned()).collect();
    let mb = blocks.remove(from);
    let ms = metas.remove(from);
    let t = target_index.min(metas.len());
    blocks.insert(t, mb);
    metas.insert(t, ms);
    let mut new_measures: Vec<Measure> = song.measures[..pre_len].to_vec();
    let mut new_sections = Vec::with_capacity(metas.len());
    let mut acc = pre_len;
    for (mut meta, block) in metas.into_iter().zip(blocks) {
        meta.start = acc;
        meta.repeat = meta.repeat.filter(|&x| x != 0);
        acc += block.len();
        new_measures.extend(block);
        new_sections.push(meta);
    }
    song.measures = new_measures;
    song.sections = Some(new_sections);
}

pub fn rename_section(song: &mut Song, id: &str, name: &str) {
    if let Some(s) = song.sections.as_mut().and_then(|v| v.iter_mut().find(|s| s.id == id)) {
        s.name = name.to_string();
    }
}

/// Ritornello: quantas vezes a seção toca, limitado a 1..=16 (1 = sem
/// repetição → remove o campo).
pub fn set_section_repeat(song: &mut Song, id: &str, times: u32) {
    if let Some(s) = song.sections.as_mut().and_then(|v| v.iter_mut().find(|s| s.id == id)) {
        let n = times.clamp(1, 16);
        s.repeat = (n != 1).then_some(n);
    }
}

/// Remove a seção E os compassos dela. A música nunca fica sem compasso (sobra
/// um vazio). UI: tire o id da seleção e zere o `startCell` depois de chamar.
pub fn remove_section(song: &mut Song, id: &str) {
    if song.sections.is_none() {
        return;
    }
    let range = section_ranges(song).into_iter().find(|r| r.id == id);
    if let Some(sections) = song.sections.as_mut() {
        sections.retain(|s| s.id != id);
    }
    let Some(range) = range else { return };
    let n = song.measures.len();
    let count = range.len();
    let reference = song.measures.get(range.start).map(|m| (m.beats, m.subdivision));
    let from = range.start.min(n);
    let to = (range.start + count).min(n);
    song.measures.drain(from..to);
    for s in song.sections.iter_mut().flatten() {
        if s.start > range.start {
            s.start = s.start.saturating_sub(count);
        }
    }
    // música não fica sem compasso
    if song.measures.is_empty() {
        let (beats, sub) = reference.unwrap_or((4, 2));
        let fresh = make_measure(song, beats, sub);
        song.measures.push(fresh);
    }
}
