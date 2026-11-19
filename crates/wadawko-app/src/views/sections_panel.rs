//! Painel de SEÇÕES à esquerda do editor (`docs/planejamento/tracker.md` §3):
//! a estrutura da música como lista (com grupos recolhíveis), mais os TRECHOS
//! de ensaio. Clique seleciona a seção (região de playback) e leva o editor até
//! ela; Ctrl junta, Shift faz intervalo; duplo clique renomeia; arrastar
//! reordena (Alt clona); botão direito abre o menu; o quadradinho de cor abre
//! as cores. Copiar/colar seções vai como JSON pela área de transferência
//! (funciona entre músicas).
//!
//! É uma view em cache da moldura (`shell/chrome.rs`, `Part::Sections`): tudo
//! que ela mostra entra em [`Shell::sections_panel_key`].

use std::hash::Hash;

use gpui_kit::{App, ClickEvent, ClipboardItem, Context, FontWeight, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, SharedString, div, prelude::*, px};
use wadawko_core::model::SECTION_COLORS;
use wadawko_core::ops::{self, structure};

use crate::keys;
use crate::shell::Shell;
use crate::song_ext::SongExt;
use crate::theme::{Theme, parse_color};
use crate::ui::{self, panel_header};
use crate::views::grid::{Cur, InputTarget};
use crate::views::tracks_panel::LeftTab;

/// Largura do painel.
pub const SECTIONS_W: f32 = 220.0;

/// Arrasto de seção no painel.
#[derive(Clone, Debug, PartialEq)]
pub struct SecDrag {
    pub id: String,
    /// índice (na ordem tocada) sobre o qual está o ponteiro
    pub over: Option<usize>,
    /// Alt: clona em vez de mover
    pub clone: bool,
    pub moved: bool,
}

/// O que recebe a cor escolhida.
#[derive(Clone, Debug, PartialEq)]
pub enum ColorTarget {
    Section(String),
    Group(String),
}

/// Menus flutuantes do painel (coordenadas de janela).
#[derive(Clone, Debug, PartialEq)]
pub enum SecMenu {
    Context { id: String, x: f32, y: f32 },
    Color { target: ColorTarget, x: f32, y: f32 },
}

impl Shell {
    /// Seções alvo das ações: as selecionadas (região), senão a do cursor.
    pub(crate) fn target_sections(&self, cx: &App) -> Vec<String> {
        let st = self.state.read(cx);
        if !st.tp.selected.is_empty() {
            return st.tp.selected.clone();
        }
        let song = st.song();
        let g = self.grid.read(cx);
        g.cursor.and_then(|c| song.secs().get(song.section_of(c.m)).and_then(|s| s.id.clone())).into_iter().collect()
    }

    /// Chave de cache do painel: tudo o que ele desenha.
    pub(crate) fn sections_panel_key(&self, k: &mut std::collections::hash_map::DefaultHasher, cx: &App) {
        let st = self.state.read(cx);
        let song = st.song();
        let cur_sec = self.grid.read(cx).cursor.map(|c| song.section_of(c.m));
        let playing_sec = st.playing().then(|| st.playhead().map(|p| song.section_of(p.mi))).flatten();
        (st.revision, st.theme.dark, &st.tp.selected, format!("{:?}", st.tp.range), cur_sec, playing_sec).hash(k);
        (format!("{:?}", self.sec_drag), format!("{:?}", self.sec_menu), self.left_tab).hash(k);
        if self.left_tab == LeftTab::Tracks {
            self.tracks_panel_key(k, cx);
        }
    }

