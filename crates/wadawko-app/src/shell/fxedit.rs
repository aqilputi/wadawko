//! Edição dos efeitos por célula (`wadawko_core::fx`; tracker §4.5): ações que
//! agem na seleção da grade (ou na célula do cursor) — rulo, flam, atraso,
//! corte, força, exato, limpar — e o campo de texto da célula.

use gpui_kit::Context;
use wadawko_core::fx::{self, Fx, MAX_FX, Ramp};

use super::Shell;
use crate::song_ext::SongExt;
use crate::views::grid::InputTarget;

/// Próximo passo do rulo: nenhum → 2 → 3 → 4 → 6 → 8 → nenhum.
fn next_roll(cur: Option<u8>) -> Option<u8> {
    match cur {
        None => Some(2),
        Some(n) => [2u8, 3, 4, 6, 8].into_iter().find(|&x| x > n),
    }
}

fn find<T>(v: &[Fx], f: impl Fn(Fx) -> Option<T>) -> Option<T> {
    v.iter().copied().find_map(f)
}

fn without(v: &mut Vec<Fx>, letter: char) {
    v.retain(|f| f.letter() != letter);
}

/// As transformações das ações (puras, testáveis).
pub(crate) fn transform(action: &str, mut v: Vec<Fx>) -> Vec<Fx> {
    match action {
        "roll" => {
            let cur = find(&v, |f| if let Fx::Roll(n, _) = f { Some(n) } else { None });
            let ramp = find(&v, |f| if let Fx::Roll(_, r) = f { Some(r) } else { None }).unwrap_or(Ramp::Flat);
            without(&mut v, 'R');
            if let Some(n) = next_roll(cur) {
                v.push(Fx::Roll(n, ramp));
            }
        }
        "roll_cresc" => {
            let (n, r) = find(&v, |f| if let Fx::Roll(n, r) = f { Some((n, r)) } else { None }).unwrap_or((4, Ramp::Flat));
            without(&mut v, 'R');
            v.push(Fx::Roll(
                n,
                match r {
                    Ramp::Flat => Ramp::Up,
                    Ramp::Up => Ramp::Down,
                    Ramp::Down => Ramp::Flat,
                },
            ));
        }
        "flam" => {
            if find(&v, |f| matches!(f, Fx::Flam(_)).then_some(())).is_some() {
                without(&mut v, 'F');
            } else {
                v.push(Fx::Flam(2));
            }
        }
        "later" | "earlier" => {
            let d = find(&v, |f| if let Fx::Delay(n) = f { Some(n) } else { None }).unwrap_or(0);
            let n = (d + if action == "later" { 1 } else { -1 }).clamp(-5, 5);
            without(&mut v, 'D');
            if n != 0 {
                v.push(Fx::Delay(n));
            }
        }
        "cut" => {
            if find(&v, |f| matches!(f, Fx::Cut(_)).then_some(())).is_some() {
                without(&mut v, 'X');
            } else {
                v.push(Fx::Cut(5));
            }
        }
        "softer" => {
            let cur = find(&v, |f| if let Fx::Vel(n) = f { Some(n) } else { None });
            without(&mut v, 'V');
            match cur {
                None => v.push(Fx::Vel(7)),
                Some(0) => {}
                Some(n) => v.push(Fx::Vel(n - 1)),
            }
        }
        "exact" => {
            if find(&v, |f| matches!(f, Fx::Human(0)).then_some(())).is_some() {
                without(&mut v, 'H');
            } else {
                without(&mut v, 'H');
                v.push(Fx::Human(0));
            }
        }
        "clear" => v.clear(),
        _ => {}
    }
    v
}

