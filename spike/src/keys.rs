//! Teclado (PLANEJAMENTO-teclado.md, K0–K3): ações GPUI, registro único de
//! comandos (paleta / which-key / hints / docs), keymap em JSON (padrão
//! embutido + ~/.config/wadawko/keymap.json), fuzzy, rótulos de salto e o
//! registro de alvos dos hints.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::{Context as _, Result, anyhow};
use gpui_kit::{Action, App, Bounds, KeyBinding, KeyBindingContextPredicate, Pixels, SharedString, Window};

// ───────────────────────────── ações ─────────────────────────────

pub mod transport {
    use gpui_kit::actions;
    actions!(
    transport,
    [TogglePlay, PlayHere, PlayFromStart, Stop, ToggleLoop, ToggleMetronome, ToggleCountIn, ToggleTrainer, ToggleSwing, Straight, SwingOn, BpmUp, BpmDown]
);
}
pub mod view {
    use gpui_kit::actions;
    actions!(view, [Grade, Partitura, Esteira, ToggleTheme, ZoomIn, ZoomOut, ZoomReset, ToggleNav]);
}
pub mod nav {
    use gpui_kit::actions;
    actions!(nav, [Editor, Pessoas, Mixer, Kit, Config]);
}
pub mod keys {
    use gpui_kit::actions;
    actions!(keys, [TogglePalette, ShowShortcuts, ReloadKeymap, ToggleVim, Cancel, ToggleRootHints, CreateKeymap]);
}
pub mod hints {
    use gpui_kit::actions;
    actions!(hints, [Jump, JumpTrack, JumpCells]);
}
pub mod project {
    use gpui_kit::actions;
    actions!(project, [Undo, Redo, New, Duplicate, NextSong, PrevSong, ExportJson, ImportJson, ExportMp3, ExportWav]);
}
pub mod section {
    use gpui_kit::actions;
    actions!(section, [Add, Clone, Remove, RepeatUp, RepeatDown, MeasureAdd, MeasureRemove, ToggleCollapse, ExpandAll, CollapseAll]);
}
pub mod track {
    use gpui_kit::actions;
    actions!(track, [Add, Remove, ToggleMute, ToggleSolo, CycleInstrument]);
}
pub mod song {
    use gpui_kit::actions;
    actions!(song, [Sub2, Sub4, Sub8]);
}
pub mod measure {
    use gpui_kit::actions;
    actions!(measure, [CycleDynamic]);
}
pub mod grid {
    use gpui_kit::actions;
    actions!(
    grid,
    [
        Left, Right, Up, Down, ExtendLeft, ExtendRight, ExtendUp, ExtendDown, NextBeat, PrevBeat, EndOfBeat, NextMeasure,
        PrevMeasure, MeasureStart, MeasureEnd, FirstFilled, SongStart, SongEnd, NextSection, PrevSection, FirstTrack,
        LastTrack, HalfRight, HalfLeft, DeleteCell, DeleteCellBack, DeleteMeasureRow, DeleteBeat, DeleteToMeasureEnd,
        DeleteMeasureAll, DeleteSectionRow, YankMeasureRow, YankBeat, YankSectionRow, PasteAfter, PasteBefore, Copy, Cut,
        Paste, DeleteSelection, ToggleSfz, Accent, Unaccent, ToggleSelectMode, ModeEdit, ModeSelect
    ]
);
}
pub mod vim {
    use gpui_kit::actions;
    actions!(
    vim,
    [
        Insert, Append, InsertMeasureStart, AppendMeasureEnd, Substitute, Visual, VisualMeasure, Normal, ChangeMeasureRow,
        ChangeBeat, ChangeSelection, SwapAnchor, Repeat, PushRegister, SetMark, GotoMark, Search, SearchBack,
        SearchNext, SearchPrev, CommandLine, Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8,
        Digit9
    ]
);
}

// ───────────────────────────── registro (fonte única) ─────────────────────────────

pub struct Spec {
    /// nome da ação GPUI ("transport::TogglePlay")
    pub id: &'static str,
    pub title: &'static str,
    pub group: &'static str,
    pub keywords: &'static str,
}

