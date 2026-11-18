//! Modo projeção (F7d de `docs/planejamento/daw.md`, o *Director* do
//! Hydrogen): para projetar na TV do ensaio. Tela cheia, fundo escuro e letras
//! grandes: a seção atual (e a volta, `2/3`), o compasso dentro dela, os tempos
//! do compasso acendendo, o BPM que está soando e a próxima seção.
//!
//! O que mostrar sai de [`director_info`], função pura sobre o mapa de tempo do
//! schedule (o mesmo do áudio, com repetições e região). Tocando, segue o
//! playhead audível; parado, o cursor da grade.

use gpui_kit::{FontWeight, Hsla, SharedString, div, prelude::*, px, rgb};
use wadawko_audio::{PPQ, TimeMap};
use wadawko_core::Song;

use crate::shell::Shell;
use crate::song_ext::SongExt;
use crate::theme::track_color;

/// O que o modo projeção mostra.
#[derive(Clone, Debug, PartialEq)]
pub struct DirectorInfo {
    pub section: String,
    /// volta atual da seção (1-based) e quantas ela tem
    pub pass: u32,
    pub passes: u32,
    /// compasso dentro da seção (1-based) e quantos ela tem
    pub bar: usize,
    pub bars: usize,
    /// tempo atual (0-based) e tempos do compasso
    pub beat: usize,
    pub beats: usize,
    /// próxima seção na ordem tocada (`None` = acaba aqui)
    pub next: Option<String>,
}

/// Info do instante `tick` (ticks do schedule). `None` fora da execução.
pub fn director_info(song: &Song, time: &TimeMap, tick: f64) -> Option<DirectorInfo> {
    let u = time.bar_at(tick)?;
    let mi = *time.order.get(u)?;
    let secs = song.secs();
    let s = song.section_of(mi);
    let sec = secs.get(s)?;
    let in_sec = |m: usize| m >= sec.start && m < sec.end;
    // volta: quantos começos da seção houve na sequência contínua dela até aqui
    let mut k = u;
    while k > 0 && in_sec(time.order[k - 1]) {
        k -= 1;
    }
    let pass = time.order[k..=u].iter().filter(|&&m| m == sec.start).count().max(1) as u32;
    let beats = time.shape.get(mi).map_or(4, |x| x.0).max(1);
    let beat = (((tick - time.bar_tick(u)) / PPQ).floor().max(0.0) as usize).min(beats - 1);
    let next = time.order[u + 1..].iter().map(|&m| song.section_of(m)).find(|&n| n != s).and_then(|n| secs.get(n)).map(|x| x.name.clone());
    Some(DirectorInfo { section: sec.name.clone(), pass, passes: sec.repeat.max(1), bar: mi - sec.start + 1, bars: sec.end - sec.start, beat, beats, next })
}

