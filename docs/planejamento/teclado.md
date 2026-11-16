---
type: Plan
title: "UX por teclado (paleta, which-key, hints, vim)"
description: "Trilha de teclado do app nativo: registro único de comandos, keymap em JSON, paleta Ctrl+K, which-key, saltos por rótulo e modo vim."
tags: [planejamento, teclado, ux]
status: stable
estado: "implementado (K0–K4)"
---

# WaDAWko nativo — Planejamento de UX por teclado

> Trilha transversal do app Rust + GPUI: bindings modais estilo Vim na grade, paleta de comandos `Ctrl+K`, comandos em sequência (chords) com *command hint* (which-key) e *jump* por rótulos (*element hints*) com `Ctrl+Espaço`. Complementa `/planejamento/gpui.md` (Fases 3–4) e os itens "teclado-first", "command palette `Ctrl+K`" e "mapa de atalhos" do `/planejamento/redesign-web.md` (§66, §276).

Data: 2026-11-10 · Status: planejamento, pré-código · Base lida: `spike/src/{main,grid,model}.rs`, `src/App.svelte`, `src/lib/{CommandPalette,Grid,StatusBar,ContextToolbar}.svelte`, `/planejamento/gpui.md` (§8.0) e o código-fonte de `gpui-pre 0.3.12`, `gpui-component 0.9.1`, `gpui-base 0.9.1`, `gpui-kit 0.9.1` em `~/.cargo/registry`.

