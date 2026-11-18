//! Vim além do básico (docs/planejamento/teclado.md §3.4–3.7, K4): repetição `.`,
//! registradores `"x`, marcas `m`/`'`, busca `/` `?` `n` `N`, linha de comando `:`
//! e o prompt de texto (também usado para renomear seção/linha/música e BPM).

use gpui_kit::{Context, FontWeight, KeyDownEvent, SharedString, Window, div, prelude::*, px};
use wadawko_core::ops;

use crate::keys;
use crate::shell::Shell;
use crate::song_ext::SongExt;
use crate::theme::Theme;
use crate::views::Page;
use crate::views::grid::{Cur, GridView, InputTarget, Lane, Motion, VMode};

/// Mudanças repetíveis com `.`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChangeKind {
    DeleteCell,
    DeleteCellBack,
    DeleteMeasureRow,
    DeleteMeasureAll,
    DeleteBeat,
    DeleteToMeasureEnd,
    DeleteSectionRow,
    PasteAfter,
    PasteBefore,
    ToggleSfz,
    Accent,
    Unaccent,
    Insert,
    Append,
    InsertMeasureStart,
    AppendMeasureEnd,
    Substitute,
    ChangeMeasureRow,
    ChangeBeat,
}

#[derive(Clone)]
pub struct Change {
    pub kind: ChangeKind,
    pub count: usize,
    pub inserted: Vec<String>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum PromptKind {
    Register,
    SetMark,
    GotoMark,
    Search {
        back: bool,
    },
    Command,
    /// texto livre com destino (renomear, BPM, treino)
    Input(InputTarget),
}

pub struct Prompt {
    pub kind: PromptKind,
    pub text: String,
    /// posição no histórico de `:` (↑/↓)
    pub hist: Option<usize>,
}

pub fn fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ā' => 'a',
            'é' | 'ê' | 'ē' => 'e',
            'í' | 'ī' => 'i',
            'ó' | 'ô' | 'õ' | 'ō' => 'o',
            'ú' | 'ū' => 'u',
            'ç' => 'c',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

/// Executa uma mudança na grade. Devolve true se ela entra em insert.
pub fn run_change(g: &mut GridView, kind: ChangeKind, n: usize, cx: &mut Context<GridView>) -> bool {
    use ChangeKind::*;
    let del = |g: &mut GridView, r: Option<crate::views::grid::Range4>, linewise: bool, cx: &mut Context<GridView>| {
        if let Some(r) = r {
            g.delete(r, linewise, cx)
        }
    };
    match kind {
        DeleteCell | Substitute => del(g, g.r_cells(n), false, cx),
        DeleteCellBack => del(g, g.r_cell_back(n), false, cx),
        DeleteMeasureRow | ChangeMeasureRow => del(g, g.r_measures(n, false), true, cx),
        DeleteMeasureAll => del(g, g.r_measures(n, true), true, cx),
        DeleteBeat | ChangeBeat => del(g, g.r_beats(n), false, cx),
        DeleteToMeasureEnd => del(g, g.r_to_measure_end(), false, cx),
        DeleteSectionRow => del(g, g.r_section(), false, cx),
        PasteAfter => {
            let sel = g.reg_sel;
            for _ in 0..n {
                g.reg_sel = sel;
                g.paste(true, cx)
            }
        }
        PasteBefore => g.paste(false, cx),
        ToggleSfz => g.toggle_sfz(cx),
        Accent => g.map_words(|w| w.to_uppercase(), cx),
        Unaccent => g.map_words(|w| w.to_lowercase(), cx),
        Insert => {}
        Append => g.motion(Motion::Right, 1, false, cx),
        InsertMeasureStart => g.motion(Motion::MeasureStart, 1, false, cx),
        AppendMeasureEnd => g.motion(Motion::MeasureEnd, 1, false, cx),
    }
    let insert = matches!(kind, Insert | Append | InsertMeasureStart | AppendMeasureEnd | Substitute | ChangeMeasureRow | ChangeBeat);
    if insert {
        g.anchor = None;
        g.vmode = VMode::Insert;
        g.insert_log.clear();
    }
    cx.notify();
    insert
}

impl Shell {
    pub fn set_offset(&mut self, ms: f32, cx: &mut Context<Self>) {
        let ms = ms.clamp(-300.0, 300.0).round();
        self.state.update(cx, |s, _| s.av_offset_ms = ms);
        self.save_settings(cx);
        self.flash(format!("sincronia: visual {ms:+.0} ms"), cx);
    }

