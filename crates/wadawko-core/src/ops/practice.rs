//! Trechos de ensaio com nome (F7b de `docs/planejamento/daw.md`): guardar o
//! trecho de loop atual com um nome, renomear, remover e resolver de volta
//! para compassos.
//!
//! Cada ponta é ancorada na SEÇÃO (id + compasso dentro dela): mover ou
//! clonar seções e acrescentar compassos no fim de uma seção não desloca o
//! trecho. Seção apagada → o trecho não resolve (`resolve` = `None`); a UI o
//! mostra apagado e deixa remover.

use crate::ids::{IdKind, new_id};
use crate::model::{Extra, PracticeAnchor, PracticeSpot, Song};
use crate::ops::sections::{section_of_measure, section_ranges};
use crate::timeline::Region;

/// Âncora do compasso `mi` (seção que o contém + deslocamento).
pub fn anchor_of(song: &Song, mi: usize) -> PracticeAnchor {
    match section_of_measure(song, mi) {
        Some(s) => PracticeAnchor { section: s.id, bar: mi - s.start },
        None => PracticeAnchor { section: String::new(), bar: mi },
    }
}

/// Compasso de uma âncora; preso ao fim da seção se ela encolheu. `None` se a
/// seção não existe mais (ou a música ficou sem compassos).
pub fn bar_of(song: &Song, a: &PracticeAnchor) -> Option<usize> {
    if song.measures.is_empty() {
        return None;
    }
    if a.section.is_empty() {
        return Some(a.bar.min(song.measures.len() - 1));
    }
    let s = section_ranges(song).into_iter().find(|s| s.id == a.section)?;
    Some(s.start + a.bar.min(s.last()? - s.start))
}

/// Região `[start, end)` do trecho, ou `None` se alguma ponta se perdeu.
pub fn resolve(song: &Song, spot: &PracticeSpot) -> Option<Region> {
    let (a, b) = (bar_of(song, &spot.start)?, bar_of(song, &spot.end)?);
    Some(Region { start_bar: a.min(b), end_bar: a.max(b) + 1 })
}

pub fn spots(song: &Song) -> &[PracticeSpot] {
    song.practice.as_deref().unwrap_or(&[])
}

/// Guarda `r` como trecho chamado `name` (vazio = "c3–c6"). Devolve o id.
pub fn add_spot(song: &mut Song, name: &str, r: Region) -> Option<String> {
    if r.end_bar <= r.start_bar || r.start_bar >= song.measures.len() {
        return None;
    }
    let last = r.end_bar.min(song.measures.len()) - 1;
    let name = match name.trim() {
        "" => default_name(r),
        n => n.to_string(),
    };
    let id = new_id(IdKind::Section, "tr");
    let spot = PracticeSpot { id: id.clone(), name, start: anchor_of(song, r.start_bar), end: anchor_of(song, last), extra: Extra::new() };
    song.practice.get_or_insert_with(Vec::new).push(spot);
    Some(id)
}

pub fn rename_spot(song: &mut Song, id: &str, name: &str) {
    if let Some(s) = song.practice.as_mut().and_then(|v| v.iter_mut().find(|s| s.id == id))
        && !name.trim().is_empty()
    {
        s.name = name.trim().to_string();
    }
}

pub fn remove_spot(song: &mut Song, id: &str) {
    if let Some(v) = song.practice.as_mut() {
        v.retain(|s| s.id != id);
        if v.is_empty() {
            song.practice = None;
        }
    }
}

/// Nome padrão: `c3–c6` (1-based, fim inclusivo) ou `c3`.
pub fn default_name(r: Region) -> String {
    if r.end_bar <= r.start_bar + 1 { format!("c{}", r.start_bar + 1) } else { format!("c{}–c{}", r.start_bar + 1, r.end_bar) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo_song;
    use crate::ops::measures::add_measure;
    use crate::ops::sections::{append_section, move_section, remove_section};

    fn song_with_two_sections() -> (Song, String, String) {
        let mut s = demo_song();
        // garante duas seções de 2 compassos cada
        s.measures.truncate(2);
        s.sections = None;
        let a = append_section(&mut s, Some("A"));
        let b = append_section(&mut s, Some("B"));
        (s, a, b)
    }

    #[test]
    fn guarda_e_resolve() {
        let mut s = demo_song();
        let n = s.measures.len();
        assert!(n >= 3);
        let id = add_spot(&mut s, "", Region { start_bar: 1, end_bar: 3 }).unwrap();
        let spot = spots(&s)[0].clone();
        assert_eq!(spot.id, id);
        assert_eq!(spot.name, "c2–c3");
        assert_eq!(resolve(&s, &spot), Some(Region { start_bar: 1, end_bar: 3 }));
        rename_spot(&mut s, &id, " entrada do B ");
        assert_eq!(spots(&s)[0].name, "entrada do B");
        // ida e volta pelo JSON
        let back: Song = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(spots(&back), spots(&s));
        remove_spot(&mut s, &id);
        assert!(s.practice.is_none());
        // região vazia / fora da música
        assert!(add_spot(&mut s, "x", Region { start_bar: 2, end_bar: 2 }).is_none());
        assert!(add_spot(&mut s, "x", Region { start_bar: n, end_bar: n + 1 }).is_none());
    }

    #[test]
    fn acompanha_seções() {
        let (mut s, a, b) = song_with_two_sections();
        let rb = section_ranges(&s).into_iter().find(|x| x.id == b).unwrap();
        // trecho = toda a seção B
        let id = add_spot(&mut s, "B", rb.region()).unwrap();
        let spot = |s: &Song| spots(s).iter().find(|x| x.id == id).unwrap().clone();
        assert_eq!(resolve(&s, &spot(&s)), Some(rb.region()));
        // compasso a mais na seção A: B anda um compasso e o trecho vai junto
        add_measure(&mut s, Some(&a));
        let rb2 = section_ranges(&s).into_iter().find(|x| x.id == b).unwrap();
        assert_eq!(rb2.start, rb.start + 1);
        assert_eq!(resolve(&s, &spot(&s)), Some(rb2.region()));
        // mover B para o começo: o trecho segue a seção
        move_section(&mut s, &b, 0);
        let rb3 = section_ranges(&s).into_iter().find(|x| x.id == b).unwrap();
        assert_eq!(resolve(&s, &spot(&s)), Some(rb3.region()));
        // apagar B: o trecho se perde
        remove_section(&mut s, &b);
        assert_eq!(resolve(&s, &spot(&s)), None);
    }
}