    /// Clique numa seção do painel.
    fn sec_click(&mut self, id: String, ctrl: bool, shift: bool, double: bool, cx: &mut Context<Self>) {
        let song = self.song(cx);
        if double {
            if let Some(sec) = song.section(&id) {
                let name = sec.name.clone();
                self.ask_later(InputTarget::SectionName(id), name, cx);
            }
            return;
        }
        let order: Vec<String> = ops::sections::section_ranges(&song).into_iter().map(|r| r.id).collect();
        if shift && let Some(anchor) = self.sec_anchor.clone() {
            let (a, b) = (order.iter().position(|x| *x == anchor), order.iter().position(|x| *x == id));
            if let (Some(a), Some(b)) = (a, b) {
                let (lo, hi) = (a.min(b), a.max(b));
                let ids = order[lo..=hi].to_vec();
                self.state.update(cx, |s, cx| {
                    s.tp.selected = ids;
                    s.sync_audio();
                    cx.notify();
                });
            }
        } else {
            self.state.update(cx, |s, cx| s.select_section(&id, ctrl, cx));
            self.sec_anchor = Some(id.clone());
        }
        // leva o editor até a seção (o cursor vai para o 1º compasso dela)
        if let Some(r) = ops::sections::section_ranges(&song).into_iter().find(|r| r.id == id) {
            let t = self.grid.read(cx).cursor.map_or(0, |c| c.t);
            self.grid.update(cx, |g, cx| {
                g.cursor = Some(Cur { m: r.start.min(song.measures.len().saturating_sub(1)), t, k: 0 });
                g.anchor = None;
                cx.notify();
            });
            self.reveal_cursor(cx);
        }
        cx.notify();
    }

    /// Termina o arrasto (soltou sobre a seção `to`).
    fn sec_drop(&mut self, to: usize, cx: &mut Context<Self>) {
        let Some(d) = self.sec_drag.take() else { return };
        if !d.moved {
            cx.notify();
            return;
        }
        let song = self.song(cx);
        let order: Vec<String> = ops::sections::section_ranges(&song).into_iter().map(|r| r.id).collect();
        let Some(from) = order.iter().position(|x| *x == d.id) else { return };
        if to == from && !d.clone {
            cx.notify();
            return;
        }
        let id = d.id.clone();
        // soltar sobre uma seção = ocupar o lugar dela E entrar no grupo dela (ou sair, se ela não tem)
        let target_group = order.get(to).and_then(|t| song.section(t)).and_then(|t| t.group.clone());
        self.edit(cx, move |s| {
            let src = if d.clone { ops::sections::clone_section(s, &id) } else { Some(id.clone()) };
            if let Some(src) = src {
                let to = if d.clone && to > from { to + 1 } else { to };
                ops::sections::move_section(s, &src, to);
                if let Some(sec) = s.sections.as_mut().and_then(|v| v.iter_mut().find(|x| x.id == src)) {
                    sec.group = target_group.clone();
                }
                structure::prune_groups(s);
            }
        });
        self.flash(if d.clone { "seção clonada" } else { "seção movida" }, cx);
    }

    /// Copia as seções alvo (JSON na área de transferência).
    pub(crate) fn sections_copy(&mut self, cut: bool, cx: &mut Context<Self>) {
        let ids = self.target_sections(cx);
        let song = self.song(cx);
        match structure::copy_sections(&song, &ids) {
            Some(json) => {
                cx.write_to_clipboard(ClipboardItem::new_string(json));
                if cut {
                    self.edit(cx, |s| ids.iter().for_each(|id| ops::sections::remove_section(s, id)));
                    self.state.update(cx, |s, _| s.tp.selected.clear());
                }
                self.flash(format!("{} seç{} {}", ids.len(), if ids.len() == 1 { "ão" } else { "ões" }, if cut { "recortada(s)" } else { "copiada(s)" }), cx);
            }
            None => self.flash("nenhuma seção para copiar", cx),
        }
    }

    /// Cola seções da área de transferência depois da seção alvo (ou no fim).
    pub(crate) fn sections_paste(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else {
            self.flash("área de transferência vazia", cx);
            return;
        };
        if !structure::is_clip(&text) {
            self.flash("a área de transferência não tem seções copiadas", cx);
            return;
        }
        let after = self.target_sections(cx).last().cloned();
        let mut res = Ok(Vec::new());
        self.edit(cx, |s| res = structure::paste_sections(s, &text, after.as_deref()));
        match res {
            Ok(ids) => {
                let n = ids.len();
                self.state.update(cx, |s, cx| {
                    s.tp.selected = ids;
                    s.sync_audio();
                    cx.notify();
                });
                self.flash(format!("{n} seç{} colada(s)", if n == 1 { "ão" } else { "ões" }), cx);
            }
            Err(e) => self.flash(e, cx),
        }
    }

