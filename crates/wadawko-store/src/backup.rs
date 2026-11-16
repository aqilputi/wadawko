//! Import do backup do localStorage (`data/backup-localstorage-*.json`): um
//! dicionário chave → string, igual ao que `GET /api/kv` devolve.
//!
//! A importação é uma fusão que não destrói nada por padrão: músicas que o
//! banco ainda não tem entram (o JSON é gravado byte a byte como está no
//! backup); as que já existem ficam como estão, a menos de `overwrite`.

use std::path::Path;

use anyhow::{Context as _, Result, bail};
use serde_json::Value;
use wadawko_core::Song;
use wadawko_core::model::{PROJECT_KEY_PREFIX, ProjectMeta, THEME_KEY, WORKSPACE_KEY, Workspace};

use crate::db::{Db, KvWrite};
use crate::library::{display_title, now, read_workspace};

/// Opções de [`import_backup`].
#[derive(Clone, Copy, Debug, Default)]
pub struct BackupOptions {
    /// substitui músicas (e o tema) que já existem no banco com conteúdo diferente
    pub overwrite: bool,
}

/// O que a importação fez.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BackupReport {
    /// músicas novas: (id, título)
    pub added: Vec<(String, String)>,
    /// músicas substituídas (`overwrite`): (id, título)
    pub overwritten: Vec<(String, String)>,
    /// já estavam no banco com o mesmo conteúdo
    pub unchanged: Vec<String>,
    /// já estavam no banco com conteúdo diferente e foram mantidas (sem `overwrite`)
    pub kept_existing: Vec<String>,
    /// entradas do backup que não puderam ser importadas: (chave, motivo)
    pub invalid: Vec<(String, String)>,
    /// outras chaves gravadas (ex.: `wadawko:theme`)
    pub other_set: Vec<String>,
    /// chaves do backup ignoradas (desconhecidas ou internas do app web)
    pub ignored: Vec<String>,
    /// o banco não tinha workspace e um foi criado
    pub workspace_created: bool,
}

impl BackupReport {
    /// Alguma coisa foi gravada?
    pub fn changed(&self) -> bool {
        !self.added.is_empty() || !self.overwritten.is_empty() || !self.other_set.is_empty() || self.workspace_created
    }
}

/// Importa um arquivo de backup para o banco.
pub fn import_backup_file(db: &Db, path: impl AsRef<Path>, options: BackupOptions) -> Result<BackupReport> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).with_context(|| format!("ler {}", path.display()))?;
    import_backup(db, &text, options).with_context(|| format!("importar {}", path.display()))
}

