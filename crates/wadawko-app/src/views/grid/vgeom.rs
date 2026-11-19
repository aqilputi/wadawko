//! Geometria do editor vertical (estilo tracker — docs/planejamento/tracker.md §2.2):
//! o tempo corre para baixo (uma linha da tela = uma célula), cada instrumento é
//! uma coluna com sub-colunas (`nota` · `dyn` · `kiai`; `fx` entra na T8).
//!
//! Dentro do canvas de uma seção: `GUTTER_W` (números) + `MEAS_W` (coluna do
//! compasso) fixos à esquerda, depois as colunas, que rolam em x por `offset`.

use wadawko_core::Song;

use super::Rows;
use crate::song_ext::SongExt;

/// altura de uma linha (uma célula), padrão e faixa ajustável (`Ctrl+Alt+↑/↓`)
pub const ROW_H: f32 = 28.0;
pub const ROW_H_MIN: f32 = 20.0;
pub const ROW_H_MAX: f32 = 44.0;
pub const ROW_H_STEP: f32 = 4.0;
/// largura da sub-coluna de nota, padrão e faixa ajustável (`Ctrl+roda`, `z i`/`z o`)
pub const COL_W: f32 = 96.0;
pub const COL_W_MIN: f32 = 72.0;
pub const COL_W_MAX: f32 = 144.0;
pub const COL_W_STEP: f32 = 8.0;
/// sub-coluna de efeitos (T8: ainda não aparece)
pub const FX_W: f32 = 44.0;
/// sub-coluna de dinâmica da linha
pub const DYN_W: f32 = 36.0;
/// sub-coluna de kiai
pub const KYAI_W: f32 = 64.0;
/// gutter: 2 px cor da seção · 8 px faixa de loop · 38 px números
pub const GUTTER_W: f32 = 48.0;
/// coluna do compasso: dinâmica, `<` `>`, andamento, cunha
pub const MEAS_W: f32 = 72.0;
/// cabeçalho de colunas (fixo no topo do editor)
pub const COL_HEAD_H: f32 = 44.0;
/// parte fixa à esquerda das colunas
pub const LEFT_W: f32 = GUTTER_W + MEAS_W;
/// fresta entre as colunas de duas linhas (mostra o `bg`)
pub const COL_GAP: f32 = 2.0;
/// faixa de loop no gutter: x e largura
pub const LOOP_X: f32 = 2.0;
pub const LOOP_W: f32 = 8.0;

/// Layout do editor: vertical (tracker, padrão) ou a grade horizontal de antes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub enum Layout {
    #[default]
    Vertical,
    Horizontal,
}

impl Layout {
    pub fn name(self) -> &'static str {
        match self {
            Layout::Vertical => "vertical",
            Layout::Horizontal => "horizontal",
        }
    }
    pub fn parse(s: &str) -> Option<Layout> {
        match s {
            "vertical" => Some(Layout::Vertical),
            "horizontal" => Some(Layout::Horizontal),
            _ => None,
        }
    }
    pub fn toggled(self) -> Layout {
        match self {
            Layout::Vertical => Layout::Horizontal,
            Layout::Horizontal => Layout::Vertical,
        }
    }
}

/// Parte de uma coluna sob um x.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sub {
    Note,
    /// sub-coluna de dinâmica, com o x local a ela
    Dyn(f32),
    Kyai,
    Fx,
}

/// x de cada sub-coluna por linha (instrumento), a partir do início das colunas.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cols {
    /// início do grupo de sub-colunas da linha `t`
    pub x: Vec<f32>,
    /// largura do grupo da linha `t` (sem a fresta)
    pub group_w: Vec<f32>,
    /// x da sub-coluna de dinâmica, relativo ao início do grupo
    pub dyn_x: Vec<Option<f32>>,
    /// x da sub-coluna de kiai, relativo ao início do grupo
    pub kyai_x: Vec<Option<f32>>,
    /// x da sub-coluna de efeitos (T8; por enquanto sempre `None`)
    pub fx_x: Vec<Option<f32>>,
    /// largura da sub-coluna de nota
    pub note_w: f32,
    /// largura total das colunas
    pub w: f32,
}

