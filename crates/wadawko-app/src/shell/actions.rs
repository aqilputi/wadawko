//! Handlers de TODAS as ações globais, registrados no `div` raiz (valem com o
//! foco em qualquer lugar; a paleta, os botões e os hints despacham as mesmas ações).

use std::time::{Duration, Instant};

use gpui_kit::{Context, Window, prelude::*};
use wadawko_core::{Hairpin, Song, ops};

use super::{HintScope, Shell};
use crate::keys::modal::{ChangeKind as K, PromptKind};
use crate::song_ext::SongExt;
use crate::views::grid::{InputTarget, Lane, Menu, Mode, Motion as M, VMode};
use crate::views::{Page, Tab};

macro_rules! on {
    ($d:expr, $cx:expr, $A:ty, |$this:ident, $w:ident, $c:ident| $body:expr) => {
        $d.on_action($cx.listener(move |$this: &mut Shell, _: &$A, $w: &mut Window, $c: &mut Context<Shell>| {
            #[allow(unused_variables)]
            let _ = &$w;
            $body;
            $c.notify();
        }))
    };
}

/// Nome da subdivisão do metrônomo.
pub(crate) fn subdiv_name(d: u8) -> &'static str {
    match d {
        2 => "colcheias (2 por tempo)",
        3 => "tercinas (3 por tempo)",
        4 => "semicolcheias (4 por tempo)",
        _ => "só os tempos",
    }
}

/// BPM das batidas do tap tempo: média dos intervalos (até as 8 últimas
/// batidas); `None` com menos de 2.
pub(crate) fn tap_bpm(taps: &[Instant]) -> Option<f64> {
    let t = &taps[taps.len().saturating_sub(8)..];
    if t.len() < 2 {
        return None;
    }
    let secs = (t[t.len() - 1] - t[0]).as_secs_f64() / (t.len() - 1) as f64;
    (secs > 0.0).then(|| 60.0 / secs)
}

impl Shell {
    /// Tap tempo (F8): cada batida entra na média; uma pausa de mais de 2 s
    /// recomeça a contagem. A partir da 2ª batida o BPM da música muda.
    pub(crate) fn tap_tempo(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        if self.taps.last().is_some_and(|&t| now - t > Duration::from_secs(2)) {
            self.taps.clear();
        }
        self.taps.push(now);
        if self.taps.len() > 8 {
            self.taps.remove(0);
        }
        match tap_bpm(&self.taps) {
            Some(b) => {
                let b = b.clamp(30.0, 300.0).round();
                self.state.update(cx, |s, cx| s.set_bpm(b, cx));
                self.flash(format!("tap: {b:.0} BPM ({} batidas)", self.taps.len()), cx);
            }
            None => self.flash("tap: continue batendo no ritmo…", cx),
        }
    }

    /// `Loop: seleção` (F2): o trecho passa a ser os compassos da seleção da
    /// grade, ou só o compasso do cursor; liga o loop.
    pub(crate) fn loop_selection(&mut self, cx: &mut Context<Self>) {
        let (cur, sel) = {
            let g = self.grid.read(cx);
            (g.cursor, g.selection())
        };
        let Some(c) = cur else { return };
        let (m0, m1) = sel.map_or((c.m, c.m), |(a, b)| (a.m.min(b.m), a.m.max(b.m)));
        let r = wadawko_core::timeline::Region { start_bar: m0, end_bar: m1 + 1 };
        self.state.update(cx, |s, cx| s.set_range(Some(r), cx));
        self.flash(format!("loop: {}", crate::state::range_label(r)), cx);
    }

    /// `[` / `]` (F2): o compasso do cursor vira o início (`end = false`) ou o
    /// fim do trecho; sem trecho, `[` vai até o fim da música e `]` vem do começo.
    pub(crate) fn loop_edge(&mut self, end: bool, cx: &mut Context<Self>) {
        let Some(c) = self.grid.read(cx).cursor else { return };
        let n = self.song(cx).measures.len();
        let cur = self.state.read(cx).tp.range;
        let r = match (cur, end) {
            (Some(r), false) => wadawko_core::timeline::Region { start_bar: c.m, end_bar: r.end_bar.max(c.m + 1) },
            (Some(r), true) => wadawko_core::timeline::Region { start_bar: r.start_bar.min(c.m), end_bar: c.m + 1 },
            (None, false) => wadawko_core::timeline::Region { start_bar: c.m, end_bar: n },
            (None, true) => wadawko_core::timeline::Region { start_bar: 0, end_bar: c.m + 1 },
        };
        self.state.update(cx, |s, cx| s.set_range(Some(r), cx));
        self.flash(format!("loop: {}", crate::state::range_label(r)), cx);
    }

    fn go(&mut self, page: Page, tab: Option<Tab>) {
        self.page = page;
        if let Some(t) = tab {
            self.tab = t;
        }
    }

