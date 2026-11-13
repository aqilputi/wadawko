//! Ordem de execução e dinâmica (`src/lib/timeline.ts`) + `swing_warp` (de `audio.ts`).

use crate::model::{Dynamic, Hairpin, Song};

/// Região de compassos `[start_bar, end_bar)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub start_bar: usize,
    pub end_bar: usize,
}

/// Ordem de execução: índice do compasso-fonte para cada compasso tocado.
/// Compassos antes da 1ª seção tocam 1×; cada seção toca `repeat` vezes
/// (ritornello). Com região, só os compassos dentro dela — mas as seções
/// selecionadas continuam repetindo.
pub fn play_order(song: &Song, region: Option<Region>) -> Vec<usize> {
    let n = song.measures.len();
    let lo = region.map_or(0, |r| r.start_bar);
    let hi = region.map_or(n, |r| r.end_bar).min(n);
    let in_range = |mi: usize| mi >= lo && mi < hi;
    let mut secs: Vec<_> = song.sections().iter().collect();
    secs.sort_by_key(|s| s.start);
    let mut out = Vec::new();
    for mi in 0..secs.first().map_or(n, |s| s.start) {
        if in_range(mi) {
            out.push(mi);
        }
    }
    for (i, s) in secs.iter().enumerate() {
        let end = secs.get(i + 1).map_or(n, |x| x.start).min(n);
        for _ in 0..s.times() {
            for mi in s.start..end {
                if in_range(mi) {
                    out.push(mi);
                }
            }
        }
    }
    out
}

/// Instrumentos em destaque que ainda existem (aceita o formato antigo `mainTrackId`).
pub fn main_tracks(song: &Song) -> Vec<String> {
    let ids: Vec<String> = match (&song.main_track_ids, &song.main_track_id) {
        (Some(ids), _) => ids.clone(),
        (None, Some(id)) if !id.is_empty() => vec![id.clone()],
        _ => Vec::new(),
    };
    ids.into_iter().filter(|id| song.tracks.iter().any(|t| &t.id == id)).collect()
}

