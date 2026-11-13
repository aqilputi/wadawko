//! WaDAWko nativo — protótipo com a UX do app web + teclado (PLANEJAMENTO-teclado.md).
//!
//! Layout espelha src/App.svelte. Lê as músicas de data/wadawko.db em modo
//! SOMENTE LEITURA; edições ficam em memória (com desfazer/refazer).
//!
//! Teclado: modo vim (padrão; desliga com Ctrl+K Ctrl+V ou clicando no chip de
//! modo) ou modo simples. Ctrl+K paleta de comandos · Ctrl+Space / Alt+F saltar
//! para elementos · `space` = líder (normal) com dica das próximas teclas.
//! Keymap: assets/keymap-default.json + ~/.config/wadawko/keymap.json (recarrega sozinho).
//!
//! `--bench N`, `--mute`, `--dark`, `--db caminho`, `--render "Título" saida.wav`.

extern crate gpui_kit as gpui;

mod audio;
mod export;
mod extra;
mod grid;
mod keys;
mod model;
mod sound;
mod stats;
mod theme;

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime};

thread_local! {
    /// (nº de paints da grade, máx ms) — lido pelo teste `--demo enter`
    static STATS_SNAP: Cell<(u64, f32)> = const { Cell::new((0, 0.0)) };
}

/// quantos renders da raiz já aconteceram (teste de "a tela para de atualizar")
pub static RENDERS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static DBG_FOLLOW_H: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static DBG_FOLLOW_V: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static DBG_GRID_RENDERS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// soma (µs) do tempo gasto no render da raiz e intervalo máximo entre renders
pub static RENDER_US: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static RENDER_MAX_US: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

use gpui_kit::{
    Action, App, Bounds, ClickEvent, Context, Entity, FocusHandle, FontWeight, KeyContext, KeyDownEvent, Keystroke,
    MouseButton, Pixels, Render, ScrollEvent, ScrollHandle, SharedString, TextAlign, Window, WindowBounds,
    WindowOptions, canvas, deferred, div, fill, point, prelude::*, px, quad, size,
};

use audio::{Audio, Cmd, PPQ};
use grid::{Cur, GridEvent, GridView, Mode, Motion, VMode};
use keys::HintAct;
use model::{ProjectEntry, Song};
use stats::Stats;
use theme::Theme;

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Editor,
    Pessoas,
    Mixer,
    Kit,
    Config,
}

#[derive(Clone, Copy, PartialEq)]
enum View {
    Grade,
    Partitura,
    Esteira,
}

#[derive(Clone, Copy, PartialEq)]
enum HintScope {
    /// elementos da UI + inícios de tempo
    All,
    /// células visíveis da linha do cursor
    Track,
    /// todas as células visíveis
    Cells,
}

struct HintLabel {
    label: String,
    bounds: Bounds<Pixels>,
    act: HintAct,
    cell: bool,
}

struct HintSession {
    scope: HintScope,
    typed: String,
    labels: Rc<RefCell<Vec<HintLabel>>>,
}

struct Palette {
    query: String,
    sel: usize,
}

struct Bench {
    until: Instant,
    started: Instant,
    last_zoom: Instant,
    zoom_in: bool,
    warmed: bool,
}

struct Spike {
    projects: Vec<ProjectEntry>,
    current: usize,
    song: Rc<RefCell<Song>>,
    bank: Rc<sound::Bank>,
    order: Rc<RefCell<(Vec<(u64, usize)>, u64)>>,
    grid: Entity<GridView>,
    audio: Audio,
    focus: FocusHandle,
    palette_focus: FocusHandle,
    hint_focus: FocusHandle,
    prompt_focus: FocusHandle,
    scroll: ScrollHandle,
    theme: Rc<Cell<Theme>>,
    stats: Rc<RefCell<Stats>>,
    page: Page,
    view: View,
    nav_open: bool,
    looping: bool,
    metronome: bool,
    count_in: bool,
    trainer: bool,
    add_instrument: usize,
    db_path: PathBuf,
    db_error: Option<String>,
    bench: Option<Bench>,
    hud: String,
    last_hud: Instant,
    // teclado
    vim_on: bool,
    count: Option<u32>,
    palette: Option<Palette>,
    hints: Option<HintSession>,
    pending: Vec<Keystroke>,
    pending_since: Instant,
    toast: Option<(String, Instant)>,
    prompt: Option<extra::Prompt>,
    last_change: Option<extra::Change>,
    recording: bool,
    marks: HashMap<char, Cur>,
    last_search: Option<(String, bool)>,
    cmd_history: Vec<String>,
    /// última tecla recebida (página Configurações → testar tecla)
    last_key: Option<(String, Option<String>, Instant)>,
    /// Ctrl+/: dica de atalhos na raiz
    root_hints: bool,
    /// calibração visual × som (ms)
    av_offset_ms: f32,
    /// intenção de tocar + instante do último comando (ver `playing`)
    want_play: bool,
    play_cmd_at: Instant,
    /// export em andamento (mensagem p/ a status bar)
    exporting: Option<String>,
    // histórico (desfazer/refazer)
    last: Song,
    undo: Vec<Song>,
    redo: Vec<Song>,
}

