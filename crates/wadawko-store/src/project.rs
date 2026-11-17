//! Workspace + música atual em memória (a parte de projetos de
//! `store.svelte.ts`): bootstrap, trocar, nova, duplicar, renomear, excluir,
//! salvar, tema e settings do app nativo.
//!
//! A UI é dona de um `ProjectStore` na sua thread. Edições entram por
//! [`ProjectStore::update_song`], que troca o `Arc<Song>` em memória e agenda a
//! gravação (debounce de 400 ms, thread de I/O). As operações de projeto gravam
//! antes o que estiver pendente — o `save()` que o TS chama antes de trocar.

use std::sync::Arc;

use anyhow::{Context as _, Result, anyhow, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use wadawko_core::Song;
use wadawko_core::demo::demo_song;
use wadawko_core::model::{ProjectMeta, THEME_KEY, WORKSPACE_KEY, Workspace, project_key};

use crate::autosave::{Autosave, Payload};
use crate::db::{Db, KvWrite};
use crate::library::{display_title, now, read_song, read_workspace, try_read_song, unused_project_id};

/// Chave das settings do app nativo (um objeto JSON). O app web não a conhece
/// nem a toca; fica fora das chaves `workspace`/`project:*`/`theme`.
pub const NATIVE_SETTINGS_KEY: &str = "wadawko:native:settings";

/// Tema (`wadawko:theme`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    pub fn parse(s: &str) -> Option<Theme> {
        match s {
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            _ => None,
        }
    }

    pub fn toggled(self) -> Theme {
        match self {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        }
    }
}

/// Workspace e música aberta, com gravação automática.
pub struct ProjectStore {
    // ordem dos campos = ordem de drop: o autosave grava o pendente e encerra a
    // thread antes de o banco fechar (e fazer o checkpoint do WAL)
    autosave: Option<Autosave>,
    db: Db,
    workspace: Workspace,
    song: Arc<Song>,
    epoch: u64,
}

impl std::fmt::Debug for ProjectStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectStore").field("db", &self.db).field("current_id", &self.workspace.current_id).field("epoch", &self.epoch).finish()
    }
}

impl ProjectStore {
    /// Abre o banco em `path` (leitura/escrita) e carrega o workspace.
    pub fn open_path(path: impl AsRef<std::path::Path>) -> Result<ProjectStore> {
        ProjectStore::open(Db::open(path)?)
    }

    /// Carrega o workspace e a música atual (`bootstrap` do TS).
    ///
    /// - workspace válido com música atual legível: usa como está, sem gravar nada;
    /// - banco vazio: cria o projeto com a música de exemplo e grava;
    /// - workspace existente cuja música atual não lê: abre a primeira música
    ///   legível da lista ou, se nenhuma ler, acrescenta a de exemplo — a lista
    ///   é preservada (o TS recriava o workspace só com a demo, perdendo as
    ///   referências às outras músicas).
    ///
    /// Com banco somente leitura nada é gravado: o estado vale só em memória e
    /// as operações que escrevem devolvem erro.
    pub fn open(db: Db) -> Result<ProjectStore> {
        let read_only = db.is_read_only();
        let existing = read_workspace(&db)?;
        let mut changed = false;
        let mut new_song: Option<(String, Song)> = None;

        let (workspace, song) = 'boot: {
            let mut ws = existing.unwrap_or_default();
            if let Some(id) = ws.current_id.clone()
                && let Some(song) = try_read_song(&db, &id)
            {
                break 'boot (ws, song);
            }
            let readable = ws.projects.iter().find_map(|p| try_read_song(&db, &p.id).map(|s| (p.id.clone(), s)));
            if let Some((id, song)) = readable {
                ws.current_id = Some(id);
                changed = true;
                break 'boot (ws, song);
            }
            let song = demo_song();
            let id = unused_project_id(&db, &ws)?;
            ws.projects.push(ProjectMeta { id: id.clone(), title: display_title(&song), updated_at: now(), ..Default::default() });
            ws.current_id = Some(id.clone());
            changed = true;
            new_song = Some((id, song.clone()));
            (ws, song)
        };

