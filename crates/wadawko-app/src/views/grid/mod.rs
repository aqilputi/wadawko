//! Grade de seções (porte de `Grid.svelte`): estado, movimento, edição modal
//! (vim) e simples, seleção, registradores, kiai, dinâmicas e mouse.
//!
//! Rótulos e botões são elementos GPUI (alvos de salto); as células são um
//! canvas por seção com culling de colunas (`paint.rs`). O render está em
//! `render.rs`; a paleta flutuante, em `views/palette.rs`.
//!
//! A grade NÃO é dona da música: guarda um snapshot (`Arc<Song>`) e toda
//! mudança passa por [`GridView::edit`] → `AppState::edit`.

mod paint;
mod render;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::{Bounds, ClipboardItem, Context, Entity, EventEmitter, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, ShapedLine, SharedString, point, px, size};
use wadawko_core::ops::{self, CellPos};
use wadawko_core::{Dynamic, Hairpin, Song, kyai, phrase};

use crate::song_ext::{Sec, SongExt};
use crate::state::{AppState, StateEvent};
use crate::stats::Stats;

// ───────────────────────────── layout (px, como no CSS) ─────────────────────────────

pub const LABEL_W: f32 = 216.0;
pub const SEC_HEAD_H: f32 = 40.0;
pub const SEC_DYN_H: f32 = 26.0;
pub const TIMELINE_H: f32 = 30.0;
pub const CELL_H: f32 = 42.0;
pub const TDYN_H: f32 = 22.0;
pub const KYAI_H: f32 = 24.0;
pub const ROW_PAD: f32 = 12.0;
pub const SEC_GAP: f32 = 34.0;
pub const CELL_X0: f32 = 16.0;
pub const MEASURE_GAP: f32 = 16.0;
pub const BASE_CW: f32 = 54.0;

/// Posição vertical de cada linha dentro do corpo de uma seção (a partir do topo
/// das células). Linhas com dinâmica própria e/ou kiai são mais altas.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rows {
    pub y: Vec<f32>,
    pub dynr: Vec<bool>,
    pub kyai: Vec<bool>,
    pub h: f32,
}

impl Rows {
    pub fn new(song: &Song, show_dyn: &HashSet<String>, show_kyai: &HashSet<String>) -> Rows {
        let mut r = Rows::default();
        for tr in &song.tracks {
            let d = show_dyn.contains(&tr.id) || ops::measures::track_has_dynamics(song, &tr.id);
            let k = show_kyai.contains(&tr.id) || ops::cells::track_has_kyai(song, &tr.id);
            r.y.push(r.h);
            r.dynr.push(d);
            r.kyai.push(k);
            r.h += CELL_H + 4.0 + if d { TDYN_H + 2.0 } else { 0.0 } + if k { KYAI_H + 2.0 } else { 0.0 } + ROW_PAD;
        }
        r
    }
    pub fn dyn_y(&self, t: usize) -> f32 {
        self.y[t] + CELL_H + 4.0
    }
    pub fn kyai_y(&self, t: usize) -> f32 {
        self.dyn_y(t) + if self.dynr[t] { TDYN_H + 2.0 } else { 0.0 }
    }
    pub fn row_h(&self, t: usize) -> f32 {
        self.y.get(t + 1).copied().unwrap_or(self.h) - self.y[t]
    }
    pub fn body_h(&self) -> f32 {
        SEC_DYN_H + TIMELINE_H + self.h
    }
    /// linha sob `y` (relativo ao topo das células)
    pub fn track_at(&self, y: f32) -> Option<usize> {
        if y < 0.0 || y >= self.h {
            return None;
        }
        self.y.iter().rposition(|&ty| ty <= y)
    }
}

/// x (local ao canvas, sem offset) do início de cada compasso da seção.
pub fn measure_xs(song: &Song, s: usize, cw: f32) -> Vec<f32> {
    let (a, b) = song.section_range(s);
    let mut x = CELL_X0;
    (a..b)
        .map(|m| {
            let here = x;
            x += song.mlen(m) as f32 * cw + MEASURE_GAP;
            here
        })
        .collect()
}

pub fn sec_key(sec: &Sec) -> String {
    sec.id.clone().unwrap_or_default()
}

// ───────────────────────────── estado ─────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cur {
    pub m: usize,
    pub t: usize,
    pub k: usize,
}

/// Modo do modelo "simples" (vim desligado): editar ou selecionar com o mouse.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Edit,
    Select,
}

/// Onde o cursor escreve: nas notas ou na sub-linha de kiai da linha.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lane {
    Cells,
    Kyai,
}

/// Modo de teclado da grade.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VMode {
    Normal,
    Insert,
    Visual,
    /// visual por compasso (V)
    VLine,
    /// vim desligado
    Simple,
}

impl VMode {
    pub fn ctx(self) -> &'static str {
        match self {
            VMode::Normal => "normal",
            VMode::Insert => "insert",
            VMode::Visual => "visual",
            VMode::VLine => "vline",
            VMode::Simple => "simple",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            VMode::Normal => "NORMAL",
            VMode::Insert => "INSERIR",
            VMode::Visual => "VISUAL",
            VMode::VLine => "V-COMPASSO",
            VMode::Simple => "SIMPLES",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    NextBeat,
    PrevBeat,
    EndOfBeat,
    NextMeasure,
    PrevMeasure,
    MeasureStart,
    MeasureEnd,
    FirstFilled,
    SongStart,
    SongEnd,
    NextSection,
    PrevSection,
    FirstTrack,
    LastTrack,
    HalfRight,
    HalfLeft,
}

/// Bloco copiado: [linha][célula]; `linewise` = compassos inteiros (yy).
#[derive(Clone, Default)]
pub struct Register {
    pub rows: Vec<Vec<Option<String>>>,
    pub linewise: bool,
}

/// Geometria do último frame, lida pelo playhead/follow/hints/flash (sem re-render da grade).
#[derive(Default)]
pub struct Geom {
    pub sections: Vec<Bounds<Pixels>>,
    pub root: Bounds<Pixels>,
    pub offset: f32,
    pub cw: f32,
    pub rows: Rows,
}

/// O que a grade pede ao Shell para perguntar ao usuário (prompt de texto).
#[derive(Clone, Debug, PartialEq)]
pub enum InputTarget {
    SectionName(String),
    TrackName(String),
    /// renomear pessoa (página Pessoas)
    PersonName(String),
    /// renomear instrumento custom (página Kit)
    InstrumentName(String),
    SongTitle,
    Bpm,
    TrainerPct,
    TrainerStep,
}

pub enum GridEvent {
    Ask(InputTarget, String),
}

/// Popover aberto sobre a grade.
#[derive(Clone, Debug, PartialEq)]
pub enum Menu {
    /// escolher a dinâmica do compasso `m` (geral, ou da linha `track`)
    Dyn { m: usize, track: Option<usize>, x: f32, y: f32 },
    /// edição da linha `t` (aberto no bloco da seção `s`)
    Row { t: usize, s: usize },
}

/// Arrasto de um bloco (mover; com Alt, clonar): nasce ao apertar sobre uma
/// nota ou sobre a seleção; só "levanta" depois de [`DRAG_START_PX`] de movimento
/// (antes disso, soltar = clique normal).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveDrag {
    pub from: Cur,
    pub dt: isize,
    pub dg: isize,
    pub clone: bool,
    /// ponto (janela) onde o bloco foi pego
    pub grab: (f32, f32),
    /// já passou do limiar e está arrastando (o bloco flutua)
    pub started: bool,
    /// a seleção foi criada por este aperto (clique sem arrastar a desfaz)
    pub picked: bool,
}

