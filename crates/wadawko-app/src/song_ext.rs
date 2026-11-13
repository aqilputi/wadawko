//! Ponte entre os índices que a grade usa (seção `s`, compasso `m`, linha `t`,
//! célula `k`, coluna global `g`) e o modelo do `wadawko-core` (ids de linha e
//! de seção, `cells` por id). Só leitura: toda escrita passa por
//! `AppState::edit` + `wadawko_core::ops`.

use wadawko_core::Song;
use wadawko_core::ops::people::{cell_owner_of, person_audible};
use wadawko_core::ops::sections::section_ranges;

/// Bloco de seção como a grade mostra. `id == None` = bloco virtual (música sem
/// seções, ou compassos antes da 1ª seção).
#[derive(Clone, Debug, PartialEq)]
pub struct Sec {
    pub id: Option<String>,
    pub name: String,
    pub start: usize,
    pub end: usize,
    pub repeat: u32,
}

pub trait SongExt {
    fn secs(&self) -> Vec<Sec>;
    fn section_range(&self, s: usize) -> (usize, usize);
    fn section_of(&self, m: usize) -> usize;
    fn mlen(&self, m: usize) -> usize;
    fn flat(&self, m: usize, k: usize) -> usize;
    fn unflat(&self, g: usize) -> (usize, usize);
    fn tid(&self, t: usize) -> &str;
    fn cell(&self, t: usize, m: usize, k: usize) -> Option<&str>;
    fn cell_at(&self, t: usize, g: usize) -> Option<&str>;
    /// linha audível segundo mudo/solo das LINHAS
    fn audible(&self, t: usize) -> bool;
    /// dono efetivo da célula está audível (mudo/solo das PESSOAS)?
    fn owner_audible(&self, t: usize, m: usize, k: usize) -> bool;
    /// subdivisão vigente (a do 1º compasso)
    fn sub(&self) -> usize;
}

impl SongExt for Song {
    fn secs(&self) -> Vec<Sec> {
        let n = self.measures.len();
        let mut out = Vec::new();
        let ranges = section_ranges(self);
        let first = ranges.first().map(|r| r.start).unwrap_or(n);
        if first > 0 || ranges.is_empty() {
            out.push(Sec { id: None, name: if ranges.is_empty() { "Música".into() } else { "Início".into() }, start: 0, end: first.min(n), repeat: 1 });
        }
        for r in ranges {
            out.push(Sec { id: Some(r.id), name: r.name, start: r.start.min(n), end: r.end_excl.min(n), repeat: r.repeat.unwrap_or(1).max(1) });
        }
        out
    }

    fn section_range(&self, s: usize) -> (usize, usize) {
        self.secs().get(s).map(|x| (x.start, x.end)).unwrap_or((0, 0))
    }

    fn section_of(&self, m: usize) -> usize {
        let secs = self.secs();
        secs.iter().rposition(|x| x.start <= m).unwrap_or(0)
    }

    fn mlen(&self, m: usize) -> usize {
        self.measures.get(m).map(|x| x.cell_count()).unwrap_or(0)
    }

    fn flat(&self, m: usize, k: usize) -> usize {
        self.measures.iter().take(m).map(|x| x.cell_count()).sum::<usize>() + k
    }

    fn unflat(&self, g: usize) -> (usize, usize) {
        let mut acc = 0;
        for (m, meas) in self.measures.iter().enumerate() {
            let n = meas.cell_count();
            if g < acc + n {
                return (m, g - acc);
            }
            acc += n;
        }
        let last = self.measures.len().saturating_sub(1);
        (last, self.mlen(last).saturating_sub(1))
    }

    fn tid(&self, t: usize) -> &str {
        self.tracks.get(t).map(|x| x.id.as_str()).unwrap_or("")
    }

    fn cell(&self, t: usize, m: usize, k: usize) -> Option<&str> {
        self.measures.get(m)?.cell(self.tid(t), k)
    }

    fn cell_at(&self, t: usize, g: usize) -> Option<&str> {
        if g >= self.total_cells() {
            return None;
        }
        let (m, k) = self.unflat(g);
        self.cell(t, m, k)
    }

    fn audible(&self, t: usize) -> bool {
        let Some(tr) = self.tracks.get(t) else { return false };
        let any_solo = self.tracks.iter().any(|x| x.solo);
        !tr.muted && (!any_solo || tr.solo)
    }

    fn owner_audible(&self, t: usize, m: usize, k: usize) -> bool {
        let Some(tr) = self.tracks.get(t) else { return true };
        if tr.person_ids().is_empty() {
            return true;
        }
        person_audible(self, cell_owner_of(self, tr, m, k))
    }

    fn sub(&self) -> usize {
        self.measures.first().map(|m| m.subdivision).unwrap_or(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_unflat_roundtrip_on_demo() {
        let song = wadawko_core::demo::demo_song();
        let total = song.total_cells();
        assert!(total > 0);
        for g in 0..total {
            let (m, k) = song.unflat(g);
            assert_eq!(song.flat(m, k), g);
        }
        let secs = song.secs();
        assert!(!secs.is_empty());
        assert_eq!(secs.first().unwrap().start, 0);
        assert_eq!(secs.last().unwrap().end, song.measures.len());
        for (s, sec) in secs.iter().enumerate() {
            for m in sec.start..sec.end {
                assert_eq!(song.section_of(m), s);
            }
        }
    }

    #[test]
    fn song_without_sections_gets_one_virtual_block() {
        let mut song = wadawko_core::demo::demo_song();
        song.sections = None;
        let secs = song.secs();
        assert_eq!(secs.len(), 1);
        assert_eq!(secs[0].id, None);
        assert_eq!((secs[0].start, secs[0].end), (0, song.measures.len()));
    }
}
