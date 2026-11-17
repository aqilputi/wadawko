//! Esteira — visão estilo Taiko no Tatsujin (porte de `src/lib/Lane.svelte` +
//! `src/lib/lane.ts`).
//!
//! - [`note_events`] (pura, testada) transforma a música em discos com o tick
//!   ABSOLUTO na linha do tempo TOCADA (um compasso por entrada de
//!   `play_order`, então ritornellos e região aparecem desenrolados); a conta do
//!   golpe (offset + swing) é a mesma do compilador do áudio (`schedule.rs`).
//! - A view pinta tudo num `canvas` só: trilhos de pessoa, tambores-alvo, campo,
//!   discos (com culling pela janela visível por busca binária) e as pétalas de
//!   cerejeira (sistema de partículas).
//! - Relógio: `AppState::audible_tick()`; `x = (seg_evento − t) · 380`.
//! - Easter egg: 5 cliques no logo (topbar) → [`LaneView::logo_click`] → fue.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::{
    App, BorderStyle, Bounds, Context, Entity, FontWeight, Hsla, PathBuilder, Pixels, Render, ShapedLine, SharedString, TextAlign, Window, canvas, div, fill, point, prelude::*, px, quad, rgb, size,
};
use wadawko_audio::{PPQ, TempoCurve};
use wadawko_core::ops::people::{cell_owner_of, person_audible, rails_for_tracks};
use wadawko_core::ops::sections::selected_region;
use wadawko_core::phrase::resolve_cell;
use wadawko_core::timeline::{play_order, swing_warp};
use wadawko_core::vocab::Hand;
use wadawko_core::{Song, Zone};

use crate::shell::Shell;
use crate::song_ext::SongExt;
use crate::state::AppState;
use crate::theme::{Theme, parse_color};

/// velocidade constante da esteira (px/s). BPM alto = notas mais juntas.
pub const PX_PER_SEC: f32 = 380.0;

// medidas do CSS do web (1rem = 16px)
const LANE_H: f32 = 70.4;
const LANE_GAP: f32 = 9.6;
const DRUM_W: f32 = 70.4;
const FACE: f32 = 51.2;
const RAIL_W: f32 = 12.8;
const RAIL_GAP: f32 = 4.8;
const RAIL_PAD: f32 = 6.4;
const STRIPE: f32 = 51.2;
/// duração do pulso do tambor (ms)
const PULSE_MS: f32 = 240.0;
const PETAL_COLORS: [u32; 5] = [0xffd7e8, 0xffb7d5, 0xff9ec4, 0xffc9de, 0xfce4ec];
/// teto de pétalas vivas (cada uma é um path pintado por frame)
const MAX_PETALS: usize = 150;
/// passos de rotação do cache de pétalas pré-tesseladas (15° cada)
const PETAL_ROT_STEPS: usize = 24;

// ───────────────────────────── eventos (puro) ─────────────────────────────

/// Um disco da esteira.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteEvent {
    /// índice da linha em `song.tracks`
    pub track: usize,
    /// instante na linha do tempo tocada, em ticks (`PPQ` por tempo)
    pub tick: f64,
    /// célula-fonte global e índice do golpe — casam com `HitFlash`/`HitFired`
    pub cell: usize,
    pub hit: usize,
    /// couro (head) = vermelho · fuchi (rim) = azul
    pub zone: Zone,
    pub accent: bool,
    /// golpe fantasma (tsu/ku): disco menor e apagado
    pub ghost: bool,
    /// duas mãos no mesmo instante (doron): disco grande com anel duplo
    pub both_hands: bool,
    pub hand: Hand,
    /// sílaba desenhada no disco (só no 1º golpe da célula)
    pub label: String,
    /// dono da nota (só em instrumento compartilhado) → anel na cor da pessoa
    pub owner: Option<String>,
    /// dono mudo (submix silenciado) → disco transparente
    pub owner_muted: bool,
}

/// Eventos por linha (cada vetor em ordem de tick) + duração de uma passada.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaneData {
    pub by_track: Vec<Vec<NoteEvent>>,
    pub len_ticks: f64,
}

impl LaneData {
    pub fn count(&self) -> usize {
        self.by_track.iter().map(Vec::len).sum()
    }
}

/// ticks → segundos no andamento `bpm`.
pub fn tick_secs(tick: f64, bpm: f64) -> f64 {
    tick / PPQ * 60.0 / bpm.max(1.0)
}

