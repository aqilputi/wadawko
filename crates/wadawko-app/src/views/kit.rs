//! Kit (porte de `src/lib/KitPage.svelte`): instrumentos custom da música
//! (cartão com nome, base, nº de linhas que usam, duplicar, excluir e um editor
//! por zona — couro e borda — com pitch, ganho, decaimento, overtones e filtro
//! de timbre) + a lista dos prontos (builtins/presets) com preview e "editar
//! cópia".
//!
//! Diferenças do web: o nome renomeia pelo prompt do Shell (clique no nome);
//! excluir não pede confirmação — avisa no toast quantas linhas voltaram ao som
//! base e Ctrl+Z desfaz. Os sliders coalescem num passo de desfazer por gesto.

use std::rc::Rc;

use gpui_kit::{App, Context, Entity, FontWeight, Render, SharedString, Window, div, prelude::*, px};
use wadawko_core::model::{InstrumentDef, InstrumentId, InstrumentTone, Song, ToneType, Zone, ZoneParams};
use wadawko_core::ops;
use wadawko_core::ops::kit::ZonePatch;

use crate::keys;
use crate::shell::Shell;
use crate::state::AppState;
use crate::theme::Theme;
use crate::ui;
use crate::views::grid::InputTarget;
use crate::views::widgets::{self, Slider};

gpui_kit::actions!(kit, [New, Rename]);

// ───────────────────────────── lógica pura ─────────────────────────────

/// Valores neutros de uma zona (iguais aos defaults do áudio / web).
pub const PITCH_DEFAULT: f64 = 0.0;
pub const GAIN_DEFAULT: f64 = 0.0;
pub const DECAY_DEFAULT: f64 = 0.5;
pub const OVERTONES_DEFAULT: f64 = 0.0;
/// filtro novo (ao escolher um tipo sem filtro anterior)
pub const CUTOFF_DEFAULT: f64 = 1200.0;
pub const Q_DEFAULT: f64 = 1.0;

/// Opções do seletor de timbre (`None` = sem filtro), na ordem do web.
pub const FILTERS: [(Option<ToneType>, &str); 4] = [(None, "nenhum"), (Some(ToneType::Lowpass), "passa-baixa"), (Some(ToneType::Highpass), "passa-alta"), (Some(ToneType::Bandpass), "passa-banda")];

/// Quantas linhas da música usam a def (`instrumentId`; fallback = id do base,
/// como no web: `(t.instrumentId ?? t.instrument) === defId`).
pub fn used_by(song: &Song, def_id: &str) -> usize {
    song.tracks.iter().filter(|t| t.instrument_id.as_deref().unwrap_or(t.instrument.as_str()) == def_id).count()
}

/// Novo filtro ao trocar o tipo: mantém corte/ressonância do anterior.
pub fn retone(cur: Option<&InstrumentTone>, kind: Option<ToneType>) -> Option<InstrumentTone> {
    kind.map(|kind| InstrumentTone { kind, cutoff: cur.map_or(CUTOFF_DEFAULT, |t| t.cutoff), q: cur.map_or(Q_DEFAULT, |t| t.q) })
}

/// Alvo do renomear: a def custom pedida, se ainda existe; senão a primeira custom.
pub fn rename_target(song: &Song, wanted: Option<&str>) -> Option<(String, String)> {
    let defs = song.instruments();
    let d = wanted.and_then(|id| defs.iter().find(|d| d.id == id)).or_else(|| defs.first())?;
    Some((d.id.clone(), d.name.clone()))
}

fn pct(v: f64) -> String {
    format!("{}%", (v * 100.0).round() as i64)
}

fn zone_title(z: Zone) -> &'static str {
    match z {
        Zone::Head => "COURO",
        Zone::Rim => "BORDA (FUCHI)",
    }
}

fn zone_key(z: Zone) -> &'static str {
    match z {
        Zone::Head => "head",
        Zone::Rim => "rim",
    }
}

/// Preview de uma zona com a def ATUAL da música (reflete a última edição).
fn preview(state: &Entity<AppState>, def_id: &str, zone: Zone, cx: &mut App) {
    state.update(cx, |s, cx| {
        let def = ops::kit::instrument_by_id(s.song(), Some(def_id)).cloned();
        match (def, s.audio.as_mut()) {
            (Some(d), Some(a)) => a.preview_instrument(&d, zone),
            (Some(_), None) => {
                s.flash("áudio ainda carregando (ou sem dispositivo)");
                cx.notify();
            }
            _ => {}
        }
    });
}

