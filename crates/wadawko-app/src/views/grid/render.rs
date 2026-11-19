//! Render da grade: blocos de seção (cabeçalho + rótulos das linhas + canvas das
//! células), popovers (dinâmica, edição da linha) e a paleta flutuante.

use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui_kit::{App, Bounds, ClickEvent, Context, FontWeight, Hsla, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, ScrollEvent, SharedString, Window, canvas, div, prelude::*, px};
use wadawko_core::ops;
use wadawko_core::{Song, TempoMark, kyai};

use super::paint::{PaintArgs, paint_section};
use super::vgeom::{self, Cols, LEFT_W, Layout};
use super::{DYNAMICS, GridEvent, GridView, InputTarget, KYAI_H, LABEL_W, Menu, SEC_DYN_H, SEC_GAP, SEC_HEAD_H, TDYN_H, TIMELINE_H, sec_key};
use crate::keys::{self, HintAct};
use crate::song_ext::SongExt;
use crate::theme::{Theme, parse_color, track_color};
use crate::ui::{self, small_btn_on};

type Op = Rc<dyn Fn(&mut GridView, &mut Context<GridView>)>;

/// chip clicável dos popovers
fn chip(id: impl Into<gpui_kit::ElementId>, th: &Theme, on: bool) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .px(px(8.0))
        .h(px(24.0))
        .flex()
        .flex_none()
        .items_center()
        .rounded(px(ui::R1))
        .border_1()
        .text_size(px(12.0))
        .cursor_pointer()
        .when(on, |d| d.bg(th.accent_wash).border_color(th.accent).text_color(th.accent))
        .when(!on, |d| d.bg(th.paper_raise).border_color(th.line).text_color(th.ink).hover(|s| s.bg(th.paper_hover)))
}