/// Discos da música para a ordem de execução `order` (saída de `play_order`:
/// compasso tocado → compasso-fonte). Espelha `noteEvents` do `lane.ts`, com a
/// matemática de tempo do compilador do áudio (offset do golpe + swing).
pub fn note_events(song: &Song, order: &[usize]) -> LaneData {
    let swing = song.swing();
    let mut cell_start = Vec::with_capacity(song.measures.len());
    let mut acc = 0;
    for m in &song.measures {
        cell_start.push(acc);
        acc += m.cell_count();
    }
    let mut by_track: Vec<Vec<NoteEvent>> = vec![Vec::new(); song.tracks.len()];
    // início de cada compasso tocado, em tempos (compassos de fórmulas diferentes)
    let starts = wadawko_core::ops::measures::bar_beat_starts(song, order);
    for (u, &mi) in order.iter().enumerate() {
        let Some(meas) = song.measures.get(mi) else { continue };
        let sub = meas.subdivision.max(1);
        for ci in 0..meas.cell_count() {
            let beat = ci / sub;
            let cell_frac = (ci % sub) as f64 / sub as f64;
            for (ti, track) in song.tracks.iter().enumerate() {
                let Some(syl) = meas.cell(&track.id, ci).filter(|s| !s.is_empty()) else { continue };
                let Some(res) = resolve_cell(syl) else { continue };
                let owner = if track.is_shared() { cell_owner_of(song, track, mi, ci) } else { None };
                let owner_muted = owner.is_some_and(|o| !person_audible(song, Some(o)));
                // agrupa golpes simultâneos (mesmo offset): R+L juntos = uma nota de duas mãos
                let mut groups: Vec<Vec<usize>> = Vec::new();
                for (hi, h) in res.hits.iter().enumerate() {
                    match groups.iter_mut().find(|g| res.hits[g[0]].offset == h.offset) {
                        Some(g) => g.push(hi),
                        None => groups.push(vec![hi]),
                    }
                }
                for g in &groups {
                    let first = &res.hits[g[0]];
                    let both = g.len() >= 2 && g.iter().any(|&i| res.hits[i].hand != first.hand);
                    let beat_frac = swing_warp(cell_frac + first.offset / sub as f64, swing);
                    by_track[ti].push(NoteEvent {
                        track: ti,
                        tick: (starts[u] + beat as f64 + beat_frac) * PPQ,
                        cell: cell_start[mi] + ci,
                        hit: g[0],
                        zone: first.zone,
                        accent: first.accent || both,
                        ghost: first.vel <= 0.25 && !both,
                        both_hands: both,
                        hand: first.hand,
                        label: if g.contains(&0) { syl.to_string() } else { String::new() },
                        owner: owner.map(str::to_string),
                        owner_muted,
                    });
                }
            }
        }
    }
    for v in &mut by_track {
        v.sort_by(|a, b| a.tick.total_cmp(&b.tick));
    }
    LaneData { by_track, len_ticks: starts[order.len()] * PPQ }
}

/// Relógio da esteira: o mapa de andamento da música (rall./accel.) com o
/// engine em `bpm` (o da música, ou o do treino) e a duração de uma passada.
#[derive(Clone)]
pub struct LaneClock {
    pub curve: TempoCurve,
    pub bpm: f64,
    pub len_ticks: f64,
}

impl LaneClock {
    /// Andamento constante (sem marcações).
    #[cfg(test)]
    pub fn constant(bpm: f64) -> Self {
        LaneClock { curve: TempoCurve::constant(bpm), bpm, len_ticks: 0.0 }
    }

    /// Segundos desde o início da passada até `tick`; fora de `0..len` (passadas
    /// vizinhas no loop) soma/subtrai a duração de uma passada.
    pub fn secs(&self, tick: f64) -> f64 {
        let bpm = self.bpm.max(1.0);
        if self.curve.segments.is_empty() || self.len_ticks <= 0.0 {
            return tick_secs(tick, bpm);
        }
        let lap = self.curve.seconds_to(self.len_ticks, bpm);
        let k = (tick / self.len_ticks).floor();
        self.curve.seconds_to(tick - k * self.len_ticks, bpm) + k * lap
    }
}

/// Posição x (px, relativa à linha de julgamento) de um evento em `tick` quando
/// o relógio está em `now_tick` (distância em TEMPO REAL: num rall. as notas se
/// afastam, num accel. se aproximam).
pub fn note_x(tick: f64, now_tick: f64, clock: &LaneClock) -> f32 {
    ((clock.secs(tick) - clock.secs(now_tick)) * PX_PER_SEC as f64) as f32
}

/// Intervalo `[i0, i1)` dos eventos (ordenados por tick) cujo x pode cair em
/// `[-90, width + 48]` — o mesmo corte do `Lane.svelte` (com mudança de andamento,
/// um superconjunto: usa o andamento mais rápido do mapa).
pub fn visible_range(events: &[NoteEvent], now_tick: f64, clock: &LaneClock, width: f32) -> (usize, usize) {
    let ticks_per_px = clock.bpm.max(1.0) * clock.curve.max_factor() / 60.0 * PPQ / PX_PER_SEC as f64;
    let lo = now_tick - 90.0 * ticks_per_px;
    let hi = now_tick + (width as f64 + 48.0) * ticks_per_px;
    (events.partition_point(|e| e.tick < lo), events.partition_point(|e| e.tick <= hi))
}

// ───────────────────────────── partículas ─────────────────────────────

/// `cubic-bezier(x1, y1, x2, y2)` do CSS avaliado em `t` (bisseção em x).
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    let f = |a: f32, b: f32, s: f32| 3.0 * (1.0 - s) * (1.0 - s) * s * a + 3.0 * (1.0 - s) * s * s * b + s * s * s;
    // pontas exatas (a bissecção devolve o meio do último intervalo, ~1e-5 fora)
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..18 {
        let mid = (lo + hi) * 0.5;
        if f(x1, x2, mid) < t { lo = mid } else { hi = mid }
    }
    f(y1, y2, (lo + hi) * 0.5)
}

/// Pétala de cerejeira: estoura pra fora do tambor e cai flutuando.
#[derive(Clone, Copy, Debug)]
struct Petal {
    track: usize,
    born: Instant,
    /// duração (s)
    dur: f32,
    size: f32,
    color: u32,
    bx: f32,
    by: f32,
    drift: f32,
    fall: f32,
    /// giro total (graus)
    spin: f32,
}

