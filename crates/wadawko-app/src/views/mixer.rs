//! Mixer (porte de `src/lib/MixerPage.svelte`): um grupo por pessoa com o
//! barramento dela (volume, mudo, solo) e um strip por linha (fader em dB, pan,
//! mudo/solo). Linhas sem pessoa ficam no grupo "Sem pessoa"; linha
//! compartilhada aparece no grupo de cada pessoa (borda tracejada).
//!
//! Mudo/solo têm 2 camadas, como no áudio (`apply_mix`): a da linha
//! (`track.muted/solo`) e a da pessoa (`person.muted/solo`). Um strip fica
//! esmaecido quando não soa por qualquer uma das duas.
//!
//! Toda mudança passa por `AppState::edit`/`edit_coalesced` (desfazer, autosave
//! e `apply_mix` ao vivo); os sliders coalescem num passo de desfazer por gesto.

use std::rc::Rc;

use gpui_kit::{App, Context, Entity, FontWeight, Hsla, Render, SharedString, Window, div, prelude::*, px};
use wadawko_core::model::{Person, Song, Track};
use wadawko_core::ops;

use crate::shell::Shell;
use crate::state::AppState;
use crate::theme::{Theme, parse_color};
use crate::views::widgets::{self, Slider};

pub const VOL_MIN: f64 = -40.0;
pub const VOL_MAX: f64 = 6.0;
/// volume do barramento da pessoa quando ausente (dB)
pub const BUS_DEFAULT_DB: f64 = 0.0;

// ───────────────────────────── lógica pura ─────────────────────────────

/// Regra de uma camada de mudo/solo: mudo cala; havendo algum solo na camada,
/// só os solados soam.
pub fn layer_audible(muted: bool, solo: bool, any_solo: bool) -> bool {
    !muted && (!any_solo || solo)
}

/// 1ª camada: a linha soa pelo mudo/solo de LINHAS?
pub fn line_audible(song: &Song, track: &Track) -> bool {
    layer_audible(track.muted, track.solo, song.tracks.iter().any(|t| t.solo))
}

/// 2ª camada: a pessoa soa pelo mudo/solo de PESSOAS?
pub fn person_audible(song: &Song, person: &Person) -> bool {
    layer_audible(person.is_muted(), person.is_solo(), song.people().iter().any(Person::is_solo))
}

/// Mudo/solo efetivo de um strip: a linha dentro do grupo de `person` soa se
/// passar nas duas camadas. Sem pessoa (`None`) só vale a camada da linha — o
/// barramento "sem pessoa" não é calado pelo solo de pessoas.
pub fn strip_audible(song: &Song, track: &Track, person: Option<&Person>) -> bool {
    line_audible(song, track) && person.is_none_or(|p| person_audible(song, p))
}

/// Rótulo do pan: `C`, `E50` (esquerda) ou `D50` (direita).
pub fn pan_label(pan: f64) -> String {
    let pct = (pan.abs() * 100.0).round() as i64;
    if pct == 0 {
        "C".into()
    } else if pan < 0.0 {
        format!("E{pct}")
    } else {
        format!("D{pct}")
    }
}

/// Rótulo de volume em dB inteiros (sem "-0").
pub fn db_label(db: f64) -> String {
    format!("{:.0} dB", db.round() + 0.0)
}

fn person_mut<'a>(song: &'a mut Song, id: &str) -> Option<&'a mut Person> {
    song.people.as_mut()?.iter_mut().find(|p| p.id == id)
}

// edições (faltam em `ops::tracks`/`ops::people`; ficam aqui, sem tocar no core)

pub fn set_track_volume(song: &mut Song, id: &str, db: f64) {
    if let Some(t) = song.track_mut(id) {
        t.volume = Some(db);
    }
}

pub fn set_track_pan(song: &mut Song, id: &str, pan: f64) {
    if let Some(t) = song.track_mut(id) {
        t.pan = Some(pan);
    }
}

pub fn toggle_track_mute(song: &mut Song, id: &str) {
    if let Some(t) = song.track_mut(id) {
        t.muted = !t.muted;
    }
}

pub fn toggle_track_solo(song: &mut Song, id: &str) {
    if let Some(t) = song.track_mut(id) {
        t.solo = !t.solo;
    }
}

pub fn set_person_volume(song: &mut Song, id: &str, db: f64) {
    if let Some(p) = person_mut(song, id) {
        p.volume = Some(db);
    }
}

pub fn toggle_person_mute(song: &mut Song, id: &str) {
    if let Some(p) = person_mut(song, id) {
        p.muted = Some(!p.is_muted());
    }
}

