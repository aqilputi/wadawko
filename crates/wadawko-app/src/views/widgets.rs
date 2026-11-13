//! Controles das páginas Mixer / Pessoas / Kit: slider com arrasto (horizontal
//! ou vertical), botões com id dinâmico que já são alvo de salto (Ctrl+Space) e
//! atalhos para editar a música a partir de closures de clique.
//!
//! Teclado: clicar (ou saltar com Ctrl+Space) num slider o deixa "em foco"
//! (anel azul); ←/↓ diminuem e →/↑ aumentam um passo; Esc tira o foco. As setas
//! são interceptadas na fase de captura em [`bind`], só nas páginas destas views.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use gpui_kit::{
    App, BorderStyle, Bounds, Context, CursorStyle, DispatchPhase, Entity, FontWeight, HitboxBehavior, Hsla, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    SharedString, Window, canvas, div, point, prelude::*, px, quad, size, transparent_black,
};
use wadawko_core::model::Song;

use crate::keys;
use crate::shell::Shell;
use crate::state::AppState;
use crate::theme::Theme;
use crate::ui;
use crate::views::Page;

pub type OnChange = Rc<dyn Fn(f64, &mut Window, &mut App)>;
pub type Act = Rc<dyn Fn(&mut Window, &mut App)>;

// ───────────────────────────── lógica pura ─────────────────────────────

/// Prende `v` em `[min, max]` e arredonda para o passo (a partir de `min`).
/// O resultado é limpo de ruído de ponto flutuante (0.30000000000000004 → 0.3)
/// para não sujar o JSON salvo.
pub fn quantize(v: f64, min: f64, max: f64, step: f64) -> f64 {
    let v = v.clamp(min, max);
    let q = if step > 0.0 { min + ((v - min) / step).round() * step } else { v };
    (q.clamp(min, max) * 1e6).round() / 1e6 + 0.0
}

/// Posição 0..1 do valor na faixa (para desenhar o slider).
pub fn frac(v: f64, min: f64, max: f64) -> f64 {
    if max <= min { 0.0 } else { ((v - min) / (max - min)).clamp(0.0, 1.0) }
}

/// Valor da posição `t` (0..1) do slider, já no passo.
pub fn value_at(t: f64, min: f64, max: f64, step: f64) -> f64 {
    quantize(min + t.clamp(0.0, 1.0) * (max - min), min, max, step)
}

/// Tag de coalescência do histórico: um gesto (arrasto) no mesmo controle do
/// mesmo item vira um passo de desfazer.
pub fn tag(kind: &str, id: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    kind.hash(&mut h);
    id.hash(&mut h);
    h.finish()
}

// ───────────────────────────── edição ─────────────────────────────

pub fn edit(state: &Entity<AppState>, cx: &mut App, f: impl FnOnce(&mut Song)) {
    state.update(cx, |s, cx| {
        s.edit(cx, f);
    });
}

pub fn edit_co(state: &Entity<AppState>, cx: &mut App, tag: u64, f: impl FnOnce(&mut Song)) {
    state.update(cx, |s, cx| {
        s.edit_coalesced(cx, tag, f);
    });
}

// ───────────────────────────── slider ─────────────────────────────

#[derive(Clone)]
pub struct Slider {
    /// id único na página, prefixado pela página (`mx-`, `pp-`, `kit-`)
    pub id: SharedString,
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// duplo clique volta para este valor
    pub default: Option<f64>,
    pub vertical: bool,
    /// comprimento em px (0 = ocupa o espaço do pai)
    pub len: f32,
    pub color: Hsla,
    pub on_change: OnChange,
}

thread_local! {
    /// slider em foco (teclado) — guarda o último valor renderizado
    static FOCUS: RefCell<Option<Slider>> = const { RefCell::new(None) };
    /// slider sendo arrastado
    static DRAG: RefCell<Option<SharedString>> = const { RefCell::new(None) };
}

const THUMB: f32 = 7.0; // raio do botão
const THICK: f32 = 18.0; // espessura do elemento

fn is_focused(id: &SharedString) -> bool {
    FOCUS.with(|f| f.borrow().as_ref().is_some_and(|s| &s.id == id))
}

fn is_dragging(id: &SharedString) -> bool {
    DRAG.with(|d| d.borrow().as_ref() == Some(id))
}

fn set_focus(s: &Slider) {
    FOCUS.with(|f| *f.borrow_mut() = Some(s.clone()));
}

