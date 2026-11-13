# wadawko-app — arquitetura e como plugar uma view

Binário `wadawko` (GPUI via `gpui-kit = "=0.9.1"`, `extern crate gpui_kit as gpui`).
Porte modularizado do `spike/`, com o modelo/áudio/persistência trocados pelos
crates definitivos (`wadawko-core`, `-store`, `-samples`, `-audio`, `-export`).

## Mapa dos arquivos

```
src/
  main.rs            args (--db, --read-only, --samples-dir, --song, --dark, --mute, --bench, --demo, --selftest), init, janela
  state.rs           AppState: ESTADO COMPARTILHADO (store + history + áudio + transporte + tema)
  song_ext.rs        SongExt: índices da grade (s, m, t, k, coluna global) ↔ ids do core
  theme.rs           tokens de cor (app.css) + parse_color("#rrggbb")
  ui.rs              btn / abtn / atoggle / segmented / kbd / small_btn / card
  stats.rs           histograma de frames/paint (--bench, HUD)
  shell/
    mod.rs           Shell (view raiz): foco, página/aba, overlays, playhead/flash/follow, on_key
    actions.rs       handlers de TODAS as ações globais (registrados no div raiz)
    io.rs            exportar/importar JSON, exportar WAV/MP3
  keys/
    actions.rs       ações GPUI (um nome = um comando)
    registry.rs      SPECS (título/grupo/palavras-chave) + PREFIX_TITLES — fonte única
    keymap.rs        JSON padrão + ~/.config/wadawko/keymap.json, conflitos
    settings.rs      ~/.config/wadawko/settings.json (vim, av_offset_ms)
    fuzzy.rs labels.rs hint_targets.rs   busca, rótulos, registro de alvos de salto
    palette.rs       paleta de comandos Ctrl+K            (impl Shell)
    whichkey.rs      which-key / Ctrl+/                   (impl Shell)
    hint_overlay.rs  overlay de rótulos Ctrl+Space        (impl Shell)
    modal.rs         vim: `.`, registradores, marcas, busca, `:`, prompt de texto (impl Shell)
  views/
    mod.rs           Page, Tab, Views, bind_actions  ← PONTO DE EXTENSÃO
    grid/{mod,render,paint}.rs   a grade (GridView)
    topbar.rs nav_rail.rs right_panel.rs status_bar.rs context_toolbar.rs settings.rs   (impl Shell)
    mixer.rs people.rs kit.rs lane.rs score.rs    páginas/abas — um arquivo por view
assets/keymap-default.json
```

## Estado compartilhado: `Entity<AppState>` (`state.rs`)

Há **uma** entidade GPUI com tudo que não é de uma view só:

| campo / método | o que é |
|---|---|
| `store: ProjectStore` | banco + workspace + autosave (400 ms) |
| `history: SongHistory` | desfazer/refazer (100 níveis) |
| `audio: Option<Audio>` | engine; `None` enquanto os samples carregam (thread de fundo) ou sem dispositivo |
| `bank: Option<SampleBank>` | para previews e export |
| `schedule: Arc<Schedule>` | último compilado (`schedule.time` = `TimeMap`) |
| `tp: Transport` | loop, metrônomo, count-in, treino (% e passo), seções selecionadas (região) |
| `pen: Option<String>` | "caneta": pessoa que assina as notas em linha compartilhada |
| `theme: Theme` | cores (claro/escuro), persistido no banco |
| `hits: Vec<HitFlash>` | golpes que acabaram de soar (de `EngineEvent::HitFired`) |
| `revision: u64` | muda a cada edição/troca de música (chave de cache) |

Toda view recebe um clone do `Entity<AppState>` no `new` e faz
`cx.observe(&state, |_, _, cx| cx.notify())` para re-renderizar.

### Ler a música

```rust
let song: Arc<Song> = self.state.read(cx).song().clone(); // snapshot imutável, barato
```
Guarde o `Arc` para fechar em closures de pintura (`canvas`). Para converter
índices use `crate::song_ext::SongExt` (`secs()`, `flat/unflat`, `tid(t)`, `cell(t, m, k)`…).

### Editar (o ÚNICO caminho)

```rust
self.state.update(cx, |s, cx| s.edit(cx, |song| {
    wadawko_core::ops::people::rename_person(song, &id, "Ana");
}));
```
`AppState::edit` = `history.record(antes)` → sua closure (use `wadawko_core::ops`) →
`store.update_song` (autosave) → recompilação do áudio (debounce de 30 ms, com
`apply_mix`) → `StateEvent::Edited` + `notify`. Edição que não muda nada não
cria passo de desfazer.

- **Sliders / arrasto**: `s.edit_coalesced(cx, TAG, |song| …)` — edições seguidas
  com a mesma `TAG: u64` viram um passo de desfazer (ex.: `0x4D49_0000 | índice`).
