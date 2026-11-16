mod common;

use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{TempDir, backup_fixture, wal_path};
use serde_json::Value;
use wadawko_core::Song;
use wadawko_core::demo::demo_song;
use wadawko_core::model::{THEME_KEY, WORKSPACE_KEY, Workspace, project_key};
use wadawko_store::db::data_dir_from;
use wadawko_store::*;

fn song_titled(title: &str) -> Song {
    Song { title: title.into(), ..demo_song() }
}

// ───────────────────────────── Db ─────────────────────────────

#[test]
fn db_get_set_delete_list() {
    let tmp = TempDir::new("db");
    let path = tmp.join("sub/dir/wadawko.db");
    let db = Db::open(&path).unwrap();
    assert!(path.is_file(), "cria a pasta e o arquivo");
    assert_eq!(db.get("nada").unwrap(), None);

    db.set("wadawko:theme", "dark").unwrap();
    db.set("wadawko:project:a", "{\"a\":1}").unwrap();
    db.set("wadawko:project:b", "ção 太鼓").unwrap();
    db.set("outra", "x").unwrap();
    assert_eq!(db.get("wadawko:project:b").unwrap().as_deref(), Some("ção 太鼓"));

    db.set("wadawko:theme", "light").unwrap(); // substitui
    assert_eq!(db.get("wadawko:theme").unwrap().as_deref(), Some("light"));

    assert_eq!(db.keys("wadawko:project:").unwrap(), ["wadawko:project:a", "wadawko:project:b"]);
    assert_eq!(db.keys("").unwrap().len(), 4);
    let entries = db.list("wadawko:project:").unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries[0].updated_at > 1_600_000_000_000, "updated_at em ms");
    assert_eq!(db.entry("outra").unwrap().unwrap().value, "x");
    // % e _ no prefixo não são curingas
    assert!(db.keys("wadawko:%").unwrap().is_empty());

    assert!(db.delete("outra").unwrap());
    assert!(!db.delete("outra").unwrap());
    let all = db.all().unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all["wadawko:project:a"], "{\"a\":1}");
}

#[test]
fn db_batch_is_atomic_and_persists() {
    let tmp = TempDir::new("batch");
    let path = tmp.join("w.db");
    {
        let db = Db::open(&path).unwrap();
        db.set("old", "1").unwrap();
        db.write_batch(&[KvWrite::Set("a".into(), "1".into()), KvWrite::Set("b".into(), "2".into()), KvWrite::Delete("old".into())]).unwrap();
    }
    let db = Db::open(&path).unwrap();
    assert_eq!(db.keys("").unwrap(), ["a", "b"]);
}

#[test]
fn db_uses_wal_and_checkpoints_on_close() {
    let tmp = TempDir::new("wal");
    let path = tmp.join("w.db");
    let db = Db::open(&path).unwrap();
    let clone = db.clone();
    db.set("k", &"x".repeat(50_000)).unwrap();
    let wal = wal_path(&path);
    assert!(wal.is_file() && std::fs::metadata(&wal).unwrap().len() > 0, "modo WAL: a escrita foi para o -wal");

    drop(db);
    assert!(std::fs::metadata(&wal).is_ok_and(|m| m.len() > 0), "um clone vivo mantém o banco aberto");
    drop(clone);
    assert!(std::fs::metadata(&wal).map_or(true, |m| m.len() == 0), "fechou: WAL incorporado e truncado");
    // o dado está no arquivo principal
    assert!(std::fs::metadata(&path).unwrap().len() > 50_000);
    assert_eq!(Db::open_read_only(&path).unwrap().get("k").unwrap().unwrap().len(), 50_000);
}

#[test]
fn db_read_only() {
    let tmp = TempDir::new("ro");
    let path = tmp.join("w.db");
    assert!(Db::open_read_only(&path).is_err(), "não cria banco");
    assert!(!path.exists());

    {
        let db = Db::open(&path).unwrap();
        db.set("k", "v").unwrap();
    }
    let before = std::fs::read(&path).unwrap();
    let ro = Db::open_with(&path, true).unwrap();
    assert!(ro.is_read_only());
    assert_eq!(ro.get("k").unwrap().as_deref(), Some("v"));
    assert!(ro.set("k", "outro").is_err());
    assert!(ro.delete("k").is_err());
    assert!(ro.write_batch(&[KvWrite::Delete("k".into())]).is_err());
    ro.checkpoint().unwrap(); // sem efeito
    drop(ro);
    assert_eq!(std::fs::read(&path).unwrap(), before, "somente leitura não muda o arquivo");
}

#[test]
fn db_read_only_reads_uncheckpointed_wal() {
    let tmp = TempDir::new("rowal");
    let path = tmp.join("w.db");
    let writer = Db::open(&path).unwrap();
    writer.set("só-no-wal", "sim").unwrap();
    let ro = Db::open_read_only(&path).unwrap();
    assert_eq!(ro.get("só-no-wal").unwrap().as_deref(), Some("sim"));
}

