//! Painel direito (280 px, `chrome`): bloco INSPECTOR (contextual, F4 — ver
//! `views/inspector.rs`; altura máxima de 45 %, rola) e bloco MÚSICAS
//! (trocar/nova/duplicar/renomear/excluir) com exportar/importar JSON e
//! MP3/WAV no rodapé.

use std::rc::Rc;

use gpui_kit::{App, ClickEvent, Context, FontWeight, SharedString, div, prelude::*, px};

use crate::keys;
use crate::shell::Shell;
use crate::theme::Theme;
use crate::ui::{self, abtn, btn, panel_header};

/// "10 de nov." a partir de epoch ms (UTC).
pub fn short_date(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    // civil_from_days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    const M: [&str; 12] = ["jan.", "fev.", "mar.", "abr.", "mai.", "jun.", "jul.", "ago.", "set.", "out.", "nov.", "dez."];
    format!("{:02} de {}", d, M[(m - 1) as usize])
}

impl Shell {
    pub(crate) fn right_panel(&self, th: &Theme, cx: &mut Context<Self>) -> gpui_kit::Div {
        let st = self.state.read(cx);
        let current = st.store.current_id().to_string();
        let projects: Vec<(String, String, i64)> = st.store.projects().iter().map(|p| (p.id.clone(), p.title.clone(), p.updated_at)).collect();
        let export = st.export_label();
        let many = projects.len() > 1;
        let weak = cx.entity().downgrade();
        // contexto do cursor (seção · compasso · tempo.sub · linha) — já calculado para a status bar
        let context: SharedString = self.status.pos_text().map(str::to_string).unwrap_or_else(|| "sem cursor na grade".into()).into();
        let inspector = self.inspector(th, cx);
        let mini = |id: &'static str, label: &'static str, a: Box<dyn gpui_kit::Action>| {
            btn(id, th).h(px(18.0)).w(px(20.0)).px(px(0.0)).text_size(px(10.0)).child(label).on_click(move |_: &ClickEvent, w, cx| {
                cx.stop_propagation();
                w.dispatch_action(a.boxed_clone(), cx)
            })
        };
        let mut list = div().flex().flex_col().gap(px(4.0)).p(px(8.0));
        for (i, (id, title, updated)) in projects.into_iter().enumerate() {
            let on = id == current;
            let date: SharedString = if updated > 0 { short_date(updated) } else { "agora".into() }.into();
            let w = weak.clone();
            let card = div()
                .id(("song", i))
                .relative()
                .pl(px(10.0))
                .pr(px(6.0))
                .py(px(6.0))
                .rounded(px(ui::R0))
                .border_1()
                .border_color(th.line)
                .flex()
                .items_start()
                .gap(px(4.0))
                .when(on, |d| d.bg(th.accent2_wash).child(div().absolute().left_0().top_0().bottom_0().w(px(2.0)).bg(th.accent2)))
                .when(!on, |d| d.bg(th.paper).cursor_pointer().hover(|s| s.bg(th.paper_hover)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(div().text_size(px(12.0)).font_weight(FontWeight::BOLD).text_color(if on { th.ink } else { th.ink_soft }).truncate().child(SharedString::from(title)))
                        .child(div().text_size(px(10.0)).text_color(th.ink_faint).child(date)),
                )
                .when(on, |d| {
                    d.child(
                        div()
                            .flex()
                            .gap(px(2.0))
                            .child(mini("ren", "✎", Box::new(keys::project::Rename)))
                            .child(mini("dup", "⧉", Box::new(keys::project::Duplicate)))
                            .when(many, |d| d.child(mini("del", "✕", Box::new(keys::project::Delete)))),
                    )
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select_project(i, cx)));
            list = list.child(keys::hintable(
                card,
                format!("song-{i}"),
                Rc::new(move |_, cx: &mut App| {
                    let _ = w.update(cx, |this, cx| this.select_project(i, cx));
                }),
            ));
        }
        let foot_btn = |b: gpui_kit::Stateful<gpui_kit::Div>| b.flex_1().h(px(24.0)).px(px(4.0)).text_size(px(11.0));
        div()
            .w(px(280.0))
            .flex_none()
            .h_full()
            .bg(th.chrome)
            .border_l_1()
            .border_color(th.line)
            .flex()
            .flex_col()
            // ── inspector (F4): segue o cursor da grade; no máximo 45 % do painel, rola ──
            .child(panel_header("Inspector", th).child(div().text_size(px(10.0)).font_weight(FontWeight::MEDIUM).text_color(th.ink_faint).truncate().child(context)))
            .child(div().id("inspector").flex_none().max_h(gpui_kit::relative(0.45)).overflow_y_scroll().child(inspector))
            // ── músicas ──
            .child(
                panel_header("Músicas", th).child(
                    abtn("nova", th, keys::project::New)
                        .h(px(18.0))
                        .px(px(6.0))
                        .bg(th.accent)
                        .border_color(th.accent)
                        .text_color(th.on_accent)
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .child("+ nova"),
                ),
            )
            .child(div().id("songs").flex_1().min_h_0().overflow_y_scroll().child(list))
            .when_some(export.clone(), |d, label| {
                d.child(
                    div()
                        .mx(px(8.0))
                        .px(px(6.0))
                        .h(px(22.0))
                        .rounded(px(ui::R1))
                        .bg(th.accent_wash)
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(div().flex_1().min_w_0().truncate().text_size(px(11.0)).text_color(th.accent).child(SharedString::from(format!("⏳ {label}"))))
                        .child(abtn("exp-cancel", th, keys::project::CancelExport).h(px(18.0)).px(px(6.0)).text_size(px(10.0)).child("cancelar")),
                )
            })
            .child(
                div()
                    .flex_none()
                    .p(px(8.0))
                    .border_t_1()
                    .border_color(th.line)
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex()
                            .gap(px(6.0))
                            .child(foot_btn(abtn("exp", th, keys::project::ExportJson)).child("exportar"))
                            .child(foot_btn(abtn("imp", th, keys::project::ImportJson)).child("importar")),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(6.0))
                            .child(foot_btn(abtn("mp3", th, keys::project::ExportMp3)).child(if export.is_some() { "exportando…" } else { "exportar mp3" }))
                            .child(foot_btn(abtn("wav", th, keys::project::ExportWav)).flex_none().w(px(56.0)).child("wav")),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn short_date_formats_utc_days() {
        assert_eq!(super::short_date(0), "01 de jan.");
        // 2026-11-10T12:00:00Z
        assert_eq!(super::short_date(1_794_312_000_000), "10 de nov.");
    }
}
