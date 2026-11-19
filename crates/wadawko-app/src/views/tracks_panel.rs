//! Aba LINHAS do painel da esquerda: a lista de linhas (instrumentos) da música
//! como os *track headers* de uma DAW. Cada linha mostra a cor, o nome, o
//! instrumento e M/S/★; a linha do cursor abre com o que o inspector mostra —
//! cor (e de onde vem), volume, pan, sub-linhas (dinâmica, kiai, efeitos),
//! pessoas e mover/renomear/remover. Clicar numa linha leva o cursor da grade
//! para ela.

use std::hash::Hash;
use std::rc::Rc;

use gpui_kit::{App, ClickEvent, Context, FontWeight, SharedString, Window, div, prelude::*, px};
use wadawko_core::model::TRACK_COLORS;
use wadawko_core::ops;

use crate::keys;
use crate::shell::Shell;
use crate::theme::{Theme, parse_color, track_color};
use crate::ui;
use crate::views::mixer::{self, VOL_MAX, VOL_MIN};
use crate::views::widgets::{self, Slider, chip};

/// Qual aba o painel da esquerda mostra.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LeftTab {
    #[default]
    Sections,
    Tracks,
}

type ShellOp = Rc<dyn Fn(&mut Shell, &mut Context<Shell>)>;

impl Shell {
    /// O que a aba LINHAS lê além da música (para a chave de cache do painel).
    pub(crate) fn tracks_panel_key(&self, k: &mut std::collections::hash_map::DefaultHasher, cx: &App) {
        let g = self.grid.read(cx);
        let rows = g.rows();
        (g.cursor.map(|c| c.t), &rows.dynr, &rows.kyai, &rows.fx).hash(k);
    }

