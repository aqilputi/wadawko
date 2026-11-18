//! Estrutura da música para o painel de seções (`docs/planejamento/tracker.md`
//! §3 e §5): cor de seção e de grupo, grupos de seções (contíguos; só
//! organização — não mudam a ordem tocada) e copiar/colar seções como JSON
//! (funciona entre músicas).

use serde::{Deserialize, Serialize};

use crate::ids::{IdKind, new_id};
use crate::model::{CellRows, Measure, Section, SectionGroup, Song};
use crate::ops::sections::{move_section, section_ranges};

/// `#rrggbb` válido?
pub fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn clean_color(c: Option<&str>) -> Option<String> {
    c.map(str::trim).filter(|c| is_hex_color(c)).map(str::to_ascii_lowercase)
}

// ───────────────────────────── cores ─────────────────────────────

/// Cor da seção (`None` = herda do grupo / sem cor).
pub fn set_section_color(song: &mut Song, id: &str, color: Option<&str>) {
    let color = clean_color(color);
    if let Some(s) = song.sections.as_mut().and_then(|v| v.iter_mut().find(|s| s.id == id)) {
        s.color = color;
    }
}

/// Cor efetiva da seção: a dela, senão a do grupo.
pub fn section_color<'a>(song: &'a Song, s: &'a Section) -> Option<&'a str> {
    s.color.as_deref().or_else(|| s.group.as_deref().and_then(|g| group(song, g)).and_then(|g| g.color.as_deref()))
}

/// Cor de um instrumento (def do Kit): custom grava na def; pronto grava em
/// `Song.instrument_colors`. `None` = volta à cor padrão do instrumento base.
pub fn set_instrument_color(song: &mut Song, id: &str, color: Option<&str>) {
    let color = clean_color(color);
    if let Some(def) = crate::ops::kit::custom_instrument_mut(song, id) {
        def.color = color;
        return;
    }
    let map = song.instrument_colors.get_or_insert_default();
    match color {
        Some(c) => {
            map.insert(id.to_string(), c);
        }
        None => {
            map.remove(id);
        }
    }
    if map.is_empty() {
        song.instrument_colors = None;
    }
}

/// Cor escolhida para o instrumento (def), se houver.
pub fn instrument_color<'a>(song: &'a Song, def_id: &str) -> Option<&'a str> {
    if let Some(c) = song.instrument_colors.as_ref().and_then(|m| m.get(def_id)) {
        return Some(c);
    }
    song.instruments().iter().find(|d| d.id == def_id).and_then(|d| d.color.as_deref())
}

/// Cor efetiva da linha: a própria, senão a do instrumento, senão `None` (o
/// chamador usa a padrão do instrumento base).
pub fn track_color<'a>(song: &'a Song, track: &'a crate::model::Track) -> Option<&'a str> {
    track.color.as_deref().or_else(|| instrument_color(song, crate::ops::kit::resolve_instrument(song, track).id.as_str()))
}

// ───────────────────────────── grupos ─────────────────────────────

pub fn groups(song: &Song) -> &[SectionGroup] {
    song.section_groups.as_deref().unwrap_or(&[])
}

pub fn group<'a>(song: &'a Song, id: &str) -> Option<&'a SectionGroup> {
    groups(song).iter().find(|g| g.id == id)
}

fn group_mut<'a>(song: &'a mut Song, id: &str) -> Option<&'a mut SectionGroup> {
    song.section_groups.as_mut()?.iter_mut().find(|g| g.id == id)
}

/// Agrupa as seções `ids` num grupo novo chamado `name`. Seções fora de ordem
/// são movidas (com os compassos) para ficarem juntas, a partir da primeira.
/// Seções que já estavam noutro grupo saem dele. Devolve o id do grupo.
pub fn group_sections(song: &mut Song, ids: &[String], name: &str) -> Option<String> {
    let order: Vec<String> = section_ranges(song).into_iter().map(|r| r.id).collect();
    let mut chosen: Vec<&String> = order.iter().filter(|id| ids.contains(id)).collect();
    if chosen.is_empty() {
        return None;
    }
    chosen.dedup();
    let chosen: Vec<String> = chosen.into_iter().cloned().collect();
    let first = order.iter().position(|id| *id == chosen[0])?;
    for (k, id) in chosen.iter().enumerate().skip(1) {
        move_section(song, id, first + k);
    }
    let gid = new_id(IdKind::Section, "grp");
    for s in song.sections.iter_mut().flatten() {
        if chosen.contains(&s.id) {
            s.group = Some(gid.clone());
        }
    }
    song.section_groups.get_or_insert_default().push(SectionGroup { id: gid.clone(), name: if name.trim().is_empty() { "Grupo".into() } else { name.trim().into() }, ..Default::default() });
    prune_groups(song);
    Some(gid)
}

