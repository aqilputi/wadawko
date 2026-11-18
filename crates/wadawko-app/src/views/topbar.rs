//! Barra de cima (h 44, `chrome`): marca + título à esquerda, o TRANSPORTE no
//! centro (⏮ · ▶/■ · ↺ · display com posição e BPM · reto/swing) e
//! metrônomo, count-in, treino e tema à direita (docs/planejamento/daw.md
//! §2.4 e §2.6). O display é a única "tela" da UI: campo `display` com
//! números em `accent2`.

use gpui_kit::{Action, App, Context, FontWeight, ScrollEvent, SharedString, div, prelude::*, px, transparent_black};

use super::Tab;
use crate::keys;
use crate::shell::Shell;
use crate::song_ext::SongExt;
use crate::theme::Theme;
use crate::ui::{self, abtn, abtn_bare, atoggle, icon_btn, segmented};

impl Shell {
    /// Posição mostrada no display: `c12 · 3.2` do playhead AUDÍVEL enquanto
    /// toca; parado, a célula do cursor da grade (ou de onde o play vai
    /// começar). Entra na chave de cache da barra de cima.
    pub(crate) fn transport_pos(&self, cx: &App) -> String {
        let st = self.state.read(cx);
        let song = st.song();
        let cursor = self.grid.read(cx).cursor.filter(|c| c.m < song.measures.len()).map(|c| song.flat(c.m, c.k));
        let g = if st.playing() { st.playhead().and_then(|p| p.cell).or(cursor) } else { cursor.or(Some(st.start_cell)) };
        match g.filter(|&g| g < song.total_cells()) {
            Some(g) => {
                let (m, k) = song.unflat(g);
                let sub = song.measures[m].subdivision.max(1);
                format!("c{} · {}.{}", m + 1, k / sub + 1, k % sub + 1)
            }
            None => "—".into(),
        }
    }

    pub(crate) fn topbar(&self, th: &Theme, playing: bool, cx: &mut Context<Self>) -> gpui_kit::Div {
        let pos: SharedString = self.transport_pos(cx).into();
        let st = self.state.read(cx);
        let song = st.song().clone();
        let tp = st.tp.clone();
        let bpm = song.tempo;
        let sounding = st.sounding_bpm;
        let title: SharedString = wadawko_store::display_title(&song).into();
        let chip = |id: &'static str, text: String, a: Box<dyn Action>| abtn_boxed(id, th, a).h(px(22.0)).px(px(6.0)).text_size(px(11.0)).child(SharedString::from(text));

        // ── esquerda: marca, ☰, título ──
        let left = div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .id("logo")
                    // easter egg: 5 cliques no logo tocam "Sakura Sakura" no fue
                    .on_click(cx.listener(|shell: &mut Shell, _, _, cx| shell.views.lane.update(cx, |v, cx| v.logo_click(cx))))
                    .size(px(24.0))
                    .flex_none()
                    .rounded(px(ui::R1))
                    .bg(th.accent)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(th.on_accent)
                    .text_size(px(14.0))
                    .font_family("Shippori Mincho")
                    .font_weight(FontWeight::BOLD)
                    .child("太"),
            )
            .child(icon_btn("hamb", "☰", th, keys::view::ToggleNav))
            .child(
                // clique no título = renomear a música
                abtn("title", th, keys::project::Rename)
                    .border_color(transparent_black())
                    .bg(transparent_black())
                    .h(px(28.0))
                    .px(px(6.0))
                    .min_w_0()
                    .text_size(px(16.0))
                    .font_family("Shippori Mincho")
                    .font_weight(FontWeight::BOLD)
                    .child(div().truncate().child(title)),
            );