        if changed && !read_only {
            let mut writes = Vec::new();
            if let Some((id, song)) = &new_song {
                writes.push(KvWrite::Set(project_key(id), song.to_json()));
            }
            writes.push(KvWrite::Set(WORKSPACE_KEY.into(), workspace.to_json()));
            db.write_batch(&writes).context("gravar o workspace inicial")?;
        }

        let autosave = (!read_only).then(|| Autosave::new(db.clone()));
        Ok(ProjectStore { autosave, db, workspace, song: Arc::new(song), epoch: 0 })
    }

    // ───────────── leitura ─────────────

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub fn is_read_only(&self) -> bool {
        self.db.is_read_only()
    }

    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// Lista de músicas, na ordem do workspace.
    pub fn projects(&self) -> &[ProjectMeta] {
        &self.workspace.projects
    }

    /// Id da música aberta.
    pub fn current_id(&self) -> &str {
        self.workspace.current_id.as_deref().unwrap_or_default()
    }

    /// Entrada do workspace da música aberta.
    pub fn current_meta(&self) -> Option<&ProjectMeta> {
        let id = self.current_id();
        self.workspace.projects.iter().find(|p| p.id == id)
    }

    /// A música aberta. Clonar o `Arc` é o snapshot para o histórico/áudio.
    pub fn song(&self) -> &Arc<Song> {
        &self.song
    }

    /// Conta trocas de música (trocar, nova, duplicar, excluir a atual): o
    /// `projectEpoch` do TS. Quando muda, a UI zera o histórico
    /// ([`crate::History::sync_epoch`]) e a seleção/cursor.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Lê do banco a música de qualquer projeto (para a atual, prefira [`Self::song`]).
    pub fn read_song(&self, id: &str) -> Result<Song> {
        if id == self.current_id() {
            return Ok((*self.song).clone());
        }
        read_song(&self.db, id)
    }

    // ───────────── edição e gravação ─────────────

    /// Chamado pela UI a cada edição (e a cada desfazer/refazer): troca a música
    /// em memória, atualiza título/`updatedAt` na lista e agenda a gravação.
    /// Não bloqueia em I/O. Em banco somente leitura só troca em memória.
    pub fn update_song(&mut self, song: Arc<Song>) {
        self.song = song;
        self.touch_current();
        self.schedule_current();
    }

    /// `save()` do TS: grava a música atual e o workspace agora, esperando a escrita.
    pub fn save(&mut self) -> Result<()> {
        self.ensure_writable()?;
        self.touch_current();
        self.schedule_current();
        self.flush()
    }

    /// Grava agora o que o autosave tiver pendente (sem carimbar nada). Chame
    /// ao encerrar o app; soltar o `ProjectStore` também faz isso, mas sem
    /// devolver o erro.
    pub fn flush(&self) -> Result<()> {
        match &self.autosave {
            Some(a) => a.flush(),
            None => Ok(()),
        }
    }

    /// Traz para este banco as músicas de **outro banco** (`.db` do app web ou de
    /// outra instalação, aberto só para leitura) ou de um **backup JSON** do
    /// localStorage: músicas novas entram; as que já existem ficam como estão
    /// (mesma fusão de [`crate::import_backup`]). A música aberta não muda.
    pub fn import_library(&mut self, path: impl AsRef<std::path::Path>) -> Result<crate::BackupReport> {
        let path = path.as_ref();
        self.ensure_writable()?;
        // o autosave pendente gravaria o workspace antigo por cima do importado
        self.flush()?;
        let mut head = [0u8; 16];
        let is_sqlite = std::fs::File::open(path).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head)).is_ok() && &head == b"SQLite format 3\0";
        let report = if is_sqlite {
            let src = Db::open_read_only(path)?;
            let dict: Map<String, Value> = src.all()?.into_iter().map(|(k, v)| (k, Value::String(v))).collect();
            crate::import_backup(&self.db, &Value::Object(dict).to_string(), crate::BackupOptions::default())
                .with_context(|| format!("importar {}", path.display()))?
        } else {
            crate::import_backup_file(&self.db, path, crate::BackupOptions::default())?
        };
        if let Some(mut ws) = read_workspace(&self.db)? {
            ws.current_id = self.workspace.current_id.clone();
            self.workspace = ws;
        }
        Ok(report)
    }

    /// Há edição ainda não gravada?
    pub fn has_unsaved_changes(&self) -> bool {
        self.autosave.as_ref().is_some_and(Autosave::is_dirty)
    }

    /// Erro da última gravação automática, para a UI avisar (`None` = tudo certo).
    pub fn autosave_error(&self) -> Option<String> {
        self.autosave.as_ref().and_then(Autosave::last_error)
    }

    fn ensure_writable(&self) -> Result<()> {
        if self.db.is_read_only() {
            bail!("banco aberto em modo somente leitura");
        }
        Ok(())
    }

    /// título e `updatedAt` da música atual na lista
    fn touch_current(&mut self) {
        let title = display_title(&self.song);
        if let Some(id) = self.workspace.current_id.clone()
            && let Some(meta) = self.workspace.projects.iter_mut().find(|p| p.id == id)
        {
            meta.title = title;
            meta.updated_at = now();
        }
    }

    fn schedule_current(&self) {
        let (Some(autosave), Some(id)) = (&self.autosave, &self.workspace.current_id) else { return };
        autosave.schedule_many([(project_key(id), Payload::Song(self.song.clone())), (WORKSPACE_KEY.to_string(), Payload::Text(self.workspace.to_json()))]);
    }

    fn write_workspace_with(&self, mut writes: Vec<KvWrite>) -> Result<()> {
        writes.push(KvWrite::Set(WORKSPACE_KEY.into(), self.workspace.to_json()));
        self.db.write_batch(&writes)
    }

    fn opened(&mut self, id: String, song: Song) {
        self.workspace.current_id = Some(id);
        self.song = Arc::new(song);
        self.epoch += 1;
    }

    // ───────────── projetos ─────────────

    /// Abre outra música (`switchProject`). `Ok(false)` se já era a atual;
    /// erro (sem mudar nada) se a música não existe ou não lê.
    pub fn switch_project(&mut self, id: &str) -> Result<bool> {
        if id == self.current_id() {
            return Ok(false);
        }
        self.ensure_writable()?;
        if !self.workspace.projects.iter().any(|p| p.id == id) {
            bail!("projeto {id} não está no workspace");
        }
        self.flush()?;
        let song = read_song(&self.db, id)?;
        self.opened(id.to_string(), song);
        self.write_workspace_with(Vec::new())?;
        Ok(true)
    }

    /// Cria uma música nova (a de exemplo) e a abre (`newProject`). Devolve o id.
    pub fn new_project(&mut self) -> Result<String> {
        self.add_project(demo_song())
    }

    /// Acrescenta `song` como projeto novo e o abre (importar arquivo como
    /// música nova). Devolve o id.
    pub fn add_project(&mut self, song: Song) -> Result<String> {
        self.ensure_writable()?;
        self.flush()?;
        let id = unused_project_id(&self.db, &self.workspace)?;
        self.workspace.projects.push(ProjectMeta { id: id.clone(), title: display_title(&song), updated_at: now(), ..Default::default() });
        let json = song.to_json();
        self.opened(id.clone(), song);
        self.write_workspace_with(vec![KvWrite::Set(project_key(&id), json)])?;
        Ok(id)
    }

    /// Duplica uma música como "<título> cópia" e abre a cópia
    /// (`duplicateProject`). Devolve o id novo.
    pub fn duplicate_project(&mut self, id: &str) -> Result<String> {
        self.ensure_writable()?;
        let mut copy = self.read_song(id)?;
        copy.title = format!("{} cópia", display_title(&copy));
        self.add_project(copy)
    }

    /// Renomeia uma música: muda o `title` dela (no app web o nome do projeto
    /// é o título da música) e a entrada na lista. Título vazio aparece como
    /// "Sem título".
    pub fn rename_project(&mut self, id: &str, title: &str) -> Result<()> {
        self.ensure_writable()?;
        if id == self.current_id() {
            let mut song = (*self.song).clone();
            song.title = title.to_string();
            self.song = Arc::new(song);
            return self.save();
        }
        let mut song = read_song(&self.db, id)?;
        song.title = title.to_string();
        let meta = self.workspace.projects.iter_mut().find(|p| p.id == id).ok_or_else(|| anyhow!("projeto {id} não está no workspace"))?;
        meta.title = display_title(&song);
        meta.updated_at = now();
        self.flush()?;
        self.write_workspace_with(vec![KvWrite::Set(project_key(id), song.to_json())])
    }

    /// Exclui uma música (`deleteProject`). Se era a aberta, abre a primeira da
    /// lista; se a lista ficou vazia, cria uma nova com a música de exemplo.
    pub fn delete_project(&mut self, id: &str) -> Result<()> {
        self.ensure_writable()?;
        if !self.workspace.projects.iter().any(|p| p.id == id) {
            bail!("projeto {id} não está no workspace");
        }
        // grava o pendente antes: senão o autosave recriaria a chave removida
        self.flush()?;
        let mut writes = vec![KvWrite::Delete(project_key(id))];
        self.workspace.projects.retain(|p| p.id != id);
        if self.workspace.current_id.as_deref() == Some(id) {
            if let Some(next) = self.workspace.projects.first().map(|p| p.id.clone()) {
                // como no TS: se a próxima não lê, a de exemplo entra no lugar (em memória)
                let song = try_read_song(&self.db, &next).unwrap_or_else(demo_song);
                self.opened(next, song);
            } else {
                let song = demo_song();
                let nid = unused_project_id(&self.db, &self.workspace)?;
                self.workspace.projects.push(ProjectMeta { id: nid.clone(), title: display_title(&song), updated_at: now(), ..Default::default() });
                writes.push(KvWrite::Set(project_key(&nid), song.to_json()));
                self.opened(nid, song);
            }
        }
        self.write_workspace_with(writes)
    }

    // ───────────── tema e settings ─────────────

    /// Tema gravado (`None` = nunca escolhido: seguir o sistema).
    pub fn theme(&self) -> Result<Option<Theme>> {
        Ok(self.db.get(THEME_KEY)?.as_deref().and_then(Theme::parse))
    }

    pub fn set_theme(&self, theme: Theme) -> Result<()> {
        self.db.set(THEME_KEY, theme.as_str())
    }

    /// Todas as settings do app nativo (objeto JSON em [`NATIVE_SETTINGS_KEY`]).
    pub fn settings(&self) -> Result<Map<String, Value>> {
        read_settings(&self.db)
    }

    /// Uma setting, desserializada (`None` se ausente ou de outro tipo).
    pub fn setting<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>> {
        Ok(self.settings()?.remove(name).and_then(|v| serde_json::from_value(v).ok()))
    }

    /// Grava uma setting (as outras ficam como estão).
    pub fn set_setting<T: Serialize>(&self, name: &str, value: &T) -> Result<()> {
        let mut map = self.settings()?;
        map.insert(name.to_string(), serde_json::to_value(value)?);
        self.db.set(NATIVE_SETTINGS_KEY, &Value::Object(map).to_string())
    }

    /// Remove uma setting; devolve se existia.
    pub fn remove_setting(&self, name: &str) -> Result<bool> {
        let mut map = self.settings()?;
        let had = map.remove(name).is_some();
        if had {
            self.db.set(NATIVE_SETTINGS_KEY, &Value::Object(map).to_string())?;
        }
        Ok(had)
    }
}

fn read_settings(db: &Db) -> Result<Map<String, Value>> {
    Ok(match db.get(NATIVE_SETTINGS_KEY)?.map(|raw| serde_json::from_str::<Value>(&raw)) {
        Some(Ok(Value::Object(map))) => map,
        _ => Map::new(),
    })
}
