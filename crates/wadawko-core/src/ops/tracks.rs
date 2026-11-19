//! Linhas (tracks): adicionar por InstrumentDef, remover, renomear, trocar de
//! instrumento, reordenar, destaque "main" e cor.

use crate::ids::{IdKind, new_id};
use crate::model::{BUILTIN_INSTRUMENTS, Song, TRACK_COLORS, Track};
use crate::ops::kit::instrument_by_id;
use crate::timeline::main_tracks;

/// Adiciona uma linha a partir de uma `InstrumentDef` (builtin id ou custom
/// id; id desconhecido cai no primeiro builtin, shime). O nome é o da def, com
/// sufixo " N" se já houver linhas dela. Todos os compassos ganham a linha
/// vazia. Retorna o id da linha (`<base>-<tempo36>-<n>`).
pub fn add_track(song: &mut Song, def_id: &str) -> String {
    let def = instrument_by_id(song, Some(def_id)).unwrap_or(&BUILTIN_INSTRUMENTS[0]).clone();
    let id = new_id(IdKind::Track, def.base.as_str());
    let count = song.tracks.iter().filter(|t| t.def_id() == def.id).count();
    song.tracks.push(Track {
        id: id.clone(),
        name: if count > 0 { format!("{} {}", def.name, count + 1) } else { def.name.clone() },
        instrument: def.base,
        instrument_id: Some(def.id.clone()),
        ..Default::default()
    });
    for meas in &mut song.measures {
        meas.cells.insert(id.clone(), vec![None; meas.cell_count()]);
    }
    id
}

/// Remove a linha e tudo dela nos compassos (notas, donos, kiai, dinâmicas
/// próprias) e do destaque. A coreografia/pessoas não são afetadas.
pub fn remove_track(song: &mut Song, track_id: &str) {
    song.tracks.retain(|t| t.id != track_id);
    let main = main_tracks(song);
    set_main_tracks(song, main);
    for meas in &mut song.measures {
        meas.cells.remove(track_id);
        if let Some(owners) = meas.cell_owners.as_mut() {
            owners.remove(track_id);
        }
        if let Some(kyai) = meas.kyai.as_mut() {
            kyai.remove(track_id);
        }
        if let Some(fx) = meas.fx.as_mut() {
            fx.remove(track_id);
            if fx.is_empty() {
                meas.fx = None;
            }
        }
        if let Some(td) = meas.track_dynamics.as_mut() {
            td.remove(track_id);
            if td.is_empty() {
                meas.track_dynamics = None;
            }
        }
    }
}

/// grava a lista de destaque (vazia = remove o campo); migra o formato antigo
fn set_main_tracks(song: &mut Song, ids: Vec<String>) {
    song.main_track_id = None;
    song.main_track_ids = (!ids.is_empty()).then_some(ids);
}

/// Liga/desliga o "main" de um instrumento (vários podem estar em destaque).
pub fn toggle_main_track(song: &mut Song, track_id: &str) {
    let mut cur = main_tracks(song);
    match cur.iter().position(|id| id == track_id) {
        Some(_) => cur.retain(|id| id != track_id),
        None => cur.push(track_id.to_string()),
    }
    set_main_tracks(song, cur);
}

pub fn is_main_track(song: &Song, track_id: &str) -> bool {
    main_tracks(song).iter().any(|id| id == track_id)
}

pub fn rename_track(song: &mut Song, track_id: &str, name: &str) {
    if let Some(t) = song.track_mut(track_id) {
        t.name = name.to_string();
    }
}

/// Troca o instrumento de uma linha (mantém as notas); atualiza base +
/// `instrumentId`. Def desconhecida = nada.
pub fn set_track_instrument(song: &mut Song, track_id: &str, def_id: &str) {
    let Some(def) = instrument_by_id(song, Some(def_id)) else { return };
    let (base, id) = (def.base, def.id.clone());
    if let Some(t) = song.track_mut(track_id) {
        t.instrument = base;
        t.instrument_id = Some(id);
    }
}