impl Cols {
    /// Grupos na ordem das linhas: `nota` [`fx`] [`dyn`] [`kiai`]. Linhas com dinâmica
    /// própria / kiai (as mesmas regras da grade horizontal, em `Rows`) ganham a sub-coluna.
    pub fn new(rows: &Rows, note_w: f32) -> Cols {
        let mut c = Cols { note_w, ..Cols::default() };
        for t in 0..rows.y.len() {
            let mut gw = note_w;
            let f = rows.fx.get(t).copied().unwrap_or(false).then(|| {
                let x = gw;
                gw += FX_W;
                x
            });
            c.fx_x.push(f);
            let d = rows.dynr[t].then(|| {
                let x = gw;
                gw += DYN_W;
                x
            });
            let k = rows.kyai[t].then(|| {
                let x = gw;
                gw += KYAI_W;
                x
            });
            c.x.push(c.w);
            c.group_w.push(gw);
            c.dyn_x.push(d);
            c.kyai_x.push(k);
            c.w += gw + COL_GAP;
        }
        c
    }

    /// largura de tudo o que não é nota (sub-colunas + frestas): não encolhe ao ajustar
    pub fn extras(&self) -> f32 {
        self.w - self.note_w * self.x.len() as f32
    }

    /// Coluna e sub-coluna sob `x` (relativo ao início das colunas, já com o offset).
    pub fn col_at(&self, x: f32) -> Option<(usize, Sub)> {
        if x < 0.0 {
            return None;
        }
        let t = self.x.iter().rposition(|&cx| cx <= x)?;
        let lx = x - self.x[t];
        if lx >= self.group_w[t] {
            return None;
        }
        if lx < self.note_w {
            return Some((t, Sub::Note));
        }
        if let Some(fx) = self.fx_x[t]
            && lx >= fx
            && lx < fx + FX_W
        {
            return Some((t, Sub::Fx));
        }
        if let Some(dx) = self.dyn_x[t]
            && lx >= dx
            && lx < dx + DYN_W
        {
            return Some((t, Sub::Dyn(lx - dx)));
        }
        if let Some(kx) = self.kyai_x[t]
            && lx >= kx
            && lx < kx + KYAI_W
        {
            return Some((t, Sub::Kyai));
        }
        None
    }
}

/// "Ajustar à largura": a coluna de nota encolhe (até `COL_W_MIN`) para as colunas
/// caberem em `avail` px (largura do canvas); `avail <= 0` (sem medida) = `want`.
pub fn fit_col_w(want: f32, avail: f32, n_tracks: usize, extras: f32) -> f32 {
    if avail <= 0.0 || n_tracks == 0 {
        return want;
    }
    let room = (avail - LEFT_W - extras) / n_tracks as f32;
    want.min(room.floor()).max(COL_W_MIN.min(want))
}

/// y (local ao canvas da seção) do início de cada compasso da seção.
pub fn measure_ys(song: &Song, s: usize, row_h: f32) -> Vec<f32> {
    let (a, b) = song.section_range(s);
    let mut y = 0.0;
    (a..b)
        .map(|m| {
            let here = y;
            y += song.mlen(m) as f32 * row_h;
            here
        })
        .collect()
}

/// y (local ao canvas da seção `s`) do compasso `m`, sem alocar; `None` fora da seção.
pub fn measure_y(song: &Song, s: usize, m: usize, row_h: f32) -> Option<f32> {
    let (a, b) = song.section_range(s);
    (a..b).contains(&m).then(|| (a..m).map(|i| song.mlen(i) as f32 * row_h).sum())
}

/// altura do corpo (canvas) de uma seção no editor vertical
pub fn section_body_h(song: &Song, s: usize, row_h: f32) -> f32 {
    let (a, b) = song.section_range(s);
    (a..b).map(|m| song.mlen(m) as f32 * row_h).sum::<f32>().max(row_h)
}

/// (compasso, célula) sob `y` (local ao canvas da seção).
pub fn row_at(song: &Song, s: usize, row_h: f32, y: f32) -> Option<(usize, usize)> {
    if y < 0.0 {
        return None;
    }
    let (a, b) = song.section_range(s);
    let mut top = 0.0;
    for m in a..b {
        let h = song.mlen(m) as f32 * row_h;
        if y < top + h {
            return Some((m, (((y - top) / row_h) as usize).min(song.mlen(m).saturating_sub(1))));
        }
        top += h;
    }
    None
}

/// Parte da coluna do compasso sob `x` (local à coluna) na célula `k` do compasso:
/// 1ª linha = chip de dinâmica + `<` `>`; 2ª linha = chip de andamento.
pub(crate) fn meas_part(x: f32, k: usize) -> super::DynPart {
    use super::DynPart;
    match k {
        0 if (2.0..32.0).contains(&x) => DynPart::Chip,
        0 if (34.0..50.0).contains(&x) => DynPart::Cresc,
        0 if (52.0..68.0).contains(&x) => DynPart::Dim,
        1 if (2.0..MEAS_W - 12.0).contains(&x) => DynPart::Tempo,
        _ => DynPart::None,
    }
}

