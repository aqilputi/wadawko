//! Banco SQLite `kv` — a mesma tabela e as mesmas chaves do app web
//! (`server/dbPlugin.ts`): `kv(key TEXT PRIMARY KEY, value TEXT, updated_at INTEGER)`.
//!
//! [`Db`] é barato de clonar (um `Arc`) e pode ir para outra thread: a thread
//! de I/O do [`crate::Autosave`] usa um clone. Quando o último clone é solto, o
//! WAL é incorporado ao arquivo principal (`wal_checkpoint(TRUNCATE)`).

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use rusqlite::{Connection, OpenFlags, OptionalExtension as _, params};

/// Nome da pasta do app dentro de `$XDG_DATA_HOME`.
pub const APP_DIR: &str = "wadawko";
/// Nome do arquivo do banco.
pub const DB_FILE: &str = "wadawko.db";

/// Uma linha da tabela `kv`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvEntry {
    pub key: String,
    pub value: String,
    /// `Date.now()` da gravação (ms desde a época)
    pub updated_at: i64,
}

/// Uma escrita de um lote ([`Db::write_batch`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KvWrite {
    Set(String, String),
    Delete(String),
}

struct Inner {
    conn: Mutex<Connection>,
    path: Option<PathBuf>,
    read_only: bool,
    /// banco somente leitura sem a tabela `kv` (tratado como vazio)
    no_table: bool,
}

/// Conexão com o banco `kv`.
#[derive(Clone)]
pub struct Db {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db").field("path", &self.inner.path).field("read_only", &self.inner.read_only).finish()
    }
}

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS kv (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);";

fn now_ms() -> i64 {
    wadawko_core::ids::now_ms() as i64
}