macro_rules! spec {
    ($id:literal, $title:literal, $group:literal, $kw:literal) => {
        Spec { id: $id, title: $title, group: $group, keywords: $kw }
    };
}

/// Comandos visíveis na paleta (ordem = ordem de exibição sem busca).
pub static SPECS: &[Spec] = &[
    spec!("transport::TogglePlay", "Tocar / parar", "Transporte", "play pause stop espaço"),
    spec!("transport::PlayHere", "Tocar do cursor", "Transporte", "play daqui"),
    spec!("transport::PlayFromStart", "Tocar do início", "Transporte", "play começo"),
    spec!("transport::Stop", "Parar", "Transporte", "stop"),
    spec!("transport::ToggleLoop", "Loop: ligar/desligar", "Transporte", "repetir"),
    spec!("transport::ToggleMetronome", "Metrônomo: ligar/desligar", "Transporte", "click"),
    spec!("transport::ToggleCountIn", "Count-in: ligar/desligar", "Transporte", "contagem"),
    spec!("transport::ToggleTrainer", "Treino (rampa de tempo): ligar/desligar", "Transporte", "trainer acelerar"),
    spec!("transport::ToggleSwing", "Reto / swing", "Transporte", "hane shuffle"),
    spec!("transport::BpmUp", "BPM +1", "Transporte", "tempo andamento"),
    spec!("transport::BpmDown", "BPM −1", "Transporte", "tempo andamento"),
    spec!("view::Grade", "Ver: grade", "Ver", "grid"),
    spec!("view::Partitura", "Ver: partitura", "Ver", "score pauta"),
    spec!("view::Esteira", "Ver: esteira", "Ver", "lane taiko"),
    spec!("view::ToggleTheme", "Tema: claro/escuro", "Ver", "dark light"),
    spec!("view::ZoomIn", "Zoom +", "Ver", "aproximar"),
    spec!("view::ZoomOut", "Zoom −", "Ver", "afastar"),
    spec!("view::ZoomReset", "Zoom 100%", "Ver", "resetar"),
    spec!("view::ToggleNav", "Barra lateral: mostrar/esconder", "Ver", "sidebar menu"),
    spec!("nav::Editor", "Ir para Editor", "Navegar", "página"),
    spec!("nav::Pessoas", "Ir para Pessoas", "Navegar", "página people"),
    spec!("nav::Mixer", "Ir para Mixer", "Navegar", "página volume"),
    spec!("nav::Kit", "Ir para Kit", "Navegar", "página instrumentos"),
    spec!("hints::Jump", "Saltar para um elemento", "Navegar", "hint jump rótulo"),
    spec!("hints::JumpTrack", "Saltar para célula da linha", "Navegar", "hint jump"),
    spec!("hints::JumpCells", "Saltar para qualquer célula", "Navegar", "hint jump"),
    spec!("keys::TogglePalette", "Paleta de comandos", "Teclado", "command palette ctrl k"),
    spec!("keys::ToggleRootHints", "Dica de atalhos (o que posso apertar aqui?)", "Teclado", "which key cheat sheet ctrl /"),
    spec!("nav::Config", "Configurações e mapa de atalhos", "Navegar", "config settings atalhos keymap"),
    spec!("keys::CreateKeymap", "Criar ~/.config/wadawko/keymap.json", "Teclado", "personalizar atalhos"),
    spec!("vim::CommandLine", "Linha de comando (:)", "Teclado", "ex comando"),
    spec!("vim::Search", "Buscar sílaba (/)", "Navegar", "procurar find"),
    spec!("vim::SearchNext", "Próxima ocorrência (n)", "Navegar", "busca"),
    spec!("vim::SearchPrev", "Ocorrência anterior (N)", "Navegar", "busca"),
    spec!("vim::Repeat", "Repetir última mudança (.)", "Edição", "dot repeat"),
    spec!("keys::ToggleVim", "Vim: ligar/desligar", "Teclado", "modal hjkl"),
    spec!("keys::ShowShortcuts", "Mapa de atalhos", "Teclado", "ajuda help keymap"),
    spec!("keys::ReloadKeymap", "Recarregar atalhos (keymap.json)", "Teclado", "reload"),
    spec!("project::Undo", "Desfazer", "Edição", "undo"),
    spec!("project::Redo", "Refazer", "Edição", "redo"),
    spec!("grid::Copy", "Copiar seleção", "Edição", "yank"),
    spec!("grid::Cut", "Recortar seleção", "Edição", ""),
    spec!("grid::Paste", "Colar", "Edição", "put"),
    spec!("grid::DeleteSelection", "Apagar seleção", "Edição", "limpar"),
    spec!("grid::ToggleSfz", "Alternar sforzando (!)", "Edição", "acento sfz"),
    spec!("grid::Accent", "Acentuar (MAIÚSCULA)", "Edição", "upper"),
    spec!("grid::Unaccent", "Desacentuar (minúscula)", "Edição", "lower"),
    spec!("grid::ToggleSelectMode", "Modo: editar / selecionar", "Edição", "seleção"),
    spec!("section::Add", "+ Seção", "Estrutura", "nova seção"),
    spec!("section::Clone", "Clonar seção", "Estrutura", "duplicar"),
    spec!("section::Remove", "Remover seção", "Estrutura", "apagar"),
    spec!("section::RepeatUp", "Ritornello +", "Estrutura", "repetir"),
    spec!("section::RepeatDown", "Ritornello −", "Estrutura", "repetir"),
    spec!("section::MeasureAdd", "+ Compasso (seção do cursor)", "Estrutura", "measure"),
    spec!("section::MeasureRemove", "− Compasso (seção do cursor)", "Estrutura", "measure"),
    spec!("section::ToggleCollapse", "Recolher/expandir seção", "Estrutura", "fold"),
    spec!("section::ExpandAll", "Expandir todas as seções", "Estrutura", "unfold"),
    spec!("section::CollapseAll", "Recolher todas as seções", "Estrutura", "fold"),
    spec!("song::Sub2", "Subdivisão: colcheia", "Estrutura", "grade 8"),
    spec!("song::Sub4", "Subdivisão: semicolcheia", "Estrutura", "grade 16"),
    spec!("song::Sub8", "Subdivisão: fusa", "Estrutura", "grade 32"),
    spec!("measure::CycleDynamic", "Dinâmica do compasso: próxima", "Estrutura", "pp p mp mf f ff"),
    spec!("track::Add", "+ Linha (instrumento escolhido)", "Estrutura", "track"),
    spec!("track::Remove", "Remover linha do cursor", "Estrutura", "track"),
    spec!("track::ToggleMute", "Mute da linha do cursor", "Estrutura", "mudo"),
    spec!("track::ToggleSolo", "Solo da linha do cursor", "Estrutura", ""),
    spec!("track::CycleInstrument", "Trocar instrumento de + linha", "Estrutura", ""),
    spec!("project::New", "Nova música", "Música", "criar"),
    spec!("project::Duplicate", "Duplicar música", "Música", "cópia"),
    spec!("project::ExportJson", "Exportar música (.taiko.json)", "Música", "salvar arquivo json export"),
    spec!("project::ImportJson", "Importar música (.taiko.json)", "Música", "abrir arquivo json import"),
    spec!("project::ExportMp3", "Exportar MP3", "Música", "audio render mp3"),
    spec!("project::ExportWav", "Exportar WAV", "Música", "audio render wav"),
    spec!("project::NextSong", "Próxima música", "Música", ""),
    spec!("project::PrevSong", "Música anterior", "Música", ""),
];