    pub(crate) fn sections_group(&mut self, cx: &mut Context<Self>) {
        let ids = self.target_sections(cx);
        if ids.is_empty() {
            self.flash("selecione seções no painel (Ctrl+clique) para agrupar", cx);
            return;
        }
        let n = self.song(cx).section_groups.as_ref().map_or(0, Vec::len) + 1;
        let mut gid = None;
        self.edit(cx, |s| gid = structure::group_sections(s, &ids, &format!("Grupo {n}")));
        if gid.is_some() {
            self.flash(format!("{} seç{} agrupada(s)", ids.len(), if ids.len() == 1 { "ão" } else { "ões" }), cx);
        }
    }

    pub(crate) fn sections_ungroup(&mut self, cx: &mut Context<Self>) {
        let song = self.song(cx);
        let groups: Vec<String> = self.target_sections(cx).iter().filter_map(|id| song.section(id).and_then(|s| s.group.clone())).collect();
        if groups.is_empty() {
            self.flash("a seção não está num grupo", cx);
            return;
        }
        self.edit(cx, |s| groups.iter().for_each(|g| structure::ungroup(s, g)));
        self.flash("grupo desfeito", cx);
    }

    /// Aplica uma cor escolhida no menu.
    pub(crate) fn apply_color(&mut self, target: ColorTarget, color: Option<String>, cx: &mut Context<Self>) {
        let c = color.clone();
        self.edit(cx, move |s| match &target {
            ColorTarget::Section(id) => structure::set_section_color(s, id, c.as_deref()),
            ColorTarget::Group(id) => structure::set_group_color(s, id, c.as_deref()),
        });
        self.sec_menu = None;
        cx.notify();
    }

