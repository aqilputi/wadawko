//! Geração de ids no formato do app web: `<prefixo>-<Date.now() em base 36>-<contador>`
//! (`sec-muogk8wy-1`, `person-…`, `inst-…`, `p-…`, `<base>-…` para linhas).
//!
//! O contador é por tipo e por processo (como os `let xCounter = 0` do store).
//! Para testes determinísticos, `set_deterministic(Some(ms))` fixa o relógio e
//! zera os contadores NA THREAD atual.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Tipo de id (cada um tem o seu contador, como no TS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdKind {
    Project,
    Section,
    Track,
    Person,
    Instrument,
}

const KINDS: usize = 5;
static COUNTERS: [AtomicU64; KINDS] = [const { AtomicU64::new(0) }; KINDS];

thread_local! {
    static FIXED: RefCell<Option<(u64, [u64; KINDS])>> = const { RefCell::new(None) };
}

/// `Some(ms)`: nesta thread, ids passam a usar `ms` como relógio e contadores
/// próprios começando em 1. `None` volta ao relógio real.
pub fn set_deterministic(now_ms: Option<u64>) {
    FIXED.with(|f| *f.borrow_mut() = now_ms.map(|ms| (ms, [0; KINDS])));
}

/// Número em base 36 minúscula (`Number.prototype.toString(36)`).
pub fn base36(mut n: u64) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".into();
    }
    let mut buf = Vec::new();
    while n > 0 {
        buf.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    buf.reverse();
    String::from_utf8(buf).expect("ascii")
}

/// Milissegundos desde a época (`Date.now()`).
pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Novo id `<prefix>-<tempo36>-<n>`.
pub fn new_id(kind: IdKind, prefix: &str) -> String {
    let fixed = FIXED.with(|f| {
        f.borrow_mut().as_mut().map(|(ms, counters)| {
            counters[kind as usize] += 1;
            (*ms, counters[kind as usize])
        })
    });
    let (ms, n) = fixed.unwrap_or_else(|| (now_ms(), COUNTERS[kind as usize].fetch_add(1, Ordering::Relaxed) + 1));
    format!("{prefix}-{}-{n}", base36(ms))
}

/// id de projeto (`p-…`), para o workspace.
pub fn new_project_id() -> String {
    new_id(IdKind::Project, "p")
}
