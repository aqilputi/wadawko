//! Pessoas (porte de `src/lib/PeoplePanel.svelte`): um cartão por pessoa com
//! nome (clique renomeia pelo prompt do Shell), cor (paleta `PERSON_COLORS`),
//! caneta, barramento (volume/mudo/solo — os mesmos do Mixer) e a checklist de
//! instrumentos (`toggle_track_person`; marcado por 2+ pessoas = compartilhado).
//!
//! Diferenças do web: a cor vem da paleta fixa (não há seletor de cor livre) e
//! excluir não pede confirmação — Ctrl+Z desfaz.

use std::rc::Rc;

use gpui_kit::{App, Context, Entity, FontWeight, Render, SharedString, Window, div, prelude::*, px};
use wadawko_core::model::{PERSON_COLORS, Person, Song};
use wadawko_core::ops;

use crate::keys;
use crate::shell::Shell;
use crate::state::AppState;
use crate::theme::{Theme, parse_color};
use crate::ui;
use crate::views::grid::InputTarget;
use crate::views::mixer;
use crate::views::widgets;

gpui_kit::actions!(people, [Add, Rename]);

pub struct PeopleView {
    pub state: Entity<AppState>,
    /// pessoa que a ação `people::Rename` vai renomear (posta pelo clique no nome)
    rename: Option<String>,
}

/// Quantas pessoas tocam a linha (≥ 2 = compartilhada).
#[cfg(test)]
pub fn owner_count(song: &Song, track_id: &str) -> usize {
    song.track(track_id).map_or(0, |t| t.person_ids().len())
}

/// Alvo do renomear: a pessoa pedida, se ainda existe; senão a primeira.
pub fn rename_target(song: &Song, wanted: Option<&str>) -> Option<(String, String)> {
    let p = wanted.and_then(|id| song.person(id)).or_else(|| song.people().first())?;
    Some((p.id.clone(), p.name.clone()))
}