/// Desfaz o grupo (as seções ficam, sem grupo).
pub fn ungroup(song: &mut Song, gid: &str) {
    for s in song.sections.iter_mut().flatten() {
        if s.group.as_deref() == Some(gid) {
            s.group = None;
        }
    }
    prune_groups(song);
}

/// Tira uma seção do grupo dela.
pub fn leave_group(song: &mut Song, section_id: &str) {
    if let Some(s) = song.sections.as_mut().and_then(|v| v.iter_mut().find(|s| s.id == section_id)) {
        s.group = None;
    }
    prune_groups(song);
}

pub fn rename_group(song: &mut Song, gid: &str, name: &str) {
    if let Some(g) = group_mut(song, gid)
        && !name.trim().is_empty()
    {
        g.name = name.trim().into();
    }
}

pub fn set_group_color(song: &mut Song, gid: &str, color: Option<&str>) {
    let color = clean_color(color);
    if let Some(g) = group_mut(song, gid) {
        g.color = color;
    }
}

pub fn set_group_collapsed(song: &mut Song, gid: &str, collapsed: bool) {
    if let Some(g) = group_mut(song, gid) {
        g.collapsed = collapsed;
    }
}

/// Seções do grupo, na ordem tocada.
pub fn group_sections_ids(song: &Song, gid: &str) -> Vec<String> {
    let mut v: Vec<&Section> = song.sections().iter().filter(|s| s.group.as_deref() == Some(gid)).collect();
    v.sort_by_key(|s| s.start);
    v.into_iter().map(|s| s.id.clone()).collect()
}

/// Mantém os grupos coerentes: some grupo sem seção, some referência a grupo
/// inexistente, e um grupo partido (seções não contíguas) fica só com o
/// primeiro trecho contíguo — o resto sai do grupo.
pub fn prune_groups(song: &mut Song) {
    let ids: Vec<String> = groups(song).iter().map(|g| g.id.clone()).collect();
    let order: Vec<String> = section_ranges(song).into_iter().map(|r| r.id).collect();
    let group_of = |song: &Song, sid: &str| song.section(sid).and_then(|s| s.group.clone());
    let mut drop_from: Vec<String> = Vec::new();
    for gid in &ids {
        let mut seen_run = false;
        let mut in_run = false;
        for sid in &order {
            let mine = group_of(song, sid).as_deref() == Some(gid.as_str());
            if mine && seen_run && !in_run {
                drop_from.push(sid.clone());
            }
            if mine {
                seen_run = true;
            }
            in_run = mine;
        }
    }
    for s in song.sections.iter_mut().flatten() {
        if drop_from.contains(&s.id) || s.group.as_ref().is_some_and(|g| !ids.contains(g)) {
            s.group = None;
        }
    }
    let used: Vec<String> = song.sections().iter().filter_map(|s| s.group.clone()).collect();
    if let Some(gs) = song.section_groups.as_mut() {
        gs.retain(|g| used.contains(&g.id));
        if gs.is_empty() {
            song.section_groups = None;
        }
    }
}

// ───────────────────────────── copiar / colar ─────────────────────────────

/// Seções copiadas (o que vai para a área de transferência).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    /// marcador do formato (sempre 1)
    pub wadawko_sections: u32,
    /// ids das linhas da música de origem, na ordem (para colar noutra música)
    pub tracks: Vec<String>,
    pub sections: Vec<ClipSection>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipSection {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub measures: Vec<Measure>,
}

/// JSON das seções `ids` (na ordem tocada), com compassos, notas, donos,
/// kiai, dinâmicas e efeitos.
pub fn copy_sections(song: &Song, ids: &[String]) -> Option<String> {
    let n = song.measures.len();
    let sections: Vec<ClipSection> = section_ranges(song)
        .into_iter()
        .filter(|r| ids.contains(&r.id))
        .map(|r| {
            let color = song.section(&r.id).and_then(|s| section_color(song, s)).map(str::to_string);
            ClipSection { name: r.name.clone(), repeat: r.repeat.filter(|&x| x > 1), color, measures: song.measures[r.start.min(n)..r.end_excl.min(n)].to_vec() }
        })
        .collect();
    if sections.is_empty() {
        return None;
    }
    serde_json::to_string(&Clip { wadawko_sections: 1, tracks: song.tracks.iter().map(|t| t.id.clone()).collect(), sections }).ok()
}