/// Destaque "main": os instrumentos principais sobem como de mf→f (×2) e os
/// demais descem como de mf→p (×0.5). Relativo: crescendos, sfz e marcações
/// continuam. Sem destaque, ou com TODOS em destaque, tudo ×1.
pub fn main_mul(song: &Song, track_id: &str) -> f64 {
    let main = main_tracks(song);
    if main.is_empty() || main.len() >= song.tracks.len() {
        return 1.0;
    }
    let mf = Dynamic::Mf.level();
    if main.iter().any(|id| id == track_id) { Dynamic::F.level() / mf } else { Dynamic::P.level() / mf }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Ramp {
    from: f64,
    to: f64,
    i0: usize,
    len: usize,
}

/// Multiplicador de dinâmica por compasso tocado (resultado de `dynamics_curve`).
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicsCurve {
    start_lvl: Vec<f64>,
    ramp: Vec<Option<Ramp>>,
}

impl DynamicsCurve {
    /// Multiplicador no compasso tocado `u` (índice na ordem de execução), na
    /// fração `frac` ∈ [0,1) do compasso. `u` fora da ordem → mf (1.0).
    pub fn at(&self, u: usize, frac: f64) -> f64 {
        match self.ramp.get(u).copied().flatten() {
            None => self.start_lvl.get(u).copied().unwrap_or(Dynamic::Mf.level()),
            Some(r) => {
                let t = ((u - r.i0) as f64 + frac) / r.len as f64;
                r.from + (r.to - r.from) * t
            }
        }
    }

    /// nível no início do compasso tocado `u` (sem rampa)
    pub fn start_level(&self, u: usize) -> f64 {
        self.start_lvl.get(u).copied().unwrap_or(Dynamic::Mf.level())
    }

    /// quantos compassos tocados a curva cobre
    pub fn len(&self) -> usize {
        self.start_lvl.len()
    }

    pub fn is_empty(&self) -> bool {
        self.start_lvl.is_empty()
    }
}

/// Multiplicador de dinâmica por compasso tocado, na ordem de execução `order`
/// (saída de `play_order`). Uma marcação (pp..ff) vale até a próxima;
/// crescendo/diminuendo num compasso rampa do nível atual até a próxima
/// marcação (ou um degrau acima/abaixo, se não houver).
/// Com `track_id`: marcações do instrumento têm prioridade sobre a geral no
/// compasso em que aparecem; onde ele não marca nada, segue a geral.
pub fn dynamics_curve(song: &Song, order: &[usize], track_id: Option<&str>) -> DynamicsCurve {
    let keys = Dynamic::ALL;
    let own = |mi: usize| track_id.and_then(|id| song.measures.get(mi)?.track_dynamic(id));
    let dyn_of = |mi: usize| -> Option<Dynamic> { own(mi).and_then(|o| o.dynamic).or_else(|| song.measures.get(mi)?.dynamic) };
    let hp_of = |mi: usize| -> Option<Hairpin> { own(mi).and_then(|o| o.hairpin).or_else(|| song.measures.get(mi)?.hairpin) };

    // nível no início de cada compasso tocado (marcação herdada)
    let mut start_lvl: Vec<f64> = Vec::with_capacity(order.len());
    let mut cur_keys: Vec<Dynamic> = Vec::with_capacity(order.len());
    let mut cur_key = Dynamic::Mf;
    // tocando um trecho: herda a última marcação antes dele
    for mi in 0..order.first().copied().unwrap_or(0) {
        if let Some(d) = dyn_of(mi) {
            cur_key = d;
        }
    }
    for &mi in order {
        if let Some(d) = dyn_of(mi) {
            cur_key = d;
        }
        start_lvl.push(cur_key.level());
        cur_keys.push(cur_key);
    }

    // segmentos de hairpin: sequência de compassos com o mesmo hairpin até uma marcação
    let mut ramp: Vec<Option<Ramp>> = vec![None; order.len()];
    let mut u = 0;
    while u < order.len() {
        let Some(hp) = hp_of(order[u]) else {
            u += 1;
            continue;
        };
        let mut v = u;
        while v + 1 < order.len() && hp_of(order[v + 1]) == Some(hp) && dyn_of(order[v + 1]).is_none() {
            v += 1;
        }
        let next_marked = if v + 1 < order.len() { dyn_of(order[v + 1]) } else { None };
        let from = start_lvl[u];
        let idx = cur_keys[u].index() as isize + if hp == Hairpin::Cresc { 1 } else { -1 };
        let step = keys[idx.clamp(0, keys.len() as isize - 1) as usize];
        // alvo = próxima marcação, se ela vai no sentido do hairpin (ex.: a volta
        // do ritornello pra um p não "desfaz" o crescendo); senão, um degrau
        let to = match next_marked.map(Dynamic::level) {
            Some(marked) if (hp == Hairpin::Cresc && marked > from) || (hp == Hairpin::Dim && marked < from) => marked,
            _ => step.level(),
        };
        for r in ramp.iter_mut().take(v + 1).skip(u) {
            *r = Some(Ramp { from, to, i0: u, len: v - u + 1 });
        }
        // sem marcação depois: o nível-alvo persiste após a rampa
        if next_marked.is_none() {
            let mut k = v + 1;
            while k < order.len() && dyn_of(order[k]).is_none() {
                start_lvl[k] = to;
                k += 1;
            }
        }
        u = v + 1;
    }
    DynamicsCurve { start_lvl, ramp }
}

/// Deforma a fração do tempo `f` ∈ `[0,1]` pelo swing `s` (0 = reto; 1 = a
/// colcheia do contratempo atrasa pro terço). Porte de `swingWarp` (`audio.ts`);
/// fica no core porque grade, esteira, kiai e áudio precisam da MESMA conta.
pub fn swing_warp(f: f64, s: f64) -> f64 {
    if s <= 0.0 {
        return f;
    }
    let mid = 0.5 + s * (2.0 / 3.0 - 0.5);
    if f <= 0.5 { (f * mid) / 0.5 } else { mid + ((f - 0.5) * (1.0 - mid)) / 0.5 }
}
