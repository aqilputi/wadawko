//! Shell: a view raiz. Dona do foco, do estado de teclado (vim, contagem, paleta,
//! hints, prompt) e da página/aba atual; monta NavRail + topbar + página + painel
//! de músicas + status bar e os overlays (playhead/flash, which-key, prompt,
//! paleta de comandos, hints).
//!
//! A música, o histórico, o áudio e o transporte NÃO moram aqui: estão em
//! `Entity<AppState>` (`state.rs`).

mod actions;
mod fxedit;
pub(crate) use actions::tap_bpm;
pub mod chrome;
pub mod io;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use gpui_kit::{App, Bounds, Context, Entity, FocusHandle, KeyContext, KeyDownEvent, Keystroke, Pixels, Render, ScrollHandle, Window, canvas, div, fill, point, prelude::*, px, quad, size};
use wadawko_core::{Song, Zone, phrase};

use crate::keys::{self, HintAct};
use crate::song_ext::SongExt;
use crate::state::AppState;
use crate::stats::Stats;
use crate::theme::Theme;
use crate::views::grid::{self, Cur, GridEvent, GridView, InputTarget, Lane, Layout, Mode, Motion, VMode};
use crate::views::{Page, Tab, Views};

thread_local! {
    /// (nº de paints da grade, máx ms) — lido pelos modos de teste
    pub static STATS_SNAP: Cell<(u64, f32)> = const { Cell::new((0, 0.0)) };
}

/// quantos renders da raiz já aconteceram (teste de "a tela para de atualizar")
pub static RENDERS: AtomicU64 = AtomicU64::new(0);
pub static DBG_FOLLOW_H: AtomicU64 = AtomicU64::new(0);
pub static DBG_FOLLOW_V: AtomicU64 = AtomicU64::new(0);
pub static DBG_GRID_RENDERS: AtomicU64 = AtomicU64::new(0);
/// soma (µs) do tempo gasto no render da raiz e o máximo
pub static RENDER_US: AtomicU64 = AtomicU64::new(0);
/// tempo total (µs) gasto em `GridView::render` (diagnóstico do `--demo hoverstat`)
pub static GRID_RENDER_US: AtomicU64 = AtomicU64::new(0);
/// renders de seções da grade (`SectionView`) e tempo total em µs
pub static SEC_RENDERS: AtomicU64 = AtomicU64::new(0);
pub static SEC_RENDER_US: AtomicU64 = AtomicU64::new(0);
pub static RENDER_MAX_US: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq)]
pub enum HintScope {
    /// elementos da UI + inícios de tempo
    All,
    /// células visíveis da linha do cursor
    Track,
    /// todas as células visíveis
    Cells,
}

pub struct HintLabel {
    pub label: String,
    pub bounds: Bounds<Pixels>,
    pub act: HintAct,
    pub cell: bool,
}

pub struct HintSession {
    pub scope: HintScope,
    pub typed: String,
    pub labels: Rc<RefCell<Vec<HintLabel>>>,
}

pub struct Palette {
    pub query: String,
    pub sel: usize,
}

pub struct Bench {
    until: Instant,
    started: Instant,
    last_zoom: Instant,
    zoom_in: bool,
    warmed: bool,
}

/// Onde desenhar o playhead: `pos` é x (grade horizontal) ou y (editor vertical, topo
/// da linha audível), `b` o canvas da seção e `row_h` a altura da linha (vertical).
#[derive(Clone, Copy)]
pub struct PlayheadAt {
    pub pos: f32,
    pub b: Bounds<Pixels>,
    pub vertical: bool,
    pub row_h: f32,
    /// vertical: largura ocupada (gutter + compasso + colunas), para a linha não passar das colunas
    pub w: f32,
}

/// Célula soando agora: bounds, intensidade (1 = golpe acabou de soar → 0) e se é borda.
pub struct Flash {
    pub bounds: Bounds<Pixels>,
    pub strength: f32,
    pub rim: bool,
}

pub struct Shell {
    pub state: Entity<AppState>,
    pub grid: Entity<GridView>,
    pub views: Views,
    pub focus: FocusHandle,
    pub palette_focus: FocusHandle,
    /// moldura em cache (barra de cima, lateral, painel de músicas, barra de contexto,
    /// status bar) — ver `chrome.rs`; índices em `chrome::Part`
    pub chrome: [Entity<chrome::Chrome>; 7],
    /// o que a status bar mostra (recalculado a cada frame; ela redesenha quando muda)
    pub status: crate::views::status_bar::StatusModel,
    /// altura medida da barra de contexto (0 = medir no próximo frame, sem cache)
    ctx_h: Rc<std::cell::Cell<f32>>,
    /// física do balanço do bloco arrastado (notas)
    pub swing: Rc<RefCell<crate::views::drag_ghost::Swing>>,
    pub hint_focus: FocusHandle,
    pub prompt_focus: FocusHandle,
    pub scroll: ScrollHandle,
    pub stats: Rc<RefCell<Stats>>,
    pub page: Page,
    pub tab: Tab,
    pub nav_open: bool,
    /// id da def de instrumento que "+ linha" cria
    pub add_instrument: String,
    pub bench: Option<Bench>,
    pub hud: String,
    last_hud: Instant,
    // teclado
    pub vim_on: bool,
    pub count: Option<u32>,
    pub palette: Option<Palette>,
    pub hints: Option<HintSession>,
    pub pending: Vec<Keystroke>,
    pub pending_since: Instant,
    /// instantes das últimas batidas do tap tempo (F8)
    pub taps: Vec<Instant>,
    /// modo projeção aberto (F7d); `true` no 2º campo = fomos nós que pusemos em tela cheia
    pub director: Option<bool>,
    /// pedido de sair do modo projeção (o Esc não tem a janela; o render fecha)
    pub director_close: bool,
    /// janela estreita: a barra de cima compacta (ver `views::topbar`)
    pub top_narrow: bool,
    /// painel de seções à esquerda (tracker §3)
    pub sec_panel_open: bool,
    /// aba do painel da esquerda: seções ou linhas (instrumentos)
    pub left_tab: crate::views::tracks_panel::LeftTab,
    pub sec_drag: Option<crate::views::sections_panel::SecDrag>,
    pub sec_menu: Option<crate::views::sections_panel::SecMenu>,
    /// última seção clicada (âncora do Shift+clique)
    pub sec_anchor: Option<String>,
    /// prompt pedido de um listener sem janela; o render abre
    pub pending_ask: Option<(InputTarget, String)>,
    /// minimap ligado (`view::ToggleMinimap`)
    pub mini_open: bool,
    /// linha sob o mouse no último render (o overlay redesenha quando muda)
    pub hover_band_key: Option<i32>,
    /// a grade segue o playhead tocando (`view::ToggleFollow`)
    pub follow_on: bool,
    /// seguir centrado (linha fixa a 40 %) em vez de virar a página (`view::ToggleFollowCenter`)
    pub follow_center: bool,
    /// minimap: geometria do último desenho e arrasto em curso
    pub mini_geom: Rc<Cell<crate::views::minimap::MiniGeom>>,
    pub mini_drag: bool,
    /// cursor/âncora do último render: mudou → a grade acompanha (revela o cursor)
    pub last_sel: Option<(Option<Cur>, Option<Cur>)>,
    /// ponteiro durante o arrasto de seleção na grade (rolagem automática nas bordas)
    pub drag_ptr: Rc<Cell<Option<gpui_kit::Point<Pixels>>>>,
    pub prompt: Option<keys::modal::Prompt>,
    pub last_change: Option<keys::modal::Change>,
    pub recording: bool,
    pub marks: HashMap<char, Cur>,
    pub last_search: Option<(String, bool)>,
    pub cmd_history: Vec<String>,
    /// última tecla recebida (página Configurações → testar tecla)
    pub last_key: Option<(String, Option<String>, Instant)>,
    /// Ctrl+/: dica de atalhos na raiz
    pub root_hints: bool,
}

