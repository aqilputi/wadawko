//! Overlay dos rótulos de salto (Ctrl+Space / Alt+F / `f` / `F`): elementos
//! registrados por `keys::hintable` + células da grade (pela geometria do último
//! frame). Rótulos mais curtos para os alvos mais próximos do cursor.

use std::rc::Rc;

use gpui_kit::{App, Bounds, Context, FontWeight, KeyDownEvent, MouseButton, Pixels, SharedString, TextAlign, Window, canvas, deferred, div, point, prelude::*, px, quad, size};

use crate::keys::{self, HintAct};
use crate::shell::{HintLabel, HintScope, HintSession, Shell};
use crate::theme::Theme;
use crate::views::grid::{Cur, VMode};

impl Shell {
    pub fn start_hints(&mut self, scope: HintScope, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        keys::HINTS.with(|h| h.borrow_mut().targets.clear());
        self.hints = Some(HintSession { scope, typed: String::new(), labels: Default::default() });
        self.hint_focus.focus(window, cx);
        // a grade fica em cache: força re-render para os botões dela se registrarem
        self.grid.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn stop_hints(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hints = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn hint_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(h) = self.hints.as_mut() else { return };
        match ev.keystroke.key.as_str() {
            "escape" => return self.stop_hints(window, cx),
            "backspace" => {
                h.typed.pop();
            }
            _ => {
                if let Some(ch) = ev.keystroke.key_char.as_deref()
                    && ch.len() == 1
                    && keys::HINT_ALPHABET.contains(&ch.to_lowercase())
                {
                    h.typed.push_str(&ch.to_lowercase());
                }
            }
        }
        let typed = h.typed.clone();
        let labels = h.labels.clone();
        let labels = labels.borrow();
        if let Some(hit) = labels.iter().find(|l| l.label == typed) {
            let act = hit.act.clone();
            drop(labels);
            self.stop_hints(window, cx);
            act(window, cx);
            return;
        }
        if !typed.is_empty() && !labels.iter().any(|l| l.label.starts_with(&typed)) {
            drop(labels);
            self.flash("rótulo inexistente", cx);
            self.stop_hints(window, cx);
            return;
        }
        cx.notify();
    }

    pub(crate) fn hints_view(&self, h: &HintSession, th: &Theme, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        // células (geometria do frame anterior) + elementos (registrados neste frame)
        let g = self.grid.read(cx);
        let cells: Vec<(Cur, Bounds<Pixels>)> = if self.on_grid() {
            match h.scope {
                HintScope::All => g.visible_cells(true, false),
                HintScope::Track => g.visible_cells(false, true),
                HintScope::Cells => g.visible_cells(false, false),
            }
        } else {
            Vec::new()
        };
        let cursor_pt = g.cursor.and_then(|c| cells.iter().find(|(cc, _)| *cc == c).map(|(_, b)| b.center()));
        let grid_w = self.grid.downgrade();
        let scope = h.scope;
        let typed = h.typed.clone();
        let labels = h.labels.clone();
        let editor_vb = self.scroll.bounds();
        let th = *th;

        let paint = canvas(
            |_, _, _| {},
            move |vb, _, window, cx| {
                // 1ª pintura da sessão: monta os rótulos
                if labels.borrow().is_empty() {
                    let mut targets: Vec<(Bounds<Pixels>, HintAct, bool)> = Vec::new();
                    if scope == HintScope::All {
                        keys::HINTS.with(|reg| {
                            for t in reg.borrow().targets.values() {
                                let c = t.bounds.center();
                                let in_window = vb.contains(&c);
                                let in_editor_x = c.x >= editor_vb.origin.x && c.x <= editor_vb.origin.x + editor_vb.size.width;
                                let off_editor = in_editor_x && (c.y < editor_vb.origin.y || c.y > editor_vb.origin.y + editor_vb.size.height);
                                if in_window && !off_editor && f32::from(t.bounds.size.width) > 0.0 {
                                    targets.push((t.bounds, t.act.clone(), false));
                                }
                            }
                        });
                    }
                    for (c, b) in &cells {
                        let c = *c;
                        let gw = grid_w.clone();
                        let act: HintAct = Rc::new(move |_, cx: &mut App| {
                            let _ = gw.update(cx, |gr, cx| {
                                gr.commit_edit_in_place(cx);
                                gr.cursor = Some(c);
                                if !matches!(gr.vmode, VMode::Visual | VMode::VLine) {
                                    gr.anchor = None;
                                }
                                gr.ensure_visible(c);
                                cx.notify();
                            });
                        });
                        targets.push((*b, act, true));
                    }
                    // mais perto do cursor (ou do centro) = rótulo mais curto
                    let origin = cursor_pt.unwrap_or(vb.center());
                    let dist = |b: &Bounds<Pixels>| {
                        let c = b.center();
                        let (dx, dy) = (f32::from(c.x - origin.x), f32::from(c.y - origin.y));
                        dx * dx + dy * dy * 2.0
                    };
                    targets.sort_by(|a, b| dist(&a.0).total_cmp(&dist(&b.0)));
                    let alpha: Vec<char> = keys::HINT_ALPHABET.chars().collect();
                    let names = keys::labels(targets.len(), &alpha);
                    *labels.borrow_mut() = targets.into_iter().zip(names).map(|((bounds, act, cell), label)| HintLabel { label, bounds, act, cell }).collect();
                }
                for l in labels.borrow().iter() {
                    if !l.label.starts_with(&typed) {
                        continue;
                    }
                    let text: SharedString = l.label.to_uppercase().into();
                    let mut run_done = window.text_style().to_run(typed.len());
                    run_done.color = Theme::alpha(th.on_accent, 0.45);
                    run_done.font.weight = FontWeight::BOLD;
                    let mut run_rest = window.text_style().to_run(text.len() - typed.len());
                    run_rest.color = th.on_accent;
                    run_rest.font.weight = FontWeight::BOLD;
                    let runs: Vec<_> = [run_done, run_rest].into_iter().filter(|r| r.len > 0).collect();
                    let line = window.text_system().shape_line(text, px(11.0), &runs, None);
                    let w = f32::from(line.width) + 8.0;
                    let x = f32::from(l.bounds.origin.x).max(f32::from(vb.origin.x));
                    let y = f32::from(l.bounds.origin.y).max(f32::from(vb.origin.y));
                    let r = Bounds::new(point(px(x), px(y)), size(px(w), px(16.0)));
                    let bg = if l.cell { th.accent2 } else { th.accent };
                    window.paint_quad(quad(r, px(4.0), bg, px(1.0), th.paper, gpui_kit::BorderStyle::Solid));
                    let _ = line.paint(point(px(x + 4.0), px(y + 1.0)), px(14.0), TextAlign::Left, None, window, cx);
                }
            },
        )
        .absolute()
        .inset_0();

        div()
            .id("hints")
            .absolute()
            .inset_0()
            .key_context("Hints")
            .track_focus(&self.hint_focus)
            .on_key_down(cx.listener(Self::hint_key))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.stop_hints(window, cx)))
            .bg(Theme::alpha(th.ink, 0.06))
            .child(deferred(paint).with_priority(200))
    }
}