pub fn toggle_person_solo(song: &mut Song, id: &str) {
    if let Some(p) = person_mut(song, id) {
        p.solo = Some(!p.is_solo());
    }
}

// ───────────────────────────── peças ─────────────────────────────

pub fn person_color(p: &Person, th: &Theme) -> Hsla {
    p.color.as_deref().and_then(parse_color).unwrap_or(th.accent)
}

/// Controles do barramento de uma pessoa: volume (dB) + mudo + solo. Usado no
/// cabeçalho do grupo do Mixer e no cartão da página Pessoas (`prefix` = página).
pub fn bus_controls(prefix: &'static str, state: &Entity<AppState>, p: &Person, th: &Theme, len: f32) -> gpui_kit::Div {
    let color = person_color(p, th);
    let vol = p.volume.unwrap_or(BUS_DEFAULT_DB);
    let (st, pid) = (state.clone(), p.id.clone());
    let on_vol = Rc::new(move |v: f64, _: &mut Window, cx: &mut App| widgets::edit_co(&st, cx, widgets::tag("bus-vol", &pid), |song| set_person_volume(song, &pid, v)));
    let (st, pid) = (state.clone(), p.id.clone());
    let on_mute = Rc::new(move |_: &mut Window, cx: &mut App| widgets::edit(&st, cx, |song| toggle_person_mute(song, &pid)));
    let (st, pid) = (state.clone(), p.id.clone());
    let on_solo = Rc::new(move |_: &mut Window, cx: &mut App| widgets::edit(&st, cx, |song| toggle_person_solo(song, &pid)));
    div()
        .flex()
        .items_center()
        .gap(px(7.0))
        .child(widgets::slider(
            Slider { id: format!("{prefix}bus-vol-{}", p.id).into(), value: vol, min: VOL_MIN, max: VOL_MAX, step: 1.0, default: Some(BUS_DEFAULT_DB), vertical: false, len, color, on_change: on_vol },
            th,
        ))
        .child(div().w(px(42.0)).text_size(px(10.5)).font_weight(FontWeight::BOLD).text_color(th.ink_soft).child(SharedString::from(db_label(vol))))
        .child(widgets::mini(format!("{prefix}bus-m-{}", p.id), "M", th, p.is_muted(), th.accent, on_mute))
        .child(widgets::mini(format!("{prefix}bus-s-{}", p.id), "S", th, p.is_solo(), th.accent2, on_solo))
}

/// Cartão de grupo com a faixa colorida da pessoa à esquerda.
pub fn group_card(th: &Theme, color: Hsla, body: gpui_kit::Div) -> gpui_kit::Div {
    div()
        .flex()
        .rounded(px(10.0))
        .bg(th.paper)
        .border_1()
        .border_color(th.line)
        .overflow_hidden()
        .child(div().flex_none().w(px(4.0)).bg(color))
        .child(body.flex_1().min_w_0().p(px(12.0)).flex().flex_col().gap(px(10.0)))
}

pub struct MixerView {
    pub state: Entity<AppState>,
}

