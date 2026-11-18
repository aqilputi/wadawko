//! Peças de UI compartilhadas: botões que despacham ações (clique = atalho =
//! paleta = salto por rótulo), toggle "lâmpada", grupo segmentado, aba, tecla
//! (`kbd`), cabeçalho de painel, medidor e cartão — no sistema de design
//! "estilo DAW" (docs/planejamento/daw.md §2.3–2.5): cantos retos nas
//! superfícies (`R0`), 2 px nos controles (`R1`), estados planos.

use std::rc::Rc;

use gpui_kit::{Action, App, BorderStyle, Bounds, FontWeight, SharedString, Window, canvas, div, point, prelude::*, px, quad, size, transparent_black};

use crate::keys;
use crate::theme::Theme;

/// raio de superfície: painéis, cartões, popovers, abas, células, cursor, seleção
pub const R0: f32 = 0.0;
/// raio de controle: botão, chip, kbd, campo, segmento, slider, medidor, swatch
pub const R1: f32 = 2.0;

/// Botão padrão (com hover). Para um hover próprio use [`btn_bare`]: o gpui
/// (debug) entra em pânico se `.hover` for chamado duas vezes no mesmo elemento.
pub fn btn(id: impl Into<gpui_kit::ElementId>, th: &Theme) -> gpui_kit::Stateful<gpui_kit::Div> {
    let hover = th.paper_hover;
    btn_bare(id, th).hover(move |s| s.bg(hover))
}

/// [`btn`] sem estilo de hover (o chamador define o seu). h 26, texto 12.
pub fn btn_bare(id: impl Into<gpui_kit::ElementId>, th: &Theme) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .relative()
        .flex_none()
        .h(px(26.0))
        .px(px(10.0))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .rounded(px(R1))
        .border_1()
        .border_color(th.line_strong)
        .bg(th.paper_raise)
        .text_size(px(12.0))
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
    let tip = a.boxed_clone();
    keys::hintable(with_tip(btn_bare(id, th), th, tip).on_click(move |_, w, cx| w.dispatch_action(a.boxed_clone(), cx)), id, keys::dispatch(a2))
}

/// Tooltip de um controle que despacha `a` (F14): o título do comando (o mesmo
/// da paleta) e o atalho de maior precedência no keymap ATUAL — lido na hora
/// de mostrar, então reflete o keymap do usuário recarregado. Sem título
/// conhecido, não põe tooltip. Não chame `.tooltip` de novo no elemento.
pub fn with_tip(el: gpui_kit::Stateful<gpui_kit::Div>, th: &Theme, a: Box<dyn Action>) -> gpui_kit::Stateful<gpui_kit::Div> {
    let Some(title) = keys::title_for(a.name()) else { return el };
    let th = *th;
    el.tooltip(move |_w, cx| {
        let shortcut = keys::binding_text(a.as_ref(), cx);
        cx.new(|_| Tip { title, shortcut, th }).into()
    })
}

/// A caixinha do tooltip: `paper_raise`, borda `line_strong`, R1; atalho em `kbd`.
struct Tip {
    title: &'static str,
    shortcut: Option<String>,
    th: Theme,
}

impl gpui_kit::Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        let th = &self.th;
        div()
            .ml(px(8.0))
            .mt(px(10.0))
            .px(px(6.0))
            .py(px(3.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .rounded(px(R1))
            .border_1()
            .border_color(th.line_strong)
            .bg(th.paper_raise)
            .shadow_md()
            .text_size(px(11.0))
            .text_color(th.ink)
            .child(self.title)
            .when_some(self.shortcut.clone(), |d, k| d.child(kbd(&k, th)))
    }
}

/// Botão de ícone 26×26 (glifo monocromático).
pub fn icon_btn<A: Action + Clone>(id: &'static str, glyph: &'static str, th: &Theme, a: A) -> gpui_kit::Stateful<gpui_kit::Div> {
    abtn(id, th, a).w(px(26.0)).px(px(0.0)).child(glyph)
}

/// Toggle "lâmpada": desligado é um botão comum; ligado fica PREENCHIDO com
/// `accent` e texto `on_accent` (loop, metrônomo, count-in, treino).
pub fn atoggle<A: Action + Clone>(id: &'static str, th: &Theme, on: bool, a: A) -> gpui_kit::Stateful<gpui_kit::Div> {
    if on {
        let acc = th.accent;
        abtn_bare(id, th, a).bg(acc).border_color(acc).text_color(th.on_accent).font_weight(FontWeight::BOLD).hover(move |s| s.bg(Theme::alpha(acc, 0.9)))
    } else {
        abtn(id, th, a)
    }
}

