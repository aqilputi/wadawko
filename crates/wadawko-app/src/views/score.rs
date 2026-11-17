//! Partitura (aba do Editor): mostra o `wadawko-notation` pintado pelo backend GPUI
//! do `Canvas2D` (`score/canvas.rs` + `score/surface.rs`). Paridade com
//! `src/lib/Score.svelte`: legenda, rolagem horizontal, nota acesa a cada golpe do
//! playback e auto-scroll (playhead a 40 % da área visível).
//!
//! - engrava só quando música/zoom/tema mudam (cache do `Score` por `revision`);
//! - pinta com culling por compasso visível; a coluna de nomes fica fixa;
//! - roda = rolagem horizontal; Ctrl+roda = zoom ancorado no ponteiro; arrasto = rolagem;
//! - clique num compasso = início do playback (como a régua da grade);
//! - comandos `score::ExportSvg` / `ExportPng` / `Print` (paleta) — ver `score/export.rs`.

mod canvas;
mod export;
mod surface;

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use gpui_kit::{Bounds, ContentMask, Context, Entity, FontWeight, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Render, ScrollEvent, SharedString, Window, canvas, div, fill, point, prelude::*, px, size};
use wadawko_core::Song;
use wadawko_draw::smufl::{FAMILY_BRAVURA, FAMILY_SHIPPORI_MINCHO, FAMILY_ZEN_KAKU};
use wadawko_draw::{Canvas2D, FontBook, SmuflFont, StandardFonts, Transform, pt, rect};
use wadawko_notation::{EngraveOptions, HitKey, Lit, Score, engrave};

use crate::keys;
use crate::shell::Shell;
use crate::shell::io::{default_dir, with_ext};
use crate::state::{AppState, HitFlash, StateEvent};
use crate::theme::Theme;
use crate::ui;

use canvas::GpuiCanvas;
pub use export::ExportKind;
use surface::{GlyphMode, PaintCache, WindowSurface};

const BRAVURA_OTF: &[u8] = include_bytes!("../../../../assets/fonts/Bravura.otf");
const BRAVURA_META: &[u8] = include_bytes!("../../../../assets/fonts/bravura_metadata.json");

/// As fontes da partitura, embutidas no binário (medição no engraving, contornos
/// de glifo e exports). São as mesmas que o `main` registra no text system.
pub fn fonts() -> &'static StandardFonts {
    static FONTS: OnceLock<StandardFonts> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut book = FontBook::new();
        let mut add = |bytes: &'static [u8]| book.add(bytes).expect("fonte embutida válida");
        let zen_regular = add(include_bytes!("../../../../assets/fonts/ZenKakuGothicNew-Regular.ttf"));
        let zen_medium = add(include_bytes!("../../../../assets/fonts/ZenKakuGothicNew-Medium.ttf"));
        let zen_bold = add(include_bytes!("../../../../assets/fonts/ZenKakuGothicNew-Bold.ttf"));
        let mincho_bold = add(include_bytes!("../../../../assets/fonts/ShipporiMincho-Bold.ttf"));
        let bravura = SmuflFont::from_bytes(&mut book, BRAVURA_OTF, BRAVURA_META).expect("Bravura embutida válida");
        for id in [zen_regular, zen_medium, zen_bold] {
            book.set_family(id, FAMILY_ZEN_KAKU);
        }
        book.set_family(mincho_bold, FAMILY_SHIPPORI_MINCHO);
        book.set_family(bravura.font, FAMILY_BRAVURA);
        StandardFonts { book, zen_regular, zen_medium, zen_bold, mincho_bold, bravura }
    })
}

// ───────────────────────────── contas puras ─────────────────────────────

pub const ZOOM_MIN: f32 = 0.4;
pub const ZOOM_MAX: f32 = 3.0;
/// respiro acima/abaixo da partitura dentro do cartão
const PAD_Y: f32 = 10.0;
/// arrasto menor que isto (px) ainda é clique
const CLICK_SLOP: f32 = 4.0;

/// Cores da partitura a partir dos tokens do app (`theme.rs`).
pub fn score_theme(th: &Theme) -> wadawko_notation::Theme {
    let base = if th.dark { wadawko_notation::Theme::dark() } else { wadawko_notation::Theme::light() };
    wadawko_notation::Theme {
        ink: surface::color(th.ink),
        ink_soft: surface::color(th.ink_soft),
        staff: surface::color(th.ink),
        // kiai: o vermelho fixo do app web (#d6452b), clareado no tema escuro
        kyai: base.kyai,
        accent: surface::color(th.accent),
        paper: Some(surface::color(th.paper)),
    }
}