/// Movimento mínimo (px) para um aperto sobre nota virar arrasto.
pub const DRAG_START_PX: f32 = 4.0;

/// Uma célula do bloco levantado: retângulo (janela) no instante em que levantou + texto.
#[derive(Clone, Debug)]
pub struct GhostCell {
    pub bounds: Bounds<Pixels>,
    pub text: Option<SharedString>,
}

pub type Range4 = (usize, usize, usize, usize);

type TextCache = Rc<RefCell<HashMap<(SharedString, u32, u8), ShapedLine>>>;

pub struct GridView {
    pub state: Entity<AppState>,
    /// snapshot da música (atualizado a cada edição)
    pub song: Arc<Song>,
    pub stats: Rc<RefCell<Stats>>,
    pub geom: Rc<RefCell<Geom>>,
    pub offset: f32,
    pub offset_target: f32,
    pub cw: f32,
    pub cw_target: f32,
    zoom_anchor: f32,
    pub cursor: Option<Cur>,
    /// âncora da seleção (visual, shift-setas, arrasto)
    pub anchor: Option<Cur>,
    pub lane: Lane,
    pub mode: Mode,
    pub vmode: VMode,
    pub edit: String,
    /// seções recolhidas (por id; "" = bloco virtual)
    pub collapsed: HashSet<String>,
    /// linhas com a sub-linha de dinâmica / kiai aberta à mão (por id da linha)
    pub show_dyn: HashSet<String>,
    pub show_kyai: HashSet<String>,
    /// registradores: '"' sem nome, '0' último yank, 'a'..'z', '+' área de transferência
    pub registers: HashMap<char, Register>,
    /// registrador escolhido com `"x` para a próxima operação
    pub reg_sel: Option<char>,
    /// sílabas confirmadas na sessão de insert atual (para o `.`)
    pub insert_log: Vec<String>,
    pub menu: Option<Menu>,
    pub move_drag: Option<MoveDrag>,
    /// células do bloco que está flutuando (desenhadas pelo Shell, ver `views::drag_ghost`)
    pub ghost: Rc<Vec<GhostCell>>,
    /// seção sendo arrastada (reordenar) e seção sob o ponteiro
    pub drag_sec: Option<usize>,
    pub drag_over: Option<usize>,
    dragging: bool,
    pan: Option<(f32, f32)>,
    shaped: TextCache,
    last_theme_dark: bool,
}

impl EventEmitter<GridEvent> for GridView {}

pub(crate) enum Hit {
    SecDyn(usize, DynPart, Point<Pixels>),
    Timeline(Option<(usize, usize)>),
    Cell(Cur),
    TrackDyn(usize, usize, DynPart, Point<Pixels>),
    Kyai(Cur),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum DynPart {
    Chip,
    Cresc,
    Dim,
    None,
}

impl GridView {
    pub fn new(state: Entity<AppState>, stats: Rc<RefCell<Stats>>, vim: bool, cx: &mut Context<Self>) -> Self {
        let song = state.read(cx).song().clone();
        cx.subscribe(&state, |this: &mut Self, _, ev: &StateEvent, cx| {
            if *ev == StateEvent::SongReplaced {
                this.reset_for_song();
            }
            this.sync(cx);
        })
        .detach();
        // tema, transporte, caneta: re-render (a grade fica em cache no Shell)
        cx.observe(&state, |this: &mut Self, st, cx| {
            let dark = st.read(cx).theme.dark;
            if dark != this.last_theme_dark {
                this.last_theme_dark = dark;
                this.clear_text_cache();
            }
            cx.notify();
        })
        .detach();
        let dark = state.read(cx).theme.dark;
        GridView {
            state,
            song,
            stats,
            geom: Default::default(),
            offset: 0.0,
            offset_target: 0.0,
            cw: BASE_CW,
            cw_target: BASE_CW,
            zoom_anchor: 0.0,
            cursor: Some(Cur { m: 0, t: 0, k: 0 }),
            anchor: None,
            lane: Lane::Cells,
            mode: Mode::Edit,
            vmode: if vim { VMode::Normal } else { VMode::Simple },
            edit: String::new(),
            collapsed: HashSet::new(),
            show_dyn: HashSet::new(),
            show_kyai: HashSet::new(),
            registers: HashMap::new(),
            reg_sel: None,
            insert_log: Vec::new(),
            menu: None,
            move_drag: None,
            ghost: Rc::new(Vec::new()),
            drag_sec: None,
            drag_over: None,
            dragging: false,
            pan: None,
            shaped: Default::default(),
            last_theme_dark: dark,
        }
    }

    /// Relê o snapshot da música e conserta cursor/âncora.
    pub fn sync(&mut self, cx: &mut Context<Self>) {
        self.song = self.state.read(cx).song().clone();
        self.clamp_cursor();
        cx.notify();
    }

