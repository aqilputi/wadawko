//! Minimap do editor vertical, como o do VS Code: a música inteira em miniatura
//! à direita do editor — faixa de cor de cada seção, cabeçalhos, as notas de
//! cada linha na cor da linha —, o retângulo da área visível e o playhead.
//! Clicar ou arrastar rola o editor até ali.
//!
//! Peça em cache da moldura (`Part::Minimap`): a chave junta a revisão da
//! música, a área visível (arredondada), o playhead (por linha) e o tema.

use std::cell::Cell;
use std::hash::Hash;
use std::rc::Rc;

use gpui_kit::{App, BorderStyle, Bounds, Context, Hsla, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, canvas, div, point, prelude::*, px, quad, size, transparent_black};
use wadawko_core::Song;
use wadawko_core::ops::structure;

use crate::shell::Shell;
use crate::song_ext::SongExt;
use crate::theme::{Theme, parse_color, track_color};
use crate::views::grid::vgeom::{measure_ys, section_body_h};
use crate::views::grid::{SEC_GAP, SEC_HEAD_H};

/// Largura do minimap.
pub const MINI_W: f32 = 84.0;
/// Escala máxima (música curta não vira um bloco gigante).
const MAX_K: f32 = 0.22;

/// Seção no conteúdo do editor vertical (px do conteúdo, a partir do topo da grade).
#[derive(Clone, Debug, PartialEq)]
pub struct MiniSec {
    pub top: f32,
    pub body_h: f32,
    pub collapsed: bool,
    pub color: Option<Hsla>,
    /// (compasso, y do compasso no corpo)
    pub measures: Vec<(usize, f32)>,
}

/// Geometria do último desenho (para o clique converter y → conteúdo).
#[derive(Clone, Copy, Debug, Default)]
pub struct MiniGeom {
    pub bounds: Option<Bounds<Pixels>>,
    pub k: f32,
}

/// Layout vertical da música (o mesmo de `GridView::grid_height` no vertical).
pub fn layout(song: &Song, row_h: f32, collapsed: impl Fn(usize) -> bool) -> (Vec<MiniSec>, f32) {
    let mut y = 0.0;
    let mut out = Vec::new();
    for (s, sec) in song.secs().iter().enumerate() {
        let c = collapsed(s);
        let body_h = if c { 0.0 } else { section_body_h(song, s, row_h) };
        let color = sec.id.as_deref().and_then(|id| song.section(id)).and_then(|x| structure::section_color(song, x)).and_then(parse_color);
        let ms = measure_ys(song, s, row_h);
        out.push(MiniSec { top: y, body_h, collapsed: c, color, measures: (sec.start..sec.end).zip(ms).collect() });
        y += SEC_HEAD_H + body_h + SEC_GAP;
    }
    (out, y + 20.0)
}

impl Shell {
    /// Área visível do editor no conteúdo da grade: (topo, altura); `None` sem medida.
    fn mini_viewport(&self, cx: &App) -> Option<(f32, f32)> {
        let vb = self.scroll.bounds();
        let root = self.grid.read(cx).geom.borrow().root;
        let vh = f32::from(vb.size.height);
        (vh > 0.0 && f32::from(root.size.height) > 0.0).then(|| (f32::from(vb.origin.y) - f32::from(root.origin.y), vh))
    }

    /// Playhead no conteúdo da grade (y).
    fn mini_playhead(&self, cx: &App) -> Option<f32> {
        if !self.state.read(cx).playing() {
            return None;
        }
        let ph = self.playhead(cx)?;
        let root = self.grid.read(cx).geom.borrow().root;
        ph.vertical.then(|| ph.pos - f32::from(root.origin.y))
    }

    pub(crate) fn minimap_key(&self, k: &mut std::collections::hash_map::DefaultHasher, cx: &App) {
        let st = self.state.read(cx);
        let g = self.grid.read(cx);
        let vp = self.mini_viewport(cx).map(|(t, h)| ((t / 4.0).round() as i32, (h / 4.0).round() as i32));
        let ph = self.mini_playhead(cx).map(|y| (y / g.row_h.max(1.0)).round() as i32);
        let mut col: Vec<&String> = g.collapsed.iter().collect();
        col.sort();
        (st.revision, st.theme.dark, g.row_h.to_bits(), col, vp, ph).hash(k);
    }

    /// Rola o editor para o ponto `y` (janela) do minimap ficar no centro da vista.
    fn mini_scroll_to(&mut self, y: f32, cx: &mut Context<Self>) {
        let mg = self.mini_geom.get();
        let Some(b) = mg.bounds else { return };
        if mg.k <= 0.0 {
            return;
        }
        let Some((vt, vh)) = self.mini_viewport(cx) else { return };
        let target_top = (y - f32::from(b.origin.y)) / mg.k - vh / 2.0;
        let cur = self.scroll.offset();
        let max = self.scroll.max_offset();
        let ny = (f32::from(cur.y) + (vt - target_top)).clamp(-f32::from(max.y), 0.0);
        self.scroll.set_offset(point(cur.x, px(ny)));
        cx.notify();
    }