fn popover(th: &Theme) -> gpui_kit::Div {
    div()
        .absolute()
        .p(px(10.0))
        .rounded(px(ui::R0))
        .bg(th.paper)
        .border_1()
        .border_color(th.line_strong)
        .shadow_sm()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

impl GridView {
    pub(super) fn track_edit(&mut self, t: usize, cx: &mut Context<Self>, f: impl FnOnce(&mut Song, &str)) {
        let tid = self.song.tid(t).to_string();
        self.edit(cx, |s| f(s, &tid));
    }

    /// Popover de edição da linha (⚙): nome, instrumento, ordem, principal,
    /// sub-linhas, pessoas, voz do kiai.
    fn row_menu(&self, t: usize, left: f32, top: f32, th: &Theme, cx: &mut Context<Self>) -> gpui_kit::Div {
        let song = self.song.clone();
        let Some(track) = song.tracks.get(t) else { return div() };
        let tid = track.id.clone();
        let cur_def = track.def_id().to_string();
        let lbl = |s: &'static str| div().w(px(74.0)).flex_none().text_size(px(10.5)).text_color(th.ink_faint).child(s);
        let row = || div().flex().flex_wrap().items_center().gap(px(4.0));
        let click =
            |id: &'static str, label: SharedString, on: bool, cx: &mut Context<Self>, f: Op| chip((id, t), th, on).child(label).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| f(this, cx)));

        let mut insts = row().child(lbl("instrumento"));
        for (i, def) in ops::kit::palette_instruments(&song).into_iter().enumerate() {
            let did = def.id.clone();
            let on = def.id == cur_def;
            insts = insts.child(chip(("inst", t * 100 + i), th, on).child(SharedString::from(def.name.clone())).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let did = did.clone();
                this.track_edit(t, cx, |s, tid| ops::tracks::set_track_instrument(s, tid, &did));
            })));
        }

        // cor da linha (F1): 12 swatches da paleta + "padrão" (cor do instrumento)
        let cur_col = track.color_or_default().to_string();
        let weak = cx.weak_entity();
        let mut colors = row().child(lbl("cor"));
        for (i, c) in wadawko_core::model::TRACK_COLORS.iter().enumerate() {
            let on = c.eq_ignore_ascii_case(&cur_col);
            let hsla = parse_color(c).unwrap_or(th.ink_soft);
            let (c1, c2) = (c.to_string(), c.to_string());
            let w = weak.clone();
            colors = colors.child(keys::hintable(
                div()
                    .id(("col", t * 100 + i))
                    .relative()
                    .flex_none()
                    .size(px(14.0))
                    .rounded(px(ui::R1))
                    .bg(hsla)
                    .border_2()
                    .border_color(if on { th.ink } else { gpui_kit::transparent_black() })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.track_edit(t, cx, |s, tid| ops::tracks::set_track_color(s, tid, Some(&c1))))),
                format!("col-{t}-{i}"),
                Rc::new(move |_, cx: &mut App| {
                    let c2 = c2.clone();
                    let _ = w.update(cx, |this, cx| this.track_edit(t, cx, |s, tid| ops::tracks::set_track_color(s, tid, Some(&c2))));
                }),
            ));
        }
        colors = colors.child(click("r-col-def", "padrão".into(), track.color.is_none(), cx, Rc::new(move |this, cx| this.track_edit(t, cx, |s, tid| ops::tracks::set_track_color(s, tid, None)))));

        let mut people = row().child(lbl("pessoas"));
        if song.people().is_empty() {
            people = people.child(div().text_size(px(11.0)).text_color(th.ink_faint).child("nenhuma pessoa na música (página Pessoas)"));
        }
        for (i, p) in song.people().iter().enumerate() {
            let pid = p.id.clone();
            let on = track.person_ids().contains(&p.id);
            let col = p.color.as_deref().and_then(parse_color).unwrap_or(th.ink_soft);
            people = people.child(chip(("pers", t * 100 + i), th, on).gap(px(5.0)).child(div().size(px(8.0)).rounded_full().bg(col)).child(SharedString::from(p.name.clone())).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    let pid = pid.clone();
                    this.track_edit(t, cx, |s, tid| ops::people::toggle_track_person(s, tid, &pid));
                },
            )));
        }

        let (pitch, vary, gain) = kyai::kyai_voice_or_default(track.kyai.as_ref());
        let voice = |id: &'static str, label: String, cx: &mut Context<Self>, delta: (f64, f64, f64)| {
            chip((id, t), th, false).child(SharedString::from(label)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.track_edit(t, cx, |s, tid| {
                    if let Some(tr) = s.track_mut(tid) {
                        let (p, v, g) = kyai::kyai_voice_or_default(tr.kyai.as_ref());
                        let voice = tr.kyai.get_or_insert_with(Default::default);
                        voice.pitch = Some((p + delta.0).clamp(-12.0, 12.0));
                        voice.vary = Some(((v + delta.1) * 10.0).round().clamp(0.0, 10.0) / 10.0);
                        voice.gain = Some((g + delta.2).clamp(-24.0, 12.0));
                    }
                });
            }))
        };
        let val = |s: String| div().w(px(54.0)).flex().justify_center().text_size(px(12.0)).child(SharedString::from(s));
        let has_dyn = self.rows().dynr.get(t).copied().unwrap_or(false);
        let has_kyai = self.rows().kyai.get(t).copied().unwrap_or(false);
        let is_main = ops::tracks::is_main_track(&song, &tid);
        let tid_d = tid.clone();
        let tid_k = tid.clone();
        let name = track.name.clone();

        popover(th)
            .left(px(left))
            .top(px(top))
            .w(px(520.0))
            .child(
                row()
                    .child(div().flex_1().text_size(px(14.0)).font_weight(FontWeight::BOLD).child(SharedString::from(track.name.clone())))
                    .child(click(
                        "r-ren",
                        "✎ renomear".into(),
                        false,
                        cx,
                        Rc::new(move |this, cx| {
                            this.menu = None;
                            cx.emit(GridEvent::Ask(InputTarget::TrackName(tid.clone()), name.clone()));
                        }),
                    ))
                    .child(click(
                        "r-up",
                        "↑".into(),
                        false,
                        cx,
                        Rc::new(move |this, cx| {
                            this.track_edit(t, cx, |s, tid| ops::tracks::move_track(s, tid, -1));
                            if t > 0 {
                                this.menu = this.menu.take().map(|m| if let Menu::Row { s, .. } = m { Menu::Row { t: t - 1, s } } else { m });
                            }
                        }),
                    ))
                    .child(click(
                        "r-down",
                        "↓".into(),
                        false,
                        cx,
                        Rc::new(move |this, cx| {
                            let last = this.song.tracks.len().saturating_sub(1);
                            this.track_edit(t, cx, |s, tid| ops::tracks::move_track(s, tid, 1));
                            if t < last {
                                this.menu = this.menu.take().map(|m| if let Menu::Row { s, .. } = m { Menu::Row { t: t + 1, s } } else { m });
                            }
                        }),
                    ))
                    .child(click("r-main", "★ principal".into(), is_main, cx, Rc::new(move |this, cx| this.track_edit(t, cx, ops::tracks::toggle_main_track))))
                    .child(click(
                        "r-rm",
                        "✕ remover".into(),
                        false,
                        cx,
                        Rc::new(move |this, cx| {
                            this.menu = None;
                            this.track_edit(t, cx, ops::tracks::remove_track);
                        }),
                    ))
                    .child(click(
                        "r-close",
                        "fechar".into(),
                        false,
                        cx,
                        Rc::new(|this, cx| {
                            this.menu = None;
                            cx.notify();
                        }),
                    )),
            )
            .child(insts)
            .child(colors)
            .child(people)
            .child(
                row()
                    .child(lbl("sub-linhas"))
                    .child(click(
                        "r-dyn",
                        "f dinâmica própria".into(),
                        has_dyn,
                        cx,
                        Rc::new(move |this, cx| {
                            if !this.show_dyn.remove(&tid_d) {
                                this.show_dyn.insert(tid_d.clone());
                            }
                            cx.notify();
                        }),
                    ))
                    .child(click(
                        "r-kyai",
                        "気 kiai".into(),
                        has_kyai,
                        cx,
                        Rc::new(move |this, cx| {
                            if !this.show_kyai.remove(&tid_k) {
                                this.show_kyai.insert(tid_k.clone());
                            }
                            this.clamp_cursor();
                            cx.notify();
                        }),
                    )),
            )
            .child(
                row()
                    .child(lbl("voz do kiai"))
                    .child(div().text_size(px(11.0)).text_color(th.ink_soft).child("tom"))
                    .child(voice("kv-p-", "−".into(), cx, (-1.0, 0.0, 0.0)))
                    .child(val(format!("{pitch:+.0} st")))
                    .child(voice("kv-p+", "+".into(), cx, (1.0, 0.0, 0.0)))
                    .child(div().ml(px(6.0)).text_size(px(11.0)).text_color(th.ink_soft).child("variação"))
                    .child(voice("kv-v-", "−".into(), cx, (0.0, -0.1, 0.0)))
                    .child(val(format!("{:.0}%", vary * 100.0)))
                    .child(voice("kv-v+", "+".into(), cx, (0.0, 0.1, 0.0)))
                    .child(div().ml(px(6.0)).text_size(px(11.0)).text_color(th.ink_soft).child("volume"))
                    .child(voice("kv-g-", "−".into(), cx, (0.0, 0.0, -1.0)))
                    .child(val(format!("{gain:+.0} dB")))
                    .child(voice("kv-g+", "+".into(), cx, (0.0, 0.0, 1.0)))
                    .child(click("kv-test", "▶ testar".into(), false, cx, Rc::new(move |this, cx| this.preview_kyai(t, &["so".to_string(), "re".to_string()], cx)))),
            )
    }

    fn dyn_menu(&self, m: usize, track: Option<usize>, x: f32, y: f32, th: &Theme, cx: &mut Context<Self>) -> gpui_kit::Div {
        let cur = match track {
            None => self.song.measures.get(m).and_then(|x| x.dynamic),
            Some(t) => self.song.measures.get(m).and_then(|x| x.track_dynamic(self.song.tid(t))).and_then(|d| d.dynamic),
        };
        let mut row = div().flex().items_center().gap(px(4.0));
        row = row.child(chip("dyn-none", th, cur.is_none()).child("—").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.menu = None;
            this.set_dynamic(m, track, None, cx);
        })));
        for (i, d) in DYNAMICS.into_iter().enumerate() {
            row = row.child(chip(("dyn", i), th, cur == Some(d)).italic().font_weight(FontWeight::BOLD).child(d.as_str()).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.menu = None;
                this.set_dynamic(m, track, Some(d), cx);
            })));
        }
        popover(th).left(px(x.max(0.0))).top(px(y)).child(row)
    }
}

