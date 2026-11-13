//! Gravação em segundo plano com debounce (o `schedule(key, 400)` de `db.ts`).
//!
//! A UI chama [`Autosave::schedule`] a cada edição; nada toca o disco na thread
//! da UI. Uma thread de I/O espera o sossego de 400 ms (ou o teto de espera,
//! para edições contínuas não adiarem a gravação para sempre), serializa as
//! músicas e grava tudo numa transação. [`Autosave::flush`] grava já e espera;
//! soltar o `Autosave` faz flush e encerra a thread.

use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use wadawko_core::Song;

use crate::db::{Db, KvWrite};

/// Debounce do app web (`setTimeout(() => save(songJson), 400)`).
pub const AUTOSAVE_DEBOUNCE: Duration = Duration::from_millis(400);
/// Teto de espera desde a primeira edição pendente.
pub const AUTOSAVE_MAX_WAIT: Duration = Duration::from_secs(5);
/// Espera antes de tentar de novo depois de uma falha (os 5 s de `db.ts`).
pub const AUTOSAVE_RETRY: Duration = Duration::from_secs(5);

/// O que gravar numa chave.
#[derive(Clone, Debug)]
pub enum Payload {
    /// texto pronto
    Text(String),
    /// música; o `to_json()` roda na thread de I/O
    Song(Arc<Song>),
    /// remover a chave
    Delete,
}

struct State {
    pending: BTreeMap<String, Payload>,
    /// instante da edição pendente mais antiga
    first: Option<Instant>,
    /// quando gravar (última edição + debounce, ou a nova tentativa)
    due: Option<Instant>,
    flush: bool,
    shutdown: bool,
    writing: bool,
    last_error: Option<String>,
    /// tentativas de gravação (com ou sem sucesso)
    attempts: u64,
    writes: u64,
}

struct Shared {
    state: Mutex<State>,
    /// acorda a thread de I/O
    wake: Condvar,
    /// avisa quem espera um flush
    done: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Gravador com debounce sobre um [`Db`].
pub struct Autosave {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    debounce: Duration,
}

impl Autosave {
    /// Debounce de 400 ms e teto de 5 s.
    pub fn new(db: Db) -> Autosave {
        Self::with_timing(db, AUTOSAVE_DEBOUNCE, AUTOSAVE_MAX_WAIT)
    }

    /// Tempos explícitos (testes).
    pub fn with_timing(db: Db, debounce: Duration, max_wait: Duration) -> Autosave {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                pending: BTreeMap::new(),
                first: None,
                due: None,
                flush: false,
                shutdown: false,
                writing: false,
                last_error: None,
                attempts: 0,
                writes: 0,
            }),
            wake: Condvar::new(),
            done: Condvar::new(),
        });
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new().name("wadawko-autosave".into()).spawn(move || io_loop(&shared, &db, max_wait)).expect("criar a thread de autosave")
        };
        Autosave { shared, thread: Some(thread), debounce }
    }

    /// Agenda a gravação de uma chave; reagendar a mesma chave substitui o
    /// valor pendente e reinicia o debounce. Nunca bloqueia em I/O.
    pub fn schedule(&self, key: impl Into<String>, payload: Payload) {
        self.schedule_many([(key.into(), payload)]);
    }

    /// Agenda várias chaves que devem ser gravadas juntas (mesma transação).
    pub fn schedule_many(&self, items: impl IntoIterator<Item = (String, Payload)>) {
        let now = Instant::now();
        let mut st = self.shared.lock();
        for (k, p) in items {
            st.pending.insert(k, p);
        }
        if st.pending.is_empty() {
            return;
        }
        st.first.get_or_insert(now);
        st.due = Some(now + self.debounce);
        drop(st);
        self.shared.wake.notify_all();
    }

    /// Há gravação pendente ou em andamento?
    pub fn is_dirty(&self) -> bool {
        let st = self.shared.lock();
        !st.pending.is_empty() || st.writing
    }

    /// Erro da última tentativa de gravação (`None` depois de um sucesso).
    pub fn last_error(&self) -> Option<String> {
        self.shared.lock().last_error.clone()
    }

    /// Quantas transações já foram gravadas (diagnóstico/testes).
    pub fn write_count(&self) -> u64 {
        self.shared.lock().writes
    }

    /// Grava agora o que estiver pendente e espera terminar. Em caso de falha
    /// devolve o erro; o conteúdo continua pendente e será tentado de novo.
    pub fn flush(&self) -> Result<()> {
        let mut st = self.shared.lock();
        loop {
            if st.pending.is_empty() && !st.writing {
                return Ok(());
            }
            let before = st.attempts;
            st.flush = true;
            self.shared.wake.notify_all();
            st = self.shared.done.wait(st).unwrap_or_else(|e| e.into_inner());
            // só vale o erro de uma tentativa feita depois deste pedido
            if !st.writing && st.attempts != before && !st.pending.is_empty()
                && let Some(e) = &st.last_error {
                    return Err(anyhow!("falha ao gravar no banco: {e}"));
                }
            if self.thread.as_ref().is_none_or(|t| t.is_finished()) && !st.pending.is_empty() {
                return Err(anyhow!("a thread de autosave terminou com gravações pendentes"));
            }
        }
    }
}

impl Drop for Autosave {
    fn drop(&mut self) {
        {
            let mut st = self.shared.lock();
            st.shutdown = true;
        }
        self.shared.wake.notify_all();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn io_loop(shared: &Shared, db: &Db, max_wait: Duration) {
    let mut st = shared.lock();
    loop {
        if st.pending.is_empty() {
            st.flush = false;
            shared.done.notify_all();
            if st.shutdown {
                return;
            }
            st = shared.wake.wait(st).unwrap_or_else(|e| e.into_inner());
            continue;
        }
        let now = Instant::now();
        let mut due = st.due.unwrap_or(now);
        if let Some(first) = st.first {
            // depois de uma falha `first` é None: vale só a espera da nova tentativa
            due = due.min(first + max_wait);
        }
        if !st.flush && !st.shutdown && now < due {
            st = shared.wake.wait_timeout(st, due - now).unwrap_or_else(|e| e.into_inner()).0;
            continue;
        }

        let batch = std::mem::take(&mut st.pending);
        st.first = None;
        st.due = None;
        st.flush = false;
        st.writing = true;
        drop(st);

        // serialização e disco fora do lock: a UI continua agendando
        let writes: Vec<KvWrite> = batch
            .iter()
            .map(|(k, p)| match p {
                Payload::Text(s) => KvWrite::Set(k.clone(), s.clone()),
                Payload::Song(song) => KvWrite::Set(k.clone(), song.to_json()),
                Payload::Delete => KvWrite::Delete(k.clone()),
            })
            .collect();
        let result = db.write_batch(&writes);

        st = shared.lock();
        st.writing = false;
        st.attempts += 1;
        match result {
            Ok(()) => {
                st.writes += 1;
                st.last_error = None;
            }
            Err(e) => {
                st.last_error = Some(format!("{e:#}"));
                // devolve o lote, sem passar por cima do que chegou depois
                for (k, p) in batch {
                    st.pending.entry(k).or_insert(p);
                }
                if st.shutdown {
                    shared.done.notify_all();
                    return;
                }
                st.due = Some(Instant::now() + AUTOSAVE_RETRY);
            }
        }
        shared.done.notify_all();
    }
}