        // ── centro: transporte ──
        let play = if playing {
            let acc = th.accent;
            abtn_bare("play", th, keys::transport::TogglePlay)
                .w(px(72.0))
                .px(px(0.0))
                .bg(acc)
                .border_color(acc)
                .text_color(th.on_accent)
                .font_weight(FontWeight::BOLD)
                .hover(move |s| s.bg(Theme::alpha(acc, 0.9)))
                .child("■ parar")
        } else {
            abtn("play", th, keys::transport::TogglePlay).w(px(72.0)).px(px(0.0)).text_color(th.accent).font_weight(FontWeight::BOLD).child("▶ tocar")
        };
        let acc2 = th.accent2;
        let display = div()
            .h(px(26.0))
            .flex_none()
            .px(px(6.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .rounded(px(ui::R1))
            .bg(th.display)
            .border_1()
            .border_color(th.line)
            .child(div().w(px(88.0)).flex_none().flex().justify_center().text_size(px(13.0)).font_weight(FontWeight::BOLD).text_color(th.accent2).child(pos))
            .child(div().w(px(1.0)).h(px(14.0)).flex_none().bg(th.line_strong))
            .child(div().text_size(px(9.0)).text_color(th.ink_faint).child("BPM"))
            .child(
                // roda do mouse = ±1; clique = digitar
                abtn_bare("bpm", th, keys::transport::SetBpm)
                    .w(px(44.0))
                    .h(px(22.0))
                    .px(px(0.0))
                    .bg(transparent_black())
                    .border_color(transparent_black())
                    .text_size(px(13.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(th.accent2)
                    .hover(move |s| s.bg(Theme::alpha(acc2, 0.15)))
                    .child(SharedString::from(format!("{bpm:.0}")))
                    .on_scroll(cx.listener(|this, ev: &ScrollEvent, _, cx| {
                        let d = f32::from(ev.delta.pixel_delta(px(20.0)).y);
                        let b = this.song(cx).tempo + if d > 0.0 { 1.0 } else { -1.0 };
                        this.state.update(cx, |s, cx| s.set_bpm(b, cx));
                        cx.stop_propagation();
                        cx.notify();
                    })),
            )
            .when(playing && (sounding as f64 - bpm).abs() > 0.5, |d| {
                d.child(div().text_size(px(11.0)).font_weight(FontWeight::BOLD).text_color(th.accent).child(SharedString::from(format!("♪ {sounding:.0}"))))
            });
        let center = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(icon_btn("gostart", "⏮", th, keys::transport::GoToStart))
            .child(play)
            .child(atoggle("loop", th, tp.looping, keys::transport::ToggleLoop).w(px(30.0)).px(px(0.0)).child("↺"))
            .child(display)
            .child(segmented("swing", th, vec![("reto", Box::new(keys::transport::Straight)), ("swing", Box::new(keys::transport::SwingOn))], (song.swing() > 0.0) as usize, true));

        // ── direita: metrônomo, count-in, treino (+ chips), tema ──
        let right = div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .justify_end()
            .gap(px(6.0))
            .child(atoggle("metro", th, tp.metronome, keys::transport::ToggleMetronome).w(px(30.0)).px(px(0.0)).child("🎵"))
            .child(atoggle("countin", th, tp.count_in, keys::transport::ToggleCountIn).w(px(30.0)).px(px(0.0)).child("⏱"))
            .child(atoggle("trainer", th, tp.trainer, keys::transport::ToggleTrainer).w(px(30.0)).px(px(0.0)).child("🏃"))
            .when(tp.trainer, |d| {
                d.child(chip("tr-pct", format!("de {:.0}%", tp.trainer_pct * 100.0), Box::new(keys::transport::TrainerPct))).child(chip(
                    "tr-step",
                    format!("+{:.0}%/volta", tp.trainer_step * 100.0),
                    Box::new(keys::transport::TrainerStep),
                ))
            })
            .child(div().w(px(1.0)).h(px(16.0)).flex_none().bg(th.line))
            .child(icon_btn("theme", if th.dark { "☀" } else { "☾" }, th, keys::view::ToggleTheme));

        div().h(px(44.0)).flex_none().w_full().px(px(10.0)).flex().items_center().gap(px(10.0)).bg(th.chrome).border_b_1().border_color(th.line).child(left).child(center).child(right)
    }

    /// Abas grade / partitura / esteira (texto + sublinhado `accent`).
    pub(crate) fn view_tabs(&self, th: &Theme) -> gpui_kit::AnyElement {
        div()
            .flex()
            .items_end()
            .gap(px(2.0))
            .child(ui::tab("tab-grade", "grade", self.tab == Tab::Grade, th, keys::view::Grade))
            .child(ui::tab("tab-partitura", "partitura", self.tab == Tab::Partitura, th, keys::view::Partitura))
            .child(ui::tab("tab-esteira", "esteira", self.tab == Tab::Esteira, th, keys::view::Esteira))
            .into_any_element()
    }
}

/// `abtn` para uma ação já "boxed".
fn abtn_boxed(id: &'static str, th: &Theme, a: Box<dyn Action>) -> gpui_kit::Stateful<gpui_kit::Div> {
    let a2 = a.boxed_clone();
    keys::hintable(
        crate::ui::btn(id, th).on_click(move |_, w, cx| w.dispatch_action(a.boxed_clone(), cx)),
        id,
        std::rc::Rc::new(move |w: &mut gpui_kit::Window, cx: &mut gpui_kit::App| w.dispatch_action(a2.boxed_clone(), cx)),
    )
}