    /// Caminho único de edição da grade: `AppState::edit` + snapshot novo.
    pub fn edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Song)) -> bool {
        let changed = self.state.update(cx, |s, cx| s.edit(cx, f));
        self.sync(cx);
        changed
    }

    fn pen(&self, cx: &Context<Self>) -> Option<String> {
        self.state.read(cx).pen.clone()
    }

    pub fn flash(&mut self, msg: impl Into<String>, cx: &mut Context<Self>) {
        let msg = msg.into();
        self.state.update(cx, |s, cx| {
            s.flash(msg);
            cx.notify();
        });
    }

    pub fn rows(&self) -> Rows {
        Rows::new(&self.song, &self.show_dyn, &self.show_kyai)
    }

    pub fn is_collapsed(&self, sec: &Sec) -> bool {
        self.collapsed.contains(&sec_key(sec))
    }

    pub fn toggle_collapsed(&mut self, s: usize) {
        if let Some(sec) = self.song.secs().get(s) {
            let k = sec_key(sec);
            if !self.collapsed.remove(&k) {
                self.collapsed.insert(k);
            }
        }
    }

    pub fn grid_height(&self) -> f32 {
        let body = self.rows().body_h();
        self.song.secs().iter().map(|sec| SEC_HEAD_H + if self.is_collapsed(sec) { 0.0 } else { body } + SEC_GAP).sum::<f32>() + 20.0
    }

    pub fn reset_for_song(&mut self) {
        self.collapsed.clear();
        self.show_dyn.clear();
        self.show_kyai.clear();
        self.cursor = Some(Cur { m: 0, t: 0, k: 0 });
        self.anchor = None;
        self.lane = Lane::Cells;
        self.menu = None;
        self.move_drag = None;
        self.edit.clear();
        self.offset = 0.0;
        self.offset_target = 0.0;
        if self.vmode != VMode::Simple {
            self.vmode = VMode::Normal;
        }
    }

    pub fn clear_text_cache(&mut self) {
        self.shaped.borrow_mut().clear();
    }

    /// Mantém cursor/âncora dentro da música (após remover compassos/linhas).
    pub fn clamp_cursor(&mut self) {
        let song = &self.song;
        if song.measures.is_empty() || song.tracks.is_empty() {
            self.cursor = None;
            self.anchor = None;
            return;
        }
        let fix = |c: Cur| -> Cur {
            let m = c.m.min(song.measures.len() - 1);
            let k = c.k.min(song.mlen(m).saturating_sub(1));
            Cur { m, t: c.t.min(song.tracks.len() - 1), k }
        };
        self.cursor = Some(fix(self.cursor.unwrap_or(Cur { m: 0, t: 0, k: 0 })));
        self.anchor = self.anchor.map(fix);
        if let (Lane::Kyai, Some(c)) = (self.lane, self.cursor)
            && !self.rows().kyai.get(c.t).copied().unwrap_or(false)
        {
            self.lane = Lane::Cells;
        }
    }

    fn view_w(&self) -> f32 {
        self.geom.borrow().sections.iter().map(|b| f32::from(b.size.width)).fold(0.0, f32::max).max(300.0)
    }

    fn max_offset(&self, cw: f32) -> f32 {
        let song = &self.song;
        let widest = song.secs().iter().map(|sec| (sec.start..sec.end).map(|m| song.mlen(m) as f32 * cw + MEASURE_GAP).sum::<f32>()).fold(0.0, f32::max);
        (widest + CELL_X0 - self.view_w() * 0.6).max(0.0)
    }

    pub fn animating(&self) -> bool {
        (self.offset - self.offset_target).abs() > 0.3 || (self.cw - self.cw_target).abs() > 0.01
    }

    fn step_animation(&mut self) {
        let k = 0.35;
        if (self.cw - self.cw_target).abs() > 0.01 {
            let new_cw = self.cw + (self.cw_target - self.cw) * k;
            let a = self.zoom_anchor;
            self.offset = (self.offset + a) * new_cw / self.cw - a;
            self.offset_target = (self.offset_target + a) * new_cw / self.cw - a;
            self.cw = new_cw;
        } else {
            self.cw = self.cw_target;
        }
        let m = self.max_offset(self.cw);
        self.offset_target = self.offset_target.clamp(0.0, m);
        self.offset += (self.offset_target - self.offset) * k;
        if (self.offset - self.offset_target).abs() <= 0.3 {
            self.offset = self.offset_target;
        }
    }

    pub fn zoom_at(&mut self, factor: f32, anchor_x: f32) {
        let new_cw = (self.cw_target * factor).clamp(14.0, 140.0);
        let snapped = if (new_cw - BASE_CW).abs() < 1.5 { BASE_CW } else { new_cw };
        self.zoom_anchor = anchor_x;
        self.cw_target = snapped;
    }

    pub fn zoom_reset(&mut self) {
        self.zoom_anchor = 0.0;
        self.cw_target = BASE_CW;
    }

    pub fn zoom_pct(&self) -> u32 {
        (self.cw_target / BASE_CW * 100.0).round() as u32
    }

    /// parte do "chip" de dinâmica sob `x` (relativo ao início do compasso de largura `mw`)
    fn dyn_part(x: f32, mw: f32) -> DynPart {
        if (2.0..58.0).contains(&x) {
            DynPart::Chip
        } else if x >= mw - 44.0 && x < mw - 26.0 && mw - 44.0 > 62.0 {
            DynPart::Cresc
        } else if x >= mw - 22.0 && x < mw - 4.0 && mw - 44.0 > 62.0 {
            DynPart::Dim
        } else {
            DynPart::None
        }
    }

    pub(crate) fn hit(&self, s: usize, pos: Point<Pixels>) -> Option<Hit> {
        let g = self.geom.borrow();
        let b = *g.sections.get(s)?;
        let x = f32::from(pos.x) - f32::from(b.origin.x) + self.offset;
        let y = f32::from(pos.y) - f32::from(b.origin.y);
        let song = &self.song;
        let (a, _) = song.section_range(s);
        let xs = measure_xs(song, s, self.cw);
        let mut cell = None;
        for (i, &mx) in xs.iter().enumerate() {
            let m = a + i;
            let w = song.mlen(m) as f32 * self.cw;
            if x >= mx && x < mx + w {
                cell = Some((m, ((x - mx) / self.cw) as usize, x - mx, w));
            }
        }
        if y < SEC_DYN_H {
            return cell.map(|(m, _, dx, w)| Hit::SecDyn(m, Self::dyn_part(dx, w), pos));
        }
        if y < SEC_DYN_H + TIMELINE_H {
            return Some(Hit::Timeline(cell.map(|(m, k, _, _)| (m, k))));
        }
        let ty = y - SEC_DYN_H - TIMELINE_H;
        let rows = &g.rows;
        let t = rows.track_at(ty)?;
        let (m, k, dx, w) = cell?;
        let dy = ty - rows.y[t];
        if dy <= CELL_H {
            return Some(Hit::Cell(Cur { m, t, k }));
        }
        if rows.dynr[t] && ty >= rows.dyn_y(t) && ty < rows.dyn_y(t) + TDYN_H {
            return Some(Hit::TrackDyn(m, t, Self::dyn_part(dx, w), pos));
        }
        if rows.kyai[t] && ty >= rows.kyai_y(t) && ty < rows.kyai_y(t) + KYAI_H {
            return Some(Hit::Kyai(Cur { m, t, k }));
        }
        None
    }

    // ───────── seleção ─────────

    /// Seleção efetiva (âncora + cursor); V expande para compassos inteiros.
    pub fn selection(&self) -> Option<(Cur, Cur)> {
        let (a, c) = (self.anchor?, self.cursor?);
        if self.vmode == VMode::VLine {
            let fwd = (a.m, a.k) <= (c.m, c.k);
            let (lo, hi) = if fwd { (a, c) } else { (c, a) };
            let lo = Cur { k: 0, ..lo };
            let hi = Cur { k: self.song.mlen(hi.m).saturating_sub(1), ..hi };
            return Some(if fwd { (lo, hi) } else { (hi, lo) });
        }
        Some((a, c))
    }

    /// (t0, t1, g0, g1) inclusivos: seleção, ou só a célula do cursor.
    pub fn sel_range(&self) -> Option<Range4> {
        let cur = self.cursor?;
        let (a, b) = self.selection().unwrap_or((cur, cur));
        let song = &self.song;
        let (ga, gb) = (song.flat(a.m, a.k), song.flat(b.m, b.k));
        Some((a.t.min(b.t), a.t.max(b.t), ga.min(gb), ga.max(gb)))
    }

    // ───────── movimento ─────────

    pub fn set_cursor_flat(&mut self, t: usize, g: usize) {
        let song = &self.song;
        let g = g.min(song.total_cells().saturating_sub(1));
        let (m, k) = song.unflat(g);
        self.cursor = Some(Cur { m, t: t.min(song.tracks.len().saturating_sub(1)), k });
    }

    pub fn motion(&mut self, mo: Motion, count: usize, extend: bool, cx: &mut Context<Self>) {
        if !self.edit.is_empty() {
            self.commit_edit_in_place(cx);
        }
        let Some(c) = self.cursor else { return };
        if extend && self.anchor.is_none() {
            self.anchor = Some(c);
        }
        let n = count.max(1);
        let half = ((self.view_w() / 2.0) / self.cw).max(1.0) as usize;
        let song = self.song.clone();
        let tracks = song.tracks.len();
        let total = song.total_cells();
        if tracks == 0 || total == 0 {
            return;
        }
        let g = song.flat(c.m, c.k);
        let sub = song.measures[c.m].subdivision.max(1);
        let mlen = song.mlen(c.m);
        let mstart = g - c.k;
        let secs = song.secs();
        let (t, g2) = match mo {
            Motion::Left => (c.t, g.saturating_sub(n)),
            Motion::Right => (c.t, (g + n).min(total - 1)),
            Motion::Up => (c.t.saturating_sub(n), g),
            Motion::Down => ((c.t + n).min(tracks - 1), g),
            Motion::NextBeat => {
                let mut x = g;
                for _ in 0..n {
                    let (m, k) = song.unflat(x);
                    let s = song.measures[m].subdivision.max(1);
                    x = (x - k % s + s).min(total - 1);
                }
                (c.t, x)
            }
            Motion::PrevBeat => {
                let mut x = g;
                for _ in 0..n {
                    let (m, k) = song.unflat(x);
                    let s = song.measures[m].subdivision.max(1);
                    x = if k % s > 0 { x - k % s } else { x.saturating_sub(s) };
                }
                (c.t, x)
            }
            Motion::EndOfBeat => (c.t, (g - c.k % sub + sub - 1).min(total - 1)),
            Motion::NextMeasure => (c.t, song.flat((c.m + n).min(song.measures.len() - 1), 0)),
            Motion::PrevMeasure => {
                let m = if c.k > 0 { (c.m + 1).saturating_sub(n) } else { c.m.saturating_sub(n) };
                (c.t, song.flat(m, 0))
            }
            Motion::MeasureStart => (c.t, mstart),
            Motion::MeasureEnd => (c.t, mstart + mlen.saturating_sub(1)),
            Motion::FirstFilled => {
                let k = (0..mlen).find(|&k| song.cell(c.t, c.m, k).is_some()).unwrap_or(0);
                (c.t, mstart + k)
            }
            Motion::SongStart => (c.t, 0),
            Motion::SongEnd => (c.t, total - 1),
            Motion::NextSection => {
                let s = (song.section_of(c.m) + n).min(secs.len() - 1);
                (c.t, song.flat(secs[s].start.min(song.measures.len() - 1), 0))
            }
            Motion::PrevSection => {
                let s0 = song.section_of(c.m);
                let at_start = c.k == 0 && secs[s0].start == c.m;
                let s = if at_start { s0.saturating_sub(n) } else { (s0 + 1).saturating_sub(n) };
                (c.t, song.flat(secs[s].start.min(song.measures.len() - 1), 0))
            }
            Motion::FirstTrack => (0, g),
            Motion::LastTrack => (tracks - 1, g),
            Motion::HalfRight => (c.t, (g + half * n).min(total - 1)),
            Motion::HalfLeft => (c.t, g.saturating_sub(half * n)),
        };
        self.set_cursor_flat(t, g2);
        if !extend && matches!(self.vmode, VMode::Normal | VMode::Insert | VMode::Simple) {
            self.anchor = None;
        }
        if let Some(c) = self.cursor {
            if self.lane == Lane::Kyai && !self.rows().kyai.get(c.t).copied().unwrap_or(false) {
                self.lane = Lane::Cells;
            }
            self.ensure_visible(c);
        }
        cx.notify();
    }

    pub fn ensure_visible(&mut self, c: Cur) {
        let song = self.song.clone();
        let s = song.section_of(c.m);
        let secs = song.secs();
        let Some(sec) = secs.get(s) else { return };
        let xs = measure_xs(&song, s, self.cw_target);
        let Some(&mx) = xs.get(c.m.saturating_sub(sec.start)) else { return };
        let x = mx + c.k as f32 * self.cw_target;
        let vw = self.geom.borrow().sections.get(s).map(|b| f32::from(b.size.width)).filter(|w| *w > 0.0).unwrap_or(self.view_w());
        if x < self.offset_target {
            self.offset_target = (x - self.cw_target * 2.0).max(0.0);
        } else if x + self.cw_target > self.offset_target + vw {
            self.offset_target = x + self.cw_target * 3.0 - vw;
        }
        self.collapsed.remove(&sec_key(sec));
    }

    // ───────── edição de texto ─────────

    /// Confirma o buffer na célula do cursor. Devolve a posição para onde o
    /// cursor deve ir se o texto ocupou mais de uma célula (kiai com várias sílabas).
    pub fn commit_edit_in_place(&mut self, cx: &mut Context<Self>) -> Option<Cur> {
        let (Some(c), false) = (self.cursor, self.edit.is_empty()) else { return None };
        let text = std::mem::take(&mut self.edit);
        match self.lane {
            Lane::Cells => {
                if phrase::resolve_cell(&text).is_none() {
                    self.flash(format!("“{text}” não é kuchi shōga conhecido"), cx);
                    return None;
                }
                if self.vmode == VMode::Insert {
                    self.insert_log.push(text.clone());
                }
                self.set_cell(c, Some(text), cx);
                None
            }
            Lane::Kyai => {
                let Some(syls) = kyai::split_kyai(&text) else {
                    self.flash(format!("“{text}” não dá para silabar como kiai"), cx);
                    return None;
                };
                let tid = self.song.tid(c.t).to_string();
                let mut next = None;
                self.edit(cx, |s| next = ops::cells::fill_kyai(s, &tid, c.m, c.k, &syls).next);
                self.preview_kyai(c.t, &syls, cx);
                // `next` é a 1ª célula DEPOIS; o chamador ainda avança 1, então voltamos 1
                let song = &self.song;
                let g = match next {
                    Some(p) => song.flat(p.mi, p.ci).saturating_sub(1),
                    None => song.total_cells().saturating_sub(1),
                };
                let (m, k) = song.unflat(g);
                Some(Cur { m, t: c.t, k })
            }
        }
    }

    pub fn preview_kyai(&mut self, t: usize, syls: &[String], cx: &mut Context<Self>) {
        let song = self.song.clone();
        let Some(track) = song.tracks.get(t) else { return };
        let unit = 60.0 / song.tempo.max(1.0) / song.sub() as f64;
        let seed = track.person_ids().first().cloned();
        self.state.update(cx, |st, _| {
            if !st.playing()
                && let Some(a) = st.audio.as_mut()
            {
                let refs: Vec<&str> = syls.iter().map(String::as_str).collect();
                a.preview_kyai(track, &refs, unit, seed.as_deref());
            }
        });
    }

    /// Escreve (ou apaga) a célula do cursor na faixa ativa (notas ou kiai).
    pub fn set_cell(&mut self, c: Cur, val: Option<String>, cx: &mut Context<Self>) {
        let tid = self.song.tid(c.t).to_string();
        let pen = self.pen(cx);
        match self.lane {
            Lane::Cells => {
                self.edit(cx, |s| ops::cells::set_cell(s, &tid, c.m, c.k, val.as_deref(), pen.as_deref()));
                if let Some(v) = &val {
                    let song = self.song.clone();
                    self.state.update(cx, |st, _| {
                        if !st.playing()
                            && let (Some(a), Some(track)) = (st.audio.as_mut(), song.tracks.get(c.t))
                        {
                            a.preview_cell(&song, track, v);
                        }
                    });
                }
            }
            Lane::Kyai => {
                self.edit(cx, |s| ops::cells::set_kyai(s, &tid, c.m, c.k, val.as_deref()));
            }
        }
    }

    /// Confirma o buffer e avança (espaço/Enter/Tab). false se não havia texto.
    pub fn commit_edit(&mut self, cx: &mut Context<Self>) -> bool {
        if self.cursor.is_none() || self.edit.is_empty() {
            return false;
        }
        if let Some(c) = self.commit_edit_in_place(cx) {
            self.cursor = Some(c);
        }
        self.motion(Motion::Right, 1, false, cx);
        true
    }

    /// Esc no insert: confirma sem avançar e volta ao normal.
    pub fn leave_insert(&mut self, cx: &mut Context<Self>) {
        self.commit_edit_in_place(cx);
        self.vmode = VMode::Normal;
        self.anchor = None;
        cx.notify();
    }

    // ───────── frases / kiai prontos (paleta de comandos) ─────────

    /// Insere uma frase do catálogo a partir do cursor (1 palavra por tempo, ou
    /// por célula se `per_cell`).
    pub fn insert_phrase(&mut self, i: usize, cx: &mut Context<Self>) {
        let (Some(c), Some(p)) = (self.cursor, wadawko_core::vocab::PHRASES.get(i)) else { return };
        let Some(words) = phrase::parse_words(p.phrase) else { return };
        let tid = self.song.tid(c.t).to_string();
        let pen = self.pen(cx);
        let mut next: Option<CellPos> = None;
        self.lane = Lane::Cells;
        self.edit.clear();
        self.edit(cx, |s| {
            let r = if p.per_cell {
                ops::cells::fill_sequential(s, &tid, c.m, c.k, &words, pen.as_deref())
            } else {
                ops::cells::fill_phrase_beats(s, &tid, c.m, c.k, &words, pen.as_deref())
            };
            next = r.next;
        });
        if let Some(n) = next {
            self.cursor = Some(Cur { m: n.mi, t: c.t, k: n.ci });
            self.clamp_cursor();
            if let Some(c) = self.cursor {
                self.ensure_visible(c);
            }
        }
        cx.notify();
    }

    /// Insere um kiai pronto (preset) na sub-linha de kiai do cursor.
    pub fn insert_kyai(&mut self, text: &str, cx: &mut Context<Self>) {
        let Some(c) = self.cursor else { return };
        let tid = self.song.tid(c.t).to_string();
        self.show_kyai.insert(tid);
        self.lane = Lane::Kyai;
        self.edit = text.to_string();
        self.commit_edit(cx);
    }

    // ───────── operações sobre intervalos ─────────

    fn get_at(&self, t: usize, g: usize) -> Option<String> {
        let song = &self.song;
        match self.lane {
            Lane::Cells => song.cell_at(t, g).map(str::to_string),
            Lane::Kyai => {
                if g >= song.total_cells() {
                    return None;
                }
                let (m, k) = song.unflat(g);
                ops::cells::get_kyai(song, song.tid(t), m, k).map(str::to_string)
            }
        }
    }

    fn range_rows(&self, r: Range4) -> Vec<Vec<Option<String>>> {
        let (t0, t1, g0, g1) = r;
        (t0..=t1).map(|t| (g0..=g1).map(|g| self.get_at(t, g)).collect()).collect()
    }

    /// Escreve um lote de (linha, coluna global, valor) numa única edição.
    fn write_cells(&mut self, cells: Vec<(usize, usize, Option<String>)>, cx: &mut Context<Self>) {
        let lane = self.lane;
        let pen = self.pen(cx);
        self.edit(cx, |s| {
            for (t, g, v) in &cells {
                let Some(tid) = s.tracks.get(*t).map(|x| x.id.clone()) else { continue };
                match lane {
                    Lane::Cells => ops::cells::set_cell_col(s, &tid, *g, v.as_deref(), pen.as_deref()),
                    Lane::Kyai => {
                        if let Some(p) = ops::measures::col_to_pos(s, *g) {
                            ops::cells::set_kyai(s, &tid, p.mi, p.ci, v.as_deref());
                        }
                    }
                }
            }
        });
    }

    pub fn clear_range(&mut self, r: Range4, cx: &mut Context<Self>) {
        let (t0, t1, g0, g1) = r;
        let cells = (t0..=t1).flat_map(|t| (g0..=g1).map(move |g| (t, g, None))).collect();
        self.write_cells(cells, cx);
    }

    /// Copia para o registrador escolhido (`"x`, senão o sem nome). Yank também vai
    /// para "0; sem nome ou "+ vão para a área de transferência do sistema (TSV).
    pub fn yank_into(&mut self, r: Range4, linewise: bool, is_delete: bool, cx: &mut Context<Self>) {
        let rows = self.range_rows(r);
        let reg = Register { rows, linewise };
        let target = self.reg_sel.take().unwrap_or('"');
        if target == '"' || target == '+' {
            cx.write_to_clipboard(ClipboardItem::new_string(to_tsv(&reg.rows)));
        }
        if !is_delete {
            self.registers.insert('0', reg.clone());
        }
        if target != '"' {
            self.registers.insert(target.to_ascii_lowercase(), reg.clone());
        }
        self.registers.insert('"', reg);
    }

    pub fn yank(&mut self, r: Range4, linewise: bool, cx: &mut Context<Self>) {
        self.yank_into(r, linewise, false, cx);
    }

    /// Lê um registrador; '+' (ou sem nome vazio) lê a área de transferência (TSV).
    fn read_register(&mut self, cx: &mut Context<Self>) -> Option<Register> {
        let src = self.reg_sel.take().unwrap_or('"');
        if src != '+'
            && let Some(r) = self.registers.get(&src.to_ascii_lowercase())
        {
            return Some(r.clone());
        }
        let text = cx.read_from_clipboard()?.text()?;
        let rows = from_tsv(&text);
        (!rows.is_empty()).then_some(Register { rows, linewise: false })
    }

    /// Apaga (com cópia, como o `d` do vim) e põe o cursor no início.
    pub fn delete(&mut self, r: Range4, linewise: bool, cx: &mut Context<Self>) {
        self.yank_into(r, linewise, true, cx);
        self.clear_range(r, cx);
        self.set_cursor_flat(r.0, r.2);
        self.anchor = None;
        cx.notify();
    }

    pub fn paste(&mut self, after: bool, cx: &mut Context<Self>) {
        let Some(c) = self.cursor else { return };
        let Some(reg) = self.read_register(cx) else { return };
        let g = {
            let song = &self.song;
            let g = song.flat(c.m, c.k);
            if reg.linewise {
                // compasso inteiro: p cola no próximo compasso, P neste
                let mstart = g - c.k;
                if after { mstart + song.mlen(c.m) } else { mstart }
            } else if after {
                g + 1
            } else {
                g
            }
        };
        let total = self.song.total_cells();
        let tracks = self.song.tracks.len();
        let mut cells = Vec::new();
        for (i, row) in reg.rows.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                if c.t + i < tracks && g + j < total {
                    cells.push((c.t + i, g + j, v.clone()));
                }
            }
        }
        self.write_cells(cells, cx);
        self.set_cursor_flat(c.t, g);
        cx.notify();
    }

    /// Cola sobre a seleção (visual/simples) ou no cursor.
    pub fn paste_over(&mut self, cx: &mut Context<Self>) {
        if let Some((t0, _, g0, _)) = self.sel_range() {
            self.set_cursor_flat(t0, g0);
        }
        self.anchor = None;
        self.paste(false, cx);
    }

    pub fn map_words(&mut self, f: impl Fn(&str) -> String, cx: &mut Context<Self>) {
        let Some((t0, t1, g0, g1)) = self.sel_range() else { return };
        let mut cells = Vec::new();
        for t in t0..=t1 {
            for g in g0..=g1 {
                if let Some(w) = self.get_at(t, g) {
                    cells.push((t, g, Some(f(&w))));
                }
            }
        }
        self.write_cells(cells, cx);
    }

    /// `!`: se alguma nota do intervalo ainda não é sforzando, liga em todas;
    /// se todas já são, desliga (células vazias ficam como estão).
    pub fn toggle_sfz(&mut self, cx: &mut Context<Self>) {
        if self.lane != Lane::Cells {
            return;
        }
        let Some((t0, t1, g0, g1)) = self.sel_range() else { return };
        let mut notes = Vec::new();
        for t in t0..=t1 {
            for g in g0..=g1 {
                if let Some(w) = self.get_at(t, g) {
                    notes.push((t, g, w));
                }
            }
        }
        let turn_on = notes.iter().any(|(_, _, w)| !phrase::is_sfz(w));
        let cells = notes.into_iter().filter(|(_, _, w)| phrase::is_sfz(w) != turn_on).map(|(t, g, w)| (t, g, phrase::toggle_sfz(Some(&w)))).collect();
        self.write_cells(cells, cx);
    }

    /// Move (ou clona) o bloco selecionado `dt` linhas e `dg` células. Cria
    /// compassos se o destino passar do fim, como o app web.
    pub fn move_selection(&mut self, dt: isize, dg: isize, clone: bool, cx: &mut Context<Self>) {
        if self.anchor.is_none() || (dt == 0 && dg == 0) {
            return;
        }
        let Some((t0, t1, g0, g1)) = self.sel_range() else { return };
        let tracks = self.song.tracks.len() as isize;
        if t0 as isize + dt < 0 || t1 as isize + dt >= tracks || g0 as isize + dg < 0 {
            return;
        }
        let block = self.range_rows((t0, t1, g0, g1));
        let lane = self.lane;
        let pen = self.pen(cx);
        let need = (g1 as isize + dg + 1) as usize;
        self.edit(cx, |s| {
            ops::measures::ensure_cols(s, need, None);
            let ids: Vec<String> = s.tracks.iter().map(|t| t.id.clone()).collect();
            let put = |s: &mut Song, t: usize, g: usize, v: Option<&str>| match lane {
                Lane::Cells => ops::cells::set_cell_col(s, &ids[t], g, v, pen.as_deref()),
                Lane::Kyai => {
                    if let Some(p) = ops::measures::col_to_pos(s, g) {
                        ops::cells::set_kyai(s, &ids[t], p.mi, p.ci, v);
                    }
                }
            };
            if !clone {
                for t in t0..=t1 {
                    for g in g0..=g1 {
                        put(s, t, g, None);
                    }
                }
            }
            for (i, row) in block.iter().enumerate() {
                for (j, v) in row.iter().enumerate() {
                    put(s, (t0 as isize + dt) as usize + i, (g0 as isize + dg) as usize + j, v.as_deref());
                }
            }
        });
        let shift = |c: Cur, song: &Song| {
            let g = (song.flat(c.m, c.k) as isize + dg).max(0) as usize;
            let (m, k) = song.unflat(g.min(song.total_cells().saturating_sub(1)));
            Cur { m, t: (c.t as isize + dt).clamp(0, tracks - 1) as usize, k }
        };
        let song = self.song.clone();
        self.anchor = self.anchor.map(|c| shift(c, &song));
        self.cursor = self.cursor.map(|c| shift(c, &song));
        if let Some(c) = self.cursor {
            self.ensure_visible(c);
        }
        cx.notify();
    }

    /// Atribui as células selecionadas (em linhas compartilhadas) à pessoa `person`.
    pub fn assign_selection(&mut self, person: Option<String>, cx: &mut Context<Self>) {
        let Some((t0, t1, g0, g1)) = self.sel_range() else { return };
        let song = self.song.clone();
        let mut cells: Vec<(String, CellPos)> = Vec::new();
        for t in t0..=t1 {
            let Some(tr) = song.tracks.get(t).filter(|tr| tr.is_shared()) else { continue };
            for g in g0..=g1 {
                if let Some(p) = ops::measures::col_to_pos(&song, g) {
                    cells.push((tr.id.clone(), p));
                }
            }
        }
        if cells.is_empty() {
            self.flash("nenhuma linha compartilhada na seleção (2+ pessoas na linha)", cx);
            return;
        }
        self.edit(cx, |s| ops::people::set_cells_owner(s, &cells, person.as_deref()));
    }

    // intervalos prontos (linha atual)
    pub fn r_cells(&self, count: usize) -> Option<Range4> {
        let c = self.cursor?;
        let song = &self.song;
        let g = song.flat(c.m, c.k);
        Some((c.t, c.t, g, (g + count.max(1) - 1).min(song.total_cells() - 1)))
    }
    pub fn r_cell_back(&self, count: usize) -> Option<Range4> {
        let c = self.cursor?;
        let g = self.song.flat(c.m, c.k);
        (g > 0).then(|| (c.t, c.t, g.saturating_sub(count.max(1)), g - 1))
    }
    pub fn r_measures(&self, count: usize, all_tracks: bool) -> Option<Range4> {
        let c = self.cursor?;
        let song = &self.song;
        let last = (c.m + count.max(1) - 1).min(song.measures.len() - 1);
        let (t0, t1) = if all_tracks { (0, song.tracks.len() - 1) } else { (c.t, c.t) };
        Some((t0, t1, song.flat(c.m, 0), song.flat(last, 0) + song.mlen(last).saturating_sub(1)))
    }
    pub fn r_beats(&self, count: usize) -> Option<Range4> {
        let c = self.cursor?;
        let song = &self.song;
        let sub = song.measures[c.m].subdivision.max(1);
        let g = song.flat(c.m, c.k);
        let g0 = g - c.k % sub;
        Some((c.t, c.t, g0, (g0 + sub * count.max(1) - 1).min(song.total_cells() - 1)))
    }
    pub fn r_to_measure_end(&self) -> Option<Range4> {
        let c = self.cursor?;
        let song = &self.song;
        let g = song.flat(c.m, c.k);
        Some((c.t, c.t, g, g - c.k + song.mlen(c.m).saturating_sub(1)))
    }
    pub fn r_section(&self) -> Option<Range4> {
        let c = self.cursor?;
        let song = &self.song;
        let (a, b) = song.section_range(song.section_of(c.m));
        if b <= a {
            return None;
        }
        Some((c.t, c.t, song.flat(a, 0), song.flat(b - 1, 0) + song.mlen(b - 1).saturating_sub(1)))
    }

    // ───────── dinâmicas ─────────

    pub fn set_dynamic(&mut self, m: usize, track: Option<usize>, d: Option<Dynamic>, cx: &mut Context<Self>) {
        let tid = track.map(|t| self.song.tid(t).to_string());
        self.edit(cx, |s| ops::measures::set_measure_dynamic(s, m, d, tid.as_deref()));
    }

    pub fn toggle_hairpin(&mut self, m: usize, track: Option<usize>, h: Hairpin, cx: &mut Context<Self>) {
        let tid = track.map(|t| self.song.tid(t).to_string());
        self.edit(cx, |s| ops::measures::toggle_measure_hairpin(s, m, h, tid.as_deref()));
    }

    /// Compassos alvo de uma mudança de fórmula: os da seleção (se houver) ou o do cursor.
    pub fn target_measures(&self) -> Vec<usize> {
        match (self.anchor, self.sel_range()) {
            (Some(_), Some((_, _, g0, g1))) => {
                let (m0, _) = self.song.unflat(g0);
                let (m1, _) = self.song.unflat(g1);
                (m0.min(m1)..=m0.max(m1)).collect()
            }
            _ => self.cursor.map(|c| vec![c.m]).unwrap_or_default(),
        }
    }

    /// Soma `delta` tempos à fórmula dos compassos alvo (ex.: +1 = 4/4 → 5/4).
    pub fn change_beats(&mut self, delta: isize, cx: &mut Context<Self>) {
        let targets = self.target_measures();
        if targets.is_empty() {
            self.flash("ponha o cursor num compasso", cx);
            return;
        }
        let song = self.song.clone();
        let mut changed = Vec::new();
        self.edit(cx, |s| {
            for &mi in &targets {
                let Some(m) = song.measures.get(mi) else { continue };
                let beats = (m.beats as isize + delta).clamp(ops::measures::MIN_BEATS as isize, ops::measures::MAX_BEATS as isize) as usize;
                if ops::measures::set_measure_beats(s, mi, beats) {
                    changed.push((mi, beats));
                }
            }
        });
        match changed.as_slice() {
            [] => self.flash(format!("limite: de {} a {} tempos por compasso", ops::measures::MIN_BEATS, ops::measures::MAX_BEATS), cx),
            [(mi, b)] => self.flash(format!("compasso {}: {b}/4", mi + 1), cx),
            many => self.flash(format!("{} compassos: {}/4", many.len(), many[0].1), cx),
        }
        self.clamp_cursor();
        cx.notify();
    }

    /// Atalho de teclado: pp → p → … → ff → (sem marcação).
    pub fn cycle_dynamic(&mut self, m: usize, cx: &mut Context<Self>) {
        let Some(meas) = self.song.measures.get(m) else { return };
        let next = match meas.dynamic {
            None => Some(Dynamic::Pp),
            Some(d) => DYNAMICS.get(d.index() + 1).copied(),
        };
        self.set_dynamic(m, None, next, cx);
    }

    // ───────── mouse ─────────

    fn in_selection(&self, c: Cur) -> bool {
        if self.anchor.is_none() {
            return false;
        }
        let Some((t0, t1, g0, g1)) = self.sel_range() else { return false };
        let g = self.song.flat(c.m, c.k);
        c.t >= t0 && c.t <= t1 && g >= g0 && g <= g1
    }

    fn mouse_down(&mut self, s: usize, ev: &MouseDownEvent, cx: &mut Context<Self>) {
        self.menu = None;
        match self.hit(s, ev.position) {
            Some(Hit::Cell(c)) => {
                if ev.button == MouseButton::Right {
                    // botão direito apaga (como no app web)
                    self.commit_edit_in_place(cx);
                    self.lane = Lane::Cells;
                    self.set_cell(c, None, cx);
                    return;
                }
                if self.cursor == Some(c) && !self.edit.is_empty() && self.lane == Lane::Cells {
                    return;
                }
                self.commit_edit_in_place(cx);
                // pegar: apertar sobre a seleção ou sobre uma nota pega o bloco (Alt = copia);
                // em célula vazia, arrastar seleciona (abaixo). Shift estende a seleção.
                if !ev.modifiers.shift {
                    let in_sel = self.lane == Lane::Cells && self.in_selection(c);
                    let g = self.song.flat(c.m, c.k);
                    let has_note = self.song.cell_at(c.t, g).is_some();
                    if in_sel || has_note || ev.modifiers.alt {
                        if !in_sel {
                            self.lane = Lane::Cells;
                            self.anchor = Some(c);
                            self.cursor = Some(c);
                        }
                        let grab = (f32::from(ev.position.x), f32::from(ev.position.y));
                        self.move_drag = Some(MoveDrag { from: c, dt: 0, dg: 0, clone: ev.modifiers.alt, grab, started: false, picked: !in_sel });
                        cx.notify();
                        return;
                    }
                }
                self.lane = Lane::Cells;
                if ev.modifiers.shift {
                    if self.anchor.is_none() {
                        self.anchor = self.cursor;
                    }
                } else {
                    self.anchor = None;
                    if matches!(self.vmode, VMode::Visual | VMode::VLine) {
                        self.vmode = VMode::Normal;
                    }
                }
                self.cursor = Some(c);
                self.dragging = true;
            }
            Some(Hit::Kyai(c)) => {
                self.commit_edit_in_place(cx);
                if ev.button == MouseButton::Right {
                    self.lane = Lane::Kyai;
                    self.set_cell(c, None, cx);
                    return;
                }
                self.lane = Lane::Kyai;
                self.anchor = None;
                if matches!(self.vmode, VMode::Visual | VMode::VLine) {
                    self.vmode = VMode::Normal;
                }
                self.cursor = Some(c);
                self.dragging = true;
            }
            Some(Hit::Timeline(cell)) if ev.button == MouseButton::Left => {
                self.pan = Some((f32::from(ev.position.x), self.offset_target));
                if let Some((m, k)) = cell {
                    let g = self.song.flat(m, k);
                    self.state.update(cx, |st, cx| {
                        st.start_cell = g;
                        st.seek_cell(g);
                        cx.notify();
                    });
                }
            }
            Some(Hit::SecDyn(m, part, pos)) if ev.button == MouseButton::Left => self.dyn_click(m, None, part, pos, cx),
            Some(Hit::TrackDyn(m, t, part, pos)) if ev.button == MouseButton::Left => self.dyn_click(m, Some(t), part, pos, cx),
            _ => {}
        }
        cx.notify();
    }

    fn dyn_click(&mut self, m: usize, track: Option<usize>, part: DynPart, pos: Point<Pixels>, cx: &mut Context<Self>) {
        match part {
            DynPart::Chip => {
                let root = self.geom.borrow().root;
                self.menu = Some(Menu::Dyn { m, track, x: f32::from(pos.x - root.origin.x) - 20.0, y: f32::from(pos.y - root.origin.y) + 12.0 });
            }
            DynPart::Cresc => self.toggle_hairpin(m, track, Hairpin::Cresc, cx),
            DynPart::Dim => self.toggle_hairpin(m, track, Hairpin::Dim, cx),
            DynPart::None => {}
        }
        cx.notify();
    }

    fn mouse_move(&mut self, s: usize, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        if ev.pressed_button != Some(MouseButton::Left) {
            return;
        }
        if let Some((x0, off0)) = self.pan {
            self.offset_target = off0 - (f32::from(ev.position.x) - x0);
            self.offset = self.offset_target;
            cx.notify();
            return;
        }
        if let Some(mut md) = self.move_drag {
            if !md.started {
                let (dx, dy) = (f32::from(ev.position.x) - md.grab.0, f32::from(ev.position.y) - md.grab.1);
                if dx * dx + dy * dy < DRAG_START_PX * DRAG_START_PX {
                    return;
                }
                md.started = true;
                self.ghost = Rc::new(self.ghost_cells());
                self.move_drag = Some(md);
                cx.notify();
            }
            if let Some(Hit::Cell(c)) = self.hit(s, ev.position) {
                let song = &self.song;
                let dg = song.flat(c.m, c.k) as isize - song.flat(md.from.m, md.from.k) as isize;
                let dt = c.t as isize - md.from.t as isize;
                let next = MoveDrag { dt, dg, clone: md.clone || ev.modifiers.alt, ..md };
                if Some(next) != self.move_drag {
                    self.move_drag = Some(next);
                    cx.notify();
                }
            }
            return;
        }
        // arrastar seleciona em todos os modos (no modo editar também: depois dá
        // para Ctrl+C / Delete / Alt+arrastar; digitar continua no cursor)
        let drag_selects = self.lane == Lane::Cells || self.mode == Mode::Select || matches!(self.vmode, VMode::Normal | VMode::Visual | VMode::VLine);
        let here = match self.hit(s, ev.position) {
            Some(Hit::Cell(c)) if self.lane == Lane::Cells => Some(c),
            Some(Hit::Kyai(c)) if self.lane == Lane::Kyai => self.cursor.filter(|cur| cur.t == c.t).map(|_| c),
            _ => None,
        };
        if self.dragging
            && drag_selects
            && let (Some(c), Some(cur)) = (here, self.cursor)
            && c != cur
        {
            if self.anchor.is_none() {
                self.anchor = Some(cur);
                if self.vmode == VMode::Normal {
                    self.vmode = VMode::Visual;
                }
            }
            self.cursor = Some(c);
            cx.notify();
        }
    }

    /// Soltou o botão (na grade ou em qualquer lugar da janela — o Shell repassa).
    pub fn mouse_up(&mut self, cx: &mut Context<Self>) {
        self.dragging = false;
        self.pan = None;
        if let Some(md) = self.move_drag.take() {
            self.ghost = Rc::new(Vec::new());
            if md.started {
                self.move_selection(md.dt, md.dg, md.clone, cx);
            } else if md.picked || self.mode != Mode::Select {
                // foi só um clique: cursor na célula (para digitar), sem seleção
                self.anchor = None;
                self.cursor = Some(md.from);
            }
        }
        cx.notify();
    }

    /// Esc durante o arrasto: solta o bloco sem mover.
    pub fn cancel_drag(&mut self) -> bool {
        self.ghost = Rc::new(Vec::new());
        self.move_drag.take().is_some()
    }

    /// Retângulos (janela) e textos das células da seleção, para o bloco flutuante.
    fn ghost_cells(&self) -> Vec<GhostCell> {
        let Some((t0, t1, g0, g1)) = self.sel_range() else { return Vec::new() };
        let song = &self.song;
        let mut out = Vec::new();
        for t in t0..=t1 {
            for g in g0..=g1.min(g0 + 63) {
                let (m, k) = song.unflat(g);
                if let Some(bounds) = self.cell_bounds(Cur { m, t, k }) {
                    out.push(GhostCell { bounds, text: song.cell_at(t, g).map(|w| SharedString::from(w.to_string())) });
                }
            }
        }
        out
    }

    /// Solta a seção arrastada sobre a seção `target` (reordenar).
    fn drop_section(&mut self, target: usize, cx: &mut Context<Self>) {
        let Some(from) = self.drag_sec.take() else { return };
        self.drag_over = None;
        if from == target {
            cx.notify();
            return;
        }
        self.move_section_to(from, target, cx);
    }

    /// Move a seção de índice `from` (na grade) para a posição de `target`.
    pub fn move_section_to(&mut self, from: usize, target: usize, cx: &mut Context<Self>) {
        let secs = self.song.secs();
        let Some(id) = secs.get(from).and_then(|s| s.id.clone()) else {
            self.flash("este bloco não é uma seção (crie uma com + seção)", cx);
            return;
        };
        // índice entre as seções reais (o bloco virtual do começo não conta)
        let virt = secs.first().is_some_and(|s| s.id.is_none()) as usize;
        let idx = target.saturating_sub(virt).min(secs.len() - virt - 1);
        let cur_was_in = self.cursor.map(|c| self.song.section_of(c.m) == from).unwrap_or(false);
        self.edit(cx, |s| ops::sections::move_section(s, &id, idx));
        if cur_was_in {
            // o cursor acompanha a seção
            if let Some(sec) = self.song.secs().iter().find(|s| s.id.as_deref() == Some(id.as_str())) {
                let t = self.cursor.map(|c| c.t).unwrap_or(0);
                self.cursor = Some(Cur { m: sec.start, t, k: 0 });
            }
        }
        cx.notify();
    }

    /// Alvos de salto das células visíveis: `track` = só a linha do cursor; `beats` = só inícios de tempo.
    pub fn visible_cells(&self, beats: bool, track: bool) -> Vec<(Cur, Bounds<Pixels>)> {
        let g = self.geom.borrow();
        let song = &self.song;
        let mut out = Vec::new();
        let cur_t = self.cursor.map(|c| c.t).unwrap_or(0);
        let secs = song.secs();
        for (s, b) in g.sections.iter().enumerate() {
            let Some(sec) = secs.get(s) else { continue };
            if f32::from(b.size.width) <= 0.0 || self.is_collapsed(sec) {
                continue;
            }
            let xs = measure_xs(song, s, g.cw);
            let ox = f32::from(b.origin.x);
            let oy = f32::from(b.origin.y) + SEC_DYN_H + TIMELINE_H;
            for (i, &mx) in xs.iter().enumerate() {
                let m = sec.start + i;
                let meas = &song.measures[m];
                for k in 0..meas.cell_count() {
                    if beats && k % meas.subdivision.max(1) != 0 {
                        continue;
                    }
                    let x = ox + mx + k as f32 * g.cw - g.offset;
                    if x < ox || x + g.cw > ox + f32::from(b.size.width) {
                        continue;
                    }
                    for t in 0..song.tracks.len().min(g.rows.y.len()) {
                        if track && t != cur_t {
                            continue;
                        }
                        let y = oy + g.rows.y[t];
                        out.push((Cur { m, t, k }, Bounds::new(point(px(x), px(y)), size(px(g.cw), px(CELL_H)))));
                    }
                }
            }
        }
        out
    }

    /// Bounds (em coordenadas de janela) da célula `c`, pela geometria do último frame.
    pub fn cell_bounds(&self, c: Cur) -> Option<Bounds<Pixels>> {
        let g = self.geom.borrow();
        let song = &self.song;
        let s = song.section_of(c.m);
        let b = g.sections.get(s).copied().filter(|b| f32::from(b.size.width) > 0.0)?;
        let (a, _) = song.section_range(s);
        let xs = measure_xs(song, s, g.cw);
        let x = f32::from(b.origin.x) + xs.get(c.m.checked_sub(a)?)? + c.k as f32 * g.cw - g.offset;
        let y = f32::from(b.origin.y) + SEC_DYN_H + TIMELINE_H + g.rows.y.get(c.t)?;
        Some(Bounds::new(point(px(x), px(y)), size(px(g.cw), px(CELL_H))))
    }
}