/// Golpes que estão soando agora → chaves da partitura (com a cor do destaque).
/// `at` já inclui a latência; o brilho dura `flash_ms` (≈ 1 colcheia, como no web).
pub fn lit_keys(hits: &[HitFlash], track_ids: &[String], now: Instant, flash_ms: f32, color: wadawko_draw::Color) -> Vec<Lit> {
    let dur = Duration::from_secs_f32(flash_ms.max(0.0) / 1000.0);
    hits.iter()
        .filter(|h| now >= h.at && now < h.at + dur)
        .filter_map(|h| Some((HitKey::new(track_ids.get(h.track)?.clone(), h.cell, h.hit), color)))
        .collect()
}

/// Rolagem que deixa o playhead (`x`, coordenadas da página) a 40 % da área de
/// pauta visível — a regra do `Score.svelte`, descontando a coluna fixa de nomes.
pub fn follow_scroll(x: f32, staff_x: f32, view_w: f32, content_w: f32) -> f32 {
    let visible = (view_w - staff_x).max(0.0);
    (x - staff_x - visible * 0.4).clamp(0.0, max_scroll(view_w, content_w))
}

pub fn max_scroll(view_w: f32, content_w: f32) -> f32 {
    (content_w - view_w).max(0.0)
}

/// Zoom ancorado: o ponto da partitura sob o ponteiro (`anchor` px da borda
/// esquerda da view) continua sob o ponteiro. Devolve (zoom, rolagem) novos.
pub fn zoom_about(zoom: f32, scroll: f32, anchor: f32, new_zoom: f32) -> (f32, f32) {
    let nz = new_zoom.clamp(ZOOM_MIN, ZOOM_MAX);
    if zoom.is_nan() || zoom <= 0.0 {
        return (nz, scroll);
    }
    (nz, ((scroll + anchor) * nz / zoom - anchor).max(0.0))
}

/// Célula-fonte global do clique em `x` (coordenadas da página): a célula cuja
/// posição (onde fica a nota e por onde passa o playhead) está mais perto.
pub fn cell_at_x(score: &Score, measure: usize, x: f32) -> Option<usize> {
    let m = score.measures.get(measure)?;
    if m.cells == 0 || m.cell_width <= 0.0 {
        return Some(m.cell_base);
    }
    let k = ((x - m.notes_x) / m.cell_width).round().clamp(0.0, (m.cells - 1) as f32) as usize;
    Some(m.cell_base + k)
}

// ───────────────────────────── a view ─────────────────────────────

#[derive(Clone, Copy, PartialEq)]
struct EngraveKey {
    revision: u64,
    /// endereço do `Arc<Song>`: cobre a troca de música mesmo se `revision` coincidir
    song: usize,
    zoom_bits: u32,
    dark: bool,
}

#[derive(Clone, Copy)]
struct Drag {
    x0: f32,
    scroll0: f32,
    moved: bool,
}

pub struct ScoreView {
    pub state: Entity<AppState>,
    cache: Option<(EngraveKey, Rc<Score>)>,
    pub zoom: f32,
    pub scroll_x: f32,
    drag: Option<Drag>,
    /// retângulo do canvas no último frame (para rolagem/zoom/clique)
    geo: Rc<Cell<Option<Bounds<Pixels>>>>,
    paint_cache: Rc<RefCell<PaintCache>>,
    glyphs: GlyphMode,
    exporting: bool,
    /// engravings feitos (diagnóstico: não pode crescer durante o playback)
    pub engraves: u64,
}

