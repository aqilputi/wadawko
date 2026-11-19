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
use crate::theme::{Theme, parse_color, track_color};
use crate::ui;
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

/// comprimento do fader vertical dos strips (px)
pub const FADER_LEN: f32 = 160.0;

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
        .child(widgets::mini(format!("{prefix}bus-m-{}", p.id), "M", th, p.is_muted(), th.warn, on_mute))
        .child(widgets::mini(format!("{prefix}bus-s-{}", p.id), "S", th, p.is_solo(), th.accent2, on_solo))
}

/// Cartão de grupo: cabeçalho de painel (swatch 10×10 na cor + nome) e corpo.
pub fn group_card(th: &Theme, color: Hsla, title: &str, body: gpui_kit::Div) -> gpui_kit::Div {
    div()
        .flex()
        .flex_col()
        .rounded(px(ui::R0))
        .bg(th.paper)
        .border_1()
        .border_color(th.line)
        .overflow_hidden()
        .child(ui::panel_header(title, th).child(div().absolute().left(px(10.0)).size(px(10.0)).rounded(px(ui::R1)).bg(color)).pl(px(26.0)))
        .child(body.flex_1().min_w_0().p(px(12.0)).flex().flex_col().gap(px(10.0)))
}

/// Escala de dB ao lado do fader (0, −6, −12, −24), alinhada ao trilho do slider.
fn db_scale(th: &Theme, len: f32) -> gpui_kit::Div {
    let mut col = div().relative().flex_none().w(px(18.0)).h(px(len));
    for (db, label) in [(0.0, "0"), (-6.0, "−6"), (-12.0, "−12"), (-24.0, "−24")] {
        let t = widgets::frac(db, VOL_MIN, VOL_MAX) as f32;
        let y = 7.0 + (1.0 - t) * (len - 14.0);
        col = col.child(div().absolute().right(px(0.0)).top(px(y - 6.0)).text_size(px(9.0)).text_color(th.ink_faint).child(label));
    }
    col
}

/// Fader vertical + medidor de 6 px à direita (nível/pico já com balística).
fn fader_with_meter(slider: Slider, level: crate::state::Level, th: &Theme) -> gpui_kit::Div {
    let len = slider.len;
    div().flex().items_start().gap(px(3.0)).child(db_scale(th, len)).child(widgets::slider(slider, th)).child(div().h(px(len)).py(px(7.0)).child(ui::meter(level.level, level.peak, true, th)))
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
    fn strip(&self, song: &Song, t: &Track, ti: usize, person: Option<&Person>, th: &Theme, cx: &Context<Self>) -> gpui_kit::Div {
        let gid = person.map_or("none", |p| p.id.as_str());
        let key = format!("{gid}-{}", t.id);
        let shared = person.is_some() && t.is_shared();
        let audible = strip_audible(song, t, person);
        let vol = t.volume.unwrap_or(Track::DEFAULT_VOLUME_DB);
        let pan = t.pan.unwrap_or(0.0);
        let label = ops::kit::instrument_label(song, t);
        let level = self.state.read(cx).track_level(ti);

        let (st, tid) = (self.state.clone(), t.id.clone());
        let on_vol = Rc::new(move |v: f64, _: &mut Window, cx: &mut App| widgets::edit_co(&st, cx, widgets::tag("trk-vol", &tid), |song| set_track_volume(song, &tid, v)));
        let (st, tid) = (self.state.clone(), t.id.clone());
        let on_pan = Rc::new(move |v: f64, _: &mut Window, cx: &mut App| widgets::edit_co(&st, cx, widgets::tag("trk-pan", &tid), |song| set_track_pan(song, &tid, v)));
        let (st, tid) = (self.state.clone(), t.id.clone());
        let on_mute = Rc::new(move |_: &mut Window, cx: &mut App| widgets::edit(&st, cx, |song| toggle_track_mute(song, &tid)));
        let (st, tid) = (self.state.clone(), t.id.clone());
        let on_solo = Rc::new(move |_: &mut Window, cx: &mut App| widgets::edit(&st, cx, |song| toggle_track_solo(song, &tid)));

        let small = |s: String| div().text_size(px(10.0)).font_weight(FontWeight::BOLD).text_color(th.ink_soft).child(SharedString::from(s));
        // cabeçalho de 22 px preenchido com a cor da linha (F1), como o mixer de uma DAW
        let tcol = track_color(song, t);
        let head = div()
            .w_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .w_full()
                    .h(px(22.0))
                    .px(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(if audible { tcol } else { Theme::alpha(tcol, 0.45) })
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::on_track_color())
                    .child(div().truncate().child(SharedString::from(t.name.clone()))),
            )
            .child(div().w_full().px(px(4.0)).pt(px(4.0)).text_center().text_size(px(10.0)).text_color(th.ink_soft).child(div().truncate().child(SharedString::from(if shared {
                format!("{label} · compart.")
            } else {
                label.to_string()
            }))));

        // controles esmaecem quando a linha não soa; M/S ficam nítidos para dar para desfazer
        let faders = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.0))
            .when(!audible, |d| d.opacity(0.4))
            .child(fader_with_meter(
                Slider {
                    id: format!("mx-vol-{key}").into(),
                    value: vol,
                    min: VOL_MIN,
                    max: VOL_MAX,
                    step: 1.0,
                    default: Some(Track::DEFAULT_VOLUME_DB),
                    vertical: true,
                    len: FADER_LEN,
                    color: th.ink_soft,
                    on_change: on_vol,
                },
                level,
                th,
            ))
            .child(small(db_label(vol)))
            .child(widgets::slider(
                Slider { id: format!("mx-pan-{key}").into(), value: pan, min: -1.0, max: 1.0, step: 0.1, default: Some(0.0), vertical: false, len: 64.0, color: th.accent2, on_change: on_pan },
                th,
            ))
            .child(small(pan_label(pan)));

        div()
            .flex_none()
            .w(px(84.0))
            .flex()
            .flex_col()
            .items_center()
            .rounded(px(ui::R0))
            .bg(th.paper_raise)
            .border_1()
            .border_color(th.line)
            .overflow_hidden()
            .when(shared, |d| d.border_dashed().border_color(person.map_or(th.line_strong, |p| person_color(p, th))))
            .child(head)
            .child(div().px(px(4.0)).pt(px(6.0)).pb(px(10.0)).flex().flex_col().items_center().gap(px(8.0)).child(faders).child(
                div().flex().gap(px(4.0)).child(widgets::mini(format!("mx-m-{key}"), "M", th, t.muted, th.warn, on_mute)).child(widgets::mini(
                    format!("mx-s-{key}"),
                    "S",
                    th,
                    t.solo,
                    th.accent2,
                    on_solo,
                )),
            ))
    }

    /// Strip master (F6): volume de saída, medidor L/R e LED de clip.
    fn master(&self, th: &Theme, cx: &Context<Self>) -> gpui_kit::Div {
        let st = self.state.read(cx);
        let db = st.master_db;
        let (l, r) = (st.levels.master[0], st.levels.master[1]);
        let clip = st.levels.clip();
        let state = self.state.clone();
        let on_vol = Rc::new(move |v: f64, _: &mut Window, cx: &mut App| state.update(cx, |s, cx| s.set_master_db(v, cx)));
        let meters = div().h(px(FADER_LEN)).py(px(7.0)).flex().gap(px(2.0)).child(ui::meter(l.level, l.peak, true, th)).child(ui::meter(r.level, r.peak, true, th));
        let led = div().size(px(8.0)).rounded(px(ui::R1)).border_1().border_color(if clip { th.danger } else { th.line_strong }).bg(if clip { th.danger } else { th.cell_empty });
        div().flex_none().w(px(96.0)).flex().flex_col().rounded(px(ui::R0)).bg(th.paper).border_1().border_color(th.line).overflow_hidden().child(ui::panel_header("Master", th).child(led)).child(
            div()
                .p(px(8.0))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(3.0))
                        .child(db_scale(th, FADER_LEN))
                        .child(widgets::slider(
                            Slider {
                                id: "mx-master".into(),
                                value: db,
                                min: VOL_MIN,
                                max: VOL_MAX,
                                step: 1.0,
                                default: Some(0.0),
                                vertical: true,
                                len: FADER_LEN,
                                color: th.ink_soft,
                                on_change: on_vol,
                            },
                            th,
                        ))
                        .child(meters),
                )
                .child(div().text_size(px(10.0)).font_weight(FontWeight::BOLD).text_color(th.ink_soft).child(SharedString::from(db_label(db))))
                .child(div().text_size(px(9.0)).text_color(th.ink_faint).child(if clip { "clip" } else { "saída" })),
        )
    }
}

