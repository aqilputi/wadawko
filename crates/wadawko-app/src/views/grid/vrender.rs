//! Render do editor vertical: corpo de uma seção (canvas com gutter, coluna do
//! compasso e colunas) e o **cabeçalho de colunas fixo** no topo do editor — uma
//! view em cache própria (`ColHeadView`), posta pelo Shell fora da área rolável,
//! que compartilha o `offset` horizontal com o conteúdo.

use std::rc::Rc;
use std::time::Instant;

use gpui_kit::{App, Context, FontWeight, Hsla, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, SharedString, Window, canvas, div, prelude::*, px};
use wadawko_core::ops;

use super::vgeom::{DYN_W, KYAI_W, LEFT_W, section_body_h};
use super::vpaint::{VPaintArgs, vpaint_section};
use super::{GridView, Menu};
use crate::keys::{self, HintAct};
use crate::song_ext::SongExt;
use crate::theme::{Theme, parse_color, track_color};
use crate::ui::small_btn_on;

/// renders do cabeçalho de colunas (diagnóstico do `--demo hoverstat`)
pub static COL_HEAD_RENDERS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// tempo total (µs) gasto montando o cabeçalho de colunas
pub static COL_HEAD_US: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Soma o tempo desde `t0` em `COL_HEAD_US` ao sair do escopo.
fn scopeguard_us(t0: Instant) -> impl Drop {
    struct G(Instant);
    impl Drop for G {
        fn drop(&mut self) {
            COL_HEAD_US.fetch_add(self.0.elapsed().as_micros() as u64, std::sync::atomic::Ordering::Relaxed);
        }
    }
    G(t0)
}

type Op = Rc<dyn Fn(&mut GridView, &mut Context<GridView>)>;

impl GridView {
    /// Chave do cabeçalho de colunas: tudo o que ele mostra.
    pub(crate) fn col_head_key(&self, cx: &App) -> u64 {
        let st = self.state.read(cx);
        crate::shell::chrome::key(|k| {
            use std::hash::Hash;
            (std::sync::Arc::as_ptr(&self.song) as usize, st.revision, st.theme.dark, self.layout, self.col_w_eff.to_bits(), self.offset.to_bits(), self.hover_col).hash(k);
            let menu_t = match self.menu {
                Some(Menu::Row { t, .. }) => Some(t),
                _ => None,
            };
            (menu_t, self.cursor.map(|c| c.t)).hash(k);
            // sub-colunas visíveis (dyn/kiai abertas à mão; as com conteúdo vêm da revisão)
            for set in [&self.show_dyn, &self.show_kyai] {
                let mut d: Vec<&String> = set.iter().collect();
                d.sort();
                d.hash(k);
            }
        })
    }

