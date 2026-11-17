//! Barra de contexto do editor (porte de `ContextToolbar.svelte`): modo
//! editar/selecionar, subdivisão, + seção, instrumento, + linha, zoom, caneta de
//! pessoa (quem assina as notas em linha compartilhada) e região de playback.

use std::rc::Rc;

use gpui_kit::{App, ClickEvent, Context, FontWeight, SharedString, div, prelude::*, px};
use wadawko_core::ops;

use super::grid::Mode;
use crate::keys;
use crate::shell::Shell;
use crate::song_ext::SongExt;
use crate::theme::{Theme, parse_color};
use crate::ui::{self, abtn, kbd, segmented};

impl Shell {
    pub(crate) fn context_toolbar(&self, th: &Theme, cx: &mut Context<Self>) -> gpui_kit::Div {
        let st = self.state.read(cx);
        let song = st.song().clone();
        let pen = st.pen.clone();
        let selected = st.tp.selected.clone();
        let (mode, zoom_pct, has_sel, beats) = {
            let g = self.grid.read(cx);
            let beats = g.cursor.and_then(|c| song.measures.get(c.m)).map_or(4, |m| m.beats);
            (g.mode, g.zoom_pct(), g.anchor.is_some(), beats)
        };
        let inst_name = ops::kit::instrument_by_id(&song, Some(&self.add_instrument)).map(|d| d.name.clone()).unwrap_or_else(|| "Chū-daiko".into());
        // faixa plana sobre `chrome` (h 34 numa linha; quebra em duas só se faltar largura)
        let mut toolbar =
            div().flex().flex_wrap().items_center().gap_x(px(8.0)).gap_y(px(4.0)).px(px(10.0)).py(px(4.0)).min_h(px(34.0)).rounded(px(ui::R0)).bg(th.chrome).border_b_1().border_color(th.line);
        if !self.vim_on {
            toolbar =
                toolbar.child(segmented("mode", th, vec![("✎ editar", Box::new(keys::grid::ModeEdit)), ("⬚ selecionar", Box::new(keys::grid::ModeSelect))], (mode == Mode::Select) as usize, false));
        }
        let mut toolbar = toolbar
            .child(segmented(
                "sub",
                th,
                vec![("colcheia", Box::new(keys::song::Sub2)), ("semicolcheia", Box::new(keys::song::Sub4)), ("fusa", Box::new(keys::song::Sub8))],
                match song.sub() {
                    4 => 1,
                    8 => 2,
                    _ => 0,
                },
                false,
            ))
            // fórmula do compasso do cursor (ou dos selecionados): − 4/4 +
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(div().text_size(px(11.0)).text_color(th.ink_faint).child("compasso"))
                    .child(abtn("beats-", th, keys::measure::BeatsDown).w(px(30.0)).px(px(0.0)).child("−"))
                    .child(div().w(px(34.0)).flex().justify_center().text_size(px(13.0)).font_weight(FontWeight::BOLD).text_color(th.ink).child(SharedString::from(format!("{beats}/4"))))
                    .child(abtn("beats+", th, keys::measure::BeatsUp).w(px(30.0)).px(px(0.0)).child("+")),
            )
            .child(abtn("addsec", th, keys::section::Add).child("+ seção"))
            .child(abtn("inst", th, keys::track::CycleInstrument).w(px(150.0)).justify_between().child(SharedString::from(inst_name)).child(div().text_color(th.ink_faint).child("▾")))
            .child(abtn("addline", th, keys::track::Add).child("+ linha"))
            .child(abtn("zout", th, keys::view::ZoomOut).child("🔍−"))
            .child(div().w(px(44.0)).flex().justify_center().text_size(px(12.0)).text_color(th.ink_soft).child(SharedString::from(format!("{zoom_pct}%"))))
            .child(abtn("zin", th, keys::view::ZoomIn).child("🔍+"));

        // caneta de pessoa
        if !song.people().is_empty() {
            let mut pens = div().flex().items_center().gap(px(4.0)).child(div().text_size(px(11.0)).text_color(th.ink_faint).child("✒ caneta"));
            for (i, p) in song.people().iter().enumerate() {
                let on = pen.as_deref() == Some(p.id.as_str());
                let col = p.color.as_deref().and_then(parse_color).unwrap_or(th.ink_soft);
                let pid = p.id.clone();
                let state = self.state.clone();
                let state2 = self.state.clone();
                let pid2 = p.id.clone();
                let set = move |state: &gpui_kit::Entity<crate::state::AppState>, pid: &str, cx: &mut App| {
                    state.update(cx, |s, cx| {
                        s.pen = if s.pen.as_deref() == Some(pid) { None } else { Some(pid.to_string()) };
                        cx.notify();
                    })
                };
                pens = pens.child(keys::hintable(
                    div()
                        .id(("pen", i))
                        .relative()
                        .h(px(26.0))
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .gap(px(5.0))
                        .rounded(px(ui::R1))
                        .border_1()
                        .text_size(px(12.0))
                        .cursor_pointer()
                        .when(on, |d| d.border_color(col).bg(Theme::alpha(col, 0.18)))
                        .when(!on, |d| d.border_color(th.line).hover(|s| s.bg(th.paper_hover)))
                        .child(div().size(px(8.0)).rounded_full().bg(col))
                        .child(SharedString::from(p.name.clone()))
                        .on_click(move |_: &ClickEvent, _, cx| set(&state, &pid, cx)),
                    format!("pen-{i}"),
                    Rc::new(move |_, cx: &mut App| set(&state2, &pid2, cx)),
                ));
            }
            if has_sel && pen.is_some() {
                pens = pens.child(abtn("assign", th, keys::grid::AssignToPen).h(px(26.0)).text_size(px(12.0)).child("→ atribuir seleção"));
            }
            toolbar = toolbar.child(pens);
        }

        // região de playback (seções selecionadas)
        if !selected.is_empty() {
            let names: Vec<String> = song.sections().iter().filter(|s| selected.contains(&s.id)).map(|s| s.name.clone()).collect();
            toolbar = toolbar.child(
                abtn("region", th, keys::transport::ClearRegion)
                    .h(px(26.0))
                    .text_size(px(12.0))
                    .border_color(th.accent)
                    .text_color(th.accent)
                    .bg(th.accent_wash)
                    .child(SharedString::from(format!("▶ só: {}  ✕", names.join(" + ")))),
            );
        }

        toolbar
            .child(div().flex_1())
            .child(div().flex().items_center().gap(px(6.0)).text_size(px(11.0)).text_color(th.ink_faint).child(kbd("Ctrl K", th)).child("comandos").child(kbd("Ctrl ␣", th)).child("saltar"))
    }
}