    pub(crate) fn minimap(&self, th: &Theme, cx: &mut Context<Self>) -> gpui_kit::Stateful<gpui_kit::Div> {
        let th = *th;
        let st = self.state.read(cx);
        let song = st.song().clone();
        let g = self.grid.read(cx);
        let row_h = g.row_h;
        let (secs, content_h) = layout(&song, row_h, |s| song.secs().get(s).is_some_and(|sec| g.is_collapsed(sec)));
        let tcols: Vec<Hsla> = song.tracks.iter().map(|t| track_color(&song, t)).collect();
        let vp = self.mini_viewport(cx);
        let ph = self.mini_playhead(cx);
        let mg = self.mini_geom.clone();
        let paint_geom: Rc<Cell<MiniGeom>> = mg.clone();
        let cv = canvas(
            move |b, _, _| b,
            move |b, _, window, _| {
                let (x0, y0, w, h) = (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height));
                let k = if content_h > 0.0 { (h / content_h).min(MAX_K) } else { MAX_K };
                paint_geom.set(MiniGeom { bounds: Some(b), k });
                let rect = |x: f32, y: f32, w: f32, h: f32| Bounds::new(point(px(x), px(y)), size(px(w), px(h)));
                let fill = |r: Bounds<Pixels>, c: Hsla| quad(r, px(0.0), c, px(0.0), transparent_black(), BorderStyle::Solid);
                let n = song.tracks.len().max(1);
                let cx0 = x0 + 8.0;
                let colw = ((w - 12.0) / n as f32).max(1.0);
                for sec in &secs {
                    let top = y0 + sec.top * k;
                    let total = (SEC_HEAD_H + sec.body_h) * k;
                    if top > y0 + h {
                        break;
                    }
                    // faixa de cor da seção à esquerda e o cabeçalho
                    window.paint_quad(fill(rect(x0 + 2.0, top, 3.0, total.max(1.0)), sec.color.unwrap_or(th.line_strong)));
                    window.paint_quad(fill(rect(cx0, top, w - 10.0, (SEC_HEAD_H * k).max(1.0)), Theme::alpha(th.ink_faint, 0.35)));
                    if sec.collapsed {
                        continue;
                    }
                    let body = top + SEC_HEAD_H * k;
                    let rh = (row_h * k).max(0.6);
                    for &(m, my) in &sec.measures {
                        let Some(meas) = song.measures.get(m) else { continue };
                        // barra de compasso
                        window.paint_quad(fill(rect(cx0, body + my * k, w - 10.0, 0.5), Theme::alpha(th.ink_faint, 0.5)));
                        for (t, tr) in song.tracks.iter().enumerate() {
                            let Some(row) = meas.cells.get(&tr.id) else { continue };
                            for (ci, c) in row.iter().enumerate() {
                                if c.as_deref().is_some_and(|s| !s.is_empty()) {
                                    let y = body + (my + ci as f32 * row_h) * k;
                                    window.paint_quad(fill(rect(cx0 + t as f32 * colw, y, (colw - 1.0).max(1.0), rh), Theme::alpha(tcols[t], 0.85)));
                                }
                            }
                        }
                    }
                }
                // área visível
                if let Some((vt, vh)) = vp {
                    let r = rect(x0, y0 + vt.max(0.0) * k, w, (vh * k).max(4.0));
                    window.paint_quad(quad(r, px(0.0), Theme::alpha(th.ink, 0.07), px(1.0), Theme::alpha(th.ink, 0.25), BorderStyle::Solid));
                }
                if let Some(py) = ph {
                    window.paint_quad(fill(rect(x0, y0 + py * k, w, 1.5), th.accent));
                }
            },
        )
        .size_full();
        div()
            .id("minimap")
            .w(px(MINI_W))
            .flex_none()
            .h_full()
            .bg(th.chrome)
            .border_l_1()
            .border_color(th.line)
            .cursor_pointer()
            .child(cv)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                    this.mini_drag = true;
                    this.mini_scroll_to(f32::from(ev.position.y), cx);
                }),
            )
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if this.mini_drag && ev.pressed_button == Some(MouseButton::Left) {
                    this.mini_scroll_to(f32::from(ev.position.y), cx);
                } else if this.mini_drag {
                    this.mini_drag = false;
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, _| this.mini_drag = false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wadawko_core::templates::template_song;

    #[test]
    fn layout_bate_com_a_altura_da_grade() {
        let s = template_song("ensemble5").unwrap();
        let (secs, h) = layout(&s, 28.0, |_| false);
        assert_eq!(secs.len(), 3);
        assert_eq!(secs[0].top, 0.0);
        assert_eq!(secs[1].top, SEC_HEAD_H + section_body_h(&s, 0, 28.0) + SEC_GAP);
        let sum: f32 = (0..3).map(|i| SEC_HEAD_H + section_body_h(&s, i, 28.0) + SEC_GAP).sum::<f32>() + 20.0;
        assert_eq!(h, sum);
        // recolhida: sem corpo
        let (c, _) = layout(&s, 28.0, |i| i == 1);
        assert!(c[1].collapsed && c[1].body_h == 0.0);
    }
}
