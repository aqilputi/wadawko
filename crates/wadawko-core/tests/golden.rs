//! Confere o porte contra o TypeScript ORIGINAL: `fixtures/golden.json` é gerado
//! por `tests/golden/gen.mjs` rodando `src/lib/*.ts` no node (frases, kiai,
//! constantes e ~90 roteiros de operações do store, incluindo 60 aleatórios).
//! Aqui as mesmas entradas passam pelo Rust e o resultado tem que bater.

use std::sync::LazyLock;

use serde_json::{Value, json};
use wadawko_core::model::{InstrumentTone, Song, Zone};
use wadawko_core::ops::kit::ZonePatch;
use wadawko_core::ops::{CellPos, cells, kit, measures, people, sections, tracks};
use wadawko_core::phrase::{self, ResolvedHit};
use wadawko_core::timeline::{self, Region};
use wadawko_core::vocab::{self, Hand};
use wadawko_core::{demo, ids, kyai};

static GOLDEN: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(include_str!("fixtures/golden.json")).expect("golden.json"));
static BACKUP: LazyLock<Value> =
    LazyLock::new(|| serde_json::from_str(include_str!("fixtures/backup-localstorage-2026-10-02.json")).expect("backup"));

fn base_song(name: &str) -> Song {
    let project = |id: &str| Song::from_json(BACKUP[format!("wadawko:project:{id}")].as_str().unwrap()).unwrap();
    match name {
        "demo" => demo::demo_song(),
        "big" => project("p-muog58u1-1"),
        "pap" => project("p-muplamne-1"),
        other => panic!("base desconhecida: {other}"),
    }
}

/// igualdade de JSON com tolerância numérica (sin/cos podem diferir no último bit)
fn approx_eq(a: &Value, b: &Value, path: &str) -> Result<(), String> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            if (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1.0) { Ok(()) } else { Err(format!("{path}: {x} != {y}")) }
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Err(format!("{path}: tamanhos {} != {}\n  rust: {a}\n  ts:   {b}", x.len(), y.len()));
            }
            x.iter().zip(y).enumerate().try_for_each(|(i, (p, q))| approx_eq(p, q, &format!("{path}[{i}]")))
        }
        (Value::Object(x), Value::Object(y)) => {
            if x.len() != y.len() || x.keys().any(|k| !y.contains_key(k)) {
                return Err(format!("{path}: chaves {:?} != {:?}", x.keys().collect::<Vec<_>>(), y.keys().collect::<Vec<_>>()));
            }
            x.iter().try_for_each(|(k, p)| approx_eq(p, &y[k], &format!("{path}.{k}")))
        }
        _ if a == b => Ok(()),
        _ => Err(format!("{path}:\n  rust: {a}\n  ts:   {b}")),
    }
}

fn fnv_utf16(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for c in s.encode_utf16() {
        h = (h ^ c as u32).wrapping_mul(16777619);
    }
    h
}

/// hash do JSON canônico (chaves ordenadas) — o mesmo `songHash` do gen.mjs
fn song_hash(song: &Song) -> u32 {
    fnv_utf16(&canon(&song.to_value()))
}

/// JSON com chaves ordenadas, independente da feature `preserve_order` do serde_json
fn canon(v: &Value) -> String {
    match v {
        Value::Array(a) => format!("[{}]", a.iter().map(canon).collect::<Vec<_>>().join(",")),
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            format!("{{{}}}", keys.iter().map(|k| format!("{}:{}", Value::String((*k).clone()), canon(&o[*k]))).collect::<Vec<_>>().join(","))
        }
        other => other.to_string(),
    }
}

// ───────────────────────────── roteiros de operações ─────────────────────────────

#[derive(Default)]
struct Ui {
    selected: Vec<String>,
    pen: Option<String>,
}

fn pick<T>(arr: &[T], i: usize, id: impl Fn(&T) -> &str) -> String {
    if arr.is_empty() { "nope".into() } else { id(&arr[i % arr.len()]).to_string() }
}

