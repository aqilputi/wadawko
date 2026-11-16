//! Render da grade: blocos de seção (cabeçalho + rótulos das linhas + canvas das
//! células), popovers (dinâmica, edição da linha) e a paleta flutuante.

use std::rc::Rc;
use std::time::Instant;

use gpui_kit::{App, Bounds, ClickEvent, Context, FontWeight, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, ScrollEvent, SharedString, Window, canvas, div, prelude::*, px};
use wadawko_core::ops;
use wadawko_core::{Song, kyai};

use super::paint::{PaintArgs, paint_section};
use super::{CELL_H, DYNAMICS, GridEvent, GridView, InputTarget, KYAI_H, LABEL_W, Menu, SEC_DYN_H, SEC_GAP, SEC_HEAD_H, TDYN_H, TIMELINE_H, sec_key};
use crate::keys::{self, HintAct};
use crate::song_ext::SongExt;
use crate::theme::{Theme, parse_color};
use crate::ui::small_btn;

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
        .rounded(px(6.0))
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
        .rounded(px(10.0))
        .bg(th.paper)
        .border_1()
        .border_color(th.line_strong)
        .shadow_lg()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

impl GridView {
    fn track_edit(&mut self, t: usize, cx: &mut Context<Self>, f: impl FnOnce(&mut Song, &str)) {
        let tid = self.song.tid(t).to_string();
        self.edit(cx, |s| f(s, &tid));
    }