impl ScoreView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        // re-renderiza quando a música/tema/transporte mudam
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&state, |this: &mut Self, _, ev: &StateEvent, cx| {
            if *ev == StateEvent::SongReplaced {
                this.scroll_x = 0.0;
                this.drag = None;
                cx.notify();
            }
        })
        .detach();
        // a Bravura só é usada aqui: registra no text system (modo de glifos por texto)
        if let Err(e) = cx.text_system().add_fonts(vec![Cow::Borrowed(BRAVURA_OTF)]) {
            eprintln!("partitura: não registrei a Bravura no text system: {e:#}");
        }
        ScoreView { state, cache: None, zoom: 1.0, scroll_x: 0.0, drag: None, geo: Rc::new(Cell::new(None)), paint_cache: Rc::default(), glyphs: GlyphMode::from_env(), exporting: false, engraves: 0 }
    }

    /// O `Score` da música atual, re-engravado só se música/zoom/tema mudaram.
    fn score(&mut self, song: &Arc<Song>, revision: u64, th: &Theme) -> Rc<Score> {
        let key = EngraveKey { revision, song: Arc::as_ptr(song) as usize, zoom_bits: self.zoom.to_bits(), dark: th.dark };
        if let Some((k, s)) = &self.cache
            && *k == key
        {
            return s.clone();
        }
        let opts = EngraveOptions::screen(score_theme(th)).with_zoom(self.zoom);
        let score = Rc::new(engrave(song, &opts, fonts()));
        self.engraves += 1;
        self.paint_cache.borrow_mut().trim();
        self.cache = Some((key, score.clone()));
        score
    }

    fn view_w(&self) -> f32 {
        self.geo.get().map_or(0.0, |b| f32::from(b.size.width))
    }

    fn content_w(&self) -> f32 {
        self.cache.as_ref().map_or(0.0, |(_, s)| s.size().0)
    }

    fn set_scroll(&mut self, x: f32) -> bool {
        let x = x.clamp(0.0, max_scroll(self.view_w(), self.content_w()));
        let changed = (x - self.scroll_x).abs() > 0.01;
        self.scroll_x = x;
        changed
    }

    /// Muda o zoom mantendo fixo o ponto a `anchor` px da borda esquerda
    /// (`None` = 40 % da largura, onde fica o playhead).
    pub fn set_zoom(&mut self, zoom: f32, anchor: Option<f32>, cx: &mut Context<Self>) {
        let anchor = anchor.unwrap_or(self.view_w() * 0.4);
        let (z, s) = zoom_about(self.zoom, self.scroll_x, anchor, zoom);
        if z != self.zoom {
            self.zoom = z;
            // o limite direito só é conhecido depois de re-engravar (o render ajusta)
            self.scroll_x = s;
            cx.notify();
        }
    }

    fn on_wheel(&mut self, ev: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        let d = ev.delta.pixel_delta(px(40.0));
        let (dx, dy) = (f32::from(d.x), f32::from(d.y));
        if ev.modifiers.control || ev.modifiers.platform {
            let anchor = self.geo.get().map(|b| f32::from(ev.position.x - b.origin.x));
            let step = if dy.abs() >= dx.abs() { dy } else { dx };
            self.set_zoom(self.zoom * (step / 400.0).exp(), anchor, cx);
            cx.stop_propagation();
            return;
        }
        // a roda (qualquer eixo) rola a partitura na horizontal; no fim do curso o
        // evento segue para a rolagem vertical do editor
        let delta = if dx.abs() > dy.abs() { dx } else { dy };
        if self.set_scroll(self.scroll_x - delta) {
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn on_down(&mut self, ev: &MouseDownEvent, _: &mut Window, _: &mut Context<Self>) {
        self.drag = Some(Drag { x0: f32::from(ev.position.x), scroll0: self.scroll_x, moved: false });
    }

    fn on_move(&mut self, ev: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(mut drag) = self.drag else { return };
        if ev.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            return;
        }
        let dx = f32::from(ev.position.x) - drag.x0;
        if dx.abs() > CLICK_SLOP {
            drag.moved = true;
        }
        self.drag = Some(drag);
        if drag.moved && self.set_scroll(drag.scroll0 - dx) {
            cx.notify();
        }
    }

    fn on_up(&mut self, ev: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else { return };
        if drag.moved {
            return;
        }
        // clique: o início do playback vai para a célula sob o ponteiro
        let (Some(b), Some((_, score))) = (self.geo.get(), self.cache.clone()) else { return };
        let Some(sys) = score.systems.first() else { return };
        let (vx, vy) = (f32::from(ev.position.x - b.origin.x), f32::from(ev.position.y - b.origin.y) - PAD_Y);
        if vx < sys.staff_x {
            return;
        }
        let p = pt(vx + self.scroll_x, vy.clamp(sys.bounds.top() + 0.5, sys.bounds.bottom() - 0.5));
        let Some(cell) = score.measure_at(0, p).and_then(|mi| cell_at_x(&score, mi, p.x)) else { return };
        self.state.update(cx, |st, cx| {
            st.start_cell = cell;
            st.seek_cell(cell);
            cx.notify();
        });
    }

    /// Exporta a partitura da música atual: pede o caminho (mesmo diálogo do
    /// export de JSON), gera fora da thread da UI e avisa na barra de status.
    pub fn export(&mut self, kind: ExportKind, cx: &mut Context<Self>) {
        if self.exporting {
            self.state.update(cx, |s, cx| {
                s.flash("já tem um export de partitura em andamento");
                cx.notify();
            });
            return;
        }
        let song = self.state.read(cx).song().clone();
        let dir = default_dir();
        let name = format!("{}-partitura.{}", song.slug(), kind.ext());
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        let state = self.state.clone();
        cx.spawn(async move |this, cx| {
            // sem portal de arquivos: salva direto na pasta sugerida
            let path = match rx.await {
                Ok(Ok(Some(p))) => with_ext(p, kind.ext()),
                Ok(Ok(None)) => return,
                _ => dir.join(&name),
            };
            let _ = this.update(cx, |this, _| this.exporting = true);
            let target = path.clone();
            let res = cx.background_executor().spawn(async move { export::write(&song, kind, fonts(), &target) }).await;
            let _ = this.update(cx, |this, _| this.exporting = false);
            let msg = match &res {
                Ok(files) if files.len() > 1 => format!("partitura exportada: {} páginas em {}", files.len(), path.parent().map(|p| p.display().to_string()).unwrap_or_default()),
                Ok(_) if kind == ExportKind::Print => format!("partitura para impressão: {} — abra no navegador e imprima (A4 paisagem)", path.display()),
                Ok(_) => format!("partitura exportada: {}", path.display()),
                Err(e) => format!("erro ao exportar a partitura: {e:#}"),
            };
            state.update(cx, |s, cx| {
                s.flash(msg);
                cx.notify();
            });
            if res.is_ok() && kind == ExportKind::Print {
                cx.update(|cx| cx.open_url(&format!("file://{}", path.display())));
            }
        })
        .detach();
    }
}