// ───────────────────────────── view ─────────────────────────────

pub struct KitView {
    pub state: Entity<AppState>,
    /// base do próximo "+ novo"
    new_base: InstrumentId,
    /// def que a ação `kit::Rename` vai renomear (posta pelo clique no nome)
    rename: Option<String>,
}

impl KitView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        // re-renderiza quando a música/tema/transporte mudam
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        KitView { state, new_base: InstrumentId::Shime, rename: None }
    }

    /// Cria um instrumento custom sobre o base escolhido.
    pub fn add(&mut self, cx: &mut Context<Self>) {
        let base = self.new_base;
        self.state.update(cx, |s, cx| {
            if s.edit(cx, |song| {
                ops::kit::add_instrument(song, base, None);
            }) {
                s.flash(format!("“{} custom” criado — aparece na troca de instrumento das linhas", base.label()));
            }
        });
    }

    pub fn rename_target(&self, cx: &App) -> Option<(String, String)> {
        rename_target(self.state.read(cx).song(), self.rename.as_deref())
    }

    fn play_btn(&self, id: String, label: &'static str, def_id: &str, zone: Zone, th: &Theme) -> gpui_kit::Stateful<gpui_kit::Div> {
        let (st, did) = (self.state.clone(), def_id.to_string());
        let act: widgets::Act = Rc::new(move |_: &mut Window, cx: &mut App| preview(&st, &did, zone, cx));
        let (acc, wash, on) = (th.accent, th.accent_wash, th.on_accent);
        widgets::hbtn_bare(id, th, act)
            .h(px(24.0))
            .px(px(9.0))
            .rounded(px(6.0))
            .text_size(px(11.5))
            .font_weight(FontWeight::BOLD)
            .bg(wash)
            .border_color(acc)
            .text_color(acc)
            .hover(move |s| s.bg(acc).text_color(on))
            .child(label)
    }

    /// Uma linha de controle: rótulo com valor + slider que edita a zona.
    #[allow(clippy::too_many_arguments)]
    fn ctl(
        &self,
        def_id: &str,
        zone: Zone,
        field: &'static str,
        label: &str,
        shown: String,
        value: f64,
        range: (f64, f64, f64),
        default: f64,
        th: &Theme,
        patch: fn(f64) -> ZonePatch,
    ) -> gpui_kit::Div {
        let (st, did) = (self.state.clone(), def_id.to_string());
        let tag = widgets::tag(&format!("kit-{}-{field}", zone_key(zone)), def_id);
        let on_change = Rc::new(move |v: f64, _: &mut Window, cx: &mut App| widgets::edit_co(&st, cx, tag, |song| ops::kit::update_instrument_zone(song, &did, zone, patch(v))));
        div()
            .flex()
            .flex_col()
            .gap(px(1.0))
            .child(
                div()
                    .flex()
                    .gap(px(4.0))
                    .text_size(px(11.0))
                    .text_color(th.ink_soft)
                    .child(SharedString::from(label.to_string()))
                    .child(div().font_weight(FontWeight::BOLD).text_color(th.ink).child(SharedString::from(shown))),
            )
            .child(widgets::slider(
                Slider {
                    id: format!("kit-{}-{field}-{def_id}", zone_key(zone)).into(),
                    value,
                    min: range.0,
                    max: range.1,
                    step: range.2,
                    default: Some(default),
                    vertical: false,
                    len: 0.0,
                    color: th.accent,
                    on_change,
                },
                th,
            ))
    }

    fn zone_editor(&self, def: &InstrumentDef, zone: Zone, th: &Theme) -> gpui_kit::Div {
        let p: ZoneParams = def.zone(zone).cloned().unwrap_or_default();
        let id = def.id.as_str();
        let zk = zone_key(zone);
        let pitch = p.pitch.unwrap_or(PITCH_DEFAULT);
        let gain = p.gain.unwrap_or(GAIN_DEFAULT);
        let decay = p.decay.unwrap_or(DECAY_DEFAULT);
        let over = p.overtones.unwrap_or(OVERTONES_DEFAULT);

        let head = div().flex().items_center().justify_between().child(div().text_size(px(11.5)).font_weight(FontWeight::BOLD).text_color(th.ink).child(zone_title(zone))).child(self.play_btn(
            format!("kit-play-{zk}-{id}"),
            "▶",
            id,
            zone,
            th,
        ));

        // seletor de timbre (chips "um de N")
        let mut chips = div().flex().flex_wrap().gap(px(4.0));
        for (i, (kind, label)) in FILTERS.iter().enumerate() {
            let on = p.tone.as_ref().map(|t| t.kind) == *kind;
            let (st, did, cur, kind) = (self.state.clone(), def.id.clone(), p.tone.clone(), *kind);
            let act: widgets::Act = Rc::new(move |_: &mut Window, cx: &mut App| {
                let tone = retone(cur.as_ref(), kind);
                widgets::edit(&st, cx, |song| ops::kit::update_instrument_zone(song, &did, zone, ZonePatch { tone: Some(tone), ..Default::default() }))
            });
            chips = chips.child(widgets::chip(format!("kit-tone-{zk}-{i}-{id}"), *label, th, on, act));
        }

        let mut col = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(7.0))
            .p(px(9.0))
            .rounded(px(8.0))
            .bg(th.paper_raise)
            .border_1()
            .border_color(th.line)
            .child(head)
            .child(self.ctl(id, zone, "pitch", "pitch", format!("{pitch} st"), pitch, (-24.0, 24.0, 1.0), PITCH_DEFAULT, th, |v| ZonePatch { pitch: Some(v), ..Default::default() }))
            .child(self.ctl(id, zone, "gain", "ganho", format!("{gain} dB"), gain, (-12.0, 12.0, 1.0), GAIN_DEFAULT, th, |v| ZonePatch { gain: Some(v), ..Default::default() }))
            .child(self.ctl(id, zone, "decay", "decaimento", pct(decay), decay, (0.0, 1.0, 0.05), DECAY_DEFAULT, th, |v| ZonePatch { decay: Some(v), ..Default::default() }))
            .child(self.ctl(id, zone, "over", "overtones", pct(over), over, (0.0, 1.0, 0.05), OVERTONES_DEFAULT, th, |v| ZonePatch { overtones: Some(v), ..Default::default() }))
            .child(div().flex().flex_col().gap(px(3.0)).child(div().text_size(px(11.0)).text_color(th.ink_soft).child("timbre")).child(chips));

        if let Some(t) = &p.tone {
            // corte/ressonância mexem no filtro existente (o patch troca o filtro inteiro)
            let (st, did, base) = (self.state.clone(), def.id.clone(), t.clone());
            let on_cut = Rc::new(move |v: f64, _: &mut Window, cx: &mut App| {
                let tone = InstrumentTone { cutoff: v, ..base.clone() };
                widgets::edit_co(&st, cx, widgets::tag(&format!("kit-{zk}-cut"), &did), |song| {
                    ops::kit::update_instrument_zone(song, &did, zone, ZonePatch { tone: Some(Some(tone)), ..Default::default() })
                })
            });
            let (st, did, base) = (self.state.clone(), def.id.clone(), t.clone());
            let on_q = Rc::new(move |v: f64, _: &mut Window, cx: &mut App| {
                let tone = InstrumentTone { q: v, ..base.clone() };
                widgets::edit_co(&st, cx, widgets::tag(&format!("kit-{zk}-q"), &did), |song| {
                    ops::kit::update_instrument_zone(song, &did, zone, ZonePatch { tone: Some(Some(tone)), ..Default::default() })
                })
            });
            let row = |label: &'static str, shown: String, s: Slider| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.0))
                    .child(
                        div()
                            .flex()
                            .gap(px(4.0))
                            .text_size(px(11.0))
                            .text_color(th.ink_soft)
                            .child(label)
                            .child(div().font_weight(FontWeight::BOLD).text_color(th.ink).child(SharedString::from(shown))),
                    )
                    .child(widgets::slider(s, th))
            };
            col = col
                .child(row(
                    "corte",
                    format!("{} Hz", t.cutoff.round() as i64),
                    Slider {
                        id: format!("kit-{zk}-cut-{id}").into(),
                        value: t.cutoff,
                        min: 100.0,
                        max: 8000.0,
                        step: 50.0,
                        default: Some(CUTOFF_DEFAULT),
                        vertical: false,
                        len: 0.0,
                        color: th.accent2,
                        on_change: on_cut,
                    },
                ))
                .child(row(
                    "ressonância",
                    format!("{:.1}", t.q),
                    Slider {
                        id: format!("kit-{zk}-q-{id}").into(),
                        value: t.q,
                        min: 0.1,
                        max: 12.0,
                        step: 0.1,
                        default: Some(Q_DEFAULT),
                        vertical: false,
                        len: 0.0,
                        color: th.accent2,
                        on_change: on_q,
                    },
                ));
        }
        col
    }

    fn card(&self, song: &Song, def: &InstrumentDef, th: &Theme, cx: &mut Context<Self>) -> gpui_kit::Div {
        let n = used_by(song, &def.id);

        let view = cx.entity();
        let did = def.id.clone();
        let on_rename: widgets::Act = Rc::new(move |w: &mut Window, cx: &mut App| {
            view.update(cx, |v, _| v.rename = Some(did.clone()));
            w.dispatch_action(Box::new(Rename), cx);
        });
        let name_id: SharedString = format!("kit-name-{}", def.id).into();
        let name = keys::hintable(
            div()
                .id(name_id.clone())
                .relative()
                .flex_1()
                .min_w_0()
                .px(px(4.0))
                .rounded(px(5.0))
                .text_size(px(14.5))
                .font_weight(FontWeight::BOLD)
                .text_color(th.ink)
                .cursor_pointer()
                .hover(|s| s.bg(th.paper_hover))
                .child(SharedString::from(def.name.clone()))
                .on_click({
                    let a = on_rename.clone();
                    move |_, w, cx| a(w, cx)
                }),
            name_id,
            on_rename,
        );

        let (st, did) = (self.state.clone(), def.id.clone());
        let on_dup: widgets::Act = Rc::new(move |_: &mut Window, cx: &mut App| {
            widgets::edit(&st, cx, |song| {
                ops::kit::duplicate_instrument(song, &did);
            })
        });
        let (st, did, dname) = (self.state.clone(), def.id.clone(), def.name.clone());
        let on_del: widgets::Act = Rc::new(move |_: &mut Window, cx: &mut App| {
            st.update(cx, |s, cx| {
                if s.edit(cx, |song| ops::kit::delete_instrument(song, &did)) {
                    let warn = if n > 0 { format!(" — {n} linha(s) voltaram ao som base") } else { String::new() };
                    s.flash(format!("“{dname}” excluído{warn} (Ctrl+Z desfaz)"));
                }
            })
        });

        let head = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(name)
            .child(div().flex_none().text_size(px(11.0)).text_color(th.ink_soft).child(SharedString::from(format!("base: {}", def.base.label()))))
            .when(n > 0, |d| {
                d.child(
                    div()
                        .flex_none()
                        .px(px(6.0))
                        .rounded(px(8.0))
                        .border_1()
                        .border_color(th.accent2)
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(th.accent2)
                        .child(SharedString::from(format!("{n} linha(s)"))),
                )
            })
            .child(widgets::sbtn(format!("kit-dup-{}", def.id), th, on_dup).child("⧉"))
            .child(widgets::sbtn(format!("kit-del-{}", def.id), th, on_del).child("×"));

        ui::card(th).flex_1().min_w(px(340.0)).max_w(px(560.0)).child(head).child(div().flex().gap(px(10.0)).child(self.zone_editor(def, Zone::Head, th)).child(self.zone_editor(def, Zone::Rim, th)))
    }

    fn builtin_row(&self, def: &InstrumentDef, th: &Theme) -> gpui_kit::Div {
        let (st, did) = (self.state.clone(), def.id.clone());
        let on_dup: widgets::Act = Rc::new(move |_: &mut Window, cx: &mut App| {
            st.update(cx, |s, cx| {
                let mut name = None;
                if s.edit(cx, |song| {
                    if let Some(id) = ops::kit::duplicate_instrument(song, &did) {
                        name = ops::kit::instrument_by_id(song, Some(&id)).map(|d| d.name.clone());
                    }
                }) && let Some(n) = name
                {
                    s.flash(format!("“{n}” criado em Custom — edite à vontade"));
                }
            })
        });
        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(10.0))
            .py(px(6.0))
            .rounded(px(8.0))
            .bg(th.paper)
            .border_1()
            .border_color(th.line)
            .child(div().w(px(150.0)).flex_none().text_size(px(13.0)).font_weight(FontWeight::BOLD).text_color(th.ink).child(SharedString::from(def.name.clone())))
            .child(div().w(px(100.0)).flex_none().text_size(px(11.0)).text_color(th.ink_soft).child(def.base.label()))
            .child(self.play_btn(format!("kit-bplay-head-{}", def.id), "▶ couro", &def.id, Zone::Head, th))
            .child(self.play_btn(format!("kit-bplay-rim-{}", def.id), "▶ borda", &def.id, Zone::Rim, th))
            .child(div().flex_1())
            .child(widgets::sbtn(format!("kit-bdup-{}", def.id), th, on_dup).child("⧉ editar cópia"))
    }
}