/// Importa o conteúdo de um backup (texto JSON) para o banco, numa única transação.
pub fn import_backup(db: &Db, json: &str, options: BackupOptions) -> Result<BackupReport> {
    let Value::Object(dict) = serde_json::from_str::<Value>(json).context("o backup não é JSON válido")? else {
        bail!("o backup deveria ser um objeto JSON (chave → texto)");
    };
    if db.is_read_only() {
        bail!("banco aberto em modo somente leitura");
    }

    let mut report = BackupReport::default();
    let mut writes: Vec<KvWrite> = Vec::new();
    let backup_ws = dict.get(WORKSPACE_KEY).and_then(Value::as_str).and_then(Workspace::from_json);
    if dict.contains_key(WORKSPACE_KEY) && backup_ws.is_none() {
        report.invalid.push((WORKSPACE_KEY.into(), "não é um workspace version 2".into()));
    }
    let db_ws = read_workspace(db)?;
    report.workspace_created = db_ws.is_none();
    let mut ws = db_ws.unwrap_or_default();
    let mut ws_changed = false;

    // músicas presentes no banco depois da importação, na ordem do backup: (id, título)
    let mut present: Vec<(String, String)> = Vec::new();
    let mut retitle: Vec<(String, String)> = Vec::new();

    // ordem das chaves: alfabética, sempre. O `serde_json::Map` é BTreeMap (ordenado) ou
    // IndexMap (ordem do arquivo) conforme a feature `preserve_order` — que outro crate do
    // workspace liga por unificação de features; sem ordenar, o relatório mudava de ordem
    // entre `cargo test -p wadawko-store` e `cargo test --workspace`.
    let mut entries: Vec<(&String, &Value)> = dict.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    for (key, value) in entries {
        if let Some(id) = key.strip_prefix(PROJECT_KEY_PREFIX) {
            let Some(raw) = value.as_str() else {
                report.invalid.push((key.clone(), "o valor não é texto".into()));
                continue;
            };
            let song = match Song::from_json(raw) {
                Ok(s) => s,
                Err(e) => {
                    report.invalid.push((key.clone(), format!("{e} ({})", e.detail())));
                    continue;
                }
            };
            let title = display_title(&song);
            match db.get(key)? {
                None => {
                    writes.push(KvWrite::Set(key.clone(), raw.to_string()));
                    report.added.push((id.to_string(), title.clone()));
                }
                Some(cur) if cur == raw => report.unchanged.push(id.to_string()),
                Some(_) if options.overwrite => {
                    writes.push(KvWrite::Set(key.clone(), raw.to_string()));
                    report.overwritten.push((id.to_string(), title.clone()));
                    retitle.push((id.to_string(), title.clone()));
                }
                Some(_) => {
                    report.kept_existing.push(id.to_string());
                    continue; // a entrada do workspace do banco continua valendo
                }
            }
            present.push((id.to_string(), title));
        } else if key == THEME_KEY {
            match (value.as_str(), db.get(THEME_KEY)?) {
                (None, _) => report.invalid.push((key.clone(), "o valor não é texto".into())),
                (Some(v), None) => {
                    writes.push(KvWrite::Set(key.clone(), v.to_string()));
                    report.other_set.push(key.clone());
                }
                (Some(v), Some(cur)) if cur != v && options.overwrite => {
                    writes.push(KvWrite::Set(key.clone(), v.to_string()));
                    report.other_set.push(key.clone());
                }
                _ => {}
            }
        } else if key != WORKSPACE_KEY {
            report.ignored.push(key.clone());
        }
    }

    // lista: na ordem do workspace do backup; músicas fora dele vão para o fim
    let order = |id: &str| backup_ws.as_ref().and_then(|b| b.projects.iter().position(|p| p.id == id)).unwrap_or(usize::MAX);
    present.sort_by_key(|(id, _)| order(id));
    for (id, title) in &present {
        if ws.projects.iter().any(|p| &p.id == id) {
            continue;
        }
        let meta = backup_ws
            .as_ref()
            .and_then(|b| b.projects.iter().find(|p| &p.id == id).cloned())
            .unwrap_or_else(|| ProjectMeta { id: id.clone(), title: title.clone(), updated_at: now(), ..Default::default() });
        ws.projects.push(meta);
        ws_changed = true;
    }
    for (id, title) in retitle {
        if let Some(meta) = ws.projects.iter_mut().find(|p| p.id == id) {
            let backup_meta = backup_ws.as_ref().and_then(|b| b.projects.iter().find(|p| p.id == id));
            let updated_at = backup_meta.map_or_else(now, |m| m.updated_at);
            if meta.title != title || meta.updated_at != updated_at {
                meta.title = title;
                meta.updated_at = updated_at;
                ws_changed = true;
            }
        }
    }
    if ws.current_id.is_none() {
        let listed = |id: &String| ws.projects.iter().any(|p| &p.id == id);
        let from_backup = backup_ws.as_ref().and_then(|b| b.current_id.clone()).filter(listed);
        ws.current_id = from_backup.or_else(|| ws.projects.first().map(|p| p.id.clone()));
        ws_changed |= ws.current_id.is_some();
    }

    if ws_changed {
        writes.push(KvWrite::Set(WORKSPACE_KEY.into(), ws.to_json()));
    } else {
        report.workspace_created = false;
    }
    db.write_batch(&writes)?;
    Ok(report)
}
