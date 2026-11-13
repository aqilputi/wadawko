# WaDAWko — Planejamento de Redesign (IA, sidebars, multi-música, tema, arquitetura)

> Evolução do protótipo `taiko-fu` para um app tipo DAW de taiko. Rename para **WaDAWko**
> (wadaiko 和太鼓 + DAW). Editor de kuchi shōga em grade, playback com samples, esteira
> estilo Taiko no Tatsujin, partitura, seções, múltiplas músicas.

Data: 2026-07-04 · Status: planejamento (pré-implementação) · Autor: par Claude + Fernando

Referência de DAW: **Bitwig Studio** e **Renoise**. Este doc é o mapa; a implementação vem em fases.

---

## 1. Objetivos deste ciclo

1. **Reorganizar comandos** (compasso, seção, subdivisão, transporte, arquivo) em lugares
   adequados — header/toolbar no topo, agrupados por função (padrão DAW).
2. **Sidebar esquerda** = navegação de páginas (Editor Taiko hoje; Mixer/Kit/Config no futuro).
3. **Sidebar direita** = lista de **músicas (projetos)** — poder ter várias e trocar.
4. **Múltiplas músicas** — workspace com N músicas, cada uma isolada, persistidas.
5. **Novo tema** — pastel vermelho + azul, 2–3 tons, ar moderno; light e dark.
6. **Rename** para **WaDAWko** (título, header, storage keys, export).
7. **Modularizar** a arquitetura (hoje `Grid.svelte` e `store` são monolitos).
8. **Backlog de features DAW** (Bitwig/Renoise) — o que faz sentido pro taiko.

---

## 2. Pesquisa UX (síntese)

### 2.1 Renoise (tracker) — o que roubar
- **Layout de painéis fixos**: sequencer de padrões (esquerda estreita) + editor central grande +
  browser de disco/instrumentos (canto) + faixa de efeitos embaixo. Regiões estáveis, sem janelas soltas.
- **Pattern Sequencer / Matrix**: a música é uma **lista de padrões reordenáveis e reutilizáveis**.
  Nossas **seções** já caminham pra isso — vale evoluir seção → "padrão" com possível *reuso por referência*.
- **Edição centrada no teclado**: entrar notas digitando, cursor forte, atalhos densos. Já temos digitação;
  reforçar atalhos e navegação.