/// resolve `{"$t": i}` / `$s` / `$p` / `$i` pro id atual; `null` → `None`
fn res(song: &Song, a: &Value) -> Option<String> {
    let idx = |k: &str| a.get(k).and_then(Value::as_u64).map(|i| i as usize);
    if let Some(i) = idx("$t") {
        Some(pick(&song.tracks, i, |t| &t.id))
    } else if let Some(i) = idx("$s") {
        Some(pick(song.sections(), i, |s| &s.id))
    } else if let Some(i) = idx("$p") {
        Some(pick(song.people(), i, |p| &p.id))
    } else if let Some(i) = idx("$i") {
        Some(pick(song.instruments(), i, |d| &d.id))
    } else {
        a.as_str().map(str::to_string)
    }
}

fn fill_ret(r: cells::FillResult) -> Value {
    json!({
        "written": r.written.iter().map(|p| json!([p.mi, p.ci])).collect::<Vec<_>>(),
        "next": r.next.map(|p| json!([p.mi, p.ci])),
    })
}

fn exec(song: &mut Song, ui: &mut Ui, op: &Value) -> Value {
    let name = op[0].as_str().unwrap();
    let raw = &op.as_array().unwrap()[1..];
    // resolve tudo antes de mexer na música (como o `raw.map(res)` do gen.mjs)
    let resolved: Vec<Option<String>> = raw.iter().map(|a| res(song, a)).collect();
    let selected_arg: Vec<String> = if name == "setSelected" { raw[0].as_array().unwrap().iter().map(|x| res(song, x).unwrap()).collect() } else { Vec::new() };
    let owner_cells: Vec<(String, CellPos)> = if name == "setCellsOwner" {
        raw[0]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| (res(song, &c[0]).unwrap(), CellPos::new(c[1].as_u64().unwrap() as usize, c[2].as_u64().unwrap() as usize)))
            .collect()
    } else {
        Vec::new()
    };
    let s = |i: usize| resolved[i].clone();
    let sv = |i: usize| s(i).unwrap_or_else(|| panic!("{name}: arg {i} devia ser texto: {op}"));
    let n = |i: usize| raw[i].as_u64().unwrap_or_else(|| panic!("{name}: arg {i} devia ser número: {op}")) as usize;
    let list = |i: usize| raw[i].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect::<Vec<_>>();
    let pen = ui.pen.clone();
    let pen = pen.as_deref();
    match name {
        "setSelected" => ui.selected = selected_arg,
        "setPen" => ui.pen = s(0),
        "setPersonFlags" => {
            let (id, muted, solo) = (sv(0), raw[1].as_bool().unwrap(), raw[2].as_bool().unwrap());
            if let Some(p) = song.person_mut(&id) {
                p.muted = Some(muted);
                p.solo = Some(solo);
            }
        }
        "setLegacyMain" => {
            song.main_track_ids = None;
            song.main_track_id = s(0);
        }
        "setSwing" => song.swing = raw[0].as_f64(),
        "appendSection" => {
            sections::append_section(song, s(0).as_deref());
        }
        "cloneSection" => return json!(sections::clone_section(song, &sv(0))),
        "moveSection" => sections::move_section(song, &sv(0), n(1)),
        "renameSection" => sections::rename_section(song, &sv(0), &sv(1)),
        "setSectionRepeat" => sections::set_section_repeat(song, &sv(0), n(1) as u32),
        "removeSection" => {
            let id = sv(0);
            if song.sections.is_some() {
                ui.selected.retain(|x| *x != id); // efeito de UI do store
            }
            sections::remove_section(song, &id);
        }
        "setMeasureDynamic" => {
            let d = raw[1].as_str().map(|d| wadawko_core::Dynamic::parse(d).unwrap());
            measures::set_measure_dynamic(song, n(0), d, s(2).as_deref());
        }
        "toggleMeasureHairpin" => {
            let hp = if raw[1] == "cresc" { wadawko_core::Hairpin::Cresc } else { wadawko_core::Hairpin::Dim };
            measures::toggle_measure_hairpin(song, n(0), hp, s(2).as_deref());
        }
        "addMeasure" | "removeMeasure" | "ensureCols" => {
            // sem seção explícita, o store usa a última seção selecionada
            let explicit = if name == "ensureCols" { None } else { s(0) };
            let target = explicit.or_else(|| sections::target_section(song, &ui.selected).map(|r| r.id));
            match name {
                "addMeasure" => measures::add_measure(song, target.as_deref()),
                "removeMeasure" => measures::remove_measure(song, target.as_deref()),
                _ => measures::ensure_cols(song, n(0), target.as_deref()),
            }
        }
        "setSubdivision" => measures::set_subdivision(song, n(0)),
        "setCell" => cells::set_cell(song, &sv(0), n(1), n(2), s(3).as_deref(), pen),
        "setCellCol" => cells::set_cell_col(song, &sv(0), n(1), s(2).as_deref(), pen),
        "setKyai" => cells::set_kyai(song, &sv(0), n(1), n(2), s(3).as_deref()),
        "fillSequential" => return fill_ret(cells::fill_sequential(song, &sv(0), n(1), n(2), &list(3), pen)),
        "fillPhraseBeats" => return fill_ret(cells::fill_phrase_beats(song, &sv(0), n(1), n(2), &list(3), pen)),
        "fillKyai" => return fill_ret(cells::fill_kyai(song, &sv(0), n(1), n(2), &list(3))),
        "addTrack" => {
            tracks::add_track(song, &sv(0));
        }
        "removeTrack" => tracks::remove_track(song, &sv(0)),
        "toggleMainTrack" => tracks::toggle_main_track(song, &sv(0)),
        "renameTrack" => tracks::rename_track(song, &sv(0), &sv(1)),
        "setTrackInstrument" => tracks::set_track_instrument(song, &sv(0), &sv(1)),
        "moveTrack" => tracks::move_track(song, &sv(0), raw[1].as_i64().unwrap() as i32),
        "addPerson" => {
            let id = people::add_person(song, s(0).as_deref());
            if ui.pen.is_none() {
                ui.pen = Some(id.clone()); // efeito de UI do store
            }
            return json!(id);
        }
        "renamePerson" => people::rename_person(song, &sv(0), &sv(1)),
        "setPersonColor" => people::set_person_color(song, &sv(0), &sv(1)),
        "deletePerson" => {
            let id = sv(0);
            if song.people.is_some() {
                people::delete_person(song, &id);
                if ui.pen.as_deref() == Some(&id) {
                    ui.pen = song.people().first().map(|p| p.id.clone()); // efeito de UI do store
                }
            }
        }
        "toggleTrackPerson" => people::toggle_track_person(song, &sv(0), &sv(1)),
        "assignCellOwner" => people::assign_cell_owner(song, &sv(0), n(1), n(2), s(3).as_deref()),
        "setCellsOwner" => {
            people::set_cells_owner(song, &owner_cells, s(1).as_deref());
        }
        "addInstrument" => {
            let base = wadawko_core::InstrumentId::parse(&sv(0)).unwrap();
            return json!(kit::add_instrument(song, base, s(1).as_deref()));
        }
        "updateInstrumentZone" => {
            let zone = if raw[1] == "head" { Zone::Head } else { Zone::Rim };
            let p = &raw[2];
            let patch = ZonePatch {
                pitch: p.get("pitch").and_then(Value::as_f64),
                gain: p.get("gain").and_then(Value::as_f64),
                tone: p.get("tone").map(|t| (!t.is_null()).then(|| serde_json::from_value::<InstrumentTone>(t.clone()).unwrap())),
                overtones: p.get("overtones").and_then(Value::as_f64),
                decay: p.get("decay").and_then(Value::as_f64),
            };
            kit::update_instrument_zone(song, &sv(0), zone, patch);
        }
        "renameInstrument" => kit::rename_instrument(song, &sv(0), &sv(1)),
        "duplicateInstrument" => return json!(kit::duplicate_instrument(song, &sv(0))),
        "deleteInstrument" => kit::delete_instrument(song, &sv(0)),
        other => panic!("operação desconhecida no golden: {other}"),
    }
    Value::Null
}