/// Título dos grupos de chords (which-key).
pub static PREFIX_TITLES: &[(&str, &str)] = &[
    ("space", "líder"),
    ("space t", "transporte"),
    ("space s", "seção"),
    ("space i", "instrumento"),
    ("space v", "ver"),
    ("space g", "ir para"),
    ("space u", "subdivisão"),
    ("space d", "dinâmica"),
    ("space m", "música"),
    ("space z", "zoom"),
    ("g", "ir"),
    ("z", "zoom/dobrar"),
    ("d", "apagar"),
    ("d a", "apagar em todas as linhas"),
    ("d i", "apagar na linha"),
    ("y", "copiar"),
    ("y i", "copiar na linha"),
    ("c", "reescrever"),
];

pub fn title_for(action_name: &str) -> Option<&'static str> {
    if let Some(s) = SPECS.iter().find(|s| s.id == action_name) {
        return Some(s.title);
    }
    Some(match action_name {
        "grid::ExtendLeft" => "estender seleção ←",
        "grid::ExtendRight" => "estender seleção →",
        "grid::ExtendUp" => "estender seleção ↑",
        "grid::ExtendDown" => "estender seleção ↓",
        "grid::NextBeat" => "próximo tempo",
        "grid::PrevBeat" => "tempo anterior",
        "grid::EndOfBeat" => "fim do tempo",
        "grid::NextMeasure" => "próximo compasso",
        "grid::PrevMeasure" => "compasso anterior",
        "grid::MeasureStart" => "início do compasso",
        "grid::MeasureEnd" => "fim do compasso",
        "grid::FirstFilled" => "1ª célula preenchida",
        "grid::NextSection" => "próxima seção",
        "grid::PrevSection" => "seção anterior",
        "grid::HalfRight" => "meia tela →",
        "grid::HalfLeft" => "meia tela ←",
        "grid::DeleteCellBack" => "apagar célula anterior",
        "grid::ModeEdit" => "modo editar",
        "grid::ModeSelect" => "modo selecionar",
        "vim::InsertMeasureStart" => "inserir no início do compasso",
        "vim::AppendMeasureEnd" => "inserir no fim do compasso",
        "vim::Substitute" => "reescrever célula",
        "vim::Normal" => "voltar ao normal",
        "vim::ChangeSelection" => "reescrever seleção",
        "vim::SwapAnchor" => "trocar ponta da seleção",
        "vim::Digit0" | "vim::Digit1" | "vim::Digit2" | "vim::Digit3" | "vim::Digit4" | "vim::Digit5" | "vim::Digit6"
        | "vim::Digit7" | "vim::Digit8" | "vim::Digit9" => "contagem",
        "keys::Cancel" => "cancelar",
        "transport::Straight" => "colcheias retas",
        "transport::SwingOn" => "swing",
        "grid::Left" => "célula ←",
        "grid::Right" => "célula →",
        "grid::Up" => "linha ↑",
        "grid::Down" => "linha ↓",
        "grid::SongStart" => "início da música",
        "grid::SongEnd" => "fim da música",
        "grid::FirstTrack" => "primeira linha",
        "grid::LastTrack" => "última linha",
        "grid::DeleteMeasureRow" => "compasso (linha)",
        "grid::DeleteBeat" => "tempo",
        "grid::DeleteToMeasureEnd" => "até o fim do compasso",
        "grid::DeleteMeasureAll" => "compasso (todas as linhas)",
        "grid::DeleteSectionRow" => "seção (linha)",
        "grid::YankMeasureRow" => "compasso (linha)",
        "grid::YankBeat" => "tempo",
        "grid::YankSectionRow" => "seção (linha)",
        "vim::ChangeMeasureRow" => "compasso (linha)",
        "vim::ChangeBeat" => "tempo",
        "grid::Accent" => "acentuar",
        "grid::Unaccent" => "desacentuar",
        "vim::PushRegister" => "registrador (\"a … \"z, \"+)",
        "vim::SetMark" => "marcar posição (m + letra)",
        "vim::GotoMark" => "ir para marca (' + letra)",
        "vim::SearchBack" => "buscar para trás (?)",
        "vim::Insert" => "inserir",
        "vim::Append" => "inserir depois",
        "vim::Visual" => "visual",
        "vim::VisualMeasure" => "visual por compasso",
        "grid::DeleteCell" => "apagar célula",
        "grid::PasteAfter" => "colar depois",
        "grid::PasteBefore" => "colar antes",
        _ => return None,
    })
}