/// Parte da sub-coluna de dinâmica da linha: 1ª linha do compasso = chip; 2ª = `<` | `>`.
pub(crate) fn dyn_sub_part(x: f32, k: usize) -> super::DynPart {
    use super::DynPart;
    match k {
        0 => DynPart::Chip,
        1 if x < DYN_W / 2.0 => DynPart::Cresc,
        1 => DynPart::Dim,
        _ => DynPart::None,
    }
}

// ───────────────────────────── GridView no layout vertical ─────────────────────────────

use gpui_kit::{Bounds, Pixels, Point, point, px, size};

use super::{Cur, GridView, Hit, sec_key};

impl GridView {
    /// Hit-test no canvas da seção `s` (vertical): gutter = "tocar daqui"; coluna do
    /// compasso = dinâmica/andamento; colunas = nota, dinâmica da linha ou kiai.
    pub(crate) fn vhit(&self, s: usize, pos: Point<Pixels>) -> Option<Hit> {
        let g = self.geom.borrow();
        let b = *g.sections.get(s)?;
        let x = f32::from(pos.x) - f32::from(b.origin.x);
        let y = f32::from(pos.y) - f32::from(b.origin.y);
        let (m, k) = row_at(&self.song, s, g.row_h, y)?;
        if x < 0.0 {
            return None;
        }
        // faixa de loop (8 px à esquerda do gutter): arrastar marca o trecho, como a régua horizontal
        if (LOOP_X - 2.0..LOOP_X + LOOP_W + 2.0).contains(&x) {
            return Some(Hit::LoopLane);
        }
        if x < GUTTER_W {
            return Some(Hit::Timeline(Some((m, k))));
        }
        if x < LEFT_W {
            return Some(Hit::SecDyn(m, meas_part(x - GUTTER_W, k), pos));
        }
        let (t, sub) = g.cols.col_at(x - LEFT_W + g.offset)?;
        match sub {
            Sub::Note => Some(Hit::Cell(Cur { m, t, k })),
            Sub::Dyn(lx) => Some(Hit::TrackDyn(m, t, dyn_sub_part(lx, k), pos)),
            Sub::Kyai => Some(Hit::Kyai(Cur { m, t, k })),
            Sub::Fx => Some(Hit::Fx(Cur { m, t, k })),
        }
    }

    /// Retângulo (janela) da célula de nota `c` no editor vertical.
    pub(crate) fn vcell_bounds(&self, c: Cur) -> Option<Bounds<Pixels>> {
        let g = self.geom.borrow();
        let song = &self.song;
        let s = song.section_of(c.m);
        let b = g.sections.get(s).copied().filter(|b| f32::from(b.size.width) > 0.0)?;
        let y = f32::from(b.origin.y) + measure_y(song, s, c.m, g.row_h)? + c.k as f32 * g.row_h;
        let x = f32::from(b.origin.x) + LEFT_W + g.cols.x.get(c.t)? - g.offset;
        Some(Bounds::new(point(px(x), px(y)), size(px(g.cols.note_w), px(g.row_h))))
    }

    /// Alvos de salto das células visíveis (dentro da área visível do editor e das colunas).
    pub(crate) fn vvisible_cells(&self, beats: bool, track: bool) -> Vec<(Cur, Bounds<Pixels>)> {
        let g = self.geom.borrow();
        let song = &self.song;
        let mut out = Vec::new();
        let cur_t = self.cursor.map(|c| c.t).unwrap_or(0);
        let vp = g.viewport;
        let has_vp = f32::from(vp.size.height) > 0.0;
        let (vt, vb) = (f32::from(vp.origin.y), f32::from(vp.origin.y) + f32::from(vp.size.height));
        for (s, sec) in song.secs().iter().enumerate() {
            let Some(b) = g.sections.get(s).copied() else { continue };
            if f32::from(b.size.width) <= 0.0 || self.is_collapsed(sec) {
                continue;
            }
            let (ox, oy) = (f32::from(b.origin.x), f32::from(b.origin.y));
            let right = ox + f32::from(b.size.width);
            let ys = measure_ys(song, s, g.row_h);
            for (i, &my) in ys.iter().enumerate() {
                let m = sec.start + i;
                let meas = &song.measures[m];
                for k in 0..meas.cell_count() {
                    if beats && k % meas.subdivision.max(1) != 0 {
                        continue;
                    }
                    let y = oy + my + k as f32 * g.row_h;
                    if has_vp && (y < vt || y + g.row_h > vb) {
                        continue;
                    }
                    for t in 0..song.tracks.len().min(g.cols.x.len()) {
                        if track && t != cur_t {
                            continue;
                        }
                        let x = ox + LEFT_W + g.cols.x[t] - g.offset;
                        if x < ox + LEFT_W || x + g.cols.note_w > right {
                            continue;
                        }
                        out.push((Cur { m, t, k }, Bounds::new(point(px(x), px(y)), size(px(g.cols.note_w), px(g.row_h)))));
                    }
                }
            }
        }
        out
    }

