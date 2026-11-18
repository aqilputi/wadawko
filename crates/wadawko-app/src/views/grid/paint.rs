//! Pintura do canvas de uma seção: O(células visíveis), com culling por
//! compasso/coluna e cache de texto modelado (docs/planejamento/gpui.md §4.1).

use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::{App, BorderStyle, Bounds, Hsla, Pixels, SharedString, TextAlign, Window, fill, outline, point, px, quad, size};
use wadawko_core::ops::people::cell_owner_of;
use wadawko_core::{Hairpin, Song, Zone, phrase};

use super::{CELL_H, CELL_X0, Cur, KYAI_H, Lane, MEASURE_GAP, MoveDrag, Range4, Rows, SEC_DYN_H, TDYN_H, TEMPO_X, TIMELINE_H, TextCache, VMode, measure_xs, tempo_chip_fits, tempo_chip_w};
use crate::song_ext::SongExt;
use crate::theme::{Theme, parse_color};

/// desenha um texto (com cache de modelagem): (texto, tamanho, cor, chave da cor, x, y, centralizar em largura)
type PaintText<'a> = dyn FnMut(&mut Window, &mut App, &SharedString, u32, Hsla, u8, f32, f32, Option<f32>) + 'a;

pub struct PaintArgs {
    pub s: usize,
    pub song: Arc<Song>,
    pub th: Theme,
    pub offset: f32,
    pub cw: f32,
    pub cursor: Option<Cur>,
    pub sel: Option<Range4>,
    pub vmode: VMode,
    pub lane: Lane,
    pub edit: SharedString,
    pub rows: Rows,
    pub move_drag: Option<MoveDrag>,
    pub shaped: TextCache,
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
}

fn rounded(r: Bounds<Pixels>, rad: f32, bg: Hsla) -> gpui_kit::PaintQuad {
    quad(r, px(rad), bg, px(0.0), gpui_kit::transparent_black(), BorderStyle::Solid)
}

fn line(window: &mut Window, a: (f32, f32), b: (f32, f32), color: Hsla) {
    let mut p = gpui_kit::PathBuilder::stroke(px(1.3));
    p.move_to(point(px(a.0), px(a.1)));
    p.line_to(point(px(b.0), px(b.1)));
    if let Ok(path) = p.build() {
        window.paint_path(path, color);
    }
}

/// "chip" de dinâmica + botões de hairpin + a cunha desenhada, numa faixa de altura `h`.
#[allow(clippy::too_many_arguments)]
fn paint_dyn(window: &mut Window, cx: &mut App, text: &mut PaintText<'_>, th: &Theme, mx: f32, y: f32, mw: f32, h: f32, dynamic: Option<&'static str>, hairpin: Option<Hairpin>, wedge_from: f32) {
    let label: SharedString = dynamic.unwrap_or("·").into();
    window.paint_quad(rounded(rect(mx + 2.0, y + 2.0, 56.0, h - 4.0), 5.0, if dynamic.is_some() { th.accent2_wash } else { th.paper_raise }));
    text(window, cx, &label, 12, if dynamic.is_some() { th.ink } else { th.ink_faint }, if dynamic.is_some() { 0 } else { 1 }, mx + 10.0, y + (h - 16.0) * 0.5, None);
    text(window, cx, &"⌄".into(), 11, th.ink_faint, 1, mx + 40.0, y + (h - 18.0) * 0.5, None);
    if mw - 44.0 > 62.0 {
        for (j, (glyph, kind)) in [("<", Hairpin::Cresc), (">", Hairpin::Dim)].into_iter().enumerate() {
            let hx = mx + mw - 44.0 + j as f32 * 22.0;
            let on = hairpin == Some(kind);
            window.paint_quad(rounded(rect(hx, y + 3.0, 18.0, h - 6.0), 4.0, if on { th.accent2 } else { th.paper_raise }));
            text(window, cx, &glyph.into(), 10, if on { th.on_accent } else { th.ink_faint }, if on { 4 } else { 1 }, hx, y + (h - 14.0) * 0.5, Some(18.0));
        }
    }
    // cunha do crescendo/diminuendo ao longo do compasso
    if let Some(hp) = hairpin {
        let (x0, x1) = (mx + wedge_from.max(66.0), mx + mw - 50.0);
        if x1 > x0 + 12.0 {
            let mid = y + h * 0.5;
            let open = (h * 0.28).min(6.0);
            let (a, b) = if hp == Hairpin::Cresc { (x0, x1) } else { (x1, x0) };
            line(window, (a, mid), (b, mid - open), th.accent2);
            line(window, (a, mid), (b, mid + open), th.accent2);
        }
    }
}