/// É um JSON de seções copiadas?
pub fn is_clip(text: &str) -> bool {
    text.trim_start().starts_with('{') && text.contains("\"wadawkoSections\"")
}

/// Cola as seções do `text` depois da seção `after` (`None` = no fim). As linhas
/// casam pelo id; se a origem é outra música, pela posição. Devolve os ids das
/// seções novas.
pub fn paste_sections(song: &mut Song, text: &str, after: Option<&str>) -> Result<Vec<String>, String> {
    let clip: Clip = serde_json::from_str(text).map_err(|e| format!("não são seções copiadas: {e}"))?;
    if clip.sections.is_empty() {
        return Ok(Vec::new());
    }
    let dest: Vec<String> = song.tracks.iter().map(|t| t.id.clone()).collect();
    // linha de origem → linha de destino
    let map = |src: &str| -> Option<String> {
        if dest.iter().any(|d| d == src) {
            return Some(src.to_string());
        }
        clip.tracks.iter().position(|t| t == src).and_then(|i| dest.get(i).cloned())
    };
    let remap = |rows: &CellRows, n: usize| -> CellRows {
        let mut out = CellRows::new();
        for (k, v) in rows {
            if let Some(d) = map(k) {
                let mut v = v.clone();
                v.resize(n, None);
                out.insert(d, v);
            }
        }
        out
    };
    // posição: fim da seção `after`, senão fim da música
    let at = after.and_then(|a| section_ranges(song).into_iter().find(|r| r.id == a)).map_or(song.measures.len(), |r| r.end_excl);
    let mut new_measures = Vec::new();
    let mut new_secs = Vec::new();
    for cs in &clip.sections {
        let start = at + new_measures.len();
        for m in &cs.measures {
            let n = m.cell_count();
            let mut m2 = m.clone();
            m2.cells = remap(&m.cells, n);
            for t in &dest {
                m2.cells.entry(t.clone()).or_insert_with(|| vec![None; n]);
            }
            m2.cell_owners = m.cell_owners.as_ref().map(|r| remap(r, n)).filter(|r| !r.is_empty());
            m2.kyai = m.kyai.as_ref().map(|r| remap(r, n)).filter(|r| !r.is_empty());
            m2.fx = m.fx.as_ref().map(|r| remap(r, n)).filter(|r| !r.is_empty());
            m2.track_dynamics =
                m.track_dynamics.as_ref().map(|td| td.iter().filter_map(|(k, v)| map(k).map(|d| (d, v.clone()))).collect()).filter(|td: &std::collections::BTreeMap<_, _>| !td.is_empty());
            new_measures.push(m2);
        }
        if cs.measures.is_empty() {
            continue;
        }
        new_secs.push(Section {
            id: new_id(IdKind::Section, "sec"),
            name: cs.name.clone(),
            start,
            repeat: cs.repeat.filter(|&x| x > 1),
            color: clean_color(cs.color.as_deref()),
            ..Default::default()
        });
    }
    let count = new_measures.len();
    let at = at.min(song.measures.len());
    song.measures.splice(at..at, new_measures);
    let had_sections = song.sections.is_some();
    let sections = song.sections.get_or_insert_default();
    for s in sections.iter_mut() {
        if s.start >= at {
            s.start += count;
        }
    }
    // música sem seções: o que já existia vira uma seção, para as coladas não "engolirem" o resto
    if !had_sections && at > 0 {
        sections.push(Section { id: new_id(IdKind::Section, "sec"), name: "Seção 1".into(), start: 0, ..Default::default() });
    }
    let ids: Vec<String> = new_secs.iter().map(|s| s.id.clone()).collect();
    sections.extend(new_secs);
    sections.sort_by_key(|s| s.start);
    // compassos depois das coladas que estavam numa seção sem seção nova seguinte continuam dela
    prune_groups(song);
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::cells::{set_cell, set_fx};
    use crate::ops::sections::{append_section, remove_section};
    use crate::templates::template_song;

    fn song3() -> (Song, Vec<String>) {
        let s = template_song("ensemble5").unwrap();
        let ids = section_ranges(&s).into_iter().map(|r| r.id).collect();
        (s, ids)
    }

    #[test]
    fn grupo_contiguo_e_limpeza() {
        let (mut s, ids) = song3(); // Intro, Tema, Final
        // agrupa Intro + Final (não contíguas): Final vem para depois da Intro
        let g = group_sections(&mut s, &[ids[2].clone(), ids[0].clone()], "Abertura").unwrap();
        let order: Vec<String> = section_ranges(&s).into_iter().map(|r| r.id).collect();
        assert_eq!(order, vec![ids[0].clone(), ids[2].clone(), ids[1].clone()]);
        assert_eq!(group_sections_ids(&s, &g), vec![ids[0].clone(), ids[2].clone()]);
        set_group_color(&mut s, &g, Some("#5E9FD8"));
        let intro = s.section(&ids[0]).unwrap().clone();
        assert_eq!(section_color(&s, &intro), Some("#5e9fd8"), "seção sem cor herda a do grupo");
        set_section_color(&mut s, &ids[0], Some("#d9775f"));
        assert_eq!(section_color(&s, s.section(&ids[0]).unwrap()), Some("#d9775f"));
        set_section_color(&mut s, &ids[0], Some("vermelho"));
        assert_eq!(s.section(&ids[0]).unwrap().color, None, "cor inválida limpa");
        // remover as seções do grupo faz o grupo sumir
        remove_section(&mut s, &ids[0]);
        remove_section(&mut s, &ids[2]);
        prune_groups(&mut s);
        assert!(s.section_groups.is_none());
        // JSON ida e volta com grupo
        let g2 = group_sections(&mut s, &[ids[1].clone()], "B").unwrap();
        let back = Song::from_json(&s.to_json()).unwrap();
        assert_eq!(back.section_groups.as_ref().unwrap()[0].id, g2);
        assert_eq!(back.section(&ids[1]).unwrap().group.as_deref(), Some(g2.as_str()));
        ungroup(&mut s, &g2);
        assert!(s.section_groups.is_none());
    }

    #[test]
    fn copiar_colar_leva_tudo() {
        let (mut s, ids) = song3();
        let t0 = s.tracks[1].id.clone();
        let tema = section_ranges(&s).into_iter().find(|r| r.id == ids[1]).unwrap();
        set_cell(&mut s, &t0, tema.start, 0, Some("DON"), None);
        set_fx(&mut s, &t0, tema.start, 0, Some("r4<")).unwrap();
        set_section_color(&mut s, &ids[1], Some("#8fb86a"));
        let clip = copy_sections(&s, &[ids[1].clone()]).unwrap();
        assert!(is_clip(&clip));
        let before = s.measures.len();
        let new = paste_sections(&mut s, &clip, Some(&ids[2])).unwrap();
        assert_eq!(new.len(), 1);
        let r = section_ranges(&s).into_iter().find(|r| r.id == new[0]).unwrap();
        assert_eq!(r.start, before, "colou no fim (depois do Final)");
        assert_eq!(r.len(), tema.len());
        assert_eq!(r.repeat, Some(2));
        assert_eq!(s.measures[r.start].cell(&t0, 0), Some("DON"));
        assert_eq!(s.measures[r.start].fx_at(&t0, 0), Some("R4<"));
        assert_eq!(s.section(&new[0]).unwrap().color.as_deref(), Some("#8fb86a"));
        // colar noutra música: as linhas casam pela posição
        let mut other = template_song("ensemble5").unwrap();
        let ot = other.tracks[1].id.clone();
        let n0 = other.measures.len();
        let new2 = paste_sections(&mut other, &clip, None).unwrap();
        let r2 = section_ranges(&other).into_iter().find(|r| r.id == new2[0]).unwrap();
        assert_eq!(r2.start, n0);
        assert_eq!(other.measures[r2.start].cell(&ot, 0), Some("DON"));
        assert!(other.measures[r2.start].cells.keys().all(|k| other.tracks.iter().any(|t| &t.id == k)));
        // música sem seções
        let mut plain = template_song("vazia").unwrap();
        plain.sections = None;
        append_section(&mut plain, None);
        assert!(paste_sections(&mut plain, "lixo", None).is_err());
    }
}