impl GridView {
    /// Popover da mudança de andamento do compasso `m`: alvo (±1/±5 BPM), duração
    /// em compassos (0 = na hora) e remover. Os ajustes não fecham o popover.
    fn tempo_menu(&self, m: usize, x: f32, y: f32, th: &Theme, cx: &mut Context<Self>) -> gpui_kit::Div {
        let before = ops::measures::tempo_at_measure_starts(&self.song).get(m).copied().unwrap_or(self.song.tempo);
        let cur = self.song.measures.get(m).and_then(|x| x.tempo.clone());
        let target = cur.as_ref().map_or(before, |t| t.target);
        let span = cur.as_ref().map(|t| t.span);
        let caption = |t: &'static str| div().w(px(58.0)).text_size(px(11.0)).text_color(th.ink_faint).child(t);

        let mut alvo = div().flex().items_center().gap(px(4.0)).child(caption("alvo"));
        for (i, d) in [-5.0, -1.0].into_iter().enumerate() {
            alvo = alvo.child(chip(("tempo-dn", i), th, false).child(SharedString::from(format!("{d:+.0}"))).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let span = this.song.measures.get(m).and_then(|x| x.tempo.as_ref()).map_or(1, |t| t.span);
                this.set_tempo(m, Some(TempoMark { target: target + d, span }), cx);
            })));
        }
        alvo = alvo.child(
            div()
                .w(px(70.0))
                .flex()
                .justify_center()
                .text_size(px(13.0))
                .font_weight(FontWeight::BOLD)
                .text_color(if cur.is_some() { th.ink } else { th.ink_faint })
                .child(SharedString::from(format!("♩ = {target:.0}"))),
        );
        for (i, d) in [1.0, 5.0].into_iter().enumerate() {
            alvo = alvo.child(chip(("tempo-up", i), th, false).child(SharedString::from(format!("{d:+.0}"))).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let span = this.song.measures.get(m).and_then(|x| x.tempo.as_ref()).map_or(1, |t| t.span);
                this.set_tempo(m, Some(TempoMark { target: target + d, span }), cx);
            })));
        }

        let mut dur = div().flex().items_center().gap(px(4.0)).child(caption("chegar em"));
        for (i, n) in [0usize, 1, 2, 4, 8].into_iter().enumerate() {
            let label: SharedString = match n {
                0 => "na hora".into(),
                1 => "1 compasso".into(),
                n => format!("{n}").into(),
            };
            dur = dur.child(chip(("tempo-span", i), th, span == Some(n)).child(label).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.set_tempo(m, Some(TempoMark { target, span: n }), cx);
            })));
        }

        let foot = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(div().flex_1().text_size(px(11.0)).text_color(th.ink_faint).child(SharedString::from(format!("antes: ♩ = {before:.0} · música: {:.0}", self.song.tempo))))
            .child(chip("tempo-none", th, false).child("remover").when(cur.is_none(), |d| d.opacity(0.4)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.menu = None;
                this.set_tempo(m, None, cx);
            })));

        // não deixa o popover (~420 px) passar da borda direita da grade
        let root_w = f32::from(self.geom.borrow().root.size.width);
        let x = if root_w > 0.0 { x.min(root_w - 430.0) } else { x };
        popover(th)
            .left(px(x.max(0.0)))
            .top(px(y))
            .child(div().text_size(px(12.0)).font_weight(FontWeight::BOLD).text_color(th.ink).child(SharedString::from(format!("Andamento a partir do compasso {}", m + 1))))
            .child(alvo)
            .child(dur)
            .child(foot)
    }
}

/// Roda `f` ao sair do escopo.
fn scopeguard(f: impl FnOnce()) -> impl Drop {
    struct G<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for G<F> {
        fn drop(&mut self) {
            if let Some(f) = self.0.take() {
                f()
            }
        }
    }
    G(Some(f))
}