    /// O painel inteiro (SEÇÕES + TRECHOS).
    pub(crate) fn sections_panel(&self, th: &Theme, cx: &mut Context<Self>) -> gpui_kit::Div {
        let st = self.state.read(cx);
        let song = st.song().clone();
        let selected = st.tp.selected.clone();
        let playing_sec = st.playing().then(|| st.playhead().map(|p| song.section_of(p.mi))).flatten();
        let cur_sec = self.grid.read(cx).cursor.map(|c| song.section_of(c.m));
        let ranges = ops::sections::section_ranges(&song);
        let secs = song.secs();
        let th2 = *th;
        let drag = self.sec_drag.clone();
        let mut list = div().flex().flex_col().py(px(4.0));
        let mut last_group: Option<String> = None;
        for (i, r) in ranges.iter().enumerate() {
            let Some(sec) = song.section(&r.id) else { continue };
            let gid = sec.group.clone();
            // cabeçalho do grupo na 1ª seção dele
            if gid.is_some() && gid != last_group {
                let g = gid.as_deref().and_then(|g| structure::group(&song, g)).cloned();
                if let Some(g) = g {
                    let ids = structure::group_sections_ids(&song, &g.id);
                    let total: usize = ranges.iter().filter(|x| ids.contains(&x.id)).map(|x| x.len()).sum();
                    let all_sel = !ids.is_empty() && ids.iter().all(|x| selected.contains(x));
                    let col = g.color.as_deref().and_then(parse_color);
                    let (g1, g2, g3, g4) = (g.id.clone(), g.id.clone(), g.id.clone(), g.name.clone());
                    let collapsed = g.collapsed;
                    list = list.child(
                        div()
                            .id(SharedString::from(format!("grp-{}", g.id)))
                            .h(px(22.0))
                            .px(px(6.0))
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .cursor_pointer()
                            .when(all_sel, |d| d.bg(th2.accent2_wash))
                            .hover(|s| s.bg(th2.paper_hover))
                            .child(
                                div().id(SharedString::from(format!("grp-tog-{}", g.id))).w(px(14.0)).text_size(px(10.0)).text_color(th2.ink_soft).child(if collapsed { "▸" } else { "▾" }).on_click(
                                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        cx.stop_propagation();
                                        let id = g1.clone();
                                        this.edit(cx, |s| {
                                            let c = structure::group(s, &id).is_some_and(|g| g.collapsed);
                                            structure::set_group_collapsed(s, &id, !c);
                                        });
                                    }),
                                ),
                            )
                            .child(swatch(SharedString::from(format!("grp-sw-{}", g.id)), col, &th2).on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    this.sec_menu = Some(SecMenu::Color { target: ColorTarget::Group(g2.clone()), x: f32::from(ev.position.x), y: f32::from(ev.position.y) });
                                    cx.notify();
                                }),
                            ))
                            .child(div().flex_1().min_w_0().truncate().text_size(px(10.0)).font_weight(FontWeight::BOLD).text_color(th2.ink_soft).child(SharedString::from(g.name.to_uppercase())))
                            .child(div().text_size(px(10.0)).text_color(th2.ink_faint).child(SharedString::from(format!("{total}c"))))
                            .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                                if ev.click_count() >= 2 {
                                    this.ask_later(InputTarget::GroupName(g3.clone()), g4.clone(), cx);
                                    return;
                                }
                                // selecionar o grupo = tocar o grupo
                                let ids = structure::group_sections_ids(&this.song(cx), &g3);
                                this.state.update(cx, |s, cx| {
                                    s.tp.selected = ids;
                                    s.sync_audio();
                                    cx.notify();
                                });
                                cx.notify();
                            })),
                    );
                    last_group = gid.clone();
                    if collapsed {
                        continue;
                    }
                } else {
                    last_group = None;
                }
            } else if gid.is_none() {
                last_group = None;
            } else if gid.as_deref().and_then(|g| structure::group(&song, g)).is_some_and(|g| g.collapsed) {
                continue;
            }
            let in_group = gid.is_some();
            let on = selected.contains(&r.id);
            let is_cur = cur_sec.is_some_and(|c| secs.get(c).and_then(|s| s.id.as_deref()) == Some(r.id.as_str()));
            let playing = playing_sec.is_some_and(|c| secs.get(c).and_then(|s| s.id.as_deref()) == Some(r.id.as_str()));
            let col = structure::section_color(&song, sec).and_then(parse_color);
            let has_spot = ops::practice::spots(&song).iter().any(|p| p.start.section == r.id);
            let drop_here = drag.as_ref().is_some_and(|d| d.moved && d.over == Some(i) && d.id != r.id);
            let dragging_this = drag.as_ref().is_some_and(|d| d.moved && d.id == r.id);
            let (id1, id2, id3, id4) = (r.id.clone(), r.id.clone(), r.id.clone(), r.id.clone());
            let row = div()
                .id(SharedString::from(format!("secrow-{}", r.id)))
                .relative()
                .h(px(24.0))
                .pl(px(if in_group { 20.0 } else { 6.0 }))
                .pr(px(6.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .cursor_pointer()
                .when(on, |d| d.bg(th2.accent2_wash))
                .when(!on, |d| d.hover(|s| s.bg(th2.paper_hover)))
                .when(is_cur, |d| d.child(div().absolute().left_0().top_0().bottom_0().w(px(2.0)).bg(th2.accent)))
                .when(drop_here, |d| d.border_t_2().border_color(th2.accent))
                .when(dragging_this, |d| d.opacity(0.5))
                .child(div().w(px(10.0)).flex_none().text_size(px(9.0)).text_color(th2.accent).child(if playing { "▶" } else { "" }))
                .child(swatch(SharedString::from(format!("sec-sw-{}", r.id)), col, &th2).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.sec_menu = Some(SecMenu::Color { target: ColorTarget::Section(id1.clone()), x: f32::from(ev.position.x), y: f32::from(ev.position.y) });
                        cx.notify();
                    }),
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.0))
                        .font_weight(if is_cur || on { FontWeight::BOLD } else { FontWeight::MEDIUM })
                        .text_color(if is_cur || on { th2.ink } else { th2.ink_soft })
                        .child(SharedString::from(r.name.clone())),
                )
                .when(r.repeat.is_some_and(|x| x > 1), |d| {
                    d.child(div().text_size(px(10.5)).font_weight(FontWeight::BOLD).text_color(th2.accent).child(SharedString::from(format!("{}×", r.repeat.unwrap_or(1)))))
                })
                .child(div().text_size(px(10.0)).text_color(th2.ink_faint).child(SharedString::from(format!("{}c", r.len()))))
                .when(has_spot, |d| d.child(div().text_size(px(10.0)).text_color(th2.accent2).child("⟲")))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, ev: &MouseDownEvent, _, _cx| {
                        this.sec_drag = Some(SecDrag { id: id2.clone(), over: None, clone: ev.modifiers.alt, moved: false });
                    }),
                )
                .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
                    if let Some(d) = this.sec_drag.as_mut()
                        && ev.pressed_button == Some(MouseButton::Left)
                        && d.over != Some(i)
                    {
                        d.over = Some(i);
                        d.moved = d.moved || d.id != id4;
                        cx.notify();
                    }
                }))
                .on_mouse_up(MouseButton::Left, cx.listener(move |this, _: &MouseUpEvent, _, cx| this.sec_drop(i, cx)))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                        this.sec_menu = Some(SecMenu::Context { id: id3.clone(), x: f32::from(ev.position.x), y: f32::from(ev.position.y) });
                        cx.notify();
                    }),
                )
                .on_click(cx.listener({
                    let id = r.id.clone();
                    move |this, ev: &ClickEvent, _, cx| {
                        if this.sec_drag.as_ref().is_some_and(|d| d.moved) {
                            return;
                        }
                        let m = ev.modifiers();
                        this.sec_click(id.clone(), m.control || m.platform, m.shift, ev.click_count() >= 2, cx);
                    }
                }));
            let hid = format!("sec-{i}");
            let id5 = r.id.clone();
            let w = cx.entity().downgrade();
            list = list.child(keys::hintable(
                row,
                hid,
                std::rc::Rc::new(move |_, cx: &mut App| {
                    let _ = w.update(cx, |this, cx| this.sec_click(id5.clone(), false, false, false, cx));
                }),
            ));
        }
        if ranges.is_empty() {
            list = list.child(div().px(px(10.0)).py(px(6.0)).text_size(px(10.5)).text_color(th2.ink_faint).child("A música ainda não tem seções: + cria a primeira com todos os compassos."));
        }
        // botões do cabeçalho: ícone (nome em `assets/icons`) ou texto
        let small = |id: &'static str, label: &'static str, a: Box<dyn gpui_kit::Action>| {
            let b = ui::with_tip(ui::btn(id, &th2), &th2, a.boxed_clone()).h(px(20.0)).px(px(5.0)).text_size(px(10.0)).on_click(move |_: &ClickEvent, w, cx| w.dispatch_action(a.boxed_clone(), cx));
            if crate::icons::ICONS.iter().any(|(n, _)| *n == label) { b.child(ui::icon(label, 12.0).text_color(th2.ink_soft)) } else { b.child(label) }
        };
        div()
            .w(px(SECTIONS_W))
            .flex_none()
            .h_full()
            .bg(th2.chrome)
            .border_r_1()
            .border_color(th2.line)
            .flex()
            .flex_col()
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    if this.sec_drag.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .child({
                // abas: SEÇÕES | LINHAS (os botões do cabeçalho mudam com a aba)
                let tab = |id: &'static str, label: &'static str, which: LeftTab, cx: &mut Context<Self>| {
                    let on = self.left_tab == which;
                    div()
                        .id(id)
                        .h(px(22.0))
                        .px(px(6.0))
                        .flex()
                        .items_center()
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(if on { th2.ink } else { th2.ink_faint })
                        .border_b_2()
                        .border_color(if on { th2.accent } else { gpui_kit::transparent_black() })
                        .cursor_pointer()
                        .child(label)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.left_tab = which;
                            cx.notify();
                        }))
                };
                let buttons = match self.left_tab {
                    LeftTab::Sections => div()
                        .flex()
                        .gap(px(3.0))
                        .child(small("secp-grp", "group", Box::new(keys::section::Group)))
                        .child(small("secp-copy", "copy", Box::new(keys::section::Copy)))
                        .child(small("secp-paste", "paste", Box::new(keys::section::Paste)))
                        .child(small("secp-add", "plus", Box::new(keys::section::Add))),
                    LeftTab::Tracks => div().flex().gap(px(3.0)).child(small("trkp-add", "+ linha", Box::new(keys::track::Add))),
                };
                div()
                    .h(px(26.0))
                    .flex_none()
                    .px(px(4.0))
                    .flex()
                    .items_end()
                    .justify_between()
                    .border_b_1()
                    .border_color(th2.line)
                    .child(div().flex().child(tab("ltab-sec", "SEÇÕES", LeftTab::Sections, cx)).child(tab("ltab-trk", "LINHAS", LeftTab::Tracks, cx)))
                    .child(div().pb(px(3.0)).child(buttons))
            })
            .when(self.left_tab == LeftTab::Sections, |d| {
                d.child(div().id("sec-list").flex_1().min_h_0().overflow_y_scroll().child(list))
                    .child(
                        panel_header("Trechos", &th2)
                            .child(ui::abtn("tr-save", &th2, keys::practice::Save).h(px(18.0)).px(px(6.0)).text_size(px(10.0)).font_weight(FontWeight::BOLD).child("+ guardar")),
                    )
                    .child(div().id("practice").flex_none().max_h(px(180.0)).overflow_y_scroll().child(self.practice_list(&th2, cx)))
            })
            .when(self.left_tab == LeftTab::Tracks, |d| d.child(div().id("trk-list").flex_1().min_h_0().overflow_y_scroll().child(self.tracks_list(&th2, cx))))
    }

    /// Menus flutuantes (contexto da seção, cores) — por cima de tudo.
    pub(crate) fn sec_menu_overlay(&self, th: &Theme, cx: &mut Context<Self>) -> Option<gpui_kit::AnyElement> {
        let menu = self.sec_menu.clone()?;
        let th2 = *th;
        let item = |id: SharedString, label: String, cx: &mut Context<Self>, f: MenuAct| {
            div()
                .id(id)
                .h(px(24.0))
                .px(px(10.0))
                .flex()
                .items_center()
                .text_size(px(12.0))
                .text_color(th2.ink)
                .cursor_pointer()
                .hover(|s| s.bg(th2.paper_hover))
                .child(SharedString::from(label))
                .on_click(cx.listener(move |this, _: &ClickEvent, w, cx| {
                    this.sec_menu = None;
                    f(this, w, cx);
                    cx.notify();
                }))
        };
        let (x, y, body) = match menu {
            SecMenu::Context { id, x, y } => {
                let song = self.song(cx);
                let in_group = song.section(&id).and_then(|s| s.group.clone());
                let times = song.section(&id).map_or(1, |s| s.times());
                let name = song.section(&id).map(|s| s.name.clone()).unwrap_or_default();
                // as ações agem na seleção; o menu garante que a seção clicada está nela
                let ensure = {
                    let id = id.clone();
                    move |this: &mut Shell, cx: &mut Context<Shell>| {
                        if !this.state.read(cx).tp.selected.contains(&id) {
                            let id = id.clone();
                            this.state.update(cx, |s, cx| s.select_section(&id, false, cx));
                        }
                    }
                };
                let (e1, e2, e3, e4, e5) = (ensure.clone(), ensure.clone(), ensure.clone(), ensure.clone(), ensure.clone());
                let (i1, i2, i3, i4, i5, i6) = (id.clone(), id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
                let body = div()
                    .flex()
                    .flex_col()
                    .py(px(4.0))
                    .child(item("m-ren".into(), "renomear".into(), cx, Box::new(move |this, _, cx| this.ask_later(InputTarget::SectionName(i1.clone()), name.clone(), cx))))
                    .child(item(
                        "m-col".into(),
                        "cor…".into(),
                        cx,
                        Box::new(move |this, _, cx| {
                            this.sec_menu = Some(SecMenu::Color { target: ColorTarget::Section(i2.clone()), x, y });
                            cx.notify();
                        }),
                    ))
                    .child(item(
                        "m-rep+".into(),
                        format!("ritornello + (agora {times}×)"),
                        cx,
                        Box::new(move |this, _, cx| {
                            this.edit(cx, |s| ops::sections::set_section_repeat(s, &i3, (times + 1).min(16)));
                        }),
                    ))
                    .when(times > 1, |d| {
                        d.child(item(
                            "m-rep-".into(),
                            "ritornello −".into(),
                            cx,
                            Box::new(move |this, _, cx| {
                                this.edit(cx, |s| ops::sections::set_section_repeat(s, &i4, times - 1));
                            }),
                        ))
                    })
                    .child(item(
                        "m-dup".into(),
                        "duplicar".into(),
                        cx,
                        Box::new(move |this, _, cx| {
                            this.edit(cx, |s| {
                                ops::sections::clone_section(s, &i5);
                            });
                        }),
                    ))
                    .child(item(
                        "m-copy".into(),
                        "copiar".into(),
                        cx,
                        Box::new(move |this, _, cx| {
                            e1(this, cx);
                            this.sections_copy(false, cx)
                        }),
                    ))
                    .child(item(
                        "m-paste".into(),
                        "colar depois".into(),
                        cx,
                        Box::new(move |this, _, cx| {
                            e2(this, cx);
                            this.sections_paste(cx)
                        }),
                    ))
                    .child(item(
                        "m-grp".into(),
                        "agrupar as selecionadas".into(),
                        cx,
                        Box::new(move |this, _, cx| {
                            e3(this, cx);
                            this.sections_group(cx)
                        }),
                    ))
                    .when(in_group.is_some(), |d| {
                        d.child(item(
                            "m-ungrp".into(),
                            "desfazer o grupo".into(),
                            cx,
                            Box::new(move |this, _, cx| {
                                e4(this, cx);
                                this.sections_ungroup(cx)
                            }),
                        ))
                    })
                    .child(item(
                        "m-solo".into(),
                        "tocar só esta".into(),
                        cx,
                        Box::new(move |this, _, cx| {
                            let id = i6.clone();
                            this.state.update(cx, |s, cx| {
                                s.tp.selected = vec![id];
                                s.sync_audio();
                                cx.notify();
                            });
                        }),
                    ))
                    .child(item(
                        "m-del".into(),
                        "remover".into(),
                        cx,
                        Box::new(move |this, _, cx| {
                            e5(this, cx);
                            let ids = this.target_sections(cx);
                            this.edit(cx, |s| ids.iter().for_each(|id| ops::sections::remove_section(s, id)));
                            this.state.update(cx, |s, _| s.tp.selected.clear());
                        }),
                    ));
                (x, y, body)
            }
            SecMenu::Color { target, x, y } => {
                let palette: Vec<&'static str> = SECTION_COLORS.to_vec();
                let mut sw = div().flex().flex_wrap().gap(px(6.0)).p(px(8.0)).w(px(196.0));
                for (i, c) in palette.into_iter().enumerate() {
                    let t = target.clone();
                    let col = parse_color(c).unwrap_or(th2.ink_faint);
                    sw = sw.child(
                        div()
                            .id(("pick", i))
                            .size(px(22.0))
                            .rounded(px(ui::R1))
                            .bg(col)
                            .cursor_pointer()
                            .border_1()
                            .border_color(th2.line_strong)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.apply_color(t.clone(), Some(c.to_string()), cx))),
                    );
                }
                let (t1, t2) = (target.clone(), target.clone());
                let body = div()
                    .flex()
                    .flex_col()
                    .child(div().px(px(10.0)).pt(px(8.0)).text_size(px(10.0)).font_weight(FontWeight::BOLD).text_color(th2.ink_faint).child("COR"))
                    .child(sw)
                    .child(item("pick-none".into(), "sem cor / padrão".into(), cx, Box::new(move |this, _, cx| this.apply_color(t1.clone(), None, cx))))
                    .child(item("pick-hex".into(), "outra (#rrggbb)…".into(), cx, Box::new(move |this, _, cx| this.ask_later(InputTarget::Color(t2.clone()), String::new(), cx))));
                (x, y, body)
            }
        };
        Some(
            div()
                .absolute()
                .inset_0()
                // fundo transparente: clicar fora fecha
                .child(div().id("secmenu-bg").absolute().inset_0().on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseDownEvent, _, cx| {
                        this.sec_menu = None;
                        cx.notify();
                    }),
                ))
                .child(
                    div()
                        .id("secmenu")
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .min_w(px(180.0))
                        .bg(th2.paper_raise)
                        .border_1()
                        .border_color(th2.line_strong)
                        .rounded(px(ui::R1))
                        .shadow_lg()
                        .occlude()
                        .child(body),
                )
                .into_any_element(),
        )
    }
}

/// Ação de um item de menu.
type MenuAct = Box<dyn Fn(&mut Shell, &mut gpui_kit::Window, &mut Context<Shell>)>;

/// Quadradinho de cor 10×10 (vazio = só o contorno).
fn swatch(id: SharedString, col: Option<gpui_kit::Hsla>, th: &Theme) -> gpui_kit::Stateful<gpui_kit::Div> {
    let d = div().id(id).size(px(10.0)).flex_none().rounded(px(2.0)).cursor_pointer();
    match col {
        Some(c) => d.bg(c),
        None => d.border_1().border_color(th.line_strong),
    }
}
