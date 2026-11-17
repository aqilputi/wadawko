//! [`Score`]: o layout pronto (páginas → sistemas → compassos → comandos de desenho),
//! pintura com culling, highlight por golpe, playhead e hit-test.

use std::collections::HashMap;
use std::fmt;
use std::ops::Range;

use wadawko_draw::{Canvas2D, Color, DrawCmd, Point, Rect};

use crate::options::{LayoutMode, Theme};

/// Chave estável de um golpe: a mesma do app web (`trackId:célulaGlobal:golpe`).
/// `global_cell` conta as células desde o início da música, na ordem da partitura
/// (sem desdobrar repetições); [`Score::hit_key`] converte de compasso + célula.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HitKey {
    pub track_id: String,
    pub global_cell: usize,
    pub hit: usize,
}

impl HitKey {
    pub fn new(track_id: impl Into<String>, global_cell: usize, hit: usize) -> Self {
        HitKey { track_id: track_id.into(), global_cell, hit }
    }
}

impl fmt::Display for HitKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.track_id, self.global_cell, self.hit)
    }
}

/// Golpe aceso: chave + cor.
pub type Lit = (HitKey, Color);

/// Os elementos gráficos de um golpe dentro do compasso dele.
#[derive(Clone, Debug, PartialEq)]
pub struct HitGfx {
    /// índice da linha em `song.tracks`
    pub track: usize,
    /// célula dentro do compasso
    pub cell: usize,
    /// índice do golpe dentro da célula
    pub hit: usize,
    /// caixa da cabeça da nota (coordenadas da página) — para hit-test
    pub head: Rect,
    /// borda (cabeça ×)
    pub rim: bool,
    /// índices em [`MeasureLayout::cmds`]: cabeça, haste, bandeirola, acento, sfz, sílaba
    pub cmds: Vec<u32>,
}

/// Um beam desenhado (informação semântica para testes/depuração).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamInfo {
    pub track: usize,
    /// tempo do compasso em que o grupo está
    pub beat: usize,
    /// 1 = beam principal (colcheia), 2 = semicolcheia…
    pub level: u8,
    /// nº de notas cobertas (1 = toco)
    pub notes: usize,
    pub rect: Rect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeasureLayout {
    /// índice em `song.measures`
    pub index: usize,
    pub system: usize,
    pub page: usize,
    /// do alto do sistema até o pé; x/largura do compasso (barra a barra)
    pub bounds: Rect,
    /// x da célula 0
    pub notes_x: f32,
    pub cell_width: f32,
    pub cells: usize,
    /// células antes deste compasso (base da célula global)
    pub cell_base: usize,
    pub cmds: Vec<DrawCmd>,
    pub hits: Vec<HitGfx>,
    pub beams: Vec<BeamInfo>,
}

impl MeasureLayout {
    /// x do instante `frac` ∈ [0, 1] do compasso.
    pub fn x_at(&self, frac: f32) -> f32 {
        self.notes_x + frac.clamp(0.0, 1.0) * self.cells as f32 * self.cell_width
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SystemLayout {
    pub page: usize,
    pub bounds: Rect,
    /// x onde começam os compassos (fim da coluna de nomes)
    pub staff_x: f32,
    /// compassos do sistema (índices em [`Score::measures`])
    pub measures: Range<usize>,
    /// y da linha da pauta de cada linha (track)
    pub staff_ys: Vec<f32>,
    /// faixa vertical (topo, pé) de cada linha
    pub rows: Vec<(f32, f32)>,
    /// extensão vertical das barras de compasso (para o playhead)
    pub bar_top: f32,
    pub bar_bottom: f32,
    /// nomes das linhas, trilhos de pessoa, colchete
    pub cmds: Vec<DrawCmd>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageLayout {
    pub width: f32,
    pub height: f32,
    pub systems: Range<usize>,
    /// título, andamento, rodapé
    pub cmds: Vec<DrawCmd>,
}

/// Posição do playhead devolvida por [`Score::x_at`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayheadPos {
    pub page: usize,
    pub system: usize,
    pub x: f32,
    pub top: f32,
    pub bottom: f32,
}

/// Célula sob um ponto ([`Score::cell_at`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellRef {
    pub track: usize,
    pub measure: usize,
    pub cell: usize,
}

/// Partitura engravada. Imutável; pintar não refaz layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Score {
    pub mode: LayoutMode,
    pub theme: Theme,
    /// staff space efetivo (já com zoom), em px
    pub staff_space: f32,
    pub pages: Vec<PageLayout>,
    pub systems: Vec<SystemLayout>,
    /// um por compasso da música, na ordem
    pub measures: Vec<MeasureLayout>,
    /// ids das linhas, na ordem de `song.tracks`
    pub track_ids: Vec<String>,
    pub(crate) hit_index: HashMap<(u32, u32, u16), (u32, u32)>,
    /// folga horizontal do culling (textos que vazam do compasso)
    pub(crate) slop: f32,
}