fn queries(song: &Song, ui: &Ui) -> Value {
    let order = timeline::play_order(song, None);
    let region = sections::selected_region(song, &ui.selected);
    let order_r = timeline::play_order(song, region);
    let sample = |ord: &[usize], tid: Option<&str>| {
        let curve = timeline::dynamics_curve(song, ord, tid);
        (0..ord.len()).flat_map(|u| [curve.at(u, 0.0), curve.at(u, 0.5)]).collect::<Vec<f64>>()
    };
    let swing = song.swing();
    let evs = kyai::kyai_events(song, &order_r, |f| timeline::swing_warp(f, swing));
    json!({
        "ranges": sections::section_ranges(song).iter().map(|r| json!([r.id, r.start, r.end_excl])).collect::<Vec<_>>(),
        "order": order,
        "region": region.map(|Region { start_bar, end_bar }| json!([start_bar, end_bar])),
        "orderR": order_r,
        "dyn": sample(&order, None),
        "dynTracks": song.tracks.iter().map(|t| sample(&order_r, Some(&t.id))).collect::<Vec<_>>(),
        "mainTracks": timeline::main_tracks(song),
        "mainMul": song.tracks.iter().map(|t| timeline::main_mul(song, &t.id)).collect::<Vec<_>>(),
        "rails": people::rails_for_tracks(song).iter().map(|r| json!([r.person.id, r.rows])).collect::<Vec<_>>(),
        "audible": song.people().iter().map(|p| people::person_audible(song, Some(&p.id))).collect::<Vec<_>>(),
        "owners": song.tracks.iter().map(|t| {
            song.measures.iter().enumerate()
                .flat_map(|(mi, m)| (0..m.cell_count()).map(move |ci| (mi, ci)))
                .map(|(mi, ci)| people::cell_owner(song, &t.id, mi, ci))
                .collect::<Vec<_>>()
        }).collect::<Vec<_>>(),
        "inst": song.tracks.iter().map(|t| json!([kit::resolve_instrument(song, t).id, kit::instrument_label(song, t)])).collect::<Vec<_>>(),
        "totalCols": measures::total_cols(song),
        "hasKyai": song.tracks.iter().map(|t| cells::track_has_kyai(song, &t.id)).collect::<Vec<_>>(),
        "hasDyn": song.tracks.iter().map(|t| measures::track_has_dynamics(song, &t.id)).collect::<Vec<_>>(),
        "kyai": evs.iter().map(|e| json!([
            e.track_id, e.u, e.mi, e.ci, e.beat, e.frac, e.beats, e.syl, e.idx, e.n, e.shout,
            e.gap_beats.is_finite().then_some(e.gap_beats),
            kyai::kyai_seed(&song.tracks[e.track_index], &song.measures[e.mi], e.ci),
        ])).collect::<Vec<_>>(),
    })
}

