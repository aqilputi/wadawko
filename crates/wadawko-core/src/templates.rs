//! Modelos de música (F9 de `docs/planejamento/daw.md`): pontos de partida
//! prontos — linhas, seções e um jiuchi já escrito — para começar rápido.

use crate::demo::demo_song;
use crate::ids::{IdKind, new_id};
use crate::model::{Extra, Measure, Section, Song};
use crate::ops::cells::set_cell;
use crate::ops::tracks::add_track;

/// Um modelo da lista.
pub struct Template {
    pub id: &'static str,
    pub name: &'static str,
    /// uma linha para a paleta
    pub about: &'static str,
}

pub const TEMPLATES: &[Template] = &[
    Template { id: "renshu", name: "Renshū (exemplo)", about: "3 linhas e 4 compassos já escritos — a música de exemplo" },
    Template { id: "vazia", name: "Em branco", about: "1 linha de chū, 4 compassos 4/4 vazios" },
    Template { id: "jiuchi3", name: "Jiuchi + 3 linhas", about: "shime com jiuchi escrito, chū, nagadō e ōdaiko; intro e tema" },
    Template { id: "ensemble5", name: "Ensemble 5", about: "shime, 2 chū, nagadō, ōdaiko e kane; intro, tema (2×) e final" },
    Template { id: "matsuri", name: "Matsuri (swing)", about: "jiuchi de matsuri no shime em swing, chū e kane; 110 BPM" },
];

/// A música nova do modelo `id` (`None` se o id não existe).
pub fn template_song(id: &str) -> Option<Song> {
    Some(match id {
        "renshu" => demo_song(),
        "vazia" => build("Nova música", 100.0, &["chu"], &[("A", 4, 1)], &[]),
        "jiuchi3" => build("Nova música", 100.0, &["shime", "chu", "nagado", "odaiko"], &[("Intro", 2, 1), ("Tema", 4, 1)], &[(0, &["ten", "ten", "ten", "ten", "ten", "ten", "ten", "ten"])]),
        "ensemble5" => build(
            "Nova música",
            110.0,
            &["shime", "chu", "chu", "nagado", "odaiko", "kane"],
            &[("Intro", 2, 1), ("Tema", 4, 2), ("Final", 2, 1)],
            &[(0, &["don", "ko", "don", "ko", "don", "ko", "don", "ko"]), (5, &["don", "", "don", "", "don", "", "don", ""])],
        ),
        "matsuri" => {
            let mut s = build(
                "Nova música",
                110.0,
                &["shime", "chu", "kane"],
                &[("Tema", 4, 1)],
                &[(0, &["su", "ten", "su", "ten", "su", "ten", "su", "ten"]), (2, &["don", "", "ko", "ko", "don", "", "ko", "ko"])],
            );
            s.swing = Some(1.0);
            s
        }
        _ => return None,
    })
}

/// Música com as linhas dadas (ids de instrumento builtin), seções
/// `(nome, compassos, repetições)` em 4/4 com colcheias, e padrões de 8
/// células que se repetem em todos os compassos da linha de índice dado.
fn build(title: &str, tempo: f64, instruments: &[&str], sections: &[(&str, usize, u32)], patterns: &[(usize, &[&str])]) -> Song {
    let mut song = Song { version: 1, title: title.into(), tempo, ..Default::default() };
    let bars: usize = sections.iter().map(|s| s.1).sum();
    song.measures = (0..bars).map(|_| Measure::new(4, 2, std::iter::empty())).collect();
    let ids: Vec<String> = instruments.iter().map(|i| add_track(&mut song, i)).collect();
    let mut start = 0;
    let mut secs = Vec::new();
    for &(name, n, rep) in sections {
        secs.push(Section { id: new_id(IdKind::Section, "s"), name: name.into(), start, repeat: (rep > 1).then_some(rep), extra: Extra::new() });
        start += n;
    }
    song.sections = Some(secs);
    for &(ti, pat) in patterns {
        let Some(tid) = ids.get(ti) else { continue };
        for mi in 0..bars {
            for (ci, syl) in pat.iter().enumerate() {
                if !syl.is_empty() {
                    set_cell(&mut song, tid, mi, ci, Some(syl), None);
                }
            }
        }
    }
    song
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::sections::section_ranges;

    #[test]
    fn todos_os_modelos_montam_e_voltam_do_json() {
        for t in TEMPLATES {
            let s = template_song(t.id).unwrap_or_else(|| panic!("modelo {}", t.id));
            assert!(!s.tracks.is_empty() && !s.measures.is_empty(), "{}", t.id);
            // cada compasso tem uma fileira por linha, do tamanho certo
            for m in &s.measures {
                for tr in &s.tracks {
                    assert_eq!(m.cells.get(&tr.id).map(Vec::len), Some(m.cell_count()), "{} {}", t.id, tr.id);
                }
            }
            let back = Song::from_json(&s.to_json()).unwrap();
            assert_eq!(back.tracks.len(), s.tracks.len());
        }
        assert!(template_song("nao-existe").is_none());
        let e = template_song("ensemble5").unwrap();
        assert_eq!(e.tracks.len(), 6);
        let secs = section_ranges(&e);
        assert_eq!(secs.iter().map(|s| (s.name.as_str(), s.len(), s.repeat)).collect::<Vec<_>>(), vec![("Intro", 2, None), ("Tema", 4, Some(2)), ("Final", 2, None)]);
        // jiuchi escrito no shime em todos os compassos
        let shime = &e.tracks[0].id;
        assert!(e.measures.iter().all(|m| m.cells[shime].iter().filter(|c| c.is_some()).count() == 8));
        assert_eq!(template_song("matsuri").unwrap().swing, Some(1.0));
    }
}