    /// Corpo de uma seção no editor vertical (o canvas, sem a banda).
    pub(crate) fn vsection_body(&mut self, s: usize, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let st = self.state.read(cx);
        let (range, looping) = (st.tp.range, st.tp.looping);
        let th = st.theme;
        let song = self.song.clone();
        let h = section_body_h(&song, s, self.row_h);
        let geom2 = self.geom.clone();
        let stats = self.stats.clone();
        let args = VPaintArgs {
            s,
            song,
            th,
            offset: self.offset,
            row_h: self.row_h,
            cols: self.cols(),
            cursor: self.cursor,
            sel: self.anchor.and_then(|_| self.sel_range()),
            vmode: self.vmode,
            lane: self.lane,
            edit: SharedString::from(self.edit.clone()),
            move_drag: self.move_drag,
            range,
            looping,
            loop_preview: self.loop_drag.filter(|d| d.moved).map(|d| d.region()),
            sec_color: None,
            shaped: self.shaped.clone(),
        };
        div()
            .id(("vcells", s))
            .w_full()
            .h(px(h))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, w, cx| {
                    if crate::views::widgets::clear_focus() {
                        w.refresh();
                    }
                    this.mouse_down(s, ev, cx)
                }),
            )
            .on_mouse_down(MouseButton::Right, cx.listener(move |this, ev: &MouseDownEvent, _, cx| this.mouse_down(s, ev, cx)))
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
                        vpaint_section(b, &args, window, cx);
                        stats.borrow_mut().grid_paint.record(t0.elapsed());
                    },
                )
                .size_full(),
            )
            .into_any_element()
    }

    /// Cabeçalho de colunas (44 px): barra de cor de 3 px, nome, instrumento, bolinhas
    /// de pessoa e os botões `⚙ M S ★ ×` (em hover; os ligados sempre).
    pub(crate) fn render_col_head(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        COL_HEAD_RENDERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let t0 = Instant::now();
        let _t = scopeguard_us(t0);
        let th = self.state.read(cx).theme;
        let song = self.song.clone();
        let cols = self.cols();
        let weak = cx.entity().downgrade();
        let geom = self.geom.clone();
        let cur_t = self.cursor.map(|c| c.t);
        let s_cur = self.cursor.map(|c| song.section_of(c.m)).unwrap_or(0);

        let mkc = |id: String, label: &'static str, on: bool, color: Hsla, op: Op| {
            let w1 = weak.clone();
            let w2 = weak.clone();
            let op2 = op.clone();
            let act: HintAct = Rc::new(move |_w: &mut Window, cx: &mut App| {
                let _ = w2.update(cx, |this, cx| op2(this, cx));
            });
            keys::hintable(
                small_btn_on(SharedString::from(id.clone()), label, &th, on, color).w(px(16.0)).h(px(16.0)).on_click(move |_, _, cx: &mut App| {
                    cx.stop_propagation();
                    let _ = w1.update(cx, |this, cx| op(this, cx));
                }),
                id,
                act,
            )
        };

        let mut strip = div().relative().flex().flex_none().left(px(-self.offset));
        for (t, tr) in song.tracks.iter().enumerate().take(cols.x.len()) {
            let hov = self.hover_col == Some(t);
            let audible = song.audible(t);
            let tcol = track_color(tr);
            let is_main = ops::tracks::is_main_track(&song, &tr.id);
            let id = |n: &str| format!("h-t{t}-{n}");
            let menu_on = matches!(self.menu, Some(Menu::Row { t: mt, .. }) if mt == t);
            // bolinhas de pessoa (8 px)
            let mut dots = div().flex().flex_none().gap(px(2.0));
            for pid in tr.person_ids().iter().take(4) {
                let col = song.people().iter().find(|p| &p.id == pid).and_then(|p| p.color.as_deref()).and_then(parse_color).unwrap_or(th.ink_soft);
                dots = dots.child(div().size(px(8.0)).rounded_full().bg(col));
            }
            let show = |on: bool| hov || on;
            let btns = div()
                .absolute()
                .right(px(2.0))
                .bottom(px(5.0))
                .flex()
                .gap(px(2.0))
                .when(hov, |d| d.bg(th.col_head))
                .child(
                    mkc(
                        id("set"),
                        "⚙",
                        menu_on,
                        th.accent,
                        Rc::new(move |this, cx| {
                            this.menu = if matches!(this.menu, Some(Menu::Row { t: mt, .. }) if mt == t) { None } else { Some(Menu::Row { t, s: s_cur }) };
                            cx.notify();
                        }),
                    )
                    .when(!show(menu_on), |d| d.opacity(0.0)),
                )
                .child(
                    mkc(
                        id("mute"),
                        "M",
                        tr.muted,
                        th.warn,
                        Rc::new(move |this, cx| {
                            this.track_edit(t, cx, |s, tid| {
                                if let Some(tr) = s.track_mut(tid) {
                                    tr.muted = !tr.muted;
                                }
                            })
                        }),
                    )
                    .when(!show(tr.muted), |d| d.opacity(0.0)),
                )
                .child(
                    mkc(
                        id("solo"),
                        "S",
                        tr.solo,
                        th.accent2,
                        Rc::new(move |this, cx| {
                            this.track_edit(t, cx, |s, tid| {
                                if let Some(tr) = s.track_mut(tid) {
                                    tr.solo = !tr.solo;
                                }
                            })
                        }),
                    )
                    .when(!show(tr.solo), |d| d.opacity(0.0)),
                )
                .child(mkc(id("star"), "★", is_main, th.accent, Rc::new(move |this, cx| this.track_edit(t, cx, ops::tracks::toggle_main_track))).when(!show(is_main), |d| d.opacity(0.0)))
                .child(mkc(id("rm"), "×", false, th.accent, Rc::new(move |this, cx| this.track_edit(t, cx, ops::tracks::remove_track))).when(!hov, |d| d.opacity(0.0)));
            let sub_label =
                |text: &'static str, w: f32| div().w(px(w)).flex_none().h_full().flex().items_end().justify_center().pb(px(6.0)).text_size(px(10.0)).italic().text_color(th.ink_faint).child(text);
            let mut group = div()
                .id(("col-head", t))
                .relative()
                .w(px(cols.group_w[t]))
                .mr(px(super::vgeom::COL_GAP))
                .flex_none()
                .h_full()
                .flex()
                .bg(if hov { th.paper_hover } else { th.col_head })
                .on_hover(cx.listener(move |this, over: &bool, _, cx| {
                    if *over && this.hover_col != Some(t) {
                        this.hover_col = Some(t);
                        cx.notify();
                    } else if !*over && this.hover_col == Some(t) {
                        this.hover_col = None;
                        cx.notify();
                    }
                }))
                // barra de cor da linha (3 px), esmaecida quando a linha não soa
                .child(div().absolute().left_0().right_0().top_0().h(px(3.0)).bg(if audible { tcol } else { Theme::alpha(tcol, 0.4) }))
                .child(
                    div()
                        .relative()
                        .w(px(cols.note_w))
                        .flex_none()
                        .pt(px(6.0))
                        .px(px(6.0))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(if cur_t == Some(t) {
                                            th.accent
                                        } else if audible {
                                            th.ink
                                        } else {
                                            th.ink_faint
                                        })
                                        .child(SharedString::from(tr.name.clone())),
                                )
                                .child(dots),
                        )
                        .child(div().truncate().text_size(px(10.0)).text_color(th.ink_faint).child(SharedString::from(ops::kit::instrument_label(&song, tr).to_string())))
                        .child(btns),
                );
            if cols.dyn_x[t].is_some() {
                group = group.child(sub_label("f", DYN_W));
            }
            if cols.kyai_x[t].is_some() {
                group = group.child(sub_label("気 kiai", KYAI_W));
            }
            strip = strip.child(group);
        }

        div()
            .id("col-head-root")
            .relative()
            .size_full()
            .flex()
            .overflow_hidden()
            .bg(th.bg)
            .child(
                canvas(
                    move |b, _, _| {
                        geom.borrow_mut().col_head = b;
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            // gutter + coluna do compasso: fixos
            .child(div().w(px(LEFT_W)).flex_none().h_full().flex().items_end().pb(px(6.0)).pl(px(12.0)).gap(px(18.0)).text_size(px(10.0)).text_color(th.ink_faint).child("c").child("compasso"))
            .child(div().flex_1().min_w_0().h_full().overflow_hidden().child(strip.h_full()))
            .into_any_element()
    }
}

/// O cabeçalho de colunas como view própria, desenhada em cache pelo Shell.
pub struct ColHeadView {
    pub(crate) grid: gpui_kit::WeakEntity<GridView>,
}

impl Render for ColHeadView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.grid.upgrade() {
            Some(g) => g.update(cx, |g, cx| g.render_col_head(cx)),
            None => div().into_any_element(),
        }
    }
}
