//! Teste com dados reais, opcional: `WADAWKO_TEST_DB_COPY=/pasta/com/copia/wadawko.db`.
//!
//! A variável deve apontar para uma CÓPIA do banco (com `-wal` e `-shm` ao
//! lado). O teste ainda copia os três arquivos para uma pasta temporária antes
//! de abrir qualquer coisa em modo escrita; nunca aponte para `data/wadawko.db`.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, wal_path};
use serde_json::Value;
use wadawko_core::Song;
use wadawko_core::model::PROJECT_KEY_PREFIX;
use wadawko_store::*;

fn copy_trio(src: &Path, dir: &Path) -> PathBuf {
    let dst = dir.join("wadawko.db");
    for suffix in ["", "-wal", "-shm"] {
        let mut from = src.as_os_str().to_owned();
        from.push(suffix);
        let mut to = dst.as_os_str().to_owned();
        to.push(suffix);
        if Path::new(&from).exists() {
            std::fs::copy(&from, &to).unwrap();
        }
    }
    dst
}

#[test]
fn real_database_copy_reads_and_survives_a_write_cycle() {
    let Some(src) = std::env::var_os("WADAWKO_TEST_DB_COPY").map(PathBuf::from) else {
        eprintln!("WADAWKO_TEST_DB_COPY não definido: teste com dados reais pulado");
        return;
    };
    assert!(!src.components().any(|c| c.as_os_str() == "data"), "use uma cópia, não o banco do repositório");
    let tmp = TempDir::new("real");
    let path = copy_trio(&src, tmp.path());

    // 1. somente leitura: tudo lê, e o JSON regravado é equivalente
    let before = {
        let ro = Db::open_read_only(&path).unwrap();
        let projects = list_projects(&ro).unwrap();
        assert!(!projects.is_empty());
        for p in &projects {
            let song = p.song.as_ref().unwrap_or_else(|e| panic!("{} ({}) não leu: {e}", p.meta.id, p.meta.title));
            let raw = ro.get(&format!("{PROJECT_KEY_PREFIX}{}", p.meta.id)).unwrap().unwrap();
            let original: Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(song.to_value(), original, "{}: o round-trip mudou o JSON", p.meta.id);
            assert_eq!(Song::from_json(&song.to_json()).unwrap(), *song);
            eprintln!("{} {:<12} {:>3} compassos  {:>6} bytes  byte-idêntico: {}", p.meta.id, p.meta.title, song.measures.len(), raw.len(), song.to_json() == raw);
        }
        assert!(projects.iter().any(|p| p.current), "currentId aponta para uma música listada");
        ro.all().unwrap()
    };

    // 2. abrir como o app e fechar: nada muda, e o WAL é incorporado
    {
        let store = ProjectStore::open_path(&path).unwrap();
        assert!(store.projects().len() >= 4);
    }
    assert!(std::fs::metadata(wal_path(&path)).map_or(true, |m| m.len() == 0), "WAL incorporado ao fechar");
    let db = Db::open(&path).unwrap();
    assert_eq!(db.all().unwrap(), before, "abrir e fechar não altera nenhuma chave");
    drop(db);

    // 3. migração para um caminho novo
    let dst = tmp.join("xdg/wadawko/wadawko.db");
    let fresh = copy_trio(&src, &tmp.join("."));
    assert!(matches!(migrate_db_if_missing(&fresh, &dst).unwrap(), Migration::Copied { .. }));
    assert_eq!(Db::open_read_only(&dst).unwrap().all().unwrap(), before);
}
