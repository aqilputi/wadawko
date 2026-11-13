//! Histórico de desfazer/refazer (porte do `past`/`future` de `App.svelte`).
//!
//! O histórico não guarda o estado atual — quem guarda é a UI. O protocolo é:
//!
//! ```
//! # use std::sync::Arc;
//! # use wadawko_store::History;
//! let mut song = Arc::new(1);            // na UI: Arc<Song>
//! let mut history = History::new();
//!
//! // edição: registra o estado ANTERIOR e troca o atual
//! history.record(song.clone());
//! song = Arc::new(2);
//!
//! // desfazer/refazer: entrega o atual, recebe o que deve passar a valer
//! if let Some(prev) = history.undo(song.clone()) { song = prev; }
//! assert_eq!(*song, 1);
//! if let Some(next) = history.redo(song.clone()) { song = next; }
//! assert_eq!(*song, 2);
//! ```
//!
//! Com `T = Arc<Song>` cada nível custa um ponteiro (as músicas são
//! compartilhadas), em vez de uma string JSON como no app web.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Níveis de desfazer do app web (`if (past.length > 100) past.shift()`).
pub const DEFAULT_HISTORY_LIMIT: usize = 100;
/// Janela padrão de coalescência de edições seguidas com a mesma etiqueta.
pub const DEFAULT_COALESCE_WINDOW: Duration = Duration::from_millis(800);

/// Pilhas de desfazer/refazer com limite de níveis.
#[derive(Debug, Clone)]
pub struct History<T> {
    past: VecDeque<T>,
    future: Vec<T>,
    limit: usize,
    window: Duration,
    /// etiqueta e instante da última edição coalescível
    last: Option<(u64, Instant)>,
    /// último `epoch` visto por [`History::sync_epoch`]
    epoch: Option<u64>,
}

impl<T> Default for History<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> History<T> {
    /// Histórico com 100 níveis.
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_HISTORY_LIMIT)
    }

    /// Histórico com `limit` níveis (mínimo 1).
    pub fn with_limit(limit: usize) -> Self {
        History { past: VecDeque::new(), future: Vec::new(), limit: limit.max(1), window: DEFAULT_COALESCE_WINDOW, last: None, epoch: None }
    }

    /// Muda a janela de coalescência de [`History::record_coalesced`].
    pub fn set_coalesce_window(&mut self, window: Duration) {
        self.window = window;
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Registra uma edição: `before` é o estado imediatamente ANTERIOR a ela.
    /// Descarta o que havia para refazer; além do limite, o nível mais antigo sai.
    pub fn record(&mut self, before: T) {
        self.last = None;
        self.push_past(before);
        self.future.clear();
    }

    /// Como [`History::record`], mas edições seguidas com a mesma `tag` dentro
    /// da janela (ex.: arrastar um fader, digitar o título) viram um único
    /// nível: só a primeira guarda o `before`. Devolve `true` se criou um nível.
    pub fn record_coalesced(&mut self, before: T, tag: u64) -> bool {
        self.record_coalesced_at(before, tag, Instant::now())
    }

    /// [`History::record_coalesced`] com relógio explícito (testes).
    pub fn record_coalesced_at(&mut self, before: T, tag: u64, now: Instant) -> bool {
        let merge = matches!(self.last, Some((t, at)) if t == tag && now.saturating_duration_since(at) <= self.window) && !self.past.is_empty();
        self.last = Some((tag, now));
        if merge {
            // a edição anterior do grupo já guardou o estado de antes
            self.future.clear();
            return false;
        }
        self.push_past(before);
        self.future.clear();
        true
    }

    /// Encerra o grupo de coalescência em andamento (ex.: soltou o fader): a
    /// próxima edição cria um nível novo mesmo com a mesma etiqueta.
    pub fn seal(&mut self) {
        self.last = None;
    }

    fn push_past(&mut self, state: T) {
        self.past.push_back(state);
        while self.past.len() > self.limit {
            self.past.pop_front();
        }
    }

    /// Desfaz: recebe o estado atual (vai para a pilha de refazer) e devolve o
    /// anterior. `None` (e `current` é descartado) se não há o que desfazer.
    pub fn undo(&mut self, current: T) -> Option<T> {
        let prev = self.past.pop_back()?;
        self.last = None;
        self.future.push(current);
        Some(prev)
    }

    /// Refaz: recebe o estado atual (volta para a pilha de desfazer) e devolve
    /// o seguinte. `None` se não há o que refazer.
    pub fn redo(&mut self, current: T) -> Option<T> {
        let next = self.future.pop()?;
        self.last = None;
        self.push_past(current);
        Some(next)
    }

    /// Zera tudo (troca de música).
    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
        self.last = None;
    }

    /// Zera o histórico quando `epoch` (ver `ProjectStore::epoch`) mudou desde a
    /// última chamada — o `projectEpoch` do app web. Devolve `true` se zerou.
    pub fn sync_epoch(&mut self, epoch: u64) -> bool {
        let changed = self.epoch.is_some_and(|e| e != epoch);
        self.epoch = Some(epoch);
        if changed {
            self.clear();
        }
        changed
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Quantos níveis há para desfazer.
    pub fn undo_len(&self) -> usize {
        self.past.len()
    }

    /// Quantos níveis há para refazer.
    pub fn redo_len(&self) -> usize {
        self.future.len()
    }
}