impl Petal {
    /// (dx, dy, rotação em rad, escala, opacidade) na idade `age` (s); None = acabou.
    fn pose(&self, age: f32) -> Option<(f32, f32, f32, f32, f32)> {
        if age < 0.0 || age >= self.dur {
            return None;
        }
        let p = cubic_bezier(0.2, 0.7, 0.4, 1.0, age / self.dur);
        let rad = std::f32::consts::PI / 180.0;
        Some(if p < 0.35 {
            let k = p / 0.35;
            (self.bx * k, self.by * k, self.spin * 0.4 * k * rad, 1.0, 1.0)
        } else {
            let k = (p - 0.35) / 0.65;
            (self.bx + self.drift * k, self.by + self.fall * k, (self.spin * 0.4 + self.spin * 0.6 * k) * rad, 1.0 - 0.3 * k, 1.0 - k)
        })
    }
}

/// Estado de efeitos que sobrevive entre frames (pulsos dos tambores + pétalas).
#[derive(Default)]
struct Fx {
    /// (linha, instante audível do golpe)
    pulses: Vec<(usize, Instant)>,
    petals: Vec<Petal>,
}

struct Rng(u64);

impl Rng {
    /// uniforme em [0, 1)
    fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }
}

fn spawn_petals(fx: &mut Fx, rng: &mut Rng, track: usize, at: Instant) {
    let count = 4 + (rng.unit() * 3.0) as usize; // 4–6 pétalas
    for _ in 0..count {
        if fx.petals.len() >= MAX_PETALS {
            return;
        }
        let angle = -std::f32::consts::FRAC_PI_2 + (rng.unit() - 0.5) * std::f32::consts::PI * 1.6;
        let burst = 26.0 + rng.unit() * 44.0;
        fx.petals.push(Petal {
            track,
            born: at,
            dur: (700.0 + rng.unit() * 500.0) / 1000.0,
            size: 6.0 + rng.unit() * 7.0,
            color: PETAL_COLORS[((rng.unit() * PETAL_COLORS.len() as f32) as usize).min(PETAL_COLORS.len() - 1)],
            bx: angle.cos() * burst,
            by: angle.sin() * burst,
            fall: 34.0 + rng.unit() * 30.0,
            drift: (rng.unit() - 0.5) * 36.0,
            spin: (rng.unit() - 0.5) * 720.0,
        });
    }
}

// ───────────────────────────── view ─────────────────────────────

type TextCache = Rc<RefCell<HashMap<(SharedString, u32, u8), ShapedLine>>>;

pub struct LaneView {
    pub state: Entity<AppState>,
    /// eventos em cache: (revisão, seções selecionadas) → dados
    cache: Option<(u64, Vec<String>, Rc<LaneData>)>,
    /// golpes de `s.hits` já transformados em pulso/pétalas
    seen: Vec<(usize, usize, usize, Instant)>,
    fx: Rc<RefCell<Fx>>,
    rng: Rng,
    shaped: TextCache,
    shaped_key: (u64, bool),
    // easter egg do fue
    logo_clicks: u32,
    logo_last: Instant,
    fue_until: Option<Instant>,
}

impl LaneView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        // re-renderiza quando a música/tema/transporte mudam
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) | 1;
        LaneView {
            state,
            cache: None,
            seen: Vec::new(),
            fx: Rc::new(RefCell::new(Fx::default())),
            rng: Rng(seed),
            shaped: Rc::new(RefCell::new(HashMap::new())),
            shaped_key: (u64::MAX, false),
            logo_clicks: 0,
            logo_last: Instant::now(),
            fue_until: None,
        }
    }

    /// Easter egg 🌸: 5 cliques seguidos no logo (≤ 1,5 s entre eles) tocam
    /// "Sakura Sakura" no fue. Chamado pelo logo da topbar.
    pub fn logo_click(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        if now.duration_since(self.logo_last) > Duration::from_millis(1500) {
            self.logo_clicks = 0;
        }
        self.logo_last = now;
        self.logo_clicks += 1;
        if self.logo_clicks < 5 {
            return;
        }
        self.logo_clicks = 0;
        if self.fue_until.is_some_and(|t| now < t) {
            return;
        }
        let dur_ms = self.state.update(cx, |s, cx| {
            let ms = s.audio.as_mut().map(|a| a.play_fue());
            match ms {
                Some(ms) if ms > 0 => s.flash("🌸 Sakura Sakura"),
                Some(_) => {}
                None => s.flash("carregando samples…"),
            }
            cx.notify();
            ms.unwrap_or(0)
        });
        self.fue_until = Some(now + Duration::from_millis(dur_ms.max(1600)));
        cx.notify();
    }

    fn data(&mut self, song: &Song, revision: u64, selected: &[String]) -> Rc<LaneData> {
        if let Some((rev, sel, d)) = &self.cache
            && *rev == revision
            && sel == selected
        {
            return d.clone();
        }
        // mesma ordem que o áudio compila (`AppState::sync_audio`)
        let order = play_order(song, selected_region(song, selected));
        let d = Rc::new(note_events(song, &order));
        self.cache = Some((revision, selected.to_vec(), d.clone()));
        d
    }
}

struct PaintArgs {
    song: Arc<Song>,
    data: Rc<LaneData>,
    th: Theme,
    /// tick do relógio (audível tocando; posição inicial parado)
    now_tick: f64,
    clock: LaneClock,
    /// tocando em loop: a passada seguinte/anterior também aparece
    wrap: bool,
    fx: Rc<RefCell<Fx>>,
    shaped: TextCache,
}

