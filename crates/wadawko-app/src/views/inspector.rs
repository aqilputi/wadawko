//! Inspector contextual (F4): propriedades da SEÇÃO, do COMPASSO, da LINHA e da
//! CÉLULA sob o cursor da grade, editáveis sem popover — como o inspector das
//! DAWs, um painel único que segue a seleção. Vive no painel direito, que é uma
//! view em cache: tudo que ele lê entra na chave do `Panel` em
//! `Shell::refresh_chrome`. Os botões despacham as mesmas ações da paleta e dos
//! atalhos (`abtn`); o que não tem ação (dinâmica direta, M/S/★, pessoas, cor)
//! edita pelo caminho único `AppState::edit`.

use std::rc::Rc;

use gpui_kit::{App, ClickEvent, Context, FontWeight, SharedString, Window, div, prelude::*, px};
use wadawko_core::vocab::Hand;
use wadawko_core::{Dynamic, Hairpin, Zone, ops, phrase};

use crate::keys;
use crate::shell::Shell;
use crate::song_ext::SongExt;
use crate::theme::{Theme, parse_color, track_color};
use crate::ui::{self, small_btn_on};
use crate::views::grid::Lane;
use crate::views::mixer::{self, VOL_MAX, VOL_MIN};
use crate::views::widgets::{self, Slider};

/// Operação sobre o Shell disparada por um chip do inspector.
type ShellOp = Rc<dyn Fn(&mut Shell, &mut Context<Shell>)>;

/// Chave do inspector para o cache do painel: tudo que ele mostra.
#[derive(Hash)]
pub struct InspectorKey {
    pub cursor: Option<(usize, usize, usize)>,
    pub lane_kyai: bool,
    pub anchor: Option<(usize, usize, usize)>,
    pub dynr: bool,
    pub kyai: bool,
}

impl Shell {
    pub(crate) fn inspector_key(&self, cx: &App) -> InspectorKey {
        let g = self.grid.read(cx);
        let t = g.cursor.map(|c| c.t).unwrap_or(0);
        let rows = g.rows();
        InspectorKey {
            cursor: g.cursor.map(|c| (c.m, c.t, c.k)),
            lane_kyai: g.lane == Lane::Kyai,
            anchor: g.anchor.map(|c| (c.m, c.t, c.k)),
            dynr: rows.dynr.get(t).copied().unwrap_or(false),
            kyai: rows.kyai.get(t).copied().unwrap_or(false),
        }
    }