impl MixerView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        // re-renderiza quando a música/tema/transporte mudam
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        MixerView { state }
    }

    /// Strip de uma linha dentro do grupo `gid` (id da pessoa ou "none").
    fn strip(&self, song: &Song, t: &Track, person: Option<&Person>, th: &Theme) -> gpui_kit::Div {
        let gid = person.map_or("none", |p| p.id.as_str());
        let key = format!("{gid}-{}", t.id);
        let shared = person.is_some() && t.is_shared();
        let audible = strip_audible(song, t, person);
        let vol = t.volume.unwrap_or(Track::DEFAULT_VOLUME_DB);
        let pan = t.pan.unwrap_or(0.0);
        let label = ops::kit::instrument_label(song, t);

        let (st, tid) = (self.state.clone(), t.id.clone());
        let on_vol = Rc::new(move |v: f64, _: &mut Window, cx: &mut App| widgets::edit_co(&st, cx, widgets::tag("trk-vol", &tid), |song| set_track_volume(song, &tid, v)));
        let (st, tid) = (self.state.clone(), t.id.clone());
        let on_pan = Rc::new(move |v: f64, _: &mut Window, cx: &mut App| widgets::edit_co(&st, cx, widgets::tag("trk-pan", &tid), |song| set_track_pan(song, &tid, v)));
        let (st, tid) = (self.state.clone(), t.id.clone());
        let on_mute = Rc::new(move |_: &mut Window, cx: &mut App| widgets::edit(&st, cx, |song| toggle_track_mute(song, &tid)));
        let (st, tid) = (self.state.clone(), t.id.clone());
        let on_solo = Rc::new(move |_: &mut Window, cx: &mut App| widgets::edit(&st, cx, |song| toggle_track_solo(song, &tid)));

        let small = |s: String| div().text_size(px(10.5)).font_weight(FontWeight::BOLD).text_color(th.ink_soft).child(SharedString::from(s));
        let head = div()
            .w_full()
            .min_h(px(40.0))
            .flex()
            .flex_col()
            .items_center()
            .text_center()
            .child(div().text_size(px(12.5)).font_weight(FontWeight::BOLD).text_color(th.ink).child(SharedString::from(t.name.clone())))
            .child(div().text_size(px(10.0)).text_color(th.ink_soft).child(SharedString::from(if shared { format!("{label} · compart.") } else { label.to_string() })));

        // controles esmaecem quando a linha não soa; M/S ficam nítidos para dar para desfazer
        let faders = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.0))
            .when(!audible, |d| d.opacity(0.4))
            .child(head)
            .child(widgets::slider(
                Slider {
                    id: format!("mx-vol-{key}").into(),
                    value: vol,
                    min: VOL_MIN,
                    max: VOL_MAX,
                    step: 1.0,
                    default: Some(Track::DEFAULT_VOLUME_DB),
                    vertical: true,
                    len: 128.0,
                    color: th.accent,
                    on_change: on_vol,
                },
                th,
            ))
            .child(small(db_label(vol)))
            .child(widgets::slider(
                Slider { id: format!("mx-pan-{key}").into(), value: pan, min: -1.0, max: 1.0, step: 0.1, default: Some(0.0), vertical: false, len: 80.0, color: th.accent2, on_change: on_pan },
                th,
            ))
            .child(small(pan_label(pan)));

        div()
            .flex_none()
            .w(px(104.0))
            .px(px(8.0))
            .py(px(12.0))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(10.0))
            .rounded(px(10.0))
            .bg(th.paper_raise)
            .border_1()
            .border_color(th.line)
            .when(shared, |d| d.border_dashed().border_color(person.map_or(th.line_strong, |p| person_color(p, th))))
            .child(faders)
            .child(
                div()
                    .flex()
                    .gap(px(5.0))
                    .child(widgets::mini(format!("mx-m-{key}"), "M", th, t.muted, th.accent, on_mute))
                    .child(widgets::mini(format!("mx-s-{key}"), "S", th, t.solo, th.accent2, on_solo)),
            )
    }
}

impl Render for MixerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (th, song) = {
            let st = self.state.read(cx);
            (st.theme, st.song().clone())
        };
        let muted_note = |text: &'static str| div().text_size(px(12.0)).text_color(th.ink_soft).child(text);
        let mut page = div().w_full().flex().flex_col().gap(px(14.0)).child(widgets::page_title("Mixer"));

        for p in song.people() {
            let color = person_color(p, &th);
            let audible = person_audible(&song, p);
            let head = div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(8.0))
                .child(div().w(px(14.0)).h(px(14.0)).rounded(px(3.0)).bg(color).border_1().border_color(th.line_strong))
                .child(div().text_size(px(14.5)).font_weight(FontWeight::BOLD).text_color(if audible { th.ink } else { th.ink_faint }).child(SharedString::from(p.name.clone())))
                .child(div().w(px(1.0)).h(px(20.0)).mx(px(6.0)).bg(th.line))
                .child(bus_controls("mx-", &self.state, p, &th, 130.0));
            let mut strips = div().flex().flex_wrap().gap(px(12.0));
            let mut n = 0;
            for t in song.tracks.iter().filter(|t| t.person_ids().contains(&p.id)) {
                strips = strips.child(self.strip(&song, t, Some(p), &th));
                n += 1;
            }
            if n == 0 {
                strips = strips.child(muted_note("Sem instrumentos."));
            }
            page = page.child(group_card(&th, color, div().child(head).child(strips)));
        }

        let ungrouped: Vec<&Track> = song.tracks.iter().filter(|t| t.person_ids().is_empty()).collect();
        if !ungrouped.is_empty() {
            let mut strips = div().flex().flex_wrap().gap(px(12.0));
            for t in ungrouped {
                strips = strips.child(self.strip(&song, t, None, &th));
            }
            let head = div().text_size(px(14.5)).font_weight(FontWeight::BOLD).text_color(th.ink).child("Sem pessoa");
            page = page.child(group_card(&th, th.line_strong, div().child(head).child(strips)));
        }
        if song.tracks.is_empty() {
            page = page.child(muted_note("Sem faixas. Adicione uma linha no Editor."));
        }
        page.child(div().text_size(px(11.0)).text_color(th.ink_faint).child(
            "Arraste os sliders (duplo clique volta ao padrão) · Ctrl+Space salta para um controle; num slider, ←/→ ajustam e Esc solta · M = mudo, S = solo (linha e pessoa são camadas separadas).",
        ))
    }
}