impl Shell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, state: Entity<AppState>, bench_secs: Option<u64>) -> Self {
        // keymap (padrão + usuário)
        let errors = keys::load_keymap(cx);
        for e in &errors {
            eprintln!("keymap: {e}");
        }
        if !errors.is_empty() {
            state.update(cx, |s, _| s.flash(format!("keymap: {}", errors.join(" · "))));
        }
        let settings = keys::load_settings();
        let vim_on = settings.vim;
        state.update(cx, |s, _| {
            s.av_offset_ms = settings.av_offset_ms;
            s.tp.metro = settings.metro;
            s.tp.count_in_loop = settings.count_in_loop;
        });

        let stats = Rc::new(RefCell::new(Stats::default()));
        let grid = cx.new(|cx| {
            let mut g = GridView::new(state.clone(), stats.clone(), vim_on, cx);
            g.cell_h = settings.row_h;
            g.layout = settings.layout;
            g.row_h = settings.tracker_row_h;
            g.col_w = settings.tracker_col_w;
            g.col_w_eff = settings.tracker_col_w;
            g
        });
        cx.subscribe_in(&grid, window, |this: &mut Self, _g, ev: &GridEvent, window, cx| match ev {
            GridEvent::Ask(target, initial) => this.ask(target.clone(), initial.clone(), window, cx),
            GridEvent::SettingsChanged => this.save_settings(cx),
        })
        .detach();
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let views = Views::new(&state, cx);

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
            this.last_key = Some((k, a.clone(), Instant::now()));
            if this.root_hints && a.as_deref() != Some("keys::ToggleRootHints") {
                this.root_hints = false;
            }
            cx.notify();
        })
        .detach();

        let focus = cx.focus_handle();
        focus.focus(window, cx);

        let bench = bench_secs.map(|s| Bench { until: Instant::now() + Duration::from_secs(s + 2), started: Instant::now(), last_zoom: Instant::now(), zoom_in: true, warmed: false });
        Shell {
            state,
            grid,
            views,
            focus,
            palette_focus: cx.focus_handle(),
            swing: Rc::new(RefCell::new(Default::default())),
            chrome: {
                let me = cx.weak_entity();
                [chrome::Part::Top, chrome::Part::Nav, chrome::Part::Panel, chrome::Part::Context, chrome::Part::Status, chrome::Part::Sections, chrome::Part::Minimap]
                    .map(|p| cx.new(|_| chrome::Chrome::new(me.clone(), p)))
            },
            status: Default::default(),
            ctx_h: Rc::new(std::cell::Cell::new(0.0)),
            hint_focus: cx.focus_handle(),
            prompt_focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            stats,
            page: Page::Editor,
            tab: Tab::Grade,
            nav_open: true,
            add_instrument: "chu".into(),
            bench,
            hud: String::new(),
            last_hud: Instant::now(),
            vim_on,
            count: None,
            palette: None,
            hints: None,
            pending: Vec::new(),
            pending_since: Instant::now(),
            taps: Vec::new(),
            director: None,
            director_close: false,
            top_narrow: false,
            sec_panel_open: true,
            left_tab: Default::default(),
            sec_drag: None,
            sec_menu: None,
            sec_anchor: None,
            pending_ask: None,
            mini_geom: Default::default(),
            mini_drag: false,
            mini_open: true,
            hover_band_key: None,
            follow_on: true,
            follow_center: false,
            last_sel: None,
            drag_ptr: Default::default(),
            prompt: None,
            last_change: None,
            recording: false,
            marks: HashMap::new(),
            last_search: None,
            cmd_history: Vec::new(),
            last_key: None,
            root_hints: false,
        }
    }

    // ───────── atalhos para o estado ─────────

    pub fn song(&self, cx: &App) -> Arc<Song> {
        self.state.read(cx).song().clone()
    }

    /// Calcula a chave de cada peça da moldura (tudo que ela mostra) e notifica
    /// só as que mudaram — o resto é reaproveitado do frame anterior.
    fn refresh_chrome(&mut self, playing: bool, viewport_w: f32, cx: &mut Context<Self>) {
        use chrome::hash_into as h;
        let st = self.state.read(cx);
        let dark = st.theme.dark;
        let rev = st.revision;
        let current = st.store.current_id().to_string();
        let export = st.export_label();
        let keys = [
            // barra de cima: inclui a posição do display (muda de célula em célula tocando)
            chrome::key(|k| {
                h(k, &(dark, playing, rev, &current, st.playback_bpm.to_bits(), st.sounding_bpm.to_bits(), self.top_narrow));
                h(k, &format!("{:?}", st.tp));
                h(k, &self.transport_pos(cx));
            }),
            chrome::key(|k| h(k, &(dark, format!("{:?}", self.page)))),
            // painel: músicas + tudo que o INSPECTOR mostra (cursor, seleção, região, caneta)
            chrome::key(|k| {
                h(k, &(dark, rev, &current, &export, self.status.pos_text()));
                h(k, &(&st.tp.selected, format!("{:?}", st.tp.range), &st.pen, self.inspector_key(cx)));
                for p in st.store.projects() {
                    h(k, &(&p.id, &p.title, p.updated_at));
                }
            }),
            // barra de contexto: tudo que ela lê + a largura (quebra de linha muda a altura)
            chrome::key(|k| {
                let g = self.grid.read(cx);
                let beats = g.cursor.and_then(|c| st.song().measures.get(c.m)).map(|m| m.beats);
                h(k, &(dark, rev, &st.pen, &st.tp.selected, self.vim_on, &self.add_instrument, format!("{:?}", st.tp.range)));
                h(k, &(g.mode, g.zoom_pct(), g.anchor.is_some(), beats, viewport_w.round() as i32));
            }),
            chrome::key(|k| h(k, &(dark, &self.status))),
            chrome::key(|k| self.sections_panel_key(k, cx)),
            chrome::key(|k| self.minimap_key(k, cx)),
        ];
        for (i, (view, key)) in self.chrome.iter().zip(keys).enumerate() {
            view.update(cx, |c, cx| {
                if c.key != key {
                    c.key = key;
                    cx.notify();
                    if i == chrome::Part::Context as usize {
                        // conteúdo novo pode ter outra altura: mede de novo
                        self.ctx_h.set(0.0);
                    }
                }
            });
        }
    }

    /// Força a moldura a re-renderizar (ex.: Ctrl+Space precisa que os botões
    /// dela se registrem de novo como alvos de salto).
    pub fn invalidate_chrome(&mut self, cx: &mut Context<Self>) {
        for view in &self.chrome {
            view.update(cx, |c, cx| {
                c.key = 0;
                cx.notify();
            });
        }
    }

    pub fn theme(&self, cx: &App) -> Theme {
        self.state.read(cx).theme
    }

    pub fn playing(&self, cx: &App) -> bool {
        self.state.read(cx).playing()
    }

    pub fn flash(&mut self, msg: impl Into<String>, cx: &mut Context<Self>) {
        let msg = msg.into();
        self.state.update(cx, |s, cx| {
            s.flash(msg);
            cx.notify();
        });
    }

    /// Edição disparada pelo Shell (ações de seção/linha/música): mesmo caminho único.
    pub fn edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Song)) -> bool {
        let r = self.state.update(cx, |s, cx| s.edit(cx, f));
        cx.notify();
        r
    }

    pub fn reload_keymap(&mut self, cx: &mut Context<Self>) {
        let errors = keys::load_keymap(cx);
        let msg = if errors.is_empty() { "atalhos recarregados".to_string() } else { format!("keymap: {}", errors.join(" · ")) };
        self.flash(msg, cx);
    }

    pub fn save_settings(&self, cx: &App) {
        let st = self.state.read(cx);
        let g = self.grid.read(cx);
        let _ = keys::save_settings(keys::Settings {
            vim: self.vim_on,
            av_offset_ms: st.av_offset_ms,
            metro: st.tp.metro,
            count_in_loop: st.tp.count_in_loop,
            row_h: g.cell_h,
            layout: g.layout,
            tracker_row_h: g.row_h,
            tracker_col_w: g.col_w,
        });
    }

    // ───────── transporte / projeto ─────────

    pub fn toggle_play(&mut self, here: bool, cx: &mut Context<Self>) {
        let cur = self.grid.read(cx).cursor;
        let song = self.song(cx);
        self.state.update(cx, |s, cx| {
            if s.playing() {
                s.stop(cx);
            } else {
                if here && let Some(c) = cur {
                    s.start_cell = song.flat(c.m, c.k);
                }
                let from = s.start_cell;
                s.play(from, cx);
            }
        });
        cx.notify();
    }

    pub fn set_theme(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| s.set_theme(dark, cx));
        cx.notify();
    }

    pub fn set_vim(&mut self, on: bool, cx: &mut Context<Self>) {
        self.vim_on = on;
        self.count = None;
        self.save_settings(cx);
        self.grid.update(cx, |g, cx| {
            g.commit_edit_in_place(cx);
            g.anchor = None;
            g.vmode = if on { VMode::Normal } else { VMode::Simple };
            cx.notify();
        });
        self.flash(if on { "vim ligado" } else { "vim desligado (modo simples)" }, cx);
    }

    pub fn select_project(&mut self, i: usize, cx: &mut Context<Self>) {
        let id = self.state.read(cx).store.projects().get(i).map(|p| p.id.clone());
        if let Some(id) = id {
            self.state.update(cx, |s, cx| s.switch_project(&id, cx));
            self.scroll.set_offset(point(px(0.0), px(0.0)));
        }
        cx.notify();
    }

    /// Abre o prompt de texto para renomear/definir algo (pedido pela grade ou por uma ação).
    /// Como [`Self::ask`], para listeners sem a janela: o próximo render abre o prompt.
    pub fn ask_later(&mut self, target: InputTarget, initial: String, cx: &mut Context<Self>) {
        self.pending_ask = Some((target, initial));
        cx.notify();
    }

    pub fn ask(&mut self, target: InputTarget, initial: String, window: &mut Window, cx: &mut Context<Self>) {
        self.open_prompt(keys::modal::PromptKind::Input(target), window, cx);
        if let Some(p) = self.prompt.as_mut() {
            p.text = initial;
        }
    }

    // ───────── grade ─────────

    pub fn take_count(&mut self) -> usize {
        self.count.take().map(|c| c as usize).unwrap_or(1)
    }

    pub fn mv(&mut self, mo: Motion, extend: bool, cx: &mut Context<Self>) {
        let n = self.take_count();
        self.grid.update(cx, |g, cx| {
            let ext = extend || matches!(g.vmode, VMode::Visual | VMode::VLine);
            g.motion(mo, n, ext, cx);
        });
        cx.notify();
    }

    pub fn g<R>(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut GridView, &mut Context<GridView>, usize) -> R) -> R {
        let n = self.take_count();
        let r = self.grid.update(cx, |g, cx| f(g, cx, n));
        cx.notify();
        r
    }

    /// Fim de operação em visual: volta ao normal (vim) e limpa a seleção.
    pub fn end_visual(g: &mut GridView) {
        g.anchor = None;
        if matches!(g.vmode, VMode::Visual | VMode::VLine) {
            g.vmode = VMode::Normal;
        }
    }

    pub fn cursor_section(&self, cx: &App) -> usize {
        let c = self.grid.read(cx).cursor.unwrap_or(Cur { m: 0, t: 0, k: 0 });
        self.song(cx).section_of(c.m)
    }

    /// id da seção do cursor (None = bloco virtual, sem seção)
    pub fn cursor_section_id(&self, cx: &App) -> Option<String> {
        let s = self.cursor_section(cx);
        self.song(cx).secs().get(s).and_then(|x| x.id.clone())
    }

    pub fn cursor_track(&self, cx: &App) -> usize {
        self.grid.read(cx).cursor.map(|c| c.t).unwrap_or(0)
    }

    pub fn cursor_track_id(&self, cx: &App) -> Option<String> {
        self.song(cx).tracks.get(self.cursor_track(cx)).map(|t| t.id.clone())
    }

    // ───────── digitação (o que nenhum atalho pegou) ─────────

    fn on_key(&mut self, ev: &KeyDownEvent, _w: &mut Window, cx: &mut Context<Self>) {
        if self.page != Page::Editor || self.tab != Tab::Grade {
            if ev.keystroke.key == "space" && !self.vim_on {
                self.toggle_play(false, cx);
            }
            return;
        }
        let k = ev.keystroke.key.as_str();
        let m = ev.keystroke.modifiers;
        let (vmode, mode, editing, lane) = {
            let g = self.grid.read(cx);
            (g.vmode, g.mode, !g.edit.is_empty(), g.lane)
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
                    && ch.chars().all(|c| c.is_alphabetic() || c == '!' || (lane == Lane::Kyai && matches!(c, '-' | 'ー' | '~')))
                {
                    let ch = ch.to_string();
                    self.grid.update(cx, |g, cx| {
                        if g.cursor.is_some() {
                            // começou a digitar: a seleção (do arrasto) sai
                            g.anchor = None;
                            g.edit.push_str(&ch);
                            cx.notify();
                        }
                    });
                }
            }
        }
        cx.notify();
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.sec_menu = None;
        self.sec_drag = None;
        if self.director.is_some() {
            self.director_close = true;
        }
        self.count = None;
        self.root_hints = false;
        self.grid.update(cx, |g, cx| {
            g.edit.clear();
            g.anchor = None;
            g.menu = None;
            g.cancel_drag();
            if matches!(g.vmode, VMode::Visual | VMode::VLine | VMode::Insert) {
                g.vmode = VMode::Normal;
            }
            cx.notify();
        });
        cx.notify();
    }

    // ───────── playhead / flash / follow / bench ─────────

    pub(crate) fn on_grid(&self) -> bool {
        self.page == Page::Editor && self.tab == Tab::Grade
    }

    /// Posição do playhead (x na grade horizontal; y no editor vertical) e o canvas da
    /// seção — tocando, a posição audível; parado, a célula de onde o play vai começar.
    pub(crate) fn playhead(&self, cx: &App) -> Option<PlayheadAt> {
        let st = self.state.read(cx);
        let song = st.song();
        let (m, frac) = if st.playing() {
            let p = st.playhead()?;
            (p.mi, p.frac)
        } else {
            let (m, k) = song.unflat(st.start_cell.min(song.total_cells().saturating_sub(1)));
            (m, k as f32 / song.mlen(m).max(1) as f32)
        };
        if m >= song.measures.len() {
            return None;
        }
        let s = song.section_of(m);
        let (a, _) = song.section_range(s);
        let grid = self.grid.read(cx);
        // offset ATUAL da grade: o `geom.offset` só é escrito quando a grade
        // re-renderiza (um frame depois), e usar ele fazia o follow aplicar o
        // deslocamento duas vezes (pulo duplo + 3 re-renders por virada de página)
        let offset = grid.offset;
        let g = grid.geom.clone();
        let g = g.borrow();
        let b = *g.sections.get(s)?;
        if f32::from(b.size.width) <= 0.0 {
            return None;
        }
        if grid.layout == Layout::Vertical {
            // topo da linha (célula) audível — o playhead anda de linha em linha
            let len = song.mlen(m).max(1);
            let k = ((frac * len as f32).floor() as usize).min(len - 1);
            let y = f32::from(b.origin.y) + grid::measure_y(song, s, m, g.row_h)? + k as f32 * g.row_h;
            let w = (grid::LEFT_W + g.cols.w - g.offset).min(f32::from(b.size.width));
            return Some(PlayheadAt { pos: y, b, vertical: true, row_h: g.row_h, w });
        }
        let xs = grid::measure_xs(song, s, g.cw);
        let x = f32::from(b.origin.x) + xs.get(m.checked_sub(a)?)? + frac * song.mlen(m) as f32 * g.cw - offset;
        Some(PlayheadAt { pos: x, b, vertical: false, row_h: 0.0, w: 0.0 })
    }

    /// Células que acabaram de soar (eventos `HitFired` do engine, já atrasados
    /// pela latência de saída) + rótulo da posição para a status bar.
    fn playing_cells(&self, cx: &App) -> (Vec<Flash>, Option<String>) {
        let st = self.state.read(cx);
        if !st.playing() {
            return (Vec::new(), None);
        }
        let song = st.song();
        let label = st.playhead().and_then(|p| p.cell).map(|g| {
            let (m, k) = song.unflat(g);
            let sub = song.measures[m].subdivision.max(1);
            format!("c{} · {}.{}", m + 1, k / sub + 1, k % sub + 1)
        });
        let grid = self.grid.read(cx);
        let now = Instant::now();
        let dur = st.flash_ms() / 1000.0;
        let mut out = Vec::new();
        for h in &st.hits {
            if now < h.at || h.cell >= song.total_cells() {
                continue;
            }
            let age = now.duration_since(h.at).as_secs_f32();
            if age > dur {
                continue;
            }
            let (m, k) = song.unflat(h.cell);
            let Some(bounds) = grid.cell_bounds(Cur { m, t: h.track, k }) else { continue };
            let (rim, vel) = song.cell(h.track, m, k).and_then(phrase::resolve_cell).and_then(|r| r.hits.get(h.hit).map(|x| (x.zone == Zone::Rim, x.vel as f32))).unwrap_or((false, 0.8));
            out.push(Flash { bounds, strength: (1.0 - age / dur).powf(0.8) * (0.7 + 0.3 * vel.min(1.0)), rim });
        }
        (out, label)
    }

    /// Mini-medidores das linhas (F3): um retângulo de 3 px sob o nome de cada
    /// linha, por seção visível, com o nível/pico já com balística. Desenhados no
    /// overlay (sem cache), nunca dentro da grade.
    fn row_meters(&self, cx: &App) -> Vec<(Bounds<Pixels>, f32, f32)> {
        let st = self.state.read(cx);
        if !st.levels.active() {
            return Vec::new();
        }
        let song = st.song();
        let levels: Vec<crate::state::Level> = (0..song.tracks.len()).map(|t| st.track_level(t)).collect();
        if levels.iter().all(|l| l.level <= 0.0 && l.peak <= 0.0) {
            return Vec::new();
        }
        let grid = self.grid.read(cx);
        let g = grid.geom.borrow();
        if grid.layout == Layout::Vertical {
            // vertical: 3 px no rodapé do cabeçalho de colunas, na largura da nota
            let hb = g.col_head;
            if f32::from(hb.size.width) <= 0.0 {
                return Vec::new();
            }
            let x0 = f32::from(hb.origin.x) + grid::LEFT_W - g.offset;
            let y = f32::from(hb.origin.y) + f32::from(hb.size.height) - 3.0;
            return levels
                .iter()
                .enumerate()
                .filter(|(t, l)| (l.level > 0.0 || l.peak > 0.0) && *t < g.cols.x.len())
                .map(|(t, l)| (Bounds::new(point(px(x0 + g.cols.x[t] + 2.0), px(y)), size(px(g.cols.note_w - 4.0), px(3.0))), l.level, l.peak))
                .filter(|(b, _, _)| f32::from(b.origin.x) >= f32::from(hb.origin.x) + grid::LEFT_W - 2.0)
                .collect();
        }
        let w = (grid::LABEL_W - g.label_pad - 150.0).max(24.0);
        let mut out = Vec::new();
        for b in g.sections.iter().filter(|b| f32::from(b.size.width) > 0.0) {
            let x = f32::from(b.origin.x) - grid::LABEL_W + g.label_pad;
            let top = f32::from(b.origin.y) + grid::SEC_DYN_H + grid::TIMELINE_H;
            for (t, l) in levels.iter().enumerate() {
                if (l.level <= 0.0 && l.peak <= 0.0) || t >= g.rows.y.len() {
                    continue;
                }
                let y = top + g.rows.y[t] + g.rows.cell_h - 4.0;
                out.push((Bounds::new(point(px(x), px(y)), size(px(w), px(3.0))), l.level, l.peak));
            }
        }
        out
    }

    fn follow(&mut self, cx: &mut Context<Self>) {
        if !self.follow_on || !self.playing(cx) || !self.on_grid() {
            return;
        }
        let Some(ph) = self.playhead(cx) else { return };
        if ph.vertical {
            let vb = self.scroll.bounds();
            let (vt, vh) = (f32::from(vb.origin.y), f32::from(vb.size.height));
            // follow centrado (T3b): a linha que soa fica fixa a 40 % da vista e a música
            // rola por baixo; custa um re-render das seções visíveis por frame
            if self.follow_center && vh > 0.0 {
                let cur = self.scroll.offset();
                let max = self.scroll.max_offset();
                let ny = (f32::from(cur.y) - (ph.pos - (vt + vh * 0.4))).clamp(-f32::from(max.y), 0.0);
                if (ny - f32::from(cur.y)).abs() > 0.5 {
                    DBG_FOLLOW_V.fetch_add(1, Ordering::Relaxed);
                    self.scroll.set_offset(point(cur.x, px(ny)));
                }
                return;
            }
            // follow "página": vira a página quando o playhead passa de 85 % da área visível
            if vh > 0.0 && (ph.pos + ph.row_h > vt + vh * 0.85 || ph.pos < vt) {
                let cur = self.scroll.offset();
                let max = self.scroll.max_offset();
                let ny = (f32::from(cur.y) - (ph.pos - vt - vh * 0.1)).clamp(-f32::from(max.y), 0.0);
                if (ny - f32::from(cur.y)).abs() > 0.5 {
                    DBG_FOLLOW_V.fetch_add(1, Ordering::Relaxed);
                    self.scroll.set_offset(point(cur.x, px(ny)));
                }
            }
            return;
        }
        let (x, b) = (ph.pos, ph.b);
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
            if (ny - f32::from(cur.y)).abs() > 0.5 {
                DBG_FOLLOW_V.fetch_add(1, Ordering::Relaxed);
                self.scroll.set_offset(point(cur.x, px(ny)));
            }
        }
    }

    /// Rola a área do editor (vertical) até o cursor quando ele sai da vista.
    pub fn reveal_cursor(&mut self, cx: &mut Context<Self>) {
        let g = self.grid.read(cx);
        let Some(c) = g.cursor else { return };
        let Some(b) = g.cell_bounds(c) else { return };
        let y = f32::from(b.origin.y);
        let vb = self.scroll.bounds();
        let (vt, vbm) = (f32::from(vb.origin.y), f32::from(vb.origin.y) + f32::from(vb.size.height));
        let ch = f32::from(b.size.height);
        if f32::from(vb.size.height) > 0.0 && (y < vt + 10.0 || y + ch > vbm - 10.0) {
            let cur = self.scroll.offset();
            let max = self.scroll.max_offset();
            let target = if y < vt { y - vt - 80.0 } else { y + ch - vbm + 80.0 };
            let ny = (f32::from(cur.y) - target).clamp(-f32::from(max.y), 0.0);
            self.scroll.set_offset(point(cur.x, px(ny)));
        }
    }

    fn drive_bench(&mut self, cx: &mut Context<Self>) {
        if self.bench.is_none() {
            return;
        }
        let playing = self.playing(cx);
        let state = self.state.clone();
        let Some(b) = self.bench.as_mut() else { return };
        let now = Instant::now();
        if !b.warmed && now.duration_since(b.started) > Duration::from_secs(2) {
            b.warmed = true;
            b.started = now;
            self.stats.borrow_mut().reset();
            // o áudio só fica pronto depois da janela: começa a tocar aqui
            if !playing {
                state.update(cx, |s, cx| s.play(0, cx));
            }
        }
        if now >= b.until {
            let secs = now.duration_since(b.started).as_secs_f32();
            let st = self.state.read(cx);
            println!("{}", self.stats.borrow().report(st.audio.as_ref().map(|a| &**a.clock()), secs));
            if let Some(a) = st.audio.as_ref() {
                let s = a.clock().snapshot();
                println!("relógio do áudio      tick {:.0} · tocando {} · voltas {}", s.pos_tick, s.playing, s.wraps);
            }
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

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some((target, initial)) = self.pending_ask.take() {
            self.ask(target, initial, window, cx);
        }
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

impl Shell {
    fn render_inner(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::Stateful<gpui_kit::Div> {
        self.state.update(cx, |s, _| s.poll());
        let playing = self.playing(cx);
        self.stats.borrow_mut().frame(playing || self.bench.is_some());
        {
            let st = self.stats.borrow();
            STATS_SNAP.with(|s| s.set((st.grid_paint.len() as u64, st.grid_paint.max())));
        }
        self.follow(cx);
        self.drive_bench(cx);
        // frames enquanto toca e enquanto os medidores ainda caem depois de parar
        if self.bench.is_some() || playing || self.state.read(cx).levels.active() {
            window.request_animation_frame();
        }
        if self.last_hud.elapsed() > Duration::from_millis(500) {
            self.last_hud = Instant::now();
            self.hud = self.stats.borrow().compact(self.state.read(cx).audio.as_ref().map(|a| &**a.clock()));
        }
        if self.state.read(cx).toast.as_ref().is_some_and(|(_, t)| t.elapsed() > Duration::from_secs(5)) {
            self.state.update(cx, |s, _| s.toast = None);
        }

        let th = self.theme(cx);
        // área visível do editor (do último frame): culling e alvos de salto do editor vertical
        self.grid.read(cx).geom.borrow_mut().viewport = self.scroll.bounds();
        // playhead + notas soando agora (overlay sem cache, irmão da grade)
        let ph = if self.on_grid() { self.playhead(cx) } else { None };
        let (flashes, play_pos) = if self.on_grid() { self.playing_cells(cx) } else { (Vec::new(), None) };
        let row_meters = if self.on_grid() { self.row_meters(cx) } else { Vec::new() };
        self.status = self.status_model(playing, play_pos, cx);
        // moldura em cache: só re-renderiza quando a chave dela muda
        self.top_narrow = f32::from(window.viewport_size().width) < crate::views::topbar::NARROW_W;
        // a grade acompanha o cursor/seleção: mudou (teclado, vim, painel, paleta) → revela
        let sel = {
            let g = self.grid.read(cx);
            (g.cursor, g.anchor)
        };
        if self.last_sel != Some(sel) {
            let first = self.last_sel.is_none();
            self.last_sel = Some(sel);
            if !first && self.on_grid() && !self.playing(cx) && !self.grid.read(cx).is_drag_selecting() {
                self.reveal_cursor(cx);
            }
        }
        // arrastando a seleção perto da borda de cima/baixo: rola sozinho e estende
        if self.on_grid() && self.grid.read(cx).is_drag_selecting() {
            if let Some(p) = self.drag_ptr.get() {
                let vb = self.scroll.bounds();
                let (top, bottom) = (f32::from(vb.origin.y), f32::from(vb.origin.y) + f32::from(vb.size.height));
                let y = f32::from(p.y);
                const EDGE: f32 = 40.0;
                let dy = if y < top + EDGE {
                    -((top + EDGE - y).min(EDGE * 2.0) * 0.5)
                } else if y > bottom - EDGE {
                    (y - (bottom - EDGE)).min(EDGE * 2.0) * 0.5
                } else {
                    0.0
                };
                if dy != 0.0 && f32::from(vb.size.height) > 0.0 {
                    let cur = self.scroll.offset();
                    let max = self.scroll.max_offset();
                    let ny = (f32::from(cur.y) - dy).clamp(-f32::from(max.y), 0.0);
                    if (ny - f32::from(cur.y)).abs() > 0.1 {
                        self.scroll.set_offset(point(cur.x, px(ny)));
                        self.grid.update(cx, |g, cx| g.drag_select_at(p, cx));
                        window.request_animation_frame();
                    }
                }
            }
        } else if self.drag_ptr.get().is_some() {
            self.drag_ptr.set(None);
        }
        self.refresh_chrome(playing, f32::from(window.viewport_size().width), cx);
        let nav = self.chrome[1].clone().cached(gpui_kit::StyleRefinement::default().w(px(64.0)).h_full().flex_none());
        let topbar = self.chrome[0].clone().cached(gpui_kit::StyleRefinement::default().w_full().h(px(44.0)).flex_none());

        // ── página ──
        // No editor, abas + barra de contexto ficam FIXAS no topo (fora da rolagem):
        // dá para usá-las com a grade rolada em qualquer seção.
        let (header, content) = match self.page {
            Page::Editor => {
                // faixa de abas (28 px) + barra de contexto, planas sobre `chrome`
                let mut head = div().flex().flex_col().child(div().h(px(28.0)).flex_none().px(px(10.0)).flex().items_end().bg(th.chrome).border_b_1().border_color(th.line).child(self.view_tabs(&th)));
                let body = match self.views.tab(self.tab) {
                    None => {
                        // em cache com a altura medida; sem medida (chave nova), desenha
                        // sem cache e mede pelo prepaint do wrapper
                        let h = self.ctx_h.get();
                        let bar = &self.chrome[chrome::Part::Context as usize];
                        head = if h > 0.0 {
                            head.child(bar.clone().cached(gpui_kit::StyleRefinement::default().w_full().h(px(h)).flex_none()))
                        } else {
                            let cell = self.ctx_h.clone();
                            head.child(div().w_full().on_children_prepainted(move |b, _, _| cell.set(b.first().map_or(0.0, |b| f32::from(b.size.height)))).child(bar.clone()))
                        };
                        // editor vertical: cabeçalho de colunas fixo (fora da rolagem), em cache;
                        // os mini-medidores são pintados por cima, num canvas sem cache
                        let col_head = {
                            let g = self.grid.read(cx);
                            (g.layout == Layout::Vertical).then(|| g.col_head_view.clone()).flatten()
                        };
                        if let Some(v) = col_head {
                            let meters = row_meters.clone();
                            head = head.child(
                                div()
                                    .relative()
                                    .w_full()
                                    .flex_none()
                                    .h(px(grid::COL_HEAD_H))
                                    .px(px(20.0))
                                    .bg(th.bg)
                                    .border_b_1()
                                    .border_color(th.line)
                                    .child(v.cached(gpui_kit::StyleRefinement::default().w_full().h(px(grid::COL_HEAD_H))))
                                    .child(
                                        canvas(
                                            |_, _, _| {},
                                            move |vb, _, window, _| {
                                                for (b, level, peak) in &meters {
                                                    window.with_content_mask(Some(gpui_kit::ContentMask { bounds: vb }), |window| crate::ui::paint_meter(window, *b, *level, *peak, false, &th));
                                                }
                                            },
                                        )
                                        .absolute()
                                        .inset_0(),
                                    ),
                            );
                        }
                        let grid_h = self.grid.read(cx).grid_height();
                        // a grade NÃO fica em cache aqui: no gpui, uma view em cache que
                        // re-renderiza redesenha todo o seu subárvore (`refreshing`), o que
                        // anularia o cache por seção (`grid::render::SectionView`). O render
                        // da grade em si é barato; o caro (as seções) fica em cache.
                        div().w_full().h(px(grid_h)).child(self.grid.clone()).into_any_element()
                    }
                    Some(view) => view,
                };
                (Some(head), body)
            }
            Page::Config => (None, self.config_page(&th, cx).into_any_element()),
            other => (None, self.views.page(other).unwrap_or_else(|| div().into_any_element())),
        };

        // linha do tempo sob o mouse (editor vertical): faixa sutil no overlay, sem tocar nas seções
        let hover_band = if self.on_grid() && !self.grid.read(cx).is_drag_selecting() { self.grid.read(cx).vrow_band(window.mouse_position()) } else { None };
        self.hover_band_key = hover_band.map(|(y, ..)| y.round() as i32);
        let overlay = canvas(
            |_, _, _| {},
            move |vb, _, window, _| {
                if let Some((y, h, x0, x1)) = hover_band {
                    let r = Bounds::new(point(px(x0), px(y)), size(px((x1 - x0).max(0.0)), px(h)));
                    window.with_content_mask(Some(gpui_kit::ContentMask { bounds: vb }), |window| window.paint_quad(fill(r, Theme::alpha(th.ink, 0.04))));
                }
                // mini-medidores das linhas (3 px sob o nome)
                for (b, level, peak) in &row_meters {
                    let clip = vb.intersect(b);
                    if f32::from(clip.size.width) <= 0.0 || f32::from(clip.size.height) <= 0.0 {
                        continue;
                    }
                    window.with_content_mask(Some(gpui_kit::ContentMask { bounds: vb }), |window| crate::ui::paint_meter(window, *b, *level, *peak, false, &th));
                }
                for f in &flashes {
                    let clip = vb.intersect(&f.bounds);
                    if f32::from(clip.size.width) <= 0.0 || f32::from(clip.size.height) <= 0.0 {
                        continue;
                    }
                    let col = if f.rim { th.accent2 } else { th.accent };
                    window.with_content_mask(Some(gpui_kit::ContentMask { bounds: vb }), |window| {
                        window.paint_quad(quad(
                            f.bounds,
                            px(crate::ui::R0),
                            Theme::alpha(col, 0.35 * f.strength),
                            px(1.0),
                            Theme::alpha(th.playhead, f.strength.max(0.35)),
                            gpui_kit::BorderStyle::Solid,
                        ));
                    });
                }
                let Some(ph) = ph else { return };
                let b = ph.b;
                let clip = vb.intersect(&b);
                if ph.vertical {
                    // linha de 2 px no topo da linha audível + wash 12 % da linha inteira + ▶ no gutter
                    window.with_content_mask(Some(gpui_kit::ContentMask { bounds: clip }), |window| {
                        let (x0, w) = (f32::from(b.origin.x), ph.w);
                        let y = ph.pos;
                        if playing {
                            window.paint_quad(fill(Bounds::new(point(px(x0), px(y)), size(px(w), px(ph.row_h))), Theme::alpha(th.accent, 0.12)));
                            window.paint_quad(fill(Bounds::new(point(px(x0 + grid::LEFT_W), px(y)), size(px(w - grid::LEFT_W), px(2.0))), th.playhead));
                        }
                        let mut tri = gpui_kit::PathBuilder::fill();
                        let (tx, cy) = (x0 + 3.0, y + ph.row_h * 0.5);
                        tri.move_to(point(px(tx), px(cy - 5.0)));
                        tri.line_to(point(px(tx + 8.0), px(cy)));
                        tri.line_to(point(px(tx), px(cy + 5.0)));
                        tri.close();
                        if let Ok(p) = tri.build() {
                            window.paint_path(p, th.accent);
                        }
                    });
                    return;
                }
                let x = ph.pos;
                window.with_content_mask(Some(gpui_kit::ContentMask { bounds: clip }), |window| {
                    let top = f32::from(b.origin.y) + grid::SEC_DYN_H;
                    let h = f32::from(b.size.height) - grid::SEC_DYN_H;
                    if playing {
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

        // overlay (playhead/flashes) só sobre a área rolável: nunca pinta por cima do cabeçalho
        let scroll_area = div()
            .flex_1()
            .min_h_0()
            .relative()
            .child(
                div()
                    .id("editor-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .p(px(20.0))
                    .when(header.is_some(), |d| d.pt(px(12.0)))
                    // a faixa da linha sob o mouse muda → só o Shell (overlay) redesenha
                    .on_mouse_move(cx.listener(|this, ev: &gpui_kit::MouseMoveEvent, _, cx| {
                        let k = this.grid.read(cx).vrow_band(ev.position).map(|(y, ..)| y.round() as i32);
                        if k != this.hover_band_key {
                            cx.notify();
                        }
                    }))
                    .child(content),
            )
            .child(overlay);
        let editor = div().flex_1().min_w_0().h_full().flex().flex_col().bg(th.bg).when_some(header, |d, h| d.child(div().flex_none().child(h))).child(scroll_area);

        let panel = self.chrome[2].clone().cached(gpui_kit::StyleRefinement::default().w(px(280.0)).h_full().flex_none());
        let show_secs = self.sec_panel_open && self.page == Page::Editor;
        let show_mini = self.mini_open && self.on_grid() && self.grid.read(cx).layout == Layout::Vertical;
        let minimap = self.chrome[chrome::Part::Minimap as usize].clone().cached(gpui_kit::StyleRefinement::default().w(px(crate::views::minimap::MINI_W)).h_full().flex_none());
        let secs_panel = self.chrome[chrome::Part::Sections as usize].clone().cached(gpui_kit::StyleRefinement::default().w(px(crate::views::sections_panel::SECTIONS_W)).h_full().flex_none());
        let status = self.chrome[chrome::Part::Status as usize].clone().cached(gpui_kit::StyleRefinement::default().w_full().h(px(22.0)).flex_none());

        // ── workspace (contexto de teclas da grade) ──
        let vmode = self.grid.read(cx).vmode;
        let layout = self.grid.read(cx).layout;
        let ctx = format!("Workspace vim={} mode={} layout={}{}", if self.vim_on { "on" } else { "off" }, vmode.ctx(), layout.name(), if self.count.is_some() { " count" } else { "" });
        let workspace = div()
            .id("workspace")
            .key_context(KeyContext::parse(&ctx).unwrap_or_default())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .flex()
            .flex_col()
            .child(
                div().flex_1().min_h_0().flex().when(self.nav_open, |d| d.child(nav)).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(topbar)
                        .child(div().flex_1().min_h_0().flex().when(show_secs, |d| d.child(secs_panel)).child(editor).when(show_mini, |d| d.child(minimap)).child(panel)),
                ),
            )
            .child(status);

        let root = div().id("root").relative().size_full().font_family("Zen Kaku Gothic New").bg(th.bg).text_color(th.ink);
        let root = self.actions(root, cx);
        let mut root = crate::views::bind_actions(root, cx).child(workspace);

        // ── which-key (command hint): chord pendente, ou Ctrl+/ na raiz ──
        if (!self.pending.is_empty() && self.pending_since.elapsed() >= Duration::from_millis(250) && self.palette.is_none()) || (self.root_hints && self.pending.is_empty() && self.palette.is_none())
        {
            root = root.child(self.which_key(window, &th, cx));
        }

        // ── linha de prompt (: / ? " m ' e renomear) ──
        if let Some(p) = &self.prompt {
            root = root.child(div().id("prompt").absolute().inset_0().key_context("Prompt").track_focus(&self.prompt_focus).on_key_down(cx.listener(Self::prompt_key)).child(self.prompt_bar(p, &th)));
        }

        // ── menus do painel de seções ──
        if let Some(menu) = self.sec_menu_overlay(&th, cx) {
            root = root.child(menu);
        }

        // ── modo projeção (F7d): por cima de tudo ──
        if self.director_close {
            self.director_close = false;
            if let Some(ours) = self.director.take()
                && ours
                && window.is_fullscreen()
            {
                window.toggle_fullscreen();
            }
        }
        if self.director.is_some() {
            root = root.child(self.director(cx));
        }

        // ── paleta (Ctrl+K) ──
        if let Some(p) = &self.palette {
            root = root.child(self.palette_view(p, &th, cx));
        }

        // ── hints (Ctrl+Space) ──
        if let Some(h) = &self.hints {
            root = root.child(self.hints_view(h, &th, cx));
        }

        // ── bloco de notas sendo arrastado (balança), por cima de tudo ──
        let drag = {
            let g = self.grid.read(cx);
            g.move_drag.filter(|m| m.started).map(|m| (m, g.ghost.clone()))
        };
        match drag {
            Some((md, cells)) => {
                // frames só enquanto balança/sobe; parado, só o movimento do mouse redesenha
                if !self.swing.borrow().settled() {
                    window.request_animation_frame();
                }
                let (swing, grid, me) = (self.swing.clone(), self.grid.clone(), cx.entity().downgrade());
                root = root.child(
                    canvas(
                        |_, _, _| {},
                        move |_, _, window, cx| {
                            let p = window.mouse_position();
                            let (theta, lift) = swing.borrow_mut().step(md.grab, p, Instant::now());
                            crate::views::drag_ghost::paint(&cells, md.grab, md.clone, p, theta, lift, &th, window, cx);
                            window.set_window_cursor_style(gpui_kit::CursorStyle::ClosedHand);
                            // mouse mexeu → redesenha o Shell (NÃO `window.refresh()`: refresh
                            // invalida os caches e reconstruía a grade inteira a cada movimento)
                            let me = me.clone();
                            window.on_mouse_event(move |_: &gpui_kit::MouseMoveEvent, phase, _, cx| {
                                if phase == gpui_kit::DispatchPhase::Bubble {
                                    let _ = me.update(cx, |_, cx| cx.notify());
                                }
                            });
                            // soltar em QUALQUER lugar da janela larga o bloco (não só sobre a grade)
                            let grid = grid.clone();
                            window.on_mouse_event(move |ev: &gpui_kit::MouseUpEvent, phase, _, cx| {
                                if phase == gpui_kit::DispatchPhase::Capture && ev.button == gpui_kit::MouseButton::Left {
                                    grid.update(cx, |g, cx| g.mouse_up(cx));
                                }
                            });
                        },
                    )
                    .absolute()
                    .inset_0(),
                );
            }
            None => self.swing.borrow_mut().reset(),
        }

        // ── arrasto de seleção: guarda o ponteiro (a rolagem automática usa no render) ──
        if self.on_grid() && self.grid.read(cx).is_drag_selecting() {
            let (ptr, me) = (self.drag_ptr.clone(), cx.entity().downgrade());
            root = root.child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        let (ptr, me) = (ptr.clone(), me.clone());
                        window.on_mouse_event(move |ev: &gpui_kit::MouseMoveEvent, phase, _, cx| {
                            if phase == gpui_kit::DispatchPhase::Capture {
                                ptr.set(Some(ev.position));
                                let _ = me.update(cx, |_, cx| cx.notify());
                            }
                        });
                    },
                )
                .absolute()
                .inset_0(),
            );
        }

        // ── arrasto na faixa de loop da régua: mover/soltar valem em toda a janela ──
        if self.grid.read(cx).loop_drag.is_some() {
            let grid = self.grid.clone();
            root = root.child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        let g = grid.clone();
                        window.on_mouse_event(move |ev: &gpui_kit::MouseMoveEvent, phase, _, cx| {
                            if phase == gpui_kit::DispatchPhase::Bubble {
                                g.update(cx, |g, cx| g.loop_drag_move(ev.position, cx));
                            }
                        });
                        let g = grid.clone();
                        window.on_mouse_event(move |ev: &gpui_kit::MouseUpEvent, phase, _, cx| {
                            if phase == gpui_kit::DispatchPhase::Capture && ev.button == gpui_kit::MouseButton::Left {
                                g.update(cx, |g, cx| g.finish_loop_drag(cx));
                            }
                        });
                    },
                )
                .absolute()
                .inset_0(),
            );
        }
        root
    }
}