/// Tira o foco do slider; `true` se havia algum.
pub fn clear_focus() -> bool {
    FOCUS.with(|f| f.borrow_mut().take().is_some())
}

/// Move o slider em foco `steps` passos; `false` se não há slider em foco
/// cujo id comece com `prefix`.
pub fn nudge(prefix: &str, steps: i32, window: &mut Window, cx: &mut App) -> bool {
    let Some(s) = FOCUS.with(|f| f.borrow().clone()).filter(|s| s.id.starts_with(prefix)) else { return false };
    let v = quantize(s.value + steps as f64 * s.step, s.min, s.max, s.step);
    if v != s.value {
        // já reflete o novo valor para toques seguidos antes do próximo render
        FOCUS.with(|f| {
            if let Some(f) = f.borrow_mut().as_mut() {
                f.value = v;
            }
        });
        (s.on_change)(v, window, cx);
    }
    window.refresh();
    true
}

impl Slider {
    fn value_from(&self, pos: Point<Pixels>, b: Bounds<Pixels>) -> f64 {
        let t = if self.vertical {
            let len = (f32::from(b.size.height) - 2.0 * THUMB).max(1.0);
            1.0 - (f32::from(pos.y) - f32::from(b.origin.y) - THUMB) / len
        } else {
            let len = (f32::from(b.size.width) - 2.0 * THUMB).max(1.0);
            (f32::from(pos.x) - f32::from(b.origin.x) - THUMB) / len
        };
        value_at(t as f64, self.min, self.max, self.step)
    }
}

/// Slider com arrasto. Clique/arrasto chamam `on_change` com o valor já no
/// passo; é alvo de salto (o salto põe o foco de teclado nele).
pub fn slider(s: Slider, th: &Theme) -> gpui_kit::Div {
    let th = *th;
    // mantém o foco apontando para o valor/closure deste render
    FOCUS.with(|f| {
        let mut f = f.borrow_mut();
        if f.as_ref().is_some_and(|f| f.id == s.id) {
            *f = Some(s.clone());
        }
    });
    let focused = is_focused(&s.id);
    let active = is_dragging(&s.id);
    let base = div().relative().flex_none();
    let base = match (s.vertical, s.len > 0.0) {
        (true, true) => base.w(px(THICK)).h(px(s.len)),
        (true, false) => base.w(px(THICK)).h_full(),
        (false, true) => base.h(px(THICK)).w(px(s.len)),
        (false, false) => base.h(px(THICK)).w_full(),
    };
    let hint = s.clone();
    let id = s.id.clone();
    let cv = canvas(
        move |b, window, _| window.insert_hitbox(b, HitboxBehavior::Normal),
        move |b, hitbox, window, _| {
            // ── pintura: trilho, preenchimento, botão ──
            let t = frac(s.value, s.min, s.max) as f32;
            let (x0, y0, w, h) = (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height));
            let (track, filled, cx_, cy_) = if s.vertical {
                let len = (h - 2.0 * THUMB).max(1.0);
                let cy = y0 + THUMB + (1.0 - t) * len;
                let x = x0 + w / 2.0 - 2.0;
                (Bounds::new(point(px(x), px(y0 + THUMB)), size(px(4.0), px(len))), Bounds::new(point(px(x), px(cy)), size(px(4.0), px(y0 + THUMB + len - cy))), x0 + w / 2.0, cy)
            } else {
                let len = (w - 2.0 * THUMB).max(1.0);
                let cx = x0 + THUMB + t * len;
                let y = y0 + h / 2.0 - 2.0;
                (Bounds::new(point(px(x0 + THUMB), px(y)), size(px(len), px(4.0))), Bounds::new(point(px(x0 + THUMB), px(y)), size(px(cx - x0 - THUMB), px(4.0))), cx, y0 + h / 2.0)
            };
            window.paint_quad(quad(track, px(2.0), th.line_strong, px(0.0), transparent_black(), BorderStyle::Solid));
            window.paint_quad(quad(filled, px(2.0), s.color, px(0.0), transparent_black(), BorderStyle::Solid));
            let r = if active { THUMB + 1.0 } else { THUMB };
            if focused {
                let ring = Bounds::new(point(px(cx_ - r - 3.0), px(cy_ - r - 3.0)), size(px(2.0 * r + 6.0), px(2.0 * r + 6.0)));
                window.paint_quad(quad(ring, px(r + 3.0), Theme::alpha(th.accent2, 0.25), px(1.5), th.accent2, BorderStyle::Solid));
            }
            let thumb = Bounds::new(point(px(cx_ - r), px(cy_ - r)), size(px(2.0 * r), px(2.0 * r)));
            window.paint_quad(quad(thumb, px(r), s.color, px(2.0), th.paper, BorderStyle::Solid));
            window.set_cursor_style(CursorStyle::PointingHand, &hitbox);

            // ── mouse: clique posiciona, arrasto segue (mesmo fora do elemento) ──
            window.on_mouse_event({
                let s = s.clone();
                let hitbox = hitbox.clone();
                move |ev: &MouseDownEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble || ev.button != MouseButton::Left || !hitbox.is_hovered(window) {
                        return;
                    }
                    set_focus(&s);
                    let v = match s.default {
                        Some(d) if ev.click_count >= 2 => quantize(d, s.min, s.max, s.step),
                        _ => {
                            DRAG.with(|d| *d.borrow_mut() = Some(s.id.clone()));
                            s.value_from(ev.position, b)
                        }
                    };
                    if v != s.value {
                        (s.on_change)(v, window, cx);
                    }
                    window.refresh();
                    cx.stop_propagation();
                }
            });
            window.on_mouse_event({
                let s = s.clone();
                move |ev: &MouseMoveEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble || !is_dragging(&s.id) {
                        return;
                    }
                    if ev.pressed_button != Some(MouseButton::Left) {
                        // soltou fora da janela
                        DRAG.with(|d| *d.borrow_mut() = None);
                        window.refresh();
                        return;
                    }
                    let v = s.value_from(ev.position, b);
                    if v != s.value {
                        (s.on_change)(v, window, cx);
                    }
                }
            });
            window.on_mouse_event({
                let id = s.id.clone();
                move |ev: &MouseUpEvent, phase, window, _| {
                    if phase == DispatchPhase::Bubble && ev.button == MouseButton::Left && is_dragging(&id) {
                        DRAG.with(|d| *d.borrow_mut() = None);
                        window.refresh();
                    }
                }
            });
        },
    )
    .absolute()
    .inset_0();
    keys::hintable(
        base.child(cv),
        id,
        Rc::new(move |w: &mut Window, _: &mut App| {
            set_focus(&hint);
            w.refresh();
        }),
    )
}

