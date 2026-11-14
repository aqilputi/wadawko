//! Bloco flutuante do arrasto de notas.
//!
//! Ao levantar notas da grade, uma cópia do bloco "descola" do lugar (sombra
//! crescendo, leve zoom) e segue o ponteiro, preso pelo ponto onde foi pego.
//! Ele balança como um pêndulo amortecido em torno desse ponto: inclina para
//! trás com a velocidade horizontal, leva um tranco com a aceleração e volta ao
//! prumo quando o mouse para. O destino (onde vai cair) continua marcado na grade.
//!
//! Desenhado pelo Shell num canvas por cima de tudo, sem cache, a cada frame
//! enquanto o arrasto dura. O GPUI não gira texto: os cartões giram (paths) e
//! cada sílaba vai no centro girado do seu cartão, em pé.

use std::time::Instant;

use gpui_kit::{App, Hsla, PathBuilder, Pixels, Point, SharedString, TextAlign, Window, point, px};

use crate::theme::Theme;
use crate::views::grid::GhostCell;

/// rigidez da mola angular (rad/s² por rad) — define o período do balanço (~0,55 s)
const K: f32 = 130.0;
/// amortecimento (rad/s por rad/s): ζ ≈ 0,4 → balança 2–3 vezes e assenta
const C: f32 = 9.0;
/// inclinação de equilíbrio por px/s de velocidade horizontal ("arrasto do ar")
const VEL_TILT: f32 = 0.000_22;
/// tranco angular por px/s² de aceleração horizontal
const ACC_KICK: f32 = 0.004;
/// inclinação máxima (rad, ~24°)
const MAX_TILT: f32 = 0.42;
/// duração da subida (descolar da grade: sombra e zoom), s
const LIFT_S: f32 = 0.18;

/// Estado da física do balanço (sobrevive entre frames; um por arrasto).
#[derive(Default)]
pub struct Swing {
    key: Option<(f32, f32)>,
    theta: f32,
    omega: f32,
    last: Option<(Point<Pixels>, Instant)>,
    vx: f32,
    lift: f32,
}

impl Swing {
    /// Esquece o arrasto anterior (o próximo começa parado e no chão).
    pub fn reset(&mut self) {
        *self = Swing::default();
    }