    // ───────── `.` ─────────

    pub fn change(&mut self, kind: ChangeKind, cx: &mut Context<Self>) {
        let n = self.take_count();
        let insert = self.grid.update(cx, |g, cx| run_change(g, kind, n, cx));
        self.last_change = Some(Change { kind, count: n, inserted: Vec::new() });
        self.recording = insert;
        cx.notify();
    }

    /// Esc no insert: fecha a sessão e guarda o texto digitado na última mudança.
    pub fn finish_insert(&mut self, cx: &mut Context<Self>) {
        let log = self.grid.update(cx, |g, cx| {
            g.leave_insert(cx);
            std::mem::take(&mut g.insert_log)
        });
        if self.recording {
            if let Some(c) = self.last_change.as_mut() {
                c.inserted = log;
            }
            self.recording = false;
        }
        cx.notify();
    }

    pub fn repeat_change(&mut self, cx: &mut Context<Self>) {
        let Some(ch) = self.last_change.clone() else {
            self.flash("nada para repetir", cx);
            return;
        };
        let n = self.count.take().map(|c| c as usize).unwrap_or(ch.count);
        self.grid.update(cx, |g, cx| {
            let insert = run_change(g, ch.kind, n, cx);
            if insert {
                for syl in &ch.inserted {
                    if let Some(c) = g.cursor {
                        g.set_cell(c, Some(syl.clone()), cx);
                        g.motion(Motion::Right, 1, false, cx);
                    }
                }
                g.vmode = VMode::Normal;
                g.insert_log.clear();
            }
        });
        cx.notify();
    }

    // ───────── prompt (registrador, marcas, busca, `:`, renomear) ─────────

    pub fn open_prompt(&mut self, kind: PromptKind, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        self.hints = None;
        self.prompt = Some(Prompt { kind, text: String::new(), hist: None });
        self.prompt_focus.focus(window, cx);
        cx.notify();
    }

    pub fn close_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub fn prompt_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.prompt.as_mut() else { return };
        let key = ev.keystroke.key.as_str();
        let ch = ev.keystroke.key_char.clone().filter(|_| !ev.keystroke.modifiers.control && !ev.keystroke.modifiers.alt);
        let single = matches!(p.kind, PromptKind::Register | PromptKind::SetMark | PromptKind::GotoMark);
        let is_input = matches!(p.kind, PromptKind::Input(_));
        match key {
            "escape" => return self.close_prompt(window, cx),
            "backspace" => {
                if ev.keystroke.modifiers.control {
                    p.text.clear();
                } else if p.text.pop().is_none() && !is_input {
                    return self.close_prompt(window, cx);
                }
            }
            "up" | "down" if p.kind == PromptKind::Command && !self.cmd_history.is_empty() => {
                let len = self.cmd_history.len();
                let i = match (p.hist, key) {
                    (None, _) => len - 1,
                    (Some(i), "up") => i.saturating_sub(1),
                    (Some(i), _) => (i + 1).min(len - 1),
                };
                p.hist = Some(i);
                p.text = self.cmd_history[i].clone();
            }
            "enter" if !single => {
                let p = self.prompt.take().expect("prompt");
                self.focus.focus(window, cx);
                self.submit_prompt(p, window, cx);
                return;
            }
            "space" if is_input => p.text.push(' '),
            _ => {
                if let Some(c) = ch {
                    p.text.push_str(&c);
                    if single {
                        let p = self.prompt.take().expect("prompt");
                        self.focus.focus(window, cx);
                        self.submit_prompt(p, window, cx);
                        return;
                    }
                }
            }
        }
        cx.notify();
    }