/// Move a linha na ordem (reflete em grade/partitura/esteira).
/// `dir` = -1 (cima) / +1 (baixo); fora dos limites = nada.
pub fn move_track(song: &mut Song, track_id: &str, dir: i32) {
    let Some(i) = song.track_index(track_id) else { return };
    let j = i as i64 + dir as i64;
    if j < 0 || j >= song.tracks.len() as i64 {
        return;
    }
    let x = song.tracks.remove(i);
    song.tracks.insert(j as usize, x);
}

/// Define a cor da linha (`None` = volta à cor padrão do instrumento).
pub fn set_track_color(song: &mut Song, track_id: &str, color: Option<&str>) {
    if let Some(t) = song.track_mut(track_id) {
        t.color = color.map(str::to_string);
    }
}

/// Próxima cor da paleta ([`TRACK_COLORS`]) depois da cor efetiva da linha;
/// uma cor fora da paleta volta à primeira.
pub fn cycle_track_color(song: &mut Song, track_id: &str) {
    let Some(t) = song.track_mut(track_id) else { return };
    let cur = t.color_or_default();
    let next = TRACK_COLORS.iter().position(|c| c.eq_ignore_ascii_case(cur)).map_or(0, |i| (i + 1) % TRACK_COLORS.len());
    t.color = Some(TRACK_COLORS[next].to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{InstrumentId, default_track_color};

    #[test]
    fn default_color_covers_every_instrument_and_is_distinct() {
        let mut seen = std::collections::HashSet::new();
        for base in InstrumentId::ALL {
            let c = default_track_color(base);
            assert!(TRACK_COLORS.contains(&c), "{base}: {c} não está na paleta");
            assert!(seen.insert(c), "{base}: cor {c} repetida");
        }
        assert_eq!(TRACK_COLORS.len(), 12);
        assert!(TRACK_COLORS.iter().all(|c| c.len() == 7 && c.starts_with('#')));
    }

    #[test]
    fn color_is_optional_and_round_trips() {
        let mut song = Song::default();
        song.tracks.push(Track::new("a", "A", InstrumentId::Shime));
        song.tracks.push(Track::new("b", "B", InstrumentId::Kane));
        // sem cor: o JSON não tem o campo e a cor efetiva é a do instrumento
        let json = song.to_json();
        assert!(!json.contains("\"color\""), "{json}");
        assert_eq!(song.tracks[0].color_or_default(), default_track_color(InstrumentId::Shime));
        set_track_color(&mut song, "a", Some("#123456"));
        assert_eq!(song.tracks[0].color_or_default(), "#123456");
        let json = song.to_json();
        assert!(json.contains("\"color\":\"#123456\""), "{json}");
        let back = Song::from_json(&json).unwrap();
        assert_eq!(back, song);
        assert_eq!(back.tracks[1].color, None);
        set_track_color(&mut song, "a", None);
        assert_eq!(song.tracks[0].color, None);
        // música antiga (sem o campo) continua lendo igual
        let old = r#"{"version":1,"title":"x","tempo":120,"tracks":[{"id":"a","name":"A","instrument":"chu"}],"measures":[]}"#;
        let s = Song::from_json(old).unwrap();
        assert_eq!(s.tracks[0].color, None);
        assert_eq!(s.tracks[0].color_or_default(), default_track_color(InstrumentId::Chu));
    }

    #[test]
    fn cycle_walks_the_palette_from_the_effective_color() {
        let mut song = Song::default();
        song.tracks.push(Track::new("a", "A", InstrumentId::Chu)); // coral = TRACK_COLORS[0]
        cycle_track_color(&mut song, "a");
        assert_eq!(song.tracks[0].color.as_deref(), Some(TRACK_COLORS[1]));
        set_track_color(&mut song, "a", Some(TRACK_COLORS[11]));
        cycle_track_color(&mut song, "a");
        assert_eq!(song.tracks[0].color.as_deref(), Some(TRACK_COLORS[0]), "dá a volta");
        set_track_color(&mut song, "a", Some("#000000"));
        cycle_track_color(&mut song, "a");
        assert_eq!(song.tracks[0].color.as_deref(), Some(TRACK_COLORS[0]), "fora da paleta → primeira");
        cycle_track_color(&mut song, "nope");
    }
}