- Desfazer/refazer: `s.undo(cx)` / `s.redo(cx)` (ações `project::Undo/Redo`).
- Nunca guarde `&mut Song` nem escreva em `store` direto.
- Músicas: `switch_project`, `new_project`, `add_project(song)`, `duplicate_project`,
  `rename_project`, `delete_project` — emitem `StateEvent::SongReplaced` (zere seu
  estado de UI: `cx.subscribe(&state, |this, _, ev, cx| …)`).

### Relógio e eventos do playback

- `s.playing()`; `s.play(start_cell, cx)` / `s.stop(cx)` / `s.seek_cell(c)`; `s.set_bpm`, `set_loop`, `set_metronome`.
- `s.playhead() -> Option<Playhead { mi, frac, cell, tick }>` — posição AUDÍVEL
  (relógio − latência − `av_offset_ms`), dobrada para o compasso-fonte.
- `s.audible_tick()` + `s.schedule.time` (`cell_at`, `fold_tick`, `cell_tick`,
  `folded_seconds`) para contas próprias (esteira: `x = (seg_evento − t)·380`).
- `s.hits` — `HitFlash { track, cell, hit, at }`; `at` já inclui a latência, então
  pisque quando `Instant::now() >= at`, por `s.flash_ms()` ms. O Shell chama
  `s.poll()` uma vez por frame (não chame de novo).
- Enquanto `s.playing()`, o Shell pede `request_animation_frame` e o
  `notify` do estado re-renderiza quem observa; uma view com animação própria
  pode pedir `window.request_animation_frame()` no seu `render`.
- Previews (com `if let Some(a) = s.audio.as_mut()`): `preview_cell`,
  `preview_instrument(def, zone)`, `start_phrase_loop/stop_phrase_loop`,
  `preview_kyai`, `play_fue`.

### Comandos: paleta, keymap, which-key, hints

1. Declare a ação em `keys/actions.rs` (`pub mod mixer { actions!(mixer, [Foo]); }`).
2. Dê um título em `keys/registry.rs::SPECS` (`spec!("mixer::Foo", "Título", "Grupo", "palavras")`)
   → aparece na paleta Ctrl+K, no `:` e no which-key. Um teste falha se um binding padrão não tiver título.
3. (opcional) atalho em `assets/keymap-default.json` (um teste garante que não há conflito;
   chords `space x y` precisam do prefixo em `PREFIX_TITLES`).
4. Trate a ação no `bind` do SEU arquivo (roda com o foco em qualquer lugar):
   ```rust
   pub fn bind(d: Stateful<Div>, cx: &mut Context<Shell>) -> Stateful<Div> {
       d.on_action(cx.listener(|shell: &mut Shell, _: &keys::mixer::Foo, _, cx| {
           shell.views.mixer.update(cx, |v, cx| v.foo(cx));
       }))
   }
   ```
5. Botões: `ui::abtn("id", &th, keys::mixer::Foo)` despacha a mesma ação e já é
   alvo de salto (Ctrl+Space). Para um elemento qualquer virar alvo:
   `keys::hintable(el.relative(), "id-único", Rc::new(|w, cx| …))`.

## Plugar uma página/aba

`views/mod.rs` tem `enum Page { Editor, Pessoas, Mixer, Kit, Config }`,
`enum Tab { Grade, Partitura, Esteira }` e a struct `Views` com uma entidade
por arquivo. Os dois `match` (`Views::page`, `Views::tab`) já delegam para os
stubs — **cada agente edita só o próprio arquivo**:

| arquivo | struct | onde aparece |
|---|---|---|
| `views/mixer.rs` | `MixerView` | página Mixer |
| `views/people.rs` | `PeopleView` | página Pessoas |
| `views/kit.rs` | `KitView` | página Kit |
| `views/lane.rs` | `LaneView` | aba Esteira |
| `views/score.rs` | `ScoreView` | aba Partitura |

Contrato de cada um: `pub struct XView { pub state: Entity<AppState>, … }`,
`XView::new(state, cx)`, `impl Render`, `pub fn bind(d, cx) -> d`. A view é
renderizada dentro da área rolável do editor (padding 20 px, largura total).
Uma página nova = variante em `Page` + entrada em `Views` + item em `nav_rail.rs`.

## Teclas que chegam como texto

O div `workspace` tem o foco e o contexto `Workspace vim=… mode=…`; o que nenhum
binding pega cai em `Shell::on_key`, que só digita na grade quando
`page == Editor && tab == Grade`. Uma view que precise de texto deve usar o
prompt do Shell (`Shell::ask(InputTarget, inicial, window, cx)`, acrescentando a
sua variante em `views::grid::InputTarget` + `modal.rs::submit_input`) ou ter o
próprio `FocusHandle` com `key_context` próprio.

## Regras de pintura

Views pesadas seguem a grade: um `canvas` com culling, textos modelados em cache
(`shape_line`), geometria do último frame guardada num `Rc<RefCell<…>>` para
overlays sem re-render. A grade fica em `Entity::cached(...)` e o
playhead/flash é um `canvas` irmão sem cache, no Shell.