/// Grupo segmentado; cada segmento despacha a sua ação e é alvo de salto. O
/// segmento ativo não é preenchido de cor: fundo `paper_hover`, texto `ink` e
/// uma linha inferior de 2 px (`accent`, ou `accent2` se `blue`).
pub fn segmented(id: &'static str, th: &Theme, items: Vec<(&'static str, Box<dyn Action>)>, active: usize, blue: bool) -> impl IntoElement {
    let on_col = if blue { th.accent2 } else { th.accent };
    let mut row = div().flex().flex_none().h(px(26.0)).rounded(px(R1)).border_1().border_color(th.line_strong).bg(th.paper_raise).overflow_hidden();
    for (i, (l, a)) in items.into_iter().enumerate() {
        let on = i == active;
        let a2 = a.boxed_clone();
        let hid = format!("{id}-{i}");
        let tip = a.boxed_clone();
        row = row.child(keys::hintable(
            with_tip(div().id((id, i)), th, tip)
                .relative()
                .h_full()
                .px(px(10.0))
                .flex()
                .items_center()
                .text_size(px(12.0))
                .font_weight(if on { FontWeight::BOLD } else { FontWeight::MEDIUM })
                .cursor_pointer()
                .border_b_2()
                .border_color(if on { on_col } else { transparent_black() })
                .when(on, |d| d.bg(th.paper_hover).text_color(th.ink))
                .when(!on, |d| d.text_color(th.ink_soft).hover(|s| s.bg(th.paper_hover)))
                .child(l)
                .on_click(move |_, w, cx| w.dispatch_action(a.boxed_clone(), cx)),
            hid,
            Rc::new(move |w: &mut Window, cx: &mut App| w.dispatch_action(a2.boxed_clone(), cx)),
        ));
    }
    row
}

/// Aba do editor: texto `ink_soft`, h 28, sem fundo; ativa = `ink` + sublinhado 2 px `accent`.
pub fn tab<A: Action + Clone>(id: &'static str, label: &'static str, on: bool, th: &Theme, a: A) -> gpui_kit::Stateful<gpui_kit::Div> {
    let a2 = a.clone();
    let ink = th.ink;
    let el = with_tip(div().id(id), th, a.boxed_clone())
        .relative()
        .flex_none()
        .h(px(28.0))
        .px(px(10.0))
        .flex()
        .items_center()
        .text_size(px(12.0))
        .font_weight(if on { FontWeight::BOLD } else { FontWeight::MEDIUM })
        .border_b_2()
        .border_color(if on { th.accent } else { transparent_black() })
        .text_color(if on { th.ink } else { th.ink_soft })
        .cursor_pointer()
        .when(!on, |d| d.hover(move |s| s.text_color(ink)))
        .child(label)
        .on_click(move |_, w, cx| w.dispatch_action(a.boxed_clone(), cx));
    keys::hintable(el, id, keys::dispatch(a2))
}

