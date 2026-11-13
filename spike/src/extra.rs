//! Vim além do básico (PLANEJAMENTO-teclado.md §3.4–3.7, K4/K5): repetição `.`,
//! registradores `"x`, marcas `m`/`'`, busca `/` `?` `n` `N`, linha de comando `:`,
//! dica de atalhos na raiz (`Ctrl+/`) e a página Configurações (mapa de atalhos,
//! conflitos, testar tecla).

use std::time::Instant;

use gpui_kit::{App, Bounds, ClickEvent, Context, FontWeight, KeyDownEvent, Pixels, SharedString, Window, div, point, prelude::*, px, size};

use crate::audio::PPQ;
use crate::grid;

use crate::grid::{Cur, GridView, Motion, VMode};
use crate::theme::Theme;
use crate::{Spike, keys, model};

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

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PromptKind {
    Register,
    SetMark,
    GotoMark,
    Search { back: bool },
    Command,
}

pub struct Prompt {
    pub kind: PromptKind,
    pub text: String,
    /// posição no histórico de `:` (↑/↓)
    pub hist: Option<usize>,
}

fn fold(s: &str) -> String {
    s.chars()
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
    match kind {
        DeleteCell => {
            if let Some(r) = g.r_cells(n) {
                g.delete(r, false, cx)
            }
        }
        DeleteCellBack => {
            if let Some(r) = g.r_cell_back(n) {
                g.delete(r, false, cx)
            }
        }
        DeleteMeasureRow => {
            if let Some(r) = g.r_measures(n, false) {
                g.delete(r, true, cx)
            }
        }
        DeleteMeasureAll => {
            if let Some(r) = g.r_measures(n, true) {
                g.delete(r, true, cx)
            }
        }
        DeleteBeat => {
            if let Some(r) = g.r_beats(n) {
                g.delete(r, false, cx)
            }
        }
        DeleteToMeasureEnd => {
            if let Some(r) = g.r_to_measure_end() {
                g.delete(r, false, cx)
            }
        }
        DeleteSectionRow => {
            if let Some(r) = g.r_section() {
                g.delete(r, false, cx)
            }
        }
        PasteAfter => {
            let sel = g.reg_sel;
            for _ in 0..n {
                g.reg_sel = sel;
                g.paste(true, cx)
            }
        }
        PasteBefore => g.paste(false, cx),
        ToggleSfz => g.map_words(|w| if let Some(s) = w.strip_suffix('!') { s.to_string() } else { format!("{w}!") }, cx),
        Accent => g.map_words(|w| w.to_uppercase(), cx),
        Unaccent => g.map_words(|w| w.to_lowercase(), cx),
        Insert => {}
        Append => g.motion(Motion::Right, 1, false, cx),
        InsertMeasureStart => g.motion(Motion::MeasureStart, 1, false, cx),
        AppendMeasureEnd => g.motion(Motion::MeasureEnd, 1, false, cx),
        Substitute => {
            if let Some(r) = g.r_cells(n) {
                g.delete(r, false, cx)
            }
        }
        ChangeMeasureRow => {
            if let Some(r) = g.r_measures(n, false) {
                g.delete(r, true, cx)
            }
        }
        ChangeBeat => {
            if let Some(r) = g.r_beats(n) {
                g.delete(r, false, cx)
            }
        }
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

/// Célula soando agora: bounds, intensidade (1 = golpe acabou de soar → 0) e se é borda.
pub struct Flash {
    pub bounds: Bounds<Pixels>,
    pub strength: f32,
    pub rim: bool,
}

impl Spike {
    /// Células que estão soando no instante AUDÍVEL (relógio do áudio − latência − offset).
    pub fn playing_cells(&self, cx: &App) -> (Vec<Flash>, Option<String>) {
        let mut out = Vec::new();
        if !self.playing() {
            return (out, None);
        }
        let order = self.order.borrow();
        let tick = self.audio.shared.audible_tick(order.1, self.av_offset_ms);
        let Some(i) = order.0.partition_point(|(t, _)| (*t as f64) <= tick).checked_sub(1) else { return (out, None) };
        let (start, m) = order.0[i];
        let song = self.song.borrow();
        let Some(meas) = song.measures.get(m) else { return (out, None) };
        let rel = tick - start as f64;
        let sub = meas.subdivision;
        let ppq = PPQ as f64;
        // célula e fase (0..1) dentro dela, respeitando o swing das colcheias
        let (k, phase) = if song.swing && sub == 2 {
            let beat = (rel / ppq).floor();
            let p = rel - beat * ppq;
            if p < ppq * 2.0 / 3.0 { (beat as usize * 2, p / (ppq * 2.0 / 3.0)) } else { (beat as usize * 2 + 1, (p - ppq * 2.0 / 3.0) / (ppq / 3.0)) }
        } else {
            let cf = rel / (ppq / sub as f64);
            (cf.floor() as usize, cf.fract())
        };
        if k >= meas.len() {
            return (out, None);
        }
        let label = format!("c{} · {}.{}", m + 1, k / sub + 1, k % sub + 1);
        let s = song.section_of(m);
        let (a, _) = song.section_range(s);
        let g = self.grid.read(cx).geom.clone();
        let g = g.borrow();
        let Some(b) = g.sections.get(s).copied().filter(|b| f32::from(b.size.width) > 0.0) else { return (out, Some(label)) };
        let xs = grid::measure_xs(&song, s, g.cw);
        let x = f32::from(b.origin.x) + xs[m - a] + k as f32 * g.cw - g.offset;
        for (t, row) in meas.cells.iter().enumerate() {
            let Some(word) = row[k].as_deref() else { continue };
            if !song.audible(t) {
                continue;
            }
            let hits = model::resolve(word);
            // golpe mais recente dentro da célula (doko = 2 golpes): reacende em cada um
            let ph = phase as f32;
            let Some((hi, h)) = hits.iter().enumerate().filter(|(_, h)| h.offset <= ph + 1e-4).last() else { continue };
            let next = hits.iter().map(|x| x.offset).filter(|o| *o > h.offset + 1e-4).fold(1.0f32, f32::min);
            let local = ((ph - h.offset) / (next - h.offset).max(0.05)).clamp(0.0, 1.0);
            let _ = hi;
            let y = f32::from(b.origin.y) + grid::SEC_DYN_H + grid::TIMELINE_H + t as f32 * grid::TRACK_H;
            out.push(Flash {
                bounds: Bounds::new(point(px(x), px(y)), size(px(g.cw), px(grid::CELL_H))),
                // cheio no começo do golpe, depois apaga até a próxima nota
                strength: (if local < 0.35 { 1.0 } else { 1.0 - (local - 0.35) / 0.65 }).powf(0.8) * (0.7 + 0.3 * h.vel.min(1.0)),
                rim: h.rim,
            });
        }
        (out, Some(label))
    }

    pub fn set_offset(&mut self, ms: f32) {
        self.av_offset_ms = ms.clamp(-300.0, 300.0).round();
        let _ = keys::save_settings(keys::Settings { vim: self.vim_on, av_offset_ms: self.av_offset_ms });
        self.flash(format!("sincronia: visual {:+.0} ms", self.av_offset_ms));
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
            self.flash("nada para repetir");
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

    // ───────── prompt (registrador, marcas, busca, `:`) ─────────

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
        match key {
            "escape" => return self.close_prompt(window, cx),
            "backspace" => {
                if p.text.pop().is_none() {
                    return self.close_prompt(window, cx);
                }
            }
            "up" | "down" if p.kind == PromptKind::Command && !self.cmd_history.is_empty() => {
                let len = self.cmd_history.len();
                let i = match (p.hist, key) {
                    (None, "up") => len - 1,
                    (Some(i), "up") => i.saturating_sub(1),
                    (Some(i), _) => (i + 1).min(len - 1),
                    (None, _) => len - 1,
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
                    self.flash(format!("registrador inválido: {c}"));
                }
            }
            PromptKind::SetMark => {
                if let (Some(c), Some(cur)) = (t.chars().next(), self.grid.read(cx).cursor) {
                    self.marks.insert(c, cur);
                    self.flash(format!("marca '{c}"));
                }
            }
            PromptKind::GotoMark => match t.chars().next().and_then(|c| self.marks.get(&c).copied()) {
                Some(c) => self.jump_to(c, cx),
                None => self.flash("marca não definida"),
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
        }
        cx.notify();
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
            self.flash("nenhuma busca ainda (/)");
            return;
        };
        let forward = forward_rel != back;
        let q = fold(&q);
        let found = {
            let g = self.grid.read(cx);
            let song = self.song.borrow();
            let tracks = song.tracks.len();
            let total = song.total_cells();
            let cur = g.cursor.unwrap_or(Cur { m: 0, t: 0, k: 0 });
            let pos = song.flat(cur.m, cur.k) * tracks + cur.t;
            let n = total * tracks;
            (1..=n).find_map(|i| {
                let p = if forward { (pos + i) % n } else { (pos + n - i) % n };
                let (g_, t) = (p / tracks, p % tracks);
                let hit = song.cell_at(t, g_).and_then(|c| c.as_deref()).is_some_and(|w| fold(w).contains(&q));
                hit.then(|| {
                    let (m, k) = song.unflat(g_);
                    (Cur { m, t, k }, if forward { p <= pos } else { p >= pos })
                })
            })
        };
        match found {
            Some((c, wrapped)) => {
                self.jump_to(c, cx);
                if wrapped {
                    self.flash("busca recomeçou do outro lado");
                }
            }
            None => self.flash(format!("“{q}” não encontrado")),
        }
    }

    /// Linha de comando `:`.
    fn ex(&mut self, cmd: &str, window: &mut Window, cx: &mut Context<Self>) {
        let mut parts = cmd.split_whitespace();
        let head = parts.next().unwrap_or("");
        let arg: String = parts.collect::<Vec<_>>().join(" ");
        let num = |s: &str| s.parse::<usize>().ok();
        match head {
            "" => {}
            h if num(h).is_some() => {
                let m = num(h).unwrap_or(1).max(1) - 1;
                let len = self.song.borrow().measures.len();
                let t = self.cursor_track(cx);
                self.jump_to(Cur { m: m.min(len.saturating_sub(1)), t, k: 0 }, cx);
            }
            "s" | "secao" | "seção" => {
                let n = num(&arg).unwrap_or(1).max(1) - 1;
                let m = {
                    let song = self.song.borrow();
                    song.section_range(n.min(song.sections.len().saturating_sub(1))).0
                };
                let t = self.cursor_track(cx);
                self.jump_to(Cur { m, t, k: 0 }, cx);
            }
            "offset" | "sync" => match arg.trim_end_matches("ms").parse::<f32>() {
                Ok(v) => self.set_offset(v),
                Err(_) => self.flash(format!("offset atual {:+.0} ms · uso: :offset 40", self.av_offset_ms)),
            },
            "bpm" | "tempo" => match arg.parse::<f32>() {
                Ok(b) => self.set_bpm(b),
                Err(_) => self.flash("uso: :bpm 120"),
            },
            "sub" => match num(&arg) {
                Some(n @ (2 | 4 | 8)) => {
                    self.song.borrow_mut().set_subdivision(n);
                    self.changed(cx);
                }
                _ => self.flash("uso: :sub 2|4|8"),
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
                    "loop" => {
                        self.looping = on;
                        self.audio.send(crate::audio::Cmd::SetLoop(on));
                    }
                    "metro" | "metronomo" => {
                        self.metronome = on;
                        self.rebuild();
                    }
                    "swing" => {
                        self.song.borrow_mut().swing = on;
                        self.rebuild();
                    }
                    _ => self.flash(format!(":set {arg}? (vim, dark, light, loop, metro, swing — com no na frente para desligar)")),
                }
            }
            "e" | "edit" | "abrir" => {
                let best = self
                    .projects
                    .iter()
                    .enumerate()
                    .filter_map(|(i, p)| keys::fuzzy(&arg, &p.song.title).map(|s| (s, i)))
                    .max_by_key(|x| x.0)
                    .map(|x| x.1);
                match best {
                    Some(i) => self.select_project(i, cx),
                    None => self.flash(format!("nenhuma música “{arg}”")),
                }
            }
            "new" | "enew" | "nova" => self.push_project(model::Song::new_empty(if arg.is_empty() { "Sem título" } else { &arg }), cx),
            "u" | "undo" => self.undo(cx),
            "redo" => self.redo(cx),
            "w" | "write" | "salvar" => self.flash("gravar no banco ainda não existe (Fase 1) — as edições ficam em memória"),
            "q" | "quit" | "sair" => cx.quit(),
            "help" | "h" | "atalhos" => {
                self.page = crate::Page::Config;
            }
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
                self.flash(if s.is_empty() { "registradores vazios".to_string() } else { s });
            }
            "marks" => {
                let mut v: Vec<_> = self.marks.iter().map(|(c, p)| format!("'{c} c{}", p.m + 1)).collect();
                v.sort();
                self.flash(if v.is_empty() { "nenhuma marca".to_string() } else { v.join("  ") });
            }
            _ => {
                // atalho: qualquer comando da paleta pelo nome
                let best = keys::SPECS.iter().filter_map(|s| keys::fuzzy(cmd, s.title).map(|sc| (sc, s.id))).max_by_key(|x| x.0);
                match best.and_then(|(_, id)| cx.build_action(id, None).ok()) {
                    Some(a) => window.dispatch_action(a, cx),
                    None => self.flash(format!("comando desconhecido: {cmd}")),
                }
            }
        }
    }

    pub fn prompt_bar(&self, p: &Prompt, th: &Theme) -> impl IntoElement {
        let (prefix, hint) = match p.kind {
            PromptKind::Register => ("\"", "registrador: a–z, 0, + (área de transferência), \" (sem nome)"),
            PromptKind::SetMark => ("m", "letra da marca"),
            PromptKind::GotoMark => ("'", "letra da marca"),
            PromptKind::Search { back: false } => ("/", "sílaba ou trecho (sem acento) · Enter busca · n/N repetem"),
            PromptKind::Search { back: true } => ("?", "busca para trás · Enter busca · n/N repetem"),
            PromptKind::Command => (":", "12 · s 2 · bpm 120 · sub 4 · set novim · e Renshū · new · reg · marks · q · ou nome de comando"),
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
            .child(div().text_size(px(15.0)).font_weight(FontWeight::BOLD).text_color(th.accent).child(prefix))
            .child(div().text_size(px(14.0)).child(SharedString::from(format!("{}▏", p.text))))
            .child(div().flex_1())
            .child(div().text_size(px(11.0)).text_color(th.ink_faint).child(hint))
    }

    // ───────── página Configurações ─────────

    pub fn config_page(&self, th: &Theme, _cx: &mut Context<Self>) -> impl IntoElement {
        let user = keys::user_keymap_path();
        let exists = user.exists();
        let raw = keys::all_raw_bindings();
        let conflicts = keys::conflicts(&raw);
        let card = || div().p(px(14.0)).rounded(px(10.0)).bg(th.paper).border_1().border_color(th.line).flex().flex_col().gap(px(8.0));
        let h = |t: &'static str| div().text_size(px(15.0)).font_weight(FontWeight::BOLD).child(t);
        let probe = match &self.last_key {
            Some((k, a, at)) => format!("{k}  →  {}   (há {:.0} s)", a.clone().unwrap_or_else(|| "nenhuma ação (texto/sem atalho)".into()), at.elapsed().as_secs_f32()),
            None => "aperte qualquer tecla…".into(),
        };

        let mut table = div().flex().flex_col();
        let mut last_ctx = String::new();
        for b in &raw {
            if b.context != last_ctx {
                last_ctx = b.context.clone();
                table = table.child(
                    div()
                        .mt(px(10.0))
                        .mb(px(2.0))
                        .text_size(px(11.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(th.accent2)
                        .child(SharedString::from(format!("[{}] · {}", b.context, b.origin))),
                );
            }
            let title = keys::title_for(&b.action).unwrap_or("");
            table = table.child(
                div()
                    .h(px(22.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .text_size(px(12.0))
                    .child(div().w(px(150.0)).child(crate::kbd(&b.keys, th)))
                    .child(div().w(px(260.0)).text_color(th.ink).child(SharedString::from(title.to_string())))
                    .child(div().text_color(th.ink_faint).child(SharedString::from(b.action.clone()))),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(div().text_size(px(22.0)).font_family("Shippori Mincho").font_weight(FontWeight::BOLD).child("Configurações"))
            .child(
                card()
                    .child(h("Teclado"))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.0))
                            .items_center()
                            .child(crate::atoggle("cfg-vim", th, self.vim_on, keys::keys::ToggleVim).child(if self.vim_on { "Vim: ligado" } else { "Vim: desligado (modo simples)" }))
                            .child(crate::abtn("cfg-reload", th, keys::keys::ReloadKeymap).child("Recarregar atalhos"))
                            .when(!exists, |d| d.child(crate::abtn("cfg-create", th, keys::keys::CreateKeymap).child("Criar keymap.json"))),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(th.ink_soft)
                            .child(SharedString::from(format!(
                                "Seus atalhos: {} {} — mesmo formato do Zed; recarrega sozinho ao salvar. Preferência do vim: {}",
                                user.display(),
                                if exists { "(existe)" } else { "(não existe)" },
                                keys::settings_path().display()
                            ))),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(th.ink_soft)
                            .child("Ctrl+/ mostra o que dá para apertar aqui · Ctrl+K paleta · Ctrl+Space ou Alt+F saltar · : comandos · / busca"),
                    ),
            )
            .child({
                let a = &self.audio.shared;
                let lat = a.latency_ns.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e6;
                let buf = a.buffer_frames.load(std::sync::atomic::Ordering::Relaxed);
                let pw = std::env::var("PIPEWIRE_LATENCY").unwrap_or_else(|_| "—".into());
                let step = |id: &'static str, label: &'static str, d: f32| {
                    div()
                        .id(id)
                        .px(px(10.0))
                        .h(px(28.0))
                        .flex()
                        .items_center()
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(th.line_strong)
                        .bg(th.paper)
                        .text_size(px(12.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(th.paper_hover))
                        .child(label)
                        .on_click(_cx.listener(move |this: &mut Spike, _: &ClickEvent, _, cx| {
                            let v = if d == 0.0 { 0.0 } else { this.av_offset_ms + d };
                            this.set_offset(v);
                            cx.notify();
                        }))
                };
                card()
                    .child(h("Sincronia som × imagem"))
                    .child(div().text_size(px(12.0)).text_color(th.ink_soft).child(SharedString::from(format!(
                        "latência informada pelo áudio: {lat:.1} ms · buffer {buf} quadros @ {} Hz · PIPEWIRE_LATENCY={pw}",
                        a.sample_rate
                    ))))
                    .child(div().text_size(px(12.0)).text_color(th.ink_soft).child(
                        "As células piscam quando a nota soa. Ligue o metrônomo, toque e ajuste até o destaque bater com o som: se a luz vem antes do som, aumente; se vem depois, diminua. (:offset 40 também funciona.)",
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(step("off-10", "−10 ms", -10.0))
                            .child(step("off-1", "−1", -1.0))
                            .child(
                                div()
                                    .w(px(90.0))
                                    .flex()
                                    .justify_center()
                                    .text_size(px(15.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child(SharedString::from(format!("{:+.0} ms", self.av_offset_ms))),
                            )
                            .child(step("off+1", "+1", 1.0))
                            .child(step("off+10", "+10 ms", 10.0))
                            .child(step("off0", "zerar", 0.0)),
                    )
            })
            .child(
                card()
                    .child(h("Testar tecla"))
                    .child(div().text_size(px(12.0)).text_color(th.ink_soft).child("Se Ctrl+Space não aparecer aqui, o fcitx5/ibus está capturando a tecla — use Alt+F (ou mude o atalho do IME)."))
                    .child(div().px(px(10.0)).py(px(6.0)).rounded(px(6.0)).bg(th.paper_raise).text_size(px(14.0)).font_weight(FontWeight::MEDIUM).child(SharedString::from(probe))),
            )
            .child(
                card()
                    .child(h("Conflitos"))
                    .child(if conflicts.is_empty() {
                        div().text_size(px(12.0)).text_color(th.ink_soft).child("Nenhum conflito no keymap atual.")
                    } else {
                        let mut d = div().flex().flex_col().gap(px(4.0));
                        for c in conflicts {
                            d = d.child(div().text_size(px(12.0)).text_color(th.accent).child(SharedString::from(c)));
                        }
                        d
                    }),
            )
            .child(card().child(h("Mapa de atalhos")).child(table))
    }

    pub fn note_key(&mut self, k: String, action: Option<String>) {
        self.last_key = Some((k, action, Instant::now()));
    }
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
      "space t b": "transport::BpmUp"
    }
  }
]
"#,
        )?;
    }
    Ok(p)
}

#[allow(dead_code)]
fn _assert_app(_: &App) {}