impl PeopleView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        // re-renderiza quando a música/tema/transporte mudam
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        PeopleView { state, rename: None }
    }

    /// Cria uma pessoa; como no web, se não há caneta ativa a nova pessoa a assume.
    pub fn add(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |s, cx| {
            let mut new = None;
            s.edit(cx, |song| new = Some(ops::people::add_person(song, None)));
            if s.pen.is_none() && new.is_some() {
                s.pen = new;
                cx.notify();
            }
        });
    }

    pub fn rename_target(&self, cx: &App) -> Option<(String, String)> {
        rename_target(self.state.read(cx).song(), self.rename.as_deref())
    }

    fn card(&self, song: &Song, p: &Person, pen: bool, th: &Theme, cx: &mut Context<Self>) -> gpui_kit::Div {
        let color = mixer::person_color(p, th);
        let state = &self.state;

        // ── cabeçalho: cor atual, nome (renomeia), caneta, excluir ──
        let view = cx.entity();
        let pid = p.id.clone();
        let on_rename: widgets::Act = Rc::new(move |w: &mut Window, cx: &mut App| {
            view.update(cx, |v, _| v.rename = Some(pid.clone()));
            w.dispatch_action(Box::new(Rename), cx);
        });
        let name_id: SharedString = format!("pp-name-{}", p.id).into();
        let name = keys::hintable(
            div()
                .id(name_id.clone())
                .relative()
                .flex_1()
                .min_w_0()
                .px(px(4.0))
                .rounded(px(5.0))
                .text_size(px(14.5))
                .font_weight(FontWeight::BOLD)
                .text_color(th.ink)
                .cursor_pointer()
                .hover(|s| s.bg(th.paper_hover))
                .child(SharedString::from(p.name.clone()))
                .on_click({
                    let a = on_rename.clone();
                    move |_, w, cx| a(w, cx)
                }),
            name_id,
            on_rename,
        );
        let (st, pid) = (state.clone(), p.id.clone());
        let on_pen: widgets::Act = Rc::new(move |_: &mut Window, cx: &mut App| {
            st.update(cx, |s, cx| {
                s.pen = Some(pid.clone());
                cx.notify();
            })
        });
        let (st, pid, pname) = (state.clone(), p.id.clone(), p.name.clone());
        let on_del: widgets::Act = Rc::new(move |_: &mut Window, cx: &mut App| {
            st.update(cx, |s, cx| {
                if s.edit(cx, |song| ops::people::delete_person(song, &pid)) {
                    s.flash(format!("“{pname}” removida — instrumentos e notas ficam; só o agrupamento sai (Ctrl+Z desfaz)"));
                }
            })
        });
        let head = div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(div().flex_none().w(px(20.0)).h(px(20.0)).rounded(px(5.0)).bg(color).border_1().border_color(th.line_strong))
            .child(name)
            .child(widgets::chip(format!("pp-pen-{}", p.id), if pen { "✎ caneta" } else { "caneta" }, th, pen, on_pen))
            .child(widgets::sbtn(format!("pp-del-{}", p.id), th, on_del).child("×"));

        // ── paleta de cores ──
        let mut swatches = div().flex().flex_wrap().items_center().gap(px(5.0));
        for (i, hex) in PERSON_COLORS.iter().enumerate() {
            let Some(c) = parse_color(hex) else { continue };
            let on = p.color.as_deref().is_some_and(|pc| pc.eq_ignore_ascii_case(hex));
            let (st, pid) = (state.clone(), p.id.clone());
            let act: widgets::Act = Rc::new(move |_: &mut Window, cx: &mut App| widgets::edit(&st, cx, |song| ops::people::set_person_color(song, &pid, hex)));
            let id: SharedString = format!("pp-color-{}-{i}", p.id).into();
            swatches = swatches.child(keys::hintable(
                div()
                    .id(id.clone())
                    .relative()
                    .flex_none()
                    .w(px(20.0))
                    .h(px(20.0))
                    .rounded(px(10.0))
                    .bg(c)
                    .border_2()
                    .border_color(if on { th.ink } else { th.paper })
                    .cursor_pointer()
                    .hover(|s| s.border_color(th.line_strong))
                    .on_click({
                        let a = act.clone();
                        move |_, w, cx| a(w, cx)
                    }),
                id,
                act,
            ));
        }

        // ── checklist de instrumentos ──
        let mut list = div().flex().flex_col().gap(px(2.0));
        for t in &song.tracks {
            let on = t.person_ids().contains(&p.id);
            let shared = on && t.is_shared();
            let (st, pid, tid) = (state.clone(), p.id.clone(), t.id.clone());
            let act: widgets::Act = Rc::new(move |_: &mut Window, cx: &mut App| widgets::edit(&st, cx, |song| ops::people::toggle_track_person(song, &tid, &pid)));
            let id: SharedString = format!("pp-chk-{}-{}", p.id, t.id).into();
            let check = div()
                .flex_none()
                .w(px(15.0))
                .h(px(15.0))
                .rounded(px(3.0))
                .border_1()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.0))
                .font_weight(FontWeight::BOLD)
                .text_color(th.on_accent)
                .when(on, |d| d.bg(color).border_color(color).child("✓"))
                .when(!on, |d| d.bg(th.paper).border_color(th.line_strong));
            list = list.child(keys::hintable(
                div()
                    .id(id.clone())
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(7.0))
                    .px(px(6.0))
                    .py(px(5.0))
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .when(on, |d| d.bg(Theme::alpha(color, 0.12)))
                    .when(!on, |d| d.hover(|s| s.bg(th.paper_raise)))
                    .child(check)
                    .child(div().text_size(px(13.0)).font_weight(FontWeight::BOLD).text_color(th.ink).child(SharedString::from(t.name.clone())))
                    .child(div().flex_1().min_w_0().text_size(px(11.0)).text_color(th.ink_soft).child(SharedString::from(ops::kit::instrument_label(song, t).to_string())))
                    .when(shared, |d| {
                        d.child(div().flex_none().px(px(6.0)).rounded(px(8.0)).border_1().border_color(color).text_size(px(9.5)).font_weight(FontWeight::BOLD).text_color(color).child("COMPART."))
                    })
                    .on_click({
                        let a = act.clone();
                        move |_, w, cx| a(w, cx)
                    }),
                id,
                act,
            ));
        }

        mixer::group_card(th, color, div().child(head).child(swatches).child(mixer::bus_controls("pp-", state, p, th, 150.0)).child(list)).w(px(330.0))
    }
}