impl Render for LaneView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.clone();
        let st = state.read(cx);
        let th = st.theme;
        let song = st.song().clone();
        let playing = st.playing();
        let data = self.data(&song, st.revision, &st.tp.selected);

        // cache de texto: vale por (revisão, tema)
        if self.shaped_key != (st.revision, th.dark) {
            self.shaped_key = (st.revision, th.dark);
            self.shaped.borrow_mut().clear();
        }

        // golpes novos → pulso do tambor + pétalas (nascem no instante audível)
        let now = Instant::now();
        {
            let mut fx = self.fx.borrow_mut();
            let keys: Vec<_> = st.hits.iter().map(|h| (h.track, h.cell, h.hit, h.at)).collect();
            for k in &keys {
                if !self.seen.contains(k) && !fx.pulses.contains(&(k.0, k.3)) {
                    // duas mãos no mesmo instante = um estouro só
                    fx.pulses.push((k.0, k.3));
                    spawn_petals(&mut fx, &mut self.rng, k.0, k.3);
                }
            }
            self.seen = keys;
            fx.pulses.retain(|(_, at)| now < *at + Duration::from_millis(400));
            fx.petals.retain(|p| now < p.born + Duration::from_secs_f32(p.dur));
            // anima só enquanto toca ou enquanto sobram efeitos no ar
            if playing || !fx.pulses.is_empty() || !fx.petals.is_empty() {
                window.request_animation_frame();
            }
        }

        // relógio: tocando = tick audível; parado = posição inicial do play
        let tm = &st.schedule.time;
        let now_tick = if playing {
            st.audible_tick().unwrap_or(0.0)
        } else if st.schedule.has_region {
            0.0
        } else {
            tm.cell_tick(st.start_cell)
        };
        let bpm = if playing { st.playback_bpm as f64 } else { song.tempo };
        let clock = LaneClock { curve: st.schedule.curve.clone(), bpm, len_ticks: st.schedule.len_ticks };
        let n = song.tracks.len();
        let total_h = (n as f32 * (LANE_H + LANE_GAP) - LANE_GAP).max(0.0);
        let empty = data.count() == 0;
        let args = PaintArgs { song, data, th, now_tick, clock, wrap: playing && st.tp.looping, fx: self.fx.clone(), shaped: self.shaped.clone() };

        div()
            .w_full()
            .flex()
            .flex_col()
            .pt(px(8.0))
            .pb(px(16.0))
            .child(canvas(|_, _, _| (), move |b, _, window, cx| paint_lane(b, &args, window, cx)).w_full().h(px(total_h)))
            .when(empty, |d| d.child(div().px(px(8.0)).py(px(16.0)).text_size(px(12.8)).text_color(th.ink_soft).child("Sem notas ainda. Escreva na grade, depois volte pra tocar. (´-ω-`)")))
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
}

/// círculo de diâmetro `d` centrado em (cx, cy): fundo + borda interna
fn disc(window: &mut Window, cx: f32, cy: f32, d: f32, bg: Hsla, border: f32, border_color: Hsla) {
    window.paint_quad(quad(rect(cx - d * 0.5, cy - d * 0.5, d, d), px(d * 0.5), bg, px(border), border_color, BorderStyle::Solid));
}

/// anel (só contorno) de diâmetro externo `d`
fn ring(window: &mut Window, cx: f32, cy: f32, d: f32, width: f32, color: Hsla) {
    disc(window, cx, cy, d, gpui_kit::transparent_black(), width, color);
}

fn mul_alpha(c: Hsla, a: f32) -> Hsla {
    Hsla { a: c.a * a, ..c }
}

/// pétala: folha com dois "bicos" (o `border-radius: 80% 10% 80% 10%` do web),
/// centrada na origem — tesselada uma vez por (rotação, tamanho) e guardada
fn build_petal(w: f32, rot: f32) -> Option<gpui_kit::Path<Pixels>> {
    let h = w * 0.82;
    let (s, c) = rot.sin_cos();
    let tr = |px_: f32, py_: f32| point(px(px_ * c - py_ * s), px(px_ * s + py_ * c));
    let mut p = PathBuilder::fill();
    p.move_to(tr(w * 0.5, -h * 0.5));
    p.curve_to(tr(-w * 0.5, h * 0.5), tr(-w * 0.62, -h * 0.62));
    p.curve_to(tr(w * 0.5, -h * 0.5), tr(w * 0.62, h * 0.62));
    p.close();
    p.build().ok()
}

thread_local! {
    /// pétalas pré-tesseladas: (passo de rotação, largura em px inteiros) → path na origem
    static PETALS: RefCell<HashMap<(u8, u8), gpui_kit::Path<Pixels>>> = RefCell::new(HashMap::new());
}

/// Pinta uma pétala copiando a forma pré-tesselada mais próxima (rotação em
/// passos de 15°, largura em px) e transladando os vértices — sem rodar o
/// tesselador a cada frame (era o grosso do custo da esteira).
fn paint_petal(window: &mut Window, x: f32, y: f32, w: f32, rot: f32, color: Hsla) {
    let turn = std::f32::consts::TAU / PETAL_ROT_STEPS as f32;
    let step = (rot.rem_euclid(std::f32::consts::TAU) / turn).round() as usize % PETAL_ROT_STEPS;
    let wpx = w.round().clamp(2.0, 40.0) as u8;
    let path = PETALS.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(p) = c.get(&(step as u8, wpx)) {
            return Some(p.clone());
        }
        let p = build_petal(wpx as f32, step as f32 * turn)?;
        c.insert((step as u8, wpx), p.clone());
        Some(p)
    });
    let Some(mut path) = path else { return };
    let d = point(px(x), px(y));
    for v in &mut path.vertices {
        v.xy_position += d;
    }
    path.bounds.origin += d;
    window.paint_path(path, color);
}