impl Spike {
    fn new(window: &mut Window, cx: &mut Context<Self>, db_path: PathBuf, bench_secs: Option<u64>, mute: bool) -> Self {
        let fallback = || vec![ProjectEntry { id: "novo".into(), updated_ms: 0, song: Song::new_empty("Sem título") }];
        let (projects, current, db_error) = match model::load_workspace(&db_path) {
            Ok((p, c)) if !p.is_empty() => (p, c, None),
            Ok(_) => (fallback(), 0, Some("banco vazio".into())),
            Err(e) => (fallback(), 0, Some(format!("{e:#}"))),
        };
        let song = Rc::new(RefCell::new(projects[current].song.clone()));
        let bank = Rc::new(sound::load_bank(&samples_dir()).expect("decodificar samples"));
        let sr = audio::device_rate();
        let sched = Arc::new(sound::compile(&song.borrow(), &bank, sr, false));
        let order = Rc::new(RefCell::new(song.borrow().play_order()));
        let bpm = song.borrow().tempo;
        let buf: u32 = std::env::var("SPIKE_BUFFER").ok().and_then(|s| s.parse().ok()).unwrap_or(256);
        let mut audio = audio::start(bank.samples.clone(), sched, bpm, buf).expect("abrir saída de áudio");
        if mute {
            audio.send(Cmd::SetGain(0.0));
        }

        // keymap (padrão + usuário)
        let errors = keys::load_keymap(cx);
        for e in &errors {
            eprintln!("keymap: {e}");
        }
        let toast = (!errors.is_empty()).then(|| (format!("keymap: {}", errors.join(" · ")), Instant::now()));
        let settings = keys::load_settings();
        let vim_on = settings.vim;
        let av_offset_ms = settings.av_offset_ms;

        let theme = Rc::new(Cell::new(Theme::light()));
        let stats = Rc::new(RefCell::new(Stats::default()));
        let grid = cx.new(|_| GridView::new(song.clone(), theme.clone(), stats.clone(), vim_on));
        cx.subscribe(&grid, |this: &mut Self, _g, ev: &GridEvent, cx| match ev {
            GridEvent::SongChanged => {
                this.push_history();
                this.rebuild();
                cx.notify();
            }
            GridEvent::Seek(t) => {
                this.audio.send(Cmd::Seek(*t));
                cx.notify();
            }
        })
        .detach();

        // which-key: acompanha as teclas pendentes de um chord
        cx.observe_pending_input(window, |this: &mut Self, window, cx| {
            this.pending = window.pending_input().map(|p| p.keystrokes().to_vec()).unwrap_or_default();
            this.pending_since = Instant::now();
            if !this.pending.is_empty() {
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(260)).await;
                    let _ = this.update(cx, |_, cx| cx.notify());
                })
                .detach();
            }
            cx.notify();
        })
        .detach();

        // hot-reload do keymap do usuário (verifica o mtime a cada 1 s)
        cx.spawn(async move |this, cx| {
            let mtime = || std::fs::metadata(keys::user_keymap_path()).and_then(|m| m.modified()).ok();
            let mut last: Option<SystemTime> = mtime();
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let now = mtime();
                if now != last {
                    last = now;
                    if this.update(cx, |this, cx| this.reload_keymap(cx)).is_err() {
                        break;
                    }
                }
            }
        })
        .detach();

        // toda tecla: alimenta "testar tecla" e fecha a dica da raiz (Ctrl+/)
        cx.observe_keystrokes(|this: &mut Self, ev, _w, cx| {
            let os = cx.platform_info().os;
            let k = ev.keystroke.unparse(os);
            let a = ev.action.as_ref().map(|a| a.name().to_string());
            this.note_key(k, a.clone());
            if this.root_hints && a.as_deref() != Some("keys::ToggleRootHints") {
                this.root_hints = false;
            }
            cx.notify();
        })
        .detach();

        let focus = cx.focus_handle();
        focus.focus(window, cx);

        let bench = bench_secs.map(|s| Bench {
            until: Instant::now() + Duration::from_secs(s + 2),
            started: Instant::now(),
            last_zoom: Instant::now(),
            zoom_in: true,
            warmed: false,
        });
        let last = song.borrow().clone();
        let mut this = Spike {
            projects,
            current,
            song,
            bank,
            order,
            grid,
            audio,
            focus,
            palette_focus: cx.focus_handle(),
            hint_focus: cx.focus_handle(),
            prompt_focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            theme,
            stats,
            page: Page::Editor,
            view: View::Grade,
            nav_open: true,
            looping: true,
            metronome: false,
            count_in: false,
            trainer: false,
            add_instrument: 1,
            db_path,
            db_error,
            bench,
            hud: String::new(),
            last_hud: Instant::now(),
            vim_on,
            count: None,
            palette: None,
            hints: None,
            pending: Vec::new(),
            pending_since: Instant::now(),
            toast,
            prompt: None,
            last_change: None,
            recording: false,
            marks: HashMap::new(),
            last_search: None,
            cmd_history: Vec::new(),
            last_key: None,
            root_hints: false,
            av_offset_ms,
            want_play: false,
            exporting: None,
            play_cmd_at: Instant::now() - Duration::from_secs(1),
            last,
            undo: Vec::new(),
            redo: Vec::new(),
        };
        if this.bench.is_some() {
            this.send_play();
        }
        this
    }

    fn reload_keymap(&mut self, cx: &mut Context<Self>) {
        let errors = keys::load_keymap(cx);
        let msg = if errors.is_empty() { "atalhos recarregados".to_string() } else { format!("keymap: {}", errors.join(" · ")) };
        self.toast = Some((msg, Instant::now()));
        cx.notify();
    }

    /// Tocando? Logo após um comando vale a INTENÇÃO (o thread de áudio só confirma no
    /// próximo callback, e se o render consultasse só o áudio a tela parava de pedir
    /// quadros até um evento de mouse). Depois disso vale o estado real do áudio.
    fn playing(&self) -> bool {
        if self.play_cmd_at.elapsed() < Duration::from_millis(300) {
            self.want_play
        } else {
            self.audio.shared.playing.load(Ordering::Relaxed)
        }
    }

    fn send_play(&mut self) {
        self.audio.send(Cmd::Play);
        self.want_play = true;
        self.play_cmd_at = Instant::now();
    }

    fn send_stop(&mut self) {
        self.audio.send(Cmd::Stop);
        self.want_play = false;
        self.play_cmd_at = Instant::now();
    }

    fn rebuild(&mut self) {
        let song = self.song.borrow();
        let s = Arc::new(sound::compile(&song, &self.bank, self.audio.shared.sample_rate, self.metronome));
        *self.order.borrow_mut() = song.play_order();
        drop(song);
        self.audio.send(Cmd::SetSchedule(s));
        self.audio.collect_garbage();
    }

    // ───────── histórico ─────────

    fn push_history(&mut self) {
        let now = self.song.borrow().clone();
        self.undo.push(std::mem::replace(&mut self.last, now));
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn restore(&mut self, s: Song, cx: &mut Context<Self>) {
        *self.song.borrow_mut() = s.clone();
        self.last = s;
        self.rebuild();
        self.grid.update(cx, |g, cx| {
            g.clamp_cursor();
            cx.notify();
        });
        cx.notify();
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(self.song.borrow().clone());
            self.restore(prev, cx);
        } else {
            self.flash("nada para desfazer");
        }
    }

    fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.song.borrow().clone());
            self.restore(next, cx);
        }
    }

    // ───────── export / import ─────────

    fn export_json(&mut self, cx: &mut Context<Self>) {
        let song = self.song.borrow().clone();
        let dir = export::default_dir();
        let name = format!("{}.taiko.json", song.slug());
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn(async move |this, cx| {
            // sem portal de arquivos: salva direto na pasta sugerida
            let path = match rx.await {
                Ok(Ok(Some(p))) => export::with_ext(p, "json"),
                Ok(Ok(None)) => return,
                _ => dir.join(&name),
            };
            let res = serde_json::to_string_pretty(&song.to_json()).map_err(anyhow::Error::from).and_then(|j| std::fs::write(&path, j).map_err(Into::into));
            let _ = this.update(cx, |this, cx| {
                match res {
                    Ok(()) => this.flash(format!("exportado: {}", path.display())),
                    Err(e) => this.flash(format!("erro ao exportar: {e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn import_json(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Importar música".into()),
        });
        cx.spawn(async move |this, cx| {
            let paths = match rx.await {
                Ok(Ok(Some(p))) => p,
                Ok(Ok(None)) => return,
                _ => {
                    let _ = this.update(cx, |this, cx| {
                        this.flash("diálogo de arquivos indisponível (xdg-desktop-portal?)");
                        cx.notify();
                    });
                    return;
                }
            };
            let Some(path) = paths.into_iter().next() else { return };
            let res = std::fs::read_to_string(&path).map_err(anyhow::Error::from).and_then(|src| Song::from_json_str(&src));
            let _ = this.update(cx, |this, cx| {
                match res {
                    Ok(song) => {
                        let t = song.title.clone();
                        this.push_project(song, cx);
                        this.flash(format!("importado: {t}"));
                    }
                    Err(e) => this.flash(format!("não importei {}: {e:#}", path.display())),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn export_audio(&mut self, ext: &'static str, cx: &mut Context<Self>) {
        if self.exporting.is_some() {
            self.flash("já tem um export em andamento");
            return;
        }
        if ext == "mp3" && !export::ffmpeg_available() {
            self.flash("MP3 precisa do ffmpeg — instale ou use Exportar WAV");
            return;
        }
        let song = self.song.borrow().clone();
        // schedule compilado aqui (precisa do banco de samples); o render vai p/ outra thread
        let sched = Arc::new(sound::compile(&song, &self.bank, export::EXPORT_SR, false));
        let samples = self.bank.samples.clone();
        let dir = export::default_dir();
        let name = format!("{}.{ext}", song.slug());
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn(async move |this, cx| {
            let path = match rx.await {
                Ok(Ok(Some(p))) => export::with_ext(p, ext),
                Ok(Ok(None)) => return,
                _ => dir.join(&name),
            };
            let _ = this.update(cx, |this, cx| {
                this.exporting = Some(format!("renderizando {}…", path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()));
                cx.notify();
            });
            let title = song.title.clone();
            let p2 = path.clone();
            let res = cx
                .background_executor()
                .spawn(async move {
                    let t0 = std::time::Instant::now();
                    let frames = export::render(samples, sched, song.tempo);
                    export::write_audio(&p2, &frames, &title).map(|_| (export::secs(&frames), t0.elapsed().as_secs_f32()))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.exporting = None;
                match res {
                    Ok((dur, took)) => this.flash(format!("exportado {:.0} s de áudio em {:.1} s → {}", dur, took, path.display())),
                    Err(e) => this.flash(format!("erro no export: {e:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn flash(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    // ───────── transporte / projeto ─────────

    fn toggle_play(&mut self, here: bool, cx: &mut Context<Self>) {
        if self.playing() {
            self.send_stop();
        } else {
            if here && let Some(c) = self.grid.read(cx).cursor {
                let t = self.song.borrow().first_tick_of(c.m, c.k);
                self.audio.send(Cmd::Seek(t));
            }
            self.send_play();
        }
        cx.notify();
    }

    fn set_bpm(&mut self, b: f32) {
        let b = b.clamp(30.0, 300.0).round();
        self.song.borrow_mut().tempo = b;
        self.audio.send(Cmd::SetBpm(b));
    }

    fn load_current(&mut self, cx: &mut Context<Self>) {
        *self.song.borrow_mut() = self.projects[self.current].song.clone();
        self.last = self.song.borrow().clone();
        self.undo.clear();
        self.redo.clear();
        self.send_stop();
        self.audio.send(Cmd::Seek(0));
        let bpm = self.song.borrow().tempo;
        self.audio.send(Cmd::SetBpm(bpm));
        self.rebuild();
        self.grid.update(cx, |g, cx| {
            g.reset_for_song();
            cx.notify();
        });
        self.scroll.set_offset(point(px(0.0), px(0.0)));
        cx.notify();
    }

    fn select_project(&mut self, i: usize, cx: &mut Context<Self>) {
        if i == self.current || i >= self.projects.len() {
            return;
        }
        self.projects[self.current].song = self.song.borrow().clone();
        self.current = i;
        self.load_current(cx);
    }

    fn push_project(&mut self, song: Song, cx: &mut Context<Self>) {
        self.projects[self.current].song = self.song.borrow().clone();
        let id = format!("mem-{}", self.projects.len());
        self.projects.insert(0, ProjectEntry { id, updated_ms: 0, song });
        self.current = 0;
        self.load_current(cx);
    }

    fn set_theme(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.theme.set(if dark { Theme::dark() } else { Theme::light() });
        self.grid.update(cx, |g, cx| {
            g.clear_text_cache();
            cx.notify();
        });
        cx.notify();
    }

    fn set_vim(&mut self, on: bool, cx: &mut Context<Self>) {
        self.vim_on = on;
        self.count = None;
        let _ = keys::save_settings(keys::Settings { vim: on, av_offset_ms: self.av_offset_ms });
        self.grid.update(cx, |g, cx| {
            g.commit_edit_in_place(cx);
            g.anchor = None;
            g.vmode = if on { VMode::Normal } else { VMode::Simple };
            cx.notify();
        });
        self.flash(if on { "vim ligado" } else { "vim desligado (modo simples)" });
        cx.notify();
    }

    // ───────── grade ─────────

    fn take_count(&mut self) -> usize {
        self.count.take().map(|c| c as usize).unwrap_or(1)
    }

    fn mv(&mut self, mo: Motion, extend: bool, cx: &mut Context<Self>) {
        let n = self.take_count();
        self.grid.update(cx, |g, cx| {
            let ext = extend || matches!(g.vmode, VMode::Visual | VMode::VLine);
            g.motion(mo, n, ext, cx);
        });
        cx.notify();
    }

    fn g<R>(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut GridView, &mut Context<GridView>, usize) -> R) -> R {
        let n = self.take_count();
        let r = self.grid.update(cx, |g, cx| f(g, cx, n));
        cx.notify();
        r
    }

    /// Fim de operação em visual: volta ao normal (vim) e limpa a seleção.
    fn end_visual(g: &mut GridView) {
        g.anchor = None;
        if matches!(g.vmode, VMode::Visual | VMode::VLine) {
            g.vmode = VMode::Normal;
        }
    }

    fn cursor_section(&self, cx: &App) -> usize {
        let c = self.grid.read(cx).cursor.unwrap_or(Cur { m: 0, t: 0, k: 0 });
        self.song.borrow().section_of(c.m)
    }

    fn cursor_track(&self, cx: &App) -> usize {
        self.grid.read(cx).cursor.map(|c| c.t).unwrap_or(0)
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.grid.update(cx, |g, cx| g.emit_changed(cx));
        cx.notify();
    }

    // ───────── digitação (o que nenhum atalho pegou) ─────────

    fn on_key(&mut self, ev: &KeyDownEvent, _w: &mut Window, cx: &mut Context<Self>) {
        let k = ev.keystroke.key.as_str();
        let m = ev.keystroke.modifiers;
        let (vmode, mode, editing) = {
            let g = self.grid.read(cx);
            (g.vmode, g.mode, !g.edit.is_empty())
        };
        let typing = vmode == VMode::Insert || (vmode == VMode::Simple && mode == Mode::Edit);
        match k {
            "escape" => {
                self.cancel(cx);
                return;
            }
            "space" if vmode == VMode::Simple && !editing => self.toggle_play(false, cx),
            "space" | "enter" | "tab" if typing || vmode == VMode::Simple => self.grid.update(cx, |g, cx| {
                if !g.commit_edit(cx) {
                    g.motion(if m.shift { Motion::Left } else { Motion::Right }, 1, false, cx);
                }
            }),
            "backspace" if editing => self.grid.update(cx, |g, cx| {
                g.edit.pop();
                cx.notify();
            }),
            "backspace" if typing => self.grid.update(cx, |g, cx| {
                // insert: apaga a célula anterior e recua
                if let Some(r) = g.r_cell_back(1) {
                    g.clear_range(r, cx);
                    g.motion(Motion::Left, 1, false, cx);
                }
            }),
            "delete" if typing => self.grid.update(cx, |g, cx| {
                if let Some(r) = g.r_cells(1) {
                    g.clear_range(r, cx);
                }
            }),
            _ => {
                if typing
                    && let Some(ch) = ev.keystroke.key_char.as_deref()
                    && !m.control
                    && !m.alt
                    && ch.chars().all(|c| c.is_ascii_alphabetic() || c == '!')
                {
                    let ch = ch.to_string();
                    self.grid.update(cx, |g, cx| {
                        if g.cursor.is_some() {
                            g.edit.push_str(&ch);
                            cx.notify();
                        }
                    });
                }
            }
        }
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.count = None;
        self.root_hints = false;
        self.grid.update(cx, |g, cx| {
            g.edit.clear();
            g.anchor = None;
            if matches!(g.vmode, VMode::Visual | VMode::VLine | VMode::Insert) {
                g.vmode = VMode::Normal;
            }
            cx.notify();
        });
        cx.notify();
    }

    // ───────── paleta (Ctrl+K) ─────────

    fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hints = None;
        self.palette = Some(Palette { query: String::new(), sel: 0 });
        self.palette_focus.focus(window, cx);
        cx.notify();
    }

    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn toggle_state(&self, id: &str) -> Option<bool> {
        Some(match id {
            "transport::ToggleLoop" => self.looping,
            "transport::ToggleMetronome" => self.metronome,
            "transport::ToggleCountIn" => self.count_in,
            "transport::ToggleTrainer" => self.trainer,
            "transport::ToggleSwing" => self.song.borrow().swing,
            "keys::ToggleVim" => self.vim_on,
            "view::ToggleTheme" => self.theme.get().dark,
            "view::ToggleNav" => self.nav_open,
            _ => return None,
        })
    }

    /// Itens filtrados e ordenados: (título, grupo, atalho, o que fazer).
    fn palette_items(&self, cx: &App) -> Vec<(String, &'static str, Option<String>, PRun)> {
        let q = self.palette.as_ref().map(|p| p.query.as_str()).unwrap_or("");
        let mut scored: Vec<(i32, usize, (String, &'static str, Option<String>, PRun))> = Vec::new();
        for (i, s) in keys::SPECS.iter().enumerate() {
            let Ok(action) = cx.build_action(s.id, None) else { continue };
            let title = match self.toggle_state(s.id) {
                Some(true) => format!("{}  ✓", s.title),
                _ => s.title.to_string(),
            };
            let hay = format!("{} {} {}", s.title, s.group, s.keywords);
            if let Some(sc) = keys::fuzzy(q, &hay) {
                let tsc = keys::fuzzy(q, s.title).unwrap_or(sc);
                let binding = keys::binding_text(&*action, cx);
                scored.push((tsc.max(sc), i, (title, s.group, binding, PRun::Action(action))));
            }
        }
        for (i, p) in self.projects.iter().enumerate() {
            let t = if i == self.current { self.song.borrow().title.clone() } else { p.song.title.clone() };
            if let Some(sc) = keys::fuzzy(q, &format!("música {t}")) {
                scored.push((sc - 5, 1000 + i, (format!("Abrir música: {t}"), "Música", None, PRun::Song(i))));
            }
        }
        if !q.is_empty() {
            scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            // corta casamentos fracos (letras espalhadas) quando há bons
            if let Some(best) = scored.first().map(|x| x.0) {
                scored.retain(|x| x.0 >= best / 2);
            }
        }
        scored.into_iter().map(|(_, _, it)| it).collect()
    }

    fn palette_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.palette.as_mut() else { return };
        let k = ev.keystroke.key.as_str();
        match k {
            "escape" => return self.close_palette(window, cx),
            "up" => p.sel = p.sel.saturating_sub(1),
            "down" | "tab" => p.sel += 1,
            "backspace" => {
                p.query.pop();
                p.sel = 0;
            }
            "enter" => {
                let sel = p.sel;
                let mut items = self.palette_items(cx);
                if sel < items.len() {
                    let (_, _, _, run) = items.swap_remove(sel);
                    self.close_palette(window, cx);
                    match run {
                        PRun::Action(a) => window.dispatch_action(a, cx),
                        PRun::Song(i) => self.select_project(i, cx),
                    }
                }
                return;
            }
            _ => {
                if let Some(ch) = ev.keystroke.key_char.as_deref()
                    && !ev.keystroke.modifiers.control
                    && !ev.keystroke.modifiers.alt
                {
                    p.query.push_str(ch);
                    p.sel = 0;
                }
            }
        }
        let n = self.palette_items(cx).len();
        if let Some(p) = self.palette.as_mut() {
            p.sel = p.sel.min(n.saturating_sub(1));
        }
        cx.notify();
    }

    // ───────── hints (Ctrl+Space) ─────────

    fn start_hints(&mut self, scope: HintScope, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        keys::HINTS.with(|h| h.borrow_mut().targets.clear());
        self.hints = Some(HintSession { scope, typed: String::new(), labels: Default::default() });
        self.hint_focus.focus(window, cx);
        // a grade fica em cache: força re-render para os botões dela se registrarem
        self.grid.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    fn stop_hints(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hints = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn hint_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(h) = self.hints.as_mut() else { return };
        match ev.keystroke.key.as_str() {
            "escape" => return self.stop_hints(window, cx),
            "backspace" => {
                h.typed.pop();
            }
            _ => {
                if let Some(ch) = ev.keystroke.key_char.as_deref()
                    && ch.len() == 1
                    && keys::HINT_ALPHABET.contains(&ch.to_lowercase())
                {
                    h.typed.push_str(&ch.to_lowercase());
                }
            }
        }
        let typed = h.typed.clone();
        let labels = h.labels.clone();
        let labels = labels.borrow();
        if let Some(hit) = labels.iter().find(|l| l.label == typed) {
            let act = hit.act.clone();
            drop(labels);
            self.stop_hints(window, cx);
            act(window, cx);
            return;
        }
        if !typed.is_empty() && !labels.iter().any(|l| l.label.starts_with(&typed)) {
            drop(labels);
            self.flash("rótulo inexistente");
            self.stop_hints(window, cx);
            return;
        }
        cx.notify();
    }

    // ───────── playhead / follow / bench ─────────

    fn playhead(&self, cx: &App) -> Option<(f32, Bounds<Pixels>)> {
        let order = self.order.borrow();
        let tick = self.audio.shared.audible_tick(order.1, self.av_offset_ms);
        let i = order.0.partition_point(|(t, _)| (*t as f64) <= tick).checked_sub(1)?;
        let (start, m) = order.0[i];
        let song = self.song.borrow();
        let meas = song.measures.get(m)?;
        let frac = ((tick - start as f64) / (meas.beats as f64 * PPQ as f64)) as f32;
        let s = song.section_of(m);
        let (a, _) = song.section_range(s);
        let g = self.grid.read(cx).geom.clone();
        let g = g.borrow();
        let b = *g.sections.get(s)?;
        if f32::from(b.size.width) <= 0.0 {
            return None;
        }
        let xs = grid::measure_xs(&song, s, g.cw);
        let x = f32::from(b.origin.x) + xs[m - a] + frac * meas.len() as f32 * g.cw - g.offset;
        Some((x, b))
    }

    fn follow(&mut self, cx: &mut Context<Self>) {
        if !self.playing() {
            return;
        }
        let Some((x, b)) = self.playhead(cx) else { return };
        let left = f32::from(b.origin.x);
        let w = f32::from(b.size.width);
        if x > left + w * 0.85 || x < left {
            DBG_FOLLOW_H.fetch_add(1, Ordering::Relaxed);
            // durante o play a página vira de uma vez (1 redesenho), sem animar a rolagem
            self.grid.update(cx, |g, cx| {
                g.offset_target = (g.offset + (x - left) - w * 0.1).max(0.0);
                g.offset = g.offset_target;
                cx.notify();
            });
        }
        let vb = self.scroll.bounds();
        let (top, bottom) = (f32::from(b.origin.y), f32::from(b.origin.y) + f32::from(b.size.height));
        let (vt, vbm) = (f32::from(vb.origin.y), f32::from(vb.origin.y) + f32::from(vb.size.height));
        if f32::from(vb.size.height) > 0.0 && (top < vt || bottom > vbm) {
            let cur = self.scroll.offset();
            let dy = top - vt - 60.0;
            let max = self.scroll.max_offset();
            let ny = (f32::from(cur.y) - dy).clamp(-f32::from(max.y), 0.0);
            DBG_FOLLOW_V.fetch_add(1, Ordering::Relaxed);
            self.scroll.set_offset(point(cur.x, px(ny)));
        }
    }

    /// Rola a área do editor (vertical) até o cursor quando ele sai da vista.
    fn reveal_cursor(&mut self, cx: &mut Context<Self>) {
        let g = self.grid.read(cx);
        let Some(c) = g.cursor else { return };
        let s = self.song.borrow().section_of(c.m);
        let Some(b) = g.geom.borrow().sections.get(s).copied() else { return };
        if f32::from(b.size.height) <= 0.0 {
            return;
        }
        let y = f32::from(b.origin.y) + grid::SEC_DYN_H + grid::TIMELINE_H + c.t as f32 * grid::TRACK_H;
        let vb = self.scroll.bounds();
        let (vt, vbm) = (f32::from(vb.origin.y), f32::from(vb.origin.y) + f32::from(vb.size.height));
        if f32::from(vb.size.height) > 0.0 && (y < vt + 10.0 || y + grid::CELL_H > vbm - 10.0) {
            let cur = self.scroll.offset();
            let max = self.scroll.max_offset();
            let target = if y < vt { y - vt - 80.0 } else { y + grid::CELL_H - vbm + 80.0 };
            let ny = (f32::from(cur.y) - target).clamp(-f32::from(max.y), 0.0);
            self.scroll.set_offset(point(cur.x, px(ny)));
        }
    }

    fn drive_bench(&mut self, cx: &mut Context<Self>) {
        let Some(b) = self.bench.as_mut() else { return };
        let now = Instant::now();
        if !b.warmed && now.duration_since(b.started) > Duration::from_secs(2) {
            b.warmed = true;
            b.started = now;
            self.stats.borrow_mut().reset();
            self.audio.send(Cmd::ResetCounters);
        }
        if now >= b.until {
            let secs = now.duration_since(b.started).as_secs_f32();
            println!("{}", self.stats.borrow().report(&self.audio.shared, secs));
            cx.quit();
            return;
        }
        if now.duration_since(b.last_zoom) > Duration::from_millis(1500) {
            b.last_zoom = now;
            b.zoom_in = !b.zoom_in;
            let f = if b.zoom_in { 1.8 } else { 1.0 / 1.8 };
            self.grid.update(cx, |g, cx| {
                g.zoom_at(f, 300.0);
                cx.notify();
            });
        }
    }
}

enum PRun {
    Action(Box<dyn Action>),
    Song(usize),
}

// ───────────────────────────── handlers de ação ─────────────────────────────

macro_rules! on {
    ($d:expr, $cx:expr, $A:ty, |$this:ident, $w:ident, $c:ident| $body:expr) => {
        $d.on_action($cx.listener(move |$this: &mut Spike, _: &$A, $w: &mut Window, $c: &mut Context<Spike>| {
            #[allow(unused_variables)]
            let _ = &$w;
            $body;
            $c.notify();
        }))
    };
}

impl Spike {
    fn actions(&self, d: gpui_kit::Stateful<gpui_kit::Div>, cx: &mut Context<Self>) -> gpui_kit::Stateful<gpui_kit::Div> {
        use crate::grid::Motion as M;
        use crate::keys::{grid as ga, hints, keys as kb, measure, nav, project, section, song, track, transport, view, vim};
        let d = on!(d, cx, transport::TogglePlay, |this, w, cx| this.toggle_play(false, cx));
        let d = on!(d, cx, transport::PlayHere, |this, w, cx| this.toggle_play(true, cx));
        let d = on!(d, cx, transport::PlayFromStart, |this, w, cx| {
            this.audio.send(Cmd::Seek(0));
            this.send_play();
        });
        let d = on!(d, cx, transport::Stop, |this, w, cx| this.send_stop());
        let d = on!(d, cx, transport::ToggleLoop, |this, w, cx| {
            this.looping = !this.looping;
            this.audio.send(Cmd::SetLoop(this.looping));
        });
        let d = on!(d, cx, transport::ToggleMetronome, |this, w, cx| {
            this.metronome = !this.metronome;
            this.rebuild();
        });
        let d = on!(d, cx, transport::ToggleCountIn, |this, w, cx| this.count_in = !this.count_in);
        let d = on!(d, cx, transport::ToggleTrainer, |this, w, cx| this.trainer = !this.trainer);
        let d = on!(d, cx, transport::ToggleSwing, |this, w, cx| {
            let s = !this.song.borrow().swing;
            this.song.borrow_mut().swing = s;
            this.rebuild();
        });
        let d = on!(d, cx, transport::Straight, |this, w, cx| {
            this.song.borrow_mut().swing = false;
            this.rebuild();
        });
        let d = on!(d, cx, transport::SwingOn, |this, w, cx| {
            this.song.borrow_mut().swing = true;
            this.rebuild();
        });
        let d = on!(d, cx, transport::BpmUp, |this, w, cx| {
            let n = this.take_count() as f32;
            let b = this.song.borrow().tempo + n;
            this.set_bpm(b)
        });
        let d = on!(d, cx, transport::BpmDown, |this, w, cx| {
            let n = this.take_count() as f32;
            let b = this.song.borrow().tempo - n;
            this.set_bpm(b)
        });

        let d = on!(d, cx, view::Grade, |this, w, cx| {
            this.page = Page::Editor;
            this.view = View::Grade
        });
        let d = on!(d, cx, view::Partitura, |this, w, cx| {
            this.page = Page::Editor;
            this.view = View::Partitura
        });
        let d = on!(d, cx, view::Esteira, |this, w, cx| {
            this.page = Page::Editor;
            this.view = View::Esteira
        });
        let d = on!(d, cx, view::ToggleTheme, |this, w, cx| {
            let dark = !this.theme.get().dark;
            this.set_theme(dark, cx)
        });
        let d = on!(d, cx, view::ZoomIn, |this, w, cx| this.g(cx, |g, cx, _| {
            g.zoom_at(1.25, 0.0);
            cx.notify()
        }));
        let d = on!(d, cx, view::ZoomOut, |this, w, cx| this.g(cx, |g, cx, _| {
            g.zoom_at(0.8, 0.0);
            cx.notify()
        }));
        let d = on!(d, cx, view::ZoomReset, |this, w, cx| this.g(cx, |g, cx, _| {
            g.zoom_reset();
            cx.notify()
        }));
        let d = on!(d, cx, view::ToggleNav, |this, w, cx| this.nav_open = !this.nav_open);
        let d = on!(d, cx, nav::Editor, |this, w, cx| this.page = Page::Editor);
        let d = on!(d, cx, nav::Pessoas, |this, w, cx| this.page = Page::Pessoas);
        let d = on!(d, cx, nav::Mixer, |this, w, cx| this.page = Page::Mixer);
        let d = on!(d, cx, nav::Kit, |this, w, cx| this.page = Page::Kit);

        let d = on!(d, cx, kb::TogglePalette, |this, w, cx| if this.palette.is_some() { this.close_palette(w, cx) } else { this.open_palette(w, cx) });
        let d = on!(d, cx, kb::ShowShortcuts, |this, w, cx| {
            this.palette = None;
            this.page = Page::Config
        });
        let d = on!(d, cx, kb::ReloadKeymap, |this, w, cx| this.reload_keymap(cx));
        let d = on!(d, cx, kb::ToggleVim, |this, w, cx| {
            let on = !this.vim_on;
            this.set_vim(on, cx)
        });
        let d = on!(d, cx, kb::Cancel, |this, w, cx| this.cancel(cx));
        let d = on!(d, cx, hints::Jump, |this, w, cx| this.start_hints(HintScope::All, w, cx));
        let d = on!(d, cx, hints::JumpTrack, |this, w, cx| this.start_hints(HintScope::Track, w, cx));
        let d = on!(d, cx, hints::JumpCells, |this, w, cx| this.start_hints(HintScope::Cells, w, cx));

        let d = on!(d, cx, project::Undo, |this, w, cx| this.undo(cx));
        let d = on!(d, cx, project::Redo, |this, w, cx| this.redo(cx));
        let d = on!(d, cx, project::New, |this, w, cx| this.push_project(Song::new_empty("Sem título"), cx));
        let d = on!(d, cx, project::Duplicate, |this, w, cx| {
            let mut s = this.song.borrow().clone();
            s.title = format!("{} (cópia)", s.title);
            this.push_project(s, cx)
        });
        let d = on!(d, cx, project::ExportJson, |this, w, cx| this.export_json(cx));
        let d = on!(d, cx, project::ImportJson, |this, w, cx| this.import_json(cx));
        let d = on!(d, cx, project::ExportMp3, |this, w, cx| this.export_audio("mp3", cx));
        let d = on!(d, cx, project::ExportWav, |this, w, cx| this.export_audio("wav", cx));
        let d = on!(d, cx, project::NextSong, |this, w, cx| {
            let n = (this.current + 1) % this.projects.len();
            this.select_project(n, cx)
        });
        let d = on!(d, cx, project::PrevSong, |this, w, cx| {
            let n = (this.current + this.projects.len() - 1) % this.projects.len();
            this.select_project(n, cx)
        });

        let d = on!(d, cx, section::Add, |this, w, cx| {
            this.song.borrow_mut().add_section();
            this.changed(cx)
        });
        let d = on!(d, cx, section::Clone, |this, w, cx| {
            let s = this.cursor_section(cx);
            this.song.borrow_mut().clone_section(s);
            this.grid.update(cx, |g, _| g.collapsed.insert(s + 1, false));
            this.changed(cx)
        });
        let d = on!(d, cx, section::Remove, |this, w, cx| {
            let s = this.cursor_section(cx);
            this.song.borrow_mut().remove_section(s);
            this.grid.update(cx, |g, _| {
                if s < g.collapsed.len() {
                    g.collapsed.remove(s);
                }
            });
            this.changed(cx)
        });
        let d = on!(d, cx, section::RepeatUp, |this, w, cx| {
            let s = this.cursor_section(cx);
            let n = this.take_count();
            {
                let mut song = this.song.borrow_mut();
                song.sections[s].repeat = (song.sections[s].repeat + n).min(16);
            }
            this.changed(cx)
        });
        let d = on!(d, cx, section::RepeatDown, |this, w, cx| {
            let s = this.cursor_section(cx);
            let n = this.take_count();
            {
                let mut song = this.song.borrow_mut();
                song.sections[s].repeat = song.sections[s].repeat.saturating_sub(n).max(1);
            }
            this.changed(cx)
        });
        let d = on!(d, cx, section::MeasureAdd, |this, w, cx| {
            let s = this.cursor_section(cx);
            for _ in 0..this.take_count() {
                this.song.borrow_mut().add_measure(s, 1);
            }
            this.changed(cx)
        });
        let d = on!(d, cx, section::MeasureRemove, |this, w, cx| {
            let s = this.cursor_section(cx);
            for _ in 0..this.take_count() {
                this.song.borrow_mut().add_measure(s, -1);
            }
            this.changed(cx)
        });
        let d = on!(d, cx, section::ToggleCollapse, |this, w, cx| {
            let s = this.cursor_section(cx);
            this.grid.update(cx, |g, cx| {
                if let Some(c) = g.collapsed.get_mut(s) {
                    *c = !*c;
                }
                cx.notify()
            })
        });
        let d = on!(d, cx, section::ExpandAll, |this, w, cx| this.grid.update(cx, |g, cx| {
            g.collapsed.iter_mut().for_each(|c| *c = false);
            cx.notify()
        }));
        let d = on!(d, cx, section::CollapseAll, |this, w, cx| this.grid.update(cx, |g, cx| {
            g.collapsed.iter_mut().for_each(|c| *c = true);
            cx.notify()
        }));
        let d = on!(d, cx, song::Sub2, |this, w, cx| {
            this.song.borrow_mut().set_subdivision(2);
            this.changed(cx)
        });
        let d = on!(d, cx, song::Sub4, |this, w, cx| {
            this.song.borrow_mut().set_subdivision(4);
            this.changed(cx)
        });
        let d = on!(d, cx, song::Sub8, |this, w, cx| {
            this.song.borrow_mut().set_subdivision(8);
            this.changed(cx)
        });
        let d = on!(d, cx, measure::CycleDynamic, |this, w, cx| this.g(cx, |g, cx, _| {
            if let Some(c) = g.cursor {
                g.cycle_dynamic(c.m, cx)
            }
        }));
        let d = on!(d, cx, track::Add, |this, w, cx| {
            this.song.borrow_mut().add_track(model::INSTRUMENTS[this.add_instrument]);
            this.changed(cx)
        });
        let d = on!(d, cx, track::Remove, |this, w, cx| {
            let t = this.cursor_track(cx);
            this.song.borrow_mut().remove_track(t);
            this.changed(cx)
        });
        let d = on!(d, cx, track::ToggleMute, |this, w, cx| {
            let t = this.cursor_track(cx);
            let v = !this.song.borrow().tracks[t].muted;
            this.song.borrow_mut().tracks[t].muted = v;
            this.changed(cx)
        });
        let d = on!(d, cx, track::ToggleSolo, |this, w, cx| {
            let t = this.cursor_track(cx);
            let v = !this.song.borrow().tracks[t].solo;
            this.song.borrow_mut().tracks[t].solo = v;
            this.changed(cx)
        });
        let d = on!(d, cx, track::CycleInstrument, |this, w, cx| this.add_instrument = (this.add_instrument + 1) % model::INSTRUMENTS.len());

        // movimentos
        let d = on!(d, cx, ga::Left, |this, w, cx| this.mv(M::Left, false, cx));
        let d = on!(d, cx, ga::Right, |this, w, cx| this.mv(M::Right, false, cx));
        let d = on!(d, cx, ga::Up, |this, w, cx| {
            this.mv(M::Up, false, cx);
            this.reveal_cursor(cx)
        });
        let d = on!(d, cx, ga::Down, |this, w, cx| {
            this.mv(M::Down, false, cx);
            this.reveal_cursor(cx)
        });
        let d = on!(d, cx, ga::ExtendLeft, |this, w, cx| this.mv(M::Left, true, cx));
        let d = on!(d, cx, ga::ExtendRight, |this, w, cx| this.mv(M::Right, true, cx));
        let d = on!(d, cx, ga::ExtendUp, |this, w, cx| this.mv(M::Up, true, cx));
        let d = on!(d, cx, ga::ExtendDown, |this, w, cx| this.mv(M::Down, true, cx));
        let d = on!(d, cx, ga::NextBeat, |this, w, cx| this.mv(M::NextBeat, false, cx));
        let d = on!(d, cx, ga::PrevBeat, |this, w, cx| this.mv(M::PrevBeat, false, cx));
        let d = on!(d, cx, ga::EndOfBeat, |this, w, cx| this.mv(M::EndOfBeat, false, cx));
        let d = on!(d, cx, ga::NextMeasure, |this, w, cx| this.mv(M::NextMeasure, false, cx));
        let d = on!(d, cx, ga::PrevMeasure, |this, w, cx| this.mv(M::PrevMeasure, false, cx));
        let d = on!(d, cx, ga::MeasureStart, |this, w, cx| this.mv(M::MeasureStart, false, cx));
        let d = on!(d, cx, ga::MeasureEnd, |this, w, cx| this.mv(M::MeasureEnd, false, cx));
        let d = on!(d, cx, ga::FirstFilled, |this, w, cx| this.mv(M::FirstFilled, false, cx));
        let d = on!(d, cx, ga::SongStart, |this, w, cx| {
            this.mv(M::SongStart, false, cx);
            this.reveal_cursor(cx)
        });
        let d = on!(d, cx, ga::SongEnd, |this, w, cx| {
            this.mv(M::SongEnd, false, cx);
            this.reveal_cursor(cx)
        });
        let d = on!(d, cx, ga::NextSection, |this, w, cx| {
            this.mv(M::NextSection, false, cx);
            this.reveal_cursor(cx)
        });
        let d = on!(d, cx, ga::PrevSection, |this, w, cx| {
            this.mv(M::PrevSection, false, cx);
            this.reveal_cursor(cx)
        });
        let d = on!(d, cx, ga::FirstTrack, |this, w, cx| this.mv(M::FirstTrack, false, cx));
        let d = on!(d, cx, ga::LastTrack, |this, w, cx| this.mv(M::LastTrack, false, cx));
        let d = on!(d, cx, ga::HalfRight, |this, w, cx| this.mv(M::HalfRight, false, cx));
        let d = on!(d, cx, ga::HalfLeft, |this, w, cx| this.mv(M::HalfLeft, false, cx));

        // operações (normal) — repetíveis com `.`
        use extra::ChangeKind as K;
        let d = on!(d, cx, ga::DeleteCell, |this, w, cx| this.change(K::DeleteCell, cx));
        let d = on!(d, cx, ga::DeleteCellBack, |this, w, cx| this.change(K::DeleteCellBack, cx));
        let d = on!(d, cx, ga::DeleteMeasureRow, |this, w, cx| this.change(K::DeleteMeasureRow, cx));
        let d = on!(d, cx, ga::DeleteMeasureAll, |this, w, cx| this.change(K::DeleteMeasureAll, cx));
        let d = on!(d, cx, ga::DeleteBeat, |this, w, cx| this.change(K::DeleteBeat, cx));
        let d = on!(d, cx, ga::DeleteToMeasureEnd, |this, w, cx| this.change(K::DeleteToMeasureEnd, cx));
        let d = on!(d, cx, ga::DeleteSectionRow, |this, w, cx| this.change(K::DeleteSectionRow, cx));
        let d = on!(d, cx, ga::YankMeasureRow, |this, w, cx| {
            this.g(cx, |g, cx, n| if let Some(r) = g.r_measures(n, false) { g.yank(r, true, cx) });
            this.flash("compasso copiado")
        });
        let d = on!(d, cx, ga::YankBeat, |this, w, cx| {
            this.g(cx, |g, cx, n| if let Some(r) = g.r_beats(n) { g.yank(r, false, cx) });
            this.flash("tempo copiado")
        });
        let d = on!(d, cx, ga::YankSectionRow, |this, w, cx| {
            this.g(cx, |g, cx, _| if let Some(r) = g.r_section() { g.yank(r, false, cx) });
            this.flash("seção (linha) copiada")
        });
        let d = on!(d, cx, ga::PasteAfter, |this, w, cx| this.change(K::PasteAfter, cx));
        let d = on!(d, cx, ga::PasteBefore, |this, w, cx| this.change(K::PasteBefore, cx));
        // seleção (visual / simples)
        let d = on!(d, cx, ga::Copy, |this, w, cx| {
            this.g(cx, |g, cx, _| {
                if let Some(r) = g.sel_range() {
                    g.yank(r, false, cx)
                }
                Self::end_visual(g)
            });
            this.flash("copiado")
        });
        let d = on!(d, cx, ga::Cut, |this, w, cx| this.g(cx, |g, cx, _| {
            if let Some(r) = g.sel_range() {
                g.delete(r, false, cx)
            }
            Self::end_visual(g)
        }));
        let d = on!(d, cx, ga::DeleteSelection, |this, w, cx| this.g(cx, |g, cx, _| {
            if let Some(r) = g.sel_range() {
                g.delete(r, false, cx)
            }
            Self::end_visual(g)
        }));
        let d = on!(d, cx, ga::Paste, |this, w, cx| this.g(cx, |g, cx, _| {
            g.paste_over(cx);
            Self::end_visual(g)
        }));
        let d = on!(d, cx, ga::ToggleSfz, |this, w, cx| {
            let visual = this.grid.read(cx).anchor.is_some();
            this.change(K::ToggleSfz, cx);
            if visual {
                this.last_change = None;
                this.grid.update(cx, |g, _| Self::end_visual(g));
            }
        });
        let d = on!(d, cx, ga::Accent, |this, w, cx| {
            let visual = this.grid.read(cx).anchor.is_some();
            this.change(K::Accent, cx);
            if visual {
                this.last_change = None;
                this.grid.update(cx, |g, _| Self::end_visual(g));
            }
        });
        let d = on!(d, cx, ga::Unaccent, |this, w, cx| {
            let visual = this.grid.read(cx).anchor.is_some();
            this.change(K::Unaccent, cx);
            if visual {
                this.last_change = None;
                this.grid.update(cx, |g, _| Self::end_visual(g));
            }
        });
        let d = on!(d, cx, ga::ToggleSelectMode, |this, w, cx| this.g(cx, |g, cx, _| {
            g.mode = if g.mode == Mode::Edit { Mode::Select } else { Mode::Edit };
            g.anchor = None;
            cx.notify()
        }));
        let d = on!(d, cx, ga::ModeEdit, |this, w, cx| this.g(cx, |g, cx, _| {
            g.mode = Mode::Edit;
            g.anchor = None;
            cx.notify()
        }));
        let d = on!(d, cx, ga::ModeSelect, |this, w, cx| this.g(cx, |g, cx, _| {
            g.mode = Mode::Select;
            cx.notify()
        }));

        // vim
        let d = on!(d, cx, vim::Insert, |this, w, cx| this.change(K::Insert, cx));
        let d = on!(d, cx, vim::Append, |this, w, cx| this.change(K::Append, cx));
        let d = on!(d, cx, vim::InsertMeasureStart, |this, w, cx| this.change(K::InsertMeasureStart, cx));
        let d = on!(d, cx, vim::AppendMeasureEnd, |this, w, cx| this.change(K::AppendMeasureEnd, cx));
        let d = on!(d, cx, vim::Substitute, |this, w, cx| this.change(K::Substitute, cx));
        let d = on!(d, cx, vim::ChangeMeasureRow, |this, w, cx| this.change(K::ChangeMeasureRow, cx));
        let d = on!(d, cx, vim::ChangeBeat, |this, w, cx| this.change(K::ChangeBeat, cx));
        let d = on!(d, cx, vim::ChangeSelection, |this, w, cx| this.g(cx, |g, cx, _| {
            if let Some(r) = g.sel_range() {
                g.delete(r, false, cx)
            }
            g.anchor = None;
            g.vmode = VMode::Insert
        }));
        let d = on!(d, cx, vim::Normal, |this, w, cx| this.finish_insert(cx));
        let d = on!(d, cx, vim::Repeat, |this, w, cx| this.repeat_change(cx));
        let d = on!(d, cx, vim::PushRegister, |this, w, cx| this.open_prompt(extra::PromptKind::Register, w, cx));
        let d = on!(d, cx, vim::SetMark, |this, w, cx| this.open_prompt(extra::PromptKind::SetMark, w, cx));
        let d = on!(d, cx, vim::GotoMark, |this, w, cx| this.open_prompt(extra::PromptKind::GotoMark, w, cx));
        let d = on!(d, cx, vim::Search, |this, w, cx| this.open_prompt(extra::PromptKind::Search { back: false }, w, cx));
        let d = on!(d, cx, vim::SearchBack, |this, w, cx| this.open_prompt(extra::PromptKind::Search { back: true }, w, cx));
        let d = on!(d, cx, vim::SearchNext, |this, w, cx| this.search(true, cx));
        let d = on!(d, cx, vim::SearchPrev, |this, w, cx| this.search(false, cx));
        let d = on!(d, cx, vim::CommandLine, |this, w, cx| this.open_prompt(extra::PromptKind::Command, w, cx));
        let d = on!(d, cx, nav::Config, |this, w, cx| this.page = Page::Config);
        let d = on!(d, cx, kb::ToggleRootHints, |this, w, cx| {
            this.root_hints = !this.root_hints;
            this.palette = None;
        });
        let d = on!(d, cx, kb::CreateKeymap, |this, w, cx| match extra::create_user_keymap() {
            Ok(p) => this.flash(format!("criado {} — edite e salve, recarrega sozinho", p.display())),
            Err(e) => this.flash(format!("não consegui criar: {e}")),
        });
        let d = on!(d, cx, vim::Visual, |this, w, cx| this.g(cx, |g, cx, _| {
            g.anchor = g.cursor;
            g.vmode = VMode::Visual;
            cx.notify()
        }));
        let d = on!(d, cx, vim::VisualMeasure, |this, w, cx| this.g(cx, |g, cx, _| {
            if g.anchor.is_none() {
                g.anchor = g.cursor;
            }
            g.vmode = VMode::VLine;
            cx.notify()
        }));
        let d = on!(d, cx, vim::SwapAnchor, |this, w, cx| this.g(cx, |g, cx, _| {
            std::mem::swap(&mut g.anchor, &mut g.cursor);
            cx.notify()
        }));
        let digit = |this: &mut Spike, n: u32| {
            this.count = Some((this.count.unwrap_or(0) * 10 + n).min(9999));
        };
        let d = on!(d, cx, vim::Digit0, |this, w, cx| digit(this, 0));
        let d = on!(d, cx, vim::Digit1, |this, w, cx| digit(this, 1));
        let d = on!(d, cx, vim::Digit2, |this, w, cx| digit(this, 2));
        let d = on!(d, cx, vim::Digit3, |this, w, cx| digit(this, 3));
        let d = on!(d, cx, vim::Digit4, |this, w, cx| digit(this, 4));
        let d = on!(d, cx, vim::Digit5, |this, w, cx| digit(this, 5));
        let d = on!(d, cx, vim::Digit6, |this, w, cx| digit(this, 6));
        let d = on!(d, cx, vim::Digit7, |this, w, cx| digit(this, 7));
        let d = on!(d, cx, vim::Digit8, |this, w, cx| digit(this, 8));
        on!(d, cx, vim::Digit9, |this, w, cx| digit(this, 9))
    }
}

// ───────────────────────────── peças de UI ─────────────────────────────

fn btn(id: impl Into<gpui_kit::ElementId>, th: &Theme) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .relative()
        .flex_none()
        .h(px(32.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(th.line_strong)
        .bg(th.paper)
        .text_size(px(13.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(th.ink)
        .cursor_pointer()
        .hover(|s| s.bg(th.paper_hover))
}

/// Botão que despacha uma ação (clique = atalho = paleta = salto).
fn abtn<A: Action + Clone>(id: &'static str, th: &Theme, a: A) -> gpui_kit::Stateful<gpui_kit::Div> {
    let a2 = a.clone();
    keys::hintable(btn(id, th).on_click(move |_, w, cx| w.dispatch_action(a.boxed_clone(), cx)), id, keys::dispatch(a2))
}

fn atoggle<A: Action + Clone>(id: &'static str, th: &Theme, on: bool, a: A) -> gpui_kit::Stateful<gpui_kit::Div> {
    let b = abtn(id, th, a);
    if on { b.bg(th.accent2_wash).border_color(th.accent2).text_color(th.accent2) } else { b }
}

/// Grupo segmentado; cada segmento despacha a sua ação e é alvo de salto.
fn segmented(id: &'static str, th: &Theme, items: Vec<(&'static str, Box<dyn Action>)>, active: usize, blue: bool) -> impl IntoElement {
    let mut row = div().flex().flex_none().rounded(px(7.0)).border_1().border_color(th.line_strong).bg(th.paper).overflow_hidden();
    for (i, (l, a)) in items.into_iter().enumerate() {
        let on = i == active;
        let a2 = a.boxed_clone();
        let hid = format!("{id}-{i}");
        row = row.child(keys::hintable(
            div()
                .id((id, i))
                .relative()
                .h(px(30.0))
                .px(px(14.0))
                .flex()
                .items_center()
                .text_size(px(13.0))
                .font_weight(if on { FontWeight::BOLD } else { FontWeight::MEDIUM })
                .cursor_pointer()
                .when(on, |d| d.bg(if blue { th.accent2 } else { th.accent }).text_color(th.on_accent))
                .when(!on, |d| d.text_color(th.ink_soft).hover(|s| s.bg(th.paper_hover)))
                .child(l)
                .on_click(move |_, w, cx| w.dispatch_action(a.boxed_clone(), cx)),
            hid,
            Rc::new(move |w: &mut Window, cx: &mut App| w.dispatch_action(a2.boxed_clone(), cx)),
        ));
    }
    row
}

fn kbd(text: &str, th: &Theme) -> impl IntoElement {
    div()
        .px(px(5.0))
        .h(px(18.0))
        .flex()
        .items_center()
        .rounded(px(4.0))
        .border_1()
        .border_color(th.line_strong)
        .bg(th.paper_raise)
        .text_size(px(10.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(th.ink_soft)
        .child(SharedString::from(text.to_string()))
}

impl Render for Spike {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        keys::next_frame();
        RENDERS.fetch_add(1, Ordering::Relaxed);
        let t_render = Instant::now();
        let el = self.render_inner(window, cx);
        let us = t_render.elapsed().as_micros() as u64;
        RENDER_US.fetch_add(us, Ordering::Relaxed);
        RENDER_MAX_US.fetch_max(us, Ordering::Relaxed);
        el
    }
}

impl Spike {
    fn render_inner(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::Stateful<gpui_kit::Div> {
        let playing = self.playing();
        self.stats.borrow_mut().frame(playing || self.bench.is_some());
        {
            let st = self.stats.borrow();
            STATS_SNAP.with(|s| s.set((st.grid_paint.len() as u64, st.grid_paint.max())));
        }
        self.follow(cx);
        self.drive_bench(cx);
        if self.bench.is_some() || playing {
            window.request_animation_frame();
        }
        self.audio.collect_garbage();
        if self.last_hud.elapsed() > Duration::from_millis(500) {
            self.last_hud = Instant::now();
            self.hud = self.stats.borrow().compact(&self.audio.shared);
        }
        if self.toast.as_ref().is_some_and(|(_, t)| t.elapsed() > Duration::from_secs(5)) {
            self.toast = None;
        }

        let th = self.theme.get();
        let song = self.song.borrow();
        let title: SharedString = song.title.clone().into();
        let bpm = song.tempo;
        let swing = song.swing;
        let grid_h = grid::grid_height(&song, &self.grid.read(cx).collapsed);
        let sub = song.measures.first().map(|m| m.subdivision).unwrap_or(2);
        let (mode, zoom_pct, vmode, cursor) = {
            let g = self.grid.read(cx);
            (g.mode, g.zoom_pct(), g.vmode, g.cursor)
        };
        let pos_text = cursor.map(|c| {
            let s = song.section_of(c.m);
            let sub = song.measures[c.m].subdivision;
            format!(
                "{} · c{} · {}.{} · {}",
                song.sections[s].name,
                c.m + 1,
                c.k / sub + 1,
                c.k % sub + 1,
                song.tracks.get(c.t).map(|t| t.name.as_str()).unwrap_or("")
            )
        });
        drop(song);
        let weak = cx.entity().downgrade();

        // ── NavRail ──
        let nav = {
            let items: [(&str, &str, Option<Page>, Option<Box<dyn Action>>); 5] = [
                ("📝", "Editor", Some(Page::Editor), Some(Box::new(keys::nav::Editor))),
                ("👥", "Pessoas", Some(Page::Pessoas), Some(Box::new(keys::nav::Pessoas))),
                ("🎚", "Mixer", Some(Page::Mixer), Some(Box::new(keys::nav::Mixer))),
                ("🥁", "Kit", Some(Page::Kit), Some(Box::new(keys::nav::Kit))),
                ("⚙", "Config", Some(Page::Config), Some(Box::new(keys::nav::Config))),
            ];
            let mut col = div().w(px(200.0)).flex_none().h_full().bg(th.paper).border_r_1().border_color(th.line).p(px(6.0)).flex().flex_col().gap(px(4.0));
            for (i, (icon, label, page, action)) in items.into_iter().enumerate() {
                let on = page == Some(self.page);
                let item = div()
                    .id(("nav", i))
                    .relative()
                    .h(px(38.0))
                    .px(px(12.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .rounded(px(8.0))
                    .border_1()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::MEDIUM)
                    .when(on, |d| d.bg(th.accent_wash).border_color(th.accent).text_color(th.accent))
                    .when(!on, |d| d.border_color(gpui_kit::transparent_black()).text_color(th.ink_soft))
                    .when(page.is_none(), |d| d.opacity(0.5))
                    .when(page.is_some() && !on, |d| d.cursor_pointer().hover(|s| s.bg(th.paper_hover)))
                    .child(div().w(px(20.0)).child(icon))
                    .child(label);
                col = col.child(match action {
                    Some(a) => {
                        let a2 = a.boxed_clone();
                        keys::hintable(
                            item.on_click(move |_, w, cx| w.dispatch_action(a.boxed_clone(), cx)),
                            format!("nav-{i}"),
                            Rc::new(move |w: &mut Window, cx: &mut App| w.dispatch_action(a2.boxed_clone(), cx)),
                        )
                    }
                    None => item,
                });
            }
            col
        };

        // ── Topbar ──
        let topbar = div()
            .h(px(57.0))
            .flex_none()
            .w_full()
            .px(px(16.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .bg(th.paper)
            .border_b_1()
            .border_color(th.line)
            .child(
                div()
                    .size(px(37.0))
                    .rounded(px(8.0))
                    .bg(th.accent)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(th.on_accent)
                    .text_size(px(22.0))
                    .font_family("Shippori Mincho")
                    .font_weight(FontWeight::BOLD)
                    .child("太"),
            )
            .child(abtn("hamb", &th, keys::view::ToggleNav).w(px(32.0)).px(px(0.0)).child("☰"))
            .child(div().ml(px(4.0)).text_size(px(22.0)).font_family("Shippori Mincho").font_weight(FontWeight::BOLD).child(title))
            .child(div().flex_1())
            .child(div().text_size(px(12.0)).text_color(th.ink_soft).child("BPM"))
            .child(
                div()
                    .id("bpm")
                    .w(px(64.0))
                    .h(px(30.0))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .rounded(px(7.0))
                    .border_1()
                    .border_color(th.line_strong)
                    .bg(th.paper_raise)
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child(SharedString::from(format!("{bpm:.0}")))
                    .on_scroll(cx.listener(|this, ev: &ScrollEvent, _, cx| {
                        let d = f32::from(ev.delta.pixel_delta(px(20.0)).y);
                        let b = this.song.borrow().tempo + if d > 0.0 { 1.0 } else { -1.0 };
                        this.set_bpm(b);
                        cx.notify();
                    })),
            )
            .child(segmented(
                "swing",
                &th,
                vec![("reto", Box::new(keys::transport::Straight)), ("swing", Box::new(keys::transport::SwingOn))],
                swing as usize,
                true,
            ))
            .child(atoggle("loop", &th, self.looping, keys::transport::ToggleLoop).child("↺ loop"))
            .child(atoggle("metro", &th, self.metronome, keys::transport::ToggleMetronome).w(px(38.0)).px(px(0.0)).child("🎵"))
            .child(atoggle("countin", &th, self.count_in, keys::transport::ToggleCountIn).w(px(38.0)).px(px(0.0)).child("⏱"))
            .child(atoggle("trainer", &th, self.trainer, keys::transport::ToggleTrainer).w(px(38.0)).px(px(0.0)).child("🏃"))
            .child(
                abtn("play", &th, keys::transport::TogglePlay)
                    .w(px(96.0))
                    .bg(th.accent)
                    .border_color(th.accent)
                    .text_color(th.on_accent)
                    .font_weight(FontWeight::BOLD)
                    .hover(|s| s.opacity(0.9))
                    .child(if playing { "■ parar" } else { "▶ tocar" }),
            )
            .child(abtn("theme", &th, keys::view::ToggleTheme).w(px(36.0)).px(px(0.0)).child(if th.dark { "☀" } else { "☾" }));

        // ── Editor ──
        let view_tabs = segmented(
            "view",
            &th,
            vec![("grade", Box::new(keys::view::Grade)), ("partitura", Box::new(keys::view::Partitura)), ("esteira", Box::new(keys::view::Esteira))],
            self.view as usize,
            false,
        );
        let mut toolbar = div().flex().flex_wrap().items_center().gap(px(8.0)).p(px(7.0)).rounded(px(10.0)).bg(th.paper).border_1().border_color(th.line);
        if !self.vim_on {
            toolbar = toolbar.child(segmented(
                "mode",
                &th,
                vec![("✎ editar", Box::new(keys::grid::ModeEdit)), ("⬚ selecionar", Box::new(keys::grid::ModeSelect))],
                (mode == Mode::Select) as usize,
                false,
            ));
        }
        let toolbar = toolbar
            .child(segmented(
                "sub",
                &th,
                vec![("colcheia", Box::new(keys::song::Sub2)), ("semicolcheia", Box::new(keys::song::Sub4)), ("fusa", Box::new(keys::song::Sub8))],
                match sub {
                    4 => 1,
                    8 => 2,
                    _ => 0,
                },
                false,
            ))
            .child(abtn("addsec", &th, keys::section::Add).child("+ seção"))
            .child(
                abtn("inst", &th, keys::track::CycleInstrument)
                    .w(px(150.0))
                    .justify_between()
                    .child(model::instrument_label(model::INSTRUMENTS[self.add_instrument]))
                    .child(div().text_color(th.ink_faint).child("▾")),
            )
            .child(abtn("addline", &th, keys::track::Add).child("+ linha"))
            .child(abtn("zout", &th, keys::view::ZoomOut).child("🔍−"))
            .child(div().w(px(44.0)).flex().justify_center().text_size(px(12.0)).text_color(th.ink_soft).child(SharedString::from(format!("{zoom_pct}%"))))
            .child(abtn("zin", &th, keys::view::ZoomIn).child("🔍+"))
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .text_size(px(11.0))
                    .text_color(th.ink_faint)
                    .child(kbd("Ctrl K", &th))
                    .child("comandos")
                    .child(kbd("Ctrl ␣", &th))
                    .child("saltar"),
            );

        let placeholder = |text: &'static str| div().w_full().h(px(300.0)).flex().items_center().justify_center().text_color(th.ink_faint).text_size(px(14.0)).child(text);
        let content = match (self.page, self.view) {
            (Page::Editor, View::Grade) => div()
                .flex()
                .flex_col()
                .gap(px(18.0))
                .child(div().flex().child(view_tabs))
                .child(toolbar)
                .child(self.grid.clone().cached(gpui_kit::StyleRefinement::default().w_full().h(px(grid_h))))
                .into_any_element(),
            (Page::Editor, _) => div()
                .flex()
                .flex_col()
                .gap(px(18.0))
                .child(div().flex().child(view_tabs))
                .child(placeholder("Partitura e esteira ainda não foram portadas para o app nativo (Fases 4–5)."))
                .into_any_element(),
            (Page::Pessoas, _) => placeholder("Pessoas — ainda não portado (Fase 4).").into_any_element(),
            (Page::Mixer, _) => placeholder("Mixer — ainda não portado (Fase 4).").into_any_element(),
            (Page::Kit, _) => placeholder("Kit — ainda não portado (Fase 4).").into_any_element(),
            (Page::Config, _) => self.config_page(&th, cx).into_any_element(),
        };

        // playhead
        let ph = if self.page == Page::Editor && self.view == View::Grade { self.playhead(cx) } else { None };
        let ph_playing = playing;
        // notas soando agora (destaque nas células)
        let (flashes, play_pos) = if self.page == Page::Editor && self.view == View::Grade { self.playing_cells(cx) } else { (Vec::new(), None) };
        let overlay = canvas(
            |_, _, _| {},
            move |vb, _, window, _| {
                for f in &flashes {
                    let clip = vb.intersect(&f.bounds);
                    if f32::from(clip.size.width) <= 0.0 || f32::from(clip.size.height) <= 0.0 {
                        continue;
                    }
                    let col = if f.rim { th.accent2 } else { th.accent };
                    window.with_content_mask(Some(gpui_kit::ContentMask { bounds: vb }), |window| {
                        // halo + preenchimento que apaga ao longo da célula
                        let halo = f.bounds.dilate(px(5.0 * f.strength));
                        window.paint_quad(quad(halo, px(7.0), Theme::alpha(col, 0.28 * f.strength), px(0.0), gpui_kit::transparent_black(), gpui_kit::BorderStyle::Solid));
                        window.paint_quad(quad(f.bounds, px(3.0), Theme::alpha(col, 0.6 * f.strength), px(3.0), Theme::alpha(col, f.strength.max(0.35)), gpui_kit::BorderStyle::Solid));
                    });
                }
                let Some((x, b)) = ph else { return };
                let clip = vb.intersect(&b);
                window.with_content_mask(Some(gpui_kit::ContentMask { bounds: clip }), |window| {
                    let top = f32::from(b.origin.y) + grid::SEC_DYN_H;
                    let h = f32::from(b.size.height) - grid::SEC_DYN_H;
                    if ph_playing {
                        window.paint_quad(fill(Bounds::new(point(px(x - 1.0), px(top)), size(px(2.0), px(h))), th.accent));
                    }
                    let mut tri = gpui_kit::PathBuilder::fill();
                    tri.move_to(point(px(x - 6.0), px(top)));
                    tri.line_to(point(px(x + 6.0), px(top)));
                    tri.line_to(point(px(x), px(top + 8.0)));
                    tri.close();
                    if let Ok(p) = tri.build() {
                        window.paint_path(p, th.accent);
                    }
                });
            },
        )
        .absolute()
        .inset_0();

        let editor = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .relative()
            .bg(th.bg)
            .child(div().id("editor-scroll").size_full().overflow_y_scroll().track_scroll(&self.scroll).p(px(20.0)).child(content))
            .child(overlay);

        // ── Painel de músicas ──
        let mut list = div().flex().flex_col().gap(px(8.0));
        for (i, p) in self.projects.iter().enumerate() {
            let on = i == self.current;
            let t: SharedString = if on { self.song.borrow().title.clone() } else { p.song.title.clone() }.into();
            let date: SharedString = if p.updated_ms > 0 { model::short_date(p.updated_ms) } else { "agora".into() }.into();
            let w = weak.clone();
            let card = div()
                .id(("song", i))
                .relative()
                .px(px(10.0))
                .py(px(9.0))
                .rounded(px(8.0))
                .border_1()
                .flex()
                .items_start()
                .when(on, |d| d.bg(th.accent_wash).border_color(th.accent))
                .when(!on, |d| d.bg(th.paper_raise).border_color(th.line).cursor_pointer().hover(|s| s.bg(th.paper_hover)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(div().text_size(px(14.0)).font_weight(FontWeight::BOLD).truncate().child(t))
                        .child(div().text_size(px(11.0)).text_color(th.ink_faint).child(date)),
                )
                .when(on, |d| {
                    d.child(
                        div()
                            .flex()
                            .gap(px(4.0))
                            .child(abtn("dup", &th, keys::project::Duplicate).h(px(24.0)).w(px(24.0)).px(px(0.0)).text_size(px(11.0)).child("⧉"))
                            .child(btn(("del", i), &th).h(px(24.0)).w(px(24.0)).px(px(0.0)).text_size(px(11.0)).child("✕").on_click(cx.listener(
                                move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    if this.projects.len() > 1 {
                                        this.projects.remove(i);
                                        this.current = i.min(this.projects.len() - 1);
                                        this.load_current(cx);
                                    }
                                },
                            ))),
                    )
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select_project(i, cx)));
            list = list.child(keys::hintable(
                card,
                format!("song-{i}"),
                Rc::new(move |_, cx: &mut App| {
                    let _ = w.update(cx, |this, cx| this.select_project(i, cx));
                }),
            ));
        }
        let panel = div()
            .w(px(260.0))
            .flex_none()
            .h_full()
            .bg(th.paper)
            .border_l_1()
            .border_color(th.line)
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(div().flex_1().text_size(px(13.0)).font_weight(FontWeight::BOLD).text_color(th.ink_soft).child("MÚSICAS"))
                    .child(
                        abtn("nova", &th, keys::project::New)
                            .h(px(28.0))
                            .rounded(px(999.0))
                            .bg(th.accent)
                            .border_color(th.accent)
                            .text_color(th.on_accent)
                            .text_size(px(12.0))
                            .font_weight(FontWeight::BOLD)
                            .hover(|s| s.opacity(0.9))
                            .child("+ nova"),
                    ),
            )
            .child(div().id("songs").flex_1().min_h_0().overflow_y_scroll().child(list))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.0))
                            .child(abtn("exp", &th, keys::project::ExportJson).flex_1().child("exportar"))
                            .child(abtn("imp", &th, keys::project::ImportJson).flex_1().child("importar")),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(8.0))
                            .child(abtn("mp3", &th, keys::project::ExportMp3).flex_1().child(if self.exporting.is_some() { "exportando…" } else { "exportar mp3" }))
                            .child(abtn("wav", &th, keys::project::ExportWav).w(px(64.0)).child("wav")),
                    ),
            );

        // ── Status bar: modo · showcmd · posição ──
        let os = cx.platform_info().os;
        let showcmd: String = {
            let mut s = self.count.map(|c| c.to_string()).unwrap_or_default();
            for k in &self.pending {
                if !s.is_empty() {
                    s.push(' ');
                }
                s.push_str(&k.display(os).to_string());
            }
            s
        };
        let (chip_bg, chip_fg) = match vmode {
            VMode::Normal => (th.ink, th.paper),
            VMode::Insert => (th.accent, th.on_accent),
            VMode::Visual | VMode::VLine => (th.accent2, th.on_accent),
            VMode::Simple => (th.paper_raise, th.ink_soft),
        };
        let tocando: SharedString = match (playing, &play_pos) {
            (true, Some(p)) => format!("♪ tocando · {p}").into(),
            (true, None) => "♪ tocando".into(),
            _ => "parado".into(),
        };
        let src = match &self.db_error {
            None => format!("somente leitura · {}", self.db_path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()),
            Some(e) => format!("sem banco ({e}) · em memória"),
        };
        let status = div()
            .h(px(26.0))
            .flex_none()
            .w_full()
            .px(px(10.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .bg(th.paper)
            .border_t_1()
            .border_color(th.line)
            .text_size(px(11.5))
            .text_color(th.ink_soft)
            .child(keys::hintable(
                div()
                    .id("modechip")
                    .relative()
                    .px(px(8.0))
                    .h(px(18.0))
                    .flex()
                    .items_center()
                    .rounded(px(4.0))
                    .bg(chip_bg)
                    .text_color(chip_fg)
                    .text_size(px(10.5))
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .child(vmode.label())
                    .on_click(|_, w, cx| w.dispatch_action(Box::new(keys::keys::ToggleVim), cx)),
                "modechip",
                keys::dispatch(keys::keys::ToggleVim),
            ))
            .when(!showcmd.is_empty(), |d| d.child(div().text_color(th.accent).font_weight(FontWeight::BOLD).child(SharedString::from(showcmd.clone()))))
            .when_some(pos_text, |d, p| d.child(div().text_color(th.ink_faint).child(SharedString::from(p))))
            .child(div().text_color(th.ink_faint).child("·"))
            .child(SharedString::from(format!("{bpm:.0} BPM")))
            .child(div().text_color(th.ink_faint).child("·"))
            .child(tocando)
            .when_some(self.exporting.clone(), |d, t| d.child(div().px(px(8.0)).rounded(px(4.0)).bg(th.accent_wash).text_color(th.accent).child(SharedString::from(format!("⏳ {t}")))))
            .when_some(self.toast.as_ref().map(|t| t.0.clone()), |d, t| d.child(div().px(px(8.0)).rounded(px(4.0)).bg(th.accent2_wash).text_color(th.accent2).child(SharedString::from(t))))
            .child(div().flex_1())
            .child(div().text_color(th.ink_faint).child(SharedString::from(self.hud.clone())))
            .child(div().text_color(th.ink_faint).child("·"))
            .child(div().text_color(th.ink_faint).child(SharedString::from(src)));

        // ── workspace (contexto de teclas da grade) ──
        let ctx = format!("Workspace vim={} mode={}{}", if self.vim_on { "on" } else { "off" }, vmode.ctx(), if self.count.is_some() { " count" } else { "" });
        let workspace = div()
            .id("workspace")
            .key_context(KeyContext::parse(&ctx).unwrap_or_default())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when(self.nav_open, |d| d.child(nav))
                    .child(div().flex_1().min_w_0().flex().flex_col().child(topbar).child(div().flex_1().min_h_0().flex().child(editor).child(panel))),
            )
            .child(status);

        let mut root = self.actions(div().id("root").relative().size_full().font_family("Zen Kaku Gothic New").bg(th.bg).text_color(th.ink), cx).child(workspace);

        // ── which-key (command hint): chord pendente, ou Ctrl+/ na raiz ──
        if !self.pending.is_empty() && self.pending_since.elapsed() >= Duration::from_millis(250) && self.palette.is_none() {
            root = root.child(self.which_key(window, &th, cx));
        } else if self.root_hints && self.pending.is_empty() && self.palette.is_none() {
            root = root.child(self.which_key(window, &th, cx));
        }

        // ── linha de prompt (: / ? " m ') ──
        if let Some(p) = &self.prompt {
            root = root.child(
                div()
                    .id("prompt")
                    .absolute()
                    .inset_0()
                    .key_context("Prompt")
                    .track_focus(&self.prompt_focus)
                    .on_key_down(cx.listener(Self::prompt_key))
                    .child(self.prompt_bar(p, &th)),
            );
        }

        // ── paleta (Ctrl+K) ──
        if let Some(p) = &self.palette {
            root = root.child(self.palette_view(p, &th, cx));
        }

        // ── hints (Ctrl+Space) ──
        if let Some(h) = &self.hints {
            root = root.child(self.hints_view(h, &th, cx));
        }
        root
    }
}

impl Spike {
    fn which_key(&self, window: &Window, th: &Theme, cx: &App) -> impl IntoElement {
        let os = cx.platform_info().os;
        let n = self.pending.len();
        // forma explícita ("a" vs "shift-a"): a exibição do GPUI põe tudo em maiúscula
        let prefix: String = self.pending.iter().map(|k| k.unparse(os)).collect::<Vec<_>>().join(" ");
        let mut entries: Vec<(String, String, bool)> = Vec::new();
        let mut digits = false;
        let all = window.possible_bindings_for_input(&self.pending);
        for b in &all {
            let ks = b.keystrokes();
            if ks.len() <= n || gpui_kit::is_no_action(b.action()) {
                continue;
            }
            // só ações do app (os bindings internos do gpui-component não interessam aqui)
            if ks.len() == n + 1 && keys::title_for(b.action().name()).is_none() {
                continue;
            }
            let key = ks[n].unparse(os);
            if b.action().name().starts_with("vim::Digit") && ks.len() == n + 1 {
                digits = true;
                continue;
            }
            if entries.iter().any(|e| e.0 == key) {
                continue;
            }
            if ks.len() == n + 1 {
                let t = keys::title_for(b.action().name()).unwrap_or(b.action().name());
                entries.push((key, t.to_string(), false));
            } else {
                let p = format!("{prefix} {key}").trim().to_string();
                let t = keys::prefix_title(&p).unwrap_or("…");
                let count = all.iter().filter(|o| o.keystrokes().len() > n && o.keystrokes()[n].unparse(os) == key && !gpui_kit::is_no_action(o.action())).count();
                entries.push((key, format!("+{t} ({count})"), true));
            }
        }
        if digits {
            entries.push(("1–9".into(), "contagem (repete o próximo comando)".into(), false));
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut grid_el = div().flex().flex_wrap().gap_x(px(18.0)).gap_y(px(4.0));
        for (k, t, group) in entries {
            grid_el = grid_el.child(
                div()
                    .w(px(196.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(kbd(&k, th))
                    .child(div().text_size(px(12.0)).text_color(if group { th.accent } else { th.ink_soft }).truncate().child(SharedString::from(t))),
            );
        }
        let title = if prefix.is_empty() {
            "Atalhos disponíveis aqui (Ctrl+/ ou Esc fecha)".to_string()
        } else {
            keys::prefix_title(&prefix).map(|t| format!("{prefix} — {t}")).unwrap_or(prefix)
        };
        deferred(
            div()
                .absolute()
                .left(px(220.0))
                .right(px(280.0))
                .bottom(px(34.0))
                .max_h(px(620.0))
                .overflow_hidden()
                .p(px(12.0))
                .rounded(px(10.0))
                .bg(th.paper)
                .border_1()
                .border_color(th.line_strong)
                .shadow_lg()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(div().text_size(px(12.0)).font_weight(FontWeight::BOLD).text_color(th.ink).child(SharedString::from(title)))
                .child(grid_el),
        )
        .with_priority(150)
    }

    fn palette_view(&self, p: &Palette, th: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let items = self.palette_items(cx);
        let total = items.len();
        let sel = p.sel.min(total.saturating_sub(1));
        let start = sel.saturating_sub(7);
        let mut list = div().flex().flex_col().gap(px(2.0)).overflow_hidden();
        for (i, (title, group, binding, _)) in items.into_iter().enumerate().skip(start).take(11) {
            let on = i == sel;
            list = list.child(
                div()
                    .id(("pal", i))
                    .px(px(10.0))
                    .h(px(32.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .when(on, |d| d.bg(th.accent_wash))
                    .hover(|s| s.bg(th.paper_hover))
                    .child(div().w(px(84.0)).text_size(px(10.5)).text_color(th.ink_faint).child(group))
                    .child(div().flex_1().text_size(px(13.5)).when(on, |d| d.text_color(th.accent).font_weight(FontWeight::MEDIUM)).child(SharedString::from(title)))
                    .when_some(binding, |d, b| d.child(kbd(&b, th)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if let Some(p) = this.palette.as_mut() {
                            p.sel = i;
                        }
                        let mut items = this.palette_items(cx);
                        if i < items.len() {
                            let (_, _, _, run) = items.swap_remove(i);
                            this.close_palette(window, cx);
                            match run {
                                PRun::Action(a) => window.dispatch_action(a, cx),
                                PRun::Song(j) => this.select_project(j, cx),
                            }
                        }
                    })),
            );
        }
        let query: SharedString = if p.query.is_empty() { "Digite um comando… (fuzzy, sem acento)".into() } else { format!("{}▏", p.query).into() };
        div()
            .id("palette-overlay")
            .absolute()
            .inset_0()
            .bg(Theme::alpha(th.ink, 0.18))
            .flex()
            .justify_center()
            .pt(px(70.0))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.close_palette(window, cx)))
            .child(
                div()
                    .id("palette")
                    .key_context("Palette")
                    .track_focus(&self.palette_focus)
                    .on_key_down(cx.listener(Self::palette_key))
                    // Ctrl+K Ctrl+T etc.: fecham a paleta e seguem para o handler da raiz
                    .on_action(cx.listener(|this, _: &keys::view::ToggleTheme, w, cx| {
                        this.close_palette(w, cx);
                        cx.propagate();
                    }))
                    .on_action(cx.listener(|this, _: &keys::keys::ToggleVim, w, cx| {
                        this.close_palette(w, cx);
                        cx.propagate();
                    }))
                    .on_action(cx.listener(|this, _: &keys::transport::ToggleLoop, w, cx| {
                        this.close_palette(w, cx);
                        cx.propagate();
                    }))
                    .on_action(cx.listener(|this, _: &keys::transport::ToggleMetronome, w, cx| {
                        this.close_palette(w, cx);
                        cx.propagate();
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .w(px(600.0))
                    .max_h(px(560.0))
                    .p(px(10.0))
                    .rounded(px(12.0))
                    .bg(th.paper)
                    .border_1()
                    .border_color(th.line_strong)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .h(px(38.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .rounded(px(8.0))
                            .bg(th.paper_raise)
                            .text_size(px(14.0))
                            .text_color(if p.query.is_empty() { th.ink_faint } else { th.ink })
                            .child(SharedString::from(format!("›  {query}"))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.0))
                            .items_center()
                            .text_size(px(10.5))
                            .text_color(th.ink_faint)
                            .child("também:")
                            .child(kbd("Ctrl K Ctrl T", th))
                            .child("tema")
                            .child(kbd("Ctrl K Ctrl V", th))
                            .child("vim")
                            .child(kbd("Ctrl K Ctrl L", th))
                            .child("loop")
                            .child(kbd("Ctrl K Ctrl M", th))
                            .child("metrônomo")
                            .child(kbd("Esc", th))
                            .child("fechar"),
                    )
                    .child(list)
                    .child(div().text_size(px(10.5)).text_color(th.ink_faint).child(SharedString::from(format!("{total} comandos · ↑↓ escolher · Enter executar")))),
            )
    }

    fn hints_view(&self, h: &HintSession, th: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        // células (geometria do frame anterior) + elementos (registrados neste frame)
        let g = self.grid.read(cx);
        let cells: Vec<(Cur, Bounds<Pixels>)> = if self.page == Page::Editor && self.view == View::Grade {
            match h.scope {
                HintScope::All => g.visible_cells(true, false),
                HintScope::Track => g.visible_cells(false, true),
                HintScope::Cells => g.visible_cells(false, false),
            }
        } else {
            Vec::new()
        };
        let cursor_pt = g.cursor.and_then(|c| cells.iter().find(|(cc, _)| *cc == c).map(|(_, b)| b.center()));
        let grid_w = self.grid.downgrade();
        let scope = h.scope;
        let typed = h.typed.clone();
        let labels = h.labels.clone();
        let editor_vb = self.scroll.bounds();
        let th = *th;

        let paint = canvas(
            |_, _, _| {},
            move |vb, _, window, cx| {
                // 1ª pintura da sessão: monta os rótulos
                if labels.borrow().is_empty() {
                    let mut targets: Vec<(Bounds<Pixels>, HintAct, bool)> = Vec::new();
                    if scope == HintScope::All {
                        keys::HINTS.with(|reg| {
                            for t in reg.borrow().targets.values() {
                                let c = t.bounds.center();
                                let in_window = vb.contains(&c);
                                let in_editor_x = c.x >= editor_vb.origin.x && c.x <= editor_vb.origin.x + editor_vb.size.width;
                                let off_editor = in_editor_x && (c.y < editor_vb.origin.y || c.y > editor_vb.origin.y + editor_vb.size.height);
                                if in_window && !off_editor && f32::from(t.bounds.size.width) > 0.0 {
                                    targets.push((t.bounds, t.act.clone(), false));
                                }
                            }
                        });
                    }
                    for (c, b) in &cells {
                        let c = *c;
                        let gw = grid_w.clone();
                        let act: HintAct = Rc::new(move |_, cx: &mut App| {
                            let _ = gw.update(cx, |gr, cx| {
                                gr.commit_edit_in_place(cx);
                                gr.cursor = Some(c);
                                if !matches!(gr.vmode, VMode::Visual | VMode::VLine) {
                                    gr.anchor = None;
                                }
                                gr.ensure_visible(c);
                                cx.notify();
                            });
                        });
                        targets.push((*b, act, true));
                    }
                    // mais perto do cursor (ou do centro) = rótulo mais curto
                    let origin = cursor_pt.unwrap_or(vb.center());
                    let dist = |b: &Bounds<Pixels>| {
                        let c = b.center();
                        let (dx, dy) = (f32::from(c.x - origin.x), f32::from(c.y - origin.y));
                        dx * dx + dy * dy * 2.0
                    };
                    targets.sort_by(|a, b| dist(&a.0).total_cmp(&dist(&b.0)));
                    let alpha: Vec<char> = keys::HINT_ALPHABET.chars().collect();
                    let names = keys::labels(targets.len(), &alpha);
                    *labels.borrow_mut() = targets.into_iter().zip(names).map(|((bounds, act, cell), label)| HintLabel { label, bounds, act, cell }).collect();
                }
                for l in labels.borrow().iter() {
                    if !l.label.starts_with(&typed) {
                        continue;
                    }
                    let text: SharedString = l.label.to_uppercase().into();
                    let mut run_done = window.text_style().to_run(typed.len());
                    run_done.color = Theme::alpha(th.on_accent, 0.45);
                    run_done.font.weight = FontWeight::BOLD;
                    let mut run_rest = window.text_style().to_run(text.len() - typed.len());
                    run_rest.color = th.on_accent;
                    run_rest.font.weight = FontWeight::BOLD;
                    let runs: Vec<_> = [run_done, run_rest].into_iter().filter(|r| r.len > 0).collect();
                    let line = window.text_system().shape_line(text, px(11.0), &runs, None);
                    let w = f32::from(line.width) + 8.0;
                    let x = f32::from(l.bounds.origin.x).max(f32::from(vb.origin.x));
                    let y = f32::from(l.bounds.origin.y).max(f32::from(vb.origin.y));
                    let r = Bounds::new(point(px(x), px(y)), size(px(w), px(16.0)));
                    let bg = if l.cell { th.accent2 } else { th.accent };
                    window.paint_quad(quad(r, px(4.0), bg, px(1.0), th.paper, gpui_kit::BorderStyle::Solid));
                    let _ = line.paint(point(px(x + 4.0), px(y + 1.0)), px(14.0), TextAlign::Left, None, window, cx);
                }
            },
        )
        .absolute()
        .inset_0();

        div()
            .id("hints")
            .absolute()
            .inset_0()
            .key_context("Hints")
            .track_focus(&self.hint_focus)
            .on_key_down(cx.listener(Self::hint_key))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.stop_hints(window, cx)))
            .bg(Theme::alpha(th.ink, 0.06))
            .child(deferred(paint).with_priority(200))
    }
}

fn samples_dir() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../public/samples")
}

fn main() {
    // pede ao PipeWire um quantum pequeno p/ este cliente (o padrão do grafo aqui é 1024 = 21 ms,
    // somado ao buffer do ALSA). Feito antes de qualquer thread existir.
    if std::env::var_os("PIPEWIRE_LATENCY").is_none() {
        // SAFETY: ainda single-threaded (início do main, antes do GPUI/cpal).
        unsafe { std::env::set_var("PIPEWIRE_LATENCY", "256/48000") };
    }
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let bench = args.iter().any(|a| a == "--bench").then(|| arg("--bench").and_then(|s| s.parse().ok()).unwrap_or(15));
    let mute = args.iter().any(|a| a == "--mute");
    let dark = args.iter().any(|a| a == "--dark");
    // p/ screenshots: --demo palette|hints|whichkey|visual
    let demo = arg("--demo");
    let db = arg("--db").map(PathBuf::from).unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/wadawko.db"));

    // `--render "Título" saida.wav`: render offline (sem janela) com o mesmo motor
    if let Some(i) = args.iter().position(|a| a == "--render") {
        let title = args.get(i + 1).cloned().unwrap_or_default();
        let out = PathBuf::from(args.get(i + 2).cloned().unwrap_or_else(|| "render.wav".into()));
        let (projects, cur) = model::load_workspace(&db).expect("ler banco");
        let song = projects.iter().find(|p| p.song.title == title).map(|p| &p.song).unwrap_or(&projects[cur].song);
        if out.to_string_lossy().ends_with(".json") {
            std::fs::write(&out, serde_json::to_string_pretty(&song.to_json()).expect("json")).expect("gravar json");
            println!("{} → {}", song.title, out.display());
            return;
        }
        let bank = sound::load_bank(&samples_dir()).expect("samples");
        let sr = export::EXPORT_SR;
        let sched = Arc::new(sound::compile(song, &bank, sr, false));
        let frames = export::render(bank.samples.clone(), sched, song.tempo);
        let peak = frames.iter().flat_map(|f| f.iter()).fold(0f32, |a, s| a.max(s.abs()));
        let rms = (frames.iter().flat_map(|f| f.iter()).map(|s| s * s).sum::<f32>() / (frames.len() * 2) as f32).sqrt();
        export::write_audio(&out, &frames, &song.title).expect("gravar áudio");
        println!("{} → {} ({:.1} s) pico {:.2} dBFS · RMS {:.1} dBFS", song.title, out.display(), frames.len() as f32 / sr as f32, 20.0 * peak.log10(), 20.0 * rms.log10());
        return;
    }

    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        let fonts: Vec<Cow<'static, [u8]>> = vec![
            Cow::Borrowed(include_bytes!("../assets/fonts/ZenKakuGothicNew-Regular.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/ZenKakuGothicNew-Medium.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/ZenKakuGothicNew-Bold.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/ShipporiMincho-Bold.ttf")),
        ];
        cx.text_system().add_fonts(fonts).expect("registrar fontes");
        let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
        let win = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui_kit::TitlebarOptions { title: Some("WaDAWko".into()), ..Default::default() }),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    let s = Spike::new(window, cx, db.clone(), bench, mute);
                    if dark {
                        s.theme.set(Theme::dark());
                    }
                    s
                })
            },
        )
        .expect("abrir janela");
        if demo.as_deref() == Some("enter") {
            let w = win.0;
            cx.spawn(async move |cx| {
                cx.background_executor().timer(Duration::from_millis(2000)).await;
                let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::transport::PlayHere), cx));
                cx.background_executor().timer(Duration::from_millis(300)).await;
                let a = RENDERS.load(Ordering::Relaxed);
                let ua = RENDER_US.load(Ordering::Relaxed);
                RENDER_MAX_US.store(0, Ordering::Relaxed);
                let (pa, pm) = { let st = STATS_SNAP.with(|s| s.get()); st };
                cx.background_executor().timer(Duration::from_millis(2000)).await;
                let b = RENDERS.load(Ordering::Relaxed);
                let ub = RENDER_US.load(Ordering::Relaxed);
                let (pb, pmb) = STATS_SNAP.with(|s| s.get());
                let n = (b - a).max(1);
                println!(
                    "TESTE enter→play: {} renders em 2 s (≈{:.0} fps; 0 = tela congelada) · render da raiz: média {:.2} ms, máx {:.2} ms · paint da grade: {} vezes, máx {:.2} ms",
                    b - a,
                    (b - a) as f32 / 2.0,
                    (ub - ua) as f64 / n as f64 / 1000.0,
                    RENDER_MAX_US.load(Ordering::Relaxed) as f64 / 1000.0,
                    pb - pa,
                    pmb.max(pm),
                );
                println!(
                    "  follow horizontal: {} · follow vertical (set_offset): {} · renders da grade: {}",
                    DBG_FOLLOW_H.load(Ordering::Relaxed),
                    DBG_FOLLOW_V.load(Ordering::Relaxed),
                    DBG_GRID_RENDERS.load(Ordering::Relaxed)
                );
                let _ = cx.update(|cx| cx.quit());
            })
            .detach();
        }
        if let Some(demo) = demo.clone() {
            let w = win.0;
            cx.spawn(async move |cx| {
                cx.background_executor().timer(Duration::from_millis(1500)).await;
                let _ = w.update(cx, |_, window, cx| {
                    let _ = win.1.update(cx, |s: &mut Spike, cx| match demo.as_str() {
                        "palette" => {
                            s.open_palette(window, cx);
                            if let Some(p) = s.palette.as_mut() {
                                p.query = "metr".into();
                            }
                        }
                        "hints" => s.start_hints(HintScope::All, window, cx),
                        "play" => {
                            s.send_play();
                            cx.notify();
                        }
                        "roothints" => {
                            s.root_hints = true;
                            cx.notify();
                        }
                        "cmdline" => {
                            s.open_prompt(extra::PromptKind::Command, window, cx);
                            if let Some(p) = s.prompt.as_mut() {
                                p.text = "bpm 140".into();
                            }
                        }
                        "config" => {
                            s.page = Page::Config;
                            s.note_key("ctrl-space".into(), Some("hints::Jump".into()));
                            cx.notify();
                        }
                        "whichkey" => {
                            s.pending = vec![Keystroke::parse("space").unwrap()];
                            s.pending_since = Instant::now() - Duration::from_secs(1);
                            cx.notify();
                        }
                        "visual" => {
                            s.grid.update(cx, |g, cx| {
                                g.cursor = Some(Cur { m: 0, t: 2, k: 1 });
                                g.anchor = Some(Cur { m: 0, t: 1, k: 1 });
                                g.vmode = VMode::Visual;
                                g.motion(Motion::NextBeat, 2, true, cx);
                            });
                            s.count = Some(3);
                        }
                        _ => {}
                    });
                });
            })
            .detach();
        }
        cx.activate(true);
    });
}
