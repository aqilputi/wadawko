//! Status bar: modo (chip clicável liga/desliga o vim) · showcmd · posição do
//! cursor · BPM · tocando · avisos · HUD · estado do banco.

use gpui_kit::{Context, FontWeight, SharedString, div, prelude::*, px};

use super::grid::{Lane, VMode};
use crate::keys;
use crate::shell::Shell;
use crate::song_ext::SongExt;
use crate::theme::Theme;
use crate::ui;

/// Tudo o que a status bar mostra, já como texto: o Shell calcula a cada frame e
/// só redesenha a barra (view em cache) quando o hash muda.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct StatusModel {
    vmode: Option<VMode>,
    showcmd: String,
    pos_text: Option<String>,
    bpm: String,
    tocando: String,
    /// trecho de loop (`↺ c3–c6`)
    loop_text: Option<String>,
    /// modo "ensaiar como…" ligado (`👤 sem Ana`)
    rehearse: Option<String>,
    audio: Option<String>,
    export: Option<String>,
    toast: Option<String>,
    hud: String,
    src: String,
}

impl StatusModel {
    /// "Seção 1 · c3 · 2.1 · Shime" do cursor da grade (o painel direito reaproveita).
    pub fn pos_text(&self) -> Option<&str> {
        self.pos_text.as_deref()
    }
}

impl Shell {
    pub(crate) fn status_model(&self, playing: bool, play_pos: Option<String>, cx: &mut Context<Self>) -> StatusModel {
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
        let tocando = match (playing, &play_pos) {
            (true, Some(p)) => format!("♪ tocando · {p}"),
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
        StatusModel {
            vmode: Some(vmode),
            showcmd,
            pos_text,
            bpm: format!("{:.0} BPM", song.tempo),
            tocando,
            loop_text: st.tp.range.map(|r| format!("↺ {}", crate::state::range_label(r))),
            rehearse: st.tp.rehearse.as_ref().map(|r| format!("👤 {} {}", r.listen.label(), song.person(&r.person).map_or(r.person.as_str(), |p| p.name.as_str()))),
            audio,
            export: st.export_label(),
            toast: st.toast.as_ref().map(|t| t.0.clone()),
            hud: self.hud.clone(),
            src,
        }
    }

    /// Status bar (h 22, `chrome`): segmentos separados por divisores de 1 px.
    pub(crate) fn status_bar(&self, th: &Theme, m: &StatusModel) -> gpui_kit::Div {
        let StatusModel { vmode, showcmd, pos_text, bpm, tocando, loop_text, rehearse, audio, export, toast, hud, src } = m.clone();
        let vmode = vmode.unwrap_or(VMode::Simple);
        let (chip_bg, chip_fg) = match vmode {
            VMode::Normal => (th.ink, th.chrome),
            VMode::Insert => (th.accent, th.on_accent),
            VMode::Visual | VMode::VLine => (th.accent2, th.on_accent),
            VMode::Simple => (th.paper_raise, th.ink_soft),
        };
        let sep = || div().w(px(1.0)).h(px(12.0)).flex_none().bg(th.line);
        let chip = |bg, fg| div().px(px(6.0)).h(px(16.0)).flex().items_center().rounded(px(ui::R1)).bg(bg).text_color(fg).text_size(px(10.0)).font_weight(FontWeight::BOLD);
        div()
            .h(px(22.0))
            .flex_none()
            .w_full()
            .px(px(8.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .bg(th.chrome)
            .border_t_1()
            .border_color(th.line)
            .text_size(px(11.0))
            .text_color(th.ink_soft)
            .child(keys::hintable(
                chip(chip_bg, chip_fg).id("modechip").relative().cursor_pointer().child(vmode.label()).on_click(|_, w, cx| w.dispatch_action(Box::new(keys::keys::ToggleVim), cx)),
                "modechip",
                keys::dispatch(keys::keys::ToggleVim),
            ))
            .when(!showcmd.is_empty(), |d| d.child(div().text_color(th.accent).font_weight(FontWeight::BOLD).child(SharedString::from(showcmd.clone()))))
            .when_some(pos_text, |d, p| d.child(sep()).child(div().text_color(th.ink_faint).child(SharedString::from(p))))
            .child(sep())
            .child(SharedString::from(bpm))
            .child(sep())
            .child(SharedString::from(tocando))
            .when_some(loop_text, |d, t| d.child(sep()).child(div().text_color(th.accent2).font_weight(FontWeight::BOLD).child(SharedString::from(t))))
            .when_some(rehearse, |d, t| d.child(sep()).child(chip(Theme::alpha(th.warn, 0.2), th.warn).child(SharedString::from(t))))
            .when_some(audio, |d, t| d.child(sep()).child(div().text_color(th.ink_faint).child(SharedString::from(t))))
            .when_some(export, |d, t| d.child(chip(th.accent_wash, th.accent).child(SharedString::from(format!("⏳ {t}")))))
            .when_some(toast, |d, t| d.child(chip(th.accent2_wash, th.accent2).font_weight(FontWeight::MEDIUM).truncate().child(SharedString::from(t))))
            .child(div().flex_1())
            .child(div().text_color(th.ink_faint).child(SharedString::from(hud)))
            .child(sep())
            .child(div().text_color(if src.starts_with("ERRO") { th.danger } else { th.ink_faint }).child(SharedString::from(src)))
    }
}
