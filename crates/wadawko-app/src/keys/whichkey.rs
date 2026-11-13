//! Which-key (command hint): com um chord pendente (ex.: `space`), mostra as
//! próximas teclas possíveis com título; Ctrl+/ mostra o mesmo para a raiz.

use gpui_kit::{App, FontWeight, SharedString, Window, deferred, div, prelude::*, px};

use crate::keys;
use crate::shell::Shell;
use crate::theme::Theme;
use crate::ui;

impl Shell {
    pub(crate) fn which_key(&self, window: &Window, th: &Theme, cx: &App) -> impl IntoElement + use<> {
        let os = cx.platform_info().os;
        let n = self.pending.len();
        // forma explícita ("a" vs "shift-a"): a exibição do GPUI põe tudo em maiúscula
        let prefix: String = self.pending.iter().map(|k| k.unparse(os)).collect::<Vec<_>>().join(" ");
        let mut entries: Vec<(String, String, bool)> = Vec::new();
        let mut digits = false;
        let all = window.possible_bindings_for_input(&self.pending);
        for b in &all {
            let ks = b.keystrokes();
            if ks.len() <= n || gpui_kit::is_no_action(b.action()) {
                continue;
            }
            // só ações do app (os bindings internos do gpui-component não interessam aqui)
            if ks.len() == n + 1 && keys::title_for(b.action().name()).is_none() {
                continue;
            }
            let key = ks[n].unparse(os);
            if b.action().name().starts_with("vim::Digit") && ks.len() == n + 1 {
                digits = true;
                continue;
            }
            if entries.iter().any(|e| e.0 == key) {
                continue;
            }
            if ks.len() == n + 1 {
                let t = keys::title_for(b.action().name()).unwrap_or(b.action().name());
                entries.push((key, t.to_string(), false));
            } else {
                let p = format!("{prefix} {key}").trim().to_string();
                let t = keys::prefix_title(&p).unwrap_or("…");
                let count = all.iter().filter(|o| o.keystrokes().len() > n && o.keystrokes()[n].unparse(os) == key && !gpui_kit::is_no_action(o.action())).count();
                entries.push((key, format!("+{t} ({count})"), true));
            }
        }
        if digits {
            entries.push(("1–9".into(), "contagem (repete o próximo comando)".into(), false));
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut grid_el = div().flex().flex_wrap().gap_x(px(18.0)).gap_y(px(4.0));
        for (k, t, group) in entries {
            grid_el = grid_el.child(
                div()
                    .w(px(196.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(ui::kbd(&k, th))
                    .child(div().text_size(px(12.0)).text_color(if group { th.accent } else { th.ink_soft }).truncate().child(SharedString::from(t))),
            );
        }
        let title = if prefix.is_empty() {
            "Atalhos disponíveis aqui (Ctrl+/ ou Esc fecha)".to_string()
        } else {
            keys::prefix_title(&prefix).map(|t| format!("{prefix} — {t}")).unwrap_or(prefix)
        };
        deferred(
            div()
                .absolute()
                .left(px(220.0))
                .right(px(280.0))
                .bottom(px(34.0))
                .max_h(px(620.0))
                .overflow_hidden()
                .p(px(12.0))
                .rounded(px(10.0))
                .bg(th.paper)
                .border_1()
                .border_color(th.line_strong)
                .shadow_lg()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(div().text_size(px(12.0)).font_weight(FontWeight::BOLD).text_color(th.ink).child(SharedString::from(title)))
                .child(grid_el),
        )
        .with_priority(150)
    }
}