    /// Popover de edição da linha (⚙): nome, instrumento, ordem, principal,
    /// sub-linhas, pessoas, voz do kiai.
    fn row_menu(&self, t: usize, top: f32, th: &Theme, cx: &mut Context<Self>) -> gpui_kit::Div {
        let song = self.song.clone();
        let Some(track) = song.tracks.get(t) else { return div() };
        let tid = track.id.clone();
        let cur_def = track.def_id().to_string();
        let lbl = |s: &'static str| div().w(px(74.0)).flex_none().text_size(px(10.5)).text_color(th.ink_faint).child(s);
        let row = || div().flex().flex_wrap().items_center().gap(px(4.0));
        let click = |id: &'static str, label: SharedString, on: bool, cx: &mut Context<Self>, f: Rc<dyn Fn(&mut GridView, &mut Context<GridView>)>| {
            chip((id, t), th, on).child(label).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| f(this, cx)))
        };

        let mut insts = row().child(lbl("instrumento"));
        for (i, def) in ops::kit::palette_instruments(&song).into_iter().enumerate() {
            let did = def.id.clone();
            let on = def.id == cur_def;
            insts = insts.child(chip(("inst", t * 100 + i), th, on).child(SharedString::from(def.name.clone())).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let did = did.clone();
                this.track_edit(t, cx, |s, tid| ops::tracks::set_track_instrument(s, tid, &did));
            })));
        }

        let mut people = row().child(lbl("pessoas"));
        if song.people().is_empty() {
            people = people.child(div().text_size(px(11.0)).text_color(th.ink_faint).child("nenhuma pessoa na música (página Pessoas)"));
        }
        for (i, p) in song.people().iter().enumerate() {
            let pid = p.id.clone();
            let on = track.person_ids().contains(&p.id);
            let col = p.color.as_deref().and_then(parse_color).unwrap_or(th.ink_soft);
            people = people.child(
                chip(("pers", t * 100 + i), th, on)
                    .gap(px(5.0))
                    .child(div().size(px(8.0)).rounded(px(4.0)).bg(col))
                    .child(SharedString::from(p.name.clone()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let pid = pid.clone();
                        this.track_edit(t, cx, |s, tid| ops::people::toggle_track_person(s, tid, &pid));
                    })),
            );
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
            .left(px(8.0))
            .top(px(top))
            .w(px(520.0))
            .child(
                row()
                    .child(div().flex_1().text_size(px(14.0)).font_weight(FontWeight::BOLD).child(SharedString::from(track.name.clone())))
                    .child(click("r-ren", "✎ renomear".into(), false, cx, Rc::new(move |this, cx| {
                        this.menu = None;
                        cx.emit(GridEvent::Ask(InputTarget::TrackName(tid.clone()), name.clone()));
                    })))
                    .child(click("r-up", "↑".into(), false, cx, Rc::new(move |this, cx| {
                        this.track_edit(t, cx, |s, tid| ops::tracks::move_track(s, tid, -1));
                        if t > 0 {
                            this.menu = this.menu.take().map(|m| if let Menu::Row { s, .. } = m { Menu::Row { t: t - 1, s } } else { m });
                        }
                    })))
                    .child(click("r-down", "↓".into(), false, cx, Rc::new(move |this, cx| {
                        let last = this.song.tracks.len().saturating_sub(1);
                        this.track_edit(t, cx, |s, tid| ops::tracks::move_track(s, tid, 1));
                        if t < last {
                            this.menu = this.menu.take().map(|m| if let Menu::Row { s, .. } = m { Menu::Row { t: t + 1, s } } else { m });
                        }
                    })))
                    .child(click("r-main", "★ principal".into(), is_main, cx, Rc::new(move |this, cx| this.track_edit(t, cx, ops::tracks::toggle_main_track))))
                    .child(click("r-rm", "✕ remover".into(), false, cx, Rc::new(move |this, cx| {
                        this.menu = None;
                        this.track_edit(t, cx, ops::tracks::remove_track);
                    })))
                    .child(click("r-close", "fechar".into(), false, cx, Rc::new(|this, cx| {
                        this.menu = None;
                        cx.notify();
                    }))),
            )
            .child(insts)
            .child(people)
            .child(
                row()
                    .child(lbl("sub-linhas"))
                    .child(click("r-dyn", "f dinâmica própria".into(), has_dyn, cx, Rc::new(move |this, cx| {
                        if !this.show_dyn.remove(&tid_d) {
                            this.show_dyn.insert(tid_d.clone());
                        }
                        cx.notify();
                    })))
                    .child(click("r-kyai", "気 kiai".into(), has_kyai, cx, Rc::new(move |this, cx| {
                        if !this.show_kyai.remove(&tid_k) {
                            this.show_kyai.insert(tid_k.clone());
                        }
                        this.clamp_cursor();
                        cx.notify();
                    }))),
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

impl Render for GridView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::shell::DBG_GRID_RENDERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self.animating() {
            self.step_animation();
            window.request_animation_frame();
        }
        let st = self.state.read(cx);
        let th = st.theme;
        let selected: Vec<String> = st.tp.selected.clone();
        let song = self.song.clone();
        let secs = song.secs();
        let rows = self.rows();
        {
            let mut g = self.geom.borrow_mut();
            g.offset = self.offset;
            g.cw = self.cw;
            g.rows = rows.clone();
            g.sections.resize(secs.len(), Bounds::default());
        }
        let geom = self.geom.clone();
        let weak = cx.entity().downgrade();
        let weak_fit = weak.clone();
        // barra da seção se adapta à largura da grade (do último frame; 0 = ainda sem medida → completa)
        let bar = BarFit::for_width(f32::from(self.geom.borrow().root.size.width));
        let bar_used = bar;
        let rails = ops::people::rails_for_tracks(&song);
        let rail_w = rails.len() as f32 * 5.0 + if rails.is_empty() { 0.0 } else { 4.0 };

        // botão pequeno = clique + alvo de salto com a MESMA operação
        let mk = |id: String, label: &'static str, on: bool, op: Op| {
            let w1 = weak.clone();
            let w2 = weak.clone();
            let op2 = op.clone();
            let act: HintAct = Rc::new(move |_w: &mut Window, cx: &mut App| {
                let _ = w2.update(cx, |this, cx| op2(this, cx));
            });
            keys::hintable(
                small_btn(SharedString::from(id.clone()), label, &th, on).on_click(move |_: &ClickEvent, _, cx: &mut App| {
                    // não deixa o clique chegar à barra da seção (que (des)seleciona)
                    cx.stop_propagation();
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
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, cx| {
                // soltou fora de um cabeçalho: cancela o arrasto de seção
                if this.drag_sec.take().is_some() {
                    this.drag_over = None;
                    cx.notify();
                }
            }))
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
                    move |b, _, cx| {
                        geom.borrow_mut().root = b;
                        // a largura mudou de faixa (redimensionou a janela, abriu/fechou a
                        // lateral): re-renderiza para a barra das seções se reorganizar
                        if BarFit::for_width(f32::from(b.size.width)) != bar_used {
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
        for (s, sec) in secs.iter().enumerate() {
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

            let header = div()
                .id(("sec-head", s))
                .h(px(SEC_HEAD_H))
                .flex()
                .items_center()
                .overflow_hidden()
                .rounded(px(8.0))
                .cursor_pointer()
                .when(is_sel, |d| d.bg(th.accent_wash))
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
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .id(("sec-grip", s))
                                .w(px(24.0))
                                .h(px(24.0))
                                .flex_none()
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(th.line_strong)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(th.ink_soft)
                                .text_size(px(12.0))
                                .cursor_grab()
                                .child("⠿")
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                    this.drag_sec = Some(s);
                                    this.drag_over = Some(s);
                                    cx.stop_propagation();
                                    cx.notify();
                                })),
                        )
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
                                    .text_size(px(15.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .cursor_pointer()
                                    .when(cur_sec || is_sel, |d| d.text_color(th.accent))
                                    .child(SharedString::from(if is_sel { format!("▶ {}", sec.name) } else { sec.name.clone() }))
,
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
                        .child(mk(format!("s{s}-ren"), "✎", false, Rc::new(move |_this, cx| {
                            if let Some(id) = &sid_ren {
                                cx.emit(GridEvent::Ask(InputTarget::SectionName(id.clone()), sec_name.clone()));
                            }
                        }))),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .items_center()
                        .gap(px(6.0))
                        .when(bar == BarFit::Full, |d| d.child(div().text_color(th.accent).text_size(px(14.0)).child("𝄆")))
                        .child(mk(format!("s{s}-rep-"), "−", false, rep_op(-1)))
                        .child(div().text_size(px(13.0)).text_color(if rep > 1 { th.accent } else { th.ink_soft }).child(SharedString::from(format!("{rep}×"))))
                        .child(mk(format!("s{s}-rep+"), "+", false, rep_op(1)))
                        .when(bar == BarFit::Full, |d| d.child(div().text_color(th.accent).text_size(px(14.0)).child("𝄇")))
                        .child(mk(format!("s{s}-col"), if collapsed { "▸" } else { "▾" }, false, Rc::new(move |this, cx| {
                            if !this.collapsed.remove(&key_col) {
                                this.collapsed.insert(key_col.clone());
                            }
                            cx.notify();
                        })))
                        // compassos: completo "− 4 compassos +", compacto "− 4c +", apertado some
                        // (continua no Ctrl+K e nos atalhos de section::MeasureAdd/Remove)
                        .when(bar != BarFit::Tight, |d| {
                            let n = sec.end - sec.start;
                            let label = if bar == BarFit::Full { format!("{n} compasso{}", if n == 1 { "" } else { "s" }) } else { format!("{n}c") };
                            d.child(div().w(px(if bar == BarFit::Full { 8.0 } else { 2.0 })))
                                .child(mk(format!("s{s}-m-"), "−", false, meas_op(false)))
                                .child(div().text_size(px(11.0)).text_color(th.ink_faint).child(SharedString::from(label)))
                                .child(mk(format!("s{s}-m+"), "+", false, meas_op(true)))
                        })
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
                .child(mk(format!("s{s}-rm"), "✕", false, with_sec(ops::sections::remove_section)).ml(px(10.0)).mr(px(12.0)));

            let mut block = div().flex().flex_col().mb(px(SEC_GAP)).child(header);
            y_acc += SEC_HEAD_H;

            if !collapsed {
                if let Some(Menu::Row { t, s: ms }) = self.menu
                    && ms == s
                    && t < rows.y.len()
                {
                    row_menu_at = Some((t, y_acc + SEC_DYN_H + TIMELINE_H + rows.y[t] + CELL_H + 4.0));
                }
                let mut labels = div()
                    .w(px(LABEL_W))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .child(div().h(px(SEC_DYN_H)).flex().items_center().text_size(px(10.5)).text_color(th.ink_faint).child("dinâmica · clique = pp…ff · < >"))
                    .child(div().h(px(TIMELINE_H)).flex().items_center().text_size(px(10.5)).text_color(th.ink_faint).child("↔ arraste · clique = tocar daqui"));
                for (t, tr) in song.tracks.iter().enumerate() {
                    let audible = song.audible(t);
                    let cur_row = self.cursor.map(|c| c.t) == Some(t) && cur_sec;
                    let id = |n: &str| format!("s{s}-t{t}-{n}");
                    let is_main = ops::tracks::is_main_track(&song, &tr.id);
                    let tid = tr.id.clone();
                    // trilhos de pessoa: uma barra colorida por pessoa que toca a linha
                    let mut rail_el = div().absolute().left_0().top_0().bottom_0().w(px(rail_w));
                    for (ri, rail) in rails.iter().enumerate() {
                        if rail.rows.get(t).copied().unwrap_or(false) {
                            let col = rail.person.color.as_deref().and_then(parse_color).unwrap_or(th.ink_soft);
                            let last = !rail.rows.get(t + 1).copied().unwrap_or(false);
                            rail_el = rail_el.child(
                                div()
                                    .absolute()
                                    .left(px(ri as f32 * 5.0))
                                    .top(px(if t > 0 && rail.rows[t - 1] { 0.0 } else { 3.0 }))
                                    .h(px(if last { CELL_H - 3.0 } else { rows.row_h(t) }))
                                    .w(px(3.0))
                                    .rounded(px(1.5))
                                    .bg(if ops::people::person_audible(&song, Some(&rail.person.id)) { col } else { Theme::alpha(col, 0.3) }),
                            );
                        }
                    }
                    let mut label = div().relative().h(px(rows.row_h(t))).flex().flex_col().child(rail_el).child(
                        div()
                            .h(px(CELL_H))
                            .pl(px(rail_w))
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
                                            .child(SharedString::from(if is_main { format!("★ {}", tr.name) } else { tr.name.clone() })),
                                    )
                                    .child(div().text_size(px(11.0)).text_color(th.ink_faint).truncate().child(SharedString::from(ops::kit::instrument_label(&song, tr).to_string()))),
                            )
                            .child(mk(id("set"), "⚙", self.menu == Some(Menu::Row { t, s }), Rc::new(move |this, cx| {
                                this.menu = if this.menu == Some(Menu::Row { t, s }) { None } else { Some(Menu::Row { t, s }) };
                                cx.notify();
                            })))
                            .child(mk(id("dyn"), "f", rows.dynr[t], Rc::new(move |this, cx| {
                                if !this.show_dyn.remove(&tid) {
                                    this.show_dyn.insert(tid.clone());
                                }
                                cx.notify();
                            })))
                            .child(mk(id("mute"), "M", tr.muted, Rc::new(move |this, cx| {
                                this.track_edit(t, cx, |s, tid| {
                                    if let Some(tr) = s.track_mut(tid) {
                                        tr.muted = !tr.muted;
                                    }
                                })
                            })))
                            .child(mk(id("solo"), "S", tr.solo, Rc::new(move |this, cx| {
                                this.track_edit(t, cx, |s, tid| {
                                    if let Some(tr) = s.track_mut(tid) {
                                        tr.solo = !tr.solo;
                                    }
                                })
                            })))
                            .child(mk(id("star"), "★", is_main, Rc::new(move |this, cx| this.track_edit(t, cx, ops::tracks::toggle_main_track))))
                            .child(mk(id("rm"), "×", false, Rc::new(move |this, cx| this.track_edit(t, cx, ops::tracks::remove_track)))),
                    );
                    let sub_label = |text: String, h: f32| div().mt(px(if h == TDYN_H { 4.0 } else { 2.0 })).h(px(h)).pl(px(rail_w)).flex().items_center().text_size(px(10.0)).italic().text_color(th.ink_faint).truncate().child(SharedString::from(text));
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
                    shaped: self.shaped.clone(),
                };
                let cells = div()
                    .id(("cells", s))
                    .flex_1()
                    .min_w_0()
                    .h(px(rows.body_h()))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| this.mouse_down(s, ev, cx)))
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
                y_acc += rows.body_h();
            } else if let Some(slot) = self.geom.borrow_mut().sections.get_mut(s) {
                *slot = Bounds::default();
            }
            y_acc += SEC_GAP;
            root = root.child(block);
        }

        // ── popovers ──
        match self.menu.clone() {
            Some(Menu::Dyn { m, track, x, y }) => root = root.child(self.dyn_menu(m, track, x, y, &th, cx)),
            Some(Menu::Row { t, .. }) => {
                if let Some((_, top)) = row_menu_at {
                    root = root.child(self.row_menu(t, top, &th, cx));
                }
            }
            None => {}
        }
        root
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