pub fn prefix_title(prefix: &str) -> Option<&'static str> {
    // a exibição do GPUI põe letras em maiúscula ("space D"); compara sem caixa
    PREFIX_TITLES.iter().find(|(p, _)| p.eq_ignore_ascii_case(prefix)).map(|(_, t)| *t)
}

// ───────────────────────────── keymap ─────────────────────────────

pub const DEFAULT_KEYMAP: &str = include_str!("../assets/keymap-default.json");

pub fn user_keymap_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("wadawko").join("keymap.json")
}

pub fn settings_path() -> PathBuf {
    user_keymap_path().with_file_name("settings.json")
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
                serde_json::Value::Array(a) if a.len() == 2 => {
                    cx.build_action(a[0].as_str().unwrap_or_default(), Some(a[1].clone())).map_err(|e| anyhow!("{e}"))
                }
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

/// settings.json: { "vim": bool, "av_offset_ms": f32 }
#[derive(Clone, Copy)]
pub struct Settings {
    pub vim: bool,
    /// atraso extra do visual em relação ao som (ms); positivo = visual mais tarde
    pub av_offset_ms: f32,
}

pub fn load_settings() -> Settings {
    let v = std::fs::read_to_string(settings_path()).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    Settings {
        vim: v.as_ref().and_then(|v| v["vim"].as_bool()).unwrap_or(true),
        av_offset_ms: v.as_ref().and_then(|v| v["av_offset_ms"].as_f64()).unwrap_or(0.0) as f32,
    }
}

pub fn save_settings(s: Settings) -> Result<()> {
    let p = settings_path();
    std::fs::create_dir_all(p.parent().context("sem pasta")?)?;
    std::fs::write(&p, format!("{{ \"vim\": {}, \"av_offset_ms\": {} }}\n", s.vim, s.av_offset_ms))?;
    Ok(())
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

// ───────────────────────────── fuzzy ─────────────────────────────

fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'ā' => 'a',
        'é' | 'ê' | 'è' | 'ē' => 'e',
        'í' | 'î' | 'ī' => 'i',
        'ó' | 'ô' | 'õ' | 'ö' | 'ō' => 'o',
        'ú' | 'ü' | 'ū' => 'u',
        'ç' => 'c',
        c => c.to_ascii_lowercase(),
    }
}

