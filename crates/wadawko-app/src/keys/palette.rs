//! Paleta de comandos (Ctrl+K / Ctrl+P): lista TODOS os comandos do registro
//! (`keys::SPECS`) + as músicas + frases do repertório e kiai prontos, com busca fuzzy sem acento, o atalho de cada
//! comando e ✓ nos toggles. `Ctrl+K Ctrl+T/V/L/M/S` são chords dentro dela.

use gpui_kit::{Action, App, ClickEvent, Context, FontWeight, KeyDownEvent, MouseButton, SharedString, Window, div, prelude::*, px};

use wadawko_core::kyai::KYAI_PRESETS;
use wadawko_core::vocab::PHRASES;

use crate::keys;
use crate::shell::{Palette, Shell};
use crate::theme::Theme;
use crate::ui;
use crate::views::{Page, Tab};

pub enum PRun {
    Action(Box<dyn Action>),
    Song(usize),
    /// inserir a frase `PHRASES[i]` no cursor da grade
    Phrase(usize),
    /// inserir um kiai pronto na sub-linha de kiai do cursor
    Kyai(&'static str),
    /// pôr em loop o trecho de ensaio (id)
    Practice(String),
    /// música nova a partir de um modelo (`wadawko_core::templates`)
    Template(&'static str),
    /// modo "ensaiar como…" (`None` = todos)
    Rehearse(Option<crate::state::Rehearse>),
    /// visão por pessoa (`None` = todas as linhas)
    ViewPerson(Option<String>),
}

/// item da paleta: (título, grupo, atalho, o que fazer)
pub type PItem = (String, &'static str, Option<String>, PRun);

impl Shell {
    fn toggle_state(&self, id: &str, cx: &App) -> Option<bool> {
        let st = self.state.read(cx);
        Some(match id {
            "transport::ToggleLoop" => st.tp.looping,
            "transport::ToggleMetronome" => st.tp.metronome,
            "transport::ToggleCountIn" => st.tp.count_in,
            "transport::ToggleCountInLoop" => st.tp.count_in_loop,
            "transport::ToggleTrainer" => st.tp.trainer,
            "transport::ToggleSwing" => st.song().swing() > 0.0,
            "keys::ToggleVim" => self.vim_on,
            "view::ToggleTheme" => st.theme.dark,
            "view::ToggleNav" => self.nav_open,
            _ => return None,
        })
    }

    /// Executa o item escolhido na paleta.
    fn run_palette(&mut self, run: PRun, window: &mut Window, cx: &mut Context<Self>) {
        match run {
            PRun::Action(a) => window.dispatch_action(a, cx),
            PRun::Song(i) => self.select_project(i, cx),
            PRun::Practice(id) => self.loop_spot(&id, cx),
            PRun::ViewPerson(p) => {
                let name = p.as_ref().and_then(|id| self.song(cx).person(id).map(|x| x.name.clone()));
                self.grid.update(cx, |g, cx| {
                    g.view_person = p;
                    // o cursor não pode ficar numa linha escondida
                    if let Some(c) = g.cursor {
                        let t = g.nearest_visible_pub(c.t);
                        g.cursor = Some(crate::views::grid::Cur { t, ..c });
                    }
                    cx.notify();
                });
                self.flash(name.map_or_else(|| "visão: todas as linhas".into(), |n| format!("visão: só as linhas de {n}")), cx);
            }
            PRun::Rehearse(r) => {
                let msg = match &r {
                    Some(r) => format!(
                        "ensaiar como {}: {} no som",
                        self.song(cx).person(&r.person).map_or(r.person.clone(), |p| p.name.clone()),
                        if r.listen == crate::state::Listen::Without { "sem ela" } else { "só ela" }
                    ),
                    None => "ensaiar como: todos".into(),
                };
                self.state.update(cx, |s, cx| s.set_rehearse(r, cx));
                self.flash(msg, cx);
            }
            PRun::Template(id) => {
                if let Some(song) = wadawko_core::templates::template_song(id) {
                    self.state.update(cx, |s, cx| s.add_project(song, cx));
                    self.flash("música nova a partir do modelo", cx);
                }
            }
            PRun::Phrase(_) | PRun::Kyai(_) => {
                if self.grid.read(cx).cursor.is_none() {
                    self.flash("ponha o cursor numa célula da grade primeiro", cx);
                    return;
                }
                self.page = Page::Editor;
                self.tab = Tab::Grade;
                self.grid.update(cx, |g, cx| match run {
                    PRun::Phrase(i) => g.insert_phrase(i, cx),
                    PRun::Kyai(k) => g.insert_kyai(k, cx),
                    _ => {}
                });
                cx.notify();
            }
        }
    }

    pub fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hints = None;
        self.palette = Some(Palette { query: String::new(), sel: 0 });
        self.palette_focus.focus(window, cx);
        cx.notify();
    }

    pub fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Itens filtrados e ordenados: (título, grupo, atalho, o que fazer).
    pub fn palette_items(&self, cx: &App) -> Vec<PItem> {
        let q = self.palette.as_ref().map(|p| p.query.as_str()).unwrap_or("");
        let mut scored: Vec<(i32, usize, PItem)> = Vec::new();
        for (i, s) in keys::SPECS.iter().enumerate() {
            let Ok(action) = cx.build_action(s.id, None) else { continue };
            let title = match self.toggle_state(s.id, cx) {
                Some(true) => format!("{}  ✓", s.title),
                _ => s.title.to_string(),
            };
            let hay = format!("{} {} {}", s.title, s.group, s.keywords);
            if let Some(sc) = keys::fuzzy(q, &hay) {
                let tsc = keys::fuzzy(q, s.title).unwrap_or(sc);
                let binding = keys::binding_text(&*action, cx);
                scored.push((tsc.max(sc), i, (title, s.group, binding, PRun::Action(action))));
            }
        }
        for (i, p) in self.state.read(cx).store.projects().iter().enumerate() {
            let t = &p.title;
            if let Some(sc) = keys::fuzzy(q, &format!("música {t}")) {
                scored.push((sc - 5, 1000 + i, (format!("Abrir música: {t}"), "Música", None, PRun::Song(i))));
            }
        }
        // visão por pessoa (ux #3)
        for (i, p) in self.state.read(cx).song().people().iter().enumerate() {
            if let Some(sc) = keys::fuzzy(q, &format!("visão pessoa {} linhas", p.name)) {
                scored.push((sc + if q.contains("vis") { 90 } else { 0 }, 1050 + i, (format!("Visão: só as linhas de {}", p.name), "Ver", None, PRun::ViewPerson(Some(p.id.clone())))));
            }
        }
        if self.grid.read(cx).view_person.is_some()
            && let Some(sc) = keys::fuzzy(q, "visão todas linhas todos")
        {
            scored.push((sc + if q.contains("vis") { 91 } else { 0 }, 1049, ("Visão: todas as linhas".into(), "Ver", None, PRun::ViewPerson(None))));
        }
        // ensaiar como… (ux-pesquisa #1)
        for (i, p) in self.state.read(cx).song().people().iter().enumerate() {
            for (j, (listen, what)) in [(crate::state::Listen::Without, "tocar junto, sem ela no som"), (crate::state::Listen::Only, "ouvir só ela")].into_iter().enumerate() {
                if let Some(sc) = keys::fuzzy(q, &format!("ensaiar como {} {what}", p.name)) {
                    scored.push((
                        sc + if !q.contains("ensai") {
                            0
                        } else if j == 0 {
                            80
                        } else {
                            20
                        },
                        1100 + i * 2 + j,
                        (format!("Ensaiar como {}: {what}", p.name), "Transporte", None, PRun::Rehearse(Some(crate::state::Rehearse { person: p.id.clone(), listen }))),
                    ));
                }
            }
        }
        if self.state.read(cx).tp.rehearse.is_some()
            && let Some(sc) = keys::fuzzy(q, "ensaiar como todos desligar")
        {
            scored.push((sc + 1, 1099, ("Ensaiar como: todos (desligar)".into(), "Transporte", None, PRun::Rehearse(None))));
        }
        // modelos de música (F9)
        for (i, t) in wadawko_core::templates::TEMPLATES.iter().enumerate() {
            if let Some(sc) = keys::fuzzy(q, &format!("nova música modelo template {} {}", t.name, t.about)) {
                let tsc = keys::fuzzy(q, &format!("nova {}", t.name)).unwrap_or(sc);
                scored.push((tsc.max(sc) - 4, 1200 + i, (format!("Nova música: {} — {}", t.name, t.about), "Música", None, PRun::Template(t.id))));
            }
        }
        // trechos de ensaio da música atual (F7b)
        for (i, sp) in wadawko_core::ops::practice::spots(self.state.read(cx).song()).iter().enumerate() {
            if let Some(sc) = keys::fuzzy(q, &format!("trecho ensaio loop {}", sp.name)) {
                scored.push((sc - 2, 1500 + i, (format!("Trecho: {}", sp.name), "Transporte", None, PRun::Practice(sp.id.clone()))));
            }
        }
        // frases do repertório e kiai prontos (antes na paleta flutuante da grade)
        for (i, p) in PHRASES.iter().enumerate() {
            if let Some(sc) = keys::fuzzy(q, &format!("inserir frase {} {} {}", p.name, p.phrase, p.hint)) {
                let tsc = keys::fuzzy(q, &format!("frase {}", p.name)).unwrap_or(sc);
                scored.push((tsc.max(sc) - 3, 2000 + i, (format!("Inserir frase: {} — {}", p.name, p.phrase), "Frases", None, PRun::Phrase(i))));
            }
        }
        for (i, k) in KYAI_PRESETS.iter().enumerate() {
            if let Some(sc) = keys::fuzzy(q, &format!("inserir kiai grito {k}")) {
                scored.push((sc - 3, 3000 + i, (format!("Inserir kiai: {k}"), "Frases", None, PRun::Kyai(k))));
            }
        }
        if !q.is_empty() {
            scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            // corta casamentos fracos (letras espalhadas) quando há bons
            if let Some(best) = scored.first().map(|x| x.0) {
                scored.retain(|x| x.0 >= best / 2);
            }
        }
        scored.into_iter().map(|(_, _, it)| it).collect()
    }

    fn palette_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.palette.as_mut() else { return };
        let k = ev.keystroke.key.as_str();
        match k {
            "escape" => return self.close_palette(window, cx),
            "up" => p.sel = p.sel.saturating_sub(1),
            "down" | "tab" => p.sel += 1,
            "backspace" => {
                p.query.pop();
                p.sel = 0;
            }
            "enter" => {
                let sel = p.sel;
                let mut items = self.palette_items(cx);
                if sel < items.len() {
                    let (_, _, _, run) = items.swap_remove(sel);
                    self.close_palette(window, cx);
                    self.run_palette(run, window, cx);
                }
                return;
            }
            _ => {
                if let Some(ch) = ev.keystroke.key_char.as_deref()
                    && !ev.keystroke.modifiers.control
                    && !ev.keystroke.modifiers.alt
                {
                    p.query.push_str(ch);
                    p.sel = 0;
                }
            }
        }
        let n = self.palette_items(cx).len();
        if let Some(p) = self.palette.as_mut() {
            p.sel = p.sel.min(n.saturating_sub(1));
        }
        cx.notify();
    }

    pub(crate) fn palette_view(&self, p: &Palette, th: &Theme, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let items = self.palette_items(cx);
        let total = items.len();
        let sel = p.sel.min(total.saturating_sub(1));
        let start = sel.saturating_sub(7);
        let mut list = div().flex().flex_col().gap(px(2.0)).overflow_hidden();
        for (i, (title, group, binding, _)) in items.into_iter().enumerate().skip(start).take(11) {
            let on = i == sel;
            list = list.child(
                div()
                    .id(("pal", i))
                    .relative()
                    .px(px(10.0))
                    .h(px(30.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .rounded(px(ui::R0))
                    .cursor_pointer()
                    .when(on, |d| d.bg(th.accent2_wash).child(div().absolute().left_0().top_0().bottom_0().w(px(2.0)).bg(th.accent2)))
                    .hover(|s| s.bg(th.paper_hover))
                    .child(div().w(px(84.0)).text_size(px(10.0)).font_weight(FontWeight::MEDIUM).text_color(th.ink_faint).child(SharedString::from(group.to_uppercase())))
                    .child(div().flex_1().text_size(px(13.0)).text_color(if on { th.ink } else { th.ink_soft }).when(on, |d| d.font_weight(FontWeight::MEDIUM)).child(SharedString::from(title)))
                    .when_some(binding, |d, b| d.child(ui::kbd(&b, th)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if let Some(p) = this.palette.as_mut() {
                            p.sel = i;
                        }
                        let mut items = this.palette_items(cx);
                        if i < items.len() {
                            let (_, _, _, run) = items.swap_remove(i);
                            this.close_palette(window, cx);
                            this.run_palette(run, window, cx);
                        }
                    })),
            );
        }
        let query: SharedString = if p.query.is_empty() { "Digite um comando… (fuzzy, sem acento)".into() } else { format!("{}▏", p.query).into() };
        div()
            .id("palette-overlay")
            .absolute()
            .inset_0()
            .bg(Theme::alpha(th.ink, 0.18))
            .flex()
            .justify_center()
            .pt(px(70.0))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.close_palette(window, cx)))
            .child(
                div()
                    .id("palette")
                    .key_context("Palette")
                    .track_focus(&self.palette_focus)
                    .on_key_down(cx.listener(Self::palette_key))
                    // Tab/Shift+Tab: viram ações (senão o Tab do gpui_kit, na raiz, só troca o foco)
                    .on_action(cx.listener(|this: &mut Shell, _: &keys::keys::PaletteNext, _, cx| {
                        let n = this.palette_items(cx).len();
                        if let Some(p) = this.palette.as_mut() {
                            p.sel = (p.sel + 1).min(n.saturating_sub(1));
                            cx.notify();
                        }
                    }))
                    .on_action(cx.listener(|this: &mut Shell, _: &keys::keys::PalettePrev, _, cx| {
                        if let Some(p) = this.palette.as_mut() {
                            p.sel = p.sel.saturating_sub(1);
                            cx.notify();
                        }
                    }))
                    // Ctrl+K Ctrl+T etc.: fecham a paleta e seguem para o handler da raiz
                    .on_action(cx.listener(|this, _: &keys::view::ToggleTheme, w, cx| {
                        this.close_palette(w, cx);
                        cx.propagate();
                    }))
                    .on_action(cx.listener(|this, _: &keys::keys::ToggleVim, w, cx| {
                        this.close_palette(w, cx);
                        cx.propagate();
                    }))
                    .on_action(cx.listener(|this, _: &keys::transport::ToggleLoop, w, cx| {
                        this.close_palette(w, cx);
                        cx.propagate();
                    }))
                    .on_action(cx.listener(|this, _: &keys::transport::ToggleMetronome, w, cx| {
                        this.close_palette(w, cx);
                        cx.propagate();
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .w(px(600.0))
                    .max_h(px(560.0))
                    .p(px(10.0))
                    .rounded(px(ui::R0))
                    .bg(th.paper)
                    .border_1()
                    .border_color(th.line_strong)
                    .shadow_sm()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .h(px(38.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .rounded(px(ui::R1))
                            .bg(th.paper_raise)
                            .text_size(px(14.0))
                            .text_color(if p.query.is_empty() { th.ink_faint } else { th.ink })
                            .child(SharedString::from(format!("›  {query}"))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.0))
                            .items_center()
                            .text_size(px(10.5))
                            .text_color(th.ink_faint)
                            .child("também:")
                            .child(ui::kbd("Ctrl K Ctrl T", th))
                            .child("tema")
                            .child(ui::kbd("Ctrl K Ctrl V", th))
                            .child("vim")
                            .child(ui::kbd("Ctrl K Ctrl L", th))
                            .child("loop")
                            .child(ui::kbd("Ctrl K Ctrl M", th))
                            .child("metrônomo")
                            .child(ui::kbd("Esc", th))
                            .child("fechar"),
                    )
                    .child(list)
                    .child(div().text_size(px(10.5)).text_color(th.ink_faint).child(SharedString::from(format!("{total} comandos · ↑↓ escolher · Enter executar")))),
            )
    }
}
