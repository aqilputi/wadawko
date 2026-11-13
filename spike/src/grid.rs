//! Grade de seções no visual do Grid.svelte + edição modal (vim) e simples.
//! Rótulos e botões são elementos GPUI (alvos de salto); as células são um
//! canvas por seção com culling de colunas (custo O(células visíveis)).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::{
    App, Bounds, BorderStyle, ClickEvent, ClipboardItem, Context, EventEmitter, FontWeight, Hsla, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Render, ScrollEvent, ShapedLine, SharedString,
    TextAlign, Window, canvas, div, fill, outline, point, prelude::*, px, quad, size,
};

use crate::keys::{self, HintAct};
use crate::model::{self, DYNAMICS, PALETTE, Song};
use crate::stats::Stats;
use crate::theme::Theme;

// ───────────────────────────── layout (px, como no CSS) ─────────────────────────────

pub const LABEL_W: f32 = 216.0;
pub const SEC_HEAD_H: f32 = 40.0;
pub const SEC_DYN_H: f32 = 26.0;
pub const TIMELINE_H: f32 = 30.0;
pub const CELL_H: f32 = 42.0;
pub const TDYN_H: f32 = 22.0;
pub const TRACK_H: f32 = CELL_H + 4.0 + TDYN_H + 12.0;
pub const SEC_GAP: f32 = 34.0;
pub const CELL_X0: f32 = 16.0;
pub const MEASURE_GAP: f32 = 16.0;
pub const BASE_CW: f32 = 54.0;

pub fn body_h(tracks: usize) -> f32 {
    SEC_DYN_H + TIMELINE_H + tracks as f32 * TRACK_H
}

pub fn grid_height(song: &Song, collapsed: &[bool]) -> f32 {
    (0..song.sections.len())
        .map(|s| SEC_HEAD_H + if collapsed.get(s).copied().unwrap_or(false) { 0.0 } else { body_h(song.tracks.len()) } + SEC_GAP)
        .sum::<f32>()
        + 20.0
}