    fn submit_prompt(&mut self, p: Prompt, window: &mut Window, cx: &mut Context<Self>) {
        let t = p.text.trim().to_string();
        match p.kind {
            PromptKind::Register => {
                let c = t.chars().next().unwrap_or('"');
                if c.is_ascii_alphanumeric() || c == '+' || c == '"' {
                    self.grid.update(cx, |g, _| g.reg_sel = Some(c));
                } else {
                    self.flash(format!("registrador inválido: {c}"), cx);
                }
            }
            PromptKind::SetMark => {
                if let (Some(c), Some(cur)) = (t.chars().next(), self.grid.read(cx).cursor) {
                    self.marks.insert(c, cur);
                    self.flash(format!("marca '{c}"), cx);
                }
            }
            PromptKind::GotoMark => match t.chars().next().and_then(|c| self.marks.get(&c).copied()) {
                Some(c) => self.jump_to(c, cx),
                None => self.flash("marca não definida", cx),
            },
            PromptKind::Search { back } => {
                if !t.is_empty() {
                    self.last_search = Some((t.clone(), back));
                }
                self.search(!back, cx);
            }
            PromptKind::Command => {
                if !t.is_empty() {
                    self.cmd_history.retain(|h| h != &t);
                    self.cmd_history.push(t.clone());
                }
                self.ex(&t, window, cx);
            }
            PromptKind::Input(target) => self.submit_input(target, &t, cx),
        }
        cx.notify();
    }

    /// Enter num prompt de texto: aplica o valor ao destino.
    pub fn submit_input(&mut self, target: InputTarget, t: &str, cx: &mut Context<Self>) {
        let num = t.trim_end_matches('%').trim().replace(',', ".").parse::<f64>().ok();
        match target {
            InputTarget::SectionName(id) if !t.is_empty() => {
                self.edit(cx, |s| ops::sections::rename_section(s, &id, t));
            }
            InputTarget::TrackName(id) if !t.is_empty() => {
                self.edit(cx, |s| ops::tracks::rename_track(s, &id, t));
            }
            InputTarget::PersonName(id) if !t.is_empty() => {
                self.edit(cx, |s| ops::people::rename_person(s, &id, t));
            }
            InputTarget::InstrumentName(id) if !t.is_empty() => {
                self.edit(cx, |s| ops::kit::rename_instrument(s, &id, t));
            }
            InputTarget::SongTitle if !t.is_empty() => {
                let id = self.state.read(cx).store.current_id().to_string();
                self.state.update(cx, |s, cx| s.rename_project(&id, t, cx));
            }
            InputTarget::Bpm => match num {
                Some(b) => self.state.update(cx, |s, cx| s.set_bpm(b, cx)),
                None => self.flash("BPM: digite um número (30–300)", cx),
            },
            InputTarget::TrainerPct => match num {
                Some(v) => self.state.update(cx, |s, cx| {
                    s.tp.trainer_pct = (v / 100.0).clamp(0.2, 1.0);
                    cx.notify()
                }),
                None => self.flash("treino: % inicial entre 20 e 100", cx),
            },
            InputTarget::TrainerStep => match num {
                Some(v) => self.state.update(cx, |s, cx| {
                    s.tp.trainer_step = (v / 100.0).clamp(0.01, 0.5);
                    cx.notify()
                }),
                None => self.flash("treino: passo entre 1 e 50 (% por volta)", cx),
            },
            _ => {}
        }
    }

    pub fn jump_to(&mut self, c: Cur, cx: &mut Context<Self>) {
        self.grid.update(cx, |g, cx| {
            g.cursor = Some(c);
            g.clamp_cursor();
            if let Some(c) = g.cursor {
                g.ensure_visible(c);
            }
            cx.notify();
        });
        self.reveal_cursor(cx);
    }