- **Layouts salváveis** (presets 1–8) e painéis redimensionáveis por drag.
- Refs: [Pattern Editor](https://tutorials.renoise.com/wiki/Pattern_Editor), [Disk Browser](https://tutorials.renoise.com/wiki/Disk_Browser), [Atalhos](https://tutorials.renoise.com/wiki/Keyboard_Shortcuts).

### 2.2 Bitwig Studio — o que roubar
- **Anatomia clara**: header → menus/transport → corpo. Corpo = **painel central** + **painel secundário**
  (embaixo, contextual) + **access panels à direita** (Browser, Project, Monitoring) + **Inspector à esquerda**.
- **Inspector à esquerda** mostra propriedades do que está selecionado (clip/track/device) sem sair da tela.
  → nosso equivalente: propriedades da faixa/seção/célula selecionada.
- **Painéis destacáveis** pra multi-monitor. (Fase futura.)
- **Views Arrange / Mix / Edit** trocáveis no mesmo corpo. → nossas views (grade/partitura/esteira/mixer)
  viram páginas/tabs.
- Refs: [Anatomy of the Bitwig Window](https://www.bitwig.com/userguide/latest/anatomy_of_the_bitwig_studio_window/), [The Window Body](https://www.bitwig.com/userguide/latest/the_window_body/), [Transport Area](https://www.bitwig.com/userguide/latest/the_window_menus_transport_area/).

### 2.3 Sidebar / navegação (boas práticas web)
- Vertical à esquerda escala melhor que topo pra navegação de app; **5–7 itens de 1º nível** com ícone+label,
  sub-itens só texto, largura ~240–260px, **redimensionável por drag**, estados ativos claros.
- CTA de destaque ("+ Nova música").
- Refs: [NN/g vertical nav](https://www.nngroup.com/articles/vertical-nav/), [Adobe Spectrum side-nav](https://spectrum.adobe.com/page/side-navigation/), [UX Planet sidebar](https://uxplanet.org/best-ux-practices-for-designing-a-sidebar-9174ee0ecaa2).

### 2.4 Editores de taiko / kuchi shōga
- Confirmado no `PLANEJAMENTO.md`: não há concorrente que junte grade + sílabas nativas + multi-instrumento +
  playback. Nosso diferencial = **prática de ensemble** (loop de seção, tempo, mão D/E). Feature-âncora:
  **Practice Trainer** (loop de seção com rampa de tempo) — nenhum editor de taiko tem.

---

## 3. Princípios de design adotados

- **Regiões estáveis, nada "na frente"** do conteúdo que edita (aprendizado do HUD flutuante).
- **Comando perto do contexto**: transporte global no topo; estrutura (compasso/seção/subdivisão) perto da grade.
- **Teclado-first** com atalhos e futura **command palette** (`Ctrl/⌘+K`).
- **Progressive disclosure**: sidebars colapsáveis; inspector aparece só quando há seleção.
- **Um sistema visual**: tokens de cor/spacing/tipografia únicos (ver §7), light+dark.

---

## 4. Nova Arquitetura de Informação (layout)

```
┌───────────────────────────────────────────────────────────────────────────┐
│  TOPBAR: ☰  WaDAWko 太  | ‹título música›  | ▚ transporte ▶ ■ ↺ | BPM swing | ⚙ ☀/☾ │
├────┬──────────────────────────────────────────────────────────┬────────────┤
│ N  │  VIEW TABS: [grade] [partitura] [esteira] [mixer]         │  MÚSICAS   │
│ A  │  ┌──────────────────────────────────────────────────┐    │  + nova    │
│ V  │  │  CONTEXT TOOLBAR (muda por view/seleção):         │    │ ▸ Matsuri  │
│    │  │   editar|selecionar · +compasso −compasso ·       │    │ ▸ Renshuu  │
│ R  │  │   subdivisão · +seção · zoom                      │    │ ▸ Solo Odai│
│ A  │  ├──────────────────────────────────────────────────┤    │            │
│ I  │  │                                                  │    │ ── recentes│
│ L  │  │        EDITOR (grade em blocos de seção /         │    │ ── modelos │
│    │  │        partitura / esteira / mixer)               │    │            │
│ 📝 │  │                                                  │    │ [inspector]│
│ 🎚 │  │                                                  │    │ propriedades│
│ 🥁 │  │                                                  │    │ da seleção │
│ ⚙  │  └──────────────────────────────────────────────────┘    │            │
├────┴──────────────────────────────────────────────────────────┴────────────┤
│  STATUS BAR: seção ativa · compasso/tempo · seleção · dica de atalho · autosave │
└───────────────────────────────────────────────────────────────────────────┘
```

- **NAV RAIL (esq., ~56px colapsado / 220px aberto)**: páginas do app. Ícones + label.
  Hoje: **Editor** (📝). Futuro: **Mixer** (🎚), **Kit/Samples** (🥁), **Config** (⚙).
- **TOPBAR**: marca + título da música + **transporte global** (▶/■, loop, BPM, swing, metrônomo) + tema + menu.
- **VIEW TABS**: grade / partitura / esteira / mixer (dentro da página Editor).
- **CONTEXT TOOLBAR**: comandos que hoje estão espalhados no rodapé — reagrupados e contextuais à view.
- **SIDEBAR DIREITA (~260px, colapsável)**: aba **Músicas** (lista de projetos, +nova, recentes, modelos) e,
  quando algo está selecionado, **Inspector** (propriedades da faixa/seção/célula) — estilo Bitwig.
- **STATUS BAR (rodapé fino)**: contexto + autosave + atalhos.

Responsivo: <1100px colapsa as duas sidebars em drawers; transporte vira compacto.

---

## 5. Reorganização de comandos (de onde → pra onde)

| Comando (hoje) | Local hoje | Local novo |
|---|---|---|
| ▶ tocar / ■ parar / loop | header | **Topbar / transporte** |
| BPM, swing (reto/swing) | header | **Topbar / transporte** (swing pode ir p/ menu de ritmo) |
| grade / partitura / esteira | tabs no meio | **View tabs** (topo do editor) + nova **mixer** |
| editar / selecionar (modo) | acima da grade | **Context toolbar** (grupo "ferramentas") |
| + / − compasso | rodapé (actions) | **Context toolbar** (grupo "estrutura") |
| subdivisão (colcheia/…) | rodapé | **Context toolbar** (grupo "estrutura") + atalho |
| + seção | rodapé | **Context toolbar** (grupo "estrutura") e/ou botão na lista de seções |
| + linha (instrumento) | rodapé | **Context toolbar** (grupo "faixas") ou header do mixer |
| exportar/importar/demo | rodapé | **Menu ☰** (Arquivo) na topbar + sidebar Músicas |
| paleta de sílabas | HUD flutuante | mantém HUD (já bom) + atalhos de teclado |

Grupos da Context Toolbar (esquerda→direita): **Ferramentas** (editar/selecionar, zoom) · **Estrutura**
(compasso, subdivisão, seção) · **Faixas** (+linha) · **Overflow** (⋯).

---

## 6. Múltiplas músicas (projetos)

### Modelo de dados
```ts
interface Project {           // uma música
  id: string
  song: Song                  // (já existe)
  createdAt: number
  updatedAt: number
}
interface Workspace {
  version: 2
  projects: ProjectMeta[]     // índice leve (id, title, updatedAt) p/ a sidebar
  currentId: string | null
}
```
### Storage
- `localStorage` chaveado: `wadawko:workspace` (índice) + `wadawko:project:<id>` (cada música).
  Evita reserializar tudo a cada tecla (hoje salva `store.song` inteiro).
- Autosave por projeto (debounce). Migração automática do `taiko-fu:song` antigo → 1 projeto.
- Export: 1 música (`.wadawko.json`) ou workspace inteiro (`.wadawko-workspace.json`).
- Futuro: File System Access API (abrir pasta como biblioteca) e share por URL.

### UX (sidebar direita, aba Músicas)
- Lista com título, nº de compassos/faixas, data; clique troca a música ativa.
- **+ Nova** (em branco ou a partir de modelo/demo), **duplicar**, **renomear**, **excluir** (com undo).
- Seções **recentes** e **modelos** (matsuri, cavalinho, jiuchi…).

---

## 7. Nova paleta (pastel vermelho + azul, 2–3 tons) — light + dark

Vermelho = **couro (head)**, Azul = **fuchi (rim)** — coerente com a semântica que já existe.
Neutros levemente frios (base azulada) pra dar o ar moderno. 3 tons por cor.

```css
:root {                      /* LIGHT */
  /* neutros frios */
  --bg:        #f5f6fa;      /* fundo app */
  --surface:   #ffffff;      /* painéis */
  --surface-2: #eef1f7;      /* elevação/raise */
  --line:      #dde2ec;
  --line-2:    #c6cede;
  --ink:       #232733;
  --ink-soft:  #5c6577;
  --ink-faint: #97a0b4;

  /* vermelho (couro) — 3 tons */
  --red-1:     #fbe7ea;      /* wash */
  --red-2:     #f0949f;      /* mid */
  --red-3:     #e26374;      /* primary/ação */

  /* azul (fuchi) — 3 tons */
  --blue-1:    #e7eefb;      /* wash */
  --blue-2:    #a7c0e8;      /* mid */
  --blue-3:    #6f93cf;      /* primary/secundária */

  --accent:      var(--red-3);   /* ação principal */
  --accent-wash: var(--red-1);
  --accent-2:    var(--blue-3);  /* secundária/fuchi */
}
:root[data-theme='dark'] {   /* DARK */
  --bg:        #161922;
  --surface:   #1d212c;
  --surface-2: #262b39;
  --line:      #2c3242;
  --line-2:    #3a4256;
  --ink:       #e8ebf2;
  --ink-soft:  #9aa4ba;
  --ink-faint: #6b7590;

  --red-1:  #3a2830;  --red-2:  #d98793;  --red-3:  #ec7c8b;
  --blue-1: #22304a;  --blue-2: #8fb0e0;  --blue-3: #86a9e0;
  --accent: var(--red-3); --accent-wash: var(--red-1); --accent-2: var(--blue-3);
}
```
- **Toggle de tema** na topbar (☀/☾), default segue `prefers-color-scheme`.
- Uso: superfícies em neutros; **vermelho = ações/seleção/couro**, **azul = fuchi/estados secundários/seção ativa**.
- Tipografia mantém Shippori Mincho (marca) + Zen Kaku Gothic (UI). Cantos 8–12px, sombras suaves.

---

## 8. Arquitetura de componentes (modularização)

Hoje: `App.svelte` (grande), `Grid.svelte` (~1000 linhas), `store.svelte.ts` (tudo). Proposta:

```
src/
  main.ts
  app/
    Shell.svelte            # layout geral (rail + topbar + body + right panel + status)
    NavRail.svelte          # sidebar esquerda (páginas)
    TopBar.svelte           # marca + título + transporte + tema
    Transport.svelte        # ▶ ■ ↺ BPM swing metrônomo
    RightPanel.svelte       # container: aba Músicas | Inspector
    SongList.svelte         # lista de projetos
    Inspector.svelte        # propriedades da seleção
    StatusBar.svelte
  pages/
    editor/
      EditorPage.svelte     # view tabs + context toolbar + view atual
      ContextToolbar.svelte
      grid/
        GridView.svelte     # empilha SectionBlock
        SectionBlock.svelte # header + timeline + rows (hoje é o miolo do Grid)
        SectionHeader.svelte
        TimelineRow.svelte
        TrackRow.svelte
        Cell.svelte         # (opcional; cuidado com perf — talvez manter inline)
        Palette.svelte      # HUD flutuante
        selection.ts        # lógica de seleção/clipboard (extraída do Grid)
      score/ScoreView.svelte
      lane/LaneView.svelte
    mixer/MixerPage.svelte   # (futuro) faders/pan/mute/solo
    kit/KitPage.svelte       # (futuro) samples por instrumento
    settings/SettingsPage.svelte
  lib/
    audio/
      engine.ts             # transport, schedule, region, seek
      voices.ts             # makeVoice, playLayered, humanize
      samples.ts            # SAMPLE_FILES, TUNING, buffers
    music/
      types.ts              # Song, Track, Measure, Section
      timing.ts             # swingWarp, cellFloatAt, cellPosition (compartilhado audio+lane+grid)
      phrase.ts, vocabulary.ts, score.ts, lane.ts
      songOps.ts            # add/removeMeasure, subdivision, colToPos, ensureCols, cell ops
      sections.ts           # ranges, add/append/move/remove/select
    stores/
      workspace.svelte.ts   # projetos, música atual, storage
      ui.svelte.ts          # page, view, mode, editing, sidebars, theme
      transport.svelte.ts   # playing, loop, cursor, startCell, selectedSections
      history.svelte.ts     # undo/redo (extraído do App)
```
Regras: **stores finos por domínio** (workspace/ui/transport), **lógica pura em `lib/music`** (testável),
componentes pequenos e burros. Extrair `timing.ts` elimina a duplicação atual entre `audio.schedule` e `lane`.
Migração incremental (não big-bang): §11.

---

## 9. Backlog de features inspiradas em DAW

Prioridade: ⭐⭐⭐ alta / ⭐⭐ média / ⭐ futura.

- ⭐⭐⭐ **Practice Trainer** (feature-âncora): loop de seção + **rampa de tempo** (começa lento, acelera a cada
  N repetições), count-in, metrônomo (jiuchi). Nenhum editor de taiko tem — ouro pra ensaio de grupo.
- ⭐⭐⭐ **Metrônomo / count-in** no transporte.
- ⭐⭐⭐ **Zoom horizontal** da grade (Renoise/Bitwig) + snap/quantize configurável.
- ⭐⭐⭐ **Command palette** `Ctrl/⌘+K` (Bitwig-like) e mapa de atalhos.
- ⭐⭐ **Página Mixer**: faders de volume, pan, mute/solo por faixa (hoje só M/S), medidores.
- ⭐⭐ **Reordenar/color-code faixas** (drag), renomear inline.
- ⭐⭐ **Seções reutilizáveis** (padrão por referência, à la Renoise matrix): "A A B A".
- ⭐⭐ **Arranger** (linha de arranjo com as seções como blocos arrastáveis) — evolução da sidebar de seções.
- ⭐⭐ **Export**: render de áudio offline (WAV), imagem/PDF da partitura pra imprimir, TJA (Taiko no Tatsujin).
- ⭐⭐ **Inspector** contextual (Bitwig): propriedades de faixa/seção/célula selecionada.
- ⭐ **MIDI input** pra entrar golpes tocando; mapear pads.
- ⭐ **Painéis destacáveis / multi-monitor** (Bitwig).
- ⭐ **Share por URL** (música serializada) e biblioteca via File System Access API.
- ⭐ **Temas customizáveis** e presets de layout (Renoise 1–8).
- ⭐ **Histórico visual** (painel de undo).

Melhorias de usabilidade transversais: estados de foco/hover consistentes, tooltips com atalho, empty-states
bons, autosave visível, i18n (pt/ja/en), acessibilidade (roles/teclado — já viemos cuidando).

---

## 10. Rename → WaDAWko

- `<title>`, marca na topbar, `package.json name`, textos.
- Storage keys: `taiko-fu:*` → `wadawko:*` com **migração** automática na 1ª carga.
- Export default: `*.wadawko.json` (aceitar importar `.taiko.json` antigo).
- Mantém o ideograma 太 (taiko) na marca.

---

## 11. Roadmap por fases (incremental, sem big-bang)

- **F1 — Shell + tema + rename** (base visual): `Shell/NavRail/TopBar/RightPanel/StatusBar`, tokens novos de cor
  (light/dark + toggle), mover transporte pro topo, rename WaDAWko. Editor atual entra no corpo sem quebrar.
- **F2 — Multi-música**: `stores/workspace`, storage por projeto, sidebar Músicas (+nova/duplicar/excluir),
  migração do save antigo.
- **F3 — Reorg de comandos**: `EditorPage` + `ContextToolbar` (estrutura/ferramentas/faixas), view tabs;
  esvazia o rodapé de actions.
- **F4 — Modularização do editor**: quebrar `Grid.svelte` em `GridView/SectionBlock/TimelineRow/TrackRow`,
  extrair `selection.ts` e `lib/music/timing.ts`; separar stores `ui/transport/history`.
- **F5 — Practice Trainer + metrônomo/count-in** (feature-âncora).
- **F6 — Mixer + zoom/snap + command palette**.
- **F7+** — backlog ⭐ (arranger, export áudio/PDF/TJA, MIDI, multi-monitor, share).

Cada fase fecha com `npm run check` + build verdes e é usável ponta-a-ponta.

---

## 12. Decisões em aberto

1. **Seção = padrão reutilizável** (referência) ou continua bloco linear de compassos? (afeta arranger.)
2. **Zoom**: escala CSS do `--cellw` (simples) vs virtualização (perf p/ músicas longas).
3. **Sidebar direita**: Músicas e Inspector como abas do mesmo painel, ou dois painéis? (proponho abas.)
4. **Persistência**: localStorage agora; migrar pra IndexedDB/File System quando as músicas crescerem?
5. **Tons exatos** da paleta (§7 é proposta — validar no olho, ajustar saturação).

## 13. Referências
- Bitwig: [Window Anatomy](https://www.bitwig.com/userguide/latest/anatomy_of_the_bitwig_studio_window/), [Window Body](https://www.bitwig.com/userguide/latest/the_window_body/), [Transport](https://www.bitwig.com/userguide/latest/the_window_menus_transport_area/)
- Renoise: [Pattern Editor](https://tutorials.renoise.com/wiki/Pattern_Editor), [Disk Browser](https://tutorials.renoise.com/wiki/Disk_Browser), [Shortcuts](https://tutorials.renoise.com/wiki/Keyboard_Shortcuts)
- Sidebar/nav: [NN/g](https://www.nngroup.com/articles/vertical-nav/), [Adobe Spectrum](https://spectrum.adobe.com/page/side-navigation/), [UX Planet](https://uxplanet.org/best-ux-practices-for-designing-a-sidebar-9174ee0ecaa2)
- Base do produto: `PLANEJAMENTO.md` (visão, domínio, engine)