/// Subsequência com bônus para início de palavra e caracteres consecutivos.
/// `None` = não casa. Maior = melhor.
pub fn fuzzy(query: &str, text: &str) -> Option<i32> {
    let q: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).map(fold).collect();
    if q.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = text.chars().map(|c| fold(c.to_lowercase().next().unwrap_or(c))).collect();
    let mut score = 0;
    let mut qi = 0;
    let mut last: Option<usize> = None;
    for (i, &c) in t.iter().enumerate() {
        if qi < q.len() && c == q[qi] {
            let word_start = i == 0 || !t[i - 1].is_alphanumeric();
            score += 10 + if word_start { 15 } else { 0 } + if last == Some(i.wrapping_sub(1)) { 12 } else { 0 };
            if let Some(l) = last {
                score -= (i - l - 1).min(10) as i32;
            }
            last = Some(i);
            qi += 1;
        }
    }
    (qi == q.len()).then_some(score - t.len() as i32 / 8)
}

// ───────────────────────────── rótulos ─────────────────────────────

pub const HINT_ALPHABET: &str = "asdfghjklqwertyuiopzxcvbnm";

/// Código livre de prefixo com `n` rótulos, mais curtos primeiro.
pub fn labels(n: usize, alphabet: &[char]) -> Vec<String> {
    let mut leaves: std::collections::VecDeque<String> = alphabet.iter().map(|c| c.to_string()).collect();
    if alphabet.len() < 2 {
        return Vec::new();
    }
    // expande o ÚLTIMO dos rótulos MAIS CURTOS (o menos preferido do nível mais raso):
    // árvore balanceada — ex.: 90 alvos = 23 de 1 letra + 67 de 2, nunca "mmmf"
    while leaves.len() < n {
        let min = leaves.iter().map(|l| l.len()).min().unwrap_or(1);
        let idx = leaves.iter().rposition(|l| l.len() == min).expect("alfabeto vazio");
        let p = leaves.remove(idx).expect("índice válido");
        for c in alphabet {
            leaves.push_back(format!("{p}{c}"));
        }
    }
    let mut v: Vec<String> = leaves.into_iter().collect();
    v.sort_by_key(|l| l.len());
    v.truncate(n);
    v
}