> **Status 2026-11-10 — implementado no protótipo (`spike/`):** K0 (registro único `spike/src/keys.rs` + `spike/assets/keymap-default.json` no formato do Zed + `~/.config/wadawko/keymap.json` com recarga automática por mtime + `settings.json` com `vim`), K1 (paleta Ctrl+K/Ctrl+P própria, fuzzy sem acento, atalho de cada comando, ✓ nos toggles, músicas na lista, chords `Ctrl+K Ctrl+T/V/L/M`), K2 (which-key via `observe_pending_input` + `possible_bindings_for_input`, 250 ms, grupos com título; showcmd na status bar), K3 (Ctrl+Space / Alt+F / `space j` = UI + inícios de tempo; `f` = células da linha; `F` = todas as células; rótulos balanceados ≤ 2 letras) e um vim-lite de K4 (normal/insert/visual/V-compasso, hjkl w b e W B 0 $ _ ( ) gg G gj gk Ctrl+D/U, contagem, x X dd dw d$ D dam dis cc cw s yy yw yis Y p P, gu gU !, i a I A, v V o, u Ctrl+R, Enter toca do cursor, zi zo z0 za zR zM, leader `space …` da §8.5). Vim desligado = modo simples (letras digitam, Shift+setas selecionam, Ctrl+C/X/V, Ctrl+Z/Y). Testes: rótulos (prefix-free, ≤2 letras até 676 alvos), fuzzy, todo binding padrão tem título. **Depois (mesmo dia):** registradores nomeados (`"a`–`"z`, `"0` último yank, `""`, `"+` área de transferência em TSV), repetição `.` (inclui o texto digitado no insert), marcas `m{x}`/`'{x}`, busca `/` `?` `n` `N` (Ctrl+F no modo simples), linha de comando `:` (`:12`, `:s 2`, `:bpm`, `:sub`, `:set [no]vim|dark|loop|metro|swing`, `:e título`, `:new`, `:reg`, `:marks`, `:u`, `:redo`, `:q`, ou nome de qualquer comando da paleta; ↑/↓ histórico), **Ctrl+/** = dica de atalhos na raiz (tudo o que dá para apertar no contexto atual), página **Configurações** (Ctrl+, / `space ,` / F1: vim, recarregar/criar keymap.json, testar tecla via `observe_keystrokes`, conflitos, mapa completo de atalhos por contexto) e verificador de conflitos com teste garantindo que o keymap padrão não tem nenhum. **Ainda não:** `EntityInputHandler` (IME/compose), objetos `i`/`a` genéricos além de `dam`/`dis`/`yis`.

---

## 0. Decisões em uma página

| Tema | Decisão |
|---|---|
| Fonte única da verdade | Um **registro de ações** (`keys::registry`) em Rust: cada comando é uma GPUI `Action` + um `ActionSpec { id, título pt-BR, grupo, palavras-chave, contexto, enabled }`. Paleta, keymap, which-key, hints, tooltips e `docs/ATALHOS.md` leem desse registro. Botões da UI despacham a mesma ação que o atalho. |
| Vim × GPUI | **Keymap do GPUI com contextos dinâmicos** (`Grid vim_mode=normal vim_operator=d …`), como o Zed faz. Um `VimState` pequeno guarda contagem, operador, registrador e "última mudança". **Não** escrever um interpretador próprio em `on_key_down`. Exceções pontuais (nome de registrador, texto de busca, rótulos de hint) usam `on_key_down` em contextos sem bindings de letras. |
| Chords | Multi-stroke nativo do GPUI (`"space t m"`, `"ctrl-k ctrl-s"`, `"g g"`). Which-key = `observe_pending_input` + `possible_bindings_for_input`. Timeout de 1 s do GPUI é fixo; o popup aparece aos 250 ms. |
| `Ctrl+K` | Abre a paleta **imediatamente**; as continuações `Ctrl+K Ctrl+S` etc. são bindings no contexto `Command` (dentro da paleta). Evita o atraso de 1 s que o GPUI impõe quando um prefixo também tem binding próprio. |
| Paleta | `gpui_component::command::{Command, CommandState}` com `filterable(false)` + fuzzy próprio (`keys::fuzzy`, sem dependência, com *folding* de acentos). |
| Hints (`Ctrl+Espaço`) | Camada `deferred(canvas)` **sem cache**, irmã do overlay do playhead. Elementos registram bounds por `on_children_prepainted`; células vêm da aritmética de `Geom`/`measure_xs` (sem elementos). Rótulos = código livre de prefixo, home row primeiro, mais curtos para os alvos mais próximos do cursor. |
| Keymap do usuário | `$XDG_CONFIG_HOME/wadawko/keymap.json` no formato do Zed (`[{context, bindings}]`), com `<leader>`, `null` para desligar, hot-reload via `notify`, relatório de conflitos. O keymap **padrão** é o mesmo formato embutido (`assets/keymap-default.json`). |
| Vim on/off | `settings.json: { "vim": true|false }` + comando na paleta + chip na status bar. Vim off = "modo simples": letras digitam direto (comportamento atual do spike/web). O contexto `vim=on|off` seleciona os blocos do keymap. |
| Linux | `Ctrl+Espaço` costuma ser capturado por fcitx5/ibus. Binding secundário `alt-f` ligado por padrão; em modo normal do Vim, `f`/`F` saltam por células. Nenhum binding padrão usa teclas mortas do ABNT2 (`~ ^ \` ´ ¨`). |

---

## 1. O que o GPUI 0.3.12 já oferece — verificado vs. assumido

### 1.1 Verificado no código-fonte (caminhos em `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`)

**Ações**
- `actions!(ns, [A, B])` e `#[derive(Action)] #[action(namespace = …, name = "…", no_json, no_register, deprecated_aliases = […])]`; comentários `///` viram `Action::documentation()` — `gpui-pre-0.3.12/src/action.rs:25-174`.
- Registro global por `inventory` (`action.rs:283`); `cx.all_action_names()` (`app.rs:2794`) e `cx.build_action(name, Some(json))` (`app.rs:2784`) permitem **enumerar e construir ações por nome** — base da paleta e do keymap em arquivo.
- Ações com parâmetros precisam de `serde::Deserialize` + `schemars::JsonSchema` (o derive emite `gpui::private::{serde_json, schemars}`; `gpui-pre-macros-0.3.12/src/derive_action.rs:123-194`). **Pegadinha**: o derive gera caminhos `gpui::…`; com `gpui-kit` é preciso `extern crate gpui_kit as gpui;` na raiz do crate (o próprio `gpui-kit-0.9.1/src/lib.rs` documenta esse alias). `schemars 1.2.3`, `serde 1.0.229` e `inventory 0.3.25` já estão no registry local (dependências transitivas do gpui).
- `gpui_kit::actions!` existe como fachada (`gpui-kit-0.9.1/src/lib.rs:54`).

**Keymap e contextos**
- `KeyBinding::new(&'static str, action, Some("ctx"))` e `KeyBinding::load(keystrokes: &str, Box<dyn Action>, Option<Rc<KeyBindingContextPredicate>>, use_key_equivalents, action_input, cx)` para strings dinâmicas (`keymap/binding.rs:35,51`); `KeyBinding: Clone`, com `with_meta/meta/keystrokes/action/predicate/action_input`.
- Gramática do keystroke: `[ctrl-][alt-][shift-][cmd-][fn-]key[->key_char]`, `secondary-` resolve por SO, letra maiúscula isolada = `shift-` + minúscula, modificador sozinho é tecla (`platform/keystroke.rs:118-230`). Sequências separadas por espaço: `"space w w"`, `"ctrl-k left"` (`key_dispatch.rs:50`, testes em `keymap.rs:473-565`).
- Predicados de contexto: identificadores, `==`, `!=`, `&&`, `||`, `!`, `>` (descendente) e parênteses (`keymap/context.rs:247-415`); o parser aceita até `> < ~ " ?` como identificadores "de operador vim" (`context.rs:502`). `KeyContext::parse("Grid vim_mode=normal")`, `.set(k, v)`, `.add(id)`.
- `div().key_context("…")`, `.track_focus(&h)`, `.on_action(cx.listener(f))`, `.capture_action`, `.on_boxed_action`, `.on_key_down`, `.on_modifiers_changed`, `.tab_index/.tab_stop` (`elements/div.rs:812-860, 1139-1229`). `cx.on_action::<A>` global (`app.rs:2751`).
- Precedência: contexto mais profundo vence; entre iguais, o binding adicionado por último; `null` → `NoAction` e `Unbind("ns::Action")` desligam bindings; `KeyBindingMetaIndex` (USER 0 > VIM 1 > BASE 2 > DEFAULT 3) decide quem um `null` pode suprimir (`keymap.rs:165-271`, testes `test_disable_weaker_sources_only`).
- `cx.bind_keys(iter)`, `cx.clear_key_bindings()`, `cx.key_bindings() -> Rc<RefCell<Keymap>>` (`app.rs:2732-2745`). **`Keymap` não tem remoção individual** (só `clear`) — relevante para o hot-reload (§7).
- `Window::bindings_for_action(_in | _in_context)`, `highest_precedence_binding_for_action*`, `keystroke_text_for(action)`, `available_actions`, `is_action_available_in(action, &focus_handle)` (`window.rs:3590, 5851, 7033-7110`).

**Multi-stroke / pending input (chords)**
- `DispatchTree::dispatch_key`: se a entrada é prefixo de algum binding habilitado no contexto, fica **pendente** e a tecla **não chega aos `on_key_down`** (`window.rs:6418-6437`: `cx.propagate_event = false; return`). Sem match, cai nos key listeners (`finish_dispatch_key_event`, `window.rs:6500`).
- `PENDING_INPUT_TIMEOUT = 1 s`, `pub(crate)` — **não configurável** (`window.rs:1431`). Só há timeout se o prefixo também tem binding completo (`pending_has_binding`), se um input de texto aceita a tecla, ou se é um modificador sozinho (`window.rs:6410-6425`). Prefixo puro (ex.: `space` em modo normal sem binding isolado) **espera indefinidamente**. No timeout, o prefixo é "re-tocado" como `KeyDownEvent { prefer_character_input: true }` (`window.rs:6716`) e o binding mais curto dispara (`test_pending_input_timeout_dispatches_shorter_binding`).
- `Window::pending_input() -> Option<PendingInputStatus { keystrokes(), timeout() }>` (`window.rs:6621`), `has_pending_keystrokes`, `pending_input_keystrokes` (`6604, 6711`), `set_pending_input_timeout_paused(&Entity<T>, bool, cx)` (`6638`).
- `Context::observe_pending_input(window, |this, window, cx| …)` (`app/context.rs:503`) — chamado a cada mudança do pendente (inclusive em troca de foco/blur).
- `Window::possible_bindings_for_input(&[Keystroke]) -> Vec<KeyBinding>` (`window.rs:~7115`), apoiado em `Keymap::possible_next_bindings_for_input`, que já exclui `null`/`Unbind` e respeita contexto (`keymap.rs:285-337`). **É a fonte de dados do which-key.**
- `cx.observe_keystrokes` (depois do dispatch, com a ação resolvida) e `cx.intercept_keystrokes` (antes; `stop_propagation` cancela tudo) (`app.rs:2705-2727`).

**Texto, IME, Linux**
- `EntityInputHandler` com `accepts_text_input`, `replace_text_in_range`, etc. (`input.rs:26-127`), `ElementInputHandler` (`:152`), `window.handle_input(…)` (`window.rs:5641`). Texto só ganha de bindings quando `prefer_character_input && handler.accepts_text_input()` (`window.rs:6274`).
- Keystroke carrega `key` (tecla física/ASCII) e `key_char` (o que seria digitado); `should_match` casa binding tanto pela tecla quanto pelo `key_char` (`keystroke.rs:82`). O spike já usa `key_char` para digitar sílabas (`spike/src/main.rs:269`).
- Wayland: cliente com `xkb compose` e `text-input` (preedit `ime_pre_edit`, `gpui-pre-linux-0.3.12/src/linux/wayland/client.rs:375-390, 2092-2342`); X11 usa `xim` (`x11/client.rs:39`). Ou seja, dead keys do ABNT2 passam por compose, e um IME ativo (fcitx5/ibus) pode receber as teclas.
- Clipboard: `cx.write_to_clipboard / read_from_clipboard(ClipboardItem)` (`app.rs:1834-1882`).

**Elementos/overlays**
- `Div::on_children_prepainted(Fn(Vec<Bounds<Pixels>>, &mut Window, &mut App))` (`elements/div.rs:1866`) — captura bounds por frame. `canvas(prepaint, paint)` (o spike já captura `Geom` assim, `grid.rs:733`). `deferred(child).with_priority(n)` (`elements/deferred.rs:7,25`) e `anchored()` (`anchored.rs:39`); gpui-base usa `POPUP_PRIORITY = 100` (`gpui-base-0.9.1/src/popup.rs:16`).
- `Entity::cached(style)` validado no spike (`main.rs:743`) com overlay irmão sem cache (`main.rs:760-784`).

**gpui-component 0.9.1 / gpui-base**
- **Paleta pronta**: `command::{Command, CommandState, CommandItem, CommandGroup, CommandEntry}` (`gpui-component-0.9.1/src/command/`). `CommandItem::label/keywords/icon/action(Box<dyn Action>)/checked/disabled/child(builder)`; filtro **substring** (não fuzzy; `item.rs::matches`); `Command::filterable(false)` + `on_query` para filtro externo; ao confirmar, faz `window.dispatch_action(action, cx)` (`state.rs:630`) e mostra o `Kbd` do binding; `escape/enter/up/down` ligados no contexto `Command` por `command::init` (`state.rs:66`), chamado em `gpui_component::init` (`lib.rs:144`) ← `gpui_kit::init`.
- `kbd::Kbd::{new, binding_for_action, binding_for_action_in, global_binding_for_action, format}` (`kbd.rs`); `WindowExt::{open_dialog, close_dialog, open_sheet, push_notification}` (`window_ext.rs`); `status_bar::StatusBar::new().left(..).right(..)`; `gpui_base::actions::{Cancel, Confirm{secondary}, SelectUp/Down/First/Last…}` (namespace `ui`).
- `gpui_base::Root::register_plugin::<P: RootPlugin>` (`root.rs:53-120`): plugins renderizados **acima** do conteúdo, em ordem de registro — hospedeiro natural para hints e which-key, alternativa ao overlay no Shell.
- Testes: `gpui-kit` tem feature `test-support` que propaga para gpui/base/component (`gpui-kit-0.9.1/Cargo.toml:76`), `#[gpui_kit::test]`, `TestAppContext::{simulate_keystrokes("ctrl-k"), simulate_input, dispatch_action, dispatch_keystroke}` (`app/test_context.rs:605-660`), `VisualTestContext::simulate_keystrokes` (`visual_test_context.rs:225`), `cx.executor().advance_clock(dur)` (usado nos testes do próprio GPUI para o timeout), `gpui_kit::test::{TestWindowExt::find/click/find_all, TestQueryExt, TestInput}` com `ElementSnapshot{bounds, focused, …}` (`gpui-kit-0.9.1/src/test.rs`).
- Nenhum fuzzy matcher em gpui-component/base (grep). `nucleo-matcher 0.3.1` e `notify 8.2.2` já estão no registry local (presença verificada; API não inspecionada).

### 1.2 Assumido (confirmar nas primeiras tarefas)

1. **Contexto fresco antes do dispatch**: `dispatch_key_event` redesenha se a janela está suja (`window.rs:~6290`), então mudar `vim_mode` + `cx.notify()` numa ação garante que a tecla seguinte veja o novo contexto. (A linha do redraw foi vista; que `notify` marca a janela suja é o comportamento padrão do GPUI.)
2. **Views em cache não re-executam prepaint** enquanto não notificadas → alvos de hint dentro da grade cacheada registram-se só quando a grade re-renderiza. Mitigação: registro por id estável + `notify()` ao entrar em modo hint + sessão de hint termina em scroll/clique.
3. **Paleta despacha para o foco anterior**: `Window::dispatch_action` lê `self.focused()` na hora e difere o dispatch (`window.rs:2761`); fazendo `prev_focus.focus(window)` antes, a ação encontra o nó da grade no frame renderizado. Validar na K1.
4. **Dead keys no ABNT2**: sob xkb compose, `~`/`^`/`` ` ``/`´`/`¨` não produzem `key` utilizável para bindings — por isso nenhum binding padrão as usa. Testar em layout `br`.
5. **`Ctrl+Espaço` no Linux**: fcitx5 (gatilho padrão `Ctrl+Space`), ibus/GNOME (`Super+Space`) capturam no compositor; o app não consegue sobrepor. Validar na máquina do dev (CachyOS/Wayland) e documentar.
6. APIs de `nucleo-matcher`/`notify` (padrões conhecidos). O plano recomenda fuzzy próprio, então `nucleo` é opcional.
7. `gpui_kit::init(cx)` é obrigatório antes de usar `Command` (verificada a cadeia de `init`; o resto do que `init` faz não foi inspecionado).

### 1.3 Pegadinhas que moldam o desenho

- Prefixo com binding completo **atrasa 1 s** o binding curto (`ctrl-k` + `ctrl-k ctrl-s`). Logo: nunca dar binding isolado a um prefixo de chord no mesmo contexto — ou resolver como no §4 (`Ctrl+K` abre a paleta e as continuações vivem no contexto `Command`).
- Tecla pendente **não chega** ao `on_key_down`: no modo insert não pode existir binding de letra isolada nem chord iniciado por letra no contexto ativo — garantido pelos predicados `vim_mode == normal`.
- Prefixo expirado é re-tocado ao `on_key_down` com `prefer_character_input: true`: o handler de teclas da grade deve ignorar letras quando `vim_mode != insert`.
- `KeyBinding::new` exige `&'static str`; tudo que vem de arquivo usa `KeyBinding::load`.

---

## 2. Arquitetura

### 2.1 Registro de ações — a fonte única

```rust
// wadawko-app/src/keys/registry.rs
pub enum Group { Transporte, Ver, Navegar, Edicao, Selecao, Estrutura, Musica, Teclado, Config }

pub struct ActionSpec {
    pub id: &'static str,                 // "transport::TogglePlay" == Action::name()
    pub title: &'static str,              // "Tocar / parar"
    pub group: Group,
    pub keywords: &'static [&'static str],// ["play", "pause", "espaço"]
    pub context: &'static str,            // "Workspace" | "Grid" | "Grid && vim_mode == visual" …
    pub build: fn() -> Box<dyn Action>,   // ação sem parâmetros (ou com default) para paleta/hints
    pub enabled: fn(&Snapshot) -> bool,   // predicado sobre um snapshot imutável do estado
    pub dynamic_title: Option<fn(&Snapshot) -> String>, // "Loop: desligar" / "Loop: ligar"
}
pub static ACTIONS: &[ActionSpec] = &[ … ];
pub static PREFIX_TITLES: &[(&str, &str)] = &[("space t", "transporte"), ("space s", "seção"), ("g", "ir"), ("z", "zoom/dobrar"), ("ctrl-k", "paleta")];
```

- `Snapshot` = cópia barata do estado relevante (`playing, loop, metronome, page, view, vim_mode, has_cursor, has_selection, section_count, …`) montada pelo Shell a cada render.
- **Invariantes testadas** (`#[test]`): todo nome em `cx.all_action_names()` dos namespaces `transport|view|nav|grid|vim|section|track|project|keys|hints|config` tem `ActionSpec`; não há `id` duplicado; todo `context` parseia com `KeyBindingContextPredicate::parse`; todo binding do keymap padrão aponta para um `id` existente.
- Os helpers `btn/toggle/segmented/small_btn` do spike (`main.rs:364-429`, `grid.rs:408`) passam a receber `Box<dyn Action>` e fazem `on_click → window.dispatch_action(a)`; isso faz clique, atalho, paleta e hint convergirem na mesma ação (e registra o alvo do hint, §6).
- Ações com parâmetro (`vim::Count(u8)`, `vim::PushOperator(Op)`, `project::Select(usize)`, `track::Mute(usize)`, `section::Collapse(usize)`, `view::Set(View)`, `nav::Go(Page)`) usam `#[derive(Action, Deserialize, JsonSchema)]` para o keymap em JSON poder construí-las.

### 2.2 Vim × dispatch do GPUI — comparação e recomendação

| Opção | Como | Prós | Contras |
|---|---|---|---|
| **A. Keymap GPUI + contextos dinâmicos + `VimState` pequeno (recomendado; é o modelo do `vim` do Zed)** | `GridView::render` emite `key_context("Grid vim=on vim_mode=normal vim_operator=none vim_count=none")`; bindings com predicados; ações `vim::*` mutam `VimState` e chamam `cx.notify()` | Remapeável pelo usuário; which-key e hints "de graça" (`possible_bindings_for_input`); testável com `simulate_keystrokes`; paleta lista as ações vim; precedência por contexto já resolvida | Precisa manter o contexto em sincronia com o estado (1 função `vim_key_context(&VimState)`); contagem exige 10 bindings de dígito; nomes de registrador exigem fallback |
| B. Interpretador próprio em `on_key_down` | Máquina de estados consome `KeyDownEvent` cru em modo normal | Independente da API de keymap do GPUI | Não remapeável por arquivo; sem which-key; duplica pending-input; conflita com bindings globais (que rodam antes); teclas prefixo do keymap global nunca chegam |
| C. Híbrido A + fallback | A, mais `on_key_down` **apenas** quando `vim_operator ∈ {register, search}` ou no modo hint | Cobre `"a`, `/don`, rótulos | Dois caminhos (mas disjuntos por contexto) |

**Recomendação: C** (A como regra; `on_key_down` só onde a próxima tecla é "texto livre"). Estado:

```rust
pub struct VimState {
    pub mode: VimMode,                 // Normal | Insert | Visual(VisualKind) | Replace1
    pub count: Option<u32>,            // acumulado por vim::Count(n)
    pub operator: Option<Operator>,    // Delete | Yank | Change | Shift(dir) | Case(mode)
    pub pending: Pending,              // None | Register | ObjectInner | ObjectAll | Search | Mark | GotoMark
    pub register: Option<char>,
    pub last_change: Option<Change>,   // para `.`
    pub search: Option<String>,        // para n/N
    pub marks: HashMap<char, Cur>,
}
fn vim_key_context(s: &VimState, vim_on: bool, editing: bool) -> KeyContext // "Grid vim=on vim_mode=normal vim_operator=d vim_count=some vim_pending=none"
```

Fluxo de `d2w`: `d` → `vim::PushOperator(delete)` (contexto passa a `vim_operator=d`) → `2` → `vim::Count(2)` → `w` → `vim::Motion(NextBeat)`; o handler de `Motion` vê `operator = Delete` e `count = 2`, calcula o intervalo `[cursor, destino)`, aplica `ops::clear_cells`, grava em registrador e em `last_change`, zera operador/contagem. Objetos: `i`/`a` só têm binding em `vim_operator != none` → `vim::ObjectScope(inner|all)` → próxima letra `vim::Object(beat|measure|…)`.

### 2.3 Pilha de contextos e foco

```
Workspace vim=on page=editor          (div raiz: track_focus + on_action de tudo que é global)
└─ Grid vim_mode=… vim_operator=…     (GridView: track_focus + on_action das ações grid::/vim::)
└─ SongList                           (painel de músicas; navegável por j/k ou setas)
└─ Command                            (paleta do gpui-component, quando aberta)
└─ Hints                              (overlay de rótulos, quando ativo)
```
- Ações globais (`transport::*`, `view::*`, `nav::*`, `project::*`, `keys::*`, `config::*`) são registradas no `div` raiz; por isso funcionam com o foco em qualquer lugar e o dispatch da paleta (que percorre raiz → foco) as encontra.
- Ações `grid::*`/`vim::*` ficam no `GridView`. A paleta, antes de despachar, restaura o foco anterior (`prev_focus.focus(window)`) — ver §1.2 item 3. `enabled` também consulta `window.is_action_available_in(&a, &grid_focus)`.

### 2.4 Overlays — onde vivem hints, which-key e paleta

No `Shell` (hoje `Spike` em `main.rs`), ao lado do overlay do playhead (`main.rs:760-801`):

```
editor (relative)
├─ scroll(content: tabs, toolbar, grid.cached(...))
├─ playhead: canvas(..).absolute().inset_0()           // já existe, sem cache
├─ hints:   deferred(canvas(..)).with_priority(200)     // §6 — só quando hints.active
├─ whichkey: deferred(div popup).with_priority(150)     // §5 — só quando há pendente
└─ palette: WindowExt::open_dialog / Dialog com Command // §4
```
`deferred` garante que pintamos depois de toda a árvore (os `on_children_prepainted` já rodaram no mesmo frame). Alternativa equivalente: três `RootPlugin`s do gpui-base (§1.1); escolher no K0 — overlay no Shell é mais simples e já foi validado pelo playhead.

### 2.5 Módulos (em `wadawko-app/src/`, prototipáveis em `spike/src/`)

```
keys/
  mod.rs          init(cx): registra defaults + usuário, inicia watcher
  registry.rs     ActionSpec, ACTIONS, PREFIX_TITLES, Group, Snapshot
  actions.rs      actions!(transport, …) + structs com parâmetros
  keymap_file.rs  formato JSON, <leader>, load/validate/conflicts (puro, testável)
  loader.rs       KeyBinding::load, metas, reload sem perder bindings do gpui-component
  settings.rs     settings.json (vim, leader, hint_alphabet, which_key_delay_ms, jump_secondary)
  fuzzy.rs        score(query, text) (puro)
  palette.rs      CommandPalette (Entity) sobre gpui_component::command
  whichkey.rs     WhichKey (observe_pending_input, timer, popup)
  hints/
    mod.rs        HintSession, HintRegistry (Global), HintTarget, HintAct
    labels.rs     labels(n, alphabet) (puro)
    overlay.rs    deferred canvas + input
vim/
  mod.rs          VimState, VimMode, vim_key_context
  motions.rs      Motion enum + resolve(song, cur, motion, count) -> Cur   (puro)
  objects.rs      TextObject + range(song, cur, obj, scope) -> Range        (puro)
  ops.rs          aplica operador sobre Range: clear/yank/change/shift/case (puro, sobre &mut Song)
  registers.rs    Registers (unnamed, "0, a-z, +) + TSV <-> CellBlock
  repeat.rs       Change (para `.`)
  cmdline.rs      `:` (K4, opcional)
```

---

## 3. Modo Vim na grade

### 3.1 Modos

| Modo | Entrada | Saída | Contexto |
|---|---|---|---|
| **Normal** | padrão com vim on; `escape` de qualquer modo | — | `vim_mode=normal` |
| **Insert** | `i` (na célula), `a` (próxima célula), `I`/`A` (início/fim do compasso), `o`/`O` (novo compasso depois/antes + insert), `c…`, `s`, `r` (uma célula) | `escape` (confirma buffer e volta), `enter`/`space`/`tab` confirmam e avançam **mantendo** insert (fluxo de digitação atual) | `vim_mode=insert editing=true|false` |
| **Visual** `v` | retângulo de células a partir do cursor; motions estendem | `escape`, ou após operador | `vim_mode=visual` |
| **Visual-compasso** `V` | compassos inteiros na linha atual | idem | `vim_mode=visual_line` |
| **Visual-bloco** `ctrl-v` | compassos inteiros em todas as linhas (coluna) | idem | `vim_mode=visual_block` |
| **Replace-1** `r` | insert de uma célula; ao confirmar volta ao normal | — | `vim_mode=insert vim_once=true` |

### 3.2 Coordenadas: o que é "linha", "palavra", "caractere"

Cursor `Cur { m, t, k }` (`grid.rs:62`): `m` compasso global, `t` linha/instrumento, `k` célula no compasso. Eixo horizontal = tempo, vertical = instrumentos.

- **caractere** = célula (`h`/`l`, atravessa compassos como `move_cursor` já faz).
- **palavra** = **tempo** (beat; `k % subdivision == 0`). Justificativa: a subdivisão (colcheia/semicolcheia/fusa) é a granularidade fina e muda por música; o tempo é a unidade musical estável e o que a régua `1 e 2 e` mostra. `W`/`B`/`E` = compasso (a "PALAVRA" grande).
- **linha** = **o compasso atual na linha atual** (`0`/`$`/`dd`/`yy`/`cc`/`V`). Justificativa: cada seção é um canvas por si e o compasso é o bloco que o usuário copia/cola/limpa; `dd` apagar "a linha inteira da música" seria destrutivo demais.
- **frase/sentença** = seção (`(`/`)`), **parágrafo** = música (`gg`/`G`).
- vertical: `j`/`k` = instrumento ±1; `gj`/`gk` = último/primeiro instrumento.

### 3.3 Motions (normal e visual; aceitam contagem)

| Tecla | Ação | Descrição |
|---|---|---|
| `h` `l` | `vim::Left/Right` | célula ±1 (cruza compassos e seções) |
| `j` `k` | `vim::Down/Up` | instrumento ±1 |
| `w` `b` | `vim::NextBeat/PrevBeat` | início do próximo/anterior tempo |
| `e` | `vim::EndOfBeat` | última célula do tempo |
| `W` `B` `E` | `vim::NextMeasure/PrevMeasure/EndOfMeasure` | compasso |
| `0` `$` | `vim::StartOfMeasure/EndOfMeasure` | primeira/última célula do compasso (`0` só sem contagem pendente: contexto `vim_count == none`) |
| `_` | `vim::FirstFilledInMeasure` | primeira célula preenchida (substitui `^`, tecla morta no ABNT2) |
| `(` `)` | `vim::PrevSection/NextSection` | início da seção anterior/próxima |
| `gg` `G` | `vim::StartOfSong/EndOfSong` | `{n}G` = compasso *n* |
| `gk` `gj` | `vim::FirstTrack/LastTrack` | |
| `n` `N` | `vim::SearchNext/Prev` | próxima/anterior célula que casa com a busca |
| `/` | `vim::StartSearch` | busca por sílaba (texto livre via `on_key_down`, `enter` confirma) |
| `ctrl-d` `ctrl-u` | `vim::HalfPageRight/Left` | meia viewport no tempo |
| `zz` `zt` `zb` | `vim::ScrollCenter/Left/Right` | reposiciona a viewport (`offset_target`) |
| `'a` | `vim::GotoMark(a)` | marca (`ma` define) — K4 opcional |

### 3.4 Operadores, objetos, visual, contagem, registradores, repetição

**Operadores** (`vim::PushOperator(op)`): `d` apaga (limpa células → registrador), `y` copia, `c` apaga + insert na primeira célula, `>`/`<` desloca no tempo (1 célula × contagem; `>>` desloca o compasso), `gu`/`gU` tira/põe acento (minúscula/MAIÚSCULA — `don`↔`DON`, sem `~`), `!` alterna sfz (sem pendência: age na célula ou na seleção, como a web). Atalhos: `x`=`dl`, `X`=`dh`, `s`=`cl`, `D`=`d$`, `C`=`c$`, `Y`=`yy`, `dd`/`yy`/`cc` = compasso na linha. `p`/`P` colam depois/no cursor (bloco do registrador, respeitando a forma; em visual, substitui). `o`/`O` = novo compasso depois/antes (na seção) + insert. `u`/`ctrl-r` desfazer/refazer (`History<Arc<Song>>` do plano principal). `.` repete a última mudança.

**Objetos** (só com operador pendente ou em visual): prefixo `i` = na linha atual, `a` = **em todas as linhas** ("a" de *all*; é a leitura natural numa grade 2D): `c` célula, `b` tempo, `m` compasso, `s` seção, `t` linha inteira (todas as seções), `w` sequência contígua de células preenchidas. Ex.: `dam` apaga o compasso em todas as linhas (coluna), `yis` copia a seção na linha atual, `ciw` reescreve a frase sob o cursor.

**Visual**: `v`/`V`/`ctrl-v` como na §3.1; `o` troca o canto-âncora; `gv` reseleciona; operadores acima agem na seleção; **setas movem o bloco** (paridade com `Grid.svelte:643-653`), `alt-setas` clonam (paridade com `altKey` da web), `escape` sai. `:` abre a paleta filtrada por comandos de seleção.

**Contagem**: `1-9` → `vim::Count(n)` em normal/visual/operador-pendente; `0` acumula só com `vim_count == some`. `3l`, `2w`, `d2w`, `3dd`, `5j`, `2>>`.

**Registradores**: `"` → `vim::PushRegister` (contexto `vim_pending=register`; a letra seguinte é lida no `on_key_down`): `"a`–`"z`, `"0` último yank, `""` sem nome, `"+`/`"*` área de transferência do sistema. Conteúdo = `CellBlock { rows: Vec<Vec<Option<String>>> }`; no clipboard do sistema vai como **TSV** (linhas = instrumentos, colunas = células), legível por planilha e pelo app web.

**`.`**: `Change { operator, count, target: Motion|Object|Visual(shape), inserted: Vec<String>, register }`; repetir = reaplicar sobre o cursor atual (para insert, recoloca as sílabas gravadas).

### 3.5 Insert e a digitação que já existe; IME

- O modo insert **é** o comportamento atual do spike (`main.rs:232-283`): letras e `!` vão para `GridView.edit`; `space`/`tab`/`enter` confirmam e avançam; `shift-tab` confirma e recua; `backspace` apaga um char, e com buffer vazio recua uma célula; `delete` apaga a célula; setas confirmam e navegam; `ctrl-u` limpa o buffer; `escape` confirma e volta ao normal. A paleta flutuante de sílabas continua ancorada à célula.
- Implementar `EntityInputHandler` para `GridView` com `accepts_text_input = (mode == Insert)`: texto commitado por IME/compose entra em `edit` via `replace_text_in_range`; em normal/visual devolve `false` e os bindings vencem (`window.rs:6274`). Custo ~80 linhas; resolve dead keys e IMEs ativos sem gambiarra. Kuchi shōga é romaji, logo **não** precisamos de preedit visível.
- Regra: em insert não existe binding de letra sem modificador (nem prefixo começando por letra); os chords globais (`ctrl-k …`, `ctrl-space`) continuam funcionando.

### 3.6 Vim desligado — "modo simples" (padrão para quem não usa Vim)

Contexto `Grid && vim == off`: letras digitam direto (insert implícito, como hoje), setas navegam, `shift-setas` estendem a seleção (novo), `space` toca/para quando não está editando, `enter`/`tab` confirmam, `escape` cancela a edição/seleção, `delete`/`backspace` apagam, `ctrl-c/x/v` copiam/recortam/colam (TSV), `!` alterna sfz na seleção, `ctrl-z`/`ctrl-shift-z`/`ctrl-y` desfazem/refazem, `home`/`end` início/fim do compasso, `ctrl-home`/`ctrl-end` início/fim da música, `page up/down` instrumento ±. "Modo selecionar" da toolbar vira `ctrl-shift-s` e continua existindo. Tudo isso são ações do mesmo registro, apenas com bindings diferentes.

### 3.7 Indicador de modo na status bar

Status bar (`main.rs:914-938`) com `StatusBar::new().left(...).right(...)`:
- esquerda: chip de modo — `NORMAL` (ink), `INSERIR` (accent vermelho), `VISUAL`/`V-COMPASSO`/`V-BLOCO` (accent2 azul), `SIMPLES` quando vim off (clicável = alterna vim); depois o **showcmd**: teclas pendentes e contagem/operador (`"a 3 d`, `space t`), em `ink_faint`; depois posição `S2 · c5 · 3.2 · Chū` (seção, compasso, tempo.subdivisão, instrumento).
- centro: `120 BPM · tocando: música inteira` (como hoje).
- direita: HUD, fonte do banco, créditos.
- Enquanto a última tecla de um chord não chega, o showcmd pulsa discretamente; que `escape` limpa.

---

## 4. Paleta de comandos `Ctrl+K`

- `keys::palette::CommandPalette` (Entity) envolvendo `gpui_component::command::Command` + `CommandState`, aberta via `WindowExt::open_dialog` (ou `Dialog` próprio) com largura 34 rem e `max_h` 50 % da janela — mesma geometria do `CommandPalette.svelte`.
- Itens: para cada `ActionSpec` com `enabled(&snapshot)`: `CommandItem::new().label(título dinâmico).keywords(…).action(spec.build())`; grupos = `Group` (`CommandGroup::label`). O `Kbd` do binding aparece sozinho (a lib resolve por `highest_precedence_binding_for_action`); como as ações da grade só têm binding no contexto `Grid`, passar `child(...)` custom com `Kbd::binding_for_action_in(&a, &grid_focus, window)` para essas.
- Busca: `Command::filterable(false)` + `on_query` → `fuzzy::score` próprio (subsequência com bônus de início de palavra/consecutivo, penalidade de distância, *folding* `ção→cao`, `ô→o`, case-insensitive) → reordena os itens; `>` como primeiro caractere limita a comandos, `:` entra no modo linha de comando (K4), `?`... ("Atalhos").
- Confirmar: `on_confirm` fecha o diálogo, `prev_focus.focus(window)`, e o `Command` já despachou a ação (`state.rs:630`). Verificar ordem na K1; se preciso, não usar `.action()` e despachar nós mesmos após restaurar o foco.
- `Ctrl+K` com a paleta aberta fecha (toggle, como a web). `ctrl-p` e `space /` (vim) são aliases.
- **Chords `Ctrl+K …`**: bindings no contexto `Command` (§5). O cabeçalho da paleta (`Command::header`) lista as continuações (`ctrl-s atalhos · ctrl-t tema · m metrônomo · …`), de modo que a paleta é o próprio *command hint* do prefixo `Ctrl+K`.

---

## 5. Chords e *command hint* (which-key)

**Mecânica (toda do GPUI)**: bindings `"space t m"`, `"g g"`, `"z a"`, `"ctrl-k ctrl-s"`. O leader é literal `space` nos blocos `vim_mode == normal|visual`; o arquivo aceita `<leader>` (substituído por `settings.leader` ao carregar). Prefixos puros não têm timeout (§1.1), então `space` em modo normal espera a próxima tecla; `escape` cancela (binding `escape → keys::CancelPending` em todo contexto; o GPUI já descarta o pendente ao trocar foco).

**Popup**:
1. `cx.observe_pending_input(window, …)` no Shell: lê `window.pending_input()`; se há keystrokes, inicia um timer de `which_key_delay_ms` (250 ms padrão; 0 = imediato) via `cx.spawn` + `background_executor().timer`.
2. Ao disparar, `window.possible_bindings_for_input(&pending)` → agrupa pelo keystroke seguinte: folha (restam 1) mostra `tecla  título` (título via `registry.by_id(binding.action().name())`); nó mostra `tecla  +nome do grupo (n)` com `PREFIX_TITLES`.
3. Render: `deferred(div)` colado acima da status bar, 2–4 colunas, `Kbd` para a tecla, `ink_soft` para títulos, `accent` para grupos; some quando `pending_input()` volta a `None`.
4. Se `pending_input().timeout().is_some()` (prefixo que também é binding, ex.: `ctrl-k` dentro da paleta não se aplica; `space` com vim off), mostramos uma barra de progresso de 1 s no popup para o usuário entender que o binding curto vai disparar. Não pausamos o timeout (comportamento esperado); `set_pending_input_timeout_paused` fica reservado para quando um diálogo nosso abrir no meio de um chord.
5. Status bar: showcmd mostra as teclas pendentes imediatamente (sem esperar os 250 ms).

**Prefixos padrão**: `space` (leader; vim), `g`, `z`, `d|y|c + i|a` (vim), `ctrl-k` (global, via paleta). Tabela completa no §8.

---

## 6. Jump por rótulos (`Ctrl+Espaço`, *element hints*)

### 6.1 Alvos e registro de bounds

```rust
pub struct HintTarget { pub id: HintId, pub bounds: Bounds<Pixels>, pub kind: HintKind, pub act: HintAct, pub title: SharedString, pub frame: u64 }
pub enum HintKind { Button, Tab, Nav, Card, SectionHeader, TrackRow, Cell, Chip }
pub enum HintAct { Action(Box<dyn Action>), Cell(Cur), Focus(FocusHandle) }
#[derive(Default)] pub struct HintRegistry { pub targets: HashMap<HintId, HintTarget> }  // cx Global
```
- Helper `hintable(el, id, kind, act, title, cx)`: se `HintSession::active(cx)`, envolve `el` em `div().on_children_prepainted(move |b, _w, cx| registry.upsert(id, b[0], …))`; senão devolve `el` sem custo. Como `btn/toggle/segmented/small_btn` já centralizam todos os botões do spike, a mudança é local.
- Ao ativar (`hints::Jump`): `prev_focus = window.focused(cx)`, `registry.clear()`, `grid.update(cx, |_, cx| cx.notify())` (força a grade cacheada a re-registrar), overlay recebe foco (`track_focus(&hint_focus)` + `key_context("Hints")`). Scroll, clique ou `escape` encerram a sessão (resolve a hipótese §1.2-2 sem depender do cache).

### 6.2 Células do canvas (não têm elementos)

`GridView::visible_cells(scope) -> Vec<(Cur, Bounds<Pixels>)>` reproduz o culling de `paint_section` (`grid.rs:873-912`): para cada seção com `geom.sections[s]` não vazio, para cada compasso visível (`measure_xs`, `offset`, `cw`), células `c0..c1`, filtradas por `scope`:
- `Scope::Beats` (padrão do `ctrl-space`): só `k % subdivision == 0`, todas as linhas (~8 × 15 = 120 alvos);
- `Scope::TrackCells` (`f` no vim): todas as células visíveis da linha atual (~60);
- `Scope::AllCells` (`F`): todas as células visíveis (~480; rótulos de 3 chars).
Os bounds vêm de `Geom` compartilhado (`Rc<RefCell<Geom>>`, `grid.rs:76`), exatamente como o playhead (`main.rs:288-306`). Chips da paleta flutuante e botões dos cabeçalhos/linhas entram pelo §6.1.

### 6.3 Geração de rótulos (puro, `hints/labels.rs`)

```rust
/// Código livre de prefixo: n rótulos sobre `alphabet` (ordem = preferência), mais curtos primeiro.
pub fn labels(n: usize, alphabet: &[char]) -> Vec<String> {
    let mut leaves: VecDeque<String> = alphabet.iter().map(|c| c.to_string()).collect();
    while leaves.len() < n {
        let p = leaves.pop_back().expect("alfabeto vazio");   // expande a folha MENOS preferida
        for c in alphabet { leaves.push_back(format!("{p}{c}")); }
    }
    leaves.into_iter().take(n).collect()                        // já ordenado: 1 char, depois 2, …
}
```
- Alfabeto padrão `asdfghjklqwertyuiopzxcvbnm` (home row primeiro; `;` fora por ser `shift` no ABNT2). Com 26 letras e 250 alvos: 16 rótulos de 1 letra + 10×26 de 2 letras.
- Propriedades testadas: únicos, nenhum é prefixo de outro, `len() == n`, só chars do alfabeto, tamanhos não decrescentes, determinístico.
- Atribuição por prioridade: alvos ordenados por distância ao cursor da grade (ou ao mouse se não há cursor), com empate por ordem de leitura; botões de transporte e nav têm bônus fixo. Assim as células vizinhas e "tocar" ganham uma letra só (estilo flash.nvim).
- Rótulos exibidos em maiúsculas, digitados em minúsculas; parte já digitada em `ink_faint`.

### 6.4 Interação e pintura

- `on_key_down` no overlay: letra do alfabeto → `typed.push`; se `typed == rótulo` → ativa; se nenhum rótulo começa com `typed` → cancela com flash; `backspace` remove; `escape` cancela. Rótulos não casados somem conforme se digita (filtro progressivo).
- Ativar: `Cell(c)` → `grid.cursor = Some(c)` (se veio do insert, continua em insert); `Action(a)` → restaura foco e `window.dispatch_action(a)`; `Focus(h)` → `h.focus(window)`; `Chip` → `grid.insert_syllable`.
- Pintura: `deferred(canvas(prepaint, paint)).with_priority(200)` (acima de `POPUP_PRIORITY = 100`): por alvo, quad arredondado `accent` (UI) ou `accent2` (células) no canto superior esquerdo do bound, clampado na janela, texto via `shape_line` em cache (mesmo `shaped` da grade: ≤ 26² strings). Células de tempo têm ≥ 2·cw ≥ 28 px entre si: cabe um rótulo de 2 chars. Camada sem cache; custa O(alvos) só durante a sessão.

### 6.5 `Ctrl+Espaço` no Linux

fcitx5 usa `Ctrl+Space` como gatilho padrão; ibus/GNOME, `Super+Space`; KDE usa `Alt+Space` para o KRunner. Decisão: padrão `ctrl-space` **e** `alt-f` ligados (ambos em `hints::Jump`), `f`/`F` em modo normal do Vim, e `space j` (leader). Página Config tem um campo "testar tecla" que mostra o `Keystroke` recebido (para o usuário descobrir se o compositor engoliu). Documentar como desligar o gatilho no fcitx5. Nenhum binding padrão usa `~ ^ \` ´ ¨` (teclas mortas no ABNT2) nem `;`/`:`/`?` em posições que exijam `shift` (`:` para linha de comando é exceção consciente, vim-compatível).

---

## 7. Keymap do usuário, hot-reload e conflitos

**Arquivo** `$XDG_CONFIG_HOME/wadawko/keymap.json` (padrão `~/.config/wadawko/keymap.json`), formato do Zed:

```json
[
  { "context": "Workspace",
    "bindings": { "ctrl-space": "hints::Jump", "alt-f": "hints::Jump", "ctrl-k": "keys::TogglePalette" } },
  { "context": "Grid && vim == on && vim_mode == normal && vim_operator == none",
    "bindings": {
      "d": ["vim::PushOperator", "delete"],
      "3": ["vim::Count", 3],
      "<leader> t m": "transport::ToggleMetronome",
      "f": ["hints::Jump", { "scope": "track_cells" }],
      "x": null
    } },
  { "context": "Command", "bindings": { "ctrl-s": "keys::OpenShortcuts" } }
]
```
- `string` → ação sem parâmetros; `[nome, json]` → `cx.build_action(nome, Some(json))`; `null` → `NoAction`; `["zed::Unbind", "ns::Acao"]` aceito (é o nome real da ação no GPUI). `<leader>` substituído por `settings.leader`.
- **`assets/keymap-default.json`** usa o mesmo formato (embutido com `include_str!`) e é a origem de `docs/ATALHOS.md` e das tabelas do §8. Carregado com `KeyBindingMetaIndex(2)`; o do usuário com `(0)`, por último (vence).
- **`settings.json`**: `{ "vim": false, "leader": "space", "hint_alphabet": "asdfghjklqwertyuiopzxcvbnm", "which_key_delay_ms": 250, "jump_secondary": "alt-f" }`. Editável pela página Config (hoje o slot inativo do NavRail) e pela paleta.
- **Hot-reload**: thread com `notify 8` observando o diretório (edições atômicas trocam o inode), debounce 200 ms, canal → `cx.spawn` → `reload(cx)`. Como `Keymap` só tem `clear`, o reload faz: `let keep: Vec<KeyBinding> = cx.key_bindings().borrow().bindings().filter(|b| b.meta() != Some(USER)).cloned().collect(); cx.clear_key_bindings(); cx.bind_keys(keep); cx.bind_keys(user)` — preserva os bindings do gpui-component (`Command`, `Input`). Erros viram `push_notification` com linha/coluna e o arquivo anterior continua valendo.
- **Conflitos** (`keymap_file::check(defaults, user, layout) -> Vec<Diagnostic>`, também em `wadawko-cli keymap check`):
  1. ação inexistente / JSON inválido / keystroke inválido → erro;
  2. mesmo keystroke, mesma profundidade de contexto, ações diferentes → aviso "X sombreia Y" (avaliado sobre pilhas canônicas: `[Workspace]`, `[Workspace, Grid vim=on vim_mode=normal …]`, `[Workspace, Grid vim=off]`, `[Workspace, Command]`, `[Workspace, Hints]`, `[Workspace, SongList]`), usando `Keymap::bindings_for_input`;
  3. binding que é prefixo estrito de outro no mesmo contexto → aviso "`g` dispara só após 1 s";
  4. uso de tecla morta do layout atual (`cx.keyboard_layout().id()` contém `br`) → aviso;
  5. prefixo `<leader>` sem `settings.leader` → erro.
  Diagnósticos aparecem no painel "Atalhos" (`Ctrl+K Ctrl+S`): tabela pesquisável de bindings por contexto, com fonte (padrão/usuário) e conflitos.

---

## 8. Tabela inicial de bindings padrão

### 8.1 Globais (contexto `Workspace`, vim on ou off)

| Tecla | Ação | Título |
|---|---|---|
| `ctrl-k`, `ctrl-p` | `keys::TogglePalette` | Paleta de comandos |
| `ctrl-space`, `alt-f` | `hints::Jump` | Saltar para um elemento |
| `ctrl-z` / `ctrl-shift-z`, `ctrl-y` | `project::Undo` / `project::Redo` | Desfazer / Refazer |
| `ctrl-s` | `project::SaveNow` | Salvar agora |
| `ctrl-n` | `project::New` | Nova música |
| `ctrl-o` / `ctrl-e` | `project::Import` / `project::Export` | Importar / Exportar JSON |
| `ctrl-shift-e` | `project::ExportMp3` | Exportar MP3 |
| `ctrl-1` `ctrl-2` `ctrl-3` | `view::Set(grade\|partitura\|esteira)` | Ver: grade / partitura / esteira |
| `ctrl-,` | `nav::Go(config)` | Configurações |
| `f1` | `keys::OpenShortcuts` | Mapa de atalhos |
| `escape` | `keys::CancelPending` | Cancela chord/seleção/hint |
| `+` / `-` | `transport::BpmDelta(1 \| -1)` | BPM ±1 (como no spike) |
| `ctrl-enter` | `transport::PlayFromStart` | Tocar do início |

### 8.2 `Ctrl+K …` (contexto `Command`, dentro da paleta aberta)

| Tecla | Ação |
|---|---|
| `ctrl-s` | `keys::OpenShortcuts` |
| `ctrl-t` | `config::ToggleTheme` |
| `ctrl-v` | `config::ToggleVim` |
| `m` / `l` / `c` / `t` | `transport::ToggleMetronome` / `ToggleLoop` / `ToggleCountIn` / `ToggleTrainer` |
| `1` `2` `3` | `view::Set(…)` |
| `z` | `grid::ZoomReset` |
| `ctrl-k`, `escape` | fechar |

### 8.3 Modo simples (`Grid && vim == off`)

| Tecla | Ação |
|---|---|
| letras, `!` | insert implícito (`on_key_down` → `edit`) |
| `space` (sem edição) | `transport::TogglePlay` |
| `space`/`tab`/`enter` (editando) | `grid::CommitAdvance`; `shift-tab` `grid::CommitBack` |
| setas | `grid::Move(dir)`; `shift-setas` `grid::ExtendSelection(dir)` |
| `home`/`end`, `ctrl-home`/`ctrl-end` | início/fim do compasso / da música |
| `page up`/`page down` | instrumento anterior/próximo |
| `delete`/`backspace` | `grid::DeleteSelection` (ou pop do buffer) |
| `ctrl-c`/`ctrl-x`/`ctrl-v` | `grid::Copy`/`Cut`/`Paste` (TSV no clipboard) |
| `!` (com seleção) | `grid::ToggleSfz` |
| `ctrl-shift-s` | `grid::ToggleSelectMode` |
| `escape` | cancela edição/seleção |

### 8.4 Vim — Normal (`Grid && vim == on && vim_mode == normal`)

Motions do §3.3; operadores/objetos/visual do §3.4; mais:

| Tecla | Ação |
|---|---|
| `i` `a` `I` `A` `o` `O` `r` `s` | entrar em insert (variantes) |
| `v` `V` `ctrl-v` | visual / visual-compasso / visual-bloco |
| `enter` | `transport::TogglePlayHere` (toca do cursor; `space` é leader) |
| `f` / `F` | `hints::Jump{track_cells}` / `hints::Jump{all_cells}` |
| `1`–`9` (`0` com contagem) | `vim::Count(n)` |
| `"` | `vim::PushRegister` |
| `u` / `ctrl-r` / `.` | desfazer / refazer / repetir |
| `!` | `grid::ToggleSfz` (célula) |
| `gu` / `gU` | `vim::Case(lower\|upper)` (acento) |
| `za` / `zR` / `zM` | recolher-expandir seção / expandir todas / recolher todas |
| `zi` / `zo` / `z0` | zoom + / − / 100 % |
| `m{a-z}` / `'{a-z}` | marcas (K4 opcional) |
| `:` | linha de comando (K4 opcional): `:12` compasso 12, `:w`, `:set vim off`, `:q` |

### 8.5 Leader (`space …`, modo normal e visual)

| Chord | Ação | Chord | Ação |
|---|---|---|---|
| `space space` | tocar/parar | `space /` | paleta |
| `space t p` | tocar/parar | `space t l` | loop |
| `space t m` | metrônomo | `space t c` | count-in |
| `space t t` | treino | `space t s` | parar |
| `space s n` | nova seção | `space s c` | clonar seção |
| `space s d` | remover seção | `space s r` | renomear seção |
| `space s +` / `space s -` | ritornello ± | `space s m` / `space s M` | + / − compasso |
| `space i n` | nova linha (instrumento atual) | `space i d` | remover linha |
| `space i m` / `space i s` | mute / solo da linha | `space i i` | trocar instrumento |
| `space v g` / `v p` / `v e` | ver grade / partitura / esteira | `space g e/m/p/k` | ir para editor/mixer/pessoas/kit |
| `space u 2` / `u 4` / `u 8` | subdivisão colcheia / semicolcheia / fusa | `space d d` / `space d h` | ciclar dinâmica / hairpin |
| `space z i` / `z o` / `z 0` | zoom | `space j` | `hints::Jump` |
| `space ,` | configurações | `space ?` | mapa de atalhos |

### 8.6 Vim — Insert (`Grid && vim == on && vim_mode == insert`)

`escape` confirma + normal · `space`/`tab`/`enter` confirmam + avançam · `shift-tab` confirma + recua · `backspace` apaga char / recua · `delete` apaga célula · `ctrl-u` limpa buffer · setas confirmam + navegam · `ctrl-w` apaga buffer (vim) · `alt-1`…`alt-9` inserem o chip 1–9 do grupo couro (opcional) · `ctrl-space` continua saltando (chips incluídos).

### 8.7 Vim — Visual

`h j k l w b e W B 0 $ ( )` estendem · `o` âncora · `d x y c p > < ! gu gU` operam · setas movem o bloco · `alt-setas` clonam · `escape` sai · `gv` reseleciona · `:` paleta de seleção.

---

## 9. Comandos da paleta

Mapeados dos 18 de `App.svelte:39-57` (título dinâmico preservado) mais os novos.

| Grupo | Comando (pt-BR) | Ação | Origem |
|---|---|---|---|
| Transporte | ▶ Tocar / ■ Parar | `transport::TogglePlay` | web |
| Transporte | Tocar daqui / do início | `transport::TogglePlayHere` / `PlayFromStart` | novo |
| Transporte | Loop: ligar/desligar | `transport::ToggleLoop` | web |
| Transporte | Metrônomo: ligar/desligar | `transport::ToggleMetronome` | web |
| Transporte | Count-in: ligar/desligar | `transport::ToggleCountIn` | web |
| Transporte | Treino (rampa de tempo): ligar/desligar | `transport::ToggleTrainer` | web |
| Transporte | Reto / Swing | `transport::ToggleSwing` | toolbar |
| Transporte | BPM +1 / −1 / definir… | `transport::BpmDelta` / `SetBpm` | spike |
| Ver | Ver: grade / partitura / esteira | `view::Set(..)` | web (3) |
| Ver | Tema: claro/escuro | `config::ToggleTheme` | web |
| Ver | Zoom + / − / 100 % | `grid::ZoomIn/Out/Reset` | toolbar |
| Navegar | Ir para Mixer / Pessoas / Kit / Editor / Configurações | `nav::Go(..)` | web (3) + novo |
| Navegar | Saltar para um elemento | `hints::Jump` | novo |
| Navegar | Ir para o compasso… | `vim::GotoMeasure` | novo |
| Edição | Modo: editar / selecionar | `grid::SetMode(edit\|select)` (vim off) | web (2) |
| Edição | Vim: ligar/desligar | `config::ToggleVim` | novo |
| Edição | Desfazer / Refazer | `project::Undo/Redo` | web (Ctrl+Z/Y) |
| Edição | Copiar / Recortar / Colar / Apagar seleção | `grid::Copy/Cut/Paste/DeleteSelection` | Grid.svelte |
| Edição | Alternar sforzando / Acentuar / Desacentuar | `grid::ToggleSfz`, `vim::Case(..)` | Grid.svelte + novo |
| Estrutura | + Compasso (seção selecionada) / − Compasso | `section::AddMeasure(±1)` | web |
| Estrutura | + Seção / Clonar / Remover / Renomear seção | `section::Append/Clone/Remove/Rename` | web + grid |
| Estrutura | Ritornello + / − | `section::Repeat(±1)` | grid |
| Estrutura | Recolher / expandir seção (todas) | `section::Collapse*` | grid |
| Estrutura | Subdivisão: colcheia / semicolcheia / fusa | `song::SetSubdivision(2\|4\|8)` | toolbar |
| Estrutura | Dinâmica do compasso: pp … ff / hairpin | `measure::SetDynamic(..)` | grid |
| Estrutura | + Linha (instrumento…) / Remover linha / Mute / Solo / Principal | `track::Add(inst)/Remove/Mute/Solo/ToggleMain` | web (+Linha Chū) + grid |
| Música | Nova música / Duplicar / Excluir / Trocar para… | `project::New/Duplicate/Delete/Select(i)` | web + painel |
| Música | Importar JSON / Exportar JSON / Exportar MP3 / Salvar agora | `project::Import/Export/ExportMp3/SaveNow` | painel |
| Teclado | Mapa de atalhos / Recarregar keymap / Abrir keymap.json / Testar tecla | `keys::OpenShortcuts/Reload/OpenFile/KeyProbe` | novo |

Lista de músicas: um item por projeto (`project::Select(i)`), com `keywords` = título.

---

## 10. Fases, entregáveis, estimativas

Trilha **K**, dentro de um dev em tempo integral. K0–K2 não dependem do `GridElement` final e podem começar hoje no `spike/`; K3–K4 precisam do `GridView` da Fase 3; K5 anda com a Fase 4. Inserir em `/planejamento/gpui.md §8` como "Fase 3-K" (≈ +5–6 semanas ao total; ~4,5 k LOC + 1,5 k de testes).

| Fase | Entregáveis | Estimativa |
|---|---|---|
| **K0 Registro + keymap** | `keys::{registry, actions, keymap_file, loader, settings}`; `extern crate gpui_kit as gpui`; `assets/keymap-default.json` cobrindo todos os atalhos atuais do spike; botões despacham ações; `wadawko-cli keymap check \| doc`; status bar com modo/showcmd; hot-reload + notificação; testes do parser/conflitos/invariantes | 1 semana |
| **K1 Paleta** | `CommandPalette` sobre `Command`; fuzzy próprio; títulos dinâmicos; `enabled`; `Kbd` por contexto; foco restaurado; lista de músicas; teste `simulate_keystrokes("ctrl-k") → "metr" → enter` | 3–4 dias |
| **K2 Chords + which-key** | `WhichKey` (observe_pending_input, timer, popup, grupos/títulos); leader; `Ctrl+K …` no contexto `Command` com cabeçalho; showcmd; testes com `advance_clock(1 s)` | 3–4 dias |
| **K3 Hints** | `HintRegistry`, `hintable`, `labels()`, overlay deferred, células por `Geom`, chips, foco/cancelamento, `alt-f`/`f`/`F`, campo "testar tecla"; testes de rótulos e de bounds (`TestWindowExt::find`) | 1 semana |
| **K4 Vim** | `VimState` + contextos; motions/objetos/ops puros com testes sobre fixtures do `backup-localstorage`; operadores, visual (3 tipos), contagem, registradores + TSV, `.`, undo; `EntityInputHandler` no insert; `/` `n` `N`; opcionais: marcas, `:`; página de ajuda `space ?` | 2–2,5 semanas |
| **K5 Config + docs + polimento** | Página Config (vim, leader, alfabeto, delay, tema); painel Atalhos com conflitos; `docs/ATALHOS.md` gerado + teste de atualidade; tooltips com `Kbd`; sessão de testes em ABNT2 + fcitx5; revisão dos títulos pt-BR | 3–4 dias |

Dependências: K0 → K1 → K2; K3 depende de K0; K4 depende de K0 e do `GridView` real; K5 de tudo.

---

## 11. Riscos e mitigações

| Risco | Impacto | Mitigação |
|---|---|---|
| **Churn da API de keymap do GPUI** (`gpui-pre` semanal) | reload/which-key/hints quebram no "dia de pin" | Superfície usada confinada a `keys/loader.rs`, `whichkey.rs`, `hints/overlay.rs`; lista explícita para re-verificar: `KeyBinding::load`, `KeyBindingContextPredicate::parse`, `bind_keys/clear_key_bindings/key_bindings`, `possible_bindings_for_input`, `pending_input/observe_pending_input/set_pending_input_timeout_paused`, `bindings_for_action_in`, `dispatch_action`, `on_children_prepainted`, `deferred/with_priority`, `EntityInputHandler`, `command::{Command, CommandState}`; testes de dispatch rodam no CI a cada bump |
| **`Ctrl+Espaço` capturado pelo compositor/IME** | jump não abre | `alt-f` ligado por padrão; `f`/`F`/`space j` no Vim; "testar tecla" em Config; doc do fcitx5/ibus |
| **Timeout fixo de 1 s** do multi-stroke | prefixo com binding próprio parece travado | Regra "prefixo nunca tem binding isolado no mesmo contexto"; `Ctrl+K` resolvido via contexto `Command`; barra de progresso no which-key quando há timeout; `check` avisa o usuário que cria o caso |
| **Teclas mortas / ABNT2** | `~ ^ \`` não funcionam, `:`/`?` exigem shift | Defaults evitam teclas mortas (`_` no lugar de `^`, `gu/gU` no lugar de `~`); aviso do `check` por layout; sessão de teste em `br` |
| **IME ativo engole a digitação** (Wayland text-input) | insert não recebe letras | `EntityInputHandler` no insert recebe o texto commitado; em normal `accepts_text_input=false` |
| **Views em cache não re-registram alvos** | rótulos faltando/defasados | ids estáveis + `notify` ao iniciar + sessão encerra em scroll/clique |
| Paleta despacha para o nó errado | comando da grade não roda | restaurar foco antes do dispatch; `is_action_available_in`; fallback: despachar nós mesmos após `on_confirm` |
| Hot-reload apaga bindings do gpui-component | paleta/inputs sem teclas | reload preserva `meta != USER` (clone + rebind); teste cobre |
| Vim demais para quem não usa | atrito | vim **desligado** por padrão (modo simples = comportamento atual); toggle visível na status bar e na paleta |
| Esforço estourar na K4 | atraso da Fase 3 | K4 é incremental: Normal+Insert+hjkl/w/b+d/y/p primeiro; visual/objetos/registradores/`.` depois; `:` e marcas opcionais |

---

## 12. Testes

**Unitários puros (sem GPUI, `cargo test` rápido)**
- `hints::labels`: propriedades (único, livre de prefixo, tamanho, ordenação, alfabeto, determinismo) com casos n ∈ {1, 9, 10, 26, 27, 250, 1000}; `assign(targets, cursor)` dá os rótulos curtos aos mais próximos.
- `keys::keymap_file`: parse do JSON (string / `[nome, json]` / `null` / `Unbind`), `<leader>`, erros com posição; `check` com fixtures de sombra, prefixo, tecla morta, ação inexistente.
- `keys::fuzzy`: ordenação esperada (`"metr"` → "Metrônomo…" antes de "Treino…"), *folding* de acentos, empate estável.
- `vim::motions/objects/ops`: tabelas sobre uma `Song` fixture (do `data/backup-localstorage-2026-10-02.json`): `w`/`b` com subdivisões 2/4/8 e swing, `W`/`B` cruzando seções, `0/$/_`, `gg/G/{n}G`, `iw` em runs, `am` como coluna, `d2w`, `3dd`, `>>`, `gu/gU`, `p` com forma, `.` após `ciw`+texto.
- `vim::registers`: round-trip `CellBlock` ↔ TSV.
- `keys::registry`: invariantes (§2.1) e "todo binding padrão referencia ação existente".

**GPUI (`gpui-kit` com `test-support`, `#[gpui_kit::test]`)**
- Dispatch por contexto: janela de teste com `Shell`; `cx.simulate_keystrokes("i d o n space")` → célula preenchida e cursor avançado; `"escape 3 l"` → cursor +3; `"g g"` → início; `"space t m"` → metrônomo alterna; com vim off, `"d"` digita em vez de operar.
- Pendente/which-key: após `"space"`, `window.has_pending_keystrokes()` e `possible_bindings_for_input` não vazio; `WhichKey.visible` após `advance_clock(250 ms)`; `escape` limpa.
- Timeout: binding `ctrl-k` + `ctrl-k ctrl-s` reproduz o atraso (teste documental); com a solução do §4, `"ctrl-k"` abre a paleta sem `advance_clock`.
- Paleta: `"ctrl-k"` + `simulate_input("metr")` + `"enter"` → ação executada e foco de volta na grade.
- Hints: `"ctrl-space"` → `HintRegistry` contém o botão `play` com bounds iguais a `window.find("play").bounds()` (TestWindowExt) e as células de tempo visíveis; digitar o rótulo do `play` alterna playback; `escape` restaura foco.
- Reload: escrever keymap de usuário em `tempdir`, chamar `reload` → novo binding ativo, bindings do `Command` preservados.
- Manual (checklist K5): Wayland e X11; layout `br`; fcitx5 ligado/desligado; 10 min de uso só por teclado para gravar a "Renshū".

**Docs**: `wadawko-cli keymap doc > docs/ATALHOS.md` gerado do registro + keymap padrão; teste falha se o arquivo commitado estiver desatualizado.

---

### Critical Files for Implementation
- spike/src/main.rs — Shell: `on_key` (232-285) vira ações/keymap; helpers `btn/toggle/segmented` (364-429) passam a despachar ações e registrar alvos de hint; overlay do playhead (760-801) é o modelo das camadas de hints/which-key; status bar (914-938) recebe o indicador de modo.
- spike/src/grid.rs — `GridView` recebe `VimState`, `key_context` dinâmico, `on_action` das ações `grid::/vim::`, `EntityInputHandler` no insert, `visible_cells()` a partir de `Geom`/`measure_xs`/culling (47-57, 76-81, 873-912) e os bounds dos botões de cabeçalho/linha.
- spike/src/model.rs — `Song`/`Cur` e operações (`add_measure`, `clone_section`, `set_subdivision`, 268-367) sobre as quais `vim::{motions, objects, ops}` são escritos e testados.
- src/App.svelte — os 18 comandos da paleta (39-57) e os atalhos globais (192-208) que o registro precisa cobrir com paridade de títulos.
- /planejamento/gpui.md — onde a trilha K entra (§8, Fases 3–4) e cujo §2/§8.0 deve ganhar os itens "verificado" da §1.1 deste plano (keymap, pending input, `Command`, `Kbd`, test-support).

Fontes do GPUI consultadas (para o "dia de pin"): `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/gpui-pre-0.3.12/src/{action.rs, keymap.rs, keymap/binding.rs, keymap/context.rs, key_dispatch.rs, window.rs, app.rs, app/context.rs, app/test_context.rs, platform/keystroke.rs, input.rs, elements/div.rs, elements/deferred.rs}`, `gpui-component-0.9.1/src/{command/, kbd.rs, window_ext.rs, status_bar.rs}`, `gpui-base-0.9.1/src/{root.rs, popup.rs, actions.rs}`, `gpui-kit-0.9.1/{Cargo.toml, src/lib.rs, src/test.rs}`, `gpui-pre-macros-0.3.12/src/derive_action.rs`, `gpui-pre-linux-0.3.12/src/linux/wayland/client.rs`.