#[test]
fn operacoes_do_store_batem_com_o_ts() {
    // os contadores de id do store são globais: os casos rodam em ordem, numa thread só
    ids::set_deterministic(Some(GOLDEN["now"].as_u64().unwrap()));
    let cases = GOLDEN["cases"].as_array().unwrap();
    assert!(cases.len() >= 90);
    let mut total_ops = 0;
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let mut song = base_song(case["base"].as_str().unwrap());
        let mut ui = Ui::default();
        for (k, op) in case["ops"].as_array().unwrap().iter().enumerate() {
            let ret = exec(&mut song, &mut ui, op);
            let expected_ret = &case["rets"][k];
            if op[0] != "appendSection" {
                assert_eq!(&ret, expected_ret, "caso {name}, op #{k} {op}: retorno diferente");
            }
            assert_eq!(song_hash(&song) as u64, case["hashes"][k].as_u64().unwrap(), "caso {name}, op #{k} {op}: música diferente do TS\n{}", song.to_json());
            total_ops += 1;
        }
        if !case["song"].is_null() {
            assert_eq!(song.to_value(), case["song"], "caso {name}: música final");
        }
        approx_eq(&queries(&song, &ui), &case["queries"], "").unwrap_or_else(|e| panic!("caso {name}: consultas diferem em {e}"));
        // toda música produzida pelas operações faz round-trip
        assert_eq!(Song::from_json(&song.to_json()).unwrap(), song, "caso {name}: round-trip");
    }
    ids::set_deterministic(None);
    assert!(total_ops > 4000, "poucas operações conferidas: {total_ops}");
}

