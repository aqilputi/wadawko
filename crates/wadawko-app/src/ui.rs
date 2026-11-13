//! Peças de UI compartilhadas: botões que despacham ações (clique = atalho =
//! paleta = salto por rótulo), grupo segmentado, tecla (`kbd`), cartão.

use std::rc::Rc;

use gpui_kit::{Action, App, FontWeight, SharedString, Window, div, prelude::*, px};

use crate::keys;
use crate::theme::Theme;

/// Botão padrão (com hover). Para um hover próprio use [`btn_bare`]: o gpui
/// (debug) entra em pânico se `.hover` for chamado duas vezes no mesmo elemento.
pub fn btn(id: impl Into<gpui_kit::ElementId>, th: &Theme) -> gpui_kit::Stateful<gpui_kit::Div> {
    let hover = th.paper_hover;
    btn_bare(id, th).hover(move |s| s.bg(hover))
}

/// [`btn`] sem estilo de hover (o chamador define o seu).
pub fn btn_bare(id: impl Into<gpui_kit::ElementId>, th: &Theme) -> gpui_kit::Stateful<gpui_kit::Div> {
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
}

/// Botão que despacha uma ação (clique = atalho = paleta = salto).
pub fn abtn<A: Action + Clone>(id: &'static str, th: &Theme, a: A) -> gpui_kit::Stateful<gpui_kit::Div> {
    let hover = th.paper_hover;
    abtn_bare(id, th, a).hover(move |s| s.bg(hover))
}

/// [`abtn`] sem estilo de hover (o chamador define o seu).
pub fn abtn_bare<A: Action + Clone>(id: &'static str, th: &Theme, a: A) -> gpui_kit::Stateful<gpui_kit::Div> {
    let a2 = a.clone();
    keys::hintable(btn_bare(id, th).on_click(move |_, w, cx| w.dispatch_action(a.boxed_clone(), cx)), id, keys::dispatch(a2))
}

pub fn atoggle<A: Action + Clone>(id: &'static str, th: &Theme, on: bool, a: A) -> gpui_kit::Stateful<gpui_kit::Div> {
    let b = abtn(id, th, a);
    if on { b.bg(th.accent2_wash).border_color(th.accent2).text_color(th.accent2) } else { b }
}

/// Grupo segmentado; cada segmento despacha a sua ação e é alvo de salto.
pub fn segmented(id: &'static str, th: &Theme, items: Vec<(&'static str, Box<dyn Action>)>, active: usize, blue: bool) -> impl IntoElement {
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

pub fn kbd(text: &str, th: &Theme) -> impl IntoElement {
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

/// Botão pequeno (22×24) das linhas e cabeçalhos da grade.
pub fn small_btn(id: impl Into<gpui_kit::ElementId>, label: impl Into<SharedString>, th: &Theme, on: bool) -> gpui_kit::Stateful<gpui_kit::Div> {
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
        .child(label.into())
}

/// Cartão de página (Configurações, Kit).
pub fn card(th: &Theme) -> gpui_kit::Div {
    div().p(px(14.0)).rounded(px(10.0)).bg(th.paper).border_1().border_color(th.line).flex().flex_col().gap(px(8.0))
}
