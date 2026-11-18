//! `engrave`: `Song` → [`Score`]. Sem Formatter: todas as linhas dividem a mesma grade,
//! então `x = x_compasso + (célula + offset) · largura_da_célula`.
//!
//! Geometria vertical de uma linha (em staff spaces, a partir da linha da pauta):
//! mão direita no espaço acima (−0,5), esquerda no espaço abaixo (+0,5), ponta das hastes em
//! −4 (beams horizontais ali), acento acima da haste, `sfz`/quiáltera acima do acento;
//! abaixo: sílaba, kiai, dinâmica da linha.

use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;

use wadawko_core::ops::people::{cell_owner_of, person_audible, person_color};
use wadawko_core::ops::sections::section_ranges;
use wadawko_core::{Dynamic, Hairpin, Song};
use wadawko_draw::smufl::{FAMILY_SHIPPORI_MINCHO, FAMILY_ZEN_KAKU, SmuflGlyph, Sp, glyphs};
use wadawko_draw::{Color, DrawCmd, Path, Quad, Rect, StandardFonts, Stroke, TextAlign, TextStyle, Transform, pt, rect};

use crate::options::{EngraveOptions, LayoutMode, Theme, parse_hex_color};
use crate::rhythm::{EventKind, TrackRhythm, measure_rhythm};
use crate::score::{BeamInfo, HitGfx, MeasureLayout, PageLayout, Score, SystemLayout};

const FLAG_64TH_UP: SmuflGlyph = SmuflGlyph { name: "flag64thUp", codepoint: '\u{E246}' };
const REST_64TH: SmuflGlyph = SmuflGlyph { name: "rest64th", codepoint: '\u{E4E9}' };

/// opacidade da nota cujo dono está mudo (submix silenciado)
const MUTED_ALPHA: f32 = 0.3;

/// Estatísticas de uma linha na música inteira (decidem a altura da faixa).
#[derive(Clone, Copy, Default)]
struct TrackStat {
    /// tem sfz ou quiáltera (precisa de mais espaço acima)
    tall: bool,
    max_flags: u8,
    kyai: bool,
    dynamics: bool,
}

/// Geometria vertical de uma linha, relativa ao topo da faixa dela.
#[derive(Clone, Copy)]
struct RowMetrics {
    line: f32,
    height: f32,
    kyai_y: f32,
    dyn_y: f32,
}

#[derive(Clone, Default)]
struct MeasureMeta {
    cells: usize,
    cw: f32,
    pad_l: f32,
    pad_r: f32,
    start_repeat: bool,
    end_repeat: Option<u32>,
    section: Option<String>,
}

impl MeasureMeta {
    fn width(&self) -> f32 {
        self.pad_l + self.cells as f32 * self.cw + self.pad_r
    }
}

/// trecho de hairpin: (tipo, posição dentro do segmento, tamanho do segmento)
type HairSeg = Option<(Hairpin, usize, usize)>;

fn hairpin_segments(n: usize, hp_of: impl Fn(usize) -> Option<Hairpin>, dyn_of: impl Fn(usize) -> Option<Dynamic>) -> Vec<HairSeg> {
    let mut out = vec![None; n];
    let mut u = 0;
    while u < n {
        let Some(hp) = hp_of(u) else {
            u += 1;
            continue;
        };
        let mut v = u;
        while v + 1 < n && hp_of(v + 1) == Some(hp) && dyn_of(v + 1).is_none() {
            v += 1;
        }
        for (i, slot) in out.iter_mut().enumerate().take(v + 1).skip(u) {
            *slot = Some((hp, i - u, v - u + 1));
        }
        u = v + 1;
    }
    out
}

struct Engraver<'a> {
    song: &'a Song,
    fonts: &'a StandardFonts,
    theme: Theme,
    opts: &'a EngraveOptions,
    sp: f32,
    rhythms: Vec<Vec<TrackRhythm>>,
    stats: Vec<Vec<TrackStat>>,
    metas: Vec<MeasureMeta>,
    cell_base: Vec<usize>,
    global_hair: Vec<HairSeg>,
    track_hair: Vec<Vec<HairSeg>>,
    /// andamento que vinha soando antes de cada compasso (rótulo rall./accel.)
    tempo_before: Vec<f64>,
    head_h: f32,
}

/// Engrava a música: calcula o layout inteiro e os comandos de desenho, sem pintar.
///
/// As fontes são as de [`StandardFonts`] (Bravura + Zen Kaku + Shippori Mincho); o canvas
/// usado depois em [`Score::paint`] precisa conhecer o mesmo `FontBook` (os comandos de
/// glifo carregam o `FontId` da Bravura).
pub fn engrave(song: &Song, opts: &EngraveOptions, fonts: &StandardFonts) -> Score {
    Engraver::new(song, opts, fonts).run()
}