impl Render for PeopleView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (th, song, pen) = {
            let st = self.state.read(cx);
            (st.theme, st.song().clone(), st.pen.clone())
        };
        let note = |text: &'static str| div().text_size(px(12.5)).text_color(th.ink_soft).child(text);
        let head = div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(16.0))
            .child(
                div().flex().flex_col().gap(px(4.0)).child(widgets::page_title("Pessoas")).child(
                    note("Uma pessoa toca um ou mais instrumentos. Marque os instrumentos de cada uma; um instrumento marcado por 2 pessoas fica compartilhado. Clique no nome para renomear.").max_w(px(620.0)),
                ),
            )
            .child(ui::abtn_bare("pp-add", &th, Add).bg(th.accent).border_color(th.accent).text_color(th.on_accent).font_weight(FontWeight::BOLD).rounded(px(16.0)).hover(|s| s.bg(th.accent)).child("+ pessoa"));

        let mut page = div().w_full().flex().flex_col().gap(px(14.0)).child(head);
        if song.people().is_empty() {
            page = page.child(note("Nenhuma pessoa ainda. Crie uma e marque os instrumentos dela. (´-ω-`)"));
        } else if song.tracks.is_empty() {
            page = page.child(note("Sem instrumentos. Adicione linhas no Editor primeiro."));
        }
        let mut cards = div().flex().flex_wrap().items_start().gap(px(14.0));
        for p in song.people() {
            let is_pen = pen.as_deref() == Some(p.id.as_str());
            cards = cards.child(self.card(&song, p, is_pen, &th, cx));
        }
        page = page.child(cards);
        if song.tracks.iter().any(|t| t.is_shared()) {
            page = page.child(
                note("Instrumento compartilhado: use a caneta (acima) pra escrever notas de cada pessoa na grade, ou selecione notas e reatribua à caneta. Cada nota soa e colore pela sua dona.").max_w(px(700.0)),
            );
        }
        page
    }
}

/// Ações desta página: `people::Add` (botão "+ pessoa" e paleta) e
/// `people::Rename` (clique no nome → prompt de texto do Shell).
pub fn bind(d: gpui_kit::Stateful<gpui_kit::Div>, cx: &mut Context<Shell>) -> gpui_kit::Stateful<gpui_kit::Div> {
    d.on_action(cx.listener(|shell: &mut Shell, _: &Add, _, cx| shell.views.people.update(cx, |v, cx| v.add(cx)))).on_action(cx.listener(|shell: &mut Shell, _: &Rename, w, cx| {
        if let Some((id, name)) = shell.views.people.read(cx).rename_target(cx) {
            shell.ask(InputTarget::PersonName(id), name, w, cx);
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wadawko_core::model::{InstrumentId, Track};

    #[test]
    fn rename_target_falls_back_to_first_person() {
        let mut s = Song::default();
        assert_eq!(rename_target(&s, Some("x")), None);
        let a = ops::people::add_person(&mut s, Some("Ana"));
        let b = ops::people::add_person(&mut s, Some("Bia"));
        assert_eq!(rename_target(&s, Some(&b)), Some((b.clone(), "Bia".into())));
        assert_eq!(rename_target(&s, None), Some((a.clone(), "Ana".into())));
        // pessoa apagada entre o clique e a ação
        ops::people::delete_person(&mut s, &b);
        assert_eq!(rename_target(&s, Some(&b)), Some((a, "Ana".into())));
    }

    #[test]
    fn checklist_marks_shared() {
        let mut s = Song::default();
        s.tracks.push(Track::new("t", "Shime", InstrumentId::Shime));
        let a = ops::people::add_person(&mut s, None);
        let b = ops::people::add_person(&mut s, None);
        assert_eq!(owner_count(&s, "t"), 0);
        ops::people::toggle_track_person(&mut s, "t", &a);
        assert_eq!(owner_count(&s, "t"), 1);
        ops::people::toggle_track_person(&mut s, "t", &b);
        assert_eq!(owner_count(&s, "t"), 2);
        assert!(s.track("t").unwrap().is_shared());
        ops::people::toggle_track_person(&mut s, "t", &a);
        assert_eq!(owner_count(&s, "t"), 1);
        assert_eq!(owner_count(&s, "nope"), 0);
    }
}