    /// `n` / `N` / Enter do `/`: próxima célula cujo texto contém a busca (ordem tempo → linha).
    pub fn search(&mut self, forward_rel: bool, cx: &mut Context<Self>) {
        let Some((q, back)) = self.last_search.clone() else {
            self.flash("nenhuma busca ainda (/)", cx);
            return;
        };
        let forward = forward_rel != back;
        let q = fold(&q);
        let found = {
            let g = self.grid.read(cx);
            let song = &g.song;
            let cur = g.cursor.unwrap_or(Cur { m: 0, t: 0, k: 0 });
            find_cell(song, cur, &q, forward)
        };
        match found {
            Some((c, wrapped)) => {
                self.grid.update(cx, |g, _| g.lane = Lane::Cells);
                self.jump_to(c, cx);
                if wrapped {
                    self.flash("busca recomeçou do outro lado", cx);
                }
            }
            None => self.flash(format!("“{q}” não encontrado"), cx),
        }
    }

    /// Linha de comando `:`.
    fn ex(&mut self, cmd: &str, window: &mut Window, cx: &mut Context<Self>) {
        let mut parts = cmd.split_whitespace();
        let head = parts.next().unwrap_or("");
        let arg: String = parts.collect::<Vec<_>>().join(" ");
        let num = |s: &str| s.parse::<usize>().ok();
        let song = self.song(cx);
        match head {
            "" => {}
            h if num(h).is_some() => {
                let m = num(h).unwrap_or(1).max(1) - 1;
                let t = self.cursor_track(cx);
                self.jump_to(Cur { m: m.min(song.measures.len().saturating_sub(1)), t, k: 0 }, cx);
            }
            "s" | "secao" | "seção" => {
                let n = num(&arg).unwrap_or(1).max(1) - 1;
                let secs = song.secs();
                let m = secs.get(n.min(secs.len().saturating_sub(1))).map(|s| s.start).unwrap_or(0);
                let t = self.cursor_track(cx);
                self.jump_to(Cur { m, t, k: 0 }, cx);
            }
            "offset" | "sync" => match arg.trim_end_matches("ms").parse::<f32>() {
                Ok(v) => self.set_offset(v, cx),
                Err(_) => {
                    let cur = self.state.read(cx).av_offset_ms;
                    self.flash(format!("offset atual {cur:+.0} ms · uso: :offset 40"), cx)
                }
            },
            "bpm" | "tempo" => match arg.parse::<f64>() {
                Ok(b) => self.state.update(cx, |s, cx| s.set_bpm(b, cx)),
                Err(_) => self.flash("uso: :bpm 120", cx),
            },
            "sub" => match num(&arg) {
                Some(n @ (2 | 4 | 8)) => {
                    self.edit(cx, |s| ops::measures::set_subdivision(s, n));
                }
                _ => self.flash("uso: :sub 2|4|8", cx),
            },
            "set" => {
                let (name, on) = match arg.strip_prefix("no") {
                    Some(rest) => (rest.to_string(), false),
                    None => (arg.clone(), true),
                };
                match name.as_str() {
                    "vim" => self.set_vim(on, cx),
                    "dark" | "escuro" => self.set_theme(on, cx),
                    "light" | "claro" => self.set_theme(!on, cx),
                    "loop" => self.state.update(cx, |s, cx| s.set_loop(on, cx)),
                    "metro" | "metronomo" => self.state.update(cx, |s, cx| s.set_metronome(on, cx)),
                    "countin" => self.state.update(cx, |s, _| s.tp.count_in = on),
                    "treino" | "trainer" => self.state.update(cx, |s, _| s.tp.trainer = on),
                    "swing" => {
                        self.edit(cx, |s| s.swing = on.then_some(1.0));
                    }
                    _ => self.flash(format!(":set {arg}? (vim, dark, light, loop, metro, countin, treino, swing — com no na frente para desligar)"), cx),
                }
            }
            "e" | "edit" | "abrir" => {
                let best = {
                    let st = self.state.read(cx);
                    st.store.projects().iter().enumerate().filter_map(|(i, p)| keys::fuzzy(&arg, &p.title).map(|s| (s, i))).max_by_key(|x| x.0).map(|x| x.1)
                };
                match best {
                    Some(i) => self.select_project(i, cx),
                    None => self.flash(format!("nenhuma música “{arg}”"), cx),
                }
            }
            "new" | "enew" | "nova" => self.state.update(cx, |s, cx| {
                s.new_project(cx);
                if !arg.is_empty() {
                    let id = s.store.current_id().to_string();
                    s.rename_project(&id, &arg, cx);
                    s.history.clear();
                }
            }),
            "u" | "undo" => self.state.update(cx, |s, cx| {
                s.undo(cx);
            }),
            "redo" => self.state.update(cx, |s, cx| {
                s.redo(cx);
            }),
            "w" | "write" | "salvar" => window.dispatch_action(Box::new(keys::project::Save), cx),
            "q" | "quit" | "sair" | "wq" | "x" => {
                let _ = self.state.update(cx, |s, _| s.flush());
                cx.quit()
            }
            "help" | "h" | "atalhos" => self.page = Page::Config,
            "reg" | "registers" => {
                let g = self.grid.read(cx);
                let mut names: Vec<char> = g.registers.keys().copied().collect();
                names.sort();
                let s = names
                    .iter()
                    .map(|c| {
                        let r = &g.registers[c];
                        let first = r.rows.first().map(|row| row.iter().flatten().take(4).cloned().collect::<Vec<_>>().join(" ")).unwrap_or_default();
                        format!("\"{c} {first}")
                    })
                    .collect::<Vec<_>>()
                    .join("   ");
                self.flash(if s.is_empty() { "registradores vazios".to_string() } else { s }, cx);
            }
            "marks" => {
                let mut v: Vec<_> = self.marks.iter().map(|(c, p)| format!("'{c} c{}", p.m + 1)).collect();
                v.sort();
                self.flash(if v.is_empty() { "nenhuma marca".to_string() } else { v.join("  ") }, cx);
            }
            _ => {
                // atalho: qualquer comando da paleta pelo nome
                let best = keys::SPECS.iter().filter_map(|s| keys::fuzzy(cmd, s.title).map(|sc| (sc, s.id))).max_by_key(|x| x.0);
                match best.and_then(|(_, id)| cx.build_action(id, None).ok()) {
                    Some(a) => window.dispatch_action(a, cx),
                    None => self.flash(format!("comando desconhecido: {cmd}"), cx),
                }
            }
        }
    }