impl Score {
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// (largura, altura) da página; no modo contínuo a página 0 é a partitura inteira.
    pub fn size(&self) -> (f32, f32) {
        self.page_size(0)
    }

    pub fn page_size(&self, page: usize) -> (f32, f32) {
        self.pages.get(page).map_or((0.0, 0.0), |p| (p.width, p.height))
    }

    pub fn total_cells(&self) -> usize {
        self.measures.last().map_or(0, |m| m.cell_base + m.cells)
    }

    // ───────────── chaves e golpes ─────────────

    /// Chave do golpe `hit` da célula `cell` do compasso `measure`.
    pub fn hit_key(&self, track_id: &str, measure: usize, cell: usize, hit: usize) -> Option<HitKey> {
        let m = self.measures.get(measure)?;
        Some(HitKey::new(track_id, m.cell_base + cell, hit))
    }

    fn raw_key(&self, key: &HitKey) -> Option<(u32, u32, u16)> {
        let t = self.track_ids.iter().position(|id| *id == key.track_id)?;
        Some((t as u32, u32::try_from(key.global_cell).ok()?, u16::try_from(key.hit).ok()?))
    }

    fn locate(&self, key: &HitKey) -> Option<(usize, usize)> {
        let raw = self.raw_key(key)?;
        self.hit_index.get(&raw).map(|&(m, h)| (m as usize, h as usize))
    }

    /// Elementos gráficos do golpe (e o compasso onde estão). `None` = golpe não desenhado.
    pub fn hit(&self, key: &HitKey) -> Option<(&MeasureLayout, &HitGfx)> {
        let (m, h) = self.locate(key)?;
        let meas = &self.measures[m];
        Some((meas, &meas.hits[h]))
    }

    /// Nº de chaves de golpe registradas (um rulo registra todos os golpes que representa).
    pub fn hit_count(&self) -> usize {
        self.hit_index.len()
    }

    /// Todas as chaves, em ordem (linha, célula global, golpe).
    pub fn hit_keys(&self) -> Vec<HitKey> {
        let mut raw: Vec<_> = self.hit_index.keys().copied().collect();
        raw.sort_unstable();
        raw.into_iter().map(|(t, c, h)| HitKey::new(self.track_ids[t as usize].clone(), c as usize, h as usize)).collect()
    }

    // ───────────── pintura ─────────────

    /// Pinta a página 0 (modo contínuo: a partitura inteira), só o que cruza `viewport`
    /// (em coordenadas da página). Não pinta fundo.
    pub fn paint(&self, canvas: &mut dyn Canvas2D, viewport: Rect) {
        self.paint_lit(canvas, 0, Some(viewport), &[]);
    }

    /// Pinta uma página; `viewport = None` pinta tudo.
    pub fn paint_page(&self, canvas: &mut dyn Canvas2D, page: usize, viewport: Option<Rect>) {
        self.paint_lit(canvas, page, viewport, &[]);
    }

    /// Como [`paint_page`](Self::paint_page), com os golpes de `lit` recoloridos na mesma
    /// passada (sem re-engravar: só troca a cor dos comandos do golpe).
    pub fn paint_lit(&self, canvas: &mut dyn Canvas2D, page: usize, viewport: Option<Rect>, lit: &[Lit]) {
        let Some(pg) = self.pages.get(page) else { return };
        // compasso → (comando → cor)
        let mut recolor: HashMap<usize, Vec<(u32, Color)>> = HashMap::new();
        for (key, color) in lit {
            if let Some((m, h)) = self.locate(key) {
                let v = recolor.entry(m).or_default();
                v.extend(self.measures[m].hits[h].cmds.iter().map(|&c| (c, *color)));
            }
        }
        for cmd in &pg.cmds {
            cmd.apply(canvas);
        }
        for sys in &self.systems[pg.systems.clone()] {
            if viewport.is_some_and(|v| !v.intersects(&sys.bounds)) {
                continue;
            }
            for cmd in &sys.cmds {
                cmd.apply(canvas);
            }
            for mi in self.visible_measures(sys, viewport) {
                let meas = &self.measures[mi];
                match recolor.get(&mi) {
                    None => meas.cmds.iter().for_each(|c| c.apply(canvas)),
                    Some(colors) => {
                        for (i, cmd) in meas.cmds.iter().enumerate() {
                            match colors.iter().rev().find(|(ci, _)| *ci as usize == i) {
                                Some((_, color)) => {
                                    let mut c = cmd.clone();
                                    c.set_color(*color);
                                    c.apply(canvas);
                                }
                                None => cmd.apply(canvas),
                            }
                        }
                    }
                }
            }
        }
    }

    /// Pinta **só** os golpes de `lit`, na cor de cada um — camada por cima de uma base já
    /// pintada/cacheada (o flash do playback).
    pub fn paint_highlights(&self, canvas: &mut dyn Canvas2D, page: usize, viewport: Option<Rect>, lit: &[Lit]) {
        for (key, color) in lit {
            let Some((m, h)) = self.locate(key) else { continue };
            let meas = &self.measures[m];
            if meas.page != page || viewport.is_some_and(|v| !self.inflate(v).intersects(&meas.bounds)) {
                continue;
            }
            for &ci in &meas.hits[h].cmds {
                let mut c = meas.cmds[ci as usize].clone();
                c.set_color(*color);
                c.apply(canvas);
            }
        }
    }