#[test]
fn db_read_only_without_table_is_empty() {
    let tmp = TempDir::new("notable");
    let path = tmp.join("vazio.db");
    rusqlite::Connection::open(&path).unwrap().execute_batch("CREATE TABLE outra (x); INSERT INTO outra VALUES (1);").unwrap();
    let ro = Db::open_read_only(&path).unwrap();
    assert_eq!(ro.get("k").unwrap(), None);
    assert!(ro.list("").unwrap().is_empty());
    assert!(list_projects(&ro).unwrap().is_empty());
}

#[test]
fn xdg_paths() {
    let os = |s: &'static str| Some(OsStr::new(s));
    assert_eq!(data_dir_from(os("/x/data"), os("/home/u")), Some(PathBuf::from("/x/data/wadawko")));
    assert_eq!(data_dir_from(None, os("/home/u")), Some(PathBuf::from("/home/u/.local/share/wadawko")));
    assert_eq!(data_dir_from(os(""), os("/home/u")), Some(PathBuf::from("/home/u/.local/share/wadawko")));
    assert_eq!(data_dir_from(os("relativo"), os("/home/u")), Some(PathBuf::from("/home/u/.local/share/wadawko")));
    assert_eq!(data_dir_from(None, None), None);
    assert_eq!(resolve_db_path(Some("/tmp/x.db".as_ref())).unwrap(), PathBuf::from("/tmp/x.db"));
    assert!(default_db_path().unwrap().ends_with("wadawko/wadawko.db"));
}

// ───────────────────────────── ProjectStore ─────────────────────────────

#[test]
fn bootstrap_creates_demo_and_reopens_untouched() {
    let tmp = TempDir::new("boot");
    let path = tmp.join("w.db");
    let id;
    {
        let store = ProjectStore::open_path(&path).unwrap();
        assert_eq!(store.projects().len(), 1);
        assert_eq!(store.projects()[0].title, "Renshū");
        assert_eq!(**store.song(), demo_song());
        assert_eq!(store.epoch(), 0);
        id = store.current_id().to_string();
        assert!(id.starts_with("p-"));
        // mesmo formato do app web
        let ws: Value = serde_json::from_str(&store.db().get(WORKSPACE_KEY).unwrap().unwrap()).unwrap();
        assert_eq!(ws["version"], 2);
        assert_eq!(ws["currentId"], id.as_str());
        assert_eq!(ws["projects"][0]["title"], "Renshū");
        assert!(ws["projects"][0]["updatedAt"].is_i64());
        assert_eq!(store.db().get(&project_key(&id)).unwrap().unwrap(), demo_song().to_json());
    }
    let db = Db::open(&path).unwrap();
    let stamps: Vec<_> = db.list("").unwrap().into_iter().map(|e| (e.key, e.updated_at)).collect();
    let store = ProjectStore::open(db.clone()).unwrap();
    assert_eq!(store.current_id(), id);
    assert_eq!(store.projects().len(), 1);
    drop(store);
    let after: Vec<_> = db.list("").unwrap().into_iter().map(|e| (e.key, e.updated_at)).collect();
    assert_eq!(after, stamps, "abrir e fechar sem editar não grava nada");
}

#[test]
fn project_operations() {
    let db = Db::open_in_memory().unwrap();
    let mut store = ProjectStore::open(db.clone()).unwrap();
    let first = store.current_id().to_string();

    // nova
    let second = store.new_project().unwrap();
    assert_ne!(first, second);
    assert_eq!(store.current_id(), second);
    assert_eq!(store.projects().len(), 2);
    assert_eq!(store.epoch(), 1);
    assert!(db.get(&project_key(&second)).unwrap().is_some());

    // edita a atual e troca: a edição é gravada antes da troca
    let mut edited = (**store.song()).clone();
    edited.title = "Yatai".into();
    edited.tempo = 144.0;
    store.update_song(Arc::new(edited.clone()));
    assert_eq!(store.current_meta().unwrap().title, "Yatai", "a lista reflete o título na hora");
    assert!(store.has_unsaved_changes());
    assert!(store.switch_project(&first).unwrap());
    assert!(!store.has_unsaved_changes());
    assert_eq!(store.epoch(), 2);
    assert_eq!(store.song().title, "Renshū");
    assert_eq!(read_song(&db, &second).unwrap(), edited);
    assert_eq!(read_workspace(&db).unwrap().unwrap(), *store.workspace());

    // trocar para a mesma / para uma que não existe
    assert!(!store.switch_project(&first).unwrap());
    assert!(store.switch_project("p-nao-existe").is_err());
    assert_eq!(store.current_id(), first);
    assert_eq!(store.epoch(), 2);

    // duplicar (uma que não é a atual)
    let copy = store.duplicate_project(&second).unwrap();
    assert_eq!(store.current_id(), copy);
    assert_eq!(store.song().title, "Yatai cópia");
    assert_eq!(store.song().tempo, 144.0);
    assert_eq!(store.projects().last().unwrap().title, "Yatai cópia");
    assert_eq!(read_song(&db, &copy).unwrap().title, "Yatai cópia");
    assert_eq!(store.epoch(), 3);

    // renomear a atual e outra
    store.rename_project(&copy, "Miyake").unwrap();
    assert_eq!(store.song().title, "Miyake");
    assert_eq!(read_song(&db, &copy).unwrap().title, "Miyake");
    store.rename_project(&first, "").unwrap();
    assert_eq!(store.projects()[0].title, "Sem título");
    assert_eq!(read_song(&db, &first).unwrap().title, "");
    assert_eq!(store.epoch(), 3, "renomear não troca de música");
    assert_eq!(read_workspace(&db).unwrap().unwrap(), *store.workspace());

    // excluir uma que não é a atual
    store.delete_project(&second).unwrap();
    assert_eq!(store.projects().len(), 2);
    assert_eq!(store.current_id(), copy);
    assert_eq!(store.epoch(), 3);
    assert_eq!(db.get(&project_key(&second)).unwrap(), None);

    // excluir a atual: abre a primeira da lista
    store.delete_project(&copy).unwrap();
    assert_eq!(store.current_id(), first);
    assert_eq!(store.epoch(), 4);

    // excluir a última: nasce uma de exemplo
    store.delete_project(&first).unwrap();
    assert_eq!(store.projects().len(), 1);
    assert_ne!(store.current_id(), first);
    assert_eq!(**store.song(), demo_song());
    assert_eq!(store.epoch(), 5);
    assert_eq!(db.keys("wadawko:project:").unwrap(), [project_key(store.current_id())]);
    assert_eq!(read_workspace(&db).unwrap().unwrap(), *store.workspace());
    assert!(store.delete_project("p-nao-existe").is_err());
}