/// Tecla de atalho (h 16, texto 10).
pub fn kbd(text: &str, th: &Theme) -> impl IntoElement {
    div()
        .px(px(4.0))
        .h(px(16.0))
        .flex()
        .items_center()
        .rounded(px(R1))
        .border_1()
        .border_color(th.line_strong)
        .bg(th.paper_raise)
        .text_size(px(10.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(th.ink_soft)
        .child(SharedString::from(text.to_string()))
}

/// Botão pequeno 20×16 (M/S/★/⚙/×) das linhas e cabeçalhos da grade, aceso
/// com a cor dada (M = `warn`, S = `accent2`, ★ = `accent`).
pub fn small_btn_on(id: impl Into<gpui_kit::ElementId>, label: impl Into<SharedString>, th: &Theme, on: bool, color: gpui_kit::Hsla) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(20.0))
        .h(px(16.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(R1))
        .text_size(px(10.0))
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .when(on, |d| d.bg(color).text_color(th.on_accent))
        .when(!on, |d| d.bg(th.paper_raise).text_color(th.ink_soft).hover(|s| s.bg(th.paper_hover)))
        .child(label.into())
}

/// Cabeçalho de painel: h 22, fundo `header`, título em caixa alta de 10 px.
/// Acrescente botões (h 18) à direita com `.child(...)`.
pub fn panel_header(title: &str, th: &Theme) -> gpui_kit::Div {
    div()
        .h(px(22.0))
        .flex_none()
        .w_full()
        .px(px(10.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .bg(th.header)
        .border_b_1()
        .border_color(th.line)
        .text_size(px(10.0))
        .font_weight(FontWeight::BOLD)
        .text_color(th.ink_soft)
        .child(div().flex_1().min_w_0().truncate().child(SharedString::from(title.to_uppercase())))
}

/// Cartão de página (Kit, Configurações; Pessoas e Mixer usam `group_card`):
/// `paper`, borda `line`, cantos retos, [`panel_header`] no topo (de borda a
/// borda) e `body` com o padding do cartão.
pub fn titled_card(title: &str, th: &Theme, body: gpui_kit::Div) -> gpui_kit::Div {
    div().rounded(px(R0)).bg(th.paper).border_1().border_color(th.line).flex().flex_col().overflow_hidden().child(panel_header(title, th)).child(body.p(px(14.0)).flex().flex_col().gap(px(8.0)))
}

// ───────────────────────────── medidor de nível (§2.5) ─────────────────────────────

/// Fração 0..1 do medidor para um nível linear: 0 dB = 1, −60 dB = 0 (piso).
pub fn meter_frac(level: f32) -> f32 {
    let db = 20.0 * level.max(1e-6).log10();
    ((db + 60.0) / 60.0).clamp(0.0, 1.0)
}

/// Bandas do medidor como frações: (fim da banda, cor) — verde até −18 dB,
/// `warn` até −6 dB, `accent` até 0 dB.
fn meter_bands(th: &Theme) -> [(f32, gpui_kit::Hsla); 3] {
    [(42.0 / 60.0, th.meter_lo), (54.0 / 60.0, th.meter_mid), (1.0, th.meter_hi)]
}

/// Pinta um medidor em `b`: fundo `cell_empty` com borda `line`, três bandas e
/// o pico retido como linha de 1 px `ink`. `level`/`peak` lineares (0..1+).
/// Usado por [`meter`] e pelos mini-medidores do overlay da grade.
pub fn paint_meter(window: &mut Window, b: Bounds<gpui_kit::Pixels>, level: f32, peak: f32, vertical: bool, th: &Theme) {
    let (lv, pk) = (meter_frac(level), meter_frac(peak));
    let (x0, y0, w, h) = (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height));
    window.paint_quad(quad(b, px(R1), th.cell_empty, px(1.0), th.line, BorderStyle::Solid));
    let mut from = 0.0f32;
    for (to, col) in meter_bands(th) {
        let hi = lv.min(to);
        if hi > from {
            let r = if vertical {
                Bounds::new(point(px(x0 + 1.0), px(y0 + 1.0 + (h - 2.0) * (1.0 - hi))), size(px((w - 2.0).max(1.0)), px((h - 2.0) * (hi - from))))
            } else {
                Bounds::new(point(px(x0 + 1.0 + (w - 2.0) * from), px(y0 + 1.0)), size(px((w - 2.0) * (hi - from)), px((h - 2.0).max(1.0))))
            };
            window.paint_quad(quad(r, px(0.0), col, px(0.0), transparent_black(), BorderStyle::Solid));
        }
        from = to;
    }
    if pk > 0.0 {
        let r = if vertical {
            Bounds::new(point(px(x0 + 1.0), px(y0 + 1.0 + (h - 2.0) * (1.0 - pk))), size(px((w - 2.0).max(1.0)), px(1.0)))
        } else {
            Bounds::new(point(px(x0 + (w - 1.0) * pk), px(y0 + 1.0)), size(px(1.0), px((h - 2.0).max(1.0))))
        };
        window.paint_quad(quad(r, px(0.0), th.ink, px(0.0), transparent_black(), BorderStyle::Solid));
    }
}

/// Medidor de nível: `level` e `peak` lineares (0..1+), vertical (w 6) ou
/// horizontal (h 3). A balística (queda, pico retido) é de quem chama
/// (`state::Level`); este elemento só desenha o que recebe.
pub fn meter(level: f32, peak: f32, vertical: bool, th: &Theme) -> impl IntoElement {
    let th = *th;
    let base = div().flex_none();
    let base = if vertical { base.w(px(6.0)).h_full() } else { base.h(px(3.0)).w_full() };
    base.child(canvas(|_, _, _| {}, move |b, _, window, _| paint_meter(window, b, level, peak, vertical, &th)).size_full())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_maps_db_to_fraction() {
        assert!((meter_frac(1.0) - 1.0).abs() < 1e-6, "0 dB no topo");
        assert!((meter_frac(0.5) - (1.0 - 6.02 / 60.0)).abs() < 1e-3, "−6 dB");
        assert!((meter_frac(0.001) - 0.0).abs() < 1e-6, "−60 dB na base");
        assert_eq!(meter_frac(0.0), 0.0);
        assert_eq!(meter_frac(2.0), 1.0, "acima de 0 dB prende no topo");
        let th = Theme::dark();
        let b = meter_bands(&th);
        assert!(b[0].0 < b[1].0 && b[1].0 < b[2].0 && b[2].0 == 1.0);
    }
}