fn paint_lane(b: Bounds<Pixels>, a: &PaintArgs, window: &mut Window, cx: &mut App) {
    let visible = window.content_mask().bounds.intersect(&b);
    if f32::from(visible.size.width) <= 0.0 || f32::from(visible.size.height) <= 0.0 {
        return;
    }
    let (song, th) = (&*a.song, &a.th);
    let ox = f32::from(b.origin.x);
    let oy = f32::from(b.origin.y);
    let w = f32::from(b.size.width);
    let now = Instant::now();
    let white: Hsla = rgb(0xffffff).into();
    let fuchi: Hsla = th.accent2;

    let mut cache = a.shaped.borrow_mut();
    // texto modelado em cache; `center_w` centraliza numa largura
    let mut text = |window: &mut Window, cx: &mut App, t: &SharedString, size_px: u32, color: Hsla, ckey: u8, x: f32, y: f32, center_w: Option<f32>| {
        if t.is_empty() {
            return;
        }
        let line = cache.entry((t.clone(), size_px, ckey)).or_insert_with(|| {
            let mut run = window.text_style().to_run(t.len());
            run.color = color;
            run.font.weight = FontWeight::BOLD;
            window.text_system().shape_line(t.clone(), px(size_px as f32), &[run], None)
        });
        let lw = f32::from(line.width);
        let tx = match center_w {
            Some(cw) => x + (cw - lw) * 0.5,
            None => x,
        };
        let _ = line.paint(point(px(tx), px(y)), px(size_px as f32 * 1.3), TextAlign::Left, None, window, cx);
    };

    let colors: HashMap<&str, Hsla> = song.people().iter().filter_map(|p| Some((p.id.as_str(), parse_color(p.color.as_deref()?)?))).collect();
    let rails = rails_for_tracks(song);
    let gutter = if rails.is_empty() { 0.0 } else { rails.len() as f32 * (RAIL_W + RAIL_GAP) - RAIL_GAP + RAIL_PAD };
    let drum_x = ox + gutter;
    let field_x = drum_x + DRUM_W;
    let field_w = (ox + w - field_x).max(0.0);
    let lane_y = |t: usize| oy + t as f32 * (LANE_H + LANE_GAP);
    let fx = a.fx.borrow();

    // ── trilhos de pessoa (gutter): barra contínua por trecho + nome empilhado ──
    for (ri, rail) in rails.iter().enumerate() {
        let rx = ox + ri as f32 * (RAIL_W + RAIL_GAP);
        let col = rail.person.color.as_deref().and_then(parse_color).unwrap_or(th.accent);
        for t in 0..rail.rows.len() {
            if !rail.rows[t] {
                continue;
            }
            let extend = rail.rows.get(t + 1).copied().unwrap_or(false);
            let cap_top = !(t > 0 && rail.rows[t - 1]);
            let y = lane_y(t);
            let h = if extend { LANE_H + LANE_GAP } else { LANE_H };
            let r = |on: bool| px(if on { 2.0 } else { 0.0 });
            window.paint_quad(quad(
                rect(rx, y, 3.0, h),
                gpui_kit::Corners { top_left: r(cap_top), top_right: r(cap_top), bottom_left: r(!extend), bottom_right: r(!extend) },
                col,
                px(0.0),
                gpui_kit::transparent_black(),
                BorderStyle::Solid,
            ));
            if cap_top {
                // o web escreve o nome na vertical; aqui, uma letra por linha até onde o trecho vai
                let run = (t..rail.rows.len()).take_while(|&i| rail.rows[i]).count();
                let room = run as f32 * (LANE_H + LANE_GAP) - LANE_GAP - 2.0;
                for (i, ch) in rail.person.name.chars().take((room / 10.0) as usize).enumerate() {
                    text(window, cx, &SharedString::from(ch.to_string()), 9, col, 10 + (ri % 200) as u8, rx + 4.0, y + 1.0 + i as f32 * 10.0, Some(RAIL_W - 4.0));
                }
            }
        }
    }

    for (t, track) in song.tracks.iter().enumerate() {
        let y = lane_y(t);
        let mid = y + LANE_H * 0.5;
        if y > f32::from(visible.origin.y + visible.size.height) + 80.0 || y + LANE_H < f32::from(visible.origin.y) - 80.0 {
            continue;
        }

        // ── campo: fundo listrado, borda, linha de julgamento ──
        let field = rect(field_x, y, field_w, LANE_H);
        window.paint_quad(quad(field, gpui_kit::Corners { top_left: px(0.0), bottom_left: px(0.0), top_right: px(6.0), bottom_right: px(6.0) }, th.paper, px(1.0), th.line, BorderStyle::Solid));
        window.with_content_mask(Some(gpui_kit::ContentMask { bounds: rect(field_x, y + 1.0, (field_w - 1.0).max(0.0), LANE_H - 2.0) }), |window| {
            let mut sx = field_x;
            while sx < field_x + field_w {
                window.paint_quad(fill(rect(sx, y, 1.0, LANE_H), th.paper_raise));
                sx += STRIPE;
            }
            window.paint_quad(fill(rect(field_x, y, 3.0, LANE_H), Theme::alpha(th.ink, 0.35)));

            // ── discos visíveis ──
            let events = a.data.by_track.get(t).map(Vec::as_slice).unwrap_or(&[]);
            let track_dim = if song.audible(t) { 1.0 } else { 0.3 };
            let len = a.data.len_ticks;
            let shifts: &[f64] = if a.wrap && len > 0.0 { &[-1.0, 0.0, 1.0] } else { &[0.0] };
            for &k in shifts {
                let now_tick = a.now_tick - k * len;
                let (i0, i1) = visible_range(events, now_tick, &a.clock, field_w);
                for e in &events[i0..i1] {
                    let x = note_x(e.tick, now_tick, &a.clock);
                    // some ao passar da linha de julgamento (consumida)
                    let fade = if x < 0.0 { (1.0 + x / 80.0).max(0.0) } else { 1.0 };
                    let op = fade * if e.ghost { 0.5 } else { 1.0 } * if e.owner_muted { 0.3 } else { 1.0 } * track_dim;
                    if op <= 0.004 {
                        continue;
                    }
                    let (d, bw, fs) = if e.ghost {
                        (19.2, 1.0, 0)
                    } else if e.both_hands {
                        (49.6, 3.0, 11)
                    } else if e.accent {
                        (46.4, 3.0, 11)
                    } else {
                        (35.2, 2.0, 10)
                    };
                    let ncx = field_x + x + d * 0.5;
                    let base = if e.zone == Zone::Rim { fuchi } else { th.accent };
                    let owner_col = e.owner.as_deref().and_then(|o| colors.get(o)).copied();
                    if !e.ghost {
                        // sombra
                        disc(window, ncx, mid + 1.5, d + 1.0, gpui_kit::transparent_black(), 2.0, mul_alpha(gpui_kit::black(), 0.16 * op));
                    }
                    if let Some(oc) = owner_col {
                        // instrumento compartilhado: anel na cor do dono
                        ring(window, ncx, mid, d + 6.0, 3.0, mul_alpha(oc, op));
                    } else if e.both_hands {
                        ring(window, ncx, mid, d + 4.0, 2.0, mul_alpha(th.ink, op));
                    }
                    disc(window, ncx, mid, d, mul_alpha(base, op), bw, mul_alpha(white, 0.85 * op));
                    if e.both_hands {
                        // anel duplo: segundo anel branco por dentro
                        ring(window, ncx, mid, d - 11.0, 2.0, mul_alpha(white, 0.9 * op));
                    }
                    if fs > 0 && !e.label.is_empty() && op > 0.25 {
                        text(window, cx, &SharedString::from(e.label.clone()), fs, white, 0, ncx - d * 0.5, mid - fs as f32 * 0.65, Some(d));
                    }
                }
            }
        });

        // ── tambor-alvo (pulsa no golpe) ──
        let dcx = drum_x + DRUM_W * 0.5;
        let age = fx.pulses.iter().filter(|(pt, at)| *pt == t && *at <= now).map(|(_, at)| now.duration_since(*at).as_secs_f32() * 1000.0).fold(f32::INFINITY, f32::min);
        // escala: sobe em 60 ms, segura até 240 ms, volta em 60 ms
        let amount = if age < 60.0 {
            1.0 - (1.0 - age / 60.0).powi(2)
        } else if age < PULSE_MS {
            1.0
        } else if age < PULSE_MS + 60.0 {
            (1.0 - (age - PULSE_MS) / 60.0).powi(2)
        } else {
            0.0
        };
        let fd = FACE * (1.0 + 0.16 * amount);
        disc(window, dcx, mid + 2.0, fd + 2.0, gpui_kit::transparent_black(), 3.0, mul_alpha(gpui_kit::black(), 0.10 + 0.06 * amount));
        if amount > 0.0 {
            ring(window, dcx, mid, fd + 6.0, 3.0, mul_alpha(th.accent, amount));
        }
        disc(window, dcx, mid, fd, th.paper_raise, 3.0, th.ink);
        ring(window, dcx, mid, fd - 6.0, 4.0, th.paper);
        disc(window, dcx, mid, fd - 14.0, th.paper_hover, 0.0, gpui_kit::transparent_black());
        if age < PULSE_MS {
            // anel que estoura: escala 1 → 2,1, opacidade 0,85 → 0
            let k = age / PULSE_MS;
            let ek = 1.0 - (1.0 - k) * (1.0 - k);
            ring(window, dcx, mid, FACE * (1.0 + 1.1 * ek), 3.0, mul_alpha(th.accent, 0.85 * (1.0 - ek)));
        }
        let name = SharedString::from(track.name.clone());
        text(window, cx, &name, 9, th.ink_soft, 1, drum_x, y + LANE_H - 7.5, Some(DRUM_W));
    }

    // ── pétalas (por cima de tudo, saem do centro do tambor) ──
    let dcx = drum_x + DRUM_W * 0.5;
    for p in &fx.petals {
        if now < p.born {
            continue;
        }
        let Some((dx, dy, rot, scale, op)) = p.pose(now.duration_since(p.born).as_secs_f32()) else { continue };
        let col: Hsla = rgb(p.color).into();
        paint_petal(window, dcx + dx, lane_y(p.track) + LANE_H * 0.5 + dy, p.size * scale, rot, mul_alpha(col, op));
    }
}