    /// Vertical: só a rolagem em x (colunas) é da grade; a rolagem no tempo é do
    /// Shell (`reveal_cursor`). Abre a seção recolhida do cursor.
    pub(crate) fn vensure_visible(&mut self, c: Cur) {
        let song = self.song.clone();
        let s = song.section_of(c.m);
        if let Some(sec) = song.secs().get(s) {
            self.collapsed.remove(&sec_key(sec));
        }
        let cols = self.cols();
        let Some(&x) = cols.x.get(c.t) else { return };
        let gw = cols.group_w[c.t];
        let vw = (self.view_w() - LEFT_W).max(cols.note_w);
        if x < self.offset_target {
            self.offset_target = x;
        } else if x + gw > self.offset_target + vw {
            self.offset_target = x + gw - vw;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::views::grid::{CELL_H, DynPart};

    #[test]
    fn cols_have_sub_columns_and_hit() {
        let song = wadawko_core::demo::demo_song();
        let id = song.tracks[0].id.clone();
        let rows = Rows::new(&song, &HashSet::from([id.clone()]), &HashSet::from([id]), CELL_H);
        let c = Cols::new(&rows, COL_W);
        assert_eq!(c.x.len(), song.tracks.len());
        assert_eq!(c.group_w[0], COL_W + DYN_W + KYAI_W);
        assert_eq!(c.col_at(1.0), Some((0, Sub::Note)));
        assert_eq!(c.col_at(COL_W + 3.0), Some((0, Sub::Dyn(3.0))));
        assert_eq!(c.col_at(COL_W + DYN_W + 1.0), Some((0, Sub::Kyai)));
        // fresta entre colunas: nada
        assert_eq!(c.col_at(c.group_w[0] + 0.5), None);
        if song.tracks.len() > 1 {
            assert_eq!(c.col_at(c.x[1] + 1.0), Some((1, Sub::Note)));
        }
        assert!(c.col_at(c.w + 10.0).is_none());
        assert_eq!(c.extras(), DYN_W + KYAI_W + COL_GAP * song.tracks.len() as f32);
    }

    #[test]
    fn rows_and_measures_stack_vertically() {
        let song = wadawko_core::demo::demo_song();
        let (a, b) = song.section_range(0);
        let ys = measure_ys(&song, 0, ROW_H);
        assert_eq!(ys.len(), b - a);
        assert_eq!(ys[0], 0.0);
        if b - a > 1 {
            assert_eq!(ys[1], song.mlen(a) as f32 * ROW_H);
        }
        assert_eq!(measure_y(&song, 0, a, ROW_H), Some(0.0));
        assert_eq!(measure_y(&song, 0, b, ROW_H), None);
        assert_eq!(row_at(&song, 0, ROW_H, 1.0), Some((a, 0)));
        assert_eq!(row_at(&song, 0, ROW_H, ROW_H + 1.0), Some((a, 1)));
        assert_eq!(row_at(&song, 0, ROW_H, section_body_h(&song, 0, ROW_H) + 1.0), None);
        assert_eq!(row_at(&song, 0, ROW_H, -1.0), None);
    }

    #[test]
    fn fit_shrinks_to_min_then_scrolls() {
        // cabe: fica no pedido
        assert_eq!(fit_col_w(96.0, 2000.0, 8, 16.0), 96.0);
        // não cabe: encolhe
        let w = fit_col_w(96.0, 800.0, 8, 16.0);
        assert!((COL_W_MIN..96.0).contains(&w));
        // muito estreito: para em 72 (aí rola em x)
        assert_eq!(fit_col_w(96.0, 300.0, 8, 16.0), COL_W_MIN);
        // sem medida
        assert_eq!(fit_col_w(120.0, 0.0, 8, 16.0), 120.0);
    }

    #[test]
    fn measure_column_parts() {
        assert_eq!(meas_part(10.0, 0), DynPart::Chip);
        assert_eq!(meas_part(40.0, 0), DynPart::Cresc);
        assert_eq!(meas_part(60.0, 0), DynPart::Dim);
        assert_eq!(meas_part(20.0, 1), DynPart::Tempo);
        assert_eq!(meas_part(20.0, 2), DynPart::None);
        assert_eq!(dyn_sub_part(5.0, 1), DynPart::Cresc);
        assert_eq!(dyn_sub_part(30.0, 1), DynPart::Dim);
    }
}