#[test]
fn pending_edit_is_not_resurrected_by_delete() {
    let db = Db::open_in_memory().unwrap();
    let mut store = ProjectStore::open(db.clone()).unwrap();
    let first = store.current_id().to_string();
    let second = store.new_project().unwrap();
    store.update_song(Arc::new(song_titled("pendente")));
    store.delete_project(&second).unwrap();
    store.flush().unwrap();
    drop(store);
    assert_eq!(db.keys("wadawko:project:").unwrap(), [project_key(&first)]);
    let ws = read_workspace(&db).unwrap().unwrap();
    assert_eq!(ws.projects.len(), 1);
    assert_eq!(ws.current_id.as_deref(), Some(first.as_str()));
}

#[test]
fn save_updates_title_and_timestamp() {
    let db = Db::open_in_memory().unwrap();
    let mut store = ProjectStore::open(db.clone()).unwrap();
    let id = store.current_id().to_string();
    let t0 = store.current_meta().unwrap().updated_at;
    std::thread::sleep(Duration::from_millis(5));
    store.update_song(Arc::new(song_titled("Hachijō")));
    store.save().unwrap();
    let ws = read_workspace(&db).unwrap().unwrap();
    assert_eq!(ws.projects[0].title, "Hachijō");
    assert!(ws.projects[0].updated_at > t0);
    assert_eq!(read_song(&db, &id).unwrap().title, "Hachijō");
    assert_eq!(store.autosave_error(), None);
}

#[test]
fn drop_flushes_pending_autosave() {
    let tmp = TempDir::new("dropflush");
    let path = tmp.join("w.db");
    let id;
    {
        let mut store = ProjectStore::open_path(&path).unwrap();
        id = store.current_id().to_string();
        store.update_song(Arc::new(song_titled("gravada ao fechar")));
        // sem esperar os 400 ms
    }
    assert!(std::fs::metadata(wal_path(&path)).map_or(true, |m| m.len() == 0), "checkpoint ao fechar");
    let db = Db::open_read_only(&path).unwrap();
    assert_eq!(read_song(&db, &id).unwrap().title, "gravada ao fechar");
    assert_eq!(read_workspace(&db).unwrap().unwrap().projects[0].title, "gravada ao fechar");
}

#[test]
fn bootstrap_keeps_list_when_current_is_unreadable() {
    let db = Db::open_in_memory().unwrap();
    let good = add_song(&db, &song_titled("boa"), false).unwrap();
    let bad = add_song(&db, &song_titled("quebrada"), true).unwrap();
    db.set(&project_key(&bad), "{\"version\":3}").unwrap();

    let store = ProjectStore::open(db.clone()).unwrap();
    assert_eq!(store.current_id(), good, "abre a primeira legível");
    assert_eq!(store.projects().len(), 2, "a lista não é recriada");
    assert!(store.read_song(&bad).is_err());
    drop(store);

    // nenhuma legível: a de exemplo entra no fim, as outras continuam listadas
    db.set(&project_key(&good), "não é json").unwrap();
    let store = ProjectStore::open(db.clone()).unwrap();
    assert_eq!(store.projects().len(), 3);
    assert_eq!(**store.song(), demo_song());
    assert_eq!(db.get(&project_key(&good)).unwrap().as_deref(), Some("não é json"), "o conteúdo ilegível não é apagado");
}

