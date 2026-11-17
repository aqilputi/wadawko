//! Keymap em JSON no formato do Zed: padrão embutido + `~/.config/wadawko/keymap.json`
//! (recarregado sozinho pelo Shell quando o arquivo muda) e o verificador de conflitos.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::{Result, anyhow};
use gpui_kit::{Action, App, KeyBinding, KeyBindingContextPredicate};

// ───────────────────────────── keymap ─────────────────────────────

pub const DEFAULT_KEYMAP: &str = include_str!("../../assets/keymap-default.json");

pub fn user_keymap_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("wadawko").join("keymap.json")
}

fn parse_keymap(src: &str, origin: &str, cx: &App, errors: &mut Vec<String>) -> Vec<KeyBinding> {
    let json: serde_json::Value = match serde_json::from_str(src) {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!("{origin}: JSON inválido ({e})"));
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for block in json.as_array().into_iter().flatten() {
        let ctx = block["context"].as_str();
        let pred = match ctx.map(KeyBindingContextPredicate::parse) {
            None => None,
            Some(Ok(p)) => Some(Rc::new(p)),
            Some(Err(e)) => {
                errors.push(format!("{origin}: contexto inválido {ctx:?} ({e})"));
                continue;
            }
        };
        for (keys, action) in block["bindings"].as_object().into_iter().flatten() {
            let built: Result<Box<dyn Action>> = match action {
                serde_json::Value::String(name) => cx.build_action(name, None).map_err(|e| anyhow!("{e}")),
                serde_json::Value::Null => cx.build_action("zed::NoAction", None).map_err(|e| anyhow!("{e}")),
                serde_json::Value::Array(a) if a.len() == 2 => cx.build_action(a[0].as_str().unwrap_or_default(), Some(a[1].clone())).map_err(|e| anyhow!("{e}")),
                _ => Err(anyhow!("valor inesperado")),
            };
            match built {
                Ok(a) => match KeyBinding::load(keys, a, pred.clone(), false, None, cx) {
                    Ok(b) => out.push(b),
                    Err(e) => errors.push(format!("{origin}: tecla inválida {keys:?} ({e})")),
                },
                Err(e) => errors.push(format!("{origin}: {keys:?} → {action}: {e}")),
            }
        }
    }
    out
}

/// (Re)carrega padrão + usuário. Devolve erros legíveis (o resto continua valendo).
pub fn load_keymap(cx: &mut App) -> Vec<String> {
    let mut errors = Vec::new();
    let mut bindings = parse_keymap(DEFAULT_KEYMAP, "keymap padrão", cx, &mut errors);
    let path = user_keymap_path();
    if let Ok(src) = std::fs::read_to_string(&path) {
        bindings.extend(parse_keymap(&src, &path.display().to_string(), cx, &mut errors));
    }
    // gpui-component registra os seus (inputs, dialogs) no init: guardamos esse
    // conjunto na primeira carga e o recolocamos em cada reload (Keymap só tem clear)
    let base = BASE_BINDINGS.with(|b| {
        let mut b = b.borrow_mut();
        if b.is_none() {
            *b = Some(cx.key_bindings().borrow().bindings().cloned().collect::<Vec<_>>());
        }
        b.clone().unwrap_or_default()
    });
    cx.clear_key_bindings();
    cx.bind_keys(base);
    cx.bind_keys(bindings);
    errors
}

thread_local! {
    static BASE_BINDINGS: RefCell<Option<Vec<KeyBinding>>> = const { RefCell::new(None) };
}

/// Texto do atalho de maior precedência para uma ação (qualquer contexto).
pub fn binding_text(action: &dyn Action, cx: &App) -> Option<String> {
    let km = cx.key_bindings();
    let km = km.borrow();
    let b = km.bindings_for_action(action).last()?.clone();
    Some(b.keystrokes().iter().map(|k| k.display(cx.platform_info().os).to_string()).collect::<Vec<_>>().join(" "))
}

// ───────────────────────────── conflitos ─────────────────────────────

/// Um binding lido de um arquivo de keymap (sem construir ações).
pub struct RawBinding {
    pub origin: String,
    pub context: String,
    pub keys: String,
    pub action: String,
}

pub fn raw_bindings(src: &str, origin: &str) -> Vec<RawBinding> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(src) else { return Vec::new() };
    let mut out = Vec::new();
    for block in json.as_array().into_iter().flatten() {
        let context = block["context"].as_str().unwrap_or("(global)").to_string();
        for (keys, a) in block["bindings"].as_object().into_iter().flatten() {
            let action = match a {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Null => "(desligado)".into(),
                other => other.to_string(),
            };
            let keys = keys.split_whitespace().map(|k| k.to_ascii_lowercase()).collect::<Vec<_>>().join(" ");
            out.push(RawBinding { origin: origin.to_string(), context: context.clone(), keys, action });
        }
    }
    out
}

/// Padrão + usuário, na ordem de carga.
pub fn all_raw_bindings() -> Vec<RawBinding> {
    let mut v = raw_bindings(DEFAULT_KEYMAP, "padrão");
    if let Ok(src) = std::fs::read_to_string(user_keymap_path()) {
        v.extend(raw_bindings(&src, "usuário"));
    }
    v
}

/// Avisos: mesma tecla + mesmo contexto com ações diferentes (a última vence), e
/// tecla que também é começo de sequência no mesmo contexto (só dispara após 1 s).
pub fn conflicts(all: &[RawBinding]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            if a.context != b.context {
                continue;
            }
            if a.keys == b.keys && a.action != b.action {
                out.push(format!("“{}” em [{}]: {} ({}) é sobrescrito por {} ({})", a.keys, a.context, a.action, a.origin, b.action, b.origin));
            }
            for (short, long) in [(a, b), (b, a)] {
                if long.keys.starts_with(&format!("{} ", short.keys)) && short.action != "(desligado)" {
                    out.push(format!("“{}” ({}) também começa “{}” em [{}]: só dispara depois de 1 s", short.keys, short.action, long.keys, short.context));
                }
            }
        }
    }
    out
}