pub fn paint_section(b: Bounds<Pixels>, a: &PaintArgs, window: &mut Window, cx: &mut App) {
    // seção fora da área visível (rolagem vertical do editor): nada a pintar
    let visible = window.content_mask().bounds.intersect(&b);
    if f32::from(visible.size.width) <= 0.0 || f32::from(visible.size.height) <= 0.0 {
        return;
    }
    let (song, th, offset, cw, rows) = (&*a.song, &a.th, a.offset, a.cw, &a.rows);
    let n_tracks = song.tracks.len().min(rows.y.len());
    let ox = f32::from(b.origin.x);
    let oy = f32::from(b.origin.y);
    let w = f32::from(b.size.width);
    let (sa, sb) = song.section_range(a.s);
    let xs = measure_xs(song, a.s, cw);
    let mut cache = a.shaped.borrow_mut();
    let mut text = |window: &mut Window, cx: &mut App, t: &SharedString, size_px: u32, color: Hsla, ckey: u8, x: f32, y: f32, center_w: Option<f32>| {
        if t.is_empty() {
            return;
        }
        let line = cache.entry((t.clone(), size_px, ckey)).or_insert_with(|| {
            let mut run = window.text_style().to_run(t.len());
            run.color = color;
            window.text_system().shape_line(t.clone(), px(size_px as f32), &[run], None)
        });
        let lw = f32::from(line.width);
        let tx = match center_w {
            Some(cw) => x + (cw - lw) * 0.5,
            None => x,
        };
        let lh = size_px as f32 * 1.3;
        let _ = line.paint(point(px(tx), px(y)), px(lh), TextAlign::Left, None, window, cx);
    };

    // cores das pessoas (dono da nota em linha compartilhada)
    let colors: HashMap<&str, Hsla> = song.people().iter().filter_map(|p| Some((p.id.as_str(), parse_color(p.color.as_deref()?)?))).collect();
    let any_person_gate = song.people().iter().any(|p| p.is_muted() || p.is_solo());

    // andamento que vinha soando antes de cada compasso (rótulo rall./accel.)
    let tempo_before = if song.measures[sa..sb].iter().any(|m| m.tempo.is_some()) { wadawko_core::ops::measures::tempo_at_measure_starts(song) } else { Vec::new() };

    window.with_content_mask(Some(gpui_kit::ContentMask { bounds: b }), |window| {
        let cells_top = oy + SEC_DYN_H + TIMELINE_H;
        let bar_x = ox + CELL_X0 - 10.0 - offset;
        for t in 0..n_tracks {
            window.paint_quad(fill(rect(bar_x, cells_top + rows.y[t], 3.0, CELL_H), th.ink));
        }

        for (i, m) in (sa..sb).enumerate() {
            let meas = &song.measures[m];
            let len = meas.cell_count();
            let mx = ox + xs[i] - offset;
            let mw = len as f32 * cw;
            if mx > ox + w || mx + mw + MEASURE_GAP < ox {
                continue;
            }
            // mudança de andamento (rall./accel./a tempo) entre o chip e os hairpins
            let marked = meas.tempo.is_some();
            let tempo_fits = tempo_chip_fits(mw, marked);
            let wedge_from = if tempo_fits { TEMPO_X + tempo_chip_w(marked) + 6.0 } else { 0.0 };
            paint_dyn(window, cx, &mut text, th, mx, oy, mw, SEC_DYN_H, meas.dynamic.map(|d| d.as_str()), meas.hairpin, wedge_from);
            if tempo_fits {
                let tw = tempo_chip_w(marked);
                let r = rect(mx + TEMPO_X, oy + 2.0, tw, SEC_DYN_H - 4.0);
                match &meas.tempo {
                    Some(t) => {
                        window.paint_quad(rounded(r, 5.0, th.accent_wash));
                        let before = tempo_before.get(m).copied().unwrap_or(song.tempo);
                        let mut label = wadawko_core::ops::measures::tempo_mark_label(t, before);
                        if t.span > 1 {
                            label.push_str(&format!(" ·{}c", t.span));
                        }
                        text(window, cx, &label.into(), 11, th.ink, 0, mx + TEMPO_X, oy + (SEC_DYN_H - 15.0) * 0.5, Some(tw));
                    }
                    None => {
                        window.paint_quad(rounded(r, 5.0, th.paper_raise));
                        text(window, cx, &"♩".into(), 12, th.ink_faint, 1, mx + TEMPO_X, oy + (SEC_DYN_H - 16.0) * 0.5, Some(tw));
                    }
                }
            }

            let c0 = (((ox - mx) / cw).floor().max(0.0)) as usize;
            let c1 = ((((ox + w) - mx) / cw).ceil().max(0.0) as usize).min(len);
            let sub = meas.subdivision.max(1);
            for k in c0..c1 {
                let x = mx + k as f32 * cw;
                let (lbl, strong): (SharedString, bool) = if k % sub == 0 {
                    ((k / sub + 1).to_string().into(), true)
                } else if sub == 2 {
                    ("e".into(), false)
                } else if sub == 4 {
                    (["", "e", "&", "a"][k % 4].into(), false)
                } else {
                    ("·".into(), false)
                };
                if cw >= 22.0 || strong {
                    text(window, cx, &lbl, 12, if strong { th.ink } else { th.ink_faint }, if strong { 2 } else { 1 }, x, oy + SEC_DYN_H + 7.0, Some(cw));
                }
            }

            for t in 0..n_tracks {
                let track = &song.tracks[t];
                let y = cells_top + rows.y[t];
                let audible = song.audible(t);
                let shared = track.is_shared();
                let has_people = !track.person_ids().is_empty();
                let row = meas.cells.get(&track.id);
                for k in c0..c1 {
                    let x = mx + k as f32 * cw;
                    let beat_start = k % sub == 0;
                    let lx = if beat_start { x + 2.0 } else { x + 1.0 };
                    let cell = row.and_then(|r| r.get(k)).and_then(|c| c.as_deref());
                    let bg = if cell.is_some() { th.paper } else { th.cell_empty };
                    window.paint_quad(fill(rect(lx, y, x + cw - lx, CELL_H), bg));
                    if beat_start && k > 0 {
                        window.paint_quad(fill(rect(x, y, 2.0, CELL_H), th.line_strong));
                    }
                    let mut dim = !audible;
                    if let Some(word) = cell {
                        let inner = x + cw - lx;
                        match phrase::resolve_cell(word) {
                            Some(res) => {
                                // barras de golpe agrupadas por instante (doko = 2 barras)
                                let mut groups: Vec<(f32, bool, f32)> = Vec::new();
                                for h in &res.hits {
                                    let off = h.offset as f32;
                                    if let Some(g) = groups.iter_mut().find(|g| (g.0 - off).abs() < 1e-3) {
                                        g.2 = g.2.max(h.vel as f32);
                                    } else {
                                        groups.push((off, h.zone == Zone::Rim, h.vel as f32));
                                    }
                                }
                                for (gi, g) in groups.iter().enumerate() {
                                    let gx0 = lx + g.0 * inner + 2.0;
                                    let gx1 = groups.get(gi + 1).map(|n| lx + n.0 * inner - 1.0).unwrap_or(x + cw - 2.0);
                                    let col = if g.1 { th.accent2 } else { th.accent };
                                    let col = Theme::alpha(col, (0.45 + 0.55 * g.2.min(1.0)).min(1.0));
                                    window.paint_quad(quad(
                                        rect(gx0, y + CELL_H - 15.0, (gx1 - gx0).max(2.0), 12.0),
                                        gpui_kit::Corners { top_left: px(2.0), top_right: px(2.0), bottom_left: px(3.0), bottom_right: px(3.0) },
                                        col,
                                        px(0.0),
                                        gpui_kit::transparent_black(),
                                        BorderStyle::Solid,
                                    ));
                                }
                                if res.sfz {
                                    window.paint_quad(rounded(rect(x + cw - 8.0, y + 3.0, 5.0, 5.0), 2.5, th.accent));
                                }
                            }
                            None => window.paint_quad(outline(rect(lx, y, inner, CELL_H), th.accent, BorderStyle::Solid)),
                        }
                        // dono da nota: faixa na cor da pessoa (só em linha compartilhada)
                        if has_people {
                            let owner = cell_owner_of(song, track, m, k);
                            if shared && let Some(col) = owner.and_then(|o| colors.get(o)) {
                                window.paint_quad(fill(rect(lx, y, inner, 3.0), *col));
                            }
                            if any_person_gate && !song.owner_audible(t, m, k) {
                                dim = true;
                            }
                        }
                        let editing_here = a.lane == Lane::Cells && a.cursor == Some(Cur { m, t, k }) && !a.edit.is_empty();
                        if cw >= 26.0 && !editing_here {
                            let size_px = if cw >= 44.0 { 13 } else { 11 };
                            text(window, cx, &SharedString::from(word.to_string()), size_px, th.ink, 0, x, y + 6.0, Some(cw));
                        }
                    }
                    if dim {
                        // dono/linha em mudo: nota transparente
                        window.paint_quad(fill(rect(lx, y, x + cw - lx, CELL_H), Theme::alpha(th.bg, 0.6)));
                    }
                }
                // sub-linha de dinâmica própria do instrumento
                if rows.dynr[t] {
                    let dy = cells_top + rows.dyn_y(t);
                    window.paint_quad(rounded(rect(mx + 2.0, dy, mw - 4.0, TDYN_H), 4.0, Theme::alpha(th.paper_raise, 0.6)));
                    let mark = meas.track_dynamic(&track.id);
                    paint_dyn(window, cx, &mut text, th, mx, dy, mw, TDYN_H, mark.and_then(|d| d.dynamic).map(|d| d.as_str()), mark.and_then(|d| d.hairpin), 0.0);
                }
                // sub-linha de kiai
                if rows.kyai[t] {
                    let ky = cells_top + rows.kyai_y(t);
                    let krow = meas.kyai.as_ref().and_then(|k| k.get(&track.id));
                    for k in c0..c1 {
                        let x = mx + k as f32 * cw;
                        let syl = krow.and_then(|r| r.get(k)).and_then(|c| c.as_deref());
                        window.paint_quad(rounded(rect(x + 1.0, ky, cw - 2.0, KYAI_H), 4.0, if syl.is_some() { th.accent_wash } else { Theme::alpha(th.paper_raise, 0.55) }));
                        let editing_here = a.lane == Lane::Kyai && a.cursor == Some(Cur { m, t, k }) && !a.edit.is_empty();
                        if let Some(syl) = syl
                            && cw >= 22.0
                            && !editing_here
                        {
                            text(window, cx, &SharedString::from(syl.to_string()), 11, th.accent, 3, x, ky + 4.0, Some(cw));
                        }
                    }
                }
            }
            let sx = mx + mw + MEASURE_GAP * 0.5 - 1.5;
            for t in 0..n_tracks {
                window.paint_quad(fill(rect(sx, cells_top + rows.y[t], 3.0, CELL_H), th.ink));
            }
        }

        // seleção: intervalo linear [g0, g1] × linhas [t0, t1], recortado por compasso desta seção;
        // com arrasto de bloco, também o destino (deslocado)
        let mut spans: Vec<(Range4, Hsla, f32)> = Vec::new();
        if let Some(sel) = a.sel {
            spans.push((sel, th.accent2, 0.16));
            if let Some(md) = a.move_drag.filter(|md| md.dt != 0 || md.dg != 0) {
                let (t0, t1, g0, g1) = sel;
                let nt0 = t0 as isize + md.dt;
                let ng0 = g0 as isize + md.dg;
                if nt0 >= 0 && ng0 >= 0 && ((t1 as isize + md.dt) as usize) < n_tracks {
                    spans.push(((nt0 as usize, (t1 as isize + md.dt) as usize, ng0 as usize, (g1 as isize + md.dg) as usize), if md.clone { th.accent } else { th.ink }, 0.12));
                }
            }
        }
        for ((t0, t1, g0, g1), col, alpha) in spans {
            if sb <= sa || t1 >= n_tracks {
                continue;
            }
            let mut acc = song.flat(sa, 0);
            for (i, m) in (sa..sb).enumerate() {
                let len = song.mlen(m);
                if len == 0 {
                    continue;
                }
                let (lo, hi) = (g0.max(acc), g1.min(acc + len - 1));
                if lo <= hi {
                    let mx = ox + xs[i] - offset;
                    let x0 = mx + (lo - acc) as f32 * cw;
                    let x1 = mx + (hi - acc + 1) as f32 * cw;
                    let (y0, y1) = if a.lane == Lane::Kyai && rows.kyai[t0] {
                        (cells_top + rows.kyai_y(t0), cells_top + rows.kyai_y(t1) + KYAI_H)
                    } else {
                        (cells_top + rows.y[t0], cells_top + rows.y[t1] + CELL_H)
                    };
                    let r = rect(x0, y0, x1 - x0, y1 - y0);
                    window.paint_quad(fill(r, Theme::alpha(col, alpha)));
                    window.paint_quad(outline(r, col, BorderStyle::Solid));
                }
                acc += len;
            }
        }

        // cursor: normal/visual = bloco preenchido; insert/simples = contorno (+ texto em edição)
        if let Some(c) = a.cursor
            && c.m >= sa
            && c.m < sb
            && c.t < n_tracks
        {
            let x = ox + xs[c.m - sa] + c.k as f32 * cw - offset;
            let (y, h) = if a.lane == Lane::Kyai && rows.kyai[c.t] { (cells_top + rows.kyai_y(c.t), KYAI_H) } else { (cells_top + rows.y[c.t], CELL_H) };
            let r = rect(x, y, cw, h);
            let wash = if matches!(a.vmode, VMode::Normal | VMode::Visual | VMode::VLine) { Theme::alpha(th.accent, 0.18) } else { gpui_kit::transparent_black() };
            window.paint_quad(quad(r, px(3.0), wash, px(2.0), th.accent, BorderStyle::Solid));
            if !a.edit.is_empty() {
                let er = rect(x, y, (a.edit.len() as f32 * 9.0 + 14.0).max(cw), h);
                window.paint_quad(quad(er, px(3.0), th.accent_wash, px(2.0), th.accent, BorderStyle::Solid));
                let shown: SharedString = format!("{}▏", a.edit).into();
                let mut run = window.text_style().to_run(shown.len());
                run.color = th.ink;
                let line = window.text_system().shape_line(shown, px(14.0), &[run], None);
                let _ = line.paint(point(px(x + 7.0), px(y + (h - 18.0) * 0.5)), px(18.0), TextAlign::Left, None, window, cx);
            }
        }
    });
}