impl<'a> Engraver<'a> {
    fn new(song: &'a Song, opts: &'a EngraveOptions, fonts: &'a StandardFonts) -> Self {
        let z = opts.zoom();
        let sp = opts.staff_space.max(1.0) * z;
        let n = song.measures.len();
        let rhythms: Vec<Vec<TrackRhythm>> =
            song.measures.iter().map(|m| song.tracks.iter().map(|t| measure_rhythm(m, &t.id)).collect()).collect();

        // estatísticas por linha → altura de cada faixa
        let mut stats = vec![vec![TrackStat::default(); song.tracks.len()]; n];
        for (mi, meas) in song.measures.iter().enumerate() {
            for (ti, track) in song.tracks.iter().enumerate() {
                let st = &mut stats[mi][ti];
                let r = &rhythms[mi][ti];
                st.tall |= !r.tuplets.is_empty();
                for ev in &r.events {
                    if let EventKind::Note { hits, .. } = &ev.kind {
                        st.max_flags = st.max_flags.max(ev.flags);
                        st.tall |= hits.iter().any(|h| h.sfz);
                    }
                }
                st.kyai |= (0..meas.cell_count()).any(|ci| meas.kyai_at(&track.id, ci).is_some_and(|k| !k.is_empty()));
                st.dynamics |= meas.track_dynamic(&track.id).is_some_and(|d| d.dynamic.is_some() || d.hairpin.is_some());
            }
        }
        // seções: ritornello e nomes
        let mut metas = vec![MeasureMeta::default(); n];
        for r in section_ranges(song) {
            if r.start < n {
                metas[r.start].section = Some(r.name.clone());
            }
            let times = r.repeat.unwrap_or(1);
            if times >= 2
                && let Some(last) = r.last().filter(|&l| l < n)
            {
                metas[r.start].start_repeat = true;
                metas[last].end_repeat = Some(times);
            }
        }
        let mut cell_base = Vec::with_capacity(n);
        let mut acc = 0;
        for (mi, meas) in song.measures.iter().enumerate() {
            let m = &mut metas[mi];
            m.cells = meas.cell_count();
            m.cw = (opts.beat_width / meas.subdivision.max(1) as f32).max(opts.min_cell_width) * z;
            m.pad_l = (1.3 + if m.start_repeat { 1.6 } else { 0.0 }) * sp;
            m.pad_r = (0.4 + if m.end_repeat.is_some() { 1.7 } else { 0.0 } + if mi + 1 == n { 0.9 } else { 0.0 }) * sp;
            cell_base.push(acc);
            acc += m.cells;
        }

        let global_hair = hairpin_segments(n, |i| song.measures[i].hairpin, |i| song.measures[i].dynamic);
        let track_hair = song
            .tracks
            .iter()
            .map(|t| {
                hairpin_segments(
                    n,
                    |i| song.measures[i].track_dynamic(&t.id).and_then(|d| d.hairpin),
                    |i| song.measures[i].track_dynamic(&t.id).and_then(|d| d.dynamic),
                )
            })
            .collect();

        Engraver {
            song,
            fonts,
            theme: opts.theme,
            opts,
            sp,
            rhythms,
            stats,
            metas,
            cell_base,
            global_hair,
            track_hair,
            tempo_before: if song.measures.iter().any(|m| m.tempo.is_some()) { wadawko_core::ops::measures::tempo_at_measure_starts(song) } else { Vec::new() },
            head_h: 2.3 * sp,
        }
    }

    /// Altura de cada faixa num sistema: só reserva espaço para sfz/quiáltera, kiai e
    /// dinâmica da linha se algum compasso do sistema usa.
    fn rows_for(&self, plan: &std::ops::Range<usize>) -> Vec<RowMetrics> {
        let sp = self.sp;
        (0..self.song.tracks.len())
            .map(|ti| {
                let mut st = TrackStat::default();
                for row in &self.stats[plan.clone()] {
                    let s = row[ti];
                    st.tall |= s.tall;
                    st.max_flags = st.max_flags.max(s.max_flags);
                    st.kyai |= s.kyai;
                    st.dynamics |= s.dynamics;
                }
                let above = 5.7 + if st.tall { 1.9 } else { 0.0 } + 0.75 * f32::from(st.max_flags.saturating_sub(2));
                let mut below = 3.3;
                let kyai_y = below + 1.0;
                if st.kyai {
                    below += 1.5;
                }
                let dyn_y = below + 1.2;
                if st.dynamics {
                    below += 2.5;
                }
                RowMetrics { line: above * sp, height: (above + below + 0.4) * sp, kyai_y: kyai_y * sp, dyn_y: dyn_y * sp }
            })
            .collect()
    }

    // ───────────── helpers de texto/glifo ─────────────

    fn text_style(&self, size: f32) -> TextStyle {
        TextStyle::new(FAMILY_ZEN_KAKU, size)
    }

    fn text_width(&self, text: &str, style: &TextStyle) -> f32 {
        self.fonts.book.measure_text(text, style).map_or(text.chars().count() as f32 * style.size * 0.55, |m| m.width)
    }

    /// corta o texto com "…" para caber em `max_w`
    fn fit(&self, text: &str, style: &TextStyle, max_w: f32) -> String {
        if self.text_width(text, style) <= max_w {
            return text.to_string();
        }
        let mut chars: Vec<char> = text.chars().collect();
        while chars.pop().is_some() {
            let s: String = chars.iter().collect::<String>() + "…";
            if self.text_width(&s, style) <= max_w {
                return s;
            }
        }
        String::new()
    }

    fn glyph(&self, cmds: &mut Vec<DrawCmd>, g: SmuflGlyph, x: f32, y: f32, scale: f32, color: Color) -> u32 {
        cmds.push(DrawCmd::Glyph { font: self.fonts.bravura.font, codepoint: g.codepoint, origin: pt(x, y), size: 4.0 * self.sp * scale, color });
        (cmds.len() - 1) as u32
    }

    /// (esquerda, direita, topo, pé) da caixa do glifo em staff spaces, y para baixo
    fn bbox(&self, g: SmuflGlyph) -> (f32, f32, f32, f32) {
        match self.fonts.bravura.metadata.bbox(g.name) {
            Some(b) => (b.sw.x, b.ne.x, -b.ne.y, -b.sw.y),
            None => (0.0, 1.2, -0.5, 0.5),
        }
    }

    fn quad(cmds: &mut Vec<DrawCmd>, r: Rect, color: Color) -> u32 {
        cmds.push(DrawCmd::Quad(Quad::fill(r, color)));
        (cmds.len() - 1) as u32
    }

    fn text(cmds: &mut Vec<DrawCmd>, text: &str, x: f32, y: f32, style: TextStyle, color: Color) -> u32 {
        cmds.push(DrawCmd::Text { text: text.to_string(), origin: pt(x, y), style, color });
        (cmds.len() - 1) as u32
    }

