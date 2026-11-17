//! NavRail: páginas do app (porte de `NavRail.svelte`).

use std::rc::Rc;

use gpui_kit::{Action, App, FontWeight, Window, div, prelude::*, px};

use super::Page;
use crate::keys;
use crate::shell::Shell;
use crate::theme::Theme;
use crate::ui;

impl Shell {
    pub(crate) fn nav_rail(&self, th: &Theme) -> gpui_kit::Div {
        let items: [(&str, &str, Page, Box<dyn Action>); 5] = [
            ("📝", "Editor", Page::Editor, Box::new(keys::nav::Editor)),
            ("👥", "Pessoas", Page::Pessoas, Box::new(keys::nav::Pessoas)),
            ("🎚", "Mixer", Page::Mixer, Box::new(keys::nav::Mixer)),
            ("🥁", "Kit", Page::Kit, Box::new(keys::nav::Kit)),
            ("⚙", "Config", Page::Config, Box::new(keys::nav::Config)),
        ];
        let mut col = div().w(px(200.0)).flex_none().h_full().bg(th.paper).border_r_1().border_color(th.line).p(px(6.0)).flex().flex_col().gap(px(4.0));
        for (i, (icon, label, page, a)) in items.into_iter().enumerate() {
            let on = page == self.page;
            let a2 = a.boxed_clone();
            let item = div()
                .id(("nav", i))
                .relative()
                .h(px(38.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .gap(px(10.0))
                .rounded(px(ui::R0))
                .border_1()
                .text_size(px(14.0))
                .font_weight(FontWeight::MEDIUM)
                .when(on, |d| d.bg(th.accent_wash).border_color(th.accent).text_color(th.accent))
                .when(!on, |d| d.border_color(gpui_kit::transparent_black()).text_color(th.ink_soft).cursor_pointer().hover(|s| s.bg(th.paper_hover)))
                .child(div().w(px(20.0)).child(icon))
                .child(label)
                .on_click(move |_, w, cx| w.dispatch_action(a.boxed_clone(), cx));
            col = col.child(keys::hintable(item, format!("nav-{i}"), Rc::new(move |w: &mut Window, cx: &mut App| w.dispatch_action(a2.boxed_clone(), cx))));
        }
        col
    }
}