impl Render for KitView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (th, song) = {
            let st = self.state.read(cx);
            (st.theme, st.song().clone())
        };
        let note = |text: &'static str| div().text_size(px(12.5)).text_color(th.ink_soft).child(text);
        let h3 = |text: &'static str| div().mt(px(6.0)).text_size(px(11.0)).font_weight(FontWeight::BOLD).text_color(th.ink_soft).child(text);

        // ── cabeçalho: título, explicação, base do novo + "+ novo" ──
        // (empilhado: em janela estreita nada some para fora da tela)
        let mut bases = div().flex().flex_wrap().items_center().gap(px(4.0)).child(div().text_size(px(11.5)).text_color(th.ink_soft).mr(px(4.0)).child("base do novo:"));
        for base in InstrumentId::ALL {
            let view = cx.entity();
            let act: widgets::Act = Rc::new(move |_: &mut Window, cx: &mut App| {
                view.update(cx, |v, cx| {
                    v.new_base = base;
                    cx.notify();
                })
            });
            bases = bases.child(widgets::chip(format!("kit-base-{}", base.as_str()), base.label(), &th, self.new_base == base, act));
        }
        let head = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(widgets::page_title("Kit — instrumentos"))
            .child(
                note("Crie instrumentos a partir de um base e ajuste couro e borda separadamente. Os instrumentos aqui aparecem na troca de instrumento das linhas. Excluir um não mata a linha — ela volta ao som base.")
                    .max_w(px(620.0)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(10.0))
                    .child(ui::abtn_bare("kit-new", &th, New).bg(th.accent).border_color(th.accent).text_color(th.on_accent).font_weight(FontWeight::BOLD).rounded(px(16.0)).hover(|s| s.bg(th.accent)).child("+ novo"))
                    .child(bases),
            );

        let mut page = div().w_full().flex().flex_col().gap(px(12.0)).child(head);

        let custom = song.instruments();
        if !custom.is_empty() {
            let mut cards = div().flex().flex_wrap().items_start().gap(px(14.0));
            for def in custom {
                cards = cards.child(self.card(&song, def, &th, cx));
            }
            page = page.child(h3("CUSTOM")).child(cards);
        }

        let mut list = div().flex().flex_col().gap(px(5.0)).max_w(px(760.0));
        for def in ops::kit::palette_instruments(&song).into_iter().filter(|d| d.is_builtin()) {
            list = list.child(self.builtin_row(def, &th));
        }
        page.child(h3("PRONTOS")).child(list).child(div().text_size(px(11.0)).text_color(th.ink_faint).child(
            "Arraste os sliders (duplo clique volta ao neutro) · Ctrl+Space salta para um controle; num slider, ←/→ ajustam e Esc solta · clique no nome para renomear · ▶ toca uma batida da zona.",
        ))
    }
}