pub const DYNAMICS: [Dynamic; 6] = [Dynamic::Pp, Dynamic::P, Dynamic::Mp, Dynamic::Mf, Dynamic::F, Dynamic::Ff];

pub fn to_tsv(rows: &[Vec<Option<String>>]) -> String {
    rows.iter().map(|row| row.iter().map(|c| c.clone().unwrap_or_default()).collect::<Vec<_>>().join("\t")).collect::<Vec<_>>().join("\n")
}

pub fn from_tsv(text: &str) -> Vec<Vec<Option<String>>> {
    text.lines().map(|l| l.split('\t').map(|c| (!c.trim().is_empty()).then(|| c.trim().to_string())).collect()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tsv_roundtrip() {
        let rows = vec![vec![Some("don".to_string()), None, Some("doko".to_string())], vec![None, Some("ka!".to_string()), None]];
        assert_eq!(from_tsv(&to_tsv(&rows)), rows);
    }

    #[test]
    fn rows_grow_with_dynamics_and_kyai() {
        let song = wadawko_core::demo::demo_song();
        let plain = Rows::new(&song, &HashSet::new(), &HashSet::new());
        let id = song.tracks[0].id.clone();
        let with = Rows::new(&song, &HashSet::from([id.clone()]), &HashSet::from([id]));
        assert_eq!(plain.y.len(), song.tracks.len());
        assert!(with.h >= plain.h + TDYN_H + KYAI_H);
        assert!(with.dynr[0] && with.kyai[0]);
        assert_eq!(with.track_at(with.kyai_y(0) + 1.0), Some(0));
        assert_eq!(plain.track_at(plain.h + 1.0), None);
        assert_eq!(with.kyai_y(0), with.dyn_y(0) + TDYN_H + 2.0);
    }

    #[test]
    fn dyn_parts_hit_test() {
        assert_eq!(GridView::dyn_part(10.0, 400.0), DynPart::Chip);
        assert_eq!(GridView::dyn_part(400.0 - 40.0, 400.0), DynPart::Cresc);
        assert_eq!(GridView::dyn_part(400.0 - 10.0, 400.0), DynPart::Dim);
        assert_eq!(GridView::dyn_part(200.0, 400.0), DynPart::None);
        // compasso estreito: sem botões de hairpin
        assert_eq!(GridView::dyn_part(70.0, 90.0), DynPart::None);
    }
}