    pub(crate) fn tracks_list(&self, th: &Theme, cx: &mut Context<Self>) -> gpui_kit::Div {
        let th2 = *th;
        let song = self.state.read(cx).song().clone();
        let g = self.grid.read(cx);
        let cur_t = g.cursor.map(|c| c.t);
        let rows = g.rows();
        let weak = cx.entity().downgrade();
        let state = self.state.clone();
        let mut list = div().flex().flex_col().gap(px(2.0)).py(px(4.0));
        let act = |id: String, label: &'static str, on: bool, col: gpui_kit::Hsla, f: ShellOp| {
            let (w1, w2, f2) = (weak.clone(), weak.clone(), f.clone());
            keys::hintable(
                ui::btn(SharedString::from(id.clone()), &th2)
                    .h(px(18.0))
                    .w(px(20.0))
                    .px(px(0.0))
                    .text_size(px(10.0))
                    .font_weight(FontWeight::BOLD)
                    .when(on, |d| d.bg(col).border_color(col).text_color(th2.on_accent))
                    .child(label)
                    .on_click(move |_: &ClickEvent, _, cx: &mut App| {
                        cx.stop_propagation();
                        let _ = w1.update(cx, |sh, cx| f(sh, cx));
                    }),
                id,
                Rc::new(move |_: &mut Window, cx: &mut App| {
                    let _ = w2.update(cx, |sh, cx| f2(sh, cx));
                }),
            )
        };
        let lchip = |id: String, label: String, on: bool, f: ShellOp| {
            let (w1, w2, f2) = (weak.clone(), weak.clone(), f.clone());
            chip(
                id,
                label,
                &th2,
                on,
                Rc::new(move |_: &mut Window, cx: &mut App| {
                    let _ = w2.update(cx, |sh, cx| f2(sh, cx));
                }),
            )
            .h(px(20.0))
            .px(px(5.0))
            .text_size(px(10.5))
            .on_click(move |_: &ClickEvent, _, cx: &mut App| {
                let _ = w1.update(cx, |sh, cx| f(sh, cx));
            })
        };
        let lbl = |s: &'static str| div().w(px(44.0)).flex_none().text_size(px(10.0)).text_color(th2.ink_faint).child(s);
        let row = || div().flex().items_center().flex_wrap().gap(px(4.0));
        for (t, tr) in song.tracks.iter().enumerate() {
            let tid = tr.id.clone();
            let on = cur_t == Some(t);
            let tcol = track_color(&song, tr);
            let audible = !tr.muted;
            let is_main = ops::tracks::is_main_track(&song, &tid);
            let (t1, t2, t3) = (tid.clone(), tid.clone(), tid.clone());
            let head = div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(div().w(px(4.0)).h(px(28.0)).flex_none().bg(if audible { tcol } else { Theme::alpha(tcol, 0.4) }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().truncate().text_size(px(12.0)).font_weight(FontWeight::BOLD).text_color(if on { th2.ink } else { th2.ink_soft }).child(SharedString::from(tr.name.clone())))
                        .child(div().truncate().text_size(px(10.0)).text_color(th2.ink_faint).child(SharedString::from(ops::kit::instrument_label(&song, tr).to_string()))),
                )
                .child(act(
                    format!("lt-m-{t}"),
                    "M",
                    tr.muted,
                    th2.warn,
                    Rc::new(move |sh, cx| {
                        let id = t1.clone();
                        sh.edit(cx, |s| mixer::toggle_track_mute(s, &id));
                    }),
                ))
                .child(act(
                    format!("lt-s-{t}"),
                    "S",
                    tr.solo,
                    th2.accent2,
                    Rc::new(move |sh, cx| {
                        let id = t2.clone();
                        sh.edit(cx, |s| mixer::toggle_track_solo(s, &id));
                    }),
                ))
                .child(act(
                    format!("lt-st-{t}"),
                    "★",
                    is_main,
                    th2.accent,
                    Rc::new(move |sh, cx| {
                        let id = t3.clone();
                        sh.edit(cx, |s| ops::tracks::toggle_main_track(s, &id));
                    }),
                ));
            let mut card = div()
                .id(("ltrack", t))
                .mx(px(4.0))
                .px(px(6.0))
                .py(px(4.0))
                .flex()
                .flex_col()
                .gap(px(4.0))
                .rounded(px(ui::R1))
                .cursor_pointer()
                .when(on, |d| d.bg(th2.paper).border_1().border_color(th2.line_strong))
                .when(!on, |d| d.hover(|s| s.bg(th2.paper_hover)))
                .child(head)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    // leva o cursor da grade para esta linha (mesmo compasso/célula)
                    this.grid.update(cx, |g, cx| {
                        let c = g.cursor.unwrap_or(crate::views::grid::Cur { m: 0, t, k: 0 });
                        g.cursor = Some(crate::views::grid::Cur { t, ..c });
                        g.anchor = None;
                        cx.notify();
                    });
                    cx.notify();
                }));
            if on {
                // ── detalhes da linha do cursor (o que o inspector mostra) ──
                let def = ops::kit::resolve_instrument(&song, tr);
                let origin = if tr.color.is_some() {
                    "própria"
                } else if ops::structure::instrument_color(&song, &def.id).is_some() {
                    "do instrumento"
                } else {
                    "padrão"
                };
                let mut colors = row().child(lbl("cor"));
                let cur = tr.color.as_deref().map(str::to_ascii_lowercase);
                for (i, c) in TRACK_COLORS.iter().enumerate() {
                    let sel = cur.as_deref() == Some(&c.to_ascii_lowercase());
                    let id = tid.clone();
                    colors = colors.child(
                        div()
                            .id(SharedString::from(format!("lt-col-{t}-{i}")))
                            .size(px(12.0))
                            .rounded(px(2.0))
                            .bg(parse_color(c).unwrap_or(th2.ink_faint))
                            .border_1()
                            .border_color(if sel { th2.ink } else { gpui_kit::transparent_black() })
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                let id = id.clone();
                                this.edit(cx, |s| ops::tracks::set_track_color(s, &id, Some(c)));
                            })),
                    );
                }
                let id_inh = tid.clone();
                colors = colors.child(div().text_size(px(10.0)).text_color(th2.ink_faint).child(SharedString::from(origin))).when(tr.color.is_some(), |d| {
                    d.child(lchip(
                        format!("lt-inh-{t}"),
                        "herdar".into(),
                        false,
                        Rc::new(move |sh, cx| {
                            let id = id_inh.clone();
                            sh.edit(cx, |s| ops::tracks::set_track_color(s, &id, None));
                        }),
                    ))
                });
                let vol = tr.volume.unwrap_or(wadawko_core::Track::DEFAULT_VOLUME_DB);
                let pan = tr.pan.unwrap_or(0.0);
                let (st1, id1) = (state.clone(), tid.clone());
                let on_vol = Rc::new(move |v: f64, _: &mut Window, cx: &mut App| widgets::edit_co(&st1, cx, widgets::tag("trk-vol", &id1), |song| mixer::set_track_volume(song, &id1, v)));
                let (st2, id2) = (state.clone(), tid.clone());
                let on_pan = Rc::new(move |v: f64, _: &mut Window, cx: &mut App| widgets::edit_co(&st2, cx, widgets::tag("trk-pan", &id2), |song| mixer::set_track_pan(song, &id2, v)));
                let (id_d, id_k, id_f) = (tid.clone(), tid.clone(), tid.clone());
                let mut people = row().child(lbl("pessoas"));
                for p in song.people() {
                    let has = tr.person_ids().contains(&p.id);
                    let col = p.color.as_deref().and_then(parse_color).unwrap_or(th2.ink_soft);
                    let (pid, id) = (p.id.clone(), tid.clone());
                    people = people.child(
                        lchip(
                            format!("lt-p-{t}-{}", p.id),
                            p.name.clone(),
                            has,
                            Rc::new(move |sh, cx| {
                                let (pid, id) = (pid.clone(), id.clone());
                                sh.edit(cx, |s| ops::people::toggle_track_person(s, &id, &pid));
                            }),
                        )
                        .child(div().size(px(7.0)).rounded(px(ui::R1)).bg(col)),
                    );
                }
                let (id_up, id_dn) = (tid.clone(), tid.clone());
                card = card
                    .child(colors)
                    .child(
                        row()
                            .child(lbl("volume"))
                            .child(widgets::slider(
                                Slider {
                                    id: "ins-lt-vol".into(),
                                    value: vol,
                                    min: VOL_MIN,
                                    max: VOL_MAX,
                                    step: 1.0,
                                    default: Some(wadawko_core::Track::DEFAULT_VOLUME_DB),
                                    vertical: false,
                                    len: 100.0,
                                    color: th2.ink_soft,
                                    on_change: on_vol,
                                },
                                &th2,
                            ))
                            .child(div().text_size(px(10.0)).text_color(th2.ink_soft).child(SharedString::from(mixer::db_label(vol)))),
                    )
                    .child(
                        row()
                            .child(lbl("pan"))
                            .child(widgets::slider(
                                Slider { id: "ins-lt-pan".into(), value: pan, min: -1.0, max: 1.0, step: 0.1, default: Some(0.0), vertical: false, len: 100.0, color: th2.accent2, on_change: on_pan },
                                &th2,
                            ))
                            .child(div().text_size(px(10.0)).text_color(th2.ink_soft).child(SharedString::from(mixer::pan_label(pan)))),
                    )
                    .child(
                        row()
                            .child(lbl("mostrar"))
                            .child(lchip(
                                format!("lt-dyn-{t}"),
                                "f dinâmica".into(),
                                rows.dynr.get(t).copied().unwrap_or(false),
                                Rc::new(move |sh, cx| {
                                    let id = id_d.clone();
                                    sh.grid.update(cx, |g, cx| {
                                        if !g.show_dyn.remove(&id) {
                                            g.show_dyn.insert(id);
                                        }
                                        cx.notify();
                                    });
                                }),
                            ))
                            .child(lchip(
                                format!("lt-kyai-{t}"),
                                "気 kiai".into(),
                                rows.kyai.get(t).copied().unwrap_or(false),
                                Rc::new(move |sh, cx| {
                                    let id = id_k.clone();
                                    sh.grid.update(cx, |g, cx| {
                                        if !g.show_kyai.remove(&id) {
                                            g.show_kyai.insert(id);
                                        }
                                        cx.notify();
                                    });
                                }),
                            ))
                            .child(lchip(
                                format!("lt-fx-{t}"),
                                "fx efeitos".into(),
                                rows.fx.get(t).copied().unwrap_or(false),
                                Rc::new(move |sh, cx| {
                                    let id = id_f.clone();
                                    sh.grid.update(cx, |g, cx| {
                                        if !g.show_fx.remove(&id) {
                                            g.show_fx.insert(id);
                                        }
                                        cx.notify();
                                    });
                                }),
                            )),
                    )
                    .when(!song.people().is_empty(), |d| d.child(people))
                    .child(
                        row()
                            .child(lchip(
                                format!("lt-up-{t}"),
                                "↑".into(),
                                false,
                                Rc::new(move |sh, cx| {
                                    let id = id_up.clone();
                                    sh.edit(cx, |s| ops::tracks::move_track(s, &id, -1));
                                    sh.grid.update(cx, |g, _| {
                                        if let Some(c) = g.cursor.as_mut() {
                                            c.t = c.t.saturating_sub(1);
                                        }
                                    });
                                }),
                            ))
                            .child(lchip(
                                format!("lt-dn-{t}"),
                                "↓".into(),
                                false,
                                Rc::new(move |sh, cx| {
                                    let id = id_dn.clone();
                                    let n = sh.song(cx).tracks.len();
                                    sh.edit(cx, |s| ops::tracks::move_track(s, &id, 1));
                                    sh.grid.update(cx, |g, _| {
                                        if let Some(c) = g.cursor.as_mut() {
                                            c.t = (c.t + 1).min(n.saturating_sub(1));
                                        }
                                    });
                                }),
                            ))
                            .child(small_action("lt-ren", "✎ nome", &th2, Box::new(keys::track::Rename)))
                            .child(small_action("lt-set", "⚙ som", &th2, Box::new(keys::track::Settings)))
                            .child(small_action("lt-rm", "× remover", &th2, Box::new(keys::track::Remove))),
                    );
            }
            list = list.child(card);
        }
        list
    }
}

/// Botão pequeno que despacha uma ação (com tooltip e alvo de salto).
fn small_action(id: &'static str, label: &'static str, th: &Theme, a: Box<dyn gpui_kit::Action>) -> gpui_kit::Stateful<gpui_kit::Div> {
    let a2 = a.boxed_clone();
    keys::hintable(
        ui::with_tip(ui::btn(id, th), th, a.boxed_clone()).h(px(20.0)).px(px(5.0)).text_size(px(10.5)).child(label).on_click(move |_: &ClickEvent, w, cx| w.dispatch_action(a.boxed_clone(), cx)),
        id,
        Rc::new(move |w: &mut Window, cx: &mut App| w.dispatch_action(a2.boxed_clone(), cx)),
    )
}