// ───────────────────────────── constantes ─────────────────────────────

#[test]
fn vocabulario_e_constantes_batem_com_o_ts() {
    let g = &*GOLDEN;
    let ours: Vec<Value> = vocab::VOCABULARY
        .iter()
        .map(|s| {
            json!([s.name, s.hits.iter().map(|h| json!([h.offset, zone(h.zone), h.vel, hand(h.hand)])).collect::<Vec<_>>(), s.hint])
        })
        .collect();
    approx_eq(&json!(ours), &g["vocabulary"], "vocabulary").unwrap();

    let palette: Vec<Value> = vocab::PALETTE_GROUPS.iter().map(|p| json!({"label": p.label, "syls": p.syls})).collect();
    assert_eq!(json!(palette), g["palette"]);

    let phrases: Vec<Value> = vocab::PHRASES
        .iter()
        .map(|p| {
            let mut v = json!({"name": p.name, "phrase": p.phrase, "hint": p.hint});
            if p.per_cell {
                v["perCell"] = json!(true);
            }
            v
        })
        .collect();
    assert_eq!(json!(phrases), g["phrases"]);

    assert_eq!(serde_json::to_value(&*wadawko_core::model::BUILTIN_INSTRUMENTS).unwrap(), g["builtins"]);
    for i in wadawko_core::InstrumentId::ALL {
        assert_eq!(g["instrumentLabels"][i.as_str()], i.label());
    }
    assert_eq!(g["instrumentLabels"].as_object().unwrap().len(), wadawko_core::InstrumentId::ALL.len());
    for d in wadawko_core::Dynamic::ALL {
        assert_eq!(g["dynamicLevels"][d.as_str()].as_f64().unwrap(), d.level());
    }
    assert_eq!(g["dynamicLevels"].as_object().unwrap().len(), 6);
    assert_eq!(json!(wadawko_core::model::PERSON_COLORS), g["personColors"]);
    assert_eq!(json!(kyai::KYAI_PRESETS), g["kyaiPresets"]);
    assert_eq!(demo::demo_song().to_value(), g["demo"]);
}

fn zone(z: Zone) -> &'static str {
    match z {
        Zone::Head => "head",
        Zone::Rim => "rim",
    }
}

fn hand(h: Hand) -> &'static str {
    match h {
        Hand::R => "R",
        Hand::L => "L",
    }
}

// ───────────────────────────── frases ─────────────────────────────

fn hits_json(hits: &[ResolvedHit]) -> Value {
    json!(hits.iter().map(|h| json!([h.offset, zone(h.zone), h.vel, hand(h.hand), h.accent, h.sfz])).collect::<Vec<_>>())
}