    pub fn prompt_bar(&self, p: &Prompt, th: &Theme) -> impl IntoElement + use<> {
        let (prefix, hint): (&str, &str) = match &p.kind {
            PromptKind::Register => ("\"", "registrador: a–z, 0, + (área de transferência), \" (sem nome)"),
            PromptKind::SetMark => ("m", "letra da marca"),
            PromptKind::GotoMark => ("'", "letra da marca"),
            PromptKind::Search { back: false } => ("/", "sílaba ou trecho (sem acento) · Enter busca · n/N repetem"),
            PromptKind::Search { back: true } => ("?", "busca para trás · Enter busca · n/N repetem"),
            PromptKind::Command => (":", "12 · s 2 · bpm 120 · sub 4 · set novim · e Renshū · new · reg · marks · w · q · ou nome de comando"),
            PromptKind::Input(InputTarget::SectionName(_)) => ("seção:", "Enter confirma · Esc cancela · Ctrl+Backspace limpa"),
            PromptKind::Input(InputTarget::TrackName(_)) => ("linha:", "Enter confirma · Esc cancela · Ctrl+Backspace limpa"),
            PromptKind::Input(InputTarget::PersonName(_)) => ("pessoa:", "Enter confirma · Esc cancela · Ctrl+Backspace limpa"),
            PromptKind::Input(InputTarget::InstrumentName(_)) => ("instrumento:", "Enter confirma · Esc cancela · Ctrl+Backspace limpa"),
            PromptKind::Input(InputTarget::SongTitle) => ("música:", "Enter confirma · Esc cancela · Ctrl+Backspace limpa"),
            PromptKind::Input(InputTarget::Bpm) => ("BPM:", "30–300 · Enter confirma"),
            PromptKind::Input(InputTarget::TrainerPct) => ("treino, começa em %:", "20–100 · Enter confirma"),
            PromptKind::Input(InputTarget::TrainerStep) => ("treino, +% por volta:", "1–50 · Enter confirma"),
        };
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(26.0))
            .h(px(30.0))
            .px(px(12.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .bg(th.paper)
            .border_t_1()
            .border_color(th.accent)
            .child(div().text_size(px(15.0)).font_weight(FontWeight::BOLD).text_color(th.accent).child(SharedString::from(prefix.to_string())))
            .child(div().text_size(px(14.0)).child(SharedString::from(format!("{}▏", p.text))))
            .child(div().flex_1())
            .child(div().text_size(px(11.0)).text_color(th.ink_faint).child(SharedString::from(hint.to_string())))
    }
}

/// Próxima célula (ordem tempo → linha) cujo texto, sem acento/caixa, contém `q`.
/// Devolve (posição, deu a volta?).
pub fn find_cell(song: &wadawko_core::Song, cur: Cur, q: &str, forward: bool) -> Option<(Cur, bool)> {
    let tracks = song.tracks.len();
    let total = song.total_cells();
    if tracks == 0 || total == 0 {
        return None;
    }
    let pos = song.flat(cur.m, cur.k) * tracks + cur.t;
    let n = total * tracks;
    (1..=n).find_map(|i| {
        let p = if forward { (pos + i) % n } else { (pos + n - i) % n };
        let (g_, t) = (p / tracks, p % tracks);
        let hit = song.cell_at(t, g_).is_some_and(|w| fold(w).contains(q));
        hit.then(|| {
            let (m, k) = song.unflat(g_);
            (Cur { m, t, k }, if forward { p <= pos } else { p >= pos })
        })
    })
}

pub fn create_user_keymap() -> anyhow::Result<std::path::PathBuf> {
    let p = keys::user_keymap_path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if !p.exists() {
        std::fs::write(
            &p,
            r#"[
  {
    "context": "Workspace && mode == normal",
    "bindings": {
      "space t +": "transport::BpmUp"
    }
  }
]
"#,
        )?;
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wadawko_core::ops::cells::set_cell;

    #[test]
    fn search_finds_next_and_wraps() {
        let mut song = wadawko_core::demo::demo_song();
        let ids: Vec<String> = song.tracks.iter().map(|t| t.id.clone()).collect();
        // limpa tudo e planta duas ocorrências
        for mi in 0..song.measures.len() {
            for id in &ids {
                for ci in 0..song.measures[mi].cell_count() {
                    set_cell(&mut song, id, mi, ci, None, None);
                }
            }
        }
        set_cell(&mut song, &ids[0], 0, 1, Some("DOKO"), None);
        let last = song.measures.len() - 1;
        set_cell(&mut song, &ids[0], last, 0, Some("doko!"), None);
        let start = Cur { m: 0, t: 0, k: 0 };
        let (a, w) = find_cell(&song, start, &fold("Doko"), true).unwrap();
        assert_eq!((a, w), (Cur { m: 0, t: 0, k: 1 }, false));
        let (b, w) = find_cell(&song, a, "doko", true).unwrap();
        assert_eq!((b, w), (Cur { m: last, t: 0, k: 0 }, false));
        let (c, w) = find_cell(&song, b, "doko", true).unwrap();
        assert_eq!((c, w), (a, true));
        let (d, w) = find_cell(&song, a, "doko", false).unwrap();
        assert_eq!((d, w), (b, true));
        assert!(find_cell(&song, start, "karan", true).is_none());
    }

    #[test]
    fn fold_removes_accents() {
        assert_eq!(fold("Seção Ōdaiko"), "secao odaiko");
    }
}
