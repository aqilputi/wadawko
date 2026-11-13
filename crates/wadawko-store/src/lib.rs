//! # wadawko-store
//!
//! Persistência do WaDAWko nativo, sobre o MESMO banco do app web
//! (`data/wadawko.db`: tabela `kv`, chaves `wadawko:workspace`,
//! `wadawko:project:<id>`, `wadawko:theme`).
//!
//! | módulo | origem (TS) | o que tem |
//! |---|---|---|
//! | [`db`] | `server/dbPlugin.ts` | [`Db`]: SQLite `kv` (WAL, checkpoint ao fechar, somente leitura opcional), caminho XDG |
//! | [`library`] | `readSong`/`readWorkspace` | leitura/listagem/busca sem estado (o que o CLI usa) |
//! | [`project`] | `store.svelte.ts` | [`ProjectStore`]: workspace + música atual, trocar/nova/duplicar/renomear/excluir, tema, settings |
//! | [`autosave`] | `db.ts` (`schedule`) | [`Autosave`]: debounce de 400 ms, escrita em thread de I/O |
//! | [`history`] | `App.svelte` | [`History`]: desfazer/refazer com 100 níveis |
//! | [`backup`] | — | import do backup do localStorage |
//! | [`songfile`] | `exportSong`/`importSong` | arquivo `.wadawko.json` / `.taiko.json` |
//! | [`migrate`] | — | cópia de primeira execução para o caminho XDG |
//!
//! ## Uso na UI
//!
//! ```no_run
//! use std::sync::Arc;
//! use wadawko_store::{Db, History, ProjectStore, default_db_path};
//! # fn main() -> anyhow::Result<()> {
//! let mut store = ProjectStore::open(Db::open(default_db_path()?)?)?;
//! let mut history: History<Arc<wadawko_core::Song>> = History::new();
//!
//! // uma edição
//! let before = store.song().clone();
//! let mut song = (*before).clone();
//! song.tempo = 120.0;
//! history.record(before);
//! store.update_song(Arc::new(song)); // grava sozinho, 400 ms depois
//!
//! // desfazer
//! if let Some(prev) = history.undo(store.song().clone()) {
//!     store.update_song(prev);
//! }
//!
//! // trocar de música zera o histórico
//! # let other_id = "p-x-1";
//! store.switch_project(other_id)?;
//! history.sync_epoch(store.epoch());
//!
//! // ao encerrar
//! store.flush()?;
//! # Ok(()) }
//! ```

pub mod autosave;
pub mod backup;
pub mod db;
pub mod history;
pub mod library;
pub mod migrate;
pub mod project;
pub mod songfile;

pub use autosave::{AUTOSAVE_DEBOUNCE, AUTOSAVE_MAX_WAIT, Autosave, Payload};
pub use backup::{BackupOptions, BackupReport, import_backup, import_backup_file};
pub use db::{Db, KvEntry, KvWrite, default_data_dir, default_db_path, resolve_db_path};
pub use history::{DEFAULT_HISTORY_LIMIT, History};
pub use library::{ProjectInfo, UNTITLED, add_song, display_title, find_project, list_projects, read_song, read_workspace};
pub use migrate::{Migration, migrate_db_if_missing};
pub use project::{NATIVE_SETTINGS_KEY, ProjectStore, Theme};
pub use songfile::{IMPORT_EXTENSIONS, export_song_to_dir, is_song_file, read_song_file, song_file_name, write_song_file};

/// Histórico de músicas da UI.
pub type SongHistory = History<std::sync::Arc<wadawko_core::Song>>;