// ───────────────────────────── botões ─────────────────────────────

/// Botão padrão com id dinâmico: clique = alvo de salto.
pub fn hbtn(id: impl Into<SharedString>, th: &Theme, act: Act) -> gpui_kit::Stateful<gpui_kit::Div> {
    let hover = th.paper_hover;
    hbtn_bare(id, th, act).hover(move |s| s.bg(hover))
}

/// [`hbtn`] sem estilo de hover (o chamador define o seu; `.hover` duas vezes
/// entra em pânico no gpui em debug).
pub fn hbtn_bare(id: impl Into<SharedString>, th: &Theme, act: Act) -> gpui_kit::Stateful<gpui_kit::Div> {
    let id: SharedString = id.into();
    let a = act.clone();
    keys::hintable(ui::btn_bare(id.clone(), th).on_click(move |_, w, cx| a(w, cx)), id, act)
}

/// Botão compacto (26 px de altura) para cabeçalhos de cartão.
pub fn sbtn(id: impl Into<SharedString>, th: &Theme, act: Act) -> gpui_kit::Stateful<gpui_kit::Div> {
    hbtn(id, th, act).h(px(26.0)).px(px(8.0)).text_size(px(12.0)).rounded(px(6.0)).bg(th.paper_raise).text_color(th.ink_soft)
}

/// Botão quadrado M/S (mudo/solo): aceso com `color` quando `on`.
pub fn mini(id: impl Into<SharedString>, label: &'static str, th: &Theme, on: bool, color: Hsla, act: Act) -> gpui_kit::Stateful<gpui_kit::Div> {
    let b = hbtn_bare(id, th, act).w(px(27.0)).h(px(27.0)).px(px(0.0)).rounded(px(5.0)).text_size(px(11.0)).font_weight(FontWeight::BOLD).child(label);
    let hover = th.paper_hover;
    if on { b.bg(color).border_color(color).text_color(th.on_accent).hover(move |s| s.bg(color)) } else { b.bg(th.paper_raise).text_color(th.ink_soft).hover(move |s| s.bg(hover)) }
}

