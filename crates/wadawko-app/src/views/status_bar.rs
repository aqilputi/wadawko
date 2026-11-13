//! Status bar: modo (chip clicável liga/desliga o vim) · showcmd · posição do
//! cursor · BPM · tocando · avisos · HUD · estado do banco.

use gpui_kit::{Context, FontWeight, SharedString, div, prelude::*, px};

use super::grid::{Lane, VMode};
use crate::keys;
use crate::shell::Shell;
use crate::song_ext::SongExt;
use crate::theme::Theme;

impl Shell {
    pub(crate) fn status_bar(&self, th: &Theme, playing: bool, play_pos: Option<String>, cx: &mut Context<Self>) -> gpui_kit::Div {
        let st = self.state.read(cx);
        let song = st.song().clone();
        let (vmode, cursor, lane) = {
            let g = self.grid.read(cx);
            (g.vmode, g.cursor, g.lane)
        };
        let pos_text = cursor.filter(|c| c.m < song.measures.len()).map(|c| {
            let secs = song.secs();
            let s = song.section_of(c.m);
            let sub = song.measures[c.m].subdivision.max(1);
            format!(
                "{} · c{} · {}.{} · {}{}",
                secs.get(s).map(|x| x.name.as_str()).unwrap_or(""),
                c.m + 1,
                c.k / sub + 1,
                c.k % sub + 1,
                song.tracks.get(c.t).map(|t| t.name.as_str()).unwrap_or(""),
                if lane == Lane::Kyai { " · kiai" } else { "" }
            )
        });
        let os = cx.platform_info().os;
        let showcmd: String = {
            let mut s = self.count.map(|c| c.to_string()).unwrap_or_default();
            for k in &self.pending {
                if !s.is_empty() {
                    s.push(' ');
                }
                s.push_str(&k.display(os).to_string());
            }
            s
        };
        let (chip_bg, chip_fg) = match vmode {
            VMode::Normal => (th.ink, th.paper),
            VMode::Insert => (th.accent, th.on_accent),
            VMode::Visual | VMode::VLine => (th.accent2, th.on_accent),
            VMode::Simple => (th.paper_raise, th.ink_soft),
        };
        let tocando: SharedString = match (playing, &play_pos) {
            (true, Some(p)) => format!("♪ tocando · {p}").into(),
            (true, None) => "♪ tocando".into(),
            _ => "parado".into(),
        };
        let src = if st.read_only() {
            format!("somente leitura · {}", st.db_label)
        } else if let Some(e) = st.store.autosave_error() {
            format!("ERRO ao gravar: {e}")
        } else if st.store.has_unsaved_changes() {
            format!("gravando… · {}", st.db_label)
        } else {
            format!("gravado · {}", st.db_label)
        };
        let audio = match (&st.audio, &st.audio_error) {
            (Some(_), _) if st.mute => Some("🔇 mudo (--mute)".to_string()),
            (Some(_), _) => None,
            (None, Some(e)) => Some(format!("sem áudio: {e}")),
            (None, None) => Some("carregando samples…".to_string()),
        };
        div()
            .h(px(26.0))
            .flex_none()
            .w_full()
            .px(px(10.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .bg(th.paper)
            .border_t_1()
            .border_color(th.line)
            .text_size(px(11.5))
            .text_color(th.ink_soft)
            .child(keys::hintable(
                div()
                    .id("modechip")
                    .relative()
                    .px(px(8.0))
                    .h(px(18.0))
                    .flex()
                    .items_center()
                    .rounded(px(4.0))
                    .bg(chip_bg)
                    .text_color(chip_fg)
                    .text_size(px(10.5))
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .child(vmode.label())
                    .on_click(|_, w, cx| w.dispatch_action(Box::new(keys::keys::ToggleVim), cx)),
                "modechip",
                keys::dispatch(keys::keys::ToggleVim),
            ))
            .when(!showcmd.is_empty(), |d| d.child(div().text_color(th.accent).font_weight(FontWeight::BOLD).child(SharedString::from(showcmd.clone()))))
            .when_some(pos_text, |d, p| d.child(div().text_color(th.ink_faint).child(SharedString::from(p))))
            .child(div().text_color(th.ink_faint).child("·"))
            .child(SharedString::from(format!("{:.0} BPM", song.tempo)))
            .child(div().text_color(th.ink_faint).child("·"))
            .child(tocando)
            .when_some(audio, |d, t| d.child(div().text_color(th.ink_faint).child(SharedString::from(t))))
            .when_some(st.export_label(), |d, t| d.child(div().px(px(8.0)).rounded(px(4.0)).bg(th.accent_wash).text_color(th.accent).child(SharedString::from(format!("⏳ {t}")))))
            .when_some(st.toast.as_ref().map(|t| t.0.clone()), |d, t| d.child(div().px(px(8.0)).rounded(px(4.0)).bg(th.accent2_wash).text_color(th.accent2).truncate().child(SharedString::from(t))))
            .child(div().flex_1())
            .child(div().text_color(th.ink_faint).child(SharedString::from(self.hud.clone())))
            .child(div().text_color(th.ink_faint).child("·"))
            .child(div().text_color(th.ink_faint).child(SharedString::from(src)))
    }
}