/// x (local ao canvas, sem offset) do início de cada compasso da seção.
pub fn measure_xs(song: &Song, s: usize, cw: f32) -> Vec<f32> {
    let (a, b) = song.section_range(s);
    let mut x = CELL_X0;
    (a..b)
        .map(|m| {
            let here = x;
            x += song.measures[m].len() as f32 * cw + MEASURE_GAP;
            here
        })
        .collect()
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
    pub fn types_text(self) -> bool {
        matches!(self, VMode::Insert | VMode::Simple)
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

/// Geometria do último frame, lida pelo playhead/follow/hints (sem re-render da grade).
#[derive(Default)]
pub struct Geom {
    pub sections: Vec<Bounds<Pixels>>,
    pub root: Bounds<Pixels>,
    pub offset: f32,
    pub cw: f32,
}

pub enum GridEvent {
    SongChanged,
    Seek(u64),
}

pub type Range4 = (usize, usize, usize, usize);

pub struct GridView {
    pub song: Rc<RefCell<Song>>,
    pub theme: Rc<Cell<Theme>>,
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
    pub mode: Mode,
    pub vmode: VMode,
    pub edit: String,
    pub collapsed: Vec<bool>,
    /// registradores: '"' sem nome, '0' último yank, 'a'..'z', '+' área de transferência
    pub registers: HashMap<char, Register>,
    /// registrador escolhido com `"x` para a próxima operação
    pub reg_sel: Option<char>,
    /// sílabas confirmadas na sessão de insert atual (para o `.`)
    pub insert_log: Vec<String>,
    dragging: bool,
    pan: Option<(f32, f32)>,
    shaped: Rc<RefCell<HashMap<(SharedString, u32, u8), ShapedLine>>>,
}

impl EventEmitter<GridEvent> for GridView {}

type Op = Rc<dyn Fn(&mut GridView, &mut Context<GridView>)>;

impl GridView {
    pub fn new(song: Rc<RefCell<Song>>, theme: Rc<Cell<Theme>>, stats: Rc<RefCell<Stats>>, vim: bool) -> Self {
        let n = song.borrow().sections.len();
        GridView {
            song,
            theme,
            stats,
            geom: Default::default(),
            offset: 0.0,
            offset_target: 0.0,
            cw: BASE_CW,
            cw_target: BASE_CW,
            zoom_anchor: 0.0,
            cursor: Some(Cur { m: 0, t: 0, k: 0 }),
            anchor: None,
            mode: Mode::Edit,
            vmode: if vim { VMode::Normal } else { VMode::Simple },
            edit: String::new(),
            collapsed: vec![false; n],
            registers: HashMap::new(),
            reg_sel: None,
            insert_log: Vec::new(),
            dragging: false,
            pan: None,
            shaped: Default::default(),
        }
    }

    pub fn reset_for_song(&mut self) {
        let n = self.song.borrow().sections.len();
        self.collapsed = vec![false; n];
        self.cursor = Some(Cur { m: 0, t: 0, k: 0 });
        self.anchor = None;
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
        let song = self.song.borrow();
        if song.measures.is_empty() || song.tracks.is_empty() {
            return;
        }
        let fix = |c: Cur| -> Cur {
            let m = c.m.min(song.measures.len() - 1);
            let k = c.k.min(song.measures[m].len().saturating_sub(1));
            Cur { m, t: c.t.min(song.tracks.len() - 1), k }
        };
        self.cursor = Some(fix(self.cursor.unwrap_or(Cur { m: 0, t: 0, k: 0 })));
        self.anchor = self.anchor.map(fix);
    }

    fn view_w(&self) -> f32 {
        self.geom.borrow().sections.iter().map(|b| f32::from(b.size.width)).fold(0.0, f32::max).max(300.0)
    }

    fn max_offset(&self, cw: f32) -> f32 {
        let song = self.song.borrow();
        let widest = (0..song.sections.len())
            .map(|s| {
                let (a, b) = song.section_range(s);
                (a..b).map(|m| song.measures[m].len() as f32 * cw + MEASURE_GAP).sum::<f32>()
            })
            .fold(0.0, f32::max);
        drop(song);
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

    fn hit(&self, s: usize, pos: Point<Pixels>) -> Option<Hit> {
        let g = self.geom.borrow();
        let b = *g.sections.get(s)?;
        let x = f32::from(pos.x) - f32::from(b.origin.x) + self.offset;
        let y = f32::from(pos.y) - f32::from(b.origin.y);
        let song = self.song.borrow();
        let (a, _) = song.section_range(s);
        let xs = measure_xs(&song, s, self.cw);
        let mut cell = None;
        for (i, &mx) in xs.iter().enumerate() {
            let m = a + i;
            let w = song.measures[m].len() as f32 * self.cw;
            if x >= mx && x < mx + w {
                cell = Some((m, ((x - mx) / self.cw) as usize));
            }
        }
        if y < SEC_DYN_H {
            return cell.map(|(m, _)| Hit::SecDyn(m));
        }
        if y < SEC_DYN_H + TIMELINE_H {
            return Some(Hit::Timeline(cell));
        }
        let ty = y - SEC_DYN_H - TIMELINE_H;
        let t = (ty / TRACK_H) as usize;
        if t >= song.tracks.len() || ty - t as f32 * TRACK_H > CELL_H {
            return None;
        }
        cell.map(|(m, k)| Hit::Cell(Cur { m, t, k }))
    }

    pub fn emit_changed(&mut self, cx: &mut Context<Self>) {
        self.clamp_cursor();
        cx.emit(GridEvent::SongChanged);
        cx.notify();
    }

    // ───────── seleção ─────────

    /// Seleção efetiva (âncora + cursor); V expande para compassos inteiros.
    pub fn selection(&self) -> Option<(Cur, Cur)> {
        let (a, c) = (self.anchor?, self.cursor?);
        if self.vmode == VMode::VLine {
            let song = self.song.borrow();
            let fwd = (a.m, a.k) <= (c.m, c.k);
            let (lo, hi) = if fwd { (a, c) } else { (c, a) };
            let lo = Cur { k: 0, ..lo };
            let hi = Cur { k: song.measures[hi.m].len() - 1, ..hi };
            return Some(if fwd { (lo, hi) } else { (hi, lo) });
        }
        Some((a, c))
    }

    /// (t0, t1, g0, g1) inclusivos: seleção, ou só a célula do cursor.
    pub fn sel_range(&self) -> Option<Range4> {
        let cur = self.cursor?;
        let (a, b) = self.selection().unwrap_or((cur, cur));
        let song = self.song.borrow();
        let (ga, gb) = (song.flat(a.m, a.k), song.flat(b.m, b.k));
        Some((a.t.min(b.t), a.t.max(b.t), ga.min(gb), ga.max(gb)))
    }

    // ───────── movimento ─────────

    pub fn set_cursor_flat(&mut self, t: usize, g: usize) {
        let song = self.song.borrow();
        let g = g.min(song.total_cells().saturating_sub(1));
        let (m, k) = song.unflat(g);
        drop(song);
        self.cursor = Some(Cur { m, t, k });
    }

    pub fn motion(&mut self, mo: Motion, count: usize, extend: bool, cx: &mut Context<Self>) {
        if !self.edit.is_empty() {
            self.commit_edit_in_place(cx);
        }
        let c = self.cursor.unwrap_or(Cur { m: 0, t: 0, k: 0 });
        if extend && self.anchor.is_none() {
            self.anchor = Some(c);
        }
        let n = count.max(1);
        let half = ((self.view_w() / 2.0) / self.cw).max(1.0) as usize;
        let song = self.song.borrow();
        let tracks = song.tracks.len();
        let total = song.total_cells();
        let g = song.flat(c.m, c.k);
        let sub = song.measures[c.m].subdivision;
        let mlen = song.measures[c.m].len();
        let mstart = g - c.k;
        let (t, g2) = match mo {
            Motion::Left => (c.t, g.saturating_sub(n)),
            Motion::Right => (c.t, (g + n).min(total - 1)),
            Motion::Up => (c.t.saturating_sub(n), g),
            Motion::Down => ((c.t + n).min(tracks - 1), g),
            Motion::NextBeat => {
                let mut x = g;
                for _ in 0..n {
                    let (m, k) = song.unflat(x);
                    let s = song.measures[m].subdivision;
                    x = (x - k % s + s).min(total - 1);
                }
                (c.t, x)
            }
            Motion::PrevBeat => {
                let mut x = g;
                for _ in 0..n {
                    let (m, k) = song.unflat(x);
                    let s = song.measures[m].subdivision;
                    x = if k % s > 0 { x - k % s } else { x.saturating_sub(s) };
                }
                (c.t, x)
            }
            Motion::EndOfBeat => (c.t, (g - c.k % sub + sub - 1).min(total - 1)),
            Motion::NextMeasure => {
                let m = (c.m + n).min(song.measures.len() - 1);
                (c.t, song.flat(m, 0))
            }
            Motion::PrevMeasure => {
                let m = if c.k > 0 { (c.m + 1).saturating_sub(n) } else { c.m.saturating_sub(n) };
                (c.t, song.flat(m, 0))
            }
            Motion::MeasureStart => (c.t, mstart),
            Motion::MeasureEnd => (c.t, mstart + mlen - 1),
            Motion::FirstFilled => {
                let k = song.measures[c.m].cells[c.t].iter().position(|x| x.is_some()).unwrap_or(0);
                (c.t, mstart + k)
            }
            Motion::SongStart => (c.t, 0),
            Motion::SongEnd => (c.t, total - 1),
            Motion::NextSection => {
                let s = (song.section_of(c.m) + n).min(song.sections.len() - 1);
                (c.t, song.flat(song.section_range(s).0.min(song.measures.len() - 1), 0))
            }
            Motion::PrevSection => {
                let s0 = song.section_of(c.m);
                let at_start = c.k == 0 && song.section_range(s0).0 == c.m;
                let s = if at_start { s0.saturating_sub(n) } else { (s0 + 1).saturating_sub(n) };
                (c.t, song.flat(song.section_range(s).0.min(song.measures.len() - 1), 0))
            }
            Motion::FirstTrack => (0, g),
            Motion::LastTrack => (tracks - 1, g),
            Motion::HalfRight => (c.t, (g + half * n).min(total - 1)),
            Motion::HalfLeft => (c.t, g.saturating_sub(half * n)),
        };
        drop(song);
        self.set_cursor_flat(t, g2);
        if !extend && matches!(self.vmode, VMode::Normal | VMode::Insert | VMode::Simple) {
            self.anchor = None;
        }
        if let Some(c) = self.cursor {
            self.ensure_visible(c);
        }
        cx.notify();
    }

    pub fn ensure_visible(&mut self, c: Cur) {
        let song = self.song.borrow();
        let s = song.section_of(c.m);
        let (a, _) = song.section_range(s);
        let xs = measure_xs(&song, s, self.cw_target);
        let Some(&mx) = xs.get(c.m - a) else { return };
        let x = mx + c.k as f32 * self.cw_target;
        drop(song);
        let vw = self.geom.borrow().sections.get(s).map(|b| f32::from(b.size.width)).filter(|w| *w > 0.0).unwrap_or(self.view_w());
        if x < self.offset_target {
            self.offset_target = (x - self.cw_target * 2.0).max(0.0);
        } else if x + self.cw_target > self.offset_target + vw {
            self.offset_target = x + self.cw_target * 3.0 - vw;
        }
        if self.collapsed.get(s).copied().unwrap_or(false) {
            self.collapsed[s] = false;
        }
    }

    // ───────── edição de texto ─────────

    pub fn commit_edit_in_place(&mut self, cx: &mut Context<Self>) {
        if let (Some(c), false) = (self.cursor, self.edit.is_empty()) {
            let text = std::mem::take(&mut self.edit);
            if self.vmode == VMode::Insert {
                self.insert_log.push(text.clone());
            }
            self.set_cell(c, Some(text), cx);
        }
    }

    pub fn set_cell(&mut self, c: Cur, val: Option<String>, cx: &mut Context<Self>) {
        {
            let mut song = self.song.borrow_mut();
            if let Some(cell) = song.measures.get_mut(c.m).and_then(|m| m.cells.get_mut(c.t)).and_then(|r| r.get_mut(c.k)) {
                *cell = val;
            }
        }
        self.emit_changed(cx);
    }

    /// Confirma o buffer e avança (espaço/Enter/Tab). false se não havia texto.
    pub fn commit_edit(&mut self, cx: &mut Context<Self>) -> bool {
        if self.cursor.is_none() || self.edit.is_empty() {
            return false;
        }
        self.commit_edit_in_place(cx);
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

    pub fn insert_syllable(&mut self, syl: &str, cx: &mut Context<Self>) {
        if let Some(c) = self.cursor {
            self.edit.clear();
            if self.vmode == VMode::Insert {
                self.insert_log.push(syl.to_string());
            }
            self.set_cell(c, Some(syl.to_string()), cx);
            self.motion(Motion::Right, 1, false, cx);
        }
    }

    // ───────── operações sobre intervalos ─────────

    fn range_rows(&self, r: Range4) -> Vec<Vec<Option<String>>> {
        let (t0, t1, g0, g1) = r;
        let song = self.song.borrow();
        (t0..=t1).map(|t| (g0..=g1).map(|g| song.cell_at(t, g).cloned().flatten()).collect()).collect()
    }

    pub fn clear_range(&mut self, r: Range4, cx: &mut Context<Self>) {
        let (t0, t1, g0, g1) = r;
        {
            let mut song = self.song.borrow_mut();
            for t in t0..=t1 {
                for g in g0..=g1 {
                    if let Some(c) = song.cell_at_mut(t, g) {
                        *c = None;
                    }
                }
            }
        }
        self.emit_changed(cx);
    }

    /// Copia para o registrador escolhido (`"x`, senão o sem nome). Yank também vai
    /// para "0; sem nome ou "+ vão para a área de transferência do sistema (TSV).
    pub fn yank_into(&mut self, r: Range4, linewise: bool, is_delete: bool, cx: &mut Context<Self>) {
        let rows = self.range_rows(r);
        let reg = Register { rows, linewise };
        let target = self.reg_sel.take().unwrap_or('"');
        if target == '"' || target == '+' {
            let tsv = reg
                .rows
                .iter()
                .map(|row| row.iter().map(|c| c.clone().unwrap_or_default()).collect::<Vec<_>>().join("\t"))
                .collect::<Vec<_>>()
                .join("\n");
            cx.write_to_clipboard(ClipboardItem::new_string(tsv));
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
        let rows = text
            .lines()
            .map(|l| l.split('\t').map(|c| (!c.trim().is_empty()).then(|| c.trim().to_string())).collect())
            .collect::<Vec<Vec<Option<String>>>>();
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
            let song = self.song.borrow();
            let g = song.flat(c.m, c.k);
            if reg.linewise {
                // compasso inteiro: p cola no próximo compasso, P neste
                let mstart = g - c.k;
                if after { mstart + song.measures[c.m].len() } else { mstart }
            } else if after {
                g + 1
            } else {
                g
            }
        };
        {
            let mut song = self.song.borrow_mut();
            for (i, row) in reg.rows.iter().enumerate() {
                for (j, v) in row.iter().enumerate() {
                    if let Some(cell) = song.cell_at_mut(c.t + i, g + j) {
                        *cell = v.clone();
                    }
                }
            }
        }
        self.set_cursor_flat(c.t, g);
        self.emit_changed(cx);
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
        {
            let mut song = self.song.borrow_mut();
            for t in t0..=t1 {
                for g in g0..=g1 {
                    if let Some(Some(w)) = song.cell_at_mut(t, g) {
                        *w = f(w);
                    }
                }
            }
        }
        self.emit_changed(cx);
    }

    // intervalos prontos (linha atual)
    pub fn r_cells(&self, count: usize) -> Option<Range4> {
        let c = self.cursor?;
        let song = self.song.borrow();
        let g = song.flat(c.m, c.k);
        Some((c.t, c.t, g, (g + count.max(1) - 1).min(song.total_cells() - 1)))
    }
    pub fn r_cell_back(&self, count: usize) -> Option<Range4> {
        let c = self.cursor?;
        let g = self.song.borrow().flat(c.m, c.k);
        (g > 0).then(|| (c.t, c.t, g.saturating_sub(count.max(1)), g - 1))
    }
    pub fn r_measures(&self, count: usize, all_tracks: bool) -> Option<Range4> {
        let c = self.cursor?;
        let song = self.song.borrow();
        let last = (c.m + count.max(1) - 1).min(song.measures.len() - 1);
        let (t0, t1) = if all_tracks { (0, song.tracks.len() - 1) } else { (c.t, c.t) };
        Some((t0, t1, song.flat(c.m, 0), song.flat(last, 0) + song.measures[last].len() - 1))
    }
    pub fn r_beats(&self, count: usize) -> Option<Range4> {
        let c = self.cursor?;
        let song = self.song.borrow();
        let sub = song.measures[c.m].subdivision;
        let g = song.flat(c.m, c.k);
        let g0 = g - c.k % sub;
        Some((c.t, c.t, g0, (g0 + sub * count.max(1) - 1).min(song.total_cells() - 1)))
    }
    pub fn r_to_measure_end(&self) -> Option<Range4> {
        let c = self.cursor?;
        let song = self.song.borrow();
        let g = song.flat(c.m, c.k);
        Some((c.t, c.t, g, g - c.k + song.measures[c.m].len() - 1))
    }
    pub fn r_section(&self) -> Option<Range4> {
        let c = self.cursor?;
        let song = self.song.borrow();
        let (a, b) = song.section_range(song.section_of(c.m));
        if b == 0 {
            return None;
        }
        Some((c.t, c.t, song.flat(a, 0), song.flat(b - 1, 0) + song.measures[b - 1].len() - 1))
    }

    // ───────── mouse ─────────

    fn mouse_down(&mut self, s: usize, ev: &MouseDownEvent, cx: &mut Context<Self>) {
        match self.hit(s, ev.position) {
            Some(Hit::Cell(c)) => {
                if self.cursor == Some(c) && !self.edit.is_empty() {
                    return;
                }
                self.commit_edit_in_place(cx);
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
            Some(Hit::Timeline(cell)) => {
                self.pan = Some((f32::from(ev.position.x), self.offset_target));
                if let Some((m, k)) = cell {
                    let tick = self.song.borrow().first_tick_of(m, k);
                    cx.emit(GridEvent::Seek(tick));
                }
            }
            Some(Hit::SecDyn(m)) => {
                self.cycle_dynamic(m, cx);
                return;
            }
            None => {}
        }
        cx.notify();
    }

    pub fn cycle_dynamic(&mut self, m: usize, cx: &mut Context<Self>) {
        {
            let mut song = self.song.borrow_mut();
            let Some(meas) = song.measures.get_mut(m) else { return };
            let i = meas.dynamic.as_deref().and_then(|d| DYNAMICS.iter().position(|x| *x == d));
            meas.dynamic = match i {
                None => Some("pp".into()),
                Some(i) if i + 1 < DYNAMICS.len() => Some(DYNAMICS[i + 1].into()),
                _ => None,
            };
        }
        self.emit_changed(cx);
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
        let drag_selects = self.mode == Mode::Select || matches!(self.vmode, VMode::Normal | VMode::Visual | VMode::VLine);
        if self.dragging
            && drag_selects
            && let (Some(Hit::Cell(c)), Some(cur)) = (self.hit(s, ev.position), self.cursor)
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

    fn mouse_up(&mut self, cx: &mut Context<Self>) {
        self.dragging = false;
        self.pan = None;
        cx.notify();
    }

    /// Alvos de salto das células visíveis: `track` = só a linha do cursor; `beats` = só inícios de tempo.
    pub fn visible_cells(&self, beats: bool, track: bool) -> Vec<(Cur, Bounds<Pixels>)> {
        let g = self.geom.borrow();
        let song = self.song.borrow();
        let mut out = Vec::new();
        let cur_t = self.cursor.map(|c| c.t).unwrap_or(0);
        for (s, b) in g.sections.iter().enumerate() {
            if f32::from(b.size.width) <= 0.0 || s >= song.sections.len() || self.collapsed.get(s).copied().unwrap_or(false) {
                continue;
            }
            let (a, _) = song.section_range(s);
            let xs = measure_xs(&song, s, g.cw);
            let ox = f32::from(b.origin.x);
            let oy = f32::from(b.origin.y) + SEC_DYN_H + TIMELINE_H;
            for (i, &mx) in xs.iter().enumerate() {
                let m = a + i;
                let meas = &song.measures[m];
                for k in 0..meas.len() {
                    if beats && k % meas.subdivision != 0 {
                        continue;
                    }
                    let x = ox + mx + k as f32 * g.cw - g.offset;
                    if x < ox || x + g.cw > ox + f32::from(b.size.width) {
                        continue;
                    }
                    for t in 0..song.tracks.len() {
                        if track && t != cur_t {
                            continue;
                        }
                        let y = oy + t as f32 * TRACK_H;
                        out.push((Cur { m, t, k }, Bounds::new(point(px(x), px(y)), size(px(g.cw), px(CELL_H)))));
                    }
                }
            }
        }
        out
    }
}

enum Hit {
    SecDyn(usize),
    Timeline(Option<(usize, usize)>),
    Cell(Cur),
}

// ───────────────────────────── render ─────────────────────────────

fn small_btn_base(id: SharedString, label: &'static str, th: &Theme, on: bool) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(22.0))
        .h(px(24.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.0))
        .text_size(px(11.0))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .when(on, |d| d.bg(th.accent).text_color(th.on_accent))
        .when(!on, |d| d.bg(th.paper_raise).text_color(th.ink_soft).hover(|s| s.bg(th.paper_hover)))
        .child(label)
}

impl Render for GridView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::DBG_GRID_RENDERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self.animating() {
            self.step_animation();
            window.request_animation_frame();
        }
        let th = self.theme.get();
        let song_rc = self.song.clone();
        let song = self.song.borrow();
        if self.collapsed.len() != song.sections.len() {
            self.collapsed.resize(song.sections.len(), false);
        }
        {
            let mut g = self.geom.borrow_mut();
            g.offset = self.offset;
            g.cw = self.cw;
            g.sections.resize(song.sections.len(), Bounds::default());
        }
        let n_tracks = song.tracks.len();
        let geom = self.geom.clone();
        let weak = cx.entity().downgrade();

        // botão pequeno = clique + alvo de salto com a MESMA operação
        let mk = |id: String, label: &'static str, on: bool, op: Op| {
            let w1 = weak.clone();
            let w2 = weak.clone();
            let op2 = op.clone();
            let act: HintAct = Rc::new(move |_w: &mut Window, cx: &mut App| {
                let _ = w2.update(cx, |this, cx| op2(this, cx));
            });
            keys::hintable(
                small_btn_base(id.clone().into(), label, &th, on).on_click(move |_: &ClickEvent, _, cx: &mut App| {
                    let _ = w1.update(cx, |this, cx| op(this, cx));
                }),
                id,
                act,
            )
        };

        let mut root = div()
            .id("grid-root")
            .relative()
            .w_full()
            .flex()
            .flex_col()
            .on_scroll(cx.listener(|this, ev: &ScrollEvent, _w, cx| {
                let d = ev.delta.pixel_delta(px(20.0));
                let (dx, dy) = (f32::from(d.x), f32::from(d.y));
                if ev.modifiers.control {
                    let a = this.geom.borrow().sections.first().map(|b| f32::from(ev.position.x) - f32::from(b.origin.x)).unwrap_or(0.0);
                    this.zoom_at((dy * 0.004).exp(), a.max(0.0));
                    cx.stop_propagation();
                    cx.notify();
                } else if ev.modifiers.shift || dx.abs() > dy.abs() {
                    this.offset_target -= if dx.abs() > dy.abs() { dx } else { dy };
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                canvas(
                    move |b, _, _| {
                        geom.borrow_mut().root = b;
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );

        for s in 0..song.sections.len() {
            let sec = &song.sections[s];
            let collapsed = self.collapsed[s];
            let rep = sec.repeat;
            let cur_sec = self.cursor.map(|c| song.section_of(c.m)) == Some(s);

            let header = div()
                .h(px(SEC_HEAD_H))
                .flex()
                .items_center()
                .overflow_hidden()
                .child(
                    div()
                        .w(px(LABEL_W))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(
                            div()
                                .w(px(24.0))
                                .h(px(24.0))
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(th.line_strong)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(th.ink_soft)
                                .text_size(px(12.0))
                                .child("⠿"),
                        )
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(FontWeight::MEDIUM)
                                .when(cur_sec, |d| d.text_color(th.accent))
                                .child(SharedString::from(sec.name.clone())),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .items_center()
                        .gap(px(6.0))
                        .child(div().text_color(th.accent).text_size(px(14.0)).child("𝄆"))
                        .child(mk(format!("s{s}-rep-"), "−", false, Rc::new(move |this, cx| {
                            {
                                let mut song = this.song.borrow_mut();
                                song.sections[s].repeat = song.sections[s].repeat.saturating_sub(1).max(1);
                            }
                            this.emit_changed(cx);
                        })))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(if rep > 1 { th.accent } else { th.ink_soft })
                                .child(SharedString::from(format!("{rep}×"))),
                        )
                        .child(mk(format!("s{s}-rep+"), "+", false, Rc::new(move |this, cx| {
                            {
                                let mut song = this.song.borrow_mut();
                                song.sections[s].repeat = (song.sections[s].repeat + 1).min(16);
                            }
                            this.emit_changed(cx);
                        })))
                        .child(div().text_color(th.accent).text_size(px(14.0)).child("𝄇"))
                        .child(mk(format!("s{s}-col"), if collapsed { "▸" } else { "▾" }, false, Rc::new(move |this, cx| {
                            this.collapsed[s] = !this.collapsed[s];
                            this.emit_changed(cx);
                        })))
                        .child(div().w(px(8.0)))
                        .child(mk(format!("s{s}-m-"), "−", false, Rc::new(move |this, cx| {
                            this.song.borrow_mut().add_measure(s, -1);
                            this.emit_changed(cx);
                        })))
                        .child(div().text_size(px(11.0)).text_color(th.ink_faint).child("compasso"))
                        .child(mk(format!("s{s}-m+"), "+", false, Rc::new(move |this, cx| {
                            this.song.borrow_mut().add_measure(s, 1);
                            this.emit_changed(cx);
                        }))),
                )
                .child({
                    let w1 = weak.clone();
                    let w2 = weak.clone();
                    let clone_op = move |this: &mut GridView, cx: &mut Context<GridView>| {
                        this.song.borrow_mut().clone_section(s);
                        this.collapsed.insert(s + 1, false);
                        this.emit_changed(cx);
                    };
                    keys::hintable(
                        div()
                            .id(("clone", s))
                            .relative()
                            .flex_none()
                            .px(px(8.0))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .rounded(px(5.0))
                            .border_1()
                            .border_color(th.line)
                            .text_size(px(11.0))
                            .text_color(th.ink_soft)
                            .cursor_pointer()
                            .hover(|st| st.bg(th.paper_hover))
                            .child("⧉ clonar")
                            .on_click(move |_: &ClickEvent, _, cx: &mut App| {
                                let _ = w1.update(cx, |this, cx| clone_op(this, cx));
                            }),
                        format!("s{s}-clone"),
                        Rc::new(move |_, cx: &mut App| {
                            let _ = w2.update(cx, |this, cx| clone_op(this, cx));
                        }),
                    )
                })
                .child(
                    mk(format!("s{s}-rm"), "✕", false, Rc::new(move |this, cx| {
                        this.song.borrow_mut().remove_section(s);
                        if s < this.collapsed.len() {
                            this.collapsed.remove(s);
                        }
                        this.emit_changed(cx);
                    }))
                    .ml(px(10.0))
                    .mr(px(12.0)),
                );

            let mut block = div().flex().flex_col().mb(px(SEC_GAP)).child(header);

            if !collapsed {
                let mut labels = div()
                    .w(px(LABEL_W))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .child(div().h(px(SEC_DYN_H)).flex().items_center().text_size(px(10.5)).text_color(th.ink_faint).child("dinâmica"))
                    .child(
                        div()
                            .h(px(TIMELINE_H))
                            .flex()
                            .items_center()
                            .text_size(px(10.5))
                            .text_color(th.ink_faint)
                            .child("↔ arraste · clique = tocar daqui"),
                    );
                for (t, tr) in song.tracks.iter().enumerate() {
                    let audible = song.audible(t);
                    let cur_row = self.cursor.map(|c| c.t) == Some(t) && cur_sec;
                    let id = |n: &str| format!("s{s}-t{t}-{n}");
                    labels = labels.child(
                        div()
                            .h(px(TRACK_H))
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .h(px(CELL_H))
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .pr(px(10.0))
                                    .when(!audible, |d| d.opacity(0.5))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_size(px(14.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .when(cur_row, |d| d.text_color(th.accent))
                                                    .truncate()
                                                    .child(SharedString::from(tr.name.clone())),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(th.ink_faint)
                                                    .truncate()
                                                    .child(model::instrument_label(&tr.instrument)),
                                            ),
                                    )
                                    .child(small_btn_base(id("set").into(), "⚙", &th, false))
                                    .child(small_btn_base(id("dyn").into(), "f", &th, true))
                                    .child(mk(id("mute"), "M", tr.muted, Rc::new(move |this, cx| {
                                        {
                                            let mut song = this.song.borrow_mut();
                                            song.tracks[t].muted = !song.tracks[t].muted;
                                        }
                                        this.emit_changed(cx);
                                    })))
                                    .child(mk(id("solo"), "S", tr.solo, Rc::new(move |this, cx| {
                                        {
                                            let mut song = this.song.borrow_mut();
                                            song.tracks[t].solo = !song.tracks[t].solo;
                                        }
                                        this.emit_changed(cx);
                                    })))
                                    .child(small_btn_base(id("star").into(), "★", &th, false))
                                    .child(mk(id("rm"), "×", false, Rc::new(move |this, cx| {
                                        this.song.borrow_mut().remove_track(t);
                                        this.emit_changed(cx);
                                    }))),
                            )
                            .child(
                                div()
                                    .mt(px(4.0))
                                    .h(px(TDYN_H))
                                    .flex()
                                    .items_center()
                                    .text_size(px(10.0))
                                    .italic()
                                    .text_color(th.ink_faint)
                                    .child(SharedString::from(format!("dinâmica · {}", tr.name))),
                            ),
                    );
                }

                let song2 = song_rc.clone();
                let geom2 = self.geom.clone();
                let stats = self.stats.clone();
                let shaped = self.shaped.clone();
                let sel_flat = self.anchor.and_then(|_| self.sel_range());
                let (offset, cw, cursor, edit, vmode) = (self.offset, self.cw, self.cursor, SharedString::from(self.edit.clone()), self.vmode);
                let cells = div()
                    .id(("cells", s))
                    .flex_1()
                    .min_w_0()
                    .h(px(body_h(n_tracks)))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| this.mouse_down(s, ev, cx)))
                    .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| this.mouse_move(s, ev, cx)))
                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, cx| this.mouse_up(cx)))
                    .child(
                        canvas(
                            move |b, _, _| {
                                if let Some(slot) = geom2.borrow_mut().sections.get_mut(s) {
                                    *slot = b;
                                }
                                b
                            },
                            move |b, _, window, cx| {
                                let t0 = Instant::now();
                                paint_section(b, s, &song2.borrow(), &th, offset, cw, cursor, sel_flat, vmode, &edit, &shaped, window, cx);
                                stats.borrow_mut().grid_paint.record(t0.elapsed());
                            },
                        )
                        .size_full(),
                    );
                block = block.child(div().flex().child(labels).child(cells));
            } else if let Some(slot) = self.geom.borrow_mut().sections.get_mut(s) {
                *slot = Bounds::default();
            }
            root = root.child(block);
        }

        // ── paleta flutuante de sílabas (só quando se digita: insert ou modo simples/editar) ──
        if self.vmode.types_text()
            && (self.vmode != VMode::Simple || self.mode == Mode::Edit)
            && self.anchor.is_none()
            && let Some(c) = self.cursor
        {
            let g = self.geom.borrow();
            let s = song.section_of(c.m);
            if let Some(b) = g.sections.get(s).filter(|b| f32::from(b.size.width) > 0.0) {
                let (a, _) = song.section_range(s);
                let xs = measure_xs(&song, s, self.cw);
                const PAL_W: f32 = 330.0;
                let root_w = f32::from(g.root.size.width);
                let x = f32::from(b.origin.x) - f32::from(g.root.origin.x) + xs[c.m - a] + c.k as f32 * self.cw - self.offset;
                let x = x.min(root_w - PAL_W - 4.0).max(0.0);
                let y = f32::from(b.origin.y) - f32::from(g.root.origin.y) + SEC_DYN_H + TIMELINE_H + c.t as f32 * TRACK_H + CELL_H + 6.0;
                let mut pal = div()
                    .absolute()
                    .w(px(PAL_W))
                    .left(px(x))
                    .top(px(y))
                    .p(px(8.0))
                    .rounded(px(10.0))
                    .bg(th.paper)
                    .border_1()
                    .border_color(th.line_strong)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap(px(6.0));
                for (gi, (label, syls)) in PALETTE.iter().enumerate() {
                    let mut row = div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(4.0))
                        .child(div().w(px(38.0)).text_size(px(10.0)).text_color(th.ink_faint).child(*label));
                    for (i, syl) in syls.iter().enumerate() {
                        let rim = gi == 1;
                        let syl: &'static str = syl;
                        let w = weak.clone();
                        row = row.child(keys::hintable(
                            div()
                                .id(("chip", gi * 100 + i))
                                .relative()
                                .px(px(7.0))
                                .h(px(24.0))
                                .flex()
                                .items_center()
                                .rounded(px(999.0))
                                .border_1()
                                .border_color(if rim { th.accent2 } else if gi == 2 { th.line_strong } else { th.accent })
                                .bg(if rim { th.accent2_wash } else if gi == 2 { th.paper_raise } else { th.accent_wash })
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .cursor_pointer()
                                .hover(|st| st.opacity(0.8))
                                .child(syl)
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.insert_syllable(syl, cx))),
                            format!("chip-{syl}"),
                            Rc::new(move |_, cx: &mut App| {
                                let _ = w.update(cx, |this, cx| this.insert_syllable(syl, cx));
                            }),
                        ));
                    }
                    pal = pal.child(row);
                }
                drop(g);
                root = root.child(pal);
            }
        }

        drop(song);
        root
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_section(
    b: Bounds<Pixels>,
    s: usize,
    song: &Song,
    th: &Theme,
    offset: f32,
    cw: f32,
    cursor: Option<Cur>,
    sel: Option<Range4>,
    vmode: VMode,
    edit: &SharedString,
    shaped: &Rc<RefCell<HashMap<(SharedString, u32, u8), ShapedLine>>>,
    window: &mut Window,
    cx: &mut App,
) {
    // seção fora da área visível (rolagem vertical do editor): nada a pintar
    let visible = window.content_mask().bounds.intersect(&b);
    if f32::from(visible.size.width) <= 0.0 || f32::from(visible.size.height) <= 0.0 {
        return;
    }
    let ox = f32::from(b.origin.x);
    let oy = f32::from(b.origin.y);
    let w = f32::from(b.size.width);
    let rect = |x: f32, y: f32, w: f32, h: f32| Bounds::new(point(px(x), px(y)), size(px(w), px(h)));
    let rounded = |r: Bounds<Pixels>, rad: f32, bg: Hsla| quad(r, px(rad), bg, px(0.0), gpui_kit::transparent_black(), BorderStyle::Solid);
    let (a, bm) = song.section_range(s);
    let xs = measure_xs(song, s, cw);
    let mut cache = shaped.borrow_mut();
    let mut text = |window: &mut Window, cx: &mut App, t: &SharedString, size_px: u32, color: Hsla, ckey: u8, x: f32, y: f32, center_w: Option<f32>| {
        let line = cache.entry((t.clone(), size_px, ckey)).or_insert_with(|| {
            let mut run = window.text_style().to_run(t.len());
            run.color = color;
            window.text_system().shape_line(t.clone(), px(size_px as f32), &[run], None)
        });
        let lw = f32::from(line.width);
        let tx = match center_w {
            Some(cw) => x + (cw - lw) * 0.5,
            None => x,
        };
        let lh = size_px as f32 * 1.3;
        let _ = line.paint(point(px(tx), px(y)), px(lh), TextAlign::Left, None, window, cx);
    };

    window.with_content_mask(Some(gpui_kit::ContentMask { bounds: b }), |window| {
        let cells_top = oy + SEC_DYN_H + TIMELINE_H;
        let bar_x = ox + CELL_X0 - 10.0 - offset;
        for t in 0..song.tracks.len() {
            let y = cells_top + t as f32 * TRACK_H;
            window.paint_quad(fill(rect(bar_x, y, 3.0, CELL_H), th.ink));
        }

        for (i, m) in (a..bm).enumerate() {
            let meas = &song.measures[m];
            let mx = ox + xs[i] - offset;
            let mw = meas.len() as f32 * cw;
            if mx > ox + w || mx + mw + MEASURE_GAP < ox {
                continue;
            }
            let dyn_txt: SharedString = meas.dynamic.clone().unwrap_or_else(|| "·".into()).into();
            window.paint_quad(rounded(rect(mx + 2.0, oy + 3.0, 56.0, SEC_DYN_H - 6.0), 5.0, th.paper_raise));
            text(window, cx, &dyn_txt, 12, if meas.dynamic.is_some() { th.ink } else { th.ink_faint }, 1, mx + 10.0, oy + 4.0, None);
            text(window, cx, &"⌄".into(), 11, th.ink_faint, 1, mx + 40.0, oy + 2.0, None);
            for (j, g) in ["<", ">"].iter().enumerate() {
                let hx = mx + mw - 44.0 + j as f32 * 22.0;
                if hx > mx + 62.0 {
                    window.paint_quad(rounded(rect(hx, oy + 5.0, 18.0, SEC_DYN_H - 10.0), 4.0, th.paper_raise));
                    text(window, cx, &(*g).into(), 10, th.ink_faint, 1, hx, oy + 6.0, Some(18.0));
                }
            }

            let c0 = (((ox - mx) / cw).floor().max(0.0)) as usize;
            let c1 = ((((ox + w) - mx) / cw).ceil().max(0.0) as usize).min(meas.len());
            let sub = meas.subdivision;
            for k in c0..c1 {
                let x = mx + k as f32 * cw;
                let (lbl, strong): (SharedString, bool) = if k % sub == 0 {
                    ((k / sub + 1).to_string().into(), true)
                } else if sub == 2 {
                    ("e".into(), false)
                } else if sub == 4 {
                    (["", "e", "&", "a"][k % 4].into(), false)
                } else {
                    ("·".into(), false)
                };
                if cw >= 22.0 || strong {
                    text(window, cx, &lbl, 12, if strong { th.ink } else { th.ink_faint }, if strong { 2 } else { 1 }, x, oy + SEC_DYN_H + 7.0, Some(cw));
                }
            }

            for t in 0..song.tracks.len() {
                let y = cells_top + t as f32 * TRACK_H;
                let audible = song.audible(t);
                let row = &meas.cells[t];
                for k in c0..c1 {
                    let x = mx + k as f32 * cw;
                    let beat_start = k % sub == 0;
                    let lx = if beat_start { x + 2.0 } else { x + 1.0 };
                    let cell = row[k].as_deref();
                    let bg = if cell.is_some() { th.paper } else { th.cell_empty };
                    window.paint_quad(fill(rect(lx, y, x + cw - lx, CELL_H), bg));
                    if beat_start && k > 0 {
                        window.paint_quad(fill(rect(x, y, 2.0, CELL_H), th.line_strong));
                    }
                    if let Some(word) = cell {
                        let hits = model::resolve(word);
                        let mut groups: Vec<(f32, bool, f32)> = Vec::new();
                        for h in &hits {
                            if let Some(g) = groups.iter_mut().find(|g| (g.0 - h.offset).abs() < 1e-3) {
                                g.2 = g.2.max(h.vel);
                            } else {
                                groups.push((h.offset, h.rim, h.vel));
                            }
                        }
                        let inner = x + cw - lx;
                        for (gi, g) in groups.iter().enumerate() {
                            let gx0 = lx + g.0 * inner + 2.0;
                            let gx1 = groups.get(gi + 1).map(|n| lx + n.0 * inner - 1.0).unwrap_or(x + cw - 2.0);
                            let col = if g.1 { th.accent2 } else { th.accent };
                            let col = Theme::alpha(col, 0.8 + 0.2 * g.2.min(1.0));
                            window.paint_quad(quad(
                                rect(gx0, y + CELL_H - 15.0, (gx1 - gx0).max(2.0), 12.0),
                                gpui_kit::Corners { top_left: px(2.0), top_right: px(2.0), bottom_left: px(3.0), bottom_right: px(3.0) },
                                col,
                                px(0.0),
                                gpui_kit::transparent_black(),
                                BorderStyle::Solid,
                            ));
                        }
                        let editing_here = cursor == Some(Cur { m, t, k }) && !edit.is_empty();
                        if cw >= 26.0 && !editing_here {
                            let size_px = if cw >= 44.0 { 13 } else { 11 };
                            text(window, cx, &SharedString::from(word.to_string()), size_px, th.ink, 0, x, y + 5.0, Some(cw));
                        }
                    }
                    if !audible {
                        window.paint_quad(fill(rect(lx, y, x + cw - lx, CELL_H), Theme::alpha(th.bg, 0.55)));
                    }
                }
                window.paint_quad(rounded(rect(mx + 2.0, y + CELL_H + 4.0, mw - 4.0, TDYN_H), 4.0, Theme::alpha(th.paper_raise, 0.7)));
                text(window, cx, &"·".into(), 11, th.ink_faint, 1, mx + 10.0, y + CELL_H + 6.0, None);
            }
            let sx = mx + mw + MEASURE_GAP * 0.5 - 1.5;
            for t in 0..song.tracks.len() {
                let y = cells_top + t as f32 * TRACK_H;
                window.paint_quad(fill(rect(sx, y, 3.0, CELL_H), th.ink));
            }
        }

        // seleção: intervalo linear [g0, g1] × linhas [t0, t1], recortado por compasso desta seção
        if let Some((t0, t1, g0, g1)) = sel
            && bm > a
        {
            let mut acc = song.flat(a, 0);
            for (i, m) in (a..bm).enumerate() {
                let len = song.measures[m].len();
                let (lo, hi) = (g0.max(acc), g1.min(acc + len - 1));
                if lo <= hi {
                    let mx = ox + xs[i] - offset;
                    let x0 = mx + (lo - acc) as f32 * cw;
                    let x1 = mx + (hi - acc + 1) as f32 * cw;
                    let y0 = cells_top + t0 as f32 * TRACK_H;
                    let y1 = cells_top + t1 as f32 * TRACK_H + CELL_H;
                    let r = rect(x0, y0, x1 - x0, y1 - y0);
                    window.paint_quad(fill(r, Theme::alpha(th.accent2, 0.16)));
                    window.paint_quad(outline(r, th.accent2, BorderStyle::Solid));
                }
                acc += len;
            }
        }

        // cursor: normal/visual = bloco preenchido; insert/simples = contorno (+ texto em edição)
        if let Some(c) = cursor
            && c.m >= a
            && c.m < bm
        {
            let x = ox + xs[c.m - a] + c.k as f32 * cw - offset;
            let y = cells_top + c.t as f32 * TRACK_H;
            let r = rect(x, y, cw, CELL_H);
            let wash = if matches!(vmode, VMode::Normal | VMode::Visual | VMode::VLine) { Theme::alpha(th.accent, 0.18) } else { gpui_kit::transparent_black() };
            window.paint_quad(quad(r, px(3.0), wash, px(2.0), th.accent, BorderStyle::Solid));
            if !edit.is_empty() {
                let er = rect(x, y, (edit.len() as f32 * 9.0 + 14.0).max(cw), CELL_H);
                window.paint_quad(quad(er, px(3.0), th.accent_wash, px(2.0), th.accent, BorderStyle::Solid));
                let shown: SharedString = format!("{edit}▏").into();
                let mut run = window.text_style().to_run(shown.len());
                run.color = th.ink;
                let line = window.text_system().shape_line(shown, px(14.0), &[run], None);
                let _ = line.paint(point(px(x + 7.0), px(y + 11.0)), px(18.0), TextAlign::Left, None, window, cx);
            }
        }
    });
}