    /// Pinta só a coluna de nomes/trilhos de um sistema deslocada em `dx` — para a view
    /// manter os nomes fixos enquanto a partitura rola na horizontal.
    pub fn paint_system_labels(&self, canvas: &mut dyn Canvas2D, system: usize, dx: f32) {
        let Some(sys) = self.systems.get(system) else { return };
        canvas.save();
        canvas.translate(dx, 0.0);
        for cmd in &sys.cmds {
            cmd.apply(canvas);
        }
        canvas.restore();
    }

    fn inflate(&self, v: Rect) -> Rect {
        Rect::new(v.x - self.slop, v.y, v.w + 2.0 * self.slop, v.h)
    }

    /// Compassos do sistema que cruzam o viewport (com folga para textos que vazam).
    pub fn visible_measures(&self, sys: &SystemLayout, viewport: Option<Rect>) -> Range<usize> {
        let Some(v) = viewport else { return sys.measures.clone() };
        let v = self.inflate(v);
        let ms = &self.measures[sys.measures.clone()];
        let lo = ms.partition_point(|m| m.bounds.right() < v.left());
        let hi = ms.partition_point(|m| m.bounds.left() <= v.right());
        sys.measures.start + lo..sys.measures.start + hi.max(lo)
    }

    // ───────────── playhead / auto-scroll ─────────────

    /// Posição do playhead no instante `frac` ∈ [0, 1] do compasso `measure`.
    pub fn x_at(&self, measure: usize, frac: f32) -> Option<PlayheadPos> {
        let m = self.measures.get(measure)?;
        let sys = &self.systems[m.system];
        Some(PlayheadPos { page: m.page, system: m.system, x: m.x_at(frac), top: sys.bar_top, bottom: sys.bar_bottom })
    }

    /// Como [`x_at`](Self::x_at), a partir da célula global fracionária (o `cellFloatAt` do web).
    pub fn x_at_cell(&self, global_cell: f32) -> Option<PlayheadPos> {
        let last = self.measures.last()?;
        let g = global_cell.clamp(0.0, (last.cell_base + last.cells) as f32);
        let mi = self.measures.partition_point(|m| ((m.cell_base + m.cells) as f32) <= g).min(self.measures.len() - 1);
        let m = &self.measures[mi];
        let frac = if m.cells == 0 { 0.0 } else { (g - m.cell_base as f32) / m.cells as f32 };
        self.x_at(mi, frac)
    }

    /// Scroll horizontal que deixa `x` a 40% da largura visível (igual ao `Score.svelte`),
    /// limitado ao conteúdo da página 0.
    pub fn scroll_x_for(&self, x: f32, viewport_width: f32) -> f32 {
        let max = (self.size().0 - viewport_width).max(0.0);
        (x - viewport_width * 0.4).clamp(0.0, max)
    }

    // ───────────── hit-test ─────────────

    /// Compasso sob o ponto (coordenadas da página).
    pub fn measure_at(&self, page: usize, p: Point) -> Option<usize> {
        let pg = self.pages.get(page)?;
        for sys in &self.systems[pg.systems.clone()] {
            if p.y < sys.bounds.top() || p.y > sys.bounds.bottom() {
                continue;
            }
            let ms = &self.measures[sys.measures.clone()];
            let i = ms.partition_point(|m| m.bounds.right() < p.x);
            if let Some(m) = ms.get(i).filter(|m| m.bounds.contains(p)) {
                return Some(m.index);
            }
        }
        None
    }

    /// Linha + compasso + célula sob o ponto.
    pub fn cell_at(&self, page: usize, p: Point) -> Option<CellRef> {
        let mi = self.measure_at(page, p)?;
        let m = &self.measures[mi];
        let sys = &self.systems[m.system];
        let track = sys.rows.iter().position(|&(t, b)| p.y >= t && p.y < b)?;
        if m.cells == 0 || m.cell_width <= 0.0 {
            return None;
        }
        let cell = (((p.x - m.notes_x) / m.cell_width).floor().max(0.0) as usize).min(m.cells - 1);
        Some(CellRef { track, measure: mi, cell })
    }

    /// Golpe cuja cabeça está sob o ponto (com `tolerance` px de folga).
    pub fn hit_at(&self, page: usize, p: Point, tolerance: f32) -> Option<HitKey> {
        let mi = self.measure_at(page, p)?;
        let m = &self.measures[mi];
        let best = m.hits.iter().filter(|h| h.head.inset(-tolerance).contains(p)).min_by(|a, b| dist2(a.head.center(), p).total_cmp(&dist2(b.head.center(), p)))?;
        Some(HitKey::new(self.track_ids[best.track].clone(), m.cell_base + best.cell, best.hit))
    }
}

fn dist2(a: Point, b: Point) -> f32 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}