impl Db {
    /// Abre (criando a pasta e o arquivo se preciso) em modo leitura/escrita,
    /// com `journal_mode = WAL`, e garante a tabela `kv`.
    pub fn open(path: impl AsRef<Path>) -> Result<Db> {
        let path = path.as_ref();
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).with_context(|| format!("criar a pasta {}", dir.display()))?;
        }
        let conn = Connection::open(path).with_context(|| format!("abrir o banco {}", path.display()))?;
        conn.busy_timeout(Duration::from_secs(5))?;
        // o PRAGMA devolve uma linha com o modo resultante
        let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0)).context("ativar o modo WAL")?;
        conn.execute_batch(CREATE_TABLE).context("criar a tabela kv")?;
        Ok(Db { inner: Arc::new(Inner { conn: Mutex::new(conn), path: Some(path.to_path_buf()), read_only: false, no_table: false }) })
    }

    /// Abre um banco existente em modo somente leitura: nenhuma escrita, nem
    /// criação de tabela, nem checkpoint. Um banco sem a tabela `kv` é lido como vazio.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Db> {
        let path = path.as_ref();
        if !path.is_file() {
            bail!("banco não encontrado: {}", path.display());
        }
        let conn =
            Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).with_context(|| format!("abrir o banco {} (somente leitura)", path.display()))?;
        conn.busy_timeout(Duration::from_secs(5))?;
        let has_table: bool =
            conn.query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'kv'", [], |r| r.get::<_, i64>(0)).with_context(|| format!("ler o banco {}", path.display()))? > 0;
        Ok(Db { inner: Arc::new(Inner { conn: Mutex::new(conn), path: Some(path.to_path_buf()), read_only: true, no_table: !has_table }) })
    }

    /// Abre em leitura/escrita ou somente leitura conforme `read_only`.
    pub fn open_with(path: impl AsRef<Path>, read_only: bool) -> Result<Db> {
        if read_only { Db::open_read_only(path) } else { Db::open(path) }
    }

    /// Banco em memória (testes, modo "sem disco").
    pub fn open_in_memory() -> Result<Db> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(CREATE_TABLE)?;
        Ok(Db { inner: Arc::new(Inner { conn: Mutex::new(conn), path: None, read_only: false, no_table: false }) })
    }

    /// Caminho do arquivo (`None` para banco em memória).
    pub fn path(&self) -> Option<&Path> {
        self.inner.path.as_deref()
    }

    pub fn is_read_only(&self) -> bool {
        self.inner.read_only
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        // um pânico com o lock na mão não invalida a conexão
        self.inner.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn ensure_writable(&self) -> Result<()> {
        if self.inner.read_only {
            bail!("banco aberto em modo somente leitura");
        }
        Ok(())
    }

    /// Valor de uma chave (`None` se não existir).
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        if self.inner.no_table {
            return Ok(None);
        }
        let conn = self.conn();
        let v = conn.query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get::<_, String>(0)).optional()?;
        Ok(v)
    }

    /// Linha completa de uma chave (com `updated_at`).
    pub fn entry(&self, key: &str) -> Result<Option<KvEntry>> {
        if self.inner.no_table {
            return Ok(None);
        }
        let conn = self.conn();
        let e = conn.query_row("SELECT key, value, updated_at FROM kv WHERE key = ?1", [key], |r| Ok(KvEntry { key: r.get(0)?, value: r.get(1)?, updated_at: r.get(2)? })).optional()?;
        Ok(e)
    }

    /// Grava (insere ou substitui) uma chave, carimbando `updated_at` com a hora atual.
    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.ensure_writable()?;
        let conn = self.conn();
        upsert(&conn, key, value, now_ms())?;
        Ok(())
    }

    /// Remove uma chave; devolve se ela existia.
    pub fn delete(&self, key: &str) -> Result<bool> {
        self.ensure_writable()?;
        let conn = self.conn();
        Ok(conn.execute("DELETE FROM kv WHERE key = ?1", [key])? > 0)
    }

    /// Aplica várias escritas numa única transação (tudo ou nada).
    pub fn write_batch(&self, writes: &[KvWrite]) -> Result<()> {
        self.ensure_writable()?;
        if writes.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let now = now_ms();
        for w in writes {
            match w {
                KvWrite::Set(k, v) => upsert(&tx, k, v, now)?,
                KvWrite::Delete(k) => {
                    tx.execute("DELETE FROM kv WHERE key = ?1", [k])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Linhas cujas chaves começam com `prefix` (`""` = todas), em ordem de chave.
    pub fn list(&self, prefix: &str) -> Result<Vec<KvEntry>> {
        if self.inner.no_table {
            return Ok(Vec::new());
        }
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT key, value, updated_at FROM kv WHERE substr(key, 1, length(?1)) = ?1 ORDER BY key")?;
        let rows = stmt.query_map([prefix], |r| Ok(KvEntry { key: r.get(0)?, value: r.get(1)?, updated_at: r.get(2)? }))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Só as chaves com o prefixo (sem carregar os valores).
    pub fn keys(&self, prefix: &str) -> Result<Vec<String>> {
        if self.inner.no_table {
            return Ok(Vec::new());
        }
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT key FROM kv WHERE substr(key, 1, length(?1)) = ?1 ORDER BY key")?;
        let rows = stmt.query_map([prefix], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Tudo como dicionário chave → valor (o mesmo que `GET /api/kv` devolve).
    pub fn all(&self) -> Result<BTreeMap<String, String>> {
        Ok(self.list("")?.into_iter().map(|e| (e.key, e.value)).collect())
    }

    /// Incorpora o WAL ao arquivo principal e o trunca. Sem efeito em banco
    /// somente leitura ou em memória.
    pub fn checkpoint(&self) -> Result<()> {
        if self.inner.read_only || self.inner.path.is_none() {
            return Ok(());
        }
        let conn = self.conn();
        checkpoint(&conn)
    }
}

fn upsert(conn: &Connection, key: &str, value: &str, now: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO kv (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, now],
    )?;
    Ok(())
}

fn checkpoint(conn: &Connection) -> Result<()> {
    // devolve (busy, log, checkpointed)
    let busy: i64 = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0)).context("checkpoint do WAL")?;
    if busy != 0 {
        bail!("checkpoint do WAL incompleto: banco em uso por outro processo");
    }
    Ok(())
}

impl Drop for Inner {
    fn drop(&mut self) {
        if self.read_only || self.path.is_none() {
            return;
        }
        if let Ok(conn) = self.conn.get_mut() {
            // melhor esforço: outro processo com o banco aberto pode impedir
            let _ = checkpoint(conn);
        }
    }
}

// ───────────────────────────── caminhos ─────────────────────────────

/// Pasta de dados do app a partir dos valores de `XDG_DATA_HOME` e `HOME`
/// (função pura, para teste). `XDG_DATA_HOME` vazio ou relativo é ignorado,
/// como manda a especificação XDG.
pub fn data_dir_from(xdg_data_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    let base = xdg_data_home.map(PathBuf::from).filter(|p| p.is_absolute()).or_else(|| home.filter(|h| !h.is_empty()).map(|h| PathBuf::from(h).join(".local").join("share")))?;
    Some(base.join(APP_DIR))
}

/// `$XDG_DATA_HOME/wadawko` (ou `~/.local/share/wadawko`).
pub fn default_data_dir() -> Result<PathBuf> {
    data_dir_from(std::env::var_os("XDG_DATA_HOME").as_deref(), std::env::var_os("HOME").as_deref()).context("não foi possível determinar a pasta de dados (defina XDG_DATA_HOME ou HOME)")
}

/// Caminho padrão do banco: `$XDG_DATA_HOME/wadawko/wadawko.db`.
pub fn default_db_path() -> Result<PathBuf> {
    Ok(default_data_dir()?.join(DB_FILE))
}

/// Caminho do banco a usar: o `override_path` (ex.: `--db`), senão o padrão XDG.
pub fn resolve_db_path(override_path: Option<&Path>) -> Result<PathBuf> {
    match override_path {
        Some(p) => Ok(p.to_path_buf()),
        None => default_db_path(),
    }
}