impl Shell {
    /// Células alvo: a seleção da grade, senão a célula do cursor — (compasso, linha, célula).
    fn fx_targets(&self, cx: &gpui_kit::App) -> Vec<(usize, usize, usize)> {
        let g = self.grid.read(cx);
        let song = &g.song;
        let Some((t0, t1, g0, g1)) = g.sel_range() else { return Vec::new() };
        let mut out = Vec::new();
        for t in t0..=t1.min(song.tracks.len().saturating_sub(1)) {
            for gl in g0..=g1.min(song.total_cells().saturating_sub(1)) {
                let (m, k) = song.unflat(gl);
                out.push((m, t, k));
            }
        }
        out
    }

    /// Aplica a ação a todas as células alvo que têm nota (efeito sem nota não soa).
    pub(crate) fn fx_action(&mut self, action: &'static str, cx: &mut Context<Self>) {
        let targets = self.fx_targets(cx);
        let song = self.song(cx);
        let mut changes: Vec<(String, usize, usize, Option<String>)> = Vec::new();
        let mut too_many = false;
        for &(m, t, k) in &targets {
            let Some(track) = song.tracks.get(t) else { continue };
            let Some(meas) = song.measures.get(m) else { continue };
            if meas.cell(&track.id, k).is_none_or(str::is_empty) && action != "clear" {
                continue;
            }
            let cur = meas.fx_at(&track.id, k).and_then(|s| fx::parse(s).ok()).unwrap_or_default();
            let next = transform(action, cur);
            if next.len() > MAX_FX {
                too_many = true;
                continue;
            }
            let text = (!next.is_empty()).then(|| fx::format(&next));
            if text.as_deref() != meas.fx_at(&track.id, k) {
                changes.push((track.id.clone(), m, k, text));
            }
        }
        if changes.is_empty() {
            self.flash(
                if too_many {
                    "no máximo 3 efeitos por célula"
                } else if targets.is_empty() {
                    "ponha o cursor numa célula da grade"
                } else {
                    "nenhuma nota na seleção"
                },
                cx,
            );
            return;
        }
        let n = changes.len();
        let last = changes.last().and_then(|c| c.3.clone());
        self.edit(cx, move |s| {
            for (tid, m, k, text) in &changes {
                let _ = wadawko_core::ops::cells::set_fx(s, tid, *m, *k, text.as_deref());
            }
        });
        self.flash(if n == 1 { last.map_or_else(|| "efeitos removidos".into(), |t| format!("efeitos: {t}")) } else { format!("efeitos em {n} células") }, cx);
    }

    /// Abre o campo de texto dos efeitos da célula do cursor.
    pub(crate) fn fx_edit(&mut self, window: &mut gpui_kit::Window, cx: &mut Context<Self>) {
        let g = self.grid.read(cx);
        let Some(c) = g.cursor else {
            self.flash("ponha o cursor numa célula da grade", cx);
            return;
        };
        let cur = g.song.tracks.get(c.t).and_then(|t| g.song.measures.get(c.m).and_then(|m| m.fx_at(&t.id, c.k))).unwrap_or("").to_string();
        self.ask(InputTarget::CellFx(c.m, c.t, c.k), cur, window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(a: &str, s: &str) -> String {
        fx::format(&transform(a, fx::parse(s).unwrap()))
    }

    #[test]
    fn ciclos() {
        assert_eq!(run("roll", ""), "R2");
        assert_eq!(run("roll", "R4<"), "R6<");
        assert_eq!(run("roll", "R8"), "");
        assert_eq!(run("roll_cresc", ""), "R4<");
        assert_eq!(run("roll_cresc", "R4<"), "R4>");
        assert_eq!(run("flam", "D+1"), "F D+1");
        assert_eq!(run("flam", "F"), "");
        assert_eq!(run("later", "D+5"), "D+5");
        assert_eq!(run("earlier", "D+1"), "");
        assert_eq!(run("cut", ""), "X");
        assert_eq!(run("softer", ""), "V7");
        assert_eq!(run("softer", "V0"), "");
        assert_eq!(run("exact", "H7"), "H0");
        assert_eq!(run("exact", "H0"), "");
        assert_eq!(run("clear", "R4 F X"), "");
    }
}