    /// Texto com vogais longas (ā ē ī ō ū): as fontes embutidas não têm esses glifos
    /// ("Chū", "Ōdaiko" sairiam com buraco), então escreve a vogal simples e desenha o
    /// mácron por cima.
    fn text_macron(&self, cmds: &mut Vec<DrawCmd>, text: &str, x: f32, y: f32, style: TextStyle, color: Color) {
        let font = self.fonts.book.resolve_style(&style);
        let missing = |c: char| demacron(c).is_some() && font.is_some_and(|f| !self.fonts.book.has_glyph(f, c));
        if !text.chars().any(missing) {
            Self::text(cmds, text, x, y, style, color);
            return;
        }
        let plain: String = text.chars().map(|c| if missing(c) { demacron(c).unwrap_or(c) } else { c }).collect();
        let left = TextStyle { align: TextAlign::Left, ..style.clone() };
        let total = self.text_width(&plain, &left);
        let start = match style.align {
            TextAlign::Left => x,
            TextAlign::Center => x - total / 2.0,
            TextAlign::Right => x - total,
        };
        let m = font.and_then(|f| self.fonts.book.metrics(f, style.size));
        let mut prefix = String::new();
        for c in text.chars() {
            if missing(c) {
                let base = demacron(c).unwrap_or(c);
                let x0 = start + self.text_width(&prefix, &left);
                let w = self.text_width(&base.to_string(), &left);
                let h = m.map_or(style.size * 0.6, |m| if base.is_uppercase() { m.cap_height } else { m.x_height });
                let t = style.size * if style.weight.0 >= 600 { 0.085 } else { 0.06 };
                Self::quad(cmds, rect(x0 + w * 0.18, y - h - style.size * 0.1 - t, w * 0.64, t), color);
                prefix.push(base);
            } else {
                prefix.push(c);
            }
        }
        Self::text(cmds, &plain, x, y, style, color);
    }

    // ───────────── layout geral ─────────────

