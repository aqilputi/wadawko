//! Round-trip JSON: `Song` lida e regravada tem que ser equivalente ao original
//! (mesmas chaves e valores; a ordem das chaves não importa). Fixtures: os
//! projetos do backup do localStorage (dicionário chave → string JSON) + demo.

use serde_json::{Value, json};
use wadawko_core::model::{PROJECT_KEY_PREFIX, Song, SongError, WORKSPACE_KEY, Workspace, project_key};
use wadawko_core::{InstrumentId, demo};

const BACKUP: &str = include_str!("fixtures/backup-localstorage-2026-10-02.json");

fn backup() -> serde_json::Map<String, Value> {
    serde_json::from_str(BACKUP).expect("backup é um dicionário")
}

/// `Song` → JSON (texto) → `Value`, comparado com o `Value` original
fn assert_round_trip(label: &str, original_json: &str) -> Song {
    let original: Value = serde_json::from_str(original_json).unwrap();
    let song = Song::from_json(original_json).unwrap_or_else(|e| panic!("{label}: {}", e.detail()));
    for (how, text) in [("compacto", song.to_json()), ("indentado", song.to_json_pretty())] {
        let back: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(back, original, "{label}: round-trip {how} não é equivalente");
        // inteiros continuam inteiros no texto (JSON.stringify não escreve "100.0")
        assert!(!text.contains(".0,") && !text.contains(".0}") && !text.contains(".0\n"), "{label}: número com .0 no JSON {how}");
    }
    assert_eq!(song.to_value(), original, "{label}: to_value");
    // e a segunda leitura dá a mesma música
    assert_eq!(Song::from_json(&song.to_json()).unwrap(), song, "{label}: releitura");
    song
}

#[test]
fn projetos_do_backup_fazem_round_trip() {
    let backup = backup();
    let projects: Vec<(&String, &Value)> = backup.iter().filter(|(k, _)| k.starts_with(PROJECT_KEY_PREFIX)).collect();
    assert_eq!(projects.len(), 2, "o backup tem 2 projetos");
    let mut titles = Vec::new();
    for (key, raw) in projects {
        let song = assert_round_trip(key, raw.as_str().expect("valor do backup é string JSON"));
        assert_eq!(song.version, 1);
        titles.push((song.title.clone(), song.measures.len(), song.tracks.len()));
    }
    titles.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(titles, vec![("Sample music".to_string(), 131, 4), ("yagyo".to_string(), 18, 4)]);
}

#[test]
fn conteudo_dos_projetos_do_backup() {
    let backup = backup();
    let big = Song::from_json(backup[&project_key("p-muog58u1-1")].as_str().unwrap()).unwrap();
    assert_eq!(big.tempo, 180.0);
    assert_eq!(big.swing, Some(0.0));
    assert_eq!(big.sections().len(), 18);
    assert_eq!(big.sections()[0].repeat, Some(2));
    assert_eq!(big.sections()[2].repeat, None);
    assert_eq!(big.instruments().len(), 4);
    assert_eq!(big.instruments()[0].builtin, Some(false));
    assert_eq!(big.instruments()[0].head.as_ref().unwrap().tone.as_ref().unwrap().cutoff, 900.0);
    assert_eq!(big.tracks[0].volume, Some(-20.0));
    assert!(big.measures.iter().any(|m| m.dynamic.is_some()));
    assert!(big.measures.iter().any(|m| m.hairpin.is_some()));
    assert!(big.measures.iter().any(|m| m.track_dynamics.is_some()));
    assert!(big.measures.iter().all(|m| m.cells.values().all(|row| row.len() == m.cell_count())));

    let pap = Song::from_json(backup[&project_key("p-muplamne-1")].as_str().unwrap()).unwrap();
    assert_eq!((pap.title.as_str(), pap.tempo, pap.swing), ("yagyo", 123.0, Some(1.0)));
    assert_eq!(pap.instruments()[0].base, InstrumentId::Nagado);
    assert_eq!(pap.instruments()[0].builtin, None);
    assert_eq!(pap.slug(), "yagyo");
    assert_eq!(pap.export_file_name(), "yagyo.taiko.json");
}

#[test]
fn demo_faz_round_trip() {
    // (a conferência da demo contra o TypeScript do app web fica no golden.rs: caso base-demo)
    let demo = demo::demo_song();
    assert_round_trip("demo", &demo.to_json());
    assert_eq!(demo.title, "Renshū");
    assert_eq!(demo.slug(), "renshū");
}