#[test]
fn split_word_e_resolve_cell_batem_com_o_ts() {
    let words = GOLDEN["words"].as_array().unwrap();
    assert!(words.len() > 1500);
    let mut valid = 0;
    for entry in words {
        let w = entry[0].as_str().unwrap();
        let split = phrase::split_word(w).map(|v| json!(v.iter().map(|x| json!([x.syl, x.stretch, x.accent])).collect::<Vec<_>>()));
        assert_eq!(json!(split), entry[1], "split_word({w:?})");
        let resolved = phrase::resolve_cell(w).map(|r| {
            json!({
                "sfz": r.sfz,
                "hits": hits_json(&r.hits),
                "parts": r.parts.iter().map(|p| json!([p.syl, hits_json(&p.hits)])).collect::<Vec<_>>(),
            })
        });
        valid += resolved.is_some() as usize;
        approx_eq(&json!(resolved), &entry[2], &format!("resolve_cell({w:?})")).unwrap();
        assert_eq!(phrase::resolve_cell_uncached(w).as_ref(), phrase::resolve_cell(w).as_deref(), "cache de {w:?}");
        assert_eq!(json!(phrase::is_sfz(w)), entry[3], "is_sfz({w:?})");
        assert_eq!(json!(phrase::strip_sfz(w)), entry[4], "strip_sfz({w:?})");
        assert_eq!(json!(phrase::toggle_sfz(Some(w))), entry[5], "toggle_sfz({w:?})");
    }
    assert!(valid > 1000, "poucas palavras válidas conferidas: {valid}");
    assert_eq!(phrase::toggle_sfz(None), None);

    for entry in GOLDEN["parseWords"].as_array().unwrap() {
        let text = entry[0].as_str().unwrap();
        assert_eq!(json!(phrase::parse_words(text)), entry[1], "parse_words({text:?})");
    }
}

// ───────────────────────────── kiai ─────────────────────────────

#[test]
fn kiai_bate_com_o_ts() {
    let g = &*GOLDEN;
    let texts = g["splitKyai"].as_array().unwrap();
    let mut valid = 0;
    for entry in texts {
        let text = entry[0].as_str().unwrap();
        let got = kyai::split_kyai(text);
        valid += got.is_some() as usize;
        assert_eq!(json!(got), entry[1], "split_kyai({text:?})");
    }
    assert!(texts.len() > 2000 && valid > 300, "{} textos, {valid} válidos", texts.len());

    let track_with = |voice: &Value| {
        let mut t = wadawko_core::Track::new("t", "t", wadawko_core::InstrumentId::Chu);
        t.kyai = (!voice.is_null()).then(|| serde_json::from_value(voice.clone()).unwrap());
        t
    };
    for entry in g["profiles"].as_array().unwrap() {
        let seed = entry[0].as_str().unwrap();
        let p = kyai::kyai_profile(seed, &track_with(&entry[1]));
        approx_eq(&json!([p.pitch, p.speed, p.vary, p.grit, p.breath, p.range]), &entry[2], &format!("kyai_profile({seed:?}, {})", entry[1])).unwrap();
    }
    for entry in g["contours"].as_array().unwrap() {
        let (idx, n, shout) = (entry[0].as_u64().unwrap() as usize, entry[1].as_u64().unwrap() as usize, entry[2].as_bool().unwrap());
        let (a, b) = kyai::contour(idx, n, shout);
        approx_eq(&json!([a, b]), &entry[3], &format!("contour({idx}, {n}, {shout})")).unwrap();
    }
    for entry in g["notes"].as_array().unwrap() {
        let (idx, n, shout) = (entry[0].as_u64().unwrap() as usize, entry[1].as_u64().unwrap() as usize, entry[2].as_bool().unwrap());
        let gap = entry[3].as_f64().unwrap_or(f64::INFINITY);
        let prof = kyai::kyai_profile(entry[4].as_str().unwrap(), &track_with(&entry[5]));
        let mut rs = entry[6].as_array().unwrap().iter().map(|x| x.as_f64().unwrap());
        let note = kyai::kyai_note("ya!", idx, n, shout, gap, &prof, || rs.next().unwrap());
        approx_eq(&json!([note.st0, note.st1, note.dur, note.gain]), &entry[7], &format!("kyai_note {entry}")).unwrap();
    }
    for entry in g["swing"].as_array().unwrap() {
        let (f, s) = (entry[0].as_f64().unwrap(), entry[1].as_f64().unwrap());
        approx_eq(&json!(timeline::swing_warp(f, s)), &entry[2], &format!("swing_warp({f}, {s})")).unwrap();
    }
}