impl Render for MixerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (th, song, animating) = {
            let st = self.state.read(cx);
            (st.theme, st.song().clone(), st.playing() || st.levels.active())
        };
        if animating {
            // medidores: o Shell já pede frames enquanto toca; aqui garante durante a queda
            window.request_animation_frame();
        }
        let muted_note = |text: &'static str| div().text_size(px(12.0)).text_color(th.ink_soft).child(text);
        let mut groups = div().flex_1().min_w_0().flex().flex_col().gap(px(14.0));

        for p in song.people() {
            let color = person_color(p, &th);
            let audible = person_audible(&song, p);
            let level = self.state.read(cx).person_level(&p.id);
            let head = div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(8.0))
                .text_color(if audible { th.ink } else { th.ink_faint })
                .child(div().text_size(px(11.0)).text_color(th.ink_faint).child("barramento"))
                .child(bus_controls("mx-", &self.state, p, &th, 130.0))
                .child(div().w(px(130.0)).h(px(3.0)).child(ui::meter(level.level, level.peak, false, &th)));
            let mut strips = div().flex().flex_wrap().gap(px(10.0));
            let mut n = 0;
            for (ti, t) in song.tracks.iter().enumerate().filter(|(_, t)| t.person_ids().contains(&p.id)) {
                strips = strips.child(self.strip(&song, t, ti, Some(p), &th, cx));
                n += 1;
            }
            if n == 0 {
                strips = strips.child(muted_note("Sem instrumentos."));
            }
            groups = groups.child(group_card(&th, color, &p.name, div().child(head).child(strips)));
        }

        let ungrouped: Vec<(usize, &Track)> = song.tracks.iter().enumerate().filter(|(_, t)| t.person_ids().is_empty()).collect();
        if !ungrouped.is_empty() {
            let mut strips = div().flex().flex_wrap().gap(px(10.0));
            for (ti, t) in ungrouped {
                strips = strips.child(self.strip(&song, t, ti, None, &th, cx));
            }
            groups = groups.child(group_card(&th, th.line_strong, "Sem pessoa", div().child(strips)));
        }
        if song.tracks.is_empty() {
            groups = groups.child(muted_note("Sem faixas. Adicione uma linha no Editor."));
        }
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(widgets::page_title("Mixer"))
            .child(div().flex().items_start().gap(px(14.0)).child(groups).child(self.master(&th, cx)))
            .child(div().text_size(px(11.0)).text_color(th.ink_faint).child(
                "Arraste os faders (duplo clique volta ao padrão) · Ctrl+Space salta para um controle; num slider, ←/→ ajustam e Esc solta · M = mudo, S = solo (linha e pessoa são camadas separadas) · medidores: verde até −18 dB, âmbar até −6, vermelho até 0; a linha é o pico retido.",
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
