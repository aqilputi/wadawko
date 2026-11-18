//! NavRail (64 px, `chrome`): páginas do app como itens empilhados — glifo
//! monocromático + rótulo de 10 px; a página atual leva uma barra esquerda de
//! 2 px `accent` (docs/planejamento/daw.md §2.4). `Ctrl+B` esconde.

use std::rc::Rc;

use gpui_kit::{Action, App, FontWeight, Window, div, prelude::*, px, transparent_black};

use super::Page;
use crate::keys;
use crate::shell::Shell;
use crate::theme::Theme;

impl Shell {
    pub(crate) fn nav_rail(&self, th: &Theme) -> gpui_kit::Div {
        let items: [(&str, &str, Page, Box<dyn Action>); 5] = [
            ("▦", "Editor", Page::Editor, Box::new(keys::nav::Editor)),
            ("◉", "Pessoas", Page::Pessoas, Box::new(keys::nav::Pessoas)),
            ("≡", "Mixer", Page::Mixer, Box::new(keys::nav::Mixer)),
            ("◎", "Kit", Page::Kit, Box::new(keys::nav::Kit)),
            ("⚙", "Config", Page::Config, Box::new(keys::nav::Config)),
        ];
        let mut col = div().w(px(64.0)).flex_none().h_full().bg(th.chrome).border_r_1().border_color(th.line).py(px(4.0)).flex().flex_col().gap(px(2.0));
        for (i, (icon, label, page, a)) in items.into_iter().enumerate() {
            let on = page == self.page;
            let a2 = a.boxed_clone();
            let item = div()
                .id(("nav", i))
                .relative()
                .w_full()
                .h(px(52.0))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(3.0))
                .border_l_2()
                .border_color(if on { th.accent } else { transparent_black() })
                .text_color(if on { th.ink } else { th.ink_soft })
                .cursor_pointer()
                .when(!on, |d| d.hover(|s| s.bg(th.paper_hover)))
                .child(div().text_size(px(18.0)).child(icon))
                .child(div().text_size(px(10.0)).font_weight(FontWeight::MEDIUM).child(label))
                .on_click(move |_, w, cx| w.dispatch_action(a.boxed_clone(), cx));
            col = col.child(keys::hintable(item, format!("nav-{i}"), Rc::new(move |w: &mut Window, cx: &mut App| w.dispatch_action(a2.boxed_clone(), cx))));
        }
        col
    }
}