impl Shell {
    /// Overlay de tela cheia do modo projeção (Esc ou o mesmo atalho sai).
    pub(crate) fn director(&self, cx: &mut gpui_kit::Context<Self>) -> gpui_kit::AnyElement {
        let st = self.state.read(cx);
        let song = st.song().clone();
        let playing = st.playing();
        let time = &st.schedule.time;
        // tocando: o tick audível; parado: o início da célula do cursor
        let tick = if playing {
            st.playhead().map(|p| p.tick)
        } else {
            let g = self.grid.read(cx);
            g.cursor.filter(|c| c.m < song.measures.len()).map(|c| time.cell_tick(song.flat(c.m, c.k)))
        };
        let info = tick.and_then(|t| director_info(&song, time, t));
        let bpm = if playing { st.sounding_bpm as f64 } else { song.tempo };
        // projetor: contraste máximo, independente do tema
        let (bg, ink, soft, faint): (Hsla, Hsla, Hsla, Hsla) = (rgb(0x0b0b0b).into(), rgb(0xf4f4f4).into(), rgb(0xb8b8b8).into(), rgb(0x5a5a5a).into());
        let accent = st.theme.accent;
        let title: SharedString = wadawko_store::display_title(&song).into();
        let mut root = div().absolute().inset_0().bg(bg).text_color(ink).flex().flex_col().p(px(40.0)).gap(px(18.0)).child(
            div().flex().justify_between().text_size(px(18.0)).text_color(faint).child(title).child(if playing {
                "▶ tocando · Espaço para · Esc sai"
            } else {
                "■ parado · Espaço toca · Esc sai"
            }),
        );
        match info {
            Some(i) => {
                let mut dots = div().flex().gap(px(24.0));
                for b in 0..i.beats {
                    let on = playing && b == i.beat;
                    let c = if on { if b == 0 { accent } else { ink } } else { faint };
                    dots = dots.child(div().size(px(if b == 0 { 64.0 } else { 52.0 })).rounded_full().bg(c));
                }
                let progress = ((i.bar - 1) as f32 + if playing { i.beat as f32 / i.beats as f32 } else { 0.0 }) / i.bars.max(1) as f32;
                root = root
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap(px(28.0))
                            .child(div().text_size(px(140.0)).font_weight(FontWeight::BOLD).line_height(px(150.0)).child(SharedString::from(i.section.clone())))
                            .when(i.passes > 1, |d| {
                                d.child(div().pb(px(22.0)).text_size(px(64.0)).text_color(accent).font_weight(FontWeight::BOLD).child(SharedString::from(format!("{}/{}", i.pass, i.passes))))
                            }),
                    )
                    .child(div().text_size(px(48.0)).text_color(soft).child(SharedString::from(format!("compasso {} de {}", i.bar, i.bars))))
                    .child(div().h(px(10.0)).w_full().bg(faint).child(div().h_full().w(gpui_kit::relative(progress.clamp(0.0, 1.0))).bg(accent)))
                    .child(div().flex_1().flex().items_center().child(dots))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_end()
                            .child(div().text_size(px(56.0)).text_color(soft).child(SharedString::from(match &i.next {
                                Some(n) => format!("próxima: {n}"),
                                None => "última seção".to_string(),
                            })))
                            .child(div().text_size(px(96.0)).font_weight(FontWeight::BOLD).child(SharedString::from(format!("♩ {bpm:.0}")))),
                    );
            }
            None => {
                root = root.child(div().flex_1().flex().items_center().justify_center().text_size(px(48.0)).text_color(soft).child("ponha o cursor na grade ou toque (Espaço)"));
            }
        }
        // linhas da música, como lembrete de quem toca (listras de cor)
        let mut legend = div().flex().flex_wrap().gap(px(18.0)).text_size(px(22.0)).text_color(soft);
        for t in &song.tracks {
            legend = legend.child(div().flex().items_center().gap(px(8.0)).child(div().w(px(18.0)).h(px(18.0)).bg(track_color(t))).child(SharedString::from(t.name.clone())));
        }
        root.child(legend).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wadawko_core::demo::demo_song;
    use wadawko_core::model::{Extra, Section};

    #[test]
    fn info_com_repeticao_e_proxima() {
        let mut song = demo_song();
        // A = compassos 0–1 (2×), B = 2–3
        song.sections = Some(vec![
            Section { id: "a".into(), name: "A".into(), start: 0, repeat: Some(2), extra: Extra::new() },
            Section { id: "b".into(), name: "B".into(), start: 2, repeat: None, extra: Extra::new() },
        ]);
        let order = wadawko_core::timeline::play_order(&song, None);
        assert_eq!(order, vec![0, 1, 0, 1, 2, 3]);
        let time = TimeMap::new(&song, order);
        let bar = 4.0 * PPQ;
        // 2ª volta de A, compasso 2, tempo 3
        let i = director_info(&song, &time, 3.0 * bar + 2.5 * PPQ).unwrap();
        assert_eq!((i.section.as_str(), i.pass, i.passes, i.bar, i.bars, i.beat, i.beats), ("A", 2, 2, 2, 2, 2, 4));
        assert_eq!(i.next.as_deref(), Some("B"));
        // 1ª volta
        assert_eq!(director_info(&song, &time, 0.5 * PPQ).unwrap().pass, 1);
        // B é a última
        let b = director_info(&song, &time, 5.0 * bar).unwrap();
        assert_eq!((b.section.as_str(), b.bar, b.next), ("B", 2, None));
        assert!(director_info(&song, &time, 6.0 * bar).is_none());
    }
}