#[test]
fn read_only_store_never_writes() {
    let tmp = TempDir::new("rostore");
    let path = tmp.join("w.db");
    drop(Db::open(&path).unwrap()); // banco vazio
    let before = std::fs::read(&path).unwrap();
    let mut store = ProjectStore::open(Db::open_read_only(&path).unwrap()).unwrap();
    assert!(store.is_read_only());
    assert_eq!(**store.song(), demo_song(), "demo só em memória");
    store.update_song(Arc::new(song_titled("x")));
    assert_eq!(store.song().title, "x");
    assert!(!store.has_unsaved_changes());
    assert!(store.save().is_err());
    assert!(store.new_project().is_err());
    assert!(store.delete_project(&store.current_id().to_string()).is_err());
    assert!(store.set_theme(Theme::Dark).is_err());
    store.flush().unwrap();
    drop(store);
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn theme_and_native_settings() {
    let db = Db::open_in_memory().unwrap();
    let store = ProjectStore::open(db.clone()).unwrap();
    assert_eq!(store.theme().unwrap(), None);
    store.set_theme(Theme::Dark).unwrap();
    assert_eq!(db.get(THEME_KEY).unwrap().as_deref(), Some("dark"));
    assert_eq!(store.theme().unwrap(), Some(Theme::Dark));
    assert_eq!(Theme::Dark.toggled(), Theme::Light);
    db.set(THEME_KEY, "roxo").unwrap();
    assert_eq!(store.theme().unwrap(), None);

    assert_eq!(store.setting::<f64>("zoom").unwrap(), None);
    store.set_setting("zoom", &1.5).unwrap();
    store.set_setting("window", &serde_json::json!({"w": 1280, "h": 800})).unwrap();
    assert_eq!(store.setting::<f64>("zoom").unwrap(), Some(1.5));
    assert_eq!(store.setting::<String>("zoom").unwrap(), None, "tipo errado = ausente");
    assert_eq!(store.settings().unwrap().len(), 2);
    assert!(store.remove_setting("zoom").unwrap());
    assert!(!store.remove_setting("zoom").unwrap());
    // chave própria, fora das do app web
    assert!(!NATIVE_SETTINGS_KEY.starts_with("wadawko:project:"));
    let raw: Value = serde_json::from_str(&db.get(NATIVE_SETTINGS_KEY).unwrap().unwrap()).unwrap();
    assert_eq!(raw, serde_json::json!({"window": {"w": 1280, "h": 800}}));
    // não aparece como música
    assert_eq!(list_projects(&db).unwrap().len(), 1);
}

// ───────────────────────────── library ─────────────────────────────

#[test]
fn library_list_find_add() {
    let db = Db::open_in_memory().unwrap();
    assert!(list_projects(&db).unwrap().is_empty());
    assert!(find_project(&db, "x").is_err());

    let a = add_song(&db, &song_titled("Renshū"), false).unwrap();
    let b = add_song(&db, &song_titled("Renshū"), false).unwrap();
    let c = add_song(&db, &song_titled("Mikazuchi"), false).unwrap();
    assert_eq!([&a, &b, &c].iter().collect::<std::collections::BTreeSet<_>>().len(), 3, "ids distintos");
    let ws = read_workspace(&db).unwrap().unwrap();
    assert_eq!(ws.current_id.as_deref(), Some(a.as_str()), "a primeira vira a atual");

    // órfã e ilegível
    db.set(&project_key("p-orfa-1"), &song_titled("Órfã").to_json()).unwrap();
    db.set(&project_key(&b), "{\"version\":1}").unwrap();

    let list = list_projects(&db).unwrap();
    assert_eq!(list.len(), 4);
    assert!(list[0].current && list[0].listed);
    assert!(list[1].song.is_err() && list[1].bytes == Some(13));
    assert_eq!(list[3].meta.title, "Órfã");
    assert!(!list[3].listed);

    assert_eq!(find_project(&db, &c).unwrap().meta.title, "Mikazuchi");
    assert_eq!(find_project(&db, "mikazuchi").unwrap().meta.id, c);
    assert_eq!(find_project(&db, "kazu").unwrap().meta.id, c);
    assert_eq!(find_project(&db, "órfã").unwrap().meta.id, "p-orfa-1");
    let err = find_project(&db, "Renshū").unwrap_err().to_string();
    assert!(err.contains("ambíguo") && err.contains(&a) && err.contains(&b), "{err}");
    assert!(find_project(&db, "nada disso").is_err());
}

// ───────────────────────────── Autosave ─────────────────────────────

fn wait_until(what: &str, f: impl Fn() -> bool) {
    let start = Instant::now();
    while !f() {
        assert!(start.elapsed() < Duration::from_secs(10), "tempo esgotado esperando: {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn autosave_debounces_bursts_into_one_write() {
    let db = Db::open_in_memory().unwrap();
    let auto = Autosave::with_timing(db.clone(), Duration::from_millis(150), Duration::from_secs(60));
    for i in 0..20 {
        auto.schedule("k", Payload::Text(format!("v{i}")));
        auto.schedule("song", Payload::Song(Arc::new(song_titled(&format!("t{i}")))));
    }
    assert!(auto.is_dirty());
    assert_eq!(db.get("k").unwrap(), None, "nada gravado antes do debounce");
    wait_until("gravação", || !auto.is_dirty());
    assert_eq!(auto.write_count(), 1);
    assert_eq!(db.get("k").unwrap().as_deref(), Some("v19"));
    assert_eq!(Song::from_json(&db.get("song").unwrap().unwrap()).unwrap().title, "t19");

    auto.schedule("k", Payload::Delete);
    auto.flush().unwrap();
    assert_eq!(db.get("k").unwrap(), None);
    assert_eq!(auto.write_count(), 2);
    assert_eq!(auto.last_error(), None);
    auto.flush().unwrap(); // sem pendências: imediato
}

#[test]
fn autosave_waits_for_quiet_but_not_forever() {
    let db = Db::open_in_memory().unwrap();
    // debounce longo, teto curto: edições contínuas ainda assim são gravadas
    let auto = Autosave::with_timing(db.clone(), Duration::from_secs(30), Duration::from_millis(200));
    let start = Instant::now();
    auto.schedule("k", Payload::Text("a".into()));
    std::thread::sleep(Duration::from_millis(50));
    auto.schedule("k", Payload::Text("b".into()));
    wait_until("teto de espera", || auto.write_count() == 1);
    assert!(start.elapsed() >= Duration::from_millis(190), "esperou o teto ({:?})", start.elapsed());
    assert_eq!(db.get("k").unwrap().as_deref(), Some("b"));
}

#[test]
fn autosave_default_debounce_is_400ms() {
    assert_eq!(AUTOSAVE_DEBOUNCE, Duration::from_millis(400));
    let db = Db::open_in_memory().unwrap();
    let auto = Autosave::new(db.clone());
    let start = Instant::now();
    auto.schedule("k", Payload::Text("v".into()));
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(db.get("k").unwrap(), None, "ainda dentro do debounce");
    wait_until("gravação", || db.get("k").unwrap().is_some());
    assert!(start.elapsed() >= Duration::from_millis(390));
}

#[test]
fn autosave_flushes_on_drop_and_reports_errors() {
    let db = Db::open_in_memory().unwrap();
    {
        let auto = Autosave::with_timing(db.clone(), Duration::from_secs(60), Duration::from_secs(60));
        auto.schedule("k", Payload::Text("no drop".into()));
    }
    assert_eq!(db.get("k").unwrap().as_deref(), Some("no drop"));

    // banco somente leitura: a gravação falha, o erro aparece e o drop não trava
    let tmp = TempDir::new("autoerr");
    let path = tmp.join("w.db");
    drop(Db::open(&path).unwrap());
    let auto = Autosave::with_timing(Db::open_read_only(&path).unwrap(), Duration::from_millis(1), Duration::from_secs(1));
    auto.schedule("k", Payload::Text("v".into()));
    let err = auto.flush().unwrap_err().to_string();
    assert!(err.contains("somente leitura"), "{err}");
    assert!(auto.last_error().is_some());
    assert!(auto.is_dirty(), "continua pendente para nova tentativa");
}

// ───────────────────────────── History ─────────────────────────────

#[test]
fn history_undo_redo_like_the_web_app() {
    let mut h: SongHistory = History::new();
    let v0 = Arc::new(song_titled("v0"));
    let v1 = Arc::new(song_titled("v1"));
    let v2 = Arc::new(song_titled("v2"));
    let v3 = Arc::new(song_titled("v3"));
    assert!(!h.can_undo() && !h.can_redo());
    assert!(h.undo(v0.clone()).is_none());

    h.record(v0.clone()); // v0 → v1
    h.record(v1.clone()); // v1 → v2
    assert_eq!(h.undo_len(), 2);

    let cur = h.undo(v2.clone()).unwrap();
    assert!(Arc::ptr_eq(&cur, &v1), "devolve o mesmo Arc, sem copiar a música");
    let cur = h.undo(cur).unwrap();
    assert!(Arc::ptr_eq(&cur, &v0));
    assert!(h.undo(cur.clone()).is_none());
    assert_eq!(h.redo_len(), 2);

    let cur = h.redo(cur).unwrap();
    assert!(Arc::ptr_eq(&cur, &v1));
    // edição nova depois de desfazer: o refazer some
    h.record(cur); // v1 → v3
    assert!(!h.can_redo());
    assert!(h.redo(v3.clone()).is_none());
    let cur = h.undo(v3).unwrap();
    assert!(Arc::ptr_eq(&cur, &v1));

    h.clear();
    assert!(!h.can_undo() && !h.can_redo());
}

#[test]
fn history_keeps_100_levels() {
    assert_eq!(DEFAULT_HISTORY_LIMIT, 100);
    let mut h = History::new();
    for i in 0..250 {
        h.record(i);
    }
    assert_eq!(h.undo_len(), 100);
    let mut cur = 250;
    let mut seen = Vec::new();
    while let Some(prev) = h.undo(cur) {
        seen.push(prev);
        cur = prev;
    }
    assert_eq!(seen.len(), 100);
    assert_eq!(seen[0], 249);
    assert_eq!(*seen.last().unwrap(), 150, "os 150 mais antigos saíram");
    assert_eq!(h.redo_len(), 100);
    // refazer devolve tudo, sem passar do limite
    while let Some(next) = h.redo(cur) {
        cur = next;
    }
    assert_eq!(cur, 250);
    assert_eq!(h.undo_len(), 100);

    let mut small = History::with_limit(2);
    small.record(1);
    small.record(2);
    small.record(3);
    assert_eq!(small.undo(4), Some(3));
    assert_eq!(small.undo(3), Some(2));
    assert_eq!(small.undo(2), None);
}

#[test]
fn history_coalesces_tagged_edits() {
    const VOLUME: u64 = 1;
    const TITLE: u64 = 2;
    let mut h = History::new();
    h.set_coalesce_window(Duration::from_millis(500));
    let t = Instant::now();
    let ms = Duration::from_millis;

    // arrastando o fader: 0 → 1 → 2 → 3
    assert!(h.record_coalesced_at(0, VOLUME, t));
    assert!(!h.record_coalesced_at(1, VOLUME, t + ms(100)));
    assert!(!h.record_coalesced_at(2, VOLUME, t + ms(500)), "a janela conta da última edição");
    assert_eq!(h.undo_len(), 1);
    // outra etiqueta: nível novo
    assert!(h.record_coalesced_at(3, TITLE, t + ms(550)));
    // mesma etiqueta, mas fora da janela
    assert!(h.record_coalesced_at(4, TITLE, t + ms(2000)));
    // seal() encerra o grupo
    h.seal();
    assert!(h.record_coalesced_at(5, TITLE, t + ms(2001)));
    // record() comum também encerra
    h.record(6);
    assert!(h.record_coalesced_at(7, TITLE, t + ms(2002)));
    assert_eq!(h.undo_len(), 6);

    // um desfazer volta o gesto inteiro do fader
    let mut h = History::new();
    h.record_coalesced_at(0, VOLUME, t);
    h.record_coalesced_at(1, VOLUME, t + ms(10));
    h.record_coalesced_at(2, VOLUME, t + ms(20));
    assert_eq!(h.undo(3), Some(0));
    // depois de desfazer, a mesma etiqueta abre nível novo (não funde com o grupo desfeito)
    assert!(h.record_coalesced_at(0, VOLUME, t + ms(30)));
    assert!(!h.can_redo());
}

#[test]
fn history_clears_when_project_epoch_changes() {
    let db = Db::open_in_memory().unwrap();
    let mut store = ProjectStore::open(db).unwrap();
    let mut h: SongHistory = History::new();
    assert!(!h.sync_epoch(store.epoch()), "primeira chamada só registra");

    h.record(store.song().clone());
    store.update_song(Arc::new(song_titled("editada")));
    assert!(!h.sync_epoch(store.epoch()), "editar não muda o epoch");
    assert!(h.can_undo());

    store.new_project().unwrap();
    assert!(h.sync_epoch(store.epoch()));
    assert!(!h.can_undo() && !h.can_redo());
    assert!(!h.sync_epoch(store.epoch()));
}

// ───────────────────────────── backup ─────────────────────────────

fn backup_dict() -> serde_json::Map<String, Value> {
    serde_json::from_str(&std::fs::read_to_string(backup_fixture()).unwrap()).unwrap()
}

#[test]
fn import_backup_into_empty_db_is_exact() {
    let db = Db::open_in_memory().unwrap();
    let report = import_backup_file(&db, backup_fixture(), BackupOptions::default()).unwrap();
    assert_eq!(report.added.len(), 2);
    assert!(report.workspace_created && report.changed());
    assert!(report.invalid.is_empty() && report.kept_existing.is_empty() && report.ignored.is_empty(), "{report:?}");
    assert_eq!(report.other_set, [THEME_KEY]);

    // o banco fica igual ao dicionário do backup, byte a byte
    let dict = backup_dict();
    let all = db.all().unwrap();
    assert_eq!(all.len(), dict.len());
    for (k, v) in &dict {
        if k == WORKSPACE_KEY {
            assert_eq!(serde_json::from_str::<Value>(&all[k]).unwrap(), serde_json::from_str::<Value>(v.as_str().unwrap()).unwrap());
        } else {
            assert_eq!(all[k], v.as_str().unwrap(), "{k}");
        }
    }

    // e o app abre nele sem mexer em nada
    let store = ProjectStore::open(db.clone()).unwrap();
    let backup_ws = Workspace::from_json(dict[WORKSPACE_KEY].as_str().unwrap()).unwrap();
    assert_eq!(*store.workspace(), backup_ws);
    assert_eq!(store.projects().len(), 2);
    for p in store.projects() {
        store.read_song(&p.id).unwrap();
    }
    assert_eq!(store.theme().unwrap(), Some(Theme::Light));
    drop(store);

    // importar de novo não muda nada
    let again = import_backup_file(&db, backup_fixture(), BackupOptions { overwrite: true }).unwrap();
    assert_eq!(again.unchanged.len(), 2);
    assert!(!again.changed(), "{again:?}");
    assert_eq!(db.all().unwrap(), all);
}

#[test]
fn import_backup_merges_without_destroying() {
    let dict = backup_dict();
    let backup_ws = Workspace::from_json(dict[WORKSPACE_KEY].as_str().unwrap()).unwrap();
    let clash = backup_ws.projects[0].id.clone();

    let db = Db::open_in_memory().unwrap();
    let mine = add_song(&db, &song_titled("minha"), true).unwrap();
    // uma música do backup já existe no banco, com conteúdo mais novo
    let newer = song_titled("versão mais nova");
    db.set(&project_key(&clash), &newer.to_json()).unwrap();
    let mut ws = read_workspace(&db).unwrap().unwrap();
    ws.projects.push(wadawko_core::model::ProjectMeta { id: clash.clone(), title: "versão mais nova".into(), updated_at: 5, ..Default::default() });
    db.set(WORKSPACE_KEY, &ws.to_json()).unwrap();
    db.set(THEME_KEY, "dark").unwrap();

    let report = import_backup_file(&db, backup_fixture(), BackupOptions::default()).unwrap();
    assert_eq!(report.added.len(), 1);
    assert_eq!(report.kept_existing, [clash.clone()]);
    assert!(report.other_set.is_empty() && !report.workspace_created);
    assert_eq!(read_song(&db, &clash).unwrap(), newer, "a existente não foi tocada");
    assert_eq!(db.get(THEME_KEY).unwrap().as_deref(), Some("dark"));
    let ws = read_workspace(&db).unwrap().unwrap();
    assert_eq!(ws.current_id.as_deref(), Some(mine.as_str()), "a música aberta continua a mesma");
    assert_eq!(ws.projects.len(), 3);
    assert_eq!(ws.projects[0].id, mine);
    assert_eq!(ws.projects[1].updated_at, 5);
    // as novas entram na ordem do backup, com a entrada (título/data) do backup
    let expected: Vec<_> = backup_ws.projects[1..].to_vec();
    assert_eq!(ws.projects[2..], expected[..]);

    // --overwrite substitui a que difere e o tema
    let report = import_backup_file(&db, backup_fixture(), BackupOptions { overwrite: true }).unwrap();
    assert_eq!(report.overwritten.len(), 1);
    assert_eq!(report.unchanged.len(), 1);
    assert_eq!(db.get(&project_key(&clash)).unwrap().as_deref(), dict[&project_key(&clash)].as_str());
    assert_eq!(db.get(THEME_KEY).unwrap().as_deref(), Some("light"));
    let ws = read_workspace(&db).unwrap().unwrap();
    assert_eq!(ws.projects[1], backup_ws.projects[0], "entrada da lista volta a ser a do backup");
    assert_eq!(ws.projects.len(), 3);
    assert!(read_song(&db, &mine).is_ok());
}

#[test]
fn import_backup_reports_bad_entries() {
    let db = Db::open_in_memory().unwrap();
    let good = song_titled("boa").to_json();
    let backup = serde_json::json!({
        "wadawko:project:p-ok-1": good,
        "wadawko:project:p-v2-1": "{\"version\":2,\"tracks\":[],\"measures\":[]}",
        "wadawko:project:p-lixo-1": "não é json",
        "wadawko:project:p-num-1": 42,
        "wadawko:__dirty": "[\"wadawko:workspace\"]",
        "taiko-fu:song": "{}",
    })
    .to_string();
    let report = import_backup(&db, &backup, BackupOptions::default()).unwrap();
    assert_eq!(report.added, [("p-ok-1".to_string(), "boa".to_string())]);
    assert_eq!(report.invalid.len(), 3, "{report:?}");
    assert_eq!(report.ignored, ["taiko-fu:song", "wadawko:__dirty"]);
    assert_eq!(db.keys("").unwrap(), ["wadawko:project:p-ok-1", WORKSPACE_KEY]);
    // sem workspace no backup: um é montado com o que entrou
    let ws = read_workspace(&db).unwrap().unwrap();
    assert_eq!(ws.projects.len(), 1);
    assert_eq!(ws.projects[0].title, "boa");
    assert_eq!(ws.current_id.as_deref(), Some("p-ok-1"));

    assert!(import_backup(&db, "[1,2]", BackupOptions::default()).is_err());
    assert!(import_backup(&db, "{", BackupOptions::default()).is_err());
    assert!(import_backup_file(&db, "/nao/existe.json", BackupOptions::default()).is_err());
}

// ───────────────────────────── arquivo de música ─────────────────────────────

#[test]
fn song_file_roundtrip() {
    let tmp = TempDir::new("songfile");
    let song = Song { title: "Ōdaiko no Uta".into(), ..demo_song() };
    assert_eq!(song_file_name(&song), "ōdaiko-no-uta.wadawko.json");
    assert_eq!(song_file_name(&song_titled("")), "musica.wadawko.json");

    let path = export_song_to_dir(&song, tmp.path()).unwrap();
    assert_eq!(path, tmp.join("ōdaiko-no-uta.wadawko.json"));
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, song.to_json_pretty(), "mesmo formato do export do app web");
    assert!(text.starts_with("{\n  \"version\": 1,"));
    assert_eq!(read_song_file(&path).unwrap(), song);
    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 1, "sem temporário esquecido");

    // .taiko.json do app web (compacto ou não, com BOM)
    let legacy = tmp.join("renshū.taiko.json");
    std::fs::write(&legacy, format!("\u{feff}{}", song.to_json())).unwrap();
    assert_eq!(read_song_file(&legacy).unwrap(), song);
    assert!(is_song_file(&legacy) && is_song_file(&path) && is_song_file("X.WADAWKO.JSON".as_ref()));
    assert!(!is_song_file("backup.json".as_ref()));

    // erros com as mensagens do importSong
    let bad = tmp.join("bad.json");
    std::fs::write(&bad, "{ nope").unwrap();
    assert!(read_song_file(&bad).unwrap_err().to_string().contains("JSON inválido"));
    std::fs::write(&bad, "{\"version\":2,\"tracks\":[],\"measures\":[]}").unwrap();
    assert!(read_song_file(&bad).unwrap_err().to_string().contains("não parece ser uma música"));
    assert!(read_song_file(tmp.join("nao-existe.json")).is_err());
    assert!(write_song_file(&song, tmp.join("pasta-que-nao-existe/x.json")).is_err());
}

#[test]
fn song_file_into_store_as_new_project() {
    let tmp = TempDir::new("songimport");
    let file = tmp.join("m.wadawko.json");
    write_song_file(&song_titled("Importada"), &file).unwrap();
    let db = Db::open_in_memory().unwrap();
    let mut store = ProjectStore::open(db.clone()).unwrap();
    let id = store.add_project(read_song_file(&file).unwrap()).unwrap();
    assert_eq!(store.current_id(), id);
    assert_eq!(store.projects().len(), 2);
    assert_eq!(store.epoch(), 1);
    assert_eq!(read_song(&db, &id).unwrap().title, "Importada");
}

// ───────────────────────────── migração ─────────────────────────────

#[test]
fn migration_copies_db_including_wal_without_touching_source() {
    let tmp = TempDir::new("migrate");
    let src = tmp.join("repo/data/wadawko.db");
    let dst = tmp.join("xdg/wadawko/wadawko.db");

    assert_eq!(migrate_db_if_missing(&src, &dst).unwrap(), Migration::SourceMissing);
    assert!(!dst.exists());

    // origem com parte dos dados só no WAL (conexão ainda aberta = sem checkpoint)
    drop(Db::open(&src).unwrap());
    let holder = Db::open(&src).unwrap();
    import_backup_file(&holder, backup_fixture(), BackupOptions::default()).unwrap();
    assert!(std::fs::metadata(wal_path(&src)).unwrap().len() > 0);
    assert!(std::fs::metadata(&src).unwrap().len() < 20_000, "o arquivo principal ainda não tem as músicas");
    let src_main = std::fs::read(&src).unwrap();
    let src_wal = std::fs::read(wal_path(&src)).unwrap();
    let expected = holder.all().unwrap();

    let Migration::Copied { keys, bytes } = migrate_db_if_missing(&src, &dst).unwrap() else { panic!("deveria copiar") };
    assert_eq!(keys, 4);
    assert!(bytes > 40_000);
    assert_eq!(std::fs::read(&src).unwrap(), src_main, "origem intacta");
    assert_eq!(std::fs::read(wal_path(&src)).unwrap(), src_wal, "WAL da origem intacto");
    assert!(!tmp.join("xdg/wadawko/wadawko.db.migrating").exists());

    // o destino é um banco completo e independente
    {
        let copy = Db::open(&dst).unwrap();
        assert_eq!(copy.all().unwrap(), expected);
        copy.set("só-no-destino", "1").unwrap();
    }
    assert_eq!(holder.get("só-no-destino").unwrap(), None);

    // segunda execução: não sobrescreve
    assert_eq!(migrate_db_if_missing(&src, &dst).unwrap(), Migration::DestinationExists);
    assert_eq!(Db::open_read_only(&dst).unwrap().get("só-no-destino").unwrap().as_deref(), Some("1"));
}

#[test]
fn migration_rejects_non_database() {
    let tmp = TempDir::new("migratebad");
    let src = tmp.join("lixo.db");
    std::fs::write(&src, "isto não é um banco sqlite, é só texto ".repeat(50)).unwrap();
    let dst = tmp.join("out/wadawko.db");
    assert!(migrate_db_if_missing(&src, &dst).is_err());
    assert!(!dst.exists(), "falha não deixa destino pela metade");
    assert_eq!(std::fs::read_dir(tmp.join("out")).unwrap().count(), 0);
}
