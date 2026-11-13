//! Topbar: marca, título, BPM, reto/swing, loop, metrônomo, count-in, treino
//! (com % e passo), tocar/parar, tema (porte da topbar de `App.svelte`).

use gpui_kit::{Action, Context, FontWeight, ScrollEvent, SharedString, div, prelude::*, px};

use super::Tab;
use crate::keys;
use crate::shell::Shell;
use crate::theme::Theme;
use crate::ui::{abtn, atoggle, segmented};

impl Shell {
    pub(crate) fn topbar(&self, th: &Theme, playing: bool, cx: &mut Context<Self>) -> gpui_kit::Div {
        let st = self.state.read(cx);
        let song = st.song().clone();
        let tp = st.tp.clone();
        let bpm = song.tempo;
        let sounding = st.playback_bpm;
        let title: SharedString = wadawko_store::display_title(&song).into();
        let chip = |id: &'static str, text: String, a: Box<dyn Action>| {
            abtn_boxed(id, th, a).h(px(26.0)).px(px(8.0)).text_size(px(12.0)).child(SharedString::from(text))
        };
        div()
            .h(px(57.0))
            .flex_none()
            .w_full()
            .px(px(16.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .bg(th.paper)
            .border_b_1()
            .border_color(th.line)
            .child(
                div()
                    .id("logo")
                    // easter egg: 5 cliques no logo tocam "Sakura Sakura" no fue
                    .on_click(cx.listener(|shell: &mut Shell, _, _, cx| shell.views.lane.update(cx, |v, cx| v.logo_click(cx))))
                    .size(px(37.0))
                    .flex_none()
                    .rounded(px(8.0))
                    .bg(th.accent)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(th.on_accent)
                    .text_size(px(22.0))
                    .font_family("Shippori Mincho")
                    .font_weight(FontWeight::BOLD)
                    .child("太"),
            )
            .child(abtn("hamb", th, keys::view::ToggleNav).w(px(32.0)).px(px(0.0)).child("☰"))
            .child(
                // clique no título = renomear a música
                abtn("title", th, keys::project::Rename)
                    .ml(px(4.0))
                    .border_color(gpui_kit::transparent_black())
                    .bg(gpui_kit::transparent_black())
                    .h(px(38.0))
                    .min_w_0()
                    .text_size(px(22.0))
                    .font_family("Shippori Mincho")
                    .font_weight(FontWeight::BOLD)
                    .child(div().truncate().child(title)),
            )
            .child(div().flex_1())
            .child(div().text_size(px(12.0)).text_color(th.ink_soft).child("BPM"))
            .child(
                // roda do mouse = ±1; clique = digitar
                abtn("bpm", th, keys::transport::SetBpm)
                    .w(px(64.0))
                    .h(px(30.0))
                    .px(px(8.0))
                    .justify_start()
                    .bg(th.paper_raise)
                    .child(SharedString::from(format!("{bpm:.0}")))
                    .on_scroll(cx.listener(|this, ev: &ScrollEvent, _, cx| {
                        let d = f32::from(ev.delta.pixel_delta(px(20.0)).y);
                        let b = this.song(cx).tempo + if d > 0.0 { 1.0 } else { -1.0 };
                        this.state.update(cx, |s, cx| s.set_bpm(b, cx));
                        cx.stop_propagation();
                        cx.notify();
                    })),
            )
            .when(playing && (sounding as f64 - bpm).abs() > 0.5, |d| d.child(div().text_size(px(12.0)).text_color(th.accent).child(SharedString::from(format!("♪ {sounding:.0}")))))
            .child(segmented(
                "swing",
                th,
                vec![("reto", Box::new(keys::transport::Straight)), ("swing", Box::new(keys::transport::SwingOn))],
                (song.swing() > 0.0) as usize,
                true,
            ))
            .child(atoggle("loop", th, tp.looping, keys::transport::ToggleLoop).child("↺ loop"))
            .child(atoggle("metro", th, tp.metronome, keys::transport::ToggleMetronome).w(px(38.0)).px(px(0.0)).child("🎵"))
            .child(atoggle("countin", th, tp.count_in, keys::transport::ToggleCountIn).w(px(38.0)).px(px(0.0)).child("⏱"))
            .child(atoggle("trainer", th, tp.trainer, keys::transport::ToggleTrainer).w(px(38.0)).px(px(0.0)).child("🏃"))
            .when(tp.trainer, |d| {
                d.child(chip("tr-pct", format!("de {:.0}%", tp.trainer_pct * 100.0), Box::new(keys::transport::TrainerPct)))
                    .child(chip("tr-step", format!("+{:.0}%/volta", tp.trainer_step * 100.0), Box::new(keys::transport::TrainerStep)))
            })
            .child(
                abtn("play", th, keys::transport::TogglePlay)
                    .w(px(96.0))
                    .bg(th.accent)
                    .border_color(th.accent)
                    .text_color(th.on_accent)
                    .font_weight(FontWeight::BOLD)
                    .child(if playing { "■ parar" } else { "▶ tocar" }),
            )
            .child(abtn("theme", th, keys::view::ToggleTheme).w(px(36.0)).px(px(0.0)).child(if th.dark { "☀" } else { "☾" }))
    }

    /// Abas grade / partitura / esteira.
    pub(crate) fn view_tabs(&self, th: &Theme) -> gpui_kit::AnyElement {
        segmented(
            "view",
            th,
            vec![("grade", Box::new(keys::view::Grade)), ("partitura", Box::new(keys::view::Partitura)), ("esteira", Box::new(keys::view::Esteira))],
            match self.tab {
                Tab::Grade => 0,
                Tab::Partitura => 1,
                Tab::Esteira => 2,
            },
            false,
        )
        .into_any_element()
    }
}

/// `abtn` para uma ação já "boxed".
fn abtn_boxed(id: &'static str, th: &Theme, a: Box<dyn Action>) -> gpui_kit::Stateful<gpui_kit::Div> {
    let a2 = a.boxed_clone();
    keys::hintable(crate::ui::btn(id, th).on_click(move |_, w, cx| w.dispatch_action(a.boxed_clone(), cx)), id, std::rc::Rc::new(move |w: &mut gpui_kit::Window, cx: &mut gpui_kit::App| w.dispatch_action(a2.boxed_clone(), cx)))
}