/// Handlers das ações desta view (a esteira não tem ações próprias: usa o
/// transporte global; o easter egg entra por [`LaneView::logo_click`]).
pub fn bind(d: gpui_kit::Stateful<gpui_kit::Div>, _cx: &mut Context<Shell>) -> gpui_kit::Stateful<gpui_kit::Div> {
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use wadawko_audio::BAR_TICKS;
    use wadawko_core::demo::demo_song;
    use wadawko_core::ops;

    fn identity(song: &Song) -> Vec<usize> {
        (0..song.measures.len()).collect()
    }

    /// golpes da música contados direto das células (instantes distintos por célula)
    fn expected_count(song: &Song) -> usize {
        let mut n = 0;
        for meas in &song.measures {
            for track in &song.tracks {
                for ci in 0..meas.cell_count() {
                    if let Some(res) = meas.cell(&track.id, ci).and_then(resolve_cell) {
                        let mut offs: Vec<f64> = res.hits.iter().map(|h| h.offset).collect();
                        offs.sort_by(f64::total_cmp);
                        offs.dedup();
                        n += offs.len();
                    }
                }
            }
        }
        n
    }

    #[test]
    fn count_matches_song_hits() {
        let song = demo_song();
        let d = note_events(&song, &identity(&song));
        assert!(d.count() > 20);
        assert_eq!(d.count(), expected_count(&song));
        assert_eq!(d.by_track.len(), song.tracks.len());
        assert_eq!(d.len_ticks, song.measures.len() as f64 * BAR_TICKS);
        // "doko" = dois golpes na mesma célula, só o 1º leva a sílaba
        let chu = &d.by_track[1];
        let doko: Vec<_> = chu.iter().filter(|e| e.cell == 4).collect();
        assert_eq!(doko.len(), 2);
        assert_eq!((doko[0].label.as_str(), doko[1].label.as_str()), ("doko", ""));
        assert!(doko[1].tick > doko[0].tick);
    }

    #[test]
    fn times_are_sorted_and_inside_the_song() {
        let song = demo_song();
        let d = note_events(&song, &identity(&song));
        for v in &d.by_track {
            assert!(v.windows(2).all(|w| w[0].tick <= w[1].tick));
            assert!(v.iter().all(|e| e.tick >= 0.0 && e.tick < d.len_ticks));
        }
        // 1ª célula do 2º compasso da linha 0 cai exatamente no início do compasso
        let e = d.by_track[0].iter().find(|e| e.cell == 8).unwrap();
        assert_eq!(e.tick, BAR_TICKS);
        // a 100 BPM um tempo dura 0,6 s
        assert!((tick_secs(PPQ, 100.0) - 0.6).abs() < 1e-12);
    }

    #[test]
    fn matches_the_audio_schedule_ticks() {
        // mesmos instantes que o compilador do áudio (sem precisar do banco de
        // samples: comparamos com a fórmula de `TimeMap::cell_tick` na cabeça da célula)
        let song = demo_song();
        let order = identity(&song);
        let tm = wadawko_audio::TimeMap::new(&song, order.clone());
        let d = note_events(&song, &order);
        for e in d.by_track.iter().flatten().filter(|e| e.hit == 0) {
            assert!((e.tick - tm.cell_tick(e.cell)).abs() < 1e-9, "célula {}", e.cell);
            assert_eq!(tm.cell_at(e.tick + 1.0), Some(e.cell));
        }
    }

    #[test]
    fn swing_delays_the_offbeat() {
        let mut song = demo_song();
        let straight = note_events(&song, &identity(&song));
        song.swing = Some(1.0);
        let swung = note_events(&song, &identity(&song));
        assert_eq!(straight.count(), swung.count());
        // linha 0, células 0 (tempo) e 1 (contratempo, sub = 2)
        let at = |d: &LaneData, cell: usize| d.by_track[0].iter().find(|e| e.cell == cell).unwrap().tick;
        assert_eq!(at(&straight, 1), PPQ * 0.5);
        assert_eq!(at(&swung, 0), 0.0);
        assert!((at(&swung, 1) - PPQ * 2.0 / 3.0).abs() < 1e-9);
        for v in &swung.by_track {
            assert!(v.windows(2).all(|w| w[0].tick <= w[1].tick));
        }
    }

    #[test]
    fn repeats_unroll_the_timeline() {
        let mut song = demo_song();
        let once = note_events(&song, &identity(&song));
        let id = ops::sections::append_section(&mut song, Some("A"));
        let n_after = song.measures.len();
        let plain = note_events(&song, &play_order(&song, None));
        ops::sections::set_section_repeat(&mut song, &id, 3);
        let order = play_order(&song, None);
        assert!(order.len() > n_after);
        let rep = note_events(&song, &order);
        assert_eq!(rep.len_ticks, order.len() as f64 * BAR_TICKS);
        assert!(rep.count() >= plain.count() && plain.count() >= once.count());
        // repetir a música inteira 2× dobra os discos, com as mesmas chaves de célula
        let n = demo_song().measures.len();
        let twice: Vec<usize> = (0..n).chain(0..n).collect();
        let d2 = note_events(&demo_song(), &twice);
        assert_eq!(d2.count(), once.count() * 2);
        let first = &once.by_track[1];
        let second: Vec<_> = d2.by_track[1].iter().skip(first.len()).collect();
        for (a, b) in first.iter().zip(second) {
            assert_eq!((a.cell, a.hit), (b.cell, b.hit));
            assert_eq!(b.tick, a.tick + n as f64 * BAR_TICKS);
        }
        // índice de compasso fora da música é ignorado (ordem velha após edição)
        assert_eq!(note_events(&demo_song(), &[0, 99]).count(), note_events(&demo_song(), &[0]).count());
    }

    #[test]
    fn owner_only_on_shared_tracks() {
        let mut song = demo_song();
        let ana = ops::people::add_person(&mut song, Some("Ana"));
        let bia = ops::people::add_person(&mut song, Some("Bia"));
        ops::people::toggle_track_person(&mut song, "chu1", &ana);
        // uma pessoa só: sem dono/anel
        let d = note_events(&song, &identity(&song));
        assert!(d.by_track[1].iter().all(|e| e.owner.is_none() && !e.owner_muted));
        // compartilhada: dono padrão = 1ª pessoa; célula atribuída = a outra
        ops::people::toggle_track_person(&mut song, "chu1", &bia);
        ops::people::assign_cell_owner(&mut song, "chu1", 0, 2, Some(&bia));
        let d = note_events(&song, &identity(&song));
        let chu = &d.by_track[1];
        assert!(chu.iter().all(|e| e.owner.is_some()));
        assert_eq!(chu.iter().find(|e| e.cell == 0).unwrap().owner.as_deref(), Some(ana.as_str()));
        assert_eq!(chu.iter().find(|e| e.cell == 2).unwrap().owner.as_deref(), Some(bia.as_str()));
        assert!(d.by_track[0].iter().all(|e| e.owner.is_none()));
        // dono mudo → nota marcada como silenciada
        song.people.as_mut().unwrap().iter_mut().find(|p| p.id == bia).unwrap().muted = Some(true);
        let d = note_events(&song, &identity(&song));
        let chu = &d.by_track[1];
        assert!(chu.iter().find(|e| e.cell == 2).unwrap().owner_muted);
        assert!(!chu.iter().find(|e| e.cell == 0).unwrap().owner_muted);
    }

    #[test]
    fn zones_accents_and_ghosts() {
        let song = demo_song();
        let d = note_events(&song, &identity(&song));
        let chu = &d.by_track[1];
        let by_label = |l: &str| chu.iter().find(|e| e.label == l).unwrap();
        assert!(by_label("DON").accent && by_label("DON").zone == Zone::Head);
        assert!(!by_label("don").accent && !by_label("don").ghost);
        assert_eq!(by_label("ka").zone, Zone::Rim);
        assert!(by_label("tsu").ghost);
    }

    #[test]
    fn rallentando_afasta_as_notas() {
        use wadawko_core::TempoMark;
        // 2 compassos 4/4; rall. 100 → 50 no 2º, chegando na hora (span 0) = metade da velocidade
        let mut song = demo_song();
        song.tempo = 100.0;
        song.measures.truncate(2);
        song.sections = None;
        ops::measures::set_tempo_mark(&mut song, 1, Some(TempoMark { target: 50.0, span: 0 }));
        let order = identity(&song);
        let segs = ops::measures::tempo_segments(&song, &order);
        let len = 8.0 * PPQ;
        let c = LaneClock { curve: TempoCurve::from_segments(&segs, 100.0), bpm: 100.0, len_ticks: len };
        // 1 tempo a 100 = 0,6 s; 1 tempo a 50 = 1,2 s
        assert!((note_x(PPQ, 0.0, &c) - 0.6 * PX_PER_SEC).abs() < 1e-3);
        assert!((note_x(5.0 * PPQ, 4.0 * PPQ, &c) - 1.2 * PX_PER_SEC).abs() < 1e-3);
        // treino a 50 %: o dobro da distância
        let slow = LaneClock { bpm: 50.0, ..c.clone() };
        assert!((note_x(PPQ, 0.0, &slow) - 1.2 * PX_PER_SEC).abs() < 1e-3);
        // loop: o 1º tempo da passada seguinte vem logo depois do fim desta
        let lap = 4.0 * 0.6 + 4.0 * 1.2;
        assert!((note_x(len + PPQ, len - PPQ, &c) - (1.2 + 0.6) * PX_PER_SEC).abs() < 1e-3);
        assert!((c.secs(len) - lap).abs() < 1e-9);
    }

    #[test]
    fn culling_and_position() {
        let song = demo_song();
        let d = note_events(&song, &identity(&song));
        let ev = &d.by_track[0];
        // 1 s adiante a 100 BPM = 380 px
        let c = LaneClock::constant(100.0);
        assert!((note_x(PPQ / 0.6, 0.0, &c) - PX_PER_SEC).abs() < 1e-3);
        let (i0, i1) = visible_range(ev, 0.0, &c, 600.0);
        assert_eq!(i0, 0);
        assert!(i1 > 0 && i1 < ev.len());
        for (i, e) in ev.iter().enumerate() {
            let x = note_x(e.tick, 0.0, &c);
            assert_eq!((i0..i1).contains(&i), (-90.0..=648.0).contains(&x), "evento {i} x={x}");
        }
        // no fim da música, nada à frente
        let (j0, j1) = visible_range(ev, d.len_ticks + PPQ * 4.0, &c, 600.0);
        assert_eq!(j0, j1);
    }

    #[test]
    fn petal_animation_ends() {
        let p = Petal { track: 0, born: Instant::now(), dur: 1.0, size: 8.0, color: 0xffffff, bx: 10.0, by: -20.0, drift: 5.0, fall: 40.0, spin: 360.0 };
        let (dx, dy, _, scale, op) = p.pose(0.0).unwrap();
        assert_eq!((dx, dy, scale, op), (0.0, 0.0, 1.0, 1.0));
        let (_, dy, _, scale, op) = p.pose(0.999).unwrap();
        assert!(dy > 15.0 && scale < 0.75 && op < 0.05);
        assert!(p.pose(1.0).is_none() && p.pose(-0.1).is_none());
        assert!((cubic_bezier(0.2, 0.7, 0.4, 1.0, 1.0) - 1.0).abs() < 1e-3);
    }
}