/// Tudo que o `canvas` precisa para pintar um frame.
struct PaintArgs {
    score: Rc<Score>,
    th: Theme,
    scroll: f32,
    lit: Vec<Lit>,
    /// x do playhead (coordenadas da página) e se está tocando (linha cheia) ou
    /// só marcando o início (triângulo)
    playhead: Option<(f32, f32, f32, bool)>,
    cache: Rc<RefCell<PaintCache>>,
    glyphs: GlyphMode,
}

fn paint_score(b: Bounds<Pixels>, a: &PaintArgs, window: &mut Window, cx: &mut gpui_kit::App) {
    let visible = window.content_mask().bounds.intersect(&b);
    if f32::from(visible.size.width) <= 0.0 || f32::from(visible.size.height) <= 0.0 {
        return;
    }
    let (ox, oy) = (f32::from(b.origin.x), f32::from(b.origin.y) + PAD_Y);
    let view_w = f32::from(b.size.width);
    let score = &*a.score;
    let (_, h) = score.size();
    let staff_x = score.systems.first().map_or(0.0, |s| s.staff_x);
    let book = &fonts().book;
    window.with_content_mask(Some(ContentMask { bounds: b }), |window| {
        {
            let mut cache = a.cache.borrow_mut();
            let mut surface = WindowSurface { window, cx, cache: &mut cache, fonts: book, glyphs: a.glyphs };
            let mut c = GpuiCanvas::new(&mut surface, book, Transform::translate(ox, oy));
            // compassos: rolam por baixo da coluna de nomes (recorte à direita dela)
            c.save();
            c.clip_rect(rect(staff_x - 1.5, -PAD_Y, (view_w - staff_x + 1.5).max(0.0), h + 2.0 * PAD_Y));
            c.translate(-a.scroll, 0.0);
            score.paint_lit(&mut c, 0, Some(rect(a.scroll + staff_x, 0.0, (view_w - staff_x).max(0.0), h)), &a.lit);
            c.restore();
            // coluna de nomes/trilhos fixa
            for si in 0..score.systems.len() {
                score.paint_system_labels(&mut c, si, 0.0);
            }
        }
        // playhead (ou marca de início do playback)
        if let Some((x, top, bottom, playing)) = a.playhead {
            let sx = ox + x - a.scroll;
            if sx >= ox + staff_x - 1.5 && sx <= ox + view_w {
                let col = a.th.accent;
                if playing {
                    window.paint_quad(fill(Bounds::new(point(px(sx - 1.0), px(oy + top)), size(px(2.0), px(bottom - top))), col));
                }
                let ty = oy + top - 7.0;
                let mut tri = gpui_kit::PathBuilder::fill();
                tri.move_to(point(px(sx - 5.0), px(ty)));
                tri.line_to(point(px(sx + 5.0), px(ty)));
                tri.line_to(point(px(sx), px(ty + 7.0)));
                tri.close();
                if let Ok(p) = tri.build() {
                    window.paint_path(p, col);
                }
            }
        }
    });
}