// ───────────────────────────── alvos dos hints ─────────────────────────────

pub type HintAct = Rc<dyn Fn(&mut Window, &mut App)>;

pub struct HintTarget {
    pub bounds: Bounds<Pixels>,
    pub act: HintAct,
}

#[derive(Default)]
pub struct HintRegistry {
    pub frame: u64,
    pub targets: HashMap<SharedString, HintTarget>,
}

thread_local! {
    pub static HINTS: RefCell<HintRegistry> = RefCell::new(HintRegistry::default());
}

pub fn next_frame() -> u64 {
    HINTS.with(|h| {
        let mut h = h.borrow_mut();
        h.frame += 1;
        h.frame
    })
}

/// Envolve um elemento interativo para virar alvo de salto: um canvas absoluto
/// filho registra os bounds do pai no prepaint de cada frame em que ele é renderizado.
pub fn hintable<E: gpui_kit::ParentElement + gpui_kit::Styled>(el: E, id: impl Into<SharedString>, act: HintAct) -> E {
    use gpui_kit::{Styled as _, canvas};
    let id = id.into();
    el.child(
        canvas(
            move |b, _, _| {
                HINTS.with(|h| {
                    let mut h = h.borrow_mut();
                    h.targets.insert(id.clone(), HintTarget { bounds: b, act: act.clone() });
                })
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0(),
    )
}

/// Ação → HintAct que a despacha.
pub fn dispatch<A: Action + Clone>(a: A) -> HintAct {
    Rc::new(move |w: &mut Window, cx: &mut App| w.dispatch_action(a.boxed_clone(), cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_prefix_free_and_unique() {
        let alpha: Vec<char> = HINT_ALPHABET.chars().collect();
        for n in [1, 5, 26, 27, 120, 480, 700] {
            let l = labels(n, &alpha);
            assert_eq!(l.len(), n);
            for (i, a) in l.iter().enumerate() {
                for (j, b) in l.iter().enumerate() {
                    if i != j {
                        assert!(!b.starts_with(a.as_str()), "{a} é prefixo de {b} (n={n})");
                    }
                }
            }
            assert!(l.windows(2).all(|w| w[0].len() <= w[1].len()));
            let max = l.iter().map(|x| x.len()).max().unwrap_or(0);
            assert!(max <= if n <= 26 { 1 } else if n <= 676 { 2 } else { 3 }, "rótulo longo demais para n={n}");
        }
    }

    #[test]
    fn fuzzy_folds_accents_and_ranks_word_starts() {
        assert!(fuzzy("metr", "Metrônomo: ligar/desligar").is_some());
        assert!(fuzzy("secao", "+ Seção").is_some());
        assert!(fuzzy("xyz", "Tocar").is_none());
        assert!(fuzzy("tp", "Tocar / parar").unwrap() > fuzzy("tp", "Metrônomo: ligar/desligar").unwrap_or(-999));
    }

    #[test]
    fn default_keymap_has_no_conflicts() {
        let c = conflicts(&raw_bindings(DEFAULT_KEYMAP, "padrão"));
        assert!(c.is_empty(), "{c:#?}");
    }

    #[test]
    fn conflicts_are_detected() {
        let src = r#"[{"context":"X","bindings":{"g":"a::A","g g":"a::B","h":"a::C"}},{"context":"X","bindings":{"h":"a::D"}}]"#;
        let c = conflicts(&raw_bindings(src, "t"));
        assert_eq!(c.len(), 2, "{c:#?}");
    }

    #[test]
    fn default_keymap_actions_have_known_names() {
        let v: serde_json::Value = serde_json::from_str(DEFAULT_KEYMAP).unwrap();
        for block in v.as_array().unwrap() {
            for (_, a) in block["bindings"].as_object().unwrap() {
                let name = a.as_str().unwrap();
                assert!(title_for(name).is_some() || name.starts_with("grid::") || name.starts_with("vim::") || name == "keys::Cancel", "sem título: {name}");
            }
        }
    }
}