/// Soma o tempo do render da grade em `GRID_RENDER_US` ao sair do escopo.
struct RenderTimer(Instant);
impl Drop for RenderTimer {
    fn drop(&mut self) {
        crate::shell::GRID_RENDER_US.fetch_add(self.0.elapsed().as_micros() as u64, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Render for GridView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::shell::DBG_GRID_RENDERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let _t_render = RenderTimer(Instant::now());
        if self.animating() {
            self.step_animation();
            window.request_animation_frame();
        }
        let st = self.state.read(cx);
        let th = st.theme;
        let selected: Vec<String> = st.tp.selected.clone();
        let (range, looping) = (st.tp.range, st.tp.looping);
        let song = self.song.clone();
        let secs = song.secs();
        let rows = self.rows();
        let vertical = self.layout == Layout::Vertical;
        // "ajustar à largura": a coluna de nota encolhe até COL_W_MIN antes de rolar em x
        let extras = Cols::new(&rows, self.col_w).extras();
        let n_rows = rows.y.len();
        let fit = move |root_w: f32, want: f32| vgeom::fit_col_w(want, root_w, n_rows, extras);
        self.col_w_eff = fit(f32::from(self.geom.borrow().root.size.width), self.col_w);
        let cols = Cols::new(&rows, self.col_w_eff);
        {
            let mut g = self.geom.borrow_mut();
            g.offset = self.offset;
            g.cw = self.cw;
            g.rows = rows.clone();
            g.layout = self.layout;
            g.cols = cols.clone();
            g.row_h = self.row_h;
            g.sections.resize(secs.len(), Bounds::default());
        }
        // cabeçalho de colunas (vertical): view em cache, notificada só quando a chave muda
        if vertical {
            if self.col_head_view.is_none() {
                let grid = cx.entity().downgrade();
                self.col_head_view = Some(cx.new(|_| super::vrender::ColHeadView { grid }));
            }
            let key = self.col_head_key(cx);
            if key != self.col_head_key
                && let Some(v) = &self.col_head_view
            {
                self.col_head_key = key;
                let id = v.entity_id();
                cx.defer(move |cx| cx.notify(id));
            }
        }
        let geom = self.geom.clone();
        let weak = cx.entity().downgrade();
        let weak_fit = weak.clone();
        // barra da seção se adapta à largura da grade (do último frame; 0 = ainda sem medida → completa)
        let bar = BarFit::for_width(f32::from(self.geom.borrow().root.size.width));
        let bar_used = bar;
        let (want_w, eff_used) = (self.col_w, self.col_w_eff);
        let rails = ops::people::rails_for_tracks(&song);
        let rail_w = rails.len() as f32 * 5.0 + if rails.is_empty() { 0.0 } else { 4.0 };
        self.geom.borrow_mut().label_pad = 6.0 + rail_w;

        let mut root = div()
            .id("grid-root")
            .relative()
            .w_full()
            .flex()
            .flex_col()
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    // soltou fora de um cabeçalho: cancela o arrasto de seção
                    if this.drag_sec.take().is_some() {
                        this.drag_over = None;
                        cx.notify();
                    }
                }),
            )
            .on_scroll(cx.listener(|this, ev: &ScrollEvent, _w, cx| {
                let d = ev.delta.pixel_delta(px(20.0));
                let (dx, dy) = (f32::from(d.x), f32::from(d.y));
                if ev.modifiers.control {
                    let a = this.geom.borrow().sections.first().map(|b| f32::from(ev.position.x) - f32::from(b.origin.x)).unwrap_or(0.0);
                    this.zoom_at((dy * 0.004).exp(), a.max(0.0));
                    if this.layout == Layout::Vertical {
                        cx.emit(GridEvent::SettingsChanged);
                    }
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
                    move |b, _, cx| {
                        geom.borrow_mut().root = b;
                        // a largura mudou de faixa (redimensionou a janela, abriu/fechou a
                        // lateral): re-renderiza para a barra das seções se reorganizar
                        if BarFit::for_width(f32::from(b.size.width)) != bar_used || (vertical && fit(f32::from(b.size.width), want_w) != eff_used) {
                            let _ = weak_fit.update(cx, |_, cx| cx.notify());
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );

        let mut y_acc = 0.0f32;
        let mut row_menu_at: Option<(usize, f32)> = None;
        // seções em views próprias, em cache (ver `SectionView`): a chave junta tudo que o
        // desenho de uma seção lê; só a seção sob o mouse entra a mais na dela
        let global = self.sections_key(&selected, range, looping, bar, cx);
        self.sec_views.truncate(secs.len());
        self.sec_keys.truncate(secs.len());
        while self.sec_views.len() < secs.len() {
            let i = self.sec_views.len();
            let grid = cx.entity().downgrade();
            self.sec_views.push(cx.new(|_| SectionView { grid, s: i }));
        }
        for (s, sec) in secs.iter().enumerate() {
            let collapsed = self.is_collapsed(sec);
            let key = crate::shell::chrome::key(|k| {
                std::hash::Hash::hash(&(global, self.hover_sec == Some(s)), k);
            });
            let view = self.sec_views[s].clone();
            if self.sec_keys.get(s) != Some(&key) {
                self.sec_keys.resize(secs.len(), 0);
                self.sec_keys[s] = key;
                let id = view.entity_id();
                cx.defer(move |cx| cx.notify(id));
            }
            let mut h = SEC_HEAD_H;
            y_acc += SEC_HEAD_H;
            if !collapsed {
                let body = if vertical { vgeom::section_body_h(&song, s, self.row_h) } else { rows.body_h() };
                if !vertical
                    && let Some(Menu::Row { t, s: ms }) = self.menu
                    && ms == s
                    && t < rows.y.len()
                {
                    row_menu_at = Some((t, y_acc + SEC_DYN_H + TIMELINE_H + rows.y[t] + rows.cell_h + 4.0));
                }
                y_acc += body;
                h += body;
            } else if let Some(slot) = self.geom.borrow_mut().sections.get_mut(s) {
                *slot = Bounds::default();
            }
            y_acc += SEC_GAP;
            root = root.child(div().w_full().flex_none().mb(px(SEC_GAP)).child(view.cached(gpui_kit::StyleRefinement::default().w_full().h(px(h)))));
        }

        // ── popovers ──
        match self.menu.clone() {
            Some(Menu::Dyn { m, track, x, y }) => root = root.child(self.dyn_menu(m, track, x, y, &th, cx)),
            Some(Menu::Tempo { m, x, y }) => root = root.child(self.tempo_menu(m, x, y, &th, cx)),
            Some(Menu::Row { t, .. }) if vertical => {
                // abaixo do cabeçalho de colunas (fixo): no topo da área visível do editor
                let (top, left) = {
                    let g = self.geom.borrow();
                    let top = (f32::from(g.viewport.origin.y) - f32::from(g.root.origin.y)).max(0.0) + 4.0;
                    let x = LEFT_W + cols.x.get(t).copied().unwrap_or(0.0) - self.offset;
                    let max_x = (f32::from(g.root.size.width) - 530.0).max(0.0);
                    (top, x.clamp(0.0, max_x))
                };
                root = root.child(self.row_menu(t, left, top, &th, cx));
            }
            Some(Menu::Row { t, .. }) => {
                if let Some((_, top)) = row_menu_at {
                    root = root.child(self.row_menu(t, 8.0, top, &th, cx));
                }
            }
            None => {}
        }
        root
    }
}

impl GridView {
    /// Chave comum a todas as seções: TUDO o que `render_section` lê da grade e do
    /// estado, menos a seção sob o mouse (essa entra só na chave dela). Ao
    /// acrescentar algo que a seção mostre, ponha aqui — senão ela fica velha.
    fn sections_key(&self, selected: &[String], range: Option<wadawko_core::timeline::Region>, looping: bool, bar: BarFit, cx: &App) -> u64 {
        let st = self.state.read(cx);
        let mut collapsed: Vec<&String> = self.collapsed.iter().collect();
        collapsed.sort();
        let mut dyn_rows: Vec<&String> = self.show_dyn.iter().collect();
        dyn_rows.sort();
        let mut kyai_rows: Vec<&String> = self.show_kyai.iter().collect();
        kyai_rows.sort();
        crate::shell::chrome::key(|k| {
            use std::hash::Hash;
            (Arc::as_ptr(&self.song) as usize, st.revision, st.theme.dark, selected, format!("{range:?}"), looping, format!("{bar:?}")).hash(k);
            (format!("{:?}", self.cursor), format!("{:?}", self.anchor), self.vmode, format!("{:?}", self.lane), &self.edit).hash(k);
            (format!("{:?}", self.menu), format!("{:?}", self.move_drag), format!("{:?}", self.loop_drag), self.drag_sec, self.drag_over).hash(k);
            (self.cw.to_bits(), self.offset.to_bits(), self.cell_h.to_bits(), collapsed, dyn_rows, kyai_rows).hash(k);
            (self.layout, self.col_w_eff.to_bits(), self.row_h.to_bits(), self.show_fx_all, {
                let mut v: Vec<&String> = self.show_fx.iter().collect();
                v.sort();
                v
            })
                .hash(k);
        })
    }

    /// Uma seção da grade (cabeçalho + rótulos + células). Chamada pela
    /// [`SectionView`] da seção, que fica em cache: um hover num botão daqui
    /// re-renderiza só esta seção, não a grade inteira.
    pub(crate) fn render_section(&mut self, s: usize, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let st = self.state.read(cx);
        let th = st.theme;
        let selected: Vec<String> = st.tp.selected.clone();
        let (range, looping) = (st.tp.range, st.tp.looping);
        let song = self.song.clone();
        let secs = song.secs();
        let rows = self.rows();
        let weak = cx.entity().downgrade();
        // barra da seção se adapta à largura da grade (do último frame; 0 = ainda sem medida → completa)
        let bar = BarFit::for_width(f32::from(self.geom.borrow().root.size.width));
        let rails = ops::people::rails_for_tracks(&song);
        let rail_w = rails.len() as f32 * 5.0 + if rails.is_empty() { 0.0 } else { 4.0 };

        // botão pequeno (20×16) = clique + alvo de salto com a MESMA operação; aceso na cor dada
        let mkc = |id: String, label: &'static str, on: bool, color: Hsla, op: Op| {
            let w1 = weak.clone();
            let w2 = weak.clone();
            let op2 = op.clone();
            let act: HintAct = Rc::new(move |_w: &mut Window, cx: &mut App| {
                let _ = w2.update(cx, |this, cx| op2(this, cx));
            });
            keys::hintable(
                small_btn_on(SharedString::from(id.clone()), label, &th, on, color).on_click(move |_: &ClickEvent, _, cx: &mut App| {
                    // não deixa o clique chegar à barra da seção (que (des)seleciona)
                    cx.stop_propagation();
                    let _ = w1.update(cx, |this, cx| op(this, cx));
                }),
                id,
                act,
            )
        };
        let mk = |id: String, label: &'static str, on: bool, op: Op| mkc(id, label, on, th.accent, op);

        let Some(sec) = secs.get(s) else { return div().into_any_element() };
        let sec_meta = sec.id.as_deref().and_then(|id| song.section(id));
        let sec_col = sec_meta.and_then(|m| ops::structure::section_color(&song, m)).and_then(crate::theme::parse_color);
        let group_name = sec_meta.and_then(|m| m.group.as_deref()).and_then(|g| ops::structure::group(&song, g)).map(|g| g.name.clone());
        let collapsed = self.is_collapsed(sec);
        let rep = sec.repeat;
        let cur_sec = self.cursor.map(|c| song.section_of(c.m)) == Some(s);
        let sid = sec.id.clone();
        let is_sel = sid.as_ref().is_some_and(|id| selected.contains(id));
        let dragging_this = self.drag_sec == Some(s);
        let drop_here = self.drag_sec.is_some() && self.drag_over == Some(s) && !dragging_this;
        let key = sec_key(sec);
        let with_sec = |f: fn(&mut Song, &str)| -> Op {
            let sid = sid.clone();
            Rc::new(move |this: &mut GridView, cx: &mut Context<GridView>| {
                if let Some(id) = &sid {
                    this.edit(cx, |song| f(song, id));
                }
            })
        };
        let rep_op = |delta: i32| -> Op {
            let sid = sid.clone();
            Rc::new(move |this: &mut GridView, cx: &mut Context<GridView>| {
                if let Some(id) = &sid {
                    let n = (rep as i32 + delta).clamp(1, 16) as u32;
                    this.edit(cx, |song| ops::sections::set_section_repeat(song, id, n));
                }
            })
        };
        let meas_op = |add: bool| -> Op {
            let sid = sid.clone();
            Rc::new(move |this: &mut GridView, cx: &mut Context<GridView>| {
                this.edit(cx, |song| if add { ops::measures::add_measure(song, sid.as_deref()) } else { ops::measures::remove_measure(song, sid.as_deref()) });
            })
        };
        let sid_click = sid.clone();
        let sid_hint = sid.clone();
        let sid_ren = sid.clone();
        let sec_name = sec.name.clone();
        let key_col = key.clone();

        let hov = self.hover_sec == Some(s);
        // botão do cabeçalho (h 20): só aparece com o mouse sobre a barra (continua alvo de salto)
        let mkh = |id: String, label: &'static str, op: Op| mk(id, label, false, op).h(px(20.0)).w(px(22.0)).when(!hov, |d| d.opacity(0.0));

        let header = div()
            .id(("sec-head", s))
            .relative()
            .h(px(SEC_HEAD_H))
            .flex()
            .items_center()
            .overflow_hidden()
            .rounded(px(ui::R0))
            .bg(th.paper)
            .border_b_1()
            .border_color(th.line)
            .cursor_pointer()
            // região selecionada: wash azul + barra esquerda de 2 px
            .when(is_sel, |d| d.bg(th.accent2_wash).child(div().absolute().left_0().top_0().bottom_0().w(px(2.0)).bg(th.accent2)))
            .when(hov && !is_sel, |d| d.bg(th.paper_hover))
            .on_hover(cx.listener(move |this, over: &bool, _, cx| {
                if *over && this.hover_sec != Some(s) {
                    this.hover_sec = Some(s);
                    cx.notify();
                } else if !*over && this.hover_sec == Some(s) {
                    this.hover_sec = None;
                    cx.notify();
                }
            }))
            // clique em qualquer ponto da barra (nome, vazios, 𝄆 1× 𝄇…) seleciona SÓ esta
            // seção para tocar; com Ctrl, junta/tira das outras. Os botões param a propagação.
            .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                if let Some(id) = &sid_click {
                    let additive = ev.modifiers().control || ev.modifiers().platform;
                    this.state.update(cx, |st, cx| st.select_section(id, additive, cx));
                }
            }))
            .when(drop_here, |d| d.border_t_2().border_color(th.accent))
            .when(dragging_this, |d| d.opacity(0.5))
            .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
                if this.drag_sec.is_some() && ev.pressed_button == Some(MouseButton::Left) && this.drag_over != Some(s) {
                    this.drag_over = Some(s);
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(move |this, _: &MouseUpEvent, _, cx| this.drop_section(s, cx)))
            .child(
                div()
                    .w(px(LABEL_W))
                    .flex_none()
                    .pl(px(6.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .id(("sec-grip", s))
                            .w(px(20.0))
                            .h(px(20.0))
                            .flex_none()
                            .rounded(px(ui::R1))
                            .border_1()
                            .border_color(th.line_strong)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(th.ink_soft)
                            .text_size(px(11.0))
                            .cursor_grab()
                            .child("⠿")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                    this.drag_sec = Some(s);
                                    this.drag_over = Some(s);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    // cor da seção (ou do grupo) e o grupo a que ela pertence
                    .when_some(sec_col, |d, c| d.child(div().size(px(10.0)).flex_none().rounded(px(2.0)).bg(c)))
                    .when_some(group_name.clone(), |d, g| {
                        d.child(
                            div()
                                .flex_none()
                                .max_w(px(90.0))
                                .truncate()
                                .text_size(px(9.5))
                                .font_weight(FontWeight::BOLD)
                                .text_color(th.ink_faint)
                                .child(SharedString::from(format!("{} ›", g.to_uppercase()))),
                        )
                    })
                    .child({
                        let w = weak.clone();
                        let sid2 = sid_hint.clone();
                        keys::hintable(
                            div()
                                .id(("sec-name", s))
                                .relative()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(if cur_sec || is_sel { th.ink } else { th.ink_soft })
                                .cursor_pointer()
                                .child(SharedString::from(if is_sel { format!("▶ {}", sec.name) } else { sec.name.clone() })),
                            format!("s{s}-name"),
                            Rc::new(move |_, cx: &mut App| {
                                let _ = w.update(cx, |this, cx| {
                                    if let Some(id) = &sid2 {
                                        this.state.update(cx, |st, cx| st.select_section(id, false, cx));
                                    }
                                });
                            }),
                        )
                    })
                    .child(mkh(
                        format!("s{s}-ren"),
                        "✎",
                        Rc::new(move |_this, cx| {
                            if let Some(id) = &sid_ren {
                                cx.emit(GridEvent::Ask(InputTarget::SectionName(id.clone()), sec_name.clone()));
                            }
                        }),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .items_center()
                    .gap(px(4.0))
                    .when(bar == BarFit::Full, |d| d.child(div().text_color(th.ink_faint).text_size(px(12.0)).child("𝄆")))
                    .child(mkh(format!("s{s}-rep-"), "−", rep_op(-1)))
                    .child(div().text_size(px(11.0)).font_weight(FontWeight::BOLD).text_color(if rep > 1 { th.accent } else { th.ink_soft }).child(SharedString::from(format!("{rep}×"))))
                    .child(mkh(format!("s{s}-rep+"), "+", rep_op(1)))
                    .when(bar == BarFit::Full, |d| d.child(div().text_color(th.ink_faint).text_size(px(12.0)).child("𝄇")))
                    .child(
                        mk(
                            format!("s{s}-col"),
                            if collapsed { "▸" } else { "▾" },
                            false,
                            Rc::new(move |this, cx| {
                                if !this.collapsed.remove(&key_col) {
                                    this.collapsed.insert(key_col.clone());
                                }
                                cx.notify();
                            }),
                        )
                        .h(px(20.0))
                        .w(px(22.0)),
                    )
                    // compassos: completo "− 4 compassos +", compacto "− 4c +", apertado some
                    // (continua no Ctrl+K e nos atalhos de section::MeasureAdd/Remove)
                    .when(bar != BarFit::Tight, |d| {
                        let n = sec.end - sec.start;
                        let label = if bar == BarFit::Full { format!("{n} compasso{}", if n == 1 { "" } else { "s" }) } else { format!("{n}c") };
                        d.child(div().w(px(if bar == BarFit::Full { 8.0 } else { 2.0 })))
                            .child(mkh(format!("s{s}-m-"), "−", meas_op(false)))
                            .child(div().text_size(px(10.0)).text_color(th.ink_faint).child(SharedString::from(label)))
                            .child(mkh(format!("s{s}-m+"), "+", meas_op(true)))
                    }),
            )
            .child({
                let w1 = weak.clone();
                let w2 = weak.clone();
                let op = with_sec(|song, id| {
                    ops::sections::clone_section(song, id);
                });
                let op2 = op.clone();
                keys::hintable(
                    div()
                        .id(("clone", s))
                        .relative()
                        .flex_none()
                        .px(px(6.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .rounded(px(ui::R1))
                        .border_1()
                        .border_color(th.line_strong)
                        .bg(th.paper_raise)
                        .text_size(px(10.0))
                        .text_color(th.ink_soft)
                        .cursor_pointer()
                        .when(!hov, |d| d.opacity(0.0))
                        .hover(|st| st.bg(th.paper_hover))
                        .child(if bar == BarFit::Full { "⧉ clonar" } else { "⧉" })
                        .on_click(move |_: &ClickEvent, _, cx: &mut App| {
                            cx.stop_propagation();
                            let _ = w1.update(cx, |this, cx| op(this, cx));
                        }),
                    format!("s{s}-clone"),
                    Rc::new(move |_, cx: &mut App| {
                        let _ = w2.update(cx, |this, cx| op2(this, cx));
                    }),
                )
            })
            .child(mkh(format!("s{s}-rm"), "✕", with_sec(ops::sections::remove_section)).ml(px(8.0)).mr(px(8.0)));

        let mut block = div().w_full().flex().flex_col().child(header);

        if !collapsed && self.layout == Layout::Vertical {
            block = block.child(self.vsection_body(s, cx));
        } else if !collapsed {
            let mut labels = div()
                .w(px(LABEL_W))
                .flex_none()
                .flex()
                .flex_col()
                .child(div().h(px(SEC_DYN_H)).pl(px(6.0)).flex().items_center().text_size(px(10.0)).text_color(th.ink_faint).child("dinâmica · clique = pp…ff · < >"))
                .child(div().h(px(TIMELINE_H)).pl(px(6.0)).flex().items_center().text_size(px(10.0)).text_color(th.ink_faint).truncate().child("↺ loop em cima · ↔ arraste · clique = tocar daqui"));
            // listra de cor da linha (4 px, como o track header de uma DAW) + 2 px de respiro;
            // os trilhos de pessoa vêm depois dela
            const STRIPE_W: f32 = 6.0;
            for (t, tr) in song.tracks.iter().enumerate() {
                let audible = song.audible(t);
                let cur_row = self.cursor.map(|c| c.t) == Some(t) && cur_sec;
                let id = |n: &str| format!("s{s}-t{t}-{n}");
                let is_main = ops::tracks::is_main_track(&song, &tr.id);
                let tid = tr.id.clone();
                // trilhos de pessoa: uma barra colorida por pessoa que toca a linha
                let mut rail_el = div().absolute().left(px(STRIPE_W)).top_0().bottom_0().w(px(rail_w));
                for (ri, rail) in rails.iter().enumerate() {
                    if rail.rows.get(t).copied().unwrap_or(false) {
                        let col = rail.person.color.as_deref().and_then(parse_color).unwrap_or(th.ink_soft);
                        let last = !rail.rows.get(t + 1).copied().unwrap_or(false);
                        rail_el = rail_el.child(
                            div()
                                .absolute()
                                .left(px(ri as f32 * 5.0))
                                .top(px(if t > 0 && rail.rows[t - 1] { 0.0 } else { 3.0 }))
                                .h(px(if last { rows.cell_h - 3.0 } else { rows.row_h(t) }))
                                .w(px(3.0))
                                .rounded(px(ui::R0))
                                .bg(if ops::people::person_audible(&song, Some(&rail.person.id)) { col } else { Theme::alpha(col, 0.3) }),
                        );
                    }
                }
                // listra na cor da linha (F1), esmaecida quando a linha não soa
                let tcol = track_color(&song, tr);
                let stripe = div().absolute().left_0().top_0().h(px(rows.cell_h)).w(px(4.0)).bg(if audible { tcol } else { Theme::alpha(tcol, 0.4) });
                let mut label = div().relative().h(px(rows.row_h(t))).flex().flex_col().child(stripe).child(rail_el).child(
                    div()
                        .h(px(rows.cell_h))
                        .pl(px(STRIPE_W + rail_w))
                        .flex()
                        .items_center()
                        .gap(px(3.0))
                        .pr(px(8.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(if cur_row {
                                            th.accent
                                        } else if audible {
                                            th.ink
                                        } else {
                                            th.ink_faint
                                        })
                                        .truncate()
                                        .child(SharedString::from(if is_main { format!("★ {}", tr.name) } else { tr.name.clone() })),
                                )
                                // linha baixa (F16): só o nome
                                .when(rows.cell_h >= 32.0, |d| {
                                    d.child(div().text_size(px(10.0)).text_color(th.ink_faint).truncate().child(SharedString::from(ops::kit::instrument_label(&song, tr).to_string())))
                                }),
                        )
                        .child(mk(
                            id("set"),
                            "⚙",
                            self.menu == Some(Menu::Row { t, s }),
                            Rc::new(move |this, cx| {
                                this.menu = if this.menu == Some(Menu::Row { t, s }) { None } else { Some(Menu::Row { t, s }) };
                                cx.notify();
                            }),
                        ))
                        .child(mk(
                            id("dyn"),
                            "f",
                            rows.dynr[t],
                            Rc::new(move |this, cx| {
                                if !this.show_dyn.remove(&tid) {
                                    this.show_dyn.insert(tid.clone());
                                }
                                cx.notify();
                            }),
                        ))
                        // M âmbar, S azul, ★ accent (convenção de DAW)
                        .child(mkc(
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
                        ))
                        .child(mkc(
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
                        ))
                        .child(mk(id("star"), "★", is_main, Rc::new(move |this, cx| this.track_edit(t, cx, ops::tracks::toggle_main_track))))
                        .child(mk(id("rm"), "×", false, Rc::new(move |this, cx| this.track_edit(t, cx, ops::tracks::remove_track)))),
                );
                let sub_label = |text: String, h: f32| {
                    div()
                        .mt(px(if h == TDYN_H { 4.0 } else { 2.0 }))
                        .h(px(h))
                        .pl(px(STRIPE_W + rail_w))
                        .flex()
                        .items_center()
                        .text_size(px(10.0))
                        .italic()
                        .text_color(th.ink_faint)
                        .truncate()
                        .child(SharedString::from(text))
                };
                if rows.dynr[t] {
                    label = label.child(sub_label(format!("dinâmica · {}", tr.name), TDYN_H));
                }
                if rows.kyai[t] {
                    label = label.child(sub_label(format!("気 kiai · {}", tr.name), KYAI_H).when(!rows.dynr[t], |d| d.mt(px(4.0))));
                }
                labels = labels.child(label);
            }

            let geom2 = self.geom.clone();
            let stats = self.stats.clone();
            let args = PaintArgs {
                s,
                song: song.clone(),
                th,
                offset: self.offset,
                cw: self.cw,
                cursor: self.cursor,
                sel: self.anchor.and_then(|_| self.sel_range()),
                vmode: self.vmode,
                lane: self.lane,
                edit: SharedString::from(self.edit.clone()),
                rows: rows.clone(),
                move_drag: self.move_drag,
                range,
                looping,
                loop_preview: self.loop_drag.filter(|d| d.moved).map(|d| d.region()),
                shaped: self.shaped.clone(),
            };
            let cells = div()
                .id(("cells", s))
                .flex_1()
                .min_w_0()
                .h(px(rows.body_h()))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, ev: &MouseDownEvent, w, cx| {
                        // clicar na grade solta o slider do inspector (as setas voltam a mover o cursor)
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
                            paint_section(b, &args, window, cx);
                            stats.borrow_mut().grid_paint.record(t0.elapsed());
                        },
                    )
                    .size_full(),
                );
            block = block.child(div().flex().child(labels).child(cells));
        } else if let Some(slot) = self.geom.borrow_mut().sections.get_mut(s) {
            *slot = Bounds::default();
        }
        block.into_any_element()
    }
}

/// Uma seção da grade como view própria, desenhada em cache (`AnyView::cached`).
/// A `GridView` calcula uma chave por seção a cada render e só notifica as
/// seções cuja chave mudou.
pub struct SectionView {
    grid: gpui_kit::WeakEntity<GridView>,
    s: usize,
}

impl Render for SectionView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.s;
        let t0 = Instant::now();
        let _done = scopeguard(move || {
            crate::shell::SEC_RENDERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            crate::shell::SEC_RENDER_US.fetch_add(t0.elapsed().as_micros() as u64, std::sync::atomic::Ordering::Relaxed);
        });
        match self.grid.upgrade() {
            Some(g) => g.update(cx, |g, cx| g.render_section(s, cx)),
            None => div().into_any_element(),
        }
    }
}

/// Quanto cabe na barra da seção, pela largura da grade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarFit {
    /// tudo: 𝄆 − 2× + 𝄇 ▾ − 4 compassos + ⧉ clonar ✕
    Full,
    /// sem os 𝄆 𝄇, "4c", "⧉"
    Compact,
    /// também sem os controles de compasso
    Tight,
}

impl BarFit {
    /// Larguras medidas da barra: coluna do nome (`LABEL_W`) + controles
    /// (completa ≈ 425 px, compacta ≈ 280 px).
    pub fn for_width(grid_w: f32) -> BarFit {
        let rest = grid_w - LABEL_W;
        if grid_w <= 0.0 || rest >= 430.0 {
            BarFit::Full
        } else if rest >= 285.0 {
            BarFit::Compact
        } else {
            BarFit::Tight
        }
    }
}

#[cfg(test)]
mod bar_tests {
    use super::*;

    #[test]
    fn bar_degrades_with_width() {
        assert_eq!(BarFit::for_width(0.0), BarFit::Full); // primeiro frame, sem medida
        assert_eq!(BarFit::for_width(1200.0), BarFit::Full);
        assert_eq!(BarFit::for_width(LABEL_W + 300.0), BarFit::Compact);
        assert_eq!(BarFit::for_width(410.0), BarFit::Tight); // janela de ~936 px com lateral e painel
    }
}
