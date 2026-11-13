//! Acesso direto às músicas do banco, sem estado em memória: o que o
//! `wadawko-cli` usa (e a base do [`crate::ProjectStore`]).
//!
//! Nada aqui cria a música de exemplo: um banco vazio é lido como uma
//! biblioteca vazia.

use anyhow::{Context as _, Result, anyhow, bail};
use wadawko_core::Song;
use wadawko_core::ids::{new_project_id, now_ms};
use wadawko_core::model::{PROJECT_KEY_PREFIX, ProjectMeta, WORKSPACE_KEY, Workspace, project_key};

use crate::db::{Db, KvWrite};

/// Título gravado no workspace para música sem título (`song.title || 'Sem título'`).
pub const UNTITLED: &str = "Sem título";

/// Título de uma música como aparece na lista de projetos.
pub fn display_title(song: &Song) -> String {
    if song.title.is_empty() { UNTITLED.to_string() } else { song.title.clone() }
}

pub(crate) fn now() -> i64 {
    now_ms() as i64
}

/// `readWorkspace`: `None` se a chave não existe ou não é um workspace `version: 2`.
pub fn read_workspace(db: &Db) -> Result<Option<Workspace>> {
    Ok(db.get(WORKSPACE_KEY)?.and_then(|raw| Workspace::from_json(&raw)))
}

/// Lê a música de um projeto, dizendo por que falhou (chave ausente, JSON
/// inválido, não é `version: 1`…).
pub fn read_song(db: &Db, id: &str) -> Result<Song> {
    let raw = db.get(&project_key(id))?.ok_or_else(|| anyhow!("projeto {id} não está no banco (chave {} ausente)", project_key(id)))?;
    Song::from_json(&raw).map_err(|e| anyhow!("projeto {id} ilegível: {e} ({})", e.detail()))
}

/// `readSong` do app web: `None` para qualquer falha.
pub fn try_read_song(db: &Db, id: &str) -> Option<Song> {
    read_song(db, id).ok()
}

/// Uma música do banco, para listagem.
#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub meta: ProjectMeta,
    /// é a música aberta (`currentId`)
    pub current: bool,
    /// está na lista do workspace (falso = chave `wadawko:project:*` órfã)
    pub listed: bool,
    /// tamanho do JSON gravado, em bytes (`None` se a chave não existe)
    pub bytes: Option<usize>,
    /// a música, ou o motivo de não ter sido lida
    pub song: Result<Song, String>,
}

/// Lista as músicas: primeiro as do workspace, na ordem dele; depois as chaves
/// `wadawko:project:*` que o workspace não lista (órfãs).
pub fn list_projects(db: &Db) -> Result<Vec<ProjectInfo>> {
    let ws = read_workspace(db)?.unwrap_or_default();
    let mut stored: std::collections::BTreeMap<String, crate::db::KvEntry> =
        db.list(PROJECT_KEY_PREFIX)?.into_iter().map(|e| (e.key[PROJECT_KEY_PREFIX.len()..].to_string(), e)).collect();
    let parse = |id: &str, raw: Option<&str>| -> Result<Song, String> {
        let raw = raw.ok_or_else(|| format!("chave {} ausente", project_key(id)))?;
        Song::from_json(raw).map_err(|e| format!("{e} ({})", e.detail()))
    };
    let mut out = Vec::new();
    for meta in &ws.projects {
        let entry = stored.remove(&meta.id);
        out.push(ProjectInfo {
            current: ws.current_id.as_deref() == Some(meta.id.as_str()),
            listed: true,
            bytes: entry.as_ref().map(|e| e.value.len()),
            song: parse(&meta.id, entry.as_ref().map(|e| e.value.as_str())),
            meta: meta.clone(),
        });
    }
    for (id, entry) in stored {
        let song = parse(&id, Some(&entry.value));
        let title = song.as_ref().map(display_title).unwrap_or_else(|_| "?".into());
        out.push(ProjectInfo {
            meta: ProjectMeta { id, title, updated_at: entry.updated_at, ..Default::default() },
            current: false,
            listed: false,
            bytes: Some(entry.value.len()),
            song,
        });
    }
    Ok(out)
}

/// Acha uma música por id exato, título exato (sem diferenciar maiúsculas) ou,
/// por último, trecho do título. Erro se nada casar ou se for ambíguo.
pub fn find_project(db: &Db, query: &str) -> Result<ProjectInfo> {
    let all = list_projects(db)?;
    if all.is_empty() {
        bail!("o banco não tem nenhuma música");
    }
    let q = query.to_lowercase();
    let by_id: Vec<&ProjectInfo> = all.iter().filter(|p| p.meta.id == query).collect();
    let exact: Vec<&ProjectInfo> = all.iter().filter(|p| p.meta.title.to_lowercase() == q).collect();
    let partial: Vec<&ProjectInfo> = all.iter().filter(|p| p.meta.title.to_lowercase().contains(&q)).collect();
    let hits = [by_id, exact, partial].into_iter().find(|v| !v.is_empty()).unwrap_or_default();
    match hits.as_slice() {
        [] => bail!("nenhuma música com id ou título \"{query}\""),
        [one] => Ok((*one).clone()),
        many => {
            let list: Vec<String> = many.iter().map(|p| format!("  {}  {}", p.meta.id, p.meta.title)).collect();
            bail!("\"{query}\" é ambíguo — use o id:\n{}", list.join("\n"))
        }
    }
}

/// Acrescenta uma música como projeto novo (id novo, fim da lista) e devolve o
/// id. Cria o workspace se não houver; `make_current` (ou workspace sem música
/// aberta) faz dela a música atual.
pub fn add_song(db: &Db, song: &Song, make_current: bool) -> Result<String> {
    let mut ws = read_workspace(db)?.unwrap_or_default();
    let id = unused_project_id(db, &ws)?;
    ws.projects.push(ProjectMeta { id: id.clone(), title: display_title(song), updated_at: now(), ..Default::default() });
    if make_current || ws.current_id.is_none() {
        ws.current_id = Some(id.clone());
    }
    db.write_batch(&[KvWrite::Set(project_key(&id), song.to_json()), KvWrite::Set(WORKSPACE_KEY.into(), ws.to_json())])
        .context("gravar a música nova")?;
    Ok(id)
}

/// Id de projeto que não colide com o workspace nem com chaves já gravadas
/// (o contador do id recomeça a cada processo; dois no mesmo milissegundo colidiriam).
pub(crate) fn unused_project_id(db: &Db, ws: &Workspace) -> Result<String> {
    loop {
        let id = new_project_id();
        if !ws.projects.iter().any(|p| p.id == id) && db.get(&project_key(&id))?.is_none() {
            return Ok(id);
        }
    }
}