impl Render for ScoreView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let st = self.state.read(cx);
        let th = st.theme;
        let song = st.song().clone();
        let (revision, playing, ph, start_cell, flash_ms) = (st.revision, st.playing(), st.playhead(), st.start_cell, st.flash_ms());
        let hits = st.hits.clone();

        let score = self.score(&song, revision, &th);
        let (content_w, h) = score.size();
        let view_w = self.view_w();
        let staff_x = score.systems.first().map_or(0.0, |s| s.staff_x);

        // playhead: tocando = posição audível; parado = início do playback
        let playhead = match ph.filter(|_| playing) {
            Some(p) => score.x_at(p.mi, p.frac).map(|pos| (pos.x, pos.top, pos.bottom, true)),
            None => score.x_at_cell(start_cell as f32).map(|pos| (pos.x, pos.top, pos.bottom, false)),
        };
        // auto-scroll: playhead a 40 % da área visível (regra do Score.svelte)
        if let Some((x, _, _, true)) = playhead
            && view_w > 0.0
            && self.drag.is_none_or(|d| !d.moved)
        {
            self.scroll_x = follow_scroll(x, staff_x, view_w, content_w);
        }
        if view_w > 0.0 {
            self.scroll_x = self.scroll_x.clamp(0.0, max_scroll(view_w, content_w));
        }
        // enquanto toca ou há nota acesa, pinta a cada frame (o Shell drena os eventos)
        if playing || !hits.is_empty() {
            window.request_animation_frame();
        }
        let lit = lit_keys(&hits, &score.track_ids, Instant::now(), flash_ms, score.theme.accent);

        let args = PaintArgs { score, th, scroll: self.scroll_x, lit, playhead, cache: self.paint_cache.clone(), glyphs: self.glyphs };
        let geo = self.geo.clone();
        let had_geo = geo.get().is_some();
        if !had_geo {
            // 1º frame: ainda sem largura conhecida → repinta com o auto-scroll/limites certos
            cx.notify();
        }
        let staves = canvas(move |b, _, _| geo.set(Some(b)), move |b, _, window, cx| paint_score(b, &args, window, cx)).size_full();

        let legend = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(4.0))
            .text_size(px(11.5))
            .text_color(th.ink_soft)
            .child("acima da linha = mão direita · abaixo = mão esquerda ·")
            .child(div().font_weight(FontWeight::BOLD).child("×"))
            .child("= borda (fuchi) ·")
            .child(div().font_weight(FontWeight::BOLD).child(">"))
            .child("= acento ·")
            .child(div().text_color(surface::hsla(score_theme(&th).kyai)).child("vermelho em itálico"))
            .child("= kiai (grito)");

        let zoom_btn = |id: &'static str, label: &'static str, f: fn(f32) -> f32, cx: &mut Context<Self>| {
            ui::btn(id, &th).h(px(26.0)).px(px(9.0)).child(label).on_click(cx.listener(move |this, _, _, cx| {
                let z = f(this.zoom);
                this.set_zoom(z, None, cx);
            }))
        };
        let tools = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.0))
            .child(zoom_btn("score-zoom-out", "−", |z| ((z - 0.1) * 10.0).round() / 10.0, cx))
            .child(div().w(px(42.0)).text_size(px(11.5)).text_color(th.ink_soft).flex().justify_center().child(SharedString::from(format!("{:.0}%", self.zoom * 100.0))))
            .child(zoom_btn("score-zoom-in", "+", |z| ((z + 0.1) * 10.0).round() / 10.0, cx))
            .child(zoom_btn("score-zoom-reset", "100%", |_| 1.0, cx))
            .child(div().w(px(8.0)))
            .child(ui::abtn("score-svg", &th, keys::score::ExportSvg).h(px(26.0)).child("SVG"))
            .child(ui::abtn("score-png", &th, keys::score::ExportPng).h(px(26.0)).child("PNG"))
            .child(ui::abtn("score-print", &th, keys::score::Print).h(px(26.0)).child("imprimir"));

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(div().flex().items_center().justify_between().gap(px(12.0)).child(legend).child(tools))
            .child(
                div()
                    .id("score-view")
                    .relative()
                    .w_full()
                    .h(px(h + 2.0 * PAD_Y))
                    .rounded(px(10.0))
                    .border_1()
                    .border_color(th.line)
                    .bg(th.paper)
                    .overflow_hidden()
                    .cursor_grab()
                    .on_scroll(cx.listener(Self::on_wheel))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
                    .on_mouse_move(cx.listener(Self::on_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, _| this.drag = None))
                    .child(staves),
            )
    }
}