    /// Avança a física até `now` com o pivô (ponteiro) em `p`. `key` identifica o
    /// arrasto (ponto da pegada): mudou → recomeça. Devolve (inclinação, subida 0..1).
    pub fn step(&mut self, key: (f32, f32), p: Point<Pixels>, now: Instant) -> (f32, f32) {
        if self.key != Some(key) {
            self.reset();
            self.key = Some(key);
        }
        let Some((lp, lt)) = self.last.replace((p, now)) else { return (self.theta, self.lift) };
        let dt = (now - lt).as_secs_f32().clamp(1e-3, 1.0 / 20.0);
        // velocidade suavizada (o mouse chega em degraus) e aceleração dela
        let raw = (f32::from(p.x) - f32::from(lp.x)) / dt;
        let vx = self.vx + (raw - self.vx) * (1.0 - (-dt * 22.0).exp());
        let ax = (vx - self.vx) / dt;
        self.vx = vx;
        let target = (vx * VEL_TILT).clamp(-MAX_TILT * 0.7, MAX_TILT * 0.7);
        // sub-passos (Euler semi-implícito) para ficar estável com frames irregulares
        let n = 4;
        let h = dt / n as f32;
        for i in 0..n {
            let kick = if i == 0 { (ax * ACC_KICK).clamp(-60.0, 60.0) } else { 0.0 };
            let alpha = -K * (self.theta - target) - C * self.omega + kick;
            self.omega += alpha * h;
            self.theta = (self.theta + self.omega * h).clamp(-MAX_TILT, MAX_TILT);
        }
        self.lift = (self.lift + dt / LIFT_S).min(1.0);
        (self.theta, self.lift)
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Pontos de um retângulo arredondado (local, sentido horário).
fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<(f32, f32)> {
    let r = r.min(w * 0.5).min(h * 0.5);
    let mut pts = Vec::with_capacity(20);
    let corners = [(x + w - r, y + r, -90.0f32), (x + w - r, y + h - r, 0.0), (x + r, y + h - r, 90.0), (x + r, y + r, 180.0)];
    for (cx, cy, a0) in corners {
        for i in 0..=4 {
            let a = (a0 + i as f32 * 22.5).to_radians();
            pts.push((cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    pts
}

fn polygon(window: &mut Window, pts: &[Point<Pixels>], color: Hsla) {
    let mut pb = PathBuilder::fill();
    pb.move_to(pts[0]);
    for p in &pts[1..] {
        pb.line_to(*p);
    }
    pb.close();
    if let Ok(path) = pb.build() {
        window.paint_path(path, color);
    }
}

fn polyline(window: &mut Window, pts: &[Point<Pixels>], width: f32, color: Hsla, closed: bool) {
    let mut pb = PathBuilder::stroke(px(width));
    pb.move_to(pts[0]);
    for p in &pts[1..] {
        pb.line_to(*p);
    }
    if closed {
        pb.close();
    }
    if let Ok(path) = pb.build() {
        window.paint_path(path, color);
    }
}

/// Desenha o bloco balançando. `grab` = onde foi pego (janela),
/// `pivot` = ponteiro agora.
#[allow(clippy::too_many_arguments)]
pub fn paint(cells: &[GhostCell], grab: (f32, f32), clone: bool, pivot: Point<Pixels>, theta: f32, lift: f32, th: &Theme, window: &mut Window, cx: &mut App) {
    if cells.is_empty() {
        return;
    }
    let bbox = cells.iter().skip(1).fold(cells[0].bounds, |a, c| a.union(&c.bounds));
    let (bx, by) = (f32::from(bbox.origin.x), f32::from(bbox.origin.y));
    let e = ease_out_cubic(lift);
    // o bloco fica preso pelo ponto onde foi pego (relativo à pegada)
    let rel = (bx - grab.0, by - grab.1);
    let scale = 1.0 + 0.06 * e;
    let (sn, cs) = theta.sin_cos();
    let (px0, py0) = (f32::from(pivot.x), f32::from(pivot.y));
    // local (relativo ao canto do bloco) → janela: desloca, escala e gira em torno do pivô
    let tr = |x: f32, y: f32| {
        let (lx, ly) = ((x + rel.0) * scale, (y + rel.1) * scale);
        point(px(px0 + lx * cs - ly * sn), px(py0 + lx * sn + ly * cs))
    };

    let local: Vec<(f32, f32, f32, f32, Option<&SharedString>)> = cells
        .iter()
        .map(|c| (f32::from(c.bounds.origin.x) - bx + 1.0, f32::from(c.bounds.origin.y) - by + 1.0, f32::from(c.bounds.size.width) - 2.0, f32::from(c.bounds.size.height) - 2.0, c.text.as_ref()))
        .collect();
    let edge = if clone { th.accent2 } else { th.accent };

    // sombra (não gira junto: a luz vem de cima), cresce com a subida
    let (sdx, sdy) = (4.0 * e, 11.0 * e);
    for &(x, y, w, h, _) in &local {
        let pts: Vec<_> = rounded_rect(x, y, w, h, 5.0).into_iter().map(|(x, y)| {
            let p = tr(x, y);
            point(p.x + px(sdx), p.y + px(sdy))
        }).collect();
        polygon(window, &pts, Theme::alpha(gpui_kit::black(), 0.16 * e));
    }
    // cartões + sílabas
    for &(x, y, w, h, text) in &local {
        let pts: Vec<_> = rounded_rect(x, y, w, h, 5.0).into_iter().map(|(x, y)| tr(x, y)).collect();
        match text {
            Some(_) => {
                polygon(window, &pts, th.paper);
                polygon(window, &pts, Theme::alpha(edge, 0.10));
                polyline(window, &pts, 1.6, edge, true);
            }
            None => {
                polygon(window, &pts, Theme::alpha(th.paper, 0.55));
                polyline(window, &pts, 1.0, Theme::alpha(edge, 0.45), true);
            }
        }
        if let Some(t) = text {
            let mut run = window.text_style().to_run(t.len());
            run.color = th.ink;
            let size = 13.0 * scale;
            let line = window.text_system().shape_line(t.clone(), px(size), &[run], None);
            let c = tr(x + w * 0.5, y + h * 0.5);
            let lh = size * 1.3;
            let _ = line.paint(point(c.x - line.width * 0.5, c.y - px(lh * 0.5)), px(lh), TextAlign::Left, None, window, cx);
        }
    }

    // selo "+" quando é cópia (Alt), no canto de cima à direita do bloco
    if clone {
        let w_all = f32::from(bbox.size.width);
        let c = tr(w_all, 0.0);
        let r = 8.5;
        let disc: Vec<_> = (0..16).map(|i| {
            let a = i as f32 / 16.0 * std::f32::consts::TAU;
            point(c.x + px(r * a.cos()), c.y + px(r * a.sin()))
        }).collect();
        polygon(window, &disc, th.accent2);
        polyline(window, &[point(c.x - px(4.0), c.y), point(c.x + px(4.0), c.y)], 2.0, th.on_accent, false);
        polyline(window, &[point(c.x, c.y - px(4.0)), point(c.x, c.y + px(4.0))], 2.0, th.on_accent, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn run(sw: &mut Swing, xs: impl Iterator<Item = f32>, t0: Instant, dt: f32) -> Vec<f32> {
        xs.enumerate().map(|(i, x)| sw.step((0.0, 0.0), point(px(x), px(0.0)), t0 + Duration::from_secs_f32(i as f32 * dt)).0).collect()
    }

    #[test]
    fn swings_back_when_moving_right_and_settles_when_still() {
        let mut sw = Swing::default();
        let t0 = Instant::now();
        // 0,5 s indo para a direita a 1000 px/s, depois 2 s parado
        let moving = run(&mut sw, (0..30).map(|i| i as f32 * 1000.0 / 60.0), t0, 1.0 / 60.0);
        assert!(moving.last().unwrap() > &0.05, "inclina para trás ao andar para a direita: {:?}", moving.last());
        let t1 = t0 + Duration::from_secs_f32(30.0 / 60.0);
        let x_end = 29.0 * 1000.0 / 60.0;
        let still = run(&mut sw, std::iter::repeat_n(x_end, 120), t1, 1.0 / 60.0);
        // passa do prumo (balança) e depois assenta
        assert!(still.iter().any(|&a| a < 0.0), "deveria balançar para o outro lado");
        assert!(still.last().unwrap().abs() < 0.01, "assenta: {:?}", still.last());
    }

    #[test]
    fn tilt_is_bounded_and_lift_completes() {
        let mut sw = Swing::default();
        let t0 = Instant::now();
        // puxões absurdos alternados
        let a = run(&mut sw, (0..60).map(|i| if i % 2 == 0 { 0.0 } else { 3000.0 }), t0, 1.0 / 60.0);
        assert!(a.iter().all(|t| t.abs() <= MAX_TILT + 1e-6));
        assert_eq!(sw.lift, 1.0);
        // outro arrasto (outra pegada) recomeça do zero
        let (theta, lift) = sw.step((5.0, 5.0), point(px(0.0), px(0.0)), t0);
        assert_eq!((theta, lift), (0.0, 0.0));
    }

    #[test]
    fn rounded_rect_stays_inside_box() {
        let pts = rounded_rect(10.0, 20.0, 40.0, 30.0, 5.0);
        assert_eq!(pts.len(), 20);
        assert!(pts.iter().all(|&(x, y)| (10.0 - 1e-3..=50.0 + 1e-3).contains(&x) && (20.0 - 1e-3..=50.0 + 1e-3).contains(&y)));
    }
}
