//! Pintura do canvas de uma seção no editor vertical (tracker.md §2.3, §6.4):
//! gutter (cor da seção, faixa de loop, `c12`/tempos/subdivisões), coluna do
//! compasso (dinâmica, `<` `>`, andamento, cunha vertical) e as colunas das linhas
//! (nota com barra de golpe vertical, dinâmica da linha, kiai). Culling por linha
//! visível: só as linhas dentro da área visível do editor são pintadas.

use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::{App, BorderStyle, Bounds, ContentMask, Hsla, Pixels, SharedString, TextAlign, Window, fill, outline, point, px, quad, size};
use wadawko_core::ops::people::cell_owner_of;
use wadawko_core::timeline::Region;
use wadawko_core::{Hairpin, Song, Zone, phrase};

use super::vgeom::{Cols, DYN_W, GUTTER_W, KYAI_W, LEFT_W, LOOP_W, LOOP_X, MEAS_W, measure_ys};
use super::{Cur, Lane, MoveDrag, Range4, TextCache, VMode};
use crate::song_ext::SongExt;
use crate::theme::{Theme, parse_color};
use crate::ui::{R0, R1};

pub struct VPaintArgs {
    pub s: usize,
    pub song: Arc<Song>,
    pub th: Theme,
    /// rolagem horizontal das colunas
    pub offset: f32,
    pub row_h: f32,
    pub cols: Cols,
    pub cursor: Option<Cur>,
    pub sel: Option<Range4>,
    pub vmode: VMode,
    pub lane: Lane,
    pub edit: SharedString,
    pub move_drag: Option<MoveDrag>,
    pub range: Option<Region>,
    pub looping: bool,
    pub loop_preview: Option<Region>,
    /// cor da seção nos 2 px do gutter (T7; por ora `None` = `ink_faint`)
    pub sec_color: Option<Hsla>,
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

/// cunha vertical (crescendo abre para baixo; diminuendo fecha) centrada em `xc`
fn vwedge(window: &mut Window, xc: f32, y0: f32, y1: f32, hp: Hairpin, color: Hsla) {
    if y1 < y0 + 12.0 {
        return;
    }
    let open = 4.0;
    let (a, b) = if hp == Hairpin::Cresc { (y0, y1) } else { (y1, y0) };
    line(window, (xc, a), (xc - open, b), color);
    line(window, (xc, a), (xc + open, b), color);
}

pub fn vpaint_section(b: Bounds<Pixels>, a: &VPaintArgs, window: &mut Window, cx: &mut App) {
    let visible = window.content_mask().bounds.intersect(&b);
    if f32::from(visible.size.width) <= 0.0 || f32::from(visible.size.height) <= 0.0 {
        return;
    }
    let (song, th, rh, cols, off) = (&*a.song, &a.th, a.row_h, &a.cols, a.offset);
    let n_tracks = song.tracks.len().min(cols.x.len());
    let (ox, oy) = (f32::from(b.origin.x), f32::from(b.origin.y));
    let w = f32::from(b.size.width);
    let (vy0, vy1) = (f32::from(visible.origin.y), f32::from(visible.origin.y) + f32::from(visible.size.height));
    let (sa, sb) = song.section_range(a.s);
    let ys = measure_ys(song, a.s, rh);
    let colx0 = ox + LEFT_W;
    let nw = cols.note_w;
    let syl_px: u32 = if rh <= 24.0 { 11 } else { 13 };
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
        let _ = line.paint(point(px(tx), px(y)), px(size_px as f32 * 1.3), TextAlign::Left, None, window, cx);
    };
    // y do texto centralizado numa linha
    let ty = |y: f32, size_px: u32| y + (rh - size_px as f32 * 1.3) * 0.5;

    let colors: HashMap<&str, Hsla> = song.people().iter().filter_map(|p| Some((p.id.as_str(), parse_color(p.color.as_deref()?)?))).collect();
    let any_person_gate = song.people().iter().any(|p| p.is_muted() || p.is_solo());
    let tempo_before = if song.measures[sa..sb].iter().any(|m| m.tempo.is_some()) { wadawko_core::ops::measures::tempo_at_measure_starts(song) } else { Vec::new() };
    let cols_mask = b.intersect(&rect(colx0, oy, (w - LEFT_W).max(0.0), f32::from(b.size.height)));