/// Handlers de ação: as setas que ajustam o slider em foco (valem para as três
/// páginas — Mixer, Pessoas e Kit — e são registradas uma vez, aqui).
pub fn bind(d: gpui_kit::Stateful<gpui_kit::Div>, cx: &mut Context<Shell>) -> gpui_kit::Stateful<gpui_kit::Div> {
    widgets::bind(d, cx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wadawko_core::model::InstrumentId;

    fn song() -> Song {
        let mut s = Song::default();
        for (id, people) in [("a", vec!["p1"]), ("b", vec!["p2"]), ("c", vec!["p1", "p2"]), ("d", vec![])] {
            let mut t = Track::new(id, id, InstrumentId::Shime);
            if !people.is_empty() {
                t.person_ids = Some(people.into_iter().map(String::from).collect());
            }
            s.tracks.push(t);
        }
        s.people = Some(vec![Person { id: "p1".into(), name: "Ana".into(), ..Default::default() }, Person { id: "p2".into(), name: "Bia".into(), ..Default::default() }]);
        s
    }

    fn audible(s: &Song, track: &str, person: Option<&str>) -> bool {
        strip_audible(s, s.track(track).unwrap(), person.map(|p| s.person(p).unwrap()))
    }

    #[test]
    fn layer_rule() {
        assert!(layer_audible(false, false, false));
        assert!(!layer_audible(true, false, false));
        assert!(!layer_audible(false, false, true));
        assert!(layer_audible(false, true, true));
        // mudo ganha do solo
        assert!(!layer_audible(true, true, true));
    }

    #[test]
    fn line_layer() {
        let mut s = song();
        assert!(["a", "b", "c", "d"].iter().all(|t| audible(&s, t, None)));
        toggle_track_mute(&mut s, "a");
        assert!(!audible(&s, "a", Some("p1")));
        assert!(audible(&s, "b", Some("p2")));
        toggle_track_solo(&mut s, "b");
        assert!(audible(&s, "b", Some("p2")));
        assert!(!audible(&s, "c", Some("p1")));
        assert!(!audible(&s, "d", None));
        // tirar o solo devolve as outras (menos a muda)
        toggle_track_solo(&mut s, "b");
        assert!(audible(&s, "c", Some("p1")) && audible(&s, "d", None) && !audible(&s, "a", Some("p1")));
    }

    #[test]
    fn person_layer_combines_with_line_layer() {
        let mut s = song();
        toggle_person_mute(&mut s, "p1");
        assert!(!audible(&s, "a", Some("p1")));
        // linha compartilhada: calada no grupo da Ana, soando no da Bia
        assert!(!audible(&s, "c", Some("p1")));
        assert!(audible(&s, "c", Some("p2")));
        toggle_person_mute(&mut s, "p1");
        assert_eq!(s.person("p1").unwrap().muted, Some(false));

        toggle_person_solo(&mut s, "p2");
        assert!(!audible(&s, "a", Some("p1")));
        assert!(audible(&s, "b", Some("p2")));
        // sem pessoa não entra no solo de pessoas
        assert!(audible(&s, "d", None));
        // as duas camadas valem juntas: pessoa solada, mas a linha dela muda
        toggle_track_mute(&mut s, "b");
        assert!(!audible(&s, "b", Some("p2")));
        // bate com o que o core usa para o áudio/visual
        for p in s.people() {
            assert_eq!(person_audible(&s, p), ops::people::person_audible(&s, Some(&p.id)));
        }
    }

    #[test]
    fn setters_edit_only_the_target() {
        let mut s = song();
        set_track_volume(&mut s, "a", -12.0);
        set_track_pan(&mut s, "a", -0.5);
        set_person_volume(&mut s, "p2", 3.0);
        set_track_volume(&mut s, "nope", 1.0);
        set_person_volume(&mut s, "nope", 1.0);
        assert_eq!((s.track("a").unwrap().volume, s.track("a").unwrap().pan), (Some(-12.0), Some(-0.5)));
        assert_eq!(s.track("b").unwrap().volume, None);
        assert_eq!((s.person("p1").unwrap().volume, s.person("p2").unwrap().volume), (None, Some(3.0)));
    }

    #[test]
    fn labels() {
        assert_eq!(pan_label(0.0), "C");
        assert_eq!(pan_label(-0.5), "E50");
        assert_eq!(pan_label(1.0), "D100");
        assert_eq!(pan_label(0.1), "D10");
        assert_eq!(db_label(-6.0), "-6 dB");
        assert_eq!(db_label(-0.2), "0 dB");
        assert_eq!(db_label(6.0), "6 dB");
    }
}