/// Ações desta página: `kit::New` (botão "+ novo" e paleta) e `kit::Rename`
/// (clique no nome → prompt de texto do Shell). As setas dos sliders são
/// tratadas em `widgets::bind` (registrado pelo Mixer).
pub fn bind(d: gpui_kit::Stateful<gpui_kit::Div>, cx: &mut Context<Shell>) -> gpui_kit::Stateful<gpui_kit::Div> {
    d.on_action(cx.listener(|shell: &mut Shell, _: &New, _, cx| shell.views.kit.update(cx, |v, cx| v.add(cx)))).on_action(cx.listener(|shell: &mut Shell, _: &Rename, w, cx| {
        if let Some((id, name)) = shell.views.kit.read(cx).rename_target(cx) {
            shell.ask(InputTarget::InstrumentName(id), name, w, cx);
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wadawko_core::model::Track;

    #[test]
    fn used_by_counts_instrument_id_or_base() {
        let mut s = Song::default();
        let id = ops::kit::add_instrument(&mut s, InstrumentId::Chu, None);
        let mut a = Track::new("a", "A", InstrumentId::Chu);
        a.instrument_id = Some(id.clone());
        s.tracks.push(a);
        s.tracks.push(Track::new("b", "B", InstrumentId::Chu));
        s.tracks.push(Track::new("c", "C", InstrumentId::Shime));
        assert_eq!(used_by(&s, &id), 1);
        // sem instrumentId conta pelo base (builtin "chu")
        assert_eq!(used_by(&s, "chu"), 1);
        assert_eq!(used_by(&s, "shime"), 1);
        assert_eq!(used_by(&s, "odaiko"), 0);
        // excluir devolve a linha ao base
        ops::kit::delete_instrument(&mut s, &id);
        assert_eq!(used_by(&s, &id), 0);
        assert_eq!(used_by(&s, "chu"), 2);
    }

    #[test]
    fn retone_keeps_cutoff_and_q() {
        assert_eq!(retone(None, None), None);
        let t = retone(None, Some(ToneType::Lowpass)).unwrap();
        assert_eq!((t.kind, t.cutoff, t.q), (ToneType::Lowpass, CUTOFF_DEFAULT, Q_DEFAULT));
        let cur = InstrumentTone { kind: ToneType::Lowpass, cutoff: 600.0, q: 3.5 };
        let t = retone(Some(&cur), Some(ToneType::Bandpass)).unwrap();
        assert_eq!((t.kind, t.cutoff, t.q), (ToneType::Bandpass, 600.0, 3.5));
        assert_eq!(retone(Some(&cur), None), None);
    }

    #[test]
    fn zone_edits_go_through_core_and_skip_builtins() {
        let mut s = Song::default();
        let id = ops::kit::add_instrument(&mut s, InstrumentId::Shime, None);
        ops::kit::update_instrument_zone(&mut s, &id, Zone::Rim, ZonePatch { pitch: Some(5.0), ..Default::default() });
        ops::kit::update_instrument_zone(&mut s, &id, Zone::Rim, ZonePatch { tone: Some(retone(None, Some(ToneType::Highpass))), ..Default::default() });
        let d = ops::kit::instrument_by_id(&s, Some(&id)).unwrap();
        let rim = d.rim.as_ref().unwrap();
        assert_eq!(rim.pitch, Some(5.0));
        assert_eq!(rim.tone.as_ref().map(|t| t.kind), Some(ToneType::Highpass));
        // couro intacto (neutro do add_instrument)
        assert_eq!(d.head.as_ref().unwrap().pitch, Some(0.0));
        // builtin não é editável
        let before = s.clone();
        ops::kit::update_instrument_zone(&mut s, "shime", Zone::Head, ZonePatch { pitch: Some(3.0), ..Default::default() });
        assert_eq!(s, before);
    }

    #[test]
    fn rename_target_falls_back_to_first_custom() {
        let mut s = Song::default();
        assert_eq!(rename_target(&s, Some("x")), None);
        let a = ops::kit::add_instrument(&mut s, InstrumentId::Shime, Some("A"));
        let b = ops::kit::add_instrument(&mut s, InstrumentId::Chu, Some("B"));
        assert_eq!(rename_target(&s, Some(&b)), Some((b.clone(), "B".into())));
        ops::kit::delete_instrument(&mut s, &b);
        assert_eq!(rename_target(&s, Some(&b)), Some((a, "A".into())));
        // builtins nunca são alvo
        assert_eq!(rename_target(&Song::default(), Some("shime")), None);
    }

    #[test]
    fn labels() {
        assert_eq!(pct(0.5), "50%");
        assert_eq!(pct(0.05), "5%");
        assert_eq!(FILTERS.len(), 4);
    }
}