    window.with_content_mask(Some(ContentMask { bounds: b }), |window| {
        // cor da seção: 2 px à esquerda do gutter; divisor da coluna do compasso
        window.paint_quad(fill(rect(ox, vy0.max(oy), 2.0, vy1 - vy0.max(oy)), a.sec_color.unwrap_or(th.ink_faint)));
        window.paint_quad(fill(rect(colx0 - 1.0, vy0.max(oy), 1.0, vy1 - vy0.max(oy)), th.line));

        for (i, m) in (sa..sb).enumerate() {
            let meas = &song.measures[m];
            let len = meas.cell_count();
            let y0 = oy + ys[i];
            let mh = len as f32 * rh;
            if y0 > vy1 || y0 + mh < vy0 {
                continue;
            }
            let k0 = (((vy0 - y0) / rh).floor().max(0.0)) as usize;
            let k1 = ((((vy1 - y0) / rh).ceil()).max(0.0) as usize).min(len);
            let sub = meas.subdivision.max(1);

            // ── gutter: faixa de loop (8 px) e números ──
            for (reg, alpha, handles) in [(a.range, if a.looping { 0.6 } else { 0.25 }, true), (a.loop_preview, 0.35, false)] {
                let Some(r) = reg else { continue };
                if m < r.start_bar || m >= r.end_bar {
                    continue;
                }
                window.paint_quad(fill(rect(ox + LOOP_X, y0, LOOP_W - 2.0, mh), Theme::alpha(th.accent2, alpha)));
                if handles && m == r.start_bar {
                    window.paint_quad(fill(rect(ox + LOOP_X, y0, LOOP_W - 2.0, 6.0), th.accent2));
                }
                if handles && m + 1 == r.end_bar {
                    window.paint_quad(fill(rect(ox + LOOP_X, y0 + mh - 6.0, LOOP_W - 2.0, 6.0), th.accent2));
                }
            }
            let prev_beats = (m > 0).then(|| song.measures[m - 1].beats);
            for k in k0..k1 {
                let y = y0 + k as f32 * rh;
                let gx = ox + LOOP_X + LOOP_W + 2.0;
                if k == 0 {
                    text(window, cx, &format!("c{}", m + 1).into(), 11, th.ink, 2, gx, ty(y, 11), None);
                } else if k % sub == 0 {
                    text(window, cx, &(k / sub + 1).to_string().into(), 11, th.ink, 2, gx + 4.0, ty(y, 11), None);
                } else {
                    let lbl: SharedString = match sub {
                        2 => "e".into(),
                        4 => ["", "e", "&", "a"][k % 4].into(),
                        _ => "·".into(),
                    };
                    text(window, cx, &lbl, 10, th.ink_faint, 1, gx + 4.0, ty(y, 10), None);
                }
                // fórmula nova (difere do compasso anterior): "5/4" em 9 px na 2ª linha
                if k == 1 && prev_beats.is_some_and(|p| p != meas.beats) {
                    text(window, cx, &format!("{}/4", meas.beats).into(), 9, th.ink_soft, 5, gx + 14.0, ty(y, 9), None);
                }
            }

            // ── coluna do compasso: dinâmica + < > (1ª linha), andamento (2ª), cunha ──
            let mx = ox + GUTTER_W;
            window.with_content_mask(Some(ContentMask { bounds: b.intersect(&rect(mx, oy, MEAS_W - 1.0, f32::from(b.size.height))) }), |window| {
                let ch = (rh - 6.0).min(20.0);
                let cy = y0 + (rh - ch) * 0.5;
                let dynamic = meas.dynamic.map(|d| d.as_str());
                window.paint_quad(rounded(rect(mx + 2.0, cy, 30.0, ch), R1, if dynamic.is_some() { th.accent2_wash } else { th.paper_raise }));
                text(window, cx, &dynamic.unwrap_or("·").into(), 11, if dynamic.is_some() { th.ink } else { th.ink_faint }, if dynamic.is_some() { 0 } else { 1 }, mx + 2.0, ty(y0, 11), Some(30.0));
                for (j, (glyph, kind)) in [("<", Hairpin::Cresc), (">", Hairpin::Dim)].into_iter().enumerate() {
                    let hx = mx + 34.0 + j as f32 * 18.0;
                    let on = meas.hairpin == Some(kind);
                    let bh = ch.min(16.0);
                    window.paint_quad(rounded(rect(hx, y0 + (rh - bh) * 0.5, 16.0, bh), R1, if on { th.accent2 } else { th.paper_raise }));
                    text(window, cx, &glyph.into(), 10, if on { th.on_accent } else { th.ink_faint }, if on { 4 } else { 1 }, hx, ty(y0, 10), Some(16.0));
                }
                if len > 1 {
                    let y1 = y0 + rh;
                    let tw = MEAS_W - 14.0;
                    let r = rect(mx + 2.0, y1 + (rh - ch) * 0.5, tw, ch);
                    match &meas.tempo {
                        Some(t) => {
                            window.paint_quad(rounded(r, R1, th.accent_wash));
                            let before = tempo_before.get(m).copied().unwrap_or(song.tempo);
                            let mut label = wadawko_core::ops::measures::tempo_mark_label(t, before);
                            if t.span > 1 {
                                label.push_str(&format!(" ·{}c", t.span));
                            }
                            text(window, cx, &label.into(), 10, th.ink, 0, mx + 2.0, ty(y1, 10), Some(tw));
                        }
                        None => {
                            window.paint_quad(rounded(rect(mx + 2.0, y1 + (rh - ch) * 0.5, 24.0, ch), R1, th.paper_raise));
                            text(window, cx, &"♩".into(), 12, th.ink_faint, 1, mx + 2.0, ty(y1, 12), Some(24.0));
                        }
                    }
                }
                if let Some(hp) = meas.hairpin {
                    vwedge(window, mx + MEAS_W - 7.0, y0 + rh * if len > 1 { 2.0 } else { 1.0 } + 2.0, y0 + mh - 4.0, hp, th.accent2);
                }
            });

            // ── colunas ──
            window.with_content_mask(Some(ContentMask { bounds: cols_mask }), |window| {
                for t in 0..n_tracks {
                    let track = &song.tracks[t];
                    let x = colx0 + cols.x[t] - off;
                    if x > ox + w || x + cols.group_w[t] < colx0 {
                        continue;
                    }
                    let audible = song.audible(t);
                    let shared = track.is_shared();
                    let has_people = !track.person_ids().is_empty();
                    let row = meas.cells.get(&track.id);
                    let krow = meas.kyai.as_ref().and_then(|k| k.get(&track.id));
                    let tdyn = meas.track_dynamic(&track.id);
                    for k in k0..k1 {
                        let y = y0 + k as f32 * rh;
                        let ih = rh - 1.0; // fresta de 1 px embaixo mostra o `bg`
                        let beat_start = k % sub == 0;
                        let cell = row.and_then(|r| r.get(k)).and_then(|c| c.as_deref());
                        let bg = if cell.is_some() {
                            th.cell_filled
                        } else if beat_start {
                            th.cell_beat
                        } else {
                            th.cell_empty
                        };
                        window.paint_quad(fill(rect(x, y, nw, ih), bg));
                        if beat_start && k > 0 {
                            window.paint_quad(fill(rect(x, y, cols.group_w[t], 1.0), th.line_beat));
                        }
                        let mut dim = !audible;
                        if let Some(word) = cell {
                            match phrase::resolve_cell(word) {
                                Some(res) => {
                                    // barra de golpe vertical (4 px), fatiada por instante (doko = 2)
                                    let mut groups: Vec<(f32, bool, f32)> = Vec::new();
                                    for h in &res.hits {
                                        let o = h.offset as f32;
                                        if let Some(g) = groups.iter_mut().find(|g| (g.0 - o).abs() < 1e-3) {
                                            g.2 = g.2.max(h.vel as f32);
                                        } else {
                                            groups.push((o, h.zone == Zone::Rim, h.vel as f32));
                                        }
                                    }
                                    for (gi, g) in groups.iter().enumerate() {
                                        let gy0 = y + g.0 * ih + 2.0;
                                        let gy1 = groups.get(gi + 1).map(|n| y + n.0 * ih - 1.0).unwrap_or(y + ih - 2.0);
                                        let col = if g.1 { th.accent2 } else { th.accent };
                                        let col = Theme::alpha(col, (0.45 + 0.55 * g.2.min(1.0)).min(1.0));
                                        window.paint_quad(fill(rect(x + 5.0, gy0, 4.0, (gy1 - gy0).max(2.0)), col));
                                    }
                                    if res.sfz {
                                        window.paint_quad(rounded(rect(x + nw - 7.0, y + 3.0, 4.0, 4.0), R0, th.accent));
                                    }
                                }
                                None => window.paint_quad(outline(rect(x, y, nw, ih), th.danger, BorderStyle::Solid)),
                            }
                            if has_people {
                                let owner = cell_owner_of(song, track, m, k);
                                if shared && let Some(col) = owner.and_then(|o| colors.get(o)) {
                                    window.paint_quad(fill(rect(x, y, 3.0, ih), *col));
                                }
                                if any_person_gate && !song.owner_audible(t, m, k) {
                                    dim = true;
                                }
                            }
                            let editing_here = a.lane == Lane::Cells && a.cursor == Some(Cur { m, t, k }) && !a.edit.is_empty();
                            if !editing_here {
                                text(window, cx, &SharedString::from(word.to_string()), syl_px, th.ink, 0, x + 14.0, ty(y, syl_px), None);
                            }
                        }
                        if dim {
                            window.paint_quad(fill(rect(x, y, nw, ih), Theme::alpha(th.bg, 0.6)));
                        }
                        // sub-coluna de dinâmica da linha: chip (1ª linha), < > (2ª)
                        if let Some(dx) = cols.dyn_x[t] {
                            let dxx = x + dx;
                            window.paint_quad(fill(rect(dxx, y, DYN_W, ih), Theme::alpha(th.paper_raise, 0.6)));
                            if k == 0 {
                                let d = tdyn.and_then(|d| d.dynamic).map(|d| d.as_str());
                                window.paint_quad(rounded(rect(dxx + 3.0, y + 3.0, DYN_W - 6.0, ih - 6.0), R1, if d.is_some() { th.accent2_wash } else { th.paper_raise }));
                                text(window, cx, &d.unwrap_or("·").into(), 11, if d.is_some() { th.ink } else { th.ink_faint }, if d.is_some() { 0 } else { 1 }, dxx, ty(y, 11), Some(DYN_W));
                            } else if k == 1 {
                                let hp = tdyn.and_then(|d| d.hairpin);
                                for (j, (glyph, kind)) in [("<", Hairpin::Cresc), (">", Hairpin::Dim)].into_iter().enumerate() {
                                    let hx = dxx + 1.0 + j as f32 * (DYN_W / 2.0);
                                    let on = hp == Some(kind);
                                    window.paint_quad(rounded(rect(hx, y + 4.0, DYN_W / 2.0 - 2.0, ih - 8.0), R1, if on { th.accent2 } else { th.paper_raise }));
                                    text(window, cx, &glyph.into(), 10, if on { th.on_accent } else { th.ink_faint }, if on { 4 } else { 1 }, hx, ty(y, 10), Some(DYN_W / 2.0 - 2.0));
                                }
                            }
                        }
                        // sub-coluna de kiai
                        if let Some(kx) = cols.kyai_x[t] {
                            let syl = krow.and_then(|r| r.get(k)).and_then(|c| c.as_deref());
                            window.paint_quad(fill(rect(x + kx + 1.0, y, KYAI_W - 1.0, ih), if syl.is_some() { th.accent_wash } else { Theme::alpha(th.paper_raise, 0.55) }));
                            let editing_here = a.lane == Lane::Kyai && a.cursor == Some(Cur { m, t, k }) && !a.edit.is_empty();
                            if let Some(syl) = syl
                                && !editing_here
                            {
                                text(window, cx, &SharedString::from(syl.to_string()), 11, th.accent, 3, x + kx, ty(y, 11), Some(KYAI_W));
                            }
                        }
                    }
                    // cunha da dinâmica da linha, ao longo do compasso
                    if let (Some(dx), Some(hp)) = (cols.dyn_x[t], tdyn.and_then(|d| d.hairpin)) {
                        vwedge(window, x + dx + DYN_W / 2.0, y0 + rh * 2.0 + 2.0, y0 + mh - 4.0, hp, th.accent2);
                    }
                }
            });
            // barra de compasso: 2 px `line_bar` atravessando gutter, compasso e colunas
            window.paint_quad(fill(rect(ox + LOOP_X + LOOP_W, y0, (colx0 + cols.w - off).min(ox + w) - (ox + LOOP_X + LOOP_W), 2.0), th.line_bar));
        }

        // ── seleção e destino do arrasto, cursor, buffer ──
        window.with_content_mask(Some(ContentMask { bounds: cols_mask }), |window| {
            let kyai_lane = a.lane == Lane::Kyai;
            let span_x = |t0: usize, t1: usize| -> (f32, f32) {
                match (kyai_lane, cols.kyai_x[t0], cols.kyai_x[t1]) {
                    (true, Some(k0), Some(k1)) => (colx0 + cols.x[t0] + k0 - off, colx0 + cols.x[t1] + k1 + KYAI_W - off),
                    _ => (colx0 + cols.x[t0] - off, colx0 + cols.x[t1] + nw - off),
                }
            };
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
                let (x0, x1) = span_x(t0, t1);
                let mut acc = song.flat(sa, 0);
                for (i, m) in (sa..sb).enumerate() {
                    let len = song.mlen(m);
                    if len == 0 {
                        continue;
                    }
                    let (lo, hi) = (g0.max(acc), g1.min(acc + len - 1));
                    if lo <= hi {
                        let y0 = oy + ys[i];
                        let r = rect(x0, y0 + (lo - acc) as f32 * rh, x1 - x0, (hi - lo + 1) as f32 * rh);
                        window.paint_quad(fill(r, Theme::alpha(col, alpha)));
                        window.paint_quad(outline(r, col, BorderStyle::Solid));
                    }
                    acc += len;
                }
            }

            if let Some(c) = a.cursor
                && c.m >= sa
                && c.m < sb
                && c.t < n_tracks
            {
                let y = oy + ys[c.m - sa] + c.k as f32 * rh;
                let (x, cw) = match (kyai_lane, cols.kyai_x[c.t]) {
                    (true, Some(kx)) => (colx0 + cols.x[c.t] + kx - off, KYAI_W),
                    _ => (colx0 + cols.x[c.t] - off, nw),
                };
                let r = rect(x, y, cw, rh);
                let wash = if matches!(a.vmode, VMode::Normal | VMode::Visual | VMode::VLine) { Theme::alpha(th.accent, 0.18) } else { gpui_kit::transparent_black() };
                window.paint_quad(quad(r, px(R0), wash, px(2.0), th.accent, BorderStyle::Solid));
                if !a.edit.is_empty() {
                    let er = rect(x, y, (a.edit.len() as f32 * 8.5 + 16.0).max(cw), rh);
                    window.paint_quad(quad(er, px(R0), th.accent_wash, px(2.0), th.accent, BorderStyle::Solid));
                    let shown: SharedString = format!("{}▏", a.edit).into();
                    let mut run = window.text_style().to_run(shown.len());
                    run.color = th.ink;
                    let line = window.text_system().shape_line(shown, px(13.0), &[run], None);
                    let _ = line.paint(point(px(x + 7.0), px(y + (rh - 17.0) * 0.5)), px(17.0), TextAlign::Left, None, window, cx);
                }
            }
        });
    });
}