/// Chip de escolha (grupo "um de N"): aceso quando `on`.
pub fn chip(id: impl Into<SharedString>, label: impl Into<SharedString>, th: &Theme, on: bool, act: Act) -> gpui_kit::Stateful<gpui_kit::Div> {
    let b = hbtn_bare(id, th, act).h(px(24.0)).px(px(8.0)).rounded(px(12.0)).text_size(px(11.5)).child(label.into());
    let (acc, wash) = (th.accent2, th.accent2_wash);
    let hover = th.paper_hover;
    if on { b.bg(wash).border_color(acc).text_color(acc).font_weight(FontWeight::BOLD).hover(move |s| s.bg(wash)) } else { b.bg(th.paper_raise).text_color(th.ink_soft).hover(move |s| s.bg(hover)) }
}

/// Título de página (mesmo estilo das Configurações).
pub fn page_title(text: &'static str) -> gpui_kit::Div {
    div().text_size(px(22.0)).font_family("Shippori Mincho").font_weight(FontWeight::BOLD).child(text)
}

// ───────────────────────────── teclado ─────────────────────────────

fn page_prefix(page: Page) -> Option<&'static str> {
    match page {
        Page::Mixer => Some("mx-"),
        Page::Pessoas => Some("pp-"),
        Page::Kit => Some("kit-"),
        _ => None,
    }
}

/// Setas ajustam o slider em foco (captura: antes dos handlers da grade), só
/// nas páginas Mixer/Pessoas/Kit e só se o slider em foco for da página atual.
/// Esc tira o foco (sem consumir o Esc).
pub fn bind(d: gpui_kit::Stateful<gpui_kit::Div>, cx: &mut Context<Shell>) -> gpui_kit::Stateful<gpui_kit::Div> {
    macro_rules! arrow {
        ($d:expr, $a:ty, $n:expr) => {
            $d.capture_action(cx.listener(|shell: &mut Shell, _: &$a, w, cx| {
                if let Some(p) = page_prefix(shell.page) {
                    if nudge(p, $n, w, cx) {
                        cx.stop_propagation();
                    }
                }
            }))
        };
    }
    let d = arrow!(d, keys::grid::Left, -1);
    let d = arrow!(d, keys::grid::Down, -1);
    let d = arrow!(d, keys::grid::Right, 1);
    let d = arrow!(d, keys::grid::Up, 1);
    d.capture_action(cx.listener(|_: &mut Shell, _: &keys::keys::Cancel, w, _| {
        if clear_focus() {
            w.refresh();
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_slider_roundtrip() {
        // fader do mixer: −40..+6 dB, passo 1
        for db in -40..=6 {
            let t = frac(db as f64, -40.0, 6.0);
            assert_eq!(value_at(t, -40.0, 6.0, 1.0), db as f64, "{db} dB");
        }
        assert_eq!(frac(-40.0, -40.0, 6.0), 0.0);
        assert_eq!(frac(6.0, -40.0, 6.0), 1.0);
        assert!((frac(-17.0, -40.0, 6.0) - 0.5).abs() < 1e-12);
        // fora da faixa prende
        assert_eq!(value_at(-0.3, -40.0, 6.0, 1.0), -40.0);
        assert_eq!(value_at(1.7, -40.0, 6.0, 1.0), 6.0);
        assert_eq!(frac(99.0, -40.0, 6.0), 1.0);
        // posição intermediária arredonda para o dB mais próximo
        assert_eq!(value_at(0.5 + 0.004, -40.0, 6.0, 1.0), -17.0);
    }

    #[test]
    fn quantize_cleans_float_noise() {
        // pan: −1..1, passo 0.1 — sem 0.30000000000000004 nem −0
        for i in 0..=20 {
            let want = (i as f64 - 10.0) / 10.0;
            let got = value_at(i as f64 / 20.0, -1.0, 1.0, 0.1);
            assert_eq!(got.to_string(), (want + 0.0).to_string());
        }
        assert_eq!(quantize(-0.04, -1.0, 1.0, 0.1).to_string(), "0");
        assert_eq!(quantize(0.26, 0.0, 1.0, 0.05), 0.25);
        assert_eq!(quantize(1234.0, 100.0, 8000.0, 50.0), 1250.0);
        assert_eq!(quantize(0.14, 0.1, 12.0, 0.1), 0.1);
        assert_eq!(quantize(5.0, 0.0, 1.0, 0.0), 1.0);
    }

    #[test]
    fn tags_differ_per_control_and_item() {
        assert_eq!(tag("vol", "a"), tag("vol", "a"));
        assert_ne!(tag("vol", "a"), tag("pan", "a"));
        assert_ne!(tag("vol", "a"), tag("vol", "b"));
    }
}