    fn run(mut self) -> Score {
        let sp = self.sp;
        let song = self.song;
        let n = song.measures.len();
        let paged = matches!(self.opts.mode, LayoutMode::Paged(_));
        let show_title = self.opts.show_title.unwrap_or(paged);

        // coluna da esquerda: trilhos de pessoa + nomes das linhas
        let people: Vec<_> = song.people().iter().filter(|p| song.tracks.iter().any(|t| t.person_ids().contains(&p.id))).collect();
        let label_style = self.text_style(1.3 * sp).bold();
        let (rails_w, label_w) = if self.opts.show_labels && !song.tracks.is_empty() {
            let rails = if people.is_empty() { 0.0 } else { people.len() as f32 * 1.4 * sp + 0.5 * sp };
            let widest = song.tracks.iter().map(|t| self.text_width(&t.name, &label_style)).fold(0.0, f32::max);
            (rails, widest.min(17.0 * sp) + 1.4 * sp)
        } else {
            (0.0, 0.0)
        };
        let left_w = rails_w + label_w + 0.9 * sp;

        let sys_gap = 1.6 * sp;

        // quebra em sistemas
        let (margin, page_w, page_h) = match self.opts.mode {
            LayoutMode::Continuous => (sp, None, None),
            LayoutMode::Paged(p) => (p.margin, Some(p.width), Some(p.height)),
        };
        let mut plans: Vec<std::ops::Range<usize>> = Vec::new();
        match page_w {
            None => plans.push(0..n),
            Some(pw) => {
                let avail = (pw - 2.0 * margin - left_w).max(10.0 * sp);
                let mut start = 0;
                let mut used = 0.0;
                for mi in 0..n {
                    let w = self.metas[mi].width();
                    if mi > start && used + w > avail {
                        plans.push(start..mi);
                        start = mi;
                        used = 0.0;
                    }
                    used += w;
                }
                if start < n || n == 0 {
                    plans.push(start..n);
                }
                // justifica: distribui a sobra pelas células (o último sistema só se faltar pouco)
                let last = plans.len() - 1;
                for (si, plan) in plans.iter().enumerate() {
                    let cells: usize = plan.clone().map(|mi| self.metas[mi].cells).sum();
                    let natural: f32 = plan.clone().map(|mi| self.metas[mi].width()).sum();
                    if cells == 0 {
                        continue;
                    }
                    let extra = (avail - natural) / cells as f32;
                    let avg_cw = plan.clone().map(|mi| self.metas[mi].cw * self.metas[mi].cells as f32).sum::<f32>() / cells as f32;
                    if si == last && extra > 0.35 * avg_cw {
                        continue;
                    }
                    for mi in plan.clone() {
                        self.metas[mi].cw = (self.metas[mi].cw + extra).max(0.4 * sp);
                    }
                }
            }
        }

        // título (só na 1ª página)
        let title_size = (2.4 * sp).max(14.0);
        let title_h = if show_title { title_size * 1.7 } else { 0.0 };

        // posiciona sistemas em páginas
        let mut pages: Vec<PageLayout> = Vec::new();
        let mut systems: Vec<SystemLayout> = Vec::new();
        let mut measures: Vec<MeasureLayout> = Vec::with_capacity(n);
        let mut hit_index = HashMap::new();
        let mut y = margin + title_h;
        let mut page_start = 0usize;
        let mut content_w: f32 = 0.0;
        for plan in plans {
            let row_metrics = self.rows_for(&plan);
            let rows_h: f32 = row_metrics.iter().map(|r| r.height).sum();
            let has_global_dyn = plan.clone().any(|mi| song.measures[mi].dynamic.is_some() || self.global_hair[mi].is_some());
            let sys_h = self.head_h + rows_h + if has_global_dyn { 2.8 * sp } else { 0.0 } + 0.4 * sp;
            if let Some(ph) = page_h
                && systems.len() > page_start
                && y + sys_h > ph - margin
            {
                pages.push(PageLayout { width: page_w.unwrap_or(0.0), height: ph, systems: page_start..systems.len(), cmds: Vec::new() });
                page_start = systems.len();
                y = margin;
            }
            let page = pages.len();
            let si = systems.len();
            let staff_x = margin + left_w;
            let mut row_y = y + self.head_h;
            let mut staff_ys = Vec::new();
            let mut rows = Vec::new();
            for r in &row_metrics {
                staff_ys.push(row_y + r.line);
                rows.push((row_y, row_y + r.height));
                row_y += r.height;
            }
            let (bar_top, bar_bottom) = match (staff_ys.first(), staff_ys.last()) {
                (Some(a), Some(b)) => (a - 2.0 * sp, b + 2.0 * sp),
                _ => (y + self.head_h, y + self.head_h),
            };
            let global_dyn_y = row_y + 1.4 * sp;
            let mut x = staff_x;
            let m0 = measures.len();
            for mi in plan.clone() {
                let w = self.metas[mi].width();
                let geo = MeasureGeo { row_metrics: &row_metrics, x, w, top: y, bottom: y + sys_h, staff_ys: &staff_ys, rows: &rows, bar_top, bar_bottom, global_dyn_y, last_in_system: mi + 1 == plan.end };
                let (mut m, keys) = self.engrave_measure(mi, &geo);
                m.system = si;
                m.page = page;
                for (hi, keys) in keys.into_iter().enumerate() {
                    for (t, c, h) in keys {
                        hit_index.insert((t, c, h), (mi as u32, hi as u32));
                    }
                }
                measures.push(m);
                x += w;
            }
            content_w = content_w.max(x);
            let bracket = !self.metas.get(plan.start).is_some_and(|m| m.start_repeat);
            let cmds = self.system_cmds(bracket, margin, rails_w, label_w, staff_x, &people, &staff_ys, &rows, bar_top, bar_bottom, &label_style);
            let right = if let Some(pw) = page_w { pw - margin } else { x };
            systems.push(SystemLayout {
                page,
                bounds: rect(margin, y, right - margin, sys_h),
                staff_x,
                measures: m0..measures.len(),
                staff_ys,
                rows,
                bar_top,
                bar_bottom,
                cmds,
            });
            y += sys_h + sys_gap;
        }
        let (last_w, last_h) = match (page_w, page_h) {
            (Some(w), Some(h)) => (w, h),
            _ => (content_w + margin, y - sys_gap + margin),
        };
        pages.push(PageLayout { width: last_w, height: last_h, systems: page_start..systems.len(), cmds: Vec::new() });

        // título e rodapé
        if show_title {
            let total = pages.len();
            for (pi, page) in pages.iter_mut().enumerate() {
                if pi == 0 {
                    let st = TextStyle::new(FAMILY_SHIPPORI_MINCHO, title_size).bold();
                    self.text_macron(&mut page.cmds, &song.title, margin, margin + title_size, st, self.theme.ink);
                    let tempo = format!("{} BPM", fmt_num(song.tempo));
                    let st = TextStyle::new(FAMILY_ZEN_KAKU, title_size * 0.5).align(TextAlign::Right);
                    Self::text(&mut page.cmds, &tempo, page.width - margin, margin + title_size, st, self.theme.ink_soft);
                }
                if paged {
                    let st = TextStyle::new(FAMILY_ZEN_KAKU, 9.0).align(TextAlign::Center);
                    let foot = format!("{} — {}/{}", song.title, pi + 1, total);
                    self.text_macron(&mut page.cmds, &foot, page.width / 2.0, page.height - margin * 0.45, st, self.theme.ink_soft);
                }
            }
        }

        Score {
            mode: self.opts.mode,
            theme: self.theme,
            staff_space: sp,
            pages,
            systems,
            measures,
            track_ids: song.tracks.iter().map(|t| t.id.clone()).collect(),
            hit_index,
            slop: 14.0 * sp,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn system_cmds(
        &self,
        bracket: bool,
        margin: f32,
        rails_w: f32,
        label_w: f32,
        staff_x: f32,
        people: &[&wadawko_core::Person],
        staff_ys: &[f32],
        rows: &[(f32, f32)],
        bar_top: f32,
        bar_bottom: f32,
        label_style: &TextStyle,
    ) -> Vec<DrawCmd> {
        let sp = self.sp;
        let mut cmds = Vec::new();
        if self.song.tracks.is_empty() {
            return cmds;
        }
        if self.opts.show_labels {
            // trilhos: uma coluna por pessoa, acesa nas linhas dela
            for (pi, person) in people.iter().enumerate() {
                let color = person.color.as_deref().and_then(parse_hex_color).unwrap_or(Color::rgb(0x888888));
                let x = margin + pi as f32 * 1.4 * sp;
                let mut named = false;
                for (ti, track) in self.song.tracks.iter().enumerate() {
                    if !track.person_ids().contains(&person.id) {
                        continue;
                    }
                    let (top, bottom) = rows[ti];
                    let seg = rect(x, top + 0.3 * sp, 1.1 * sp, bottom - top - 0.6 * sp);
                    cmds.push(DrawCmd::Quad(Quad::fill(seg, color).radius(0.3 * sp)));
                    if !named {
                        named = true;
                        // nome na vertical (lê de baixo para cima), dentro do 1º trecho
                        let st = self.text_style(0.85 * sp).bold();
                        let name = self.fit(&person.name, &st, seg.h - 0.8 * sp);
                        cmds.push(DrawCmd::Save);
                        cmds.push(DrawCmd::Transform(Transform::translate(x + 0.86 * sp, seg.bottom() - 0.4 * sp)));
                        cmds.push(DrawCmd::Transform(Transform::rotate(-FRAC_PI_2)));
                        Self::text(&mut cmds, &name, 0.0, 0.0, st, Color::WHITE);
                        cmds.push(DrawCmd::Restore);
                    }
                }
            }
            for (ti, track) in self.song.tracks.iter().enumerate() {
                let name = self.fit(&track.name, label_style, label_w - 1.0 * sp);
                self.text_macron(&mut cmds, &name, margin + rails_w, staff_ys[ti] + 0.45 * sp, label_style.clone(), self.theme.ink);
            }
        }
        // colchete do sistema
        if bracket && self.song.tracks.len() > 1 {
            Self::quad(&mut cmds, rect(staff_x - 0.6 * sp, bar_top, 0.32 * sp, bar_bottom - bar_top), self.theme.ink);
        }
        let _ = label_w;
        cmds
    }

    // ───────────── um compasso ─────────────

    fn engrave_measure(&self, mi: usize, geo: &MeasureGeo) -> (MeasureLayout, Vec<Vec<(u32, u32, u16)>>) {
        let sp = self.sp;
        let song = self.song;
        let meas = &song.measures[mi];
        let meta = &self.metas[mi];
        let ink = self.theme.ink;
        let d = self.fonts.bravura.defaults();
        let (thin, thick, staff_t) = (d.thin_barline_thickness * sp, d.thick_barline_thickness * sp, d.staff_line_thickness * sp);
        let gap = d.thin_thick_barline_separation * sp;
        let (x0, x1) = (geo.x, geo.x + geo.w);
        let notes_x = x0 + meta.pad_l;
        let cw = meta.cw;
        let mut cmds: Vec<DrawCmd> = Vec::new();
        let mut hits: Vec<HitGfx> = Vec::new();
        let mut keys: Vec<Vec<(u32, u32, u16)>> = Vec::new();
        let mut beams: Vec<BeamInfo> = Vec::new();
        let n_tracks = song.tracks.len();

        // linhas da pauta
        for &ly in geo.staff_ys {
            Self::quad(&mut cmds, rect(x0, ly - staff_t / 2.0, geo.w, staff_t), self.theme.staff);
        }

        // barras: esquerda (ou sinal de repetição), direita no fim do sistema, final
        if n_tracks > 0 {
            let (top, h) = (geo.bar_top, geo.bar_bottom - geo.bar_top);
            let dots = |cmds: &mut Vec<DrawCmd>, cx: f32| {
                for &ly in geo.staff_ys {
                    for dy in [-0.5, 0.5] {
                        cmds.push(DrawCmd::FillPath { path: Path::circle(pt(cx, ly + dy * sp), 0.2 * sp), color: ink });
                    }
                }
            };
            if meta.start_repeat {
                Self::quad(&mut cmds, rect(x0, top, thick, h), ink);
                Self::quad(&mut cmds, rect(x0 + thick + gap, top, thin, h), ink);
                dots(&mut cmds, x0 + thick + gap + thin + 0.45 * sp);
            } else {
                Self::quad(&mut cmds, rect(x0 - thin / 2.0, top, thin, h), ink);
            }
            let is_last = mi + 1 == song.measures.len();
            // fim de repetição colado no início da próxima: a barra grossa é uma só (a dela)
            let shared = !geo.last_in_system && self.metas.get(mi + 1).is_some_and(|m| m.start_repeat);
            if meta.end_repeat.is_some() && shared {
                Self::quad(&mut cmds, rect(x1 - gap - thin, top, thin, h), ink);
                dots(&mut cmds, x1 - gap - thin - 0.45 * sp);
            } else if meta.end_repeat.is_some() {
                Self::quad(&mut cmds, rect(x1 - thick, top, thick, h), ink);
                Self::quad(&mut cmds, rect(x1 - thick - gap - thin, top, thin, h), ink);
                dots(&mut cmds, x1 - thick - gap - thin - 0.45 * sp);
            } else if is_last {
                Self::quad(&mut cmds, rect(x1 - thick, top, thick, h), ink);
                Self::quad(&mut cmds, rect(x1 - thick - gap - thin, top, thin, h), ink);
            } else if geo.last_in_system {
                Self::quad(&mut cmds, rect(x1 - thin / 2.0, top, thin, h), ink);
            }
        }

        // cabeçalho: nº do compasso, nome da seção, ×N
        let head_y = geo.top + 1.55 * sp;
        let mut hx = x0 + 0.3 * sp;
        if self.opts.measure_numbers {
            let st = self.text_style(0.95 * sp);
            let num = (mi + 1).to_string();
            let w = self.text_width(&num, &st);
            Self::text(&mut cmds, &num, hx, head_y, st, self.theme.ink_soft);
            hx += w + 0.7 * sp;
        }
        if let Some(name) = &meta.section {
            let st = self.text_style(1.25 * sp).bold();
            self.text_macron(&mut cmds, name, hx, head_y, st, ink);
        }
        let mut right = x1 - 1.6 * sp;
        if let Some(times) = meta.end_repeat {
            let st = self.text_style(1.25 * sp).bold().align(TextAlign::Right);
            let label = format!("×{times}");
            right -= self.text_width(&label, &st) + 0.8 * sp;
            Self::text(&mut cmds, &label, x1 - 1.6 * sp, head_y, st, ink);
        }
        // mudança de andamento que começa aqui: "rall. → 90", "accel. → 130", "♩ = 100"
        if let Some(t) = &meas.tempo {
            let before = self.tempo_before.get(mi).copied().unwrap_or(song.tempo);
            let st = self.text_style(1.05 * sp).bold().align(TextAlign::Right);
            Self::text(&mut cmds, &wadawko_core::ops::measures::tempo_mark_label(t, before), right, head_y, st, self.theme.ink_soft);
        }

        // dinâmica geral (faixa ao pé do sistema)
        if meas.dynamic.is_some() || self.global_hair[mi].is_some() {
            self.dynamics(&mut cmds, meas.dynamic, self.global_hair[mi], geo.global_dyn_y, notes_x, x0, x1, ink);
        }

        for (ti, track) in song.tracks.iter().enumerate() {
            let ly = geo.staff_ys[ti];
            let row_top = geo.rows[ti].0;
            let rm = &geo.row_metrics[ti];
            let rhythm = &self.rhythms[mi][ti];
            let events = &rhythm.events;
            let shared = track.is_shared();
            let x_of = |pos: f32| notes_x + pos * cw;

            // cor por célula (dono em instrumento compartilhado)
            let cell_color = |ci: usize| -> Color {
                if !shared {
                    return ink;
                }
                let owner = cell_owner_of(song, track, mi, ci);
                let base = person_color(song, owner).and_then(parse_hex_color).unwrap_or(ink);
                if person_audible(song, owner) { base } else { base.fade(MUTED_ALPHA) }
            };

            // ponta da haste por evento (igual dentro de um grupo de beam)
            let top_for = |flags: u8| ly - (4.0 + 0.75 * f32::from(flags.saturating_sub(2))) * sp;
            let mut stem_top: Vec<f32> = events.iter().map(|e| top_for(e.flags)).collect();
            let mut in_beam = vec![false; events.len()];
            for b in &rhythm.beams {
                let top = top_for(events[b.first..=b.last].iter().map(|e| e.flags).max().unwrap_or(1));
                for i in b.first..=b.last {
                    stem_top[i] = top;
                    in_beam[i] = true;
                }
            }

            // ── sílabas (antes das notas: os golpes guardam o índice do texto) ──
            let syl_size = 1.1 * sp;
            let syl_y = ly + 2.75 * sp;
            let head_w = 1.18 * sp;
            let mut label_cmd: Vec<Option<u32>> = vec![None; events.len()];
            let mut i = 0;
            while i < events.len() {
                let cell = events[i].cell;
                let mut j = i;
                while j + 1 < events.len() && events[j + 1].cell == cell {
                    j += 1;
                }
                let color = cell_color(cell);
                let st = self.text_style(syl_size).align(TextAlign::Center);
                let items: Vec<(usize, &str, f32, f32)> = (i..=j)
                    .filter_map(|k| events[k].label.as_deref().map(|l| (k, l, x_of(events[k].pos) + head_w / 2.0, self.text_width(l, &st))))
                    .collect();
                let overlap = items.windows(2).any(|w| w[0].2 + w[0].3 / 2.0 + 0.1 * sp > w[1].2 - w[1].3 / 2.0);
                let is_roll = (i..=j).any(|k| matches!(events[k].kind, EventKind::Note { roll, .. } if roll > 0));
                let word = rhythm.words.get(cell).and_then(|w| w.as_deref());
                if let Some(word) = word.filter(|_| overlap || is_roll) {
                    // não cabe golpe a golpe: a palavra inteira, encolhida para caber na célula
                    let mut st = self.text_style(syl_size);
                    let w = self.text_width(word, &st);
                    let avail = (cw - 0.3 * sp).max(sp);
                    if w > avail {
                        st.size = (syl_size * avail / w).max(syl_size * 0.6);
                    }
                    let idx = Self::text(&mut cmds, word, x_of(cell as f32), syl_y, st, color);
                    (i..=j).filter(|&k| events[k].is_note()).for_each(|k| label_cmd[k] = Some(idx));
                } else {
                    for (k, label, xc, _) in items {
                        let c = if events[k].is_note() { color } else { self.theme.ink_soft };
                        label_cmd[k] = Some(Self::text(&mut cmds, label, xc, syl_y, st.clone(), c));
                    }
                }
                i = j + 1;
            }

            // ── kiai: itálico vermelho embaixo da sílaba ──
            for ci in 0..meta.cells {
                if let Some(k) = meas.kyai_at(&track.id, ci).filter(|k| !k.is_empty()) {
                    let shout = k.ends_with('!');
                    let mut st = self.text_style(if shout { 1.2 * sp } else { 1.1 * sp }).italic();
                    if shout {
                        st = st.bold();
                    }
                    Self::text(&mut cmds, k, x_of(ci as f32), row_top + rm.line + rm.kyai_y, st, self.theme.kyai);
                }
            }

            // ── notas e pausas ──
            // por evento: (x direito da haste, x esquerdo da cabeça) — para beams/quiálteras
            let mut stem_x: Vec<Option<(f32, f32)>> = vec![None; events.len()];
            let mut has_accent = vec![false; events.len()];
            let stem_w = d.stem_thickness * sp;
            for (ei, ev) in events.iter().enumerate() {
                let x = x_of(ev.pos);
                match &ev.kind {
                    EventKind::Rest { whole } => {
                        if *whole {
                            let (l, r, ..) = self.bbox(glyphs::REST_WHOLE);
                            let cx = notes_x + meta.cells as f32 * cw / 2.0;
                            self.glyph(&mut cmds, glyphs::REST_WHOLE, cx - (l + r) / 2.0 * sp, ly, 1.0, ink);
                        } else {
                            let g = match ev.flags {
                                0 => glyphs::REST_QUARTER,
                                1 => glyphs::REST_8TH,
                                2 => glyphs::REST_16TH,
                                3 => glyphs::REST_32ND,
                                _ => REST_64TH,
                            };
                            self.glyph(&mut cmds, g, x, ly, 1.0, ink);
                        }
                    }
                    EventKind::Note { hits: nhits, roll } => {
                        let color = cell_color(ev.cell);
                        let top = stem_top[ei];
                        let mut shared_cmds: Vec<u32> = Vec::new();
                        let mut head_cmds: Vec<(u32, Rect, bool)> = Vec::new();
                        let mut stem_right = x + head_w;
                        let mut stem_bottom = f32::MIN;
                        for (hi, h) in nhits.iter().enumerate() {
                            let g = if h.rim { glyphs::NOTEHEAD_X_BLACK } else { glyphs::NOTEHEAD_BLACK };
                            let hy = ly + if h.left { 0.5 * sp } else { -0.5 * sp };
                            let se = self.fonts.bravura.anchors(g.name).and_then(|a| a.stem_up_se).unwrap_or(Sp { x: 1.18, y: 0.168 });
                            if hi == 0 {
                                stem_right = x + se.x * sp;
                            }
                            stem_bottom = stem_bottom.max(hy - se.y * sp);
                            let (l, r, t, b) = self.bbox(g);
                            let idx = self.glyph(&mut cmds, g, x, hy, 1.0, color);
                            head_cmds.push((idx, Rect::from_ltrb(x + l * sp, hy + t * sp, x + r * sp, hy + b * sp), h.rim));
                        }
                        // haste
                        shared_cmds.push(Self::quad(&mut cmds, rect(stem_right - stem_w, top, stem_w, stem_bottom - top), color));
                        stem_x[ei] = Some((stem_right, x));
                        // bandeirola (nota fora de beam)
                        if !in_beam[ei] && ev.flags >= 1 {
                            let g = match ev.flags {
                                1 => glyphs::FLAG_8TH_UP,
                                2 => glyphs::FLAG_16TH_UP,
                                3 => glyphs::FLAG_32ND_UP,
                                _ => FLAG_64TH_UP,
                            };
                            let a = self.fonts.bravura.anchors(g.name).and_then(|a| a.stem_up_nw).unwrap_or_default();
                            shared_cmds.push(self.glyph(&mut cmds, g, stem_right - stem_w - a.x * sp, top + a.y * sp, 1.0, color));
                        }
                        // rulo: três traços na haste
                        if *roll > 0 {
                            let cx = stem_right - stem_w / 2.0;
                            let mid = (top + stem_bottom) / 2.0 + if in_beam[ei] { 0.5 * sp } else { 0.0 };
                            for k in -1..=1 {
                                let cy = mid + k as f32 * 0.55 * sp;
                                let (dx, dy, t) = (0.6 * sp, 0.3 * sp, 0.3 * sp);
                                let path = Path::polygon(&[pt(cx - dx, cy + dy), pt(cx + dx, cy - dy), pt(cx + dx, cy - dy + t), pt(cx - dx, cy + dy + t)]);
                                cmds.push(DrawCmd::FillPath { path, color });
                                shared_cmds.push((cmds.len() - 1) as u32);
                            }
                        }
                        // acento e sfz acima da haste
                        let xc = x + head_w / 2.0;
                        let mut mark_y = top - 0.35 * sp;
                        if nhits.iter().any(|h| h.accent) {
                            has_accent[ei] = true;
                            let (l, r, t, _) = self.bbox(glyphs::ACCENT_ABOVE);
                            shared_cmds.push(self.glyph(&mut cmds, glyphs::ACCENT_ABOVE, xc - (l + r) / 2.0 * sp, mark_y, 1.0, color));
                            mark_y += (t - 0.3) * sp;
                        }
                        if nhits.iter().any(|h| h.sfz) {
                            let s = 0.72;
                            let (l, r, _, b) = self.bbox(glyphs::DYNAMIC_SFZ);
                            shared_cmds.push(self.glyph(&mut cmds, glyphs::DYNAMIC_SFZ, xc - (l + r) / 2.0 * sp * s, mark_y - b * sp * s, s, color));
                        }
                        shared_cmds.extend(label_cmd[ei]);
                        // registra os golpes
                        for (hi, h) in nhits.iter().enumerate() {
                            let (idx, head, rim) = head_cmds[hi];
                            let mut c = vec![idx];
                            c.extend(&shared_cmds);
                            hits.push(HitGfx { track: ti, cell: ev.cell, hit: h.index, head, rim, cmds: c });
                            let base = (self.cell_base[mi] + ev.cell) as u32;
                            let span = if *roll > 0 { *roll } else { 1 };
                            keys.push((0..span).map(|s| (ti as u32, base, (h.index + s) as u16)).collect());
                        }
                    }
                }
            }

            // ── beams ──
            let beam_t = d.beam_thickness * sp;
            let beam_step = (d.beam_thickness + d.beam_spacing) * sp;
            for b in &rhythm.beams {
                let idx: Vec<usize> = (b.first..=b.last).collect();
                let color = cell_color(events[b.first].cell);
                let top = stem_top[b.first];
                let sx = |i: usize| stem_x[i].map_or(0.0, |s| s.0);
                let max_flags = idx.iter().map(|&i| events[i].flags).max().unwrap_or(1);
                for level in 1..=max_flags {
                    let y = top + f32::from(level - 1) * beam_step;
                    let mut k = 0;
                    while k < idx.len() {
                        if events[idx[k]].flags < level {
                            k += 1;
                            continue;
                        }
                        let mut e = k;
                        while e + 1 < idx.len() && events[idx[e + 1]].flags >= level {
                            e += 1;
                        }
                        let r = if e > k {
                            Rect::from_ltrb(sx(idx[k]) - stem_w, y, sx(idx[e]), y + beam_t)
                        } else {
                            // toco: aponta para dentro do grupo
                            let len = (1.1 * sp).min(cw * 0.3);
                            if k == 0 { Rect::from_ltrb(sx(idx[k]) - stem_w, y, sx(idx[k]) + len, y + beam_t) } else { Rect::from_ltrb(sx(idx[k]) - len, y, sx(idx[k]), y + beam_t) }
                        };
                        Self::quad(&mut cmds, r, color);
                        beams.push(BeamInfo { track: ti, beat: events[b.first].beat, level, notes: e - k + 1, rect: r });
                        k = e + 1;
                    }
                }
            }

            // ── quiálteras ──
            for t in &rhythm.tuplets {
                let notes: Vec<usize> = (t.first..=t.last).filter(|&i| stem_x[i].is_some()).collect();
                let (Some(&a), Some(&b)) = (notes.first(), notes.last()) else { continue };
                let color = cell_color(t.cell);
                let xl = x_of(events[t.first].pos);
                let xr = if t.last == b { stem_x[b].map_or(xl, |s| s.0) } else { x_of(events[t.last].pos) + head_w };
                let top = (t.first..=t.last).map(|i| stem_top[i]).fold(f32::MAX, f32::min);
                let lifted = (t.first..=t.last).any(|i| has_accent[i]);
                let s = 0.8;
                let (l, r, gt, _) = self.bbox(glyphs::TUPLET_3);
                let base = top - (0.45 + if lifted { 1.3 } else { 0.0 }) * sp;
                let cx = (xl + xr) / 2.0;
                self.glyph(&mut cmds, glyphs::TUPLET_3, cx - (l + r) / 2.0 * sp * s, base, s, color);
                let beamed = rhythm.beams.iter().any(|g| g.first <= a && g.last >= b) && notes.len() == t.last - t.first + 1;
                if !beamed {
                    let by = base + gt * sp * s / 2.0;
                    let bt = d.tuplet_bracket_thickness * sp;
                    let half = (r - l) / 2.0 * sp * s + 0.3 * sp;
                    if cx - half > xl {
                        Self::quad(&mut cmds, Rect::from_ltrb(xl, by, cx - half, by + bt), color);
                        Self::quad(&mut cmds, Rect::from_ltrb(xl, by, xl + bt, by + 0.6 * sp), color);
                    }
                    if xr > cx + half {
                        Self::quad(&mut cmds, Rect::from_ltrb(cx + half, by, xr, by + bt), color);
                        Self::quad(&mut cmds, Rect::from_ltrb(xr - bt, by, xr, by + 0.6 * sp), color);
                    }
                }
            }

            // ── dinâmica da linha ──
            let own = meas.track_dynamic(&track.id);
            let own_dyn = own.and_then(|o| o.dynamic);
            if own_dyn.is_some() || self.track_hair[ti][mi].is_some() {
                self.dynamics(&mut cmds, own_dyn, self.track_hair[ti][mi], row_top + rm.line + rm.dyn_y, notes_x, x0, x1, ink);
            }
        }

        let layout = MeasureLayout {
            index: mi,
            system: 0,
            page: 0,
            bounds: rect(x0, geo.top, geo.w, geo.bottom - geo.top),
            notes_x,
            cell_width: cw,
            cells: meta.cells,
            cell_base: self.cell_base[mi],
            cmds,
            hits,
            beams,
        };
        (layout, keys)
    }

    /// Marca de dinâmica (pp–ff) no início do compasso e/ou a fatia do hairpin dele.
    #[allow(clippy::too_many_arguments)]
    fn dynamics(&self, cmds: &mut Vec<DrawCmd>, dynamic: Option<Dynamic>, hair: HairSeg, yc: f32, notes_x: f32, x0: f32, x1: f32, color: Color) {
        let sp = self.sp;
        let s = 0.8;
        let mut xa = None;
        if let Some(dy) = dynamic {
            let g = match dy {
                Dynamic::Pp => glyphs::DYNAMIC_PP,
                Dynamic::P => glyphs::DYNAMIC_PIANO,
                Dynamic::Mp => glyphs::DYNAMIC_MP,
                Dynamic::Mf => glyphs::DYNAMIC_MF,
                Dynamic::F => glyphs::DYNAMIC_FORTE,
                Dynamic::Ff => glyphs::DYNAMIC_FF,
            };
            let (l, r, ..) = self.bbox(g);
            self.glyph(cmds, g, notes_x - l * sp * s, yc + 0.45 * sp, s, color);
            xa = Some(notes_x + (r - l) * sp * s + 0.6 * sp);
        }
        if let Some((hp, i, len)) = hair {
            let (first, last) = (i == 0, i + 1 == len);
            let xa = xa.unwrap_or(if first { notes_x } else { x0 });
            let xb = if last { x1 - 0.8 * sp } else { x1 };
            if xb <= xa {
                return;
            }
            let open = 1.6 * sp;
            let (t0, t1) = (i as f32 / len as f32, (i + 1) as f32 / len as f32);
            let (o0, o1) = match hp {
                Hairpin::Cresc => (open * t0, open * t1),
                Hairpin::Dim => (open * (1.0 - t0), open * (1.0 - t1)),
            };
            let stroke = Stroke::round(self.fonts.bravura.defaults().hairpin_thickness * sp);
            for sign in [-0.5, 0.5] {
                cmds.push(DrawCmd::Line { from: pt(xa, yc + sign * o0), to: pt(xb, yc + sign * o1), stroke, color });
            }
        }
    }
}

struct MeasureGeo<'g> {
    row_metrics: &'g [RowMetrics],
    x: f32,
    w: f32,
    top: f32,
    bottom: f32,
    staff_ys: &'g [f32],
    rows: &'g [(f32, f32)],
    bar_top: f32,
    bar_bottom: f32,
    global_dyn_y: f32,
    last_in_system: bool,
}

fn demacron(c: char) -> Option<char> {
    Some(match c {
        'ā' => 'a',
        'ē' => 'e',
        'ī' => 'i',
        'ō' => 'o',
        'ū' => 'u',
        'Ā' => 'A',
        'Ē' => 'E',
        'Ī' => 'I',
        'Ō' => 'O',
        'Ū' => 'U',
        _ => return None,
    })
}

/// número como o JS escreveria (sem ".0")
fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v}") }
}