/// Handlers das ações desta view (rodam com o foco em qualquer lugar).
pub fn bind(d: gpui_kit::Stateful<gpui_kit::Div>, cx: &mut Context<Shell>) -> gpui_kit::Stateful<gpui_kit::Div> {
    d.on_action(cx.listener(|shell: &mut Shell, _: &keys::score::ExportSvg, _, cx| shell.views.score.update(cx, |v, cx| v.export(ExportKind::Svg, cx))))
        .on_action(cx.listener(|shell: &mut Shell, _: &keys::score::ExportPng, _, cx| shell.views.score.update(cx, |v, cx| v.export(ExportKind::Png, cx))))
        .on_action(cx.listener(|shell: &mut Shell, _: &keys::score::Print, _, cx| shell.views.score.update(cx, |v, cx| v.export(ExportKind::Print, cx))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_score(zoom: f32) -> Score {
        engrave(&wadawko_core::demo::demo_song(), &EngraveOptions::screen(score_theme(&Theme::light())).with_zoom(zoom), fonts())
    }

    #[test]
    fn embedded_fonts_resolve_the_families_the_engraver_uses() {
        let f = fonts();
        assert_eq!(f.book.len(), 5);
        for (id, family) in [(f.zen_regular, FAMILY_ZEN_KAKU), (f.zen_bold, FAMILY_ZEN_KAKU), (f.mincho_bold, FAMILY_SHIPPORI_MINCHO), (f.bravura.font, FAMILY_BRAVURA)] {
            assert_eq!(f.book.info(id).unwrap().family, family);
        }
        // cabeça de nota preta: tem contorno e âncora de haste nos metadados
        assert!(!f.book.glyph_outline(f.bravura.font, '\u{E0A4}', 40.0).unwrap().is_empty());
        assert!(f.bravura.anchors("noteheadBlack").is_some());
    }

    #[test]
    fn theme_comes_from_app_tokens() {
        let (l, d) = (score_theme(&Theme::light()), score_theme(&Theme::dark()));
        assert_eq!(l.ink.hex_rgb(), "#232733");
        assert_eq!(l.accent.hex_rgb(), "#e26374");
        assert_eq!(l.paper.unwrap().hex_rgb(), "#f9fafc");
        assert_eq!(l.kyai.hex_rgb(), "#d6452b");
        assert_eq!(d.ink.hex_rgb(), "#e8ebf2");
        assert_eq!(d.paper.unwrap().hex_rgb(), "#1d212c");
        assert_ne!(d.kyai, l.kyai);
    }

    #[test]
    fn hits_map_to_score_keys_only_while_they_sound() {
        let score = demo_score(1.0);
        let ids = &score.track_ids;
        let now = Instant::now();
        let ms = Duration::from_millis;
        let col = score.theme.accent;
        // um golpe que existe na partitura: pega a 1ª chave
        let k0 = score.hit_keys().into_iter().next().unwrap();
        let t0 = ids.iter().position(|id| *id == k0.track_id).unwrap();
        let hits = [
            HitFlash { track: t0, cell: k0.global_cell, hit: k0.hit, at: now - ms(50) }, // soando
            HitFlash { track: t0, cell: k0.global_cell + 1, hit: 0, at: now + ms(30) },  // agendado (latência): ainda não
            HitFlash { track: t0, cell: k0.global_cell + 2, hit: 0, at: now - ms(500) }, // já apagou
            HitFlash { track: 99, cell: 0, hit: 0, at: now - ms(10) },                   // linha que não existe
        ];
        let lit = lit_keys(&hits, ids, now, 200.0, col);
        assert_eq!(lit, vec![(k0.clone(), col)]);
        // a chave acende de fato alguma coisa na partitura
        let mut rec = wadawko_draw::Recording::new();
        score.paint_highlights(&mut rec, 0, None, &lit);
        assert!(!rec.is_empty());
        // formato da chave = o do app web (trackId:célulaGlobal:golpe)
        assert_eq!(k0.to_string(), format!("{}:{}:{}", ids[t0], k0.global_cell, k0.hit));
    }

    #[test]
    fn flash_duration_follows_the_tempo_like_the_web_app() {
        // min(400, max(140, 30000/bpm)) — a conta mora em AppState::flash_ms; aqui a janela
        let now = Instant::now();
        let ids = vec!["a".to_string()];
        let h = [HitFlash { track: 0, cell: 0, hit: 0, at: now - Duration::from_millis(250) }];
        let c = wadawko_draw::Color::BLACK;
        assert_eq!(lit_keys(&h, &ids, now, (30_000.0f32 / 100.0).clamp(140.0, 400.0), c).len(), 1); // 100 bpm = 300 ms
        assert_eq!(lit_keys(&h, &ids, now, (30_000.0f32 / 240.0).clamp(140.0, 400.0), c).len(), 0); // 240 bpm = 140 ms
    }

    #[test]
    fn follow_keeps_the_playhead_at_40_percent_of_the_visible_staff() {
        // view de 1000 px com 200 de nomes: 40 % de 800 = 320 px depois da coluna
        assert_eq!(follow_scroll(2000.0, 200.0, 1000.0, 5000.0), 2000.0 - 200.0 - 320.0);
        // começo e fim: limitado ao conteúdo
        assert_eq!(follow_scroll(250.0, 200.0, 1000.0, 5000.0), 0.0);
        assert_eq!(follow_scroll(4990.0, 200.0, 1000.0, 5000.0), 4000.0);
        // conteúdo menor que a view: não rola
        assert_eq!(follow_scroll(300.0, 200.0, 1000.0, 600.0), 0.0);
        assert_eq!(max_scroll(1000.0, 600.0), 0.0);
    }

    #[test]
    fn zoom_is_anchored_at_the_pointer_and_clamped() {
        // ponto da página sob o ponteiro: (scroll + anchor) / zoom — invariante
        let (z, s) = zoom_about(1.0, 300.0, 200.0, 2.0);
        assert_eq!(z, 2.0);
        assert!(((s + 200.0) / z - (300.0 + 200.0) / 1.0).abs() < 1e-3);
        assert_eq!(zoom_about(1.0, 0.0, 0.0, 99.0).0, ZOOM_MAX);
        assert_eq!(zoom_about(1.0, 0.0, 0.0, 0.01).0, ZOOM_MIN);
        // afastar perto do começo não gera rolagem negativa
        assert_eq!(zoom_about(2.0, 10.0, 400.0, 1.0).1, 0.0);
    }

    #[test]
    fn click_maps_to_the_nearest_cell_of_the_measure() {
        let score = demo_score(1.0);
        let m = &score.measures[1];
        assert!(m.cells >= 2);
        assert_eq!(cell_at_x(&score, 1, m.notes_x), Some(m.cell_base));
        assert_eq!(cell_at_x(&score, 1, m.notes_x - 30.0), Some(m.cell_base));
        assert_eq!(cell_at_x(&score, 1, m.notes_x + 1.4 * m.cell_width), Some(m.cell_base + 1));
        assert_eq!(cell_at_x(&score, 1, m.notes_x + 1.6 * m.cell_width), Some(m.cell_base + 2.min(m.cells - 1)));
        assert_eq!(cell_at_x(&score, 1, m.bounds.right() + 500.0), Some(m.cell_base + m.cells - 1));
        assert_eq!(cell_at_x(&score, 9999, 0.0), None);
        // o playhead da célula clicada passa exatamente onde se clicou
        let x = score.x_at_cell((m.cell_base + 1) as f32).unwrap().x;
        assert_eq!(cell_at_x(&score, 1, x), Some(m.cell_base + 1));
    }

    #[test]
    fn zoom_scales_the_engraving() {
        let (a, b) = (demo_score(1.0), demo_score(2.0));
        assert!((b.size().0 / a.size().0 - 2.0).abs() < 0.05);
        assert!((b.staff_space / a.staff_space - 2.0).abs() < 1e-3);
    }
}