#[test]
fn workspace_do_backup_faz_round_trip() {
    let backup = backup();
    let raw = backup[WORKSPACE_KEY].as_str().unwrap();
    let ws = Workspace::from_json(raw).unwrap();
    assert_eq!(ws.version, 2);
    assert_eq!(ws.projects.len(), 2);
    assert_eq!(ws.projects[1].title, "yagyo");
    assert_eq!(ws.projects[0].updated_at, 1790862502440);
    assert!(ws.current_id.is_some());
    for p in &ws.projects {
        assert!(backup.contains_key(&project_key(&p.id)));
    }
    assert_eq!(serde_json::from_str::<Value>(&ws.to_json()).unwrap(), serde_json::from_str::<Value>(raw).unwrap());
    assert!(Workspace::from_json(r#"{"version":1,"projects":[],"currentId":null}"#).is_none());
    assert!(Workspace::from_json(r#"{"version":2}"#).is_none());
    assert!(Workspace::from_json("nope").is_none());
}

#[test]
fn chaves_desconhecidas_e_todos_os_opcionais_fazem_round_trip() {
    let full = json!({
        "version": 1,
        "title": "Tudo",
        "tempo": 92.5,
        "swing": 0.35,
        "futuro": {"a": [1, 2.5, null, "x"], "b": true},
        "choreo": {"moves": [], "lanes": {}},
        "mainTrackIds": ["a"],
        "mainTrackId": "b",
        "tracks": [
            {"id": "a", "name": "A", "instrument": "kane", "instrumentId": "inst-1", "muted": true, "solo": false,
             "volume": -7.5, "pan": -1, "personIds": ["p1", "p2"], "kyai": {"pitch": 2, "vary": 0.25, "gain": -3}, "stand": "naname"},
            {"id": "b", "name": "B", "instrument": "onagado", "muted": false, "solo": true, "personIds": []}
        ],
        "measures": [
            {"beats": 3, "subdivision": 4,
             "cells": {"a": ["don", null, "ka!", null, null, null, null, null, null, null, null, "dokodoko"], "b": []},
             "cellOwners": {"a": ["p1", null, "p2", null, null, null, null, null, null, null, null, null]},
             "dynamic": "pp", "hairpin": "dim",
             "trackDynamics": {"a": {"dynamic": "ff"}, "b": {"hairpin": "cresc"}, "c": {}},
             "kyai": {"a": ["so", null, "re!", null, null, null, null, null, null, null, null, null]},
             "anotacao": "livre"},
            {"beats": 4, "subdivision": 2, "cells": {}}
        ],
        "sections": [{"id": "s1", "name": "Intro", "start": 0, "repeat": 3, "cor": "#fff"}, {"id": "s2", "name": "B", "start": 1}],
        "people": [
            {"id": "p1", "name": "Ana", "color": "#e8663d", "muted": false, "solo": true, "volume": 1.5, "avatar": {"body": "slim", "height": 1}},
            {"id": "p2", "name": "Bia"}
        ],
        "instruments": [
            {"id": "inst-1", "name": "Custom", "base": "hira", "builtin": false,
             "head": {"pitch": -2, "gain": 0.5, "tone": {"type": "bandpass", "cutoff": 320, "q": 1.2}, "overtones": 0.4, "decay": 0.5},
             "rim": {}, "nota": 1},
            {"id": "inst-2", "name": "Vazio", "base": "shime"}
        ]
    });
    let song = assert_round_trip("completo", &full.to_string());
    assert_eq!(song.extra["futuro"]["a"][1], 2.5);
    assert_eq!(song.tracks[0].extra["stand"], "naname");
    assert_eq!(song.measures[0].extra["anotacao"], "livre");
    assert_eq!(song.sections()[0].extra["cor"], "#fff");
    assert_eq!(song.people()[0].extra["avatar"]["body"], "slim");
    assert_eq!(song.instruments()[0].extra["nota"], 1);
    assert_eq!(song.tracks[1].person_ids, Some(vec![]));
    assert_eq!(song.measures[0].cell("a", 2), Some("ka!"));
    assert_eq!(song.measures[0].kyai_at("a", 2), Some("re!"));
    assert_eq!(song.measures[0].raw_owner("a", 2), Some("p2"));
}

#[test]
fn leitura_valida_a_versao_como_o_read_song() {
    let ok = r#"{"version":1,"title":"x","tempo":100,"tracks":[],"measures":[]}"#;
    assert!(Song::from_json(ok).is_ok());
    assert!(Song::from_json(r#"{"version":1.0,"title":"x","tempo":100,"tracks":[],"measures":[]}"#).is_ok());

    for bad in [
        r#"{"version":2,"title":"x","tempo":100,"tracks":[],"measures":[]}"#,
        r#"{"version":"1","title":"x","tempo":100,"tracks":[],"measures":[]}"#,
        r#"{"title":"x","tempo":100,"tracks":[],"measures":[]}"#,
        r#"{"version":1,"title":"x","tempo":100,"measures":[]}"#,
        r#"{"version":1,"title":"x","tempo":100,"tracks":[]}"#,
        r#"{"version":1,"title":"x","tempo":100,"tracks":{},"measures":[]}"#,
        "null",
        "[]",
        r#"{"version":2,"projects":[],"currentId":null}"#,
    ] {
        let err = Song::from_json(bad).unwrap_err();
        assert!(matches!(err, SongError::NotASong(_)), "{bad} → {err:?}");
        assert_eq!(err.to_string(), "Arquivo não parece ser uma música do taiko-fu (version 1).");
    }
    let err = Song::from_json("{isso não é json").unwrap_err();
    assert!(matches!(err, SongError::InvalidJson(_)));
    assert_eq!(err.to_string(), "JSON inválido.");
    // serde direto também recusa versão errada
    assert!(serde_json::from_str::<Song>(r#"{"version":3,"title":"x","tempo":100,"tracks":[],"measures":[]}"#).is_err());
}

#[test]
fn numeros_saem_como_no_json_stringify() {
    let mut song = demo::demo_song();
    song.tempo = 120.0;
    song.swing = Some(1.0);
    song.tracks[0].volume = Some(-6.0);
    song.tracks[0].pan = Some(0.25);
    let text = song.to_json();
    assert!(text.contains(r#""tempo":120,"#), "{text}");
    assert!(text.contains(r#""swing":1,"#));
    assert!(text.contains(r#""volume":-6,"#));
    assert!(text.contains(r#""pan":0.25"#));
    song.tempo = 92.5;
    assert!(song.to_json().contains(r#""tempo":92.5,"#));
    // opcionais ausentes não aparecem
    let demo_text = demo::demo_song().to_json();
    for key in ["swing", "sections", "people", "instruments", "mainTrackIds", "mainTrackId", "instrumentId", "volume", "pan", "personIds", "kyai", "cellOwners", "dynamic", "hairpin", "trackDynamics"]
    {
        assert!(!demo_text.contains(&format!("\"{key}\"")), "{key} não devia aparecer na demo");
    }
}