    /// edição sobre a seção do cursor (nada se o cursor está num bloco sem seção)
    fn sec_edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Song, &str)) {
        match self.cursor_section_id(cx) {
            Some(id) => {
                self.edit(cx, |s| f(s, &id));
            }
            None => self.flash("o cursor não está numa seção (crie uma com + seção)", cx),
        }
    }

    fn track_edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Song, &str)) {
        if let Some(id) = self.cursor_track_id(cx) {
            self.edit(cx, |s| f(s, &id));
        }
    }

    fn set_swing(&mut self, on: bool, cx: &mut Context<Self>) {
        self.edit(cx, |s| s.swing = if on { Some(1.0) } else { None });
    }

    fn move_sel(&mut self, dt: isize, dg: isize, clone: bool, cx: &mut Context<Self>) {
        let n = self.take_count() as isize;
        self.grid.update(cx, |g, cx| {
            if g.anchor.is_none() {
                // sem seleção: move/clona só a célula do cursor
                g.anchor = g.cursor;
                g.move_selection(dt * n, dg * n, clone, cx);
                g.anchor = None;
            } else {
                g.move_selection(dt * n, dg * n, clone, cx);
            }
        });
    }

    fn toggle_row(&mut self, kyai: bool, cx: &mut Context<Self>) {
        let Some(id) = self.cursor_track_id(cx) else { return };
        self.grid.update(cx, |g, cx| {
            let set = if kyai { &mut g.show_kyai } else { &mut g.show_dyn };
            if !set.remove(&id) {
                set.insert(id);
            }
            g.clamp_cursor();
            cx.notify();
        });
    }

    fn hairpin(&mut self, h: Hairpin, cx: &mut Context<Self>) {
        self.g(cx, |g, cx, _| {
            if let Some(c) = g.cursor {
                g.toggle_hairpin(c.m, None, h, cx)
            }
        });
    }

    pub(crate) fn actions(&self, d: gpui_kit::Stateful<gpui_kit::Div>, cx: &mut Context<Self>) -> gpui_kit::Stateful<gpui_kit::Div> {
        use crate::keys::{grid as ga, hints, keys as kb, measure, nav, project, section, song, track, transport, view, vim};
        let d = on!(d, cx, transport::TogglePlay, |this, w, cx| this.toggle_play(false, cx));
        let d = on!(d, cx, transport::PlayHere, |this, w, cx| this.toggle_play(true, cx));
        let d = on!(d, cx, transport::PlayFromStart, |this, w, cx| this.state.update(cx, |s, cx| {
            s.start_cell = 0;
            s.play(0, cx)
        }));
        // ⏮: leva o playhead ao início sem tocar (tocando, continua do início)
        let d = on!(d, cx, transport::GoToStart, |this, w, cx| this.state.update(cx, |s, cx| {
            s.start_cell = 0;
            s.seek_cell(0);
            cx.notify();
        }));
        let d = on!(d, cx, transport::Stop, |this, w, cx| this.state.update(cx, |s, cx| s.stop(cx)));
        let d = on!(d, cx, transport::ToggleLoop, |this, w, cx| this.state.update(cx, |s, cx| {
            let on = !s.tp.looping;
            s.set_loop(on, cx)
        }));
        let d = on!(d, cx, transport::ToggleMetronome, |this, w, cx| this.state.update(cx, |s, cx| {
            let on = !s.tp.metronome;
            s.set_metronome(on, cx)
        }));
        let d = on!(d, cx, transport::ToggleCountIn, |this, w, cx| this.state.update(cx, |s, cx| {
            s.tp.count_in = !s.tp.count_in;
            cx.notify()
        }));
        let d = on!(d, cx, transport::ToggleCountInLoop, |this, w, cx| {
            let on = !this.state.read(cx).tp.count_in_loop;
            this.state.update(cx, |s, cx| s.set_count_in_loop(on, cx));
            this.save_settings(cx);
            this.flash(if on { "count-in a cada volta do loop" } else { "count-in só no começo" }, cx)
        });
        let d = on!(d, cx, transport::MetroPattern, |this, w, cx| {
            let v = this.state.read(cx).tp.metro.pattern_text();
            this.ask(InputTarget::MetroPattern, if v == "auto" { String::new() } else { v }, w, cx)
        });
        let d = on!(d, cx, transport::MetroVolume, |this, w, cx| {
            let v = format!("{:.0}", this.state.read(cx).tp.metro.gain * 100.0);
            this.ask(InputTarget::MetroVolume, v, w, cx)
        });
        let d = on!(d, cx, transport::MetroSubdiv, |this, w, cx| {
            let mut m = this.state.read(cx).tp.metro;
            m.subdiv = m.subdiv % 4 + 1;
            this.state.update(cx, |s, cx| s.set_metronome_config(m, cx));
            this.save_settings(cx);
            this.flash(format!("metrônomo: {}", subdiv_name(m.subdiv)), cx)
        });
        let d = on!(d, cx, transport::TapTempo, |this, w, cx| this.tap_tempo(cx));
        let d = on!(d, cx, transport::ToggleTrainer, |this, w, cx| this.state.update(cx, |s, cx| {
            s.tp.trainer = !s.tp.trainer;
            cx.notify()
        }));
        let d = on!(d, cx, transport::ToggleSwing, |this, w, cx| {
            let on = this.song(cx).swing() <= 0.0;
            this.set_swing(on, cx)
        });
        let d = on!(d, cx, transport::Straight, |this, w, cx| this.set_swing(false, cx));
        let d = on!(d, cx, transport::SwingOn, |this, w, cx| this.set_swing(true, cx));
        let d = on!(d, cx, transport::BpmUp, |this, w, cx| {
            let b = this.song(cx).tempo + this.take_count() as f64;
            this.state.update(cx, |s, cx| s.set_bpm(b, cx))
        });
        let d = on!(d, cx, transport::BpmDown, |this, w, cx| {
            let b = this.song(cx).tempo - this.take_count() as f64;
            this.state.update(cx, |s, cx| s.set_bpm(b, cx))
        });
        let d = on!(d, cx, transport::SetBpm, |this, w, cx| {
            let b = format!("{:.0}", this.song(cx).tempo);
            this.ask(InputTarget::Bpm, b, w, cx)
        });
        let d = on!(d, cx, transport::TrainerPct, |this, w, cx| {
            let v = format!("{:.0}", this.state.read(cx).tp.trainer_pct * 100.0);
            this.ask(InputTarget::TrainerPct, v, w, cx)
        });
        let d = on!(d, cx, transport::TrainerStep, |this, w, cx| {
            let v = format!("{:.0}", this.state.read(cx).tp.trainer_step * 100.0);
            this.ask(InputTarget::TrainerStep, v, w, cx)
        });
        let d = on!(d, cx, transport::ClearRegion, |this, w, cx| this.state.update(cx, |s, cx| {
            s.tp.selected.clear();
            s.set_range(None, cx);
            s.sync_audio();
            cx.notify()
        }));
        let d = on!(d, cx, transport::LoopSelection, |this, w, cx| this.loop_selection(cx));
        let d = on!(d, cx, transport::LoopStartHere, |this, w, cx| this.loop_edge(false, cx));
        let d = on!(d, cx, transport::LoopEndHere, |this, w, cx| this.loop_edge(true, cx));
        let d = on!(d, cx, transport::PlayFue, |this, w, cx| this.state.update(cx, |s, _| {
            if let Some(a) = s.audio.as_mut() {
                a.play_fue();
            }
        }));

        let d = on!(d, cx, view::Grade, |this, w, cx| this.go(Page::Editor, Some(Tab::Grade)));
        let d = on!(d, cx, view::Partitura, |this, w, cx| this.go(Page::Editor, Some(Tab::Partitura)));
        let d = on!(d, cx, view::Esteira, |this, w, cx| this.go(Page::Editor, Some(Tab::Esteira)));
        let d = on!(d, cx, view::ToggleTheme, |this, w, cx| {
            let dark = !this.theme(cx).dark;
            this.set_theme(dark, cx)
        });
        let d = on!(d, cx, view::ZoomIn, |this, w, cx| this.g(cx, |g, cx, _| {
            g.zoom_at(1.25, 0.0);
            cx.notify()
        }));
        let d = on!(d, cx, view::ZoomOut, |this, w, cx| this.g(cx, |g, cx, _| {
            g.zoom_at(0.8, 0.0);
            cx.notify()
        }));
        let d = on!(d, cx, view::ZoomReset, |this, w, cx| this.g(cx, |g, cx, _| {
            g.zoom_reset();
            cx.notify()
        }));
        let d = on!(d, cx, view::ToggleNav, |this, w, cx| this.nav_open = !this.nav_open);
        let d = on!(d, cx, nav::Editor, |this, w, cx| this.go(Page::Editor, None));
        let d = on!(d, cx, nav::Pessoas, |this, w, cx| this.go(Page::Pessoas, None));
        let d = on!(d, cx, nav::Mixer, |this, w, cx| this.go(Page::Mixer, None));
        let d = on!(d, cx, nav::Kit, |this, w, cx| this.go(Page::Kit, None));
        let d = on!(d, cx, nav::Config, |this, w, cx| this.go(Page::Config, None));

        let d = on!(d, cx, kb::TogglePalette, |this, w, cx| if this.palette.is_some() { this.close_palette(w, cx) } else { this.open_palette(w, cx) });
        let d = on!(d, cx, kb::ShowShortcuts, |this, w, cx| {
            this.palette = None;
            this.page = Page::Config
        });
        let d = on!(d, cx, kb::ReloadKeymap, |this, w, cx| this.reload_keymap(cx));
        let d = on!(d, cx, kb::ToggleVim, |this, w, cx| {
            let on = !this.vim_on;
            this.set_vim(on, cx)
        });
        let d = on!(d, cx, kb::Cancel, |this, w, cx| this.cancel(cx));
        let d = on!(d, cx, kb::ToggleRootHints, |this, w, cx| {
            this.root_hints = !this.root_hints;
            this.palette = None;
        });
        let d = on!(d, cx, kb::CreateKeymap, |this, w, cx| match crate::keys::modal::create_user_keymap() {
            Ok(p) => this.flash(format!("criado {} — edite e salve, recarrega sozinho", p.display()), cx),
            Err(e) => this.flash(format!("não consegui criar: {e}"), cx),
        });
        let d = on!(d, cx, hints::Jump, |this, w, cx| this.start_hints(HintScope::All, w, cx));
        let d = on!(d, cx, hints::JumpTrack, |this, w, cx| this.start_hints(HintScope::Track, w, cx));
        let d = on!(d, cx, hints::JumpCells, |this, w, cx| this.start_hints(HintScope::Cells, w, cx));

        let d = on!(d, cx, project::Undo, |this, w, cx| this.state.update(cx, |s, cx| {
            s.undo(cx);
        }));
        let d = on!(d, cx, project::Redo, |this, w, cx| this.state.update(cx, |s, cx| {
            s.redo(cx);
        }));
        let d = on!(d, cx, project::New, |this, w, cx| this.state.update(cx, |s, cx| s.new_project(cx)));
        let d = on!(d, cx, project::Duplicate, |this, w, cx| this.state.update(cx, |s, cx| s.duplicate_project(cx)));
        let d = on!(d, cx, project::Rename, |this, w, cx| {
            let t = this.song(cx).title.clone();
            this.ask(InputTarget::SongTitle, t, w, cx)
        });
        let d = on!(d, cx, project::Delete, |this, w, cx| this.state.update(cx, |s, cx| {
            if s.store.projects().len() > 1 {
                let id = s.store.current_id().to_string();
                s.delete_project(&id, cx)
            } else {
                s.flash("é a única música — não excluí")
            }
        }));
        let d = on!(d, cx, project::Save, |this, w, cx| this.state.update(cx, |s, _| {
            let msg = if s.read_only() {
                "banco somente leitura — nada foi gravado".to_string()
            } else {
                match s.flush() {
                    Ok(()) => "gravado".to_string(),
                    Err(e) => format!("erro ao gravar: {e:#}"),
                }
            };
            s.flash(msg)
        }));
        let d = on!(d, cx, project::ExportJson, |this, w, cx| this.export_json(cx));
        let d = on!(d, cx, project::ImportJson, |this, w, cx| this.import_json(cx));
        let d = on!(d, cx, project::ImportLibrary, |this, w, cx| this.import_library(cx));
        let d = on!(d, cx, project::ExportMp3, |this, w, cx| this.export_audio("mp3", cx));
        let d = on!(d, cx, project::ExportWav, |this, w, cx| this.export_audio("wav", cx));
        let d = on!(d, cx, project::CancelExport, |this, w, cx| this.state.update(cx, |s, _| s.cancel_export()));
        let d = on!(d, cx, project::NextSong, |this, w, cx| {
            let (n, cur) = this.project_pos(cx);
            this.select_project((cur + 1) % n.max(1), cx)
        });
        let d = on!(d, cx, project::PrevSong, |this, w, cx| {
            let (n, cur) = this.project_pos(cx);
            this.select_project((cur + n.max(1) - 1) % n.max(1), cx)
        });

        let d = on!(d, cx, section::Add, |this, w, cx| {
            this.edit(cx, |s| {
                ops::sections::append_section(s, None);
            });
        });
        let d = on!(d, cx, section::Clone, |this, w, cx| this.sec_edit(cx, |s, id| {
            ops::sections::clone_section(s, id);
        }));
        let d = on!(d, cx, section::Remove, |this, w, cx| this.sec_edit(cx, ops::sections::remove_section));
        let d = on!(d, cx, section::RepeatUp, |this, w, cx| {
            let n = this.take_count() as u32;
            this.sec_edit(cx, |s, id| {
                let cur = s.section(id).map(|x| x.times()).unwrap_or(1);
                ops::sections::set_section_repeat(s, id, (cur + n).min(16))
            })
        });
        let d = on!(d, cx, section::RepeatDown, |this, w, cx| {
            let n = this.take_count() as u32;
            this.sec_edit(cx, |s, id| {
                let cur = s.section(id).map(|x| x.times()).unwrap_or(1);
                ops::sections::set_section_repeat(s, id, cur.saturating_sub(n).max(1))
            })
        });
        let d = on!(d, cx, section::MeasureAdd, |this, w, cx| {
            let (id, n) = (this.cursor_section_id(cx), this.take_count());
            this.edit(cx, |s| (0..n).for_each(|_| ops::measures::add_measure(s, id.as_deref())));
        });
        let d = on!(d, cx, section::MeasureRemove, |this, w, cx| {
            let (id, n) = (this.cursor_section_id(cx), this.take_count());
            this.edit(cx, |s| (0..n).for_each(|_| ops::measures::remove_measure(s, id.as_deref())));
        });
        let d = on!(d, cx, section::MoveUp, |this, w, cx| {
            let s = this.cursor_section(cx);
            if s > 0 {
                this.grid.update(cx, |g, cx| g.move_section_to(s, s - 1, cx));
                this.reveal_cursor(cx)
            }
        });
        let d = on!(d, cx, section::MoveDown, |this, w, cx| {
            let s = this.cursor_section(cx);
            if s + 1 < this.song(cx).secs().len() {
                this.grid.update(cx, |g, cx| g.move_section_to(s, s + 1, cx));
                this.reveal_cursor(cx)
            }
        });
        let d = on!(d, cx, section::Rename, |this, w, cx| {
            let s = this.cursor_section(cx);
            if let Some(sec) = this.song(cx).secs().get(s)
                && let Some(id) = sec.id.clone()
            {
                this.ask(InputTarget::SectionName(id), sec.name.clone(), w, cx)
            }
        });
        let d = on!(d, cx, section::ToggleSelect, |this, w, cx| match this.cursor_section_id(cx) {
            Some(id) => this.state.update(cx, |s, cx| s.toggle_section_selected(&id, cx)),
            None => this.flash("o cursor não está numa seção", cx),
        });
        let d = on!(d, cx, section::ToggleCollapse, |this, w, cx| {
            let s = this.cursor_section(cx);
            this.grid.update(cx, |g, cx| {
                g.toggle_collapsed(s);
                cx.notify()
            })
        });
        let d = on!(d, cx, section::ExpandAll, |this, w, cx| this.grid.update(cx, |g, cx| {
            g.collapsed.clear();
            cx.notify()
        }));
        let d = on!(d, cx, section::CollapseAll, |this, w, cx| this.grid.update(cx, |g, cx| {
            g.collapsed = g.song.secs().iter().map(crate::views::grid::sec_key).collect();
            cx.notify()
        }));
        let d = on!(d, cx, song::Sub2, |this, w, cx| {
            this.edit(cx, |s| ops::measures::set_subdivision(s, 2));
        });
        let d = on!(d, cx, song::Sub4, |this, w, cx| {
            this.edit(cx, |s| ops::measures::set_subdivision(s, 4));
        });
        let d = on!(d, cx, song::Sub8, |this, w, cx| {
            this.edit(cx, |s| ops::measures::set_subdivision(s, 8));
        });
        let d = on!(d, cx, measure::CycleDynamic, |this, w, cx| this.g(cx, |g, cx, _| {
            if let Some(c) = g.cursor {
                g.cycle_dynamic(c.m, cx)
            }
        }));
        let d = on!(d, cx, measure::ToggleCresc, |this, w, cx| this.hairpin(Hairpin::Cresc, cx));
        let d = on!(d, cx, measure::ToggleDim, |this, w, cx| this.hairpin(Hairpin::Dim, cx));
        let d = on!(d, cx, measure::BeatsUp, |this, w, cx| this.g(cx, |g, cx, _| g.change_beats(1, cx)));
        let d = on!(d, cx, measure::BeatsDown, |this, w, cx| this.g(cx, |g, cx, _| g.change_beats(-1, cx)));
        let d = on!(d, cx, measure::TempoRall, |this, w, cx| this.g(cx, |g, cx, _| g.tempo_preset(Some(-10.0), cx)));
        let d = on!(d, cx, measure::TempoAccel, |this, w, cx| this.g(cx, |g, cx, _| g.tempo_preset(Some(10.0), cx)));
        let d = on!(d, cx, measure::ATempo, |this, w, cx| this.g(cx, |g, cx, _| g.tempo_preset(None, cx)));
        let d = on!(d, cx, measure::TempoClear, |this, w, cx| this.g(cx, |g, cx, _| g.clear_tempo(cx)));

        let d = on!(d, cx, track::Add, |this, w, cx| {
            let def = this.add_instrument.clone();
            this.edit(cx, |s| {
                ops::tracks::add_track(s, &def);
            });
        });
        let d = on!(d, cx, track::Remove, |this, w, cx| this.track_edit(cx, ops::tracks::remove_track));
        let d = on!(d, cx, track::ToggleMute, |this, w, cx| this.track_edit(cx, |s, id| {
            if let Some(t) = s.track_mut(id) {
                t.muted = !t.muted
            }
        }));
        let d = on!(d, cx, track::ToggleSolo, |this, w, cx| this.track_edit(cx, |s, id| {
            if let Some(t) = s.track_mut(id) {
                t.solo = !t.solo
            }
        }));
        let d = on!(d, cx, track::CycleInstrument, |this, w, cx| {
            let song = this.song(cx);
            let defs = ops::kit::palette_instruments(&song);
            let i = defs.iter().position(|d| d.id == this.add_instrument).map(|i| i + 1).unwrap_or(0);
            this.add_instrument = defs[i % defs.len().max(1)].id.clone();
        });
        let d = on!(d, cx, track::Rename, |this, w, cx| {
            let song = this.song(cx);
            if let Some(t) = song.tracks.get(this.cursor_track(cx)) {
                this.ask(InputTarget::TrackName(t.id.clone()), t.name.clone(), w, cx)
            }
        });
        let d = on!(d, cx, track::MoveUp, |this, w, cx| {
            this.track_edit(cx, |s, id| ops::tracks::move_track(s, id, -1));
            this.grid.update(cx, |g, cx| g.motion(M::Up, 1, false, cx))
        });
        let d = on!(d, cx, track::MoveDown, |this, w, cx| {
            this.track_edit(cx, |s, id| ops::tracks::move_track(s, id, 1));
            this.grid.update(cx, |g, cx| g.motion(M::Down, 1, false, cx))
        });
        let d = on!(d, cx, track::ToggleMain, |this, w, cx| this.track_edit(cx, ops::tracks::toggle_main_track));
        let d = on!(d, cx, track::CycleColor, |this, w, cx| this.track_edit(cx, ops::tracks::cycle_track_color));
        let d = on!(d, cx, track::ToggleDynRow, |this, w, cx| this.toggle_row(false, cx));
        let d = on!(d, cx, track::ToggleKyaiRow, |this, w, cx| this.toggle_row(true, cx));
        let d = on!(d, cx, track::Settings, |this, w, cx| {
            let (t, s) = (this.cursor_track(cx), this.cursor_section(cx));
            this.grid.update(cx, |g, cx| {
                g.menu = if g.menu == Some(Menu::Row { t, s }) { None } else { Some(Menu::Row { t, s }) };
                cx.notify()
            })
        });

        // movimentos
        let d = on!(d, cx, ga::Left, |this, w, cx| this.mv(M::Left, false, cx));
        let d = on!(d, cx, ga::Right, |this, w, cx| this.mv(M::Right, false, cx));
        // Tab / Shift+Tab: grava o que está sendo digitado e anda uma célula (no lugar
        // do Tab do gpui_kit, que só trocava o foco e deixava a grade sem teclado)
        let d = on!(d, cx, ga::NextCell, |this, w, cx| this.tab_cell(M::Right, cx));
        let d = on!(d, cx, ga::PrevCell, |this, w, cx| this.tab_cell(M::Left, cx));
        let d = on!(d, cx, ga::Up, |this, w, cx| {
            this.mv(M::Up, false, cx);
            this.reveal_cursor(cx)
        });
        let d = on!(d, cx, ga::Down, |this, w, cx| {
            this.mv(M::Down, false, cx);
            this.reveal_cursor(cx)
        });
        let d = on!(d, cx, ga::ExtendLeft, |this, w, cx| this.mv(M::Left, true, cx));
        let d = on!(d, cx, ga::ExtendRight, |this, w, cx| this.mv(M::Right, true, cx));
        let d = on!(d, cx, ga::ExtendUp, |this, w, cx| this.mv(M::Up, true, cx));
        let d = on!(d, cx, ga::ExtendDown, |this, w, cx| this.mv(M::Down, true, cx));
        let d = on!(d, cx, ga::NextBeat, |this, w, cx| this.mv(M::NextBeat, false, cx));
        let d = on!(d, cx, ga::PrevBeat, |this, w, cx| this.mv(M::PrevBeat, false, cx));
        let d = on!(d, cx, ga::EndOfBeat, |this, w, cx| this.mv(M::EndOfBeat, false, cx));
        let d = on!(d, cx, ga::NextMeasure, |this, w, cx| this.mv(M::NextMeasure, false, cx));
        let d = on!(d, cx, ga::PrevMeasure, |this, w, cx| this.mv(M::PrevMeasure, false, cx));
        let d = on!(d, cx, ga::MeasureStart, |this, w, cx| this.mv(M::MeasureStart, false, cx));
        let d = on!(d, cx, ga::MeasureEnd, |this, w, cx| this.mv(M::MeasureEnd, false, cx));
        let d = on!(d, cx, ga::FirstFilled, |this, w, cx| this.mv(M::FirstFilled, false, cx));
        let d = on!(d, cx, ga::SongStart, |this, w, cx| {
            this.mv(M::SongStart, false, cx);
            this.reveal_cursor(cx)
        });
        let d = on!(d, cx, ga::SongEnd, |this, w, cx| {
            this.mv(M::SongEnd, false, cx);
            this.reveal_cursor(cx)
        });
        let d = on!(d, cx, ga::NextSection, |this, w, cx| {
            this.mv(M::NextSection, false, cx);
            this.reveal_cursor(cx)
        });
        let d = on!(d, cx, ga::PrevSection, |this, w, cx| {
            this.mv(M::PrevSection, false, cx);
            this.reveal_cursor(cx)
        });
        let d = on!(d, cx, ga::FirstTrack, |this, w, cx| this.mv(M::FirstTrack, false, cx));
        let d = on!(d, cx, ga::LastTrack, |this, w, cx| this.mv(M::LastTrack, false, cx));
        let d = on!(d, cx, ga::HalfRight, |this, w, cx| this.mv(M::HalfRight, false, cx));
        let d = on!(d, cx, ga::HalfLeft, |this, w, cx| this.mv(M::HalfLeft, false, cx));

        // operações (normal) — repetíveis com `.`
        let d = on!(d, cx, ga::DeleteCell, |this, w, cx| this.change(K::DeleteCell, cx));
        let d = on!(d, cx, ga::DeleteCellBack, |this, w, cx| this.change(K::DeleteCellBack, cx));
        let d = on!(d, cx, ga::DeleteMeasureRow, |this, w, cx| this.change(K::DeleteMeasureRow, cx));
        let d = on!(d, cx, ga::DeleteMeasureAll, |this, w, cx| this.change(K::DeleteMeasureAll, cx));
        let d = on!(d, cx, ga::DeleteBeat, |this, w, cx| this.change(K::DeleteBeat, cx));
        let d = on!(d, cx, ga::DeleteToMeasureEnd, |this, w, cx| this.change(K::DeleteToMeasureEnd, cx));
        let d = on!(d, cx, ga::DeleteSectionRow, |this, w, cx| this.change(K::DeleteSectionRow, cx));
        let d = on!(d, cx, ga::YankMeasureRow, |this, w, cx| {
            this.g(cx, |g, cx, n| {
                if let Some(r) = g.r_measures(n, false) {
                    g.yank(r, true, cx)
                }
            });
            this.flash("compasso copiado", cx)
        });
        let d = on!(d, cx, ga::YankBeat, |this, w, cx| {
            this.g(cx, |g, cx, n| {
                if let Some(r) = g.r_beats(n) {
                    g.yank(r, false, cx)
                }
            });
            this.flash("tempo copiado", cx)
        });
        let d = on!(d, cx, ga::YankSectionRow, |this, w, cx| {
            this.g(cx, |g, cx, _| {
                if let Some(r) = g.r_section() {
                    g.yank(r, false, cx)
                }
            });
            this.flash("seção (linha) copiada", cx)
        });
        let d = on!(d, cx, ga::PasteAfter, |this, w, cx| this.change(K::PasteAfter, cx));
        let d = on!(d, cx, ga::PasteBefore, |this, w, cx| this.change(K::PasteBefore, cx));
        // seleção (visual / simples)
        let d = on!(d, cx, ga::Copy, |this, w, cx| {
            this.g(cx, |g, cx, _| {
                if let Some(r) = g.sel_range() {
                    g.yank(r, false, cx)
                }
                // no modo simples a seleção fica (dá para Alt+arrastar / colar em seguida),
                // como no app web; no vim o y sai do visual
                if g.vmode != VMode::Simple {
                    Self::end_visual(g)
                }
            });
            this.flash("copiado", cx)
        });
        let d = on!(d, cx, ga::Cut, |this, w, cx| this.g(cx, |g, cx, _| {
            if let Some(r) = g.sel_range() {
                g.delete(r, false, cx)
            }
            Self::end_visual(g)
        }));
        let d = on!(d, cx, ga::DeleteSelection, |this, w, cx| this.g(cx, |g, cx, _| {
            if let Some(r) = g.sel_range() {
                g.delete(r, false, cx)
            }
            Self::end_visual(g)
        }));
        let d = on!(d, cx, ga::Paste, |this, w, cx| this.g(cx, |g, cx, _| {
            g.paste_over(cx);
            Self::end_visual(g)
        }));
        let d = on!(d, cx, ga::ToggleSfz, |this, w, cx| this.change_maybe_visual(K::ToggleSfz, cx));
        let d = on!(d, cx, ga::Accent, |this, w, cx| this.change_maybe_visual(K::Accent, cx));
        let d = on!(d, cx, ga::Unaccent, |this, w, cx| this.change_maybe_visual(K::Unaccent, cx));
        let d = on!(d, cx, ga::ToggleSelectMode, |this, w, cx| this.g(cx, |g, cx, _| {
            g.mode = if g.mode == Mode::Edit { Mode::Select } else { Mode::Edit };
            g.anchor = None;
            cx.notify()
        }));
        let d = on!(d, cx, ga::ModeEdit, |this, w, cx| this.g(cx, |g, cx, _| {
            g.mode = Mode::Edit;
            g.anchor = None;
            cx.notify()
        }));
        let d = on!(d, cx, ga::ModeSelect, |this, w, cx| this.g(cx, |g, cx, _| {
            g.mode = Mode::Select;
            cx.notify()
        }));
        let d = on!(d, cx, ga::MoveSelLeft, |this, w, cx| this.move_sel(0, -1, false, cx));
        let d = on!(d, cx, ga::MoveSelRight, |this, w, cx| this.move_sel(0, 1, false, cx));
        let d = on!(d, cx, ga::MoveSelUp, |this, w, cx| this.move_sel(-1, 0, false, cx));
        let d = on!(d, cx, ga::MoveSelDown, |this, w, cx| this.move_sel(1, 0, false, cx));
        let d = on!(d, cx, ga::CloneSelLeft, |this, w, cx| this.move_sel(0, -1, true, cx));
        let d = on!(d, cx, ga::CloneSelRight, |this, w, cx| this.move_sel(0, 1, true, cx));
        let d = on!(d, cx, ga::CloneSelUp, |this, w, cx| this.move_sel(-1, 0, true, cx));
        let d = on!(d, cx, ga::CloneSelDown, |this, w, cx| this.move_sel(1, 0, true, cx));
        let d = on!(d, cx, ga::AssignToPen, |this, w, cx| {
            let pen = this.state.read(cx).pen.clone();
            match pen {
                Some(p) => this.g(cx, |g, cx, _| g.assign_selection(Some(p), cx)),
                None => this.flash("escolha a caneta (pessoa) na barra de ferramentas ou com space n", cx),
            }
        });
        let d = on!(d, cx, ga::NextPen, |this, w, cx| this.state.update(cx, |s, cx| {
            let people = s.song().people().to_vec();
            if people.is_empty() {
                s.flash("a música não tem pessoas (página Pessoas)");
            } else {
                // … → pessoa 1 → pessoa 2 → sem caneta → …
                let i = s.pen.as_ref().and_then(|p| people.iter().position(|x| &x.id == p));
                s.pen = match i {
                    None => Some(people[0].id.clone()),
                    Some(i) => people.get(i + 1).map(|p| p.id.clone()),
                };
                let name = s.pen.as_ref().and_then(|p| people.iter().find(|x| &x.id == p)).map(|p| p.name.clone());
                s.flash(match name {
                    Some(n) => format!("caneta: {n}"),
                    None => "caneta: ninguém".into(),
                });
            }
            cx.notify()
        }));
        let d = on!(d, cx, ga::ToggleKyaiLane, |this, w, cx| {
            let id = this.cursor_track_id(cx);
            this.g(cx, |g, cx, _| {
                g.commit_edit_in_place(cx);
                if g.lane == Lane::Kyai {
                    g.lane = Lane::Cells;
                } else if let Some(id) = id {
                    g.show_kyai.insert(id);
                    g.lane = Lane::Kyai;
                    g.anchor = None;
                }
                cx.notify()
            })
        });

        // vim
        let d = on!(d, cx, vim::Insert, |this, w, cx| this.change(K::Insert, cx));
        let d = on!(d, cx, vim::Append, |this, w, cx| this.change(K::Append, cx));
        let d = on!(d, cx, vim::InsertMeasureStart, |this, w, cx| this.change(K::InsertMeasureStart, cx));
        let d = on!(d, cx, vim::AppendMeasureEnd, |this, w, cx| this.change(K::AppendMeasureEnd, cx));
        let d = on!(d, cx, vim::Substitute, |this, w, cx| this.change(K::Substitute, cx));
        let d = on!(d, cx, vim::ChangeMeasureRow, |this, w, cx| this.change(K::ChangeMeasureRow, cx));
        let d = on!(d, cx, vim::ChangeBeat, |this, w, cx| this.change(K::ChangeBeat, cx));
        let d = on!(d, cx, vim::ChangeSelection, |this, w, cx| this.g(cx, |g, cx, _| {
            if let Some(r) = g.sel_range() {
                g.delete(r, false, cx)
            }
            g.anchor = None;
            g.vmode = VMode::Insert
        }));
        let d = on!(d, cx, vim::Normal, |this, w, cx| this.finish_insert(cx));
        let d = on!(d, cx, vim::Repeat, |this, w, cx| this.repeat_change(cx));
        let d = on!(d, cx, vim::PushRegister, |this, w, cx| this.open_prompt(PromptKind::Register, w, cx));
        let d = on!(d, cx, vim::SetMark, |this, w, cx| this.open_prompt(PromptKind::SetMark, w, cx));
        let d = on!(d, cx, vim::GotoMark, |this, w, cx| this.open_prompt(PromptKind::GotoMark, w, cx));
        let d = on!(d, cx, vim::Search, |this, w, cx| this.open_prompt(PromptKind::Search { back: false }, w, cx));
        let d = on!(d, cx, vim::SearchBack, |this, w, cx| this.open_prompt(PromptKind::Search { back: true }, w, cx));
        let d = on!(d, cx, vim::SearchNext, |this, w, cx| this.search(true, cx));
        let d = on!(d, cx, vim::SearchPrev, |this, w, cx| this.search(false, cx));
        let d = on!(d, cx, vim::CommandLine, |this, w, cx| this.open_prompt(PromptKind::Command, w, cx));
        let d = on!(d, cx, vim::Visual, |this, w, cx| this.g(cx, |g, cx, _| {
            g.anchor = g.cursor;
            g.vmode = VMode::Visual;
            cx.notify()
        }));
        let d = on!(d, cx, vim::VisualMeasure, |this, w, cx| this.g(cx, |g, cx, _| {
            if g.anchor.is_none() {
                g.anchor = g.cursor;
            }
            g.vmode = VMode::VLine;
            cx.notify()
        }));
        let d = on!(d, cx, vim::SwapAnchor, |this, w, cx| this.g(cx, |g, cx, _| {
            std::mem::swap(&mut g.anchor, &mut g.cursor);
            cx.notify()
        }));
        let digit = |this: &mut Shell, n: u32| {
            this.count = Some((this.count.unwrap_or(0) * 10 + n).min(9999));
        };
        let d = on!(d, cx, vim::Digit0, |this, w, cx| digit(this, 0));
        let d = on!(d, cx, vim::Digit1, |this, w, cx| digit(this, 1));
        let d = on!(d, cx, vim::Digit2, |this, w, cx| digit(this, 2));
        let d = on!(d, cx, vim::Digit3, |this, w, cx| digit(this, 3));
        let d = on!(d, cx, vim::Digit4, |this, w, cx| digit(this, 4));
        let d = on!(d, cx, vim::Digit5, |this, w, cx| digit(this, 5));
        let d = on!(d, cx, vim::Digit6, |this, w, cx| digit(this, 6));
        let d = on!(d, cx, vim::Digit7, |this, w, cx| digit(this, 7));
        let d = on!(d, cx, vim::Digit8, |this, w, cx| digit(this, 8));
        on!(d, cx, vim::Digit9, |this, w, cx| digit(this, 9))
    }

    /// (nº de músicas, índice da atual)
    /// Tab na grade: grava a edição em andamento e anda uma célula (Shift = volta).
    /// Fora da grade (outras páginas/abas) não faz nada.
    fn tab_cell(&mut self, dir: M, cx: &mut Context<Self>) {
        if self.page != Page::Editor || self.tab != Tab::Grade {
            return;
        }
        self.grid.update(cx, |g, cx| {
            if let Some(c) = g.commit_edit_in_place(cx) {
                g.cursor = Some(c);
            }
            g.motion(dir, 1, false, cx);
            g.anchor = None;
            cx.notify();
        });
        self.reveal_cursor(cx);
    }

    pub fn project_pos(&self, cx: &gpui_kit::App) -> (usize, usize) {
        let st = self.state.read(cx);
        let cur = st.store.current_id();
        (st.store.projects().len(), st.store.projects().iter().position(|p| p.id == cur).unwrap_or(0))
    }

    /// `!`, `gU`, `gu`: em visual agem sobre a seleção e voltam ao normal (não entram no `.`).
    fn change_maybe_visual(&mut self, kind: K, cx: &mut Context<Self>) {
        let visual = self.grid.read(cx).anchor.is_some();
        self.change(kind, cx);
        if visual {
            self.last_change = None;
            self.grid.update(cx, |g, _| {
                if g.mode != Mode::Select || g.vmode != VMode::Simple {
                    Self::end_visual(g)
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_tempo_media_dos_intervalos() {
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        assert_eq!(tap_bpm(&[t0]), None);
        // 500 ms entre batidas = 120 BPM
        assert!((tap_bpm(&[at(0), at(500), at(1000), at(1500)]).unwrap() - 120.0).abs() < 1e-9);
        // batidas irregulares: vale a média (1,2 s / 2 = 600 ms = 100 BPM)
        assert!((tap_bpm(&[at(0), at(550), at(1200)]).unwrap() - 100.0).abs() < 1e-9);
        // só as 8 últimas contam: as 2 primeiras (lentas) ficam de fora
        let mut v = vec![at(0), at(2000)];
        v.extend((0..8).map(|i| at(4000 + i * 400)));
        assert!((tap_bpm(&v).unwrap() - 150.0).abs() < 1e-9);
    }
}