    pub(crate) fn inspector(&self, th: &Theme, cx: &mut Context<Self>) -> gpui_kit::Div {
        let st = self.state.read(cx);
        let song = st.song().clone();
        let selected = st.tp.selected.clone();
        let state = self.state.clone();
        let (cursor, lane, dynr, kyai) = {
            let g = self.grid.read(cx);
            let rows = g.rows();
            let t = g.cursor.map(|c| c.t).unwrap_or(0);
            (g.cursor, g.lane, rows.dynr.get(t).copied().unwrap_or(false), rows.kyai.get(t).copied().unwrap_or(false))
        };
        let weak = cx.entity().downgrade();
        let th2 = *th;

        // ── peças ──
        let sub = |title: &'static str| div().h(px(18.0)).mt(px(4.0)).flex().items_center().text_size(px(10.0)).font_weight(FontWeight::BOLD).text_color(th2.ink_faint).child(title);
        let lbl = |s: &'static str| div().w(px(50.0)).flex_none().text_size(px(10.5)).text_color(th2.ink_faint).child(s);
        let row = || div().flex().items_center().flex_wrap().gap(px(4.0));
        let soft = |s: String| div().text_size(px(11.0)).text_color(th2.ink_soft).truncate().child(SharedString::from(s));
        let bold = |s: String| div().flex_1().min_w_0().text_size(px(12.0)).font_weight(FontWeight::BOLD).text_color(th2.ink).truncate().child(SharedString::from(s));
        let ibtn = |id: &'static str, label: &'static str, a: Box<dyn gpui_kit::Action>| {
            let a2 = a.boxed_clone();
            keys::hintable(
                ui::btn(id, &th2).h(px(20.0)).px(px(6.0)).text_size(px(11.0)).child(label).on_click(move |_: &ClickEvent, w, cx| w.dispatch_action(a.boxed_clone(), cx)),
                id,
                Rc::new(move |w: &mut Window, cx: &mut App| w.dispatch_action(a2.boxed_clone(), cx)),
            )
        };
        let val = |s: String| div().min_w(px(30.0)).flex().justify_center().text_size(px(11.5)).font_weight(FontWeight::BOLD).text_color(th2.ink).child(SharedString::from(s));
        // chip que edita direto (sem ação): on/off + closure com o Shell
        let chip = |id: String, label: String, on: bool, f: ShellOp| {
            let w1 = weak.clone();
            let w2 = weak.clone();
            let f2 = f.clone();
            let act: keys::HintAct = Rc::new(move |_: &mut Window, cx: &mut App| {
                let _ = w2.update(cx, |sh, cx| f2(sh, cx));
            });
            widgets::chip(id.clone(), label, &th2, on, act).h(px(20.0)).px(px(5.0)).text_size(px(11.0)).on_click(move |_: &ClickEvent, _, cx: &mut App| {
                let _ = w1.update(cx, |sh, cx| f(sh, cx));
            })
        };

        // chip ligado/desligado que despacha uma ação
        let tchip = |id: &'static str, label: &'static str, on: bool, a: Box<dyn gpui_kit::Action>| {
            let a2 = a.boxed_clone();
            widgets::chip(id, label, &th2, on, Rc::new(move |w: &mut Window, cx: &mut App| w.dispatch_action(a2.boxed_clone(), cx)))
                .h(px(20.0))
                .px(px(6.0))
                .text_size(px(11.0))
                .on_click(move |_: &ClickEvent, w, cx| w.dispatch_action(a.boxed_clone(), cx))
        };

        let mut body = div().flex().flex_col().gap(px(4.0)).px(px(10.0)).pb(px(8.0));
        let Some(c) = cursor.filter(|c| c.m < song.measures.len() && c.t < song.tracks.len()) else {
            return body.child(soft("sem cursor na grade".into()));
        };

        // ── SEÇÃO ──
        let s = song.section_of(c.m);
        let secs = song.secs();
        body = body.child(sub("SEÇÃO"));
        match secs.get(s) {
            Some(sec) if sec.id.is_some() => {
                let is_sel = sec.id.as_ref().is_some_and(|id| selected.contains(id));
                body = body
                    .child(row().child(bold(sec.name.clone())).child(ibtn("ins-sec-ren", "✎", Box::new(keys::section::Rename))))
                    .child(
                        row()
                            .child(lbl("toca"))
                            .child(ibtn("ins-rep-", "−", Box::new(keys::section::RepeatDown)))
                            .child(val(format!("{}×", sec.repeat.max(1))))
                            .child(ibtn("ins-rep+", "+", Box::new(keys::section::RepeatUp)))
                            .child(div().w(px(8.0)))
                            .child(ibtn("ins-m-", "−", Box::new(keys::section::MeasureRemove)))
                            .child(val(format!("{}c", sec.end.saturating_sub(sec.start))))
                            .child(ibtn("ins-m+", "+", Box::new(keys::section::MeasureAdd))),
                    )
                    .child(row().child(ibtn("ins-sec-clone", "⧉ clonar", Box::new(keys::section::Clone))).child(ibtn("ins-sec-rm", "✕ remover", Box::new(keys::section::Remove))).child(ibtn(
                        "ins-sec-only",
                        if is_sel { "▶ só esta ✓" } else { "▶ só esta" },
                        Box::new(keys::section::ToggleSelect),
                    )));
            }
            _ => {
                body = body.child(row().child(soft("compassos sem seção".into())).child(ibtn("ins-sec-add", "+ seção", Box::new(keys::section::Add))));
            }
        }

        // ── COMPASSO ──
        let meas = &song.measures[c.m];
        body = body.child(sub("COMPASSO"));
        let mut dyns = row().child(lbl("dinâmica"));
        for d in Dynamic::ALL {
            let m = c.m;
            let on = meas.dynamic == Some(d);
            dyns = dyns.child(chip(
                format!("ins-dyn-{}", d.as_str()),
                d.as_str().to_string(),
                on,
                Rc::new(move |sh, cx| sh.grid.update(cx, |g, cx| g.set_dynamic(m, None, if on { None } else { Some(d) }, cx))),
            ));
        }
        for (glyph, hp, id) in [("<", Hairpin::Cresc, "ins-cresc"), (">", Hairpin::Dim, "ins-dim")] {
            let m = c.m;
            dyns = dyns.child(chip(id.into(), glyph.into(), meas.hairpin == Some(hp), Rc::new(move |sh, cx| sh.grid.update(cx, |g, cx| g.toggle_hairpin(m, None, hp, cx)))));
        }
        let tempo_label = meas.tempo.as_ref().map(|t| {
            let before = ops::measures::tempo_at_measure_starts(&song).get(c.m).copied().unwrap_or(song.tempo);
            ops::measures::tempo_mark_label(t, before)
        });
        body = body
            .child(
                row().child(bold(format!("c{}", c.m + 1))).child(lbl("tempos")).child(ibtn("ins-beats-", "−", Box::new(keys::measure::BeatsDown))).child(val(format!("{}/4", meas.beats))).child(ibtn(
                    "ins-beats+",
                    "+",
                    Box::new(keys::measure::BeatsUp),
                )),
            )
            .child(dyns)
            .child(
                row()
                    .child(lbl("♩ BPM"))
                    .child(ibtn("ins-rall", "rall.", Box::new(keys::measure::TempoRall)))
                    .child(ibtn("ins-accel", "accel.", Box::new(keys::measure::TempoAccel)))
                    .child(ibtn("ins-atempo", "a tempo", Box::new(keys::measure::ATempo)))
                    .when_some(tempo_label, |d, l| d.child(soft(l)).child(ibtn("ins-tempo-rm", "✕", Box::new(keys::measure::TempoClear)))),
            );

        // ── LINHA ──
        let track = &song.tracks[c.t];
        let tid = track.id.clone();
        body = body.child(sub("LINHA"));
        let swatch = {
            keys::hintable(
                div()
                    .id("ins-color")
                    .relative()
                    .flex_none()
                    .size(px(14.0))
                    .rounded(px(ui::R1))
                    .bg(track_color(track))
                    .cursor_pointer()
                    .on_click(|_: &ClickEvent, w, cx| w.dispatch_action(Box::new(keys::track::CycleColor), cx)),
                "ins-color",
                Rc::new(move |w: &mut Window, cx: &mut App| w.dispatch_action(Box::new(keys::track::CycleColor), cx)),
            )
        };
        let is_main = ops::tracks::is_main_track(&song, &tid);
        let ms = |id: &'static str, label: &'static str, on: bool, color: gpui_kit::Hsla, f: ShellOp| {
            let w1 = weak.clone();
            let w2 = weak.clone();
            let f2 = f.clone();
            keys::hintable(
                small_btn_on(id, label, &th2, on, color).on_click(move |_: &ClickEvent, _, cx: &mut App| {
                    let _ = w1.update(cx, |sh, cx| f(sh, cx));
                }),
                id,
                Rc::new(move |_: &mut Window, cx: &mut App| {
                    let _ = w2.update(cx, |sh, cx| f2(sh, cx));
                }),
            )
        };
        let (t1, t2, t3) = (tid.clone(), tid.clone(), tid.clone());
        body = body
            .child(row().child(swatch).child(bold(track.name.clone())).child(ibtn("ins-trk-ren", "✎", Box::new(keys::track::Rename))).child(ibtn("ins-trk-set", "⚙", Box::new(keys::track::Settings))))
            .child(
                row()
                    .child(div().flex_1().min_w_0().child(soft(ops::kit::instrument_label(&song, track).to_string())))
                    .child(ms(
                        "ins-mute",
                        "M",
                        track.muted,
                        th2.warn,
                        Rc::new(move |sh, cx| {
                            let id = t1.clone();
                            sh.edit(cx, |s| mixer::toggle_track_mute(s, &id));
                        }),
                    ))
                    .child(ms(
                        "ins-solo",
                        "S",
                        track.solo,
                        th2.accent2,
                        Rc::new(move |sh, cx| {
                            let id = t2.clone();
                            sh.edit(cx, |s| mixer::toggle_track_solo(s, &id));
                        }),
                    ))
                    .child(ms(
                        "ins-main",
                        "★",
                        is_main,
                        th2.accent,
                        Rc::new(move |sh, cx| {
                            let id = t3.clone();
                            sh.edit(cx, |s| ops::tracks::toggle_main_track(s, &id));
                        }),
                    )),
            );
        let vol = track.volume.unwrap_or(wadawko_core::Track::DEFAULT_VOLUME_DB);
        let pan = track.pan.unwrap_or(0.0);
        let (st1, id1) = (state.clone(), tid.clone());
        let on_vol = Rc::new(move |v: f64, _: &mut Window, cx: &mut App| widgets::edit_co(&st1, cx, widgets::tag("trk-vol", &id1), |song| mixer::set_track_volume(song, &id1, v)));
        let (st2, id2) = (state.clone(), tid.clone());
        let on_pan = Rc::new(move |v: f64, _: &mut Window, cx: &mut App| widgets::edit_co(&st2, cx, widgets::tag("trk-pan", &id2), |song| mixer::set_track_pan(song, &id2, v)));
        body = body
            .child(
                row()
                    .child(lbl("volume"))
                    .child(widgets::slider(
                        Slider {
                            id: "ins-vol".into(),
                            value: vol,
                            min: VOL_MIN,
                            max: VOL_MAX,
                            step: 1.0,
                            default: Some(wadawko_core::Track::DEFAULT_VOLUME_DB),
                            vertical: false,
                            len: 120.0,
                            color: th2.ink_soft,
                            on_change: on_vol,
                        },
                        th,
                    ))
                    .child(soft(mixer::db_label(vol))),
            )
            .child(
                row()
                    .child(lbl("pan"))
                    .child(widgets::slider(
                        Slider { id: "ins-pan".into(), value: pan, min: -1.0, max: 1.0, step: 0.1, default: Some(0.0), vertical: false, len: 120.0, color: th2.accent2, on_change: on_pan },
                        th,
                    ))
                    .child(soft(mixer::pan_label(pan))),
            )
            .child(row().child(lbl("sub-linhas")).child(tchip("ins-dynrow", "f dinâmica", dynr, Box::new(keys::track::ToggleDynRow))).child(tchip(
                "ins-kyairow",
                "気 kiai",
                kyai,
                Box::new(keys::track::ToggleKyaiRow),
            )));
        if !song.people().is_empty() {
            let mut people = row().child(lbl("pessoas"));
            for p in song.people() {
                let on = track.person_ids().contains(&p.id);
                let col = p.color.as_deref().and_then(parse_color).unwrap_or(th2.ink_soft);
                let (pid, tid2) = (p.id.clone(), tid.clone());
                people = people.child(
                    chip(
                        format!("ins-pers-{}", p.id),
                        p.name.clone(),
                        on,
                        Rc::new(move |sh, cx| {
                            let (pid, tid) = (pid.clone(), tid2.clone());
                            sh.edit(cx, |s| ops::people::toggle_track_person(s, &tid, &pid));
                        }),
                    )
                    .child(div().size(px(7.0)).rounded(px(ui::R1)).bg(col)),
                );
            }
            body = body.child(people);
        }

        // ── CÉLULA ──
        body = body.child(sub(if lane == Lane::Kyai { "KIAI" } else { "CÉLULA" }));
        let sub_n = meas.subdivision.max(1);
        let pos = format!("c{} · {}.{}", c.m + 1, c.k / sub_n + 1, c.k % sub_n + 1);
        if lane == Lane::Kyai {
            let syl = ops::cells::get_kyai(&song, &tid, c.m, c.k).map(str::to_string);
            body = body.child(row().child(bold(syl.clone().unwrap_or_else(|| "—".into()))).child(soft(pos)));
        } else {
            let word = song.cell(c.t, c.m, c.k).map(str::to_string);
            let mut line = row().child(bold(word.clone().unwrap_or_else(|| "—".into()))).child(soft(pos));
            if let Some(res) = word.as_deref().and_then(phrase::resolve_cell) {
                let hits: Vec<String> = res
                    .hits
                    .iter()
                    .map(|h| {
                        let zone = if h.zone == Zone::Rim { "borda" } else { "couro" };
                        let hand = if h.hand == Hand::L { "E" } else { "D" };
                        format!("{hand} {zone} {:.0}%{}", h.vel * 100.0, if h.sfz { " !" } else { "" })
                    })
                    .collect();
                line = line.child(div().w_full().text_size(px(10.5)).text_color(th2.ink_soft).child(SharedString::from(hits.join(" · "))));
            } else if word.is_some() {
                line = line.child(div().w_full().text_size(px(10.5)).text_color(th2.danger).child("sílaba desconhecida"));
            }
            if track.is_shared()
                && let Some(owner) = ops::people::cell_owner_of(&song, track, c.m, c.k)
            {
                let name = song.person(owner).map(|p| p.name.as_str()).unwrap_or(owner).to_string();
                line = line.child(soft(format!("dono: {name}")));
            }
            body = body.child(line).child(
                row()
                    .child(ibtn("ins-sfz", "! sfz", Box::new(keys::grid::ToggleSfz)))
                    .child(ibtn("ins-acc", "A acento", Box::new(keys::grid::Accent)))
                    .child(ibtn("ins-unacc", "a", Box::new(keys::grid::Unaccent)))
                    .child(ibtn("ins-del", "× apagar", Box::new(keys::grid::DeleteCell))),
            );
        }
        body
    }
}
