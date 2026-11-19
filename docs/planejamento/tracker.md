---
type: Plan
title: "Editor vertical estilo tracker (seções, efeitos por célula, cores)"
description: "Pesquisa de trackers (Renoise e os modernos), modelo mental do editor vertical de taiko, painel de seções com grupos e cores, efeitos por célula legíveis, evolução do sistema de design, mapa de teclas e plano de implementação em etapas."
tags: [planejamento, tracker, editor, ux, seções, efeitos, cores, teclado]
status: draft
estado: "em implementação (2026-11-18): T0, T1, T2, T3a, T5, T6 (painel), T7 (seção/grupo/instrumento), T8 (sem T8b) feitos"
---

# WaDAWko — editor vertical estilo tracker

Data: 2026-11-18 · Status: **planejado** · Pedido do Fernando: *"faça os compassos na vertical,
fazendo algo parecido com o Renoise mas com células maiores; as seções agora ficam numa aba à
esquerda que eu posso selecionar, copiar etc.; os efeitos podem ser por célula igual Renoise;
tente um design mais moderno; faça eu poder mudar as cores dos instrumentos e das seções e
também agrupar seções; faça um planejamento de UX."*

Este documento tem dez partes: (1) pesquisa, (2) modelo mental e decisões do editor vertical,
(3) painel de seções com grupos, (4) efeitos por célula, (5) cores, (6) evolução do sistema de
design com wireframes, (7) teclado, (8) o que acontece com o que existe, (9) plano de
implementação em etapas e (10) perguntas em aberto. O agente implementador segue a parte 9; as
partes 2–7 são a especificação.

Contexto: [guia de uso](/guias/uso.md) (o app hoje), [arquitetura do app](/arquitetura/app.md)
(regras de cache e pintura), [formato da música](/referencia/formato-da-musica.md), o
[plano DAW](/planejamento/daw.md) (sistema de design que este plano **evolui**, não substitui) e o
[plano de teclado](/planejamento/teclado.md) (modo simples + vim, paleta, which-key, hints).
Código lido: `crates/wadawko-core/src/{model,vocab,phrase,timeline}.rs`,
`crates/wadawko-core/src/ops/{sections,cells,measures}.rs`,
`crates/wadawko-app/src/views/grid/{mod,render,paint}.rs`, `views/{lane,score,right_panel,inspector}.rs`,
`theme.rs`, `shell/mod.rs` (playhead/follow/overlay), `crates/wadawko-audio/src/{schedule,engine}.rs`,
`main.rs` (`--demo probe`).

> **Original, não cópia.** O editor é *inspirado* no pattern editor do Renoise e nos trackers
> modernos (tempo para baixo, colunas por instrumento, sub-colunas de efeito, sequencer de
> patterns). Nenhum asset, nome ou tema deles é copiado; os comandos de efeito são nossos e
> pensados para taiko, não para síntese.

---

## 1. Pesquisa

Feita em 2026-11-18 em três frentes: o pattern editor do Renoise (manual oficial), os trackers
modernos e o tratamento de grupos/cores nas DAWs, e como apps de percussão representam
articulações por nota. Só o que serve a um grupo de taiko; nada de síntese.

### 1.1 Renoise — o que o pattern editor faz

- **Tempo na vertical**, linha a linha, de cima para baixo; cada pattern tem de 1 a 512 linhas
  ([Tracker Interface](https://tutorials.renoise.com/wiki/Tracker_Interface)). Numeração decimal
  ou hex por preferência ([Preferences](https://tutorials.renoise.com/wiki/Preferences)).
- **Colunas**: por track até 12 *note columns* e 8 *master FX columns*; cada note column tem as
  sub-colunas Note · Instrument · Volume (`00–80`) · Panning (`00–80`, `40` = centro) · Delay
  (`00–FF`: "divides a line into 256 equal parts", `80` = 50 % da linha) · Local FX; Note é
  sempre visível e as outras ligam/desligam pelos botões **Vol/Pan/Dly/FX** do painel
  ([Tracker Interface](https://tutorials.renoise.com/wiki/Tracker_Interface),
  [Pattern Editor](https://tutorials.renoise.com/wiki/Pattern_Editor),
  [Effect Commands](https://tutorials.renoise.com/wiki/Effect_Commands)).
- **Tamanho**: fonte do pattern Small/Normal/Big/Huge + H/V-Spacing, e escala da UI 100–350 %
  ([Preferences](https://tutorials.renoise.com/wiki/Preferences),
  [HiDPI](https://tutorials.renoise.com/wiki/HiDPI_and_UI_Scaling)); a altura em px não é
  documentada (≈ 14 px por linha a 100 % nas capturas). Usuários reclamam da fonte pequena
  ([fórum](https://forum.renoise.com/t/new-pattern-font-at-normal-size-100-scaling-isnt-ideal-the-old-one-was-better/58608)).
- **Destaque de linhas**: "Highlight every xx lines", por padrão igual ao **LPB** (lines per
  beat, default 4) — um nível só; não há marcação de compasso no editor (pedido no
  [fórum](https://forum.renoise.com/t/beat-bar-counter-in-pattern-editor/58111))
  ([Song Options](https://tutorials.renoise.com/wiki/Song_Options)).
- **Pattern Follow** (`Scroll Lock`): o cursor segue o play; desligado, edita-se livremente
  enquanto toca; preferência "Separate edit & play positions"
  ([Transport Panel](https://tutorials.renoise.com/wiki/Transport_Panel),
  [Recording and Editing](https://tutorials.renoise.com/wiki/Recording_and_Editing_Notes)).
  **Edit Mode** (`Esc`, borda vermelha) e **Edit Step** (linhas que o cursor pula após inserir;
  `Ctrl+0–9`, `Ctrl+-/+`; pode ser 0) ([Pattern Editor](https://tutorials.renoise.com/wiki/Pattern_Editor),
  [Keyboard Shortcuts](https://tutorials.renoise.com/wiki/Keyboard_Shortcuts)). **Block Loop**
  loopa uma fração do pattern (`Numpad Enter`).
- **Comandos de efeito**: `0` + letra + `xy` hex para sample/instrumento (`0Rxy` retrigger a
  cada `y` ticks com `x` = variação de volume por repetição — 1–5 baixa 3–50 %, 9–D sobe,
  6/7 e E/F cumulativos; `0Qxx` atrasa a linha `xx` ticks; `0Cxy` corta o volume após `y`
  ticks; `0Yxx` "maybe" = probabilidade; `0Uxx/0Dxx` slide; `0Gxx` glide; `0Ixx/0Oxx` fade;
  `0Bxx` reverse; `0Sxx` offset), `Z…` para globais (`ZTxx` BPM, `ZLxx` LPB, `ZBxx` break)
  ([Effect Commands](https://tutorials.renoise.com/wiki/Effect_Commands),
  [origem do prefixo 0](https://forum.renoise.com/t/new-pattern-fx-commands/34424)). Na coluna
  de volume/pan cabem versões de um dígito (`Rx`, `Qx`, `Cx`, `Yx`).
- **Pattern Sequencer**: lista vertical de patterns por número (mesmo número = mesmo conteúdo);
  nomes num slot ao lado; **Section Header** insere um título na sequência e clicar nele
  seleciona todos os patterns da seção — **sem cor** (pedido no
  [fórum](https://forum.renoise.com/t/suggestion-for-colouring-sections-of-sequences/61531));
  `+`/`-`/Clone; seleção múltipla por arraste ou `Shift`; arrastar move, `Ctrl` copia, `Alt`
  insere repetições; faixa de loop arrastando à esquerda dos números; *schedule* (clique =
  toca depois do atual; `Shift` monta playlist)
  ([Pattern Sequencer](https://tutorials.renoise.com/wiki/Pattern_Sequencer)).
- **Pattern Matrix**: patterns × tracks como blocos coloridos; **alias** = cópia idêntica
  editável em um lugar só (`Ctrl+P`/`Ctrl+U`); mute de bloco com `Alt`+clique; clonar
  arrastando a borda ([Pattern Matrix](https://tutorials.renoise.com/wiki/Pattern_Matrix)).
- **Seleção e transformações**: `Shift`+setas ou arraste; `F1/F2` transpõem, `F3–F6`
  cut/copy/paste/flip com modificador = escopo (seleção/coluna/track/pattern); **Advanced Edit**
  com *content mask* (notes/instr/vol/pan/delay/fx), Shrink/Expand, Mirror, Quantize, Nudge,
  Set/Add/Mul/**Humanize** e interpolação `Ctrl+I`/`Ctrl+L`
  ([Advanced Edit](https://tutorials.renoise.com/wiki/Advanced_Edit),
  [Tracker Interface](https://tutorials.renoise.com/wiki/Tracker_Interface)).
- **Cores**: swatch acima do nome da track abre o picker (grade, saturação, RGB/HSV, "apply
  to all"); 16 cores default no tema (`Default_Color_01..16` no `.xrnc`), salvas com a música;
  a Matrix usa a cor da track por bloco ([Pattern Editor](https://tutorials.renoise.com/wiki/Pattern_Editor),
  [tema .xrnc](https://raw.githubusercontent.com/dracula/renoise/master/mb-dracula-renoise-theme.xrnc)).
- **Atalhos**: setas; `PgUp/PgDn` 16 linhas; `Home/End`; `F9–F12` = 0/25/50/75 % do pattern;
  **`Tab`/`Shift+Tab` = próxima/anterior note column (com uma só, próxima track)**;
  `Ctrl+↑/↓` pattern ±; `Space` play; `Enter` toca a linha
  ([Keyboard Shortcuts](https://tutorials.renoise.com/wiki/Keyboard_Shortcuts),
  [fórum: Tab](https://forum.renoise.com/t/jump-to-next-column-and-jump-to-next-track/72434)).

### 1.2 Trackers modernos — o que os faz parecer modernos e acessíveis

| Tracker | O que vale copiar |
|---|---|
| **Polyend Tracker** ([manual 1.9.2](https://polyend-website.fra1.digitaloceanspaces.com/wp-content/uploads/2023/11/29122938/Polyend-Tracker-Manual-1v9v2a.pdf)) | 8 tracks, 4 visíveis por padrão, **um step = nota + instrumento + 2 slots de FX**; cada campo tem **cor própria** (nota teal, instrumento âmbar, FX1 magenta, FX2 azul); FX por step com **letra legível** (`V` velocity %, `m` micro-move 0–100 %, `R` roll com divisor e variantes de rampa, `C` chance, `r` reverse) — [lista](https://backstage.polyend.com/t/what-step-effects-are-available-in-the-polyend-tracker/262); temas claro/escuro e duas fontes; "so you don't have to learn hex codes" ([CDM](https://cdm.link/2020/03/polyend-tracker-is-here)) |
| **Dirtywave M8** ([manual 3.1.4](https://cdn.shopify.com/s/files/1/0455/0485/6229/files/m8_operation_manual_v20230630.pdf)) | Song → Chain → Phrase (16 steps); colunas `N V I FX1 FX2 FX3`; comandos de **3 letras** + hex (`RET` retrigger com rampa de volume, `DEL` atrasa a linha em ticks, `KIL` corta após n ticks, `CHA` chance, `HOP`), com **texto de ajuda no rodapé mostrando o decimal**; seleção em grade, interpolação da coluna; frases usadas em outro lugar marcadas com `*` |
| **Furnace** ([settings](https://tildearrow.org/furnace/doc/latest/2-interface/settings.html), [pattern](https://tildearrow.org/furnace/doc/latest/3-pattern/index.html)) | fonte do pattern com tamanho e escala da UI; **row highlight de dois níveis (tempo e compasso)** definidos na janela Speed; cor de canal por tipo/instrumento; estilos de cabeçalho de canal; `ECxx` note cut, `EDxx` note delay, `0Cxx` retrigger ([effects](https://tildearrow.org/furnace/doc/latest/3-pattern/effects.html)) |
| **OpenMPT** ([Patterns](https://wiki.openmpt.org/Manual:_Patterns), [Display](https://wiki.openmpt.org/Manual:_Setup/Display), [General](https://wiki.openmpt.org/Manual:_Setup/General)) | Edit Step; Follow Song; **Primary Highlight = linhas por compasso, Secondary = por tempo**; "Always center active row" e "Smooth pattern scrolling"; patterns vizinhos esmaecidos; cores de canal (rainbow/random); duplo clique na célula abre "Note Properties" com todos os comandos; Interpolate/Transpose/Amplify no menu de contexto |
| **SunVox** ([manual](https://warmplace.ru/soft/sunvox/manual.php), [changelog](https://warmplace.ru/soft/sunvox/changelog.txt)) | patterns como blocos numa timeline 2D; **clone** (`Ctrl+D`, edita-se em um lugar só) vs **detach** (`Ctrl+H`, vira cópia comum); mute/solo por pattern; o pattern tem ícone/nome em vez de cor |
| **Bitwig** ([tracks](https://www.bitwig.com/userguide/latest/intro_to_tracks), [arranger](https://www.bitwig.com/userguide/latest/the_arrange_view_and_tracks), [cue markers](https://www.bitwig.com/userguide/latest/playing_back_the_arranger/)) | **grupos aninhados** (`Ctrl+G`/`Ctrl+Shift+G`), pasta aberta/fechada; cor por track com paleta e **"x" limpa a cor para herdar**; cue markers nomeados e coloridos listados na aba *Sections*; Follow Playback com **"Scroll by pages" (padrão) ou "Continuously scroll" (playhead centrado)** |
| **Ableton Live 12** ([mixing](https://www.ableton.com/en/live-manual/12/mixing/), [clip view](https://www.ableton.com/en/live-manual/12/clip-view/), [arrangement](https://www.ableton.com/en/live-manual/12/arrangement-view/)) | Group Tracks aninháveis com fold; **"Assign Track Color to Grouped Tracks and Clips"**; solo parcial "meio colorido"; locators nomeados; Follow que **pausa ao editar/rolar e retoma ao parar/reiniciar** |

Sobre a experiência de tracker em geral: "working with arrows and keys is so much faster than
a mouse" e o hex é o maior obstáculo nos primeiros dias
([MusicRadar](https://www.musicradar.com/music-tech/its-unfamiliar-intimidating-and-seemingly-impenetrable-for-producers-raised-on-daws-like-ableton-live-but-it-can-unlock-a-whole-new-world-of-creativity-i-tried-a-music-tracker-and-it-rewired-my-brain-in-a-good-way));
trackers "read information up and down" e são "written with efficiency in mind"
([RBMA](https://daily.redbullmusicacademy.com/2012/11/synth-aesthesia-trackers)); "closer to a
writing tool than an instrument" ([Elektronauts](https://www.elektronauts.com/t/trackers-click-for-so-many-non-musicians-because-theyre-closer-to-a-writing-tool-than-an-instrument/245575)).
Para quem escreve kuchi shōga — que **é** texto — isso é um ponto a favor.

### 1.3 Articulações por nota em apps de percussão

| App | Acento | Ghost | Flam / grace | Roll | Micro-timing | Outros |
|---|---|---|---|---|---|---|
| [Hydrogen](https://hydrogen-music.org/documentation/manual/manual_en_chunked/ch09s03.html) | velocity contínua (barra colorida) | velocity baixa | — | — | **Lead/Lag** ±≈10 ms a 120 BPM, régua "Note Properties" (arrastar, roda) | **Probability** 0–1 |
| [GrooveScribe](https://raw.githubusercontent.com/montulli/GrooveScribe/master/js/constants.js) | `O` (`>`) | `g` (parênteses) | `f` flam = grace + acento; `d` drag = 2 graces | `b` buzz = 3 traços no stem | — | **um caractere por subdivisão**, sticking R/L paralelo ([help](https://raw.githubusercontent.com/montulli/GrooveScribe/master/gscribe_help.html)) |
| [MuseScore 4](https://handbook.musescore.org/notation/expressive-markings/tremolos-and-rolls) | `>` (`Shift+V`) | [parênteses na cabeça](https://handbook.musescore.org/notation/pitch/noteheads) | [acciaccatura](https://musescore.org/en/handbook/4/grace-notes) | tremolo (nº de traços = valor), buzz roll próprio | — | [sticking como texto](https://handbook.musescore.org/idiomatic-notation/percussion/other-percussion-notation) |
| [Guitar Pro 8](https://www.guitar-pro.com/docs/gp8/playeffects/dynamic-tone-effects/dead-ghost-notes) | acento/acento forte, velocity automática | parênteses, "very low dynamic" | [grace before/on the beat](https://www.guitar-pro.com/docs/gp8/playeffects/ornements-expressive-effects/grace-notes) | [tremolo 8ª/16ª/32ª](https://www.guitar-pro.com/docs/gp8/playeffects/touch-effects/tremolo-picking) | — | |
| Dorico | — | — | grace notes | "z on stem" = buzz via playing technique ([fórum](https://forums.steinberg.net/t/z-on-stem-tremolo-playback-with-bbc-so/689139)) | — | sticking = lyrics ([fórum](https://forums.steinberg.net/t/percussion-stickings/663951)) |
| [Elektron Digitakt/Rytm](https://www.elektron.se/wp-content/uploads/2024/09/AnalogRytmMKIIUserManual_ENG_OS1.70_231122.pdf) | VEL por trig | | | **RETRIG** rate 1/16–1/80 + LEN + VFAD (rampa) | **micro timing** por trig (1/384 de step — [bonedo](https://www.bonedo.de/artikel/elektron-digitakt-test)) | PROB %, trig conditions |
| [Roland TR-8S](https://static.roland.com/assets/media/pdf/TR-8S_Reference_eng04_W.pdf) | ACCENT por step + nível | **weak beat** (`Shift`+pad) | **FLAM** (sub step) com *Flam Spacing* 0–8 | **SUB STEP** 1/2, 1/3, 1/4 (ratchet) | nudge só global | PROB por step |
| [Novation Circuit](https://fael-downloads-prod.focusrite.com/customer/prod/downloads/Circuit%20Tracks%20User%20Guide%20Version%202%20-%20EN.pdf) | velocity 16 níveis | | | | **micro steps** 1/6 de step | probability, mutate |
| [Live 12](https://www.ableton.com/en/live-manual/12/editing-midi/) | velocity + **deviation** aleatória | | [Ornament](https://www.ableton.com/en/live-manual/12/midi-tools/): flam velocity, grace chance | | | **Chance** por nota (triângulo no canto) |
| [FL Studio](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/channelrack.htm) | velocity | | | **Rep** (note repeat) por step | **Shift** por step (até 1/16) | graph editor por propriedade |
| TJA / Taiko no Tatsujin ([spec](https://gist.github.com/KatieFrogs/e000f406bbc70a12f3c34a07303eec8b)) | **binário**: `3/4` = nota grande | — | — | roll = **intervalo** `5…8` (balão `7` com nº de golpes) | — | um caractere por subdivisão |

Kuchi shōga: "DON" forte/"don" normal/"do" curto; "doko/doro/dogo" = dois golpes rápidos;
"tsu/tsuku" = golpes leves (ghost); "ka/kara" na borda; "su" = pausa
([Wikipedia](https://en.wikipedia.org/wiki/Kuchi_sh%C5%8Dga),
[Seattle Kokon Taiko](https://seattlekokontaiko.org/?p=391),
[UC Riverside](https://gluckprogram.ucr.edu/media/1151/download)). O Stanford Taiko escreve
**maiúscula = forte, minúscula = fraco** e liga pares no mesmo tempo com hífen (`do-KO`,
`ka-DON`), com um chart de sticking R/L paralelo
([Hanabi](https://web.stanford.edu/group/stanfordtaiko/cgi-bin/hanabi.html)). Rulo = repetição
de doko/doro ou o **oroshi** (golpes cada vez mais rápidos até o rulo —
[glossário](https://www.taikocolorado.com/taiko-glossary)); **flam não aparece como notação
formal** em nenhuma fonte de kuchi shōga — o mais próximo é o par com acento no 2º golpe.

### 1.4 O que a pesquisa decide para nós

1. **Vertical com dois níveis de destaque** (tempo e compasso), como OpenMPT/Furnace — o Renoise
   só tem um e os usuários pedem o segundo.
2. **Sub-colunas ligáveis por linha** (nota · fx · dyn · kiai), como Vol/Pan/Dly/FX do Renoise,
   e **cor por tipo de campo** (Polyend) para o olho achar o fx sem ler.
3. **Comandos de efeito legíveis em letras com ajuda em texto** (Polyend, M8 com o decimal no
   rodapé), não hex — o hex é "o maior obstáculo" e não traz nada com 6 comandos.
4. Os comandos que todo mundo tem e que servem ao taiko: **velocity, retrigger/roll com rampa,
   delay/micro-timing, cut/choke, flam** (TR-8S/Live) e **humanize** (Renoise/EZdrummer). Chance/
   probabilidade e condições (Elektron, M8, Live) ficam de fora: um ensaio é determinístico.
5. **Tab = próxima linha (instrumento)**, Edit Step, Follow com Scroll Lock e dois modos de
   rolagem (por página / centrado — Bitwig e OpenMPT oferecem os dois).
6. **Seções com nome, cor e seleção em lista** à esquerda (o Renoise tem a lista mas não a cor;
   o Bitwig tem cue markers coloridos na aba Sections); **grupos aninhados com `Ctrl+G` e cor
   herdada** (Bitwig/Live), "x" para voltar a herdar.
7. Articulação **como atributo separado da sílaba** (Hydrogen, Live), não como tipo de nota
   (GrooveScribe, TJA): a sílaba continua sendo kuchi shōga puro e o fx é opcional e ignorável
   pelo app web.
8. Na partitura, as convenções já existem: `>` acento, parênteses = ghost, acciaccatura = flam,
   tremolo = rulo (MuseScore/Guitar Pro/GrooveScribe).

Lacunas: altura de linha do Renoise em px (não documentada); o fórum da Toontrack e as páginas
do Dorico bloqueiam acesso automatizado; não há notação formal de flam em kuchi shōga.

---

## 2. Modelo mental e decisões do editor vertical

### 2.1 Em uma frase

**O tempo corre para baixo; cada linha da tela é uma célula (uma subdivisão); cada instrumento é
uma coluna; os compassos se empilham, separados por uma barra, dentro de blocos de seção.** A
música continua sendo `compassos × linhas × células` ([modelo](/referencia/formato-da-musica.md)):
o editor vertical é só outra projeção do mesmo `Song` — como a esteira e a partitura. Trocar de
vista nunca muda o som (princípio 3 do [plano DAW](/planejamento/daw.md)).

Mapa entre o que existe hoje (grade horizontal) e o editor vertical:

| Hoje (grade) | Editor vertical (tracker) |
|---|---|
| linha = instrumento (`t`) | **coluna** = instrumento; um grupo de sub-colunas por linha: `nota` · `fx` · `dyn` · `kiai` |
| coluna = célula (`m`, `k`) | **linha** = célula; linhas de um compasso = `beats × subdivision` |
| régua `1 e 2 e` em cima | **gutter** à esquerda: `c12` no início do compasso, `2` nos tempos, `e & a` nas subdivisões |
| faixa de dinâmica/andamento acima do compasso | **coluna do compasso** (entre o gutter e a 1ª linha): chip `mf`, `<` `>`, `♩`, cunha vertical |
| cabeçalho de seção (28 px) por bloco | **banda de seção** (28 px) atravessando todas as colunas; blocos empilhados com `SEC_GAP` |
| rótulos de linha repetidos por seção | **cabeçalho de colunas fixo** no topo do editor (não rola com o tempo) |
| playhead = linha vertical; follow = página horizontal | playhead = **linha horizontal** sobre todas as colunas; follow = rolagem **vertical** (centrado ou por página) |
| zoom = largura da célula (`cw`) | zoom horizontal = **largura da coluna**; zoom vertical = **altura da linha** (o `row_h` de hoje muda de papel) |
| `Cur { m, t, k }` | **igual** — `t` passa a ser coluna, `(m, k)` a linha; cursor, seleção, registradores e vim não mudam |

### 2.2 Tamanhos (px) — "células maiores que o Renoise"

O Renoise desenha linhas de ~14 px com fonte de 10–11 px (§1.1); o nosso conteúdo é texto
("dokodoko", "karan!") que precisa ser lido a 1–2 m do monitor do ensaio. Valores com faixa viram
constantes em `views/grid/vgeom.rs` e preferências em `settings.json`.

| Token | Valor | Faixa / notas |
|---|---|---|
| `ROW_H` altura da linha (uma célula) | **28** | 20–44, passo 4 (`Ctrl+Alt+↑/↓`, `z k`/`z j`); 20 = modo compacto (texto 11 px); salvo em `tracker_row_h` |
| `COL_W` largura da sub-coluna de nota | **96** | 72–144, passo 8 (`Ctrl+roda`, `z i`/`z o`); "dokodoko!" (9 chars a 13 px ≈ 66 px) cabe até 72; salvo em `tracker_col_w` |
| `FX_W` sub-coluna de efeitos | 44 | chip de 40 px: `R4<`, `D+2`; oculta por padrão (§4) |
| `DYN_W` sub-coluna de dinâmica da linha | 36 | só nas linhas com dinâmica própria (como a sub-linha de hoje) |
| `KYAI_W` sub-coluna de kiai | 64 | só nas linhas com kiai (`Ctrl+Shift+K`) |
| `GUTTER_W` gutter | 48 | 2 px cor da seção · 8 px faixa de loop · 38 px números |
| `MEAS_W` coluna do compasso | 72 | chip `mf` 30×20 + `<` `>` 16×16; 2ª linha: chip `♩`/`rall.→90`; cunha vertical nos 10 px da direita |
| `COL_HEAD_H` cabeçalho de colunas (fixo) | 44 | barra de cor 3 px no topo · nome 12 bold · instrumento 10 · ícones 16×16 em hover · mini-medidor 3 px |
| `SEC_HEAD_H` banda de seção | 28 | igual a hoje (⠿ nome `2×` `8c` + botões em hover); grupo: banda de 22 px acima da 1ª seção dele |
| `SEC_GAP` | 20 | igual |
| texto da sílaba | **13 px** (`ROW_H ≥ 24`), 11 px abaixo | `ink`; CAIXA ALTA já é o acento — sem bold extra |
| texto do fx | 10 px | `accent2` sobre `paper_raise` R1 |
| texto do gutter | 11 px bold `ink` (compasso, tempo) / 10 px `ink_faint` (subdivisão) | `c12` no 1º row do compasso; `5/4` em 9 px só quando a fórmula muda |
| alvo mínimo de clique | 28 × 72 | ≥ 24 × 24; chips do compasso 16 × 16 com 4 px de folga |

Largura típica: gutter 48 + compasso 72 + 8 linhas × 96 = **888 px**. Janela de 1400 px com nav
rail (64), painel de seções (220) e painel direito (280) deixa 836 px → **"ajustar à largura"**
(ligado por padrão): a coluna encolhe até 72 px antes de aparecer rolagem horizontal
(8 × 72 + 120 = 696). Abaixo de 1280 px o painel de seções recolhe para 36 px (só swatches, §3.6).

### 2.3 Como cada coisa do modelo aparece

- **Subdivisões diferentes por compasso**: cada compasso tem `cell_count()` linhas; um compasso em
  semicolcheias é simplesmente mais alto que um em colcheias (altura de linha constante, como o
  Renoise com LPB fixo). Hoje a subdivisão é uma só para a música (`ops::measures::set_subdivision`),
  então na prática todos têm a mesma altura por tempo; se um dia a subdivisão for por compasso,
  nada muda no editor. Destaque de tempo = a 1ª linha de cada tempo tem fundo `cell_beat` (novo
  token, §6.1) e uma linha de 1 px `line_beat` em cima; barra de compasso = 2 px `line_bar`
  atravessando todas as colunas (o "highlight every N lines" do Renoise, só que derivado de
  `beats`/`subdivision`, não configurado).
- **Fórmulas diferentes** (`beats` por compasso): só mudam o número de linhas; o gutter mostra
  `5/4` em 9 px quando a fórmula difere da do compasso anterior (como na notação). A edição
  continua na barra de ferramentas (`compasso − 4/4 +`) e no inspector.
- **Dinâmica geral e andamento**: coluna do compasso (`MEAS_W`): 1ª linha do compasso = chip
  `mf` + `<` `>`; 2ª linha = chip de andamento (`♩` ou `rall.→90 ·2c`); crescendo/diminuendo =
  cunha **vertical** nos 10 px da direita da coluna, da 1ª à última linha do compasso (mesma
  função `paint_dyn`, girada). Clique abre os mesmos popovers (`Menu::Dyn`, `Menu::Tempo`).
- **Dinâmica por linha** (`trackDynamics`): sub-coluna `dyn` (36 px) na linha, só quando ela tem
  marcações ou foi ligada (`track::ToggleDynRow`), com chip na 1ª linha do compasso e cunha vertical.
- **Kiai** (`Measure.kyai`): sub-coluna `kiai` (64 px) com uma sílaba por linha em `accent`
  sobre `accent_wash`, cursor com `Lane::Kyai` como hoje.
- **Donos das células** (`cellOwners`, linha compartilhada): listra de **3 px à esquerda** da
  célula na cor da pessoa (hoje é 3 px no topo). No cabeçalho da coluna, uma bolinha de 8 px por
  pessoa ao lado do nome; os "trilhos de pessoa" de hoje (3 px verticais à esquerda dos rótulos)
  viram uma **faixa de 2 px sob a barra de cor** do cabeçalho, contínua entre colunas vizinhas da
  mesma pessoa (as colunas já ficam na ordem das linhas; a página Pessoas continua agrupando).
- **Golpes dentro da célula** (`doko` = 2 golpes, offsets 0/0,5): uma **barra vertical de 4 px**
  à esquerda do texto, fatiada por offset — `don` = uma barra inteira; `doko` = duas metades
  empilhadas (em cima o 1º golpe). Couro `accent`, borda `accent2`, alfa 0,45–1 por velocity
  (como as barras de hoje, giradas). sfz = quadrado 4×4 `accent` no canto superior direito
  (igual). Sílaba inválida = contorno `danger` (igual).
- **Mudo/solo/pessoa inaudível**: véu `bg` 60 % sobre a célula (igual).
- **Playhead**: linha de 2 px `playhead` no topo da linha audível + wash `accent` 12 % na linha
  inteira (todas as colunas) + triângulo ▶ no gutter; pintado no overlay do Shell como hoje
  (`Shell::playhead` passa a devolver `y`). Flash da célula = contorno 1 px + wash 35 % (igual).
- **Follow** (`settings.follow`): `"center"` (padrão, estilo Renoise/Taiko no Tatsujin): tocando,
  a linha do playhead fica fixa a **40 %** da altura visível e o conteúdo rola por baixo, suave,
  a cada frame; `"page"`: vira a página quando o playhead passa de 85 % (o comportamento de hoje,
  em y); `"off"`. Toggle `view::ToggleFollow` (`Scroll Lock`, como o Renoise; `space v f`), chip
  `⤓` na barra de abas. Follow pausa durante um arrasto de seleção/bloco e volta ao soltar.
  **Custo**: o follow vertical é só `scroll.set_offset` — não re-renderiza as seções em cache (hoje
  o follow horizontal muda `grid.offset`, que entra na chave de todas as seções).
- **Zoom**: `Ctrl+roda` = largura da coluna (ancorado no ponteiro); `Ctrl+Alt+↑/↓` = altura da
  linha; `z 0` volta aos padrões (96 / 28). Roda = rola no tempo; `Shift+roda` = rola colunas.
- **Rolagem horizontal** (músicas com 10+ linhas ou muitas sub-colunas): o cabeçalho de colunas
  é fixo no topo mas compartilha o `offset_x` do conteúdo (mesma view, dois elementos irmãos).

### 2.4 Uma linha-tempo ou várias? (decisão)

Alternativas consideradas: (a) **um compasso = uma linha de blocos, seções lado a lado** (matriz
do Renoise) — perde a leitura contínua da música; (b) **tudo numa coluna só, sem bandas** (OpenMPT
com order list) — perde a estrutura visível, que para ensaio é o mais importante; (c) **blocos de
seção empilhados, cada um com as suas linhas** (escolhida). A (c) mantém a `SectionView` em cache
por seção ([arquitetura](/arquitetura/app.md)): cada seção é um bloco vertical com banda +
canvas; a chave de cache ganha `col_w`, `fx_cols`, `layout`; um hover na banda re-renderiza só
aquela seção, como hoje.

---

## 3. Painel de seções à esquerda

### 3.1 Onde e o quê

Painel de **220 px** sobre `chrome`, entre o nav rail e o editor (`Ctrl+Shift+B` mostra/esconde;
`view::ToggleSections`), com dois blocos de cabeçalho de painel (22 px): **SEÇÕES** (+ botão `+`)
e **TRECHOS** (o bloco de trechos de ensaio sai do painel direito e vem para cá — esquerda =
*estrutura e navegação*, direita = *propriedades*; o inspector e as músicas ficam à direita).
Cada seção é uma linha de 24 px: `▾/▸` (só em grupo) · swatch 10×10 na cor · nome 12 px ·
`2×` (ritornello, `ink_soft`) · `8c` (compassos, `ink_faint`) · ícone de trecho (`⟲`, se houver
trecho começando nela). Estados: seção do cursor = nome `ink` bold + barra esquerda 2 px `accent`;
seções selecionadas (região de playback) = `accent2_wash` + contorno 1 px `accent2`; seção
tocando = `▶` `accent` à esquerda do nome, com `2/3` (volta atual) em `ink_faint`; trecho de loop
(`tp.range`) = colchete fino `accent2` à direita, cobrindo as seções que ele atravessa.

```
┌──────────────────────┐
│ SEÇÕES            +  │  ← panel_header 22 px
│ ▾ ■ Intro        16c │  ← grupo (22 px, caixa alta 10 px, swatch, total de compassos)
│   ■ Chamada   2×  8c │
│ ▶ ■ Jiuchi        8c │  ← tocando (▶ accent), selecionada (wash accent2)
│ ▾ ■ Parte B      24c │
│   ■ B1            8c │
│   ■ B2        2×  8c ⟲│  ← tem trecho de ensaio
│   ■ Solo ōdaiko   8c │
│   ■ Final         4c │  ← sem grupo (nível raiz)
│                      │
│ TRECHOS           +  │
│ ⟲ entrada do B  c9–10│
│ ⟲ solo ōdaiko  c25–32│
└──────────────────────┘
        220 px
```

### 3.2 Interações (mouse)

| Gesto | Efeito |
|---|---|
| clique | seleciona **só** essa seção (região de playback — mesma semântica do clique na banda hoje) **e** rola o editor até ela (`ensure_visible`) |
| `Ctrl+clique` | junta/tira da seleção (igual hoje); `Shift+clique` = intervalo |
| clique na selecionada única | limpa a seleção (igual hoje) |
| duplo clique no nome | renomear (prompt do Shell, `InputTarget::SectionName`) |
| clique no swatch | popover de cor (§5.3) |
| arrastar ⠿ (ou a linha inteira) | reordenar — `ops::sections::move_section` (leva os compassos); soltar sobre um grupo põe dentro dele; soltar numa borda de grupo tira |
| `Alt+arrastar` | clona e solta na posição (`clone_section` + `move_section`) |
| botão direito | menu: renomear · cor ▸ · ritornello −/+ · duplicar · copiar · colar depois · agrupar · desagrupar · tocar só esta · guardar trecho · remover |
| hover | botões `⧉` `✕` aparecem (opacidade 0 → 1, como na banda) |

### 3.3 Teclado no painel (contexto `Sections`, quando o painel tem o foco)

`↑`/`↓` (vim: `j`/`k`) move o realce · `Space` alterna seleção · `Enter` leva o cursor do editor
para o 1º compasso da seção · `F2` / `r` renomear · `Ctrl+C`/`Ctrl+X`/`Ctrl+V` (vim: `y`/`d`/`p`)
copiar/recortar/colar **seções** · `Ctrl+D` duplicar · `Delete`/`x` remover · `Alt+↑/↓` mover ·
`Ctrl+G` agrupar as selecionadas / `Ctrl+Shift+G` desagrupar · `←`/`→` (`h`/`l`) recolher/expandir
grupo · `+`/`-` ritornello · `Esc` volta o foco ao editor. O painel entra no salto por rótulos
(`Ctrl+Space`: cada seção é um alvo `sec-{i}`). Da grade: `space s g` agrupa, `space s shift-g`
desagrupa, `(`/`)` continuam andando por seção.

### 3.4 Copiar e colar seções

A área de transferência recebe **dois formatos** ao copiar: texto TSV (as células, para
planilha/web, como hoje) e, como texto principal, um JSON `{"wadawko":"sections","v":1,
"sections":[{name, repeat, color, measures:[Measure…]}]}`. Colar no painel insere as seções
**depois** da realçada (com `(cópia)` no nome se o nome já existir na música), renumera `start`
e preserva dinâmicas, kiai, donos e fx. Como é texto, funciona **entre músicas** (copiar numa,
trocar de música, colar) e até entre máquinas. Colar na grade (não no painel) continua colando
células.

### 3.5 Grupos de seções — decisão: organização, navegação, cor e região; **não** mudam a ordem tocada

Modelo (campos opcionais, compatíveis — o app web preserva chaves desconhecidas em `Song` e em
`Section`):

```json
"sectionGroups": [{ "id": "grp-1", "name": "Parte B", "color": "#5e9fd8", "collapsed": false }],
"sections": [{ "id": "sec-4", "name": "B1", "start": 16, "group": "grp-1" }, …]
```

Regras: um grupo é uma **sequência contígua** de seções (na ordem por `start`); agrupar seleções
não contíguas move as seções para ficarem juntas (com confirmação na status bar: "3 seções movidas
para o grupo"); grupo vazio some; `collapsed` é estado de UI salvo no arquivo (como `repeat`).
O grupo aparece (a) no painel como nó recolhível com swatch e total de compassos, (b) no editor
como banda de 22 px acima da 1ª seção dele (nome em caixa alta 10 px + cor) e como continuidade
da linha de cor no gutter (seção sem cor herda a do grupo, §5.2), (c) no modo projeção
("Parte B · B2") e na status bar. "Tocar o grupo" = selecionar todas as seções dele (região),
que é o que o clique no nó faz.

Por que **não** dar `repeat` ao grupo (nem outra influência na ordem tocada) agora:

1. O ritornello já existe por seção; "A B A B" se escreve com duas seções clonadas ou, no futuro,
   com seções por referência (F17 do [plano DAW](/planejamento/daw.md)) — essa sim é a feature
   que muda a ordem, e merece o próprio plano.
2. Repetição aninhada mexe em `timeline::play_order`, `TimeMap::fold_tick` (o playhead dobrado),
   nas âncoras dos trechos de ensaio (`PracticeAnchor` conta compassos dentro da seção, não do
   grupo), no export MIDI e no modo projeção — e confunde o usuário ("por que tocou 6 vezes?").
3. **Compatibilidade silenciosa**: o app web ignora `sectionGroups`; se o grupo mudasse a ordem,
   o mesmo arquivo soaria diferente nos dois apps. Como organização + cor, o arquivo abre igual
   lá e só perde a cor.

Se um dia for preciso, `SectionGroup.repeat` entra como campo opcional com a mesma semântica do
da seção (pergunta aberta Q2, §10).

### 3.6 Relação com trechos, loop e região; janela estreita

- **Região** (`tp.selected`, seções) e **trecho de loop** (`tp.range`, compassos) continuam como
  hoje, com o trecho tendo precedência (`AppState::set_range`); o painel mostra os dois.
- **Trechos de ensaio** (`Song.practice`): o bloco TRECHOS muda de painel; clicar continua
  pondo em loop; cada trecho mostra `c9–10`; seções com trecho começando nelas ganham `⟲`.
- Abaixo de 1280 px de janela, o painel recolhe para **36 px**: só os swatches (10×10) empilhados
  com a inicial do nome, tooltip com o nome; clique seleciona e rola; hover expande o painel
  (220 px) por cima do editor até o mouse sair. Com `Ctrl+Shift+B` ele some de vez.
- O painel é uma view **em cache** no `shell/chrome.rs` com chave = (seções, grupos, cores,
  selecionadas, seção do cursor, seção tocando + volta, `range`, trechos, hover, foco). Tocando,
  a seção tocando entra na chave arredondada ao compasso, não ao frame.

---

## 4. Efeitos por célula estilo Renoise, para taiko

### 4.1 O conjunto (6 comandos)

Critério: só o que um grupo de taiko marca na partitura ou pede no ensaio, que o motor consegue
tocar com o que já tem (velocity, offset na célula, choke, humanização) e que lê a 1 m de
distância. Nada de pan, arpejo, slide ou sample offset — isso é síntese. Humanização e pan
**por linha** ficam no mixer (F12/F13 do backlog), não por célula.

| Comando | Sintaxe | O que faz no som | Notação (partitura) |
|---|---|---|---|
| **Velocity** | `V0`–`V9` | multiplica a velocity de todos os golpes da célula por `n/10` (`V0` = ghost, 5 %); além da CAIXA ALTA (×1,5) e do `!` (sfz) | `V0`–`V2` = nota entre parênteses (ghost); `V8`–`V9` = `>` |
| **Rulo / retrigger** | `R2`–`R9`, opcional `<` ou `>` | repete o 1º golpe `n` vezes, igualmente espaçados na célula (`R4` numa colcheia = 4 fusas), mãos alternando a partir da mão da sílaba; `<` rampa 0,6→1,0 de velocity, `>` 1,0→0,6 | tremolo (3 traços) na nota; `<`/`>` viram cunha curta |
| **Flam** | `F`, `F1`–`F3` | um golpe de apoio (**grace**) antes do principal, na mão oposta, velocity 50 %, a `n/16` da célula antes (padrão `F` = `F2` = 1/8 da célula ≈ 31 ms a 120 BPM numa colcheia); o grace nunca cai antes do início da célula anterior. É o menos idiomático dos seis (o kuchi shōga não tem notação de flam, §1.3) — se for preciso cortar um, é este | acciaccatura (nota pequena com traço) |
| **Atraso / antecipação** | `D-5`–`D+5` | desloca todos os golpes da célula em `n/10` da duração dela ("laid back" = `D+2`); o Renoise faz o mesmo com a coluna *delay* 00–FF; antecipar não passa do início da célula anterior | nada (ou seta ↓/↑ 9 px opcional) |
| **Corte / abafar** | `X`, `X1`–`X9` | abafa as vozes da linha (couro e borda) a `n/10` da célula (`X` = `X5`): fade de 20 ms, como o choke que o couro já faz nos golpes anteriores — o "pressionar a pele" do shime, uma nota curta | ponto de staccato |
| **Humanização** | `H0`–`H9` | escala a humanização do engine (±6 ms, ±12 % ganho, ±1 % pitch) de 0× a 1,8× nesta célula (`H5` = padrão; `H0` = exata, para uníssonos que precisam soar como um) | nada |

Até **3 comandos por célula**, separados por espaço, qualquer ordem (`R4< D+1`). Maiúsculas e
minúsculas valem igual (`r4` = `R4`); o app grava em maiúsculas. Combinações: `R` + `V` multiplica;
`R` + `F` = o flam vale só para o 1º golpe do rulo; `D` + `X` = o corte acompanha o deslocamento.

Por que letras e não hex: o Renoise usa `0Rxy` porque herdou o MOD/XM e precisa de 256 valores;
aqui há 6 comandos com ≤ 10 níveis, e quem lê a célula é um taiko player, não um programador —
`R4<` se lê "rulo de 4 crescendo". Letras também cabem na paleta ("Efeito: rulo ×4") e no
which-key (`space f r`).

### 4.2 Formato e compatibilidade

Campo novo e opcional no compasso, **paralelo a `cells`** (como `cellOwners` e `kyai`):

```json
"fx": { "t2": [null, "R4<", null, "D+2 X"] }
```

- Ausente = sem efeitos; linha ausente no mapa = sem efeitos nessa linha; `null` = célula sem fx.
- O app web preserva `fx` intacto (chave desconhecida em `Measure.extra`) e toca a sílaba
  **sem** o efeito — é o comportamento certo: a música não quebra, só perde o ornamento.
- `ops::cells::set_fx(song, track_id, mi, ci, Option<&str>)` normaliza (maiúsculas, dedupe por
  letra, ordem canônica `V R F D X H`) e remove o mapa quando fica todo `null`; `clear_fx`,
  `fx_at`. Mover/clonar/apagar células (`set_cell`, registradores, drag, `move_section`,
  `clone_section`, mudar fórmula/subdivisão) **levam o fx junto** — o fx vive na mesma estrutura
  paralela que `cellOwners`, que já tem esse comportamento.
- `wadawko_core::fx`: `parse(&str) -> Result<Vec<Fx>, FxError>` (com memo como `resolve_cell`),
  `format(&[Fx]) -> String`, e **`apply(res: &CellResolution, fx: &[Fx]) -> Vec<ResolvedHit>`** —
  a função única que grade, esteira, partitura, MIDI e áudio usam, para que todos concordem.
  `ResolvedHit` ganha `grace: bool` (flam) e `choke_at: Option<f64>` sai como um item próprio
  (`CellFx::Cut { at }`). Testes: tabela por comando (offsets, velocities, mãos), limites
  (`R9` numa célula de fusa, `D-5` na 1ª célula da música, `F` + `D-5`), parse de erros.

### 4.3 Impacto no `schedule.rs` e no engine

- No `compile`, depois de `resolve_cell(syl)`: `let hits = fx::apply(&resolved, &fx_of(meas, track, ci))`;
  o loop de golpes fica igual (offset → `swing_warp` → tick). Golpes extras (rulo, grace) ganham
  `hit` índices seguintes (`HitEv.hit: u8` aguenta 255); `HitEv.cell` continua a célula-fonte,
  então o flash da célula pisca a cada golpe do rulo (bom feedback).
- `HitEv.humanize: bool` → `humanize: f32` (0 = exato, 1 = padrão); o engine multiplica
  `HUMANIZE_*` por ele (`engine.rs:937`).
- `EventKind::Cut { strip_pair: u16 }` (novo): o engine chama `Voice::choke(0, 20 ms)` em todas
  as vozes do canal (as duas strips). Sem alocação, mesmo caminho do choke atual.
- O **swing** continua valendo: o fx mexe no offset dentro da célula **antes** do `swing_warp`,
  como os offsets 0/0,5 das sílabas compostas.
- Esteira (`lane.rs::note_events`) e partitura (`wadawko-notation`) passam a chamar `fx::apply` —
  um ponto de mudança cada; o MIDI sai do schedule, herda de graça. Teste de regressão:
  render offline de uma música sem fx produz o mesmo hash de antes.

### 4.4 Como aparece

- **Sub-coluna `fx`** (44 px) por linha, ligada/desligada por linha (`⚙` → "efeitos") ou para
  todas (chip `fx ▢` na barra de abas; `space v x`; `settings.fx_columns`). Oculta por padrão
  em músicas sem fx; **aparece sozinha** na 1ª vez que um fx é gravado numa linha. Cada célula
  da sub-coluna mostra o texto do fx em 10 px `accent2` num chip `paper_raise` R1 (linhas vazias:
  `·` em `ink_faint` só nas linhas de tempo). Fx inválido = chip com contorno `danger`.
- **Com a sub-coluna oculta**, a célula de nota mostra o 1º comando em 9 px `accent2` encostado
  na direita (`don    R4<`), para o fx nunca ficar invisível.
- **Inspector**, bloco CÉLULA: linha "efeitos" com um chip por comando (`R4<` com `−`/`+` no
  parâmetro, `×` remove) e `+ ▾` (menu dos 6, com presets: "rulo ×4", "rulo ×8 cresc.", "flam",
  "atrasar +20 %", "corte", "exato"); texto completo editável pelo prompt (`InputTarget::CellFx`).
- **Célula** (wireframe em §6.4).

### 4.5 Como se edita

- **Mouse**: duplo clique na célula fx = editar o texto no lugar (mesmo buffer `edit` da grade,
  com `Lane::Fx`); botão direito na célula de nota → "efeitos ▸" com os presets; arrastar
  vertical num chip de fx com `Alt` = ±1 no parâmetro (como um slider), opcional.
- **Teclado, modo simples**: `→` entra na sub-coluna fx (se visível) e letras/dígitos digitam
  no buffer (`R`, `4`, `<`, `Enter`); na célula de nota, atalhos diretos que agem na célula ou
  na **seleção**: `Ctrl+Shift+R` cicla o rulo (0 → 2 → 3 → 4 → 6 → 8 → 0), `Ctrl+Shift+F` flam,
  `Ctrl+Shift+,` / `Ctrl+Shift+.` atraso −/+, `Ctrl+Shift+X` corte, `Ctrl+Shift+V` velocity
  −1 (cicla 9→0), `Ctrl+Shift+0` limpa os fx.
- **Vim**: prefixo leader `space f` ("efeitos", entra em `PREFIX_TITLES`): `space f r` rulo
  (com contagem: `4 space f r` = `R4`), `space f shift-r` rulo cresc., `space f f` flam,
  `space f d`/`space f shift-d` atraso +/−, `space f x` corte, `space f v` velocity (contagem),
  `space f h` exato (`H0`), `space f c` limpa, `space f e` edita o texto; todos funcionam em
  visual (na seleção). `gf` = ir para a sub-coluna fx da célula (e `gn` volta à nota). Em insert,
  `Tab` dentro de uma célula vai nota → fx → próxima linha (como o Renoise).
- **Paleta**: "Efeito: rulo ×4", "Efeito: flam", "Efeito: limpar"… (ações `fx::*` em
  `keys/actions.rs` + `registry.rs`).

---

## 5. Cores

### 5.1 Três níveis para a linha: linha → instrumento → padrão do base

Hoje `track_color(track)` = `Track.color` → `default_track_color(base)` ([F1](/planejamento/daw.md)).
Entra o **instrumento do Kit** no meio:

```
cor da linha = Track.color ?? InstrumentDef.color ?? default_track_color(def.base)
```

- `InstrumentDef.color: Option<String>` (novo, opcional; builtins sem o campo caem na tabela por
  base, então nada muda em arquivos antigos). Na página **Kit**, cada cartão de instrumento ganha
  a linha "cor" com os 12 swatches + "padrão" + campo hex (prompt `InputTarget::Color`).
- A assinatura passa a `theme::track_color(song, track)` (precisa de `resolve_instrument`); o
  mixer, a esteira, o cabeçalho de coluna, o director e o inspector já chamam `track_color`.
- O popover `⚙` da linha e o inspector mostram a origem: "cor: do instrumento (●)" ou "própria";
  "padrão" na linha volta a herdar do instrumento (limpa `Track.color`).
- Trocar a cor do instrumento recolore todas as linhas que **herdam** dele — e só elas.

### 5.2 Seções e grupos

`Section.color` e `SectionGroup.color` (hex, opcionais). Resolução: `Section.color ??
group.color ?? nenhuma` (sem cor = `paper` neutro, como hoje). Paleta própria de **8 cores de
estrutura**, mais suaves que as 12 das linhas para não competir com elas (as linhas vivem nos
cabeçalhos de coluna; as seções, no painel e nas bandas):

`#d9775f` terracota · `#d9a441` mostarda · `#8fb86a` sálvia · `#4fb3a0` teal · `#5e9fd8` azul ·
`#8c86d6` lavanda · `#c97aa8` rosa · `#9a8f84` pedra — `SECTION_COLORS` em `wadawko-core::model`,
com o mesmo teste de contraste ≥ 4,5 com `Theme::on_track_color()` nos dois temas (ajustar o hex
na implementação se algum falhar, como foi feito com o roxo das linhas). Campo hex livre também.

Onde a cor da seção aparece: swatch e barra no painel; **barra de 4 px à esquerda da banda** no
editor (o nome fica `ink`, nunca na cor — contraste); **linha de 2 px no gutter** ao longo de
todas as linhas da seção (sabe-se em que seção se está mesmo com a banda fora da tela); swatch
antes do nome da seção na status bar e no modo projeção; blocos do mapa recolhido (36 px).

### 5.3 Picker, contraste e temas

- Um só componente `ui::color_picker(current, palette, on_pick)`: swatches 14×14 R1 em grade
  (12 ou 8), "padrão/herdar", e "outra…" (hex via prompt; validado por `parse_color`). Usado pelo
  `⚙` da linha, pelo Kit, pelo painel de seções e pelo inspector. Os swatches são alvos de salto
  (`col-…`, como hoje).
- **Nunca** cor como fundo de célula (regra 4 do plano DAW). Cor de linha = barra de 3 px no topo
  do cabeçalho + wash 18 % no cabeçalho; cor de seção = barra 4 px + gutter 2 px; cor de pessoa =
  listra 3 px na célula e bolinha no cabeçalho.
- Temas: as paletas são escolhidas para os dois fundos (`#262626` e `#ececec`); texto sobre cor
  preenchida é sempre `on_track_color()` (escuro); washes a 18 % sobre `paper`. O teste
  `themes_meet_contrast_and_are_opaque` ganha as paletas novas.

---

## 6. Design "mais moderno" — evolução do sistema DAW

### 6.1 O que muda nos tokens e componentes

Tudo do [plano DAW §2](/planejamento/daw.md) continua (cinzas neutros em níveis, R0/R1, uma cor
de ligado, seleção azul, densidade). O "moderno" vem de **menos cromo e mais conteúdo**: menos
bordas, estados por luminância e wash, hierarquia por alfa, movimento só onde informa.

| Item | Mudança |
|---|---|
| tokens novos | `cell_beat` (1ª linha de cada tempo: escuro `#2e2e2e`, claro `#e9e9e9`), `col_head` (fundo do cabeçalho de colunas: escuro `#222222`, claro `#f0f0f0`), `panel_sel` (= `accent2_wash`, apelido semântico) |
| bordas | células **sem** contorno (só a fresta de 1 px `bg` e as linhas de tempo/compasso); bordas 1 px `line` só em painéis e popovers |
| cabeçalho de colunas | 44 px, fundo `col_head`, barra de cor 3 px no topo, nome 12 bold, instrumento 10 `ink_faint`, ícones `⚙ M S ★ ×` 16×16 R1 só em hover (os ligados ficam sempre: M `warn`, S `accent2`, ★ `accent`), mini-medidor 3 px no rodapé (overlay) |
| banda de seção | 28 px `paper`, barra de cor 4 px, nome 12 bold, `2×` e `8c` em `ink_soft`; botões em hover como hoje; grupo = banda de 22 px `header` com caixa alta |
| hover de linha-tempo | a linha sob o mouse ganha wash `ink` 6 % em todas as colunas — pintado no **overlay** (geometria do último frame), sem re-render |
| cursor | contorno 2 px `accent` R0 (igual); em normal/visual + wash 18 % (igual) |
| seleção | `accent2` 16 % + contorno 1 px (igual); em colunas inteiras (`Ctrl+V` = bloco de linhas) o cabeçalho da coluna ganha sublinhado 2 px `accent2` |
| tipografia | sílaba 13 px (12 → 13: cresce a legibilidade a distância); fx 10 px; gutter 11/10; os demais iguais |
| profundidade | zero sombras exceto popovers (`shadow_sm`); painel de seções e inspector no nível `chrome`; editor `bg`; bandas e cabeçalhos `paper`/`col_head` — profundidade por luminância |
| animações | 1) rolagem do follow centrado (por frame, sem easing — segue o relógio); 2) bloco arrastado com mola (já existe); 3) botões de hover aparecem com opacidade (já existe, 0 → 1 instantâneo; passa a 80 ms); 4) recolher grupo/seção: sem animação (um frame); 5) toast some em fade (já existe). Nada pisca; nada se move sem causa |
| ícones | continuam glifos monocromáticos até haver `AssetSource` para SVG (fora do escopo) |

Regras de desempenho que valem (de [arquitetura](/arquitetura/app.md)): uma `SectionView` em
cache por seção, chave `sections_key` + `(hover_sec)`; cabeçalho de colunas e painel de seções são
views em cache com chave própria; playhead, flash, hover de linha, mini-medidores e loop-drag no
overlay sem cache; nunca `window.refresh()`; medir com `--demo playstat-tracker` e `hoverstat`.

### 6.2 Wireframe — tela principal (1400 px)

```
┌────┬──────────────┬──────────────────────────────────────────────────────────────┬────────────┐
│    │ 太 ☰ Matsuri      ⏮ ▶ ↺  c2·1.2  BPM 180  reto|swing            🎵 ⏱ 🏃  ☾   │ ← 44 chrome
│ ▦  ├──────────────┼──────────────────────────────────────────────────────────────┼────────────┤
│Edi │ SEÇÕES    +  │ editor  partitura  esteira        ⤓ follow  passo 1  fx ▢   │ INSPECTOR  │ ← 28
│tor │ ▾ ■ Intro 16c│──────────────────────────────────────────────────────────────│ Chamada ·  │
│    │   ■ Chamada  │ editar|selecionar  colcheia|…  compasso −4/4+  +seção +linha │ c2 · 1.2   │ ← 34
│ ◉  │ ▶ ■ Jiuchi   ├──────┬──────┬───────────┬─────────────┬───────────┬──────────┤            │
│Pes │ ▾ ■ Parte B  │      │comp. │▀▀▀Shime   │▀▀▀Chū 1  ●  │▀▀▀Chū 2 ●●│▀▀▀Ōdaiko │ SEÇÃO      │ ← 44 fixo
│    │   ■ B1    8c │      │      │ shime     │ chū         │ chū  M S ★│ ōdaiko   │ Chamada 2× │
│ ≡  │   ■ B2    8c⟲├──────┴──────┴───────────┴─────────────┴───────────┴──────────┤ 8 compassos│
│Mix │   ■ Solo  8c │▌⠿ Chamada  2×  8c                          (hover: ⧉ ✕ −/+)  │            │ ← 28 banda
│    │ ■ Final   4c │▌c1  │mf < >│ ten       │ DON         │           │ DON      │ COMPASSO   │ ← 28 linha 1.1 (cell_beat)
│ ◎  │              │▌    │ ♩    │ teke      │ tsu         │           │          │ 4/4 mf < > │   1.2
│Kit │ TRECHOS   +  │▌2   │      │ ten       │ don         │ DON       │          │ ♩ a tempo  │   2.1 (cell_beat)
│    │ ⟲ entrada B  │▌    │      │ teke      │ ku          │           │          │            │   2.2
│ ⚙  │ ⟲ solo ōdaiko│▌3   │      │ ten       │ DON    R4<  │           │ doron    │ LINHA      │
│    │              │▌    │      │ teke      │             │ don       │          │ Chū 1 ■ ▾  │
│    │              │▌4   │      │ ten       │ don    F    │ don       │          │ chū · M S ★│
│    │              │▌    │      │ teke      │ ka          │           │          │            │
│    │              │▌━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━│ CÉLULA     │ ← barra de compasso 2 px
│    │              │▌c2 ▶│ f    │ ten       │ DON         │ DON       │ DON!     │ DON · R4<  │ ← playhead (wash)
│    │              │▌    │      │ teke      │ tsu         │           │          │ efeitos:   │
│    │              │▌2   │      │ ten       │ don         │ DON       │          │ [R4<][+ ▾] │
│    ├──────────────┴─────┴──────┴───────────┴─────────────┴───────────┴──────────┴────────────┤
│    │ NORMAL │ Chamada · c2 · 1.1 · Chū 1 │ 180 BPM │ tocando 1/2 │ ↺ c1–c2 │ passo 1 │ gravado│ ← 22
└────┴──────────────────────────────────────────────────────────────────────────────────────────┘
  64       220        48    72      96          96+44        96          96              280
```

O gutter (`▌c1`, `2`, `3`…) carrega a cor da seção nos 2 px da esquerda e a faixa de loop nos
8 px seguintes (banda `accent2` quando há trecho, alças 6×6 nas pontas). As linhas `1.1`, `2.1`…
(início de tempo) têm fundo `cell_beat`; as subdivisões, `cell_empty`.

### 6.3 Wireframe — painel de seções com grupo recolhido e menu de contexto

```
┌──────────────────────┐        ┌──────────────────────┐
│ SEÇÕES            +  │        │ renomear          F2 │
│ ▸ ■ Intro   2  16c   │        │ cor               ▸  │
│ ▶ ■ Jiuchi      8c   │        │ ritornello   −  2× + │
│ ▾ ■ Parte B    24c   │        │ duplicar      Ctrl+D │
│   ■ B1          8c   │  ───▶  │ copiar / colar depois│
│   ■ B2      2×  8c ⟲ │  botão │ agrupar       Ctrl+G │
│   ■ Solo ōdaiko 8c   │  dir.  │ desagrupar           │
│ ■ Final         4c   │        │ tocar só esta        │
│                      │        │ guardar trecho…      │
│ TRECHOS           +  │        │ remover         Del  │
│ ⟲ entrada do B c9–10 │        └──────────────────────┘
└──────────────────────┘
```

### 6.4 Wireframe — células com efeitos (linhas 1.1 e 1.2 da coluna Chū 1, `ROW_H` 28)

```
   gutter  comp.      nota (96)               fx (44)
  ┌──────┬──────┬────────────────────────────┬────────┐
  │▌c1   │mf < >│▐▮ DON                    ▪ │ R4<    │  1.1  ▐ = dono (3 px, cor da pessoa) · ▮ = golpe (4 px, couro) · ▪ = sfz 4×4
  ├──────┼──────┼────────────────────────────┼────────┤
  │▌     │ ♩    │ ▮ doko               D+2   │ D+2    │  1.2  ▮ dividido em 2 (don em cima, ko embaixo); "D+2" em 9 px quando a sub-coluna fx está oculta
  │      │      │ ▮                          │        │
  ├──────┼──────┼────────────────────────────┼────────┤
  │▌2    │      │ ▯ tsu                      │ ·      │  2.1  ▯ = ghost (alfa 0,45) · fundo cell_beat
  └──────┴──────┴────────────────────────────┴────────┘
```

### 6.5 Wireframe — inspector (painel direito, 280 px)

```
┌────────────────────────────┐
│ INSPECTOR      Chamada·c2·1.1│  ← panel_header 22
│ SEÇÃO                       │
│ ■ Chamada  ✎   grupo: Intro▾│  ← swatch = cor (clique: picker) · grupo (menu: mover para…)
│ toca  − 2× +   compassos − 8 +   ⧉ ✕  ▶ só esta │
│ COMPASSO                    │
│ tempos − 4/4 +   pp p mp [mf] f ff   < >        │
│ andamento  ♩ rall. accel. a tempo               │
│ LINHA                       │
│ ■ Chū 1 ✎  cor: do instrumento ▾   ⚙           │  ← "do instrumento" | "própria"; picker
│ chū · M S ★   vol ──●──── −6 dB   pan ──●──     │
│ sub-colunas: [dyn] [kiai] [fx]                  │
│ pessoas: ● Ana  ○ Bia                           │
│ CÉLULA                      │
│ DON  ·  D couro 100 %  ·  dona Ana  ·  sfz ✓    │
│ efeitos  [R4< −+ ×] [+ ▾]   texto: R4<  ✎      │  ← chips por comando; "+" abre os presets
│ golpes resolvidos: 4× couro (R L R L) 60→100 %  │
└────────────────────────────┘
```

---

## 7. Teclado

Coerente com o [plano de teclado](/planejamento/teclado.md): as **ações são as mesmas**; o que
muda é o significado dos movimentos (↑/↓ agora andam no tempo) e o que é "linha" para o vim.
Para não quebrar quem usa a grade horizontal durante a transição (§8), os movimentos passam a
ter nome **semântico** — `grid::TimePrev/TimeNext/TrackPrev/TrackNext` — e o keymap escolhe por
contexto `layout == vertical | horizontal` (o `GridView` emite `layout=…` no `key_context`).

### 7.1 Movimento (os dois modos)

| Tecla (simples) | Vim | Ação |
|---|---|---|
| `↓` / `↑` | `j` / `k` | célula seguinte/anterior **no tempo** (cruza compassos e seções) |
| `→` / `←` | `l` / `h` | próxima/anterior **sub-coluna**; sem fx/dyn/kiai visíveis, próxima/anterior linha (instrumento) |
| `Tab` / `Shift+Tab` | `Tab` / `Shift+Tab` | próxima/anterior **linha (instrumento)** — como o Renoise (hoje `Tab` avança no tempo; no editor vertical quem avança é o *edit step*) |
| `Home` / `End` | `0` / `$` | 1ª / última célula do compasso |
| `Page Up` / `Page Down` | `W`-`B` (`shift-w`/`shift-b`) | compasso anterior/seguinte (no simples hoje é instrumento ±; passa a compasso) |
| `Ctrl+↓` / `Ctrl+↑` | `w` / `b` | próximo/anterior tempo (`e` fim do tempo) |
| `Ctrl+Home` / `Ctrl+End` | `gg` / `G` | início / fim da música (`{n}G` compasso n) |
| `Alt+Page Up/Down` | `(` / `)` | seção anterior/seguinte |
| `Ctrl+Page Up/Down` | `gk` / `gj` | 1ª / última linha (instrumento) |
| — | `gf` / `gn` | ir para a sub-coluna fx / voltar à nota |
| `Scroll Lock` | `space v f` | follow ligado/desligado (`view::ToggleFollow`) |
| `Ctrl+roda`, `Ctrl+Alt+↑/↓` | `z i`/`z o`, `z k`/`z j`, `z 0` | largura da coluna, altura da linha, padrão |

### 7.2 Editar e *edit step*

- **Edit step** (`settings.edit_step`, padrão 1): quantas células o cursor desce depois de
  confirmar uma célula (`Space`/`Enter`/`Tab`-confirma; em vim, no insert). `Alt+0`…`Alt+9` nos
  dois modos (0 = fica na célula — bom para corrigir; o Renoise usa `Ctrl+0–9`, mas aqui
  `Ctrl+1/2/3` são as abas), chip `passo 1` na barra de abas (roda ±1), `:step 2` no vim. Digitar "don doko don doko" com passo 2 numa música em semicolcheias escreve
  nos tempos; com passo 1, nas células — o fluxo de hoje.
- **Confirmação**: `Space`, `Enter`, `Tab` (confirma e desce `step`); `Shift+Enter` confirma e
  sobe; `Esc` cancela (simples) / confirma e volta ao normal (vim). `Backspace` apaga um char ou,
  vazio, sobe uma célula; `Delete` apaga a célula (e os fx dela).
- **Fx** (§4.5): sub-coluna fx digita o texto do fx; atalhos `Ctrl+Shift+R/F/,/./X/V/0` no simples
  e `space f …` no vim.
- **Objetos do vim**: "linha" continua sendo **o compasso atual na linha atual** (`dd`, `yy`,
  `cc`, `V`) — agora é um trecho vertical da coluna; `dam` apaga o compasso em todas as colunas;
  `V` = V-COMPASSO (as linhas do compasso nesta coluna); `Ctrl+V` = **V-BLOCO** (esta célula em
  todas as colunas, estendendo com `j`/`k`) — a seleção `Range4 (t0, t1, g0, g1)` não muda de
  forma, só de desenho.
- **Transformações da seleção** (o "Advanced Edit" do Renoise, só o que serve): `gU`/`gu`
  acentuar/desacentuar (existem), `!` sfz (existe), `>`/`<` deslocar no tempo (existem),
  **`grid::Reverse`** (inverte a ordem das células, `space e r` / `Ctrl+Shift+←`),
  **`grid::Interpolate`** só para fx `V` (`V2`…`V8` em rampa entre a 1ª e a última célula,
  `space f i`), e os fx em bloco (§4.5). "Expandir/encolher" (dobrar a subdivisão só do trecho)
  não entra: a subdivisão é da música.

### 7.3 Seleção com o mouse e arrasto

Iguais aos de hoje, girados: arrastar de uma célula vazia = bloco; `Shift+clique` estende;
apertar numa nota e arrastar = mover (Alt clona), com o bloco flutuante e o destino tracejado;
clique no cabeçalho da coluna = cursor na 1ª célula visível dela; `Shift+clique` em dois
cabeçalhos = colunas inteiras; clique no gutter = ponto de partida do play (como a régua);
arrasto na faixa de loop do gutter = trecho; `Alt+clique` limpa; arrastar ⠿ da banda = reordenar.

### 7.4 Painel de seções

Tabela em §3.3 (contexto `Sections`). `Ctrl+Shift+B` mostra/esconde; `space g s` (vim) leva o
foco ao painel; `Esc` devolve ao editor. Os mesmos comandos existem na paleta e no botão direito.

### 7.5 Tabela de bindings novos (resumo para `assets/keymap-default.json`)

| Contexto | Tecla | Ação |
|---|---|---|
| global | `ctrl-shift-b` | `view::ToggleSections` |
| global | `scrolllock` | `view::ToggleFollow` |
| global | `alt-0`…`alt-9` | `grid::EditStep(n)` |
| `layout == vertical` | `down`/`up`, `j`/`k` | `grid::TimeNext/TimePrev` |
| `layout == vertical` | `right`/`left`, `l`/`h` | `grid::ColNext/ColPrev` (sub-coluna; cai em `TrackNext/Prev` sem sub-colunas) |
| `layout == vertical` | `tab`/`shift-tab` | `grid::TrackNext/TrackPrev` |
| `layout == vertical` | `pageup`/`pagedown` | `grid::PrevMeasure/NextMeasure` |
| `layout == vertical` | `ctrl-down`/`ctrl-up` | `grid::NextBeat/PrevBeat` |
| `layout == vertical` | `shift-enter` | `grid::CommitBack` |
| simples | `ctrl-shift-r/f/x/v/0`, `ctrl-shift-,`/`.` | `fx::CycleRoll/ToggleFlam/ToggleCut/CycleVel/Clear/DelayDown/DelayUp` |
| simples | `ctrl-shift-left` | `grid::Reverse` |
| vim normal/visual | `space f r/shift-r/f/d/shift-d/x/v/h/c/e/i` | `fx::*` (§4.5) |
| vim normal | `g f` / `g n` | `grid::GoFx` / `grid::GoNote` |
| vim normal/visual | `space e r` | `grid::Reverse` |
| vim normal/visual | `space s g` / `space s shift-g` / `space g s` | `section::Group` / `section::Ungroup` / `nav::Sections` |
| vim normal/visual | `space v f` / `space v x` | `view::ToggleFollow` / `view::ToggleFxCols` |
| `Sections` | `up/down j/k space enter f2 r ctrl-c ctrl-x ctrl-v y d p ctrl-d delete x alt-up alt-down ctrl-g ctrl-shift-g left right h l + - escape` | §3.3 |

`+`/`-` em `Sections` sombreiam o BPM ±1 do modo simples só enquanto o painel tem o foco
(contexto mais profundo vence — regra do GPUI). Todo binding novo ganha título no
`registry.rs` (teste existente) e `python3 tools/gen-atalhos.py` regenera a referência.

---

## 8. O que acontece com o que existe

| Peça | Decisão |
|---|---|
| **Grade horizontal** | Vira `Layout::Horizontal` dentro do mesmo `GridView` (um enum; o estado — cursor, seleção, registradores, vim, buffer, menus — é compartilhado; só geometria, pintura, hit-test e `cell_bounds/visible_cells` despacham pelo layout). O editor vertical é o **padrão**; `view::ToggleLayout` (paleta, `space v l`, `settings.layout`) troca durante a transição. Na última etapa (T10), depois de o Fernando usar o vertical por alguns dias, a horizontal **sai** (menos ~1,4 k linhas: `paint.rs` + metade do `render.rs` + hit-test) — ou fica, se ele preferir (Q1). Motivo de não reescrever a view do zero: o `GridView` tem ~1,8 k linhas de lógica independente de layout que o vim e o probe exercitam; manter uma view evita duplicar bugs |
| **Abas do editor** | `Tab::Grade` passa a se chamar **editor** (`Ctrl+1`); partitura e esteira iguais |
| **Esteira** | intocada como view; adota `fx::apply` em `note_events` (rulos e flams aparecem como discos extras) |
| **Partitura** | adota `fx::apply`: rulo = tremolo, flam = acciaccatura, ghost (`V0`–`V2`) = parênteses, corte = staccato; `D`/`H` não aparecem. Etapa opcional (T8b) |
| **Inspector** | ganha: swatch/picker na SEÇÃO (+ grupo), origem da cor e picker na LINHA, "sub-colunas" (dyn/kiai/fx), bloco EFEITOS na CÉLULA; `InspectorKey` inclui `lane` (nota/fx/kiai) e o fx da célula |
| **Trechos** | o bloco vai para o painel esquerdo; ações, paleta, `practice::*` e o formato não mudam |
| **Painel direito** | fica INSPECTOR + MÚSICAS (ganha altura) |
| **Barra de contexto** | igual + chips `passo N` e `fx ▢`; `⤓ follow` na barra de abas |
| **Status bar** | ganha swatch da seção, `passo N` e `1/2` (volta do ritornello) |
| **Modo projeção** | mostra grupo · seção na cor da seção (texto `ink`, swatch grande) |
| **Mixer / Pessoas / Kit** | Kit ganha a linha "cor" por instrumento; o resto só usa `track_color(song, track)` |
| **Export MIDI / WAV / stems** | herdam os fx pelo schedule; MIDI escreve o grace como nota curta antes |
| **Probe** (`--demo probe`) | passos que usam `cell_bounds`/hints se adaptam sozinhos (as bounds viram verticais). Precisam de ajuste: *"arrasto c2→c3 na faixa de loop"* (o arrasto passa a vertical, no gutter; `measure_xs` → `measure_ys`), *"altura das linhas: Ctrl+Alt+↓ encolhe"* (`h1 == 24` com `ROW_H` 28, e o padrão volta a 28), *"seleção de seções: clique troca, Ctrl+clique junta"* (continua na banda; ganha um gêmeo no painel), *"inspector: → ajusta o slider; clicar na grade move o cursor"* (`→` agora é sub-coluna: usar `↓` no passo). Passos novos: painel de seções (clique/Ctrl+clique/arrastar/Ctrl+G/Ctrl+Z), fx (digitar `R4` na sub-coluna → `schedule.hits()` da célula tem 4), cor do instrumento (Kit → cabeçalho da coluna muda), copiar/colar seção entre músicas |
| **Capturas e docs** | `docs/imagens/grade.png` → `editor.png`; guia de uso (seção "Escrever no editor"), formato da música (`fx`, `sectionGroups`, cores), dados e arquivos (settings novos), atalhos (gerada), arquitetura (layout, painel, overlay) |

---

## 9. Plano de implementação em etapas

Regras (as do [plano DAW §4](/planejamento/daw.md) valem): branch `tracker`; commit por etapa em
português; cópia do banco em `/tmp/teste.db`; **VP** = `cargo fmt && cargo clippy --workspace
-- -D warnings && cargo test --workspace` + `--demo probe | grep -c FALHOU` = 0; capturas com
`--demo MODO` em `/tmp/shots/<etapa>-<modo>.png`, olhadas antes de seguir; nunca
`window.refresh()`; o que anima vai no overlay. `--demo playstat-tracker` e `hoverstat` ficam
dentro de **+5 %** da linha de base da grade.

Dependências: T1 → T2 → T8; T3 → T4 → (T5 → T6) → T7 → T9 → T10. **T1/T2 podem andar em
paralelo com T3/T4** (outro agente): não tocam na UI. A primeira entrega visível é **T3** (o
editor vertical com tudo o que a grade faz hoje); a segunda, **T5** (o painel).

### T0 — Linha de base · 0:15

VP; capturas `play`, `dyn`, `loop`; `--demo playstat-grade` e `hoverstat` anotados.
**Pronto quando**: números guardados em `/tmp/shots/t0.txt`.

### T1 — Modelo e core (formato + fx) · 3 h

1. `model.rs`: `Section.color`, `Section.group`, `Song.section_groups` (`sectionGroups`:
   `SectionGroup { id, name, color, collapsed }`), `InstrumentDef.color`, `Measure.fx:
   Option<CellRows>` — todos `#[serde(default, skip_serializing_if)]`; `SECTION_COLORS`.
2. `wadawko_core::fx` (novo): `Fx` enum, `parse`/`format`/`normalize`, `apply(&CellResolution,
   &[Fx]) -> (Vec<ResolvedHit>, Option<CutAt>)`, memo. `phrase::ResolvedHit` ganha `grace`.
3. `ops::cells`: `set_fx`, `fx_at`, `clear_fx`; todo caminho que copia/move células
   (`set_cell`, `fill_*`, registradores via `CellBlock`, `move_section`, `clone_section`,
   `set_measure_beats`, `set_subdivision`) carrega `fx` como carrega `cell_owners`.
   `ops::sections`: `set_section_color`, `group_sections(ids) -> group_id` (torna contíguo),
   `ungroup`, `rename_group`, `set_group_color`, `move_section` ciente de grupos (mover para
   dentro/fora), `remove_section` limpa grupo vazio; `sections_json` ↔ `paste_sections`.
   `ops::kit::set_instrument_color`; `Track::color_or_default(song)` com os 3 níveis.
4. **Testes**: round-trip JSON com e sem os campos; `fx::apply` por comando e combinações;
   grupo contíguo após `group_sections` de seleção não contígua; `paste_sections` renumera
   `start` e preserva fx/donos/kiai; `remove_section` some com grupo vazio; contraste das
   `SECTION_COLORS`.
5. Docs: [formato da música](/referencia/formato-da-musica.md).
**Pronto quando**: VP; um arquivo antigo abre e grava igual (hash do JSON sem campos novos).
**Risco**: esquecer um caminho que copia células e perder fx — o teste de `move/clone/paste`
cobre os três principais; grep por `cell_owners` lista os demais.

### T2 — Áudio e vistas que soam · 2 h (depende de T1)

1. `schedule.rs`: `fx::apply` no loop de golpes; `HitEv.humanize: f32`; `EventKind::Cut`;
   `engine.rs`: choke de canal no `Cut`, escala da humanização.
2. `lane.rs::note_events` e `wadawko-export::midi` (grace = nota de 30 ticks antes) usam `apply`.
3. **Testes**: `R4` numa colcheia → 4 `HitEv` na célula com ticks em `i/4`; `F` → golpe extra
   antes com `vel` 0,5 e mão oposta; `D+2` desloca; `X` emite `Cut`; `H0` → `humanize == 0`;
   render offline de uma fixture sem fx mantém o hash.
**Pronto quando**: VP; ouvir `R8<` num chū com `--mute` desligado uma vez.

### T3 — Editor vertical: geometria, pintura, mouse, overlay · 2 dias

**T3a (1 dia)** — `grid/vgeom.rs` (`ROW_H`, `COL_W`…, `Cols` = x de cada sub-coluna por linha,
`measure_ys`, `Geom` ganha `layout`, `cols`, `offset_x`), `grid/vpaint.rs` (gutter, coluna do
compasso, células com barras de golpe verticais, dyn/kiai/fx sub-colunas, seleção, cursor,
cunhas verticais), `grid/vrender.rs` (banda de seção, cabeçalho de colunas fixo, canvas por
seção), `GridView.layout: Layout` + `view::ToggleLayout`; `hit()`, `cell_bounds`,
`visible_cells` despacham; `sections_key` ganha `layout`, `col_w`, `fx_cols`, `offset_x`;
`row_h`/`col_w` em `settings`. Clique, seleção por arraste, edição, menus de dinâmica/andamento
funcionam. **Pronto quando**: VP (probe com os ajustes de §8); captura `play` no vertical;
`hoverstat` ≤ +5 %.
> **Decisões na implementação (2026-11-18)**: (1) **Cabeçalho de colunas** = `grid::vrender::ColHeadView`,
> view em cache posta pelo Shell na faixa fixa do editor (abaixo da barra de contexto, fora da
> rolagem), com `px(20)` para alinhar com o conteúdo; a chave (`GridView::col_head_key`) é
> calculada no render da grade e a view é notificada só quando muda. Ele desloca as colunas por
> `-offset` (o mesmo `GridView::offset` das seções) — o risco do T3 não se confirmou. Os
> mini-medidores são um `canvas` sem cache por cima dele (3 px no rodapé, largura da nota).
> (2) O popover `⚙` (`Menu::Row`) **não** fica no cabeçalho: um irmão pintado depois (a área
> rolável) o cobriria e `deferred` dentro de view em cache é arriscado; ele abre na raiz da
> grade, no topo da área visível (`Geom::viewport`, posto pelo Shell a cada frame), sob a coluna.
> (3) Gutter: clique = "tocar daqui" (sem o arrasto-para-rolar da régua); a faixa de loop (8 px)
> só **pinta** o trecho — o arrasto vertical é da T3b, e o passo do probe "faixa de loop" roda no
> `Layout::Horizontal` (troca e volta). (4) Coluna do compasso: 1ª linha = chip 30 px + `<` `>`
> 16 px; 2ª linha = chip de andamento (58 px, texto 10 px); a cunha fica nos 10 px da direita a
> partir da 3ª linha. Compasso de 1 linha não mostra o chip de andamento (inspector/paleta).
> Sub-coluna `dyn`: chip na 1ª linha, `<` | `>` na 2ª, cunha da 3ª em diante. (5) Teclas: bloco
> `Workspace && layout == vertical` no fim do keymap (no gpui, na mesma profundidade, o binding
> definido depois vence) remapeando setas, `shift-`, `alt-` e `alt-shift-` + `h j k l` do vim
> para as ações `grid::Left/Right/Up/Down` (e `Extend*`, `MoveSel*`, `CloneSel*`) existentes;
> `pageup/pagedown` continuam trocando de linha (T4). Os sliders do inspector também ouvem essas
> ações (`views/widgets.rs`): no vertical o sinal foi invertido para seguir a tecla física. O
> contexto `layout=…` está no div `workspace` do Shell (é ele quem tem o foco). (6) Zoom no
> vertical: `Ctrl+roda`, `z i`/`z o` = ±8 px na coluna, sem animação (o `cw` animado é da
> horizontal); `z 0` volta a 96/28; tudo gravado (`layout`, `tracker_row_h`, `tracker_col_w` no
> `settings.json`; `GridEvent::SettingsChanged` para o `Ctrl+roda`). "Ajustar à largura" =
> `vgeom::fit_col_w` com a largura da grade do último frame (re-render quando a faixa muda, como o
> `BarFit`). (7) Follow vertical "page": vira quando a linha do playhead passa de 85 % e põe-na a
> 10 % do topo. **Risco para a T3b**: no gpui, `AnyView::cached` só reaproveita se os *bounds* não
> mudaram (`view.rs`: `cache_key.bounds == bounds`) — rolar (inclusive `scroll.set_offset`)
> re-renderiza as seções visíveis. O "page" custa um re-render por página; o "center" (rolar a
> cada frame) re-renderizaria as seções visíveis a cada frame: medir antes, e se pesar, pintar o
> deslocamento dentro do canvas (offset y próprio, como o `offset` x) em vez de rolar o
> container. (8) O arrasto de bloco (`MoveDrag`) já funciona no vertical de graça (o `hit`
> despacha: dx → coluna, dy → tempo); o ghost e a mola não foram revistos. (9) `Cols.fx_x` existe
> e é sempre `None` (T8); o `hit` numa sub-coluna fx devolve `None`. Cor da seção no gutter =
> `ink_faint` até a T7 (`VPaintArgs::sec_color`). (10) O `--demo hoverstat` agora imprime os
> renders do cabeçalho de colunas e o tempo deles; `playstat-grade` força o horizontal e
> `playstat-tracker` o vertical. Medidas (weston headless, pixman; T0 → T3a): `playstat` Shell::render
> 0,31–0,32 ms (grade, T0) → 0,33–0,34 ms (tracker) com a grade a 0,34 ms na mesma rodada (máquina
> mais carregada); `hoverstat` por hover 0,04 ms (grade) + 0,20 ms (uma seção) → 0,07 ms (grade) +
> 0,13 ms (cabeçalho de colunas), 0 renders de seção.

**T3b (1 dia)** — `Shell::playhead` devolve `y`; overlay: linha + wash + ▶ no gutter, flash,
hover de linha, mini-medidores no cabeçalho; `follow` vertical (`center`/`page`/`off`,
`ToggleFollow`, pausa em arrasto); loop-lane no gutter (`LoopDrag` vertical, alças); arrasto de
bloco (`MoveDrag`: dx → coluna, dy → tempo) e do ⠿ da banda; rolagem horizontal com cabeçalho
sincronizado; "ajustar à largura". Modo `--demo playstat-tracker`. **Pronto quando**: VP;
tocando com loop de 2 compassos o playhead volta certo e a tela segue centrada sem re-render
das seções (`SEC_RENDERS` estável durante o play — conferir no HUD do `--bench`).
**Risco**: o cabeçalho fixo + `offset_x` — se o gpui não deixar dois irmãos partilharem o
offset sem re-render, o cabeçalho entra no mesmo `canvas` e o editor rola só verticalmente
(o conteúdo rola o x por `offset_x` próprio, como hoje). Decidir no 1º dia.

### T4 — Teclado do editor vertical · 1 dia (depende de T3)

Movimentos semânticos (`TimePrev/Next`, `TrackPrev/Next`, `ColPrev/Next`), `key_context`
`layout=…`, blocos novos do keymap (§7.5), edit step (`settings.edit_step`, `grid::EditStep(n)`,
chip, `:step`), `Shift+Enter`, `Ctrl+V` V-BLOCO, `grid::Reverse`, `gf`/`gn`, títulos no
`registry.rs`, which-key com os prefixos novos, `gen-atalhos.py`. Testes de dispatch
(`simulate_keystrokes`): `"i d o n space"` desce `step` células no vertical; `"tab"` muda de
coluna; `"alt-2"` + duas confirmações = 4 células. **Pronto quando**: VP; 10 min digitando a
"Renshū" só pelo teclado nos dois modos.

### T5 — Painel de seções · 1 dia (depende de T3; usa T1 para copiar/colar e cor)

`views/sections_panel.rs` (`impl Shell`, view em cache em `shell/chrome.rs` com a chave de
§3.6), lista plana primeiro (sem grupos): seleção (mesmas ações `section::*` + `ToggleSelect`),
rolar até a seção, renomear, botões em hover, arrastar para reordenar (ghost como o da banda),
`Alt`-clone, menu de contexto, copiar/colar JSON (`section::CopySections/PasteSections`),
teclado (contexto `Sections`), `Ctrl+Shift+B`, modo 36 px, TRECHOS migrado, hints `sec-{i}`.
**Pronto quando**: VP + passos novos do probe (clique/Ctrl+clique/arrastar/colar/Ctrl+Z);
captura `sections`.

### T6 — Grupos · 0,5–1 dia (depende de T5 e T1)

Árvore no painel (nó com `▾/▸`, swatch, total, recolher persistido), `Ctrl+G`/`Ctrl+Shift+G`,
arrastar para dentro/fora, banda de grupo de 22 px no editor, linha de cor contínua no gutter,
"tocar o grupo", director/status bar. **Pronto quando**: VP; probe "Ctrl+G agrupa 2 seções,
Ctrl+Z desagrupa"; arquivo com grupos abre no app web (validação manual com um `.taiko.json`).

### T7 — Cores · 0,5 dia (depende de T1; UI de T5)

`ui::color_picker`; Kit (cor por instrumento); `track_color(song, track)` nos 6 lugares;
origem da cor no `⚙` e no inspector; seção/grupo: painel, banda, gutter, status, director;
`InputTarget::Color`. **Pronto quando**: VP; probe "cor do instrumento muda o cabeçalho das
linhas que herdam e não das que têm cor própria"; captura `kit` e `play`.

### T8 — Efeitos na UI · 1 dia (depende de T2 e T4)

Sub-coluna fx (por linha e global, auto-aparecer), chips, marcador na célula com a sub-coluna
oculta, `Lane::Fx` no buffer de edição, ações `fx::*` (célula e seleção), `Interpolate` de `V`,
inspector EFEITOS com presets, menu de contexto, paleta, which-key `space f`. **T8b (opcional,
0,5 dia)**: partitura com tremolo/acciaccatura/parênteses/staccato. **Pronto quando**: VP; probe
"digitar R4 na sub-coluna → 4 golpes no schedule; Ctrl+Z limpa"; captura `fx`.

### T9 — Polimento "moderno" · 0,5–1 dia

Tokens `cell_beat`/`col_head`, cabeçalho de colunas final (hover de ícones 80 ms), hover de
linha no overlay, status bar/barra de abas com os chips novos, densidade 20 px conferida,
capturas finais (`docs/imagens/editor.png`, `sections.png`, `fx.png`), guia de uso, dados e
arquivos, arquitetura, `CHANGELOG.md`, `okf-check`, `gen-atalhos`. **Pronto quando**: VP;
`playstat-tracker` e `hoverstat` ≤ +5 % de T0.

### T10 — Remover a grade horizontal · 0,5 dia (só com o OK do Fernando, Q1)

Apagar `paint.rs`/render horizontal/`measure_xs`/`Layout`, `ToggleLayout`, docs; o probe
perde os passos "horizontais". **Pronto quando**: VP; `grep -rn "Layout::Horizontal"` vazio.

### Ordem e tempo

| Ordem | Etapa | Tempo | Entrega visível | Risco |
|---|---|---|---|---|
| 1 | T0 linha de base | 0:15 | números | — |
| 2 | **T3a** editor vertical (estático) | 1 d | **o editor na vertical, editável** | geometria/hit-test; probe |
| 3 | T3b playhead, follow, arrastos | 1 d | tocar e seguir, loop no gutter | cabeçalho fixo × offset |
| 4 | T4 teclado | 1 d | setas no tempo, edit step, Tab entre linhas | conflitos de keymap (teste existente) |
| 5 | **T5** painel de seções | 1 d | **a "aba" de seções à esquerda** | chave de cache do painel |
| (2–5 ∥) | T1 + T2 core/áudio | 5 h | — | caminhos que copiam células |
| 6 | T6 grupos | 0,5–1 d | grupos recolhíveis com cor | contiguidade ao arrastar |
| 7 | T7 cores | 0,5 d | cor por instrumento/seção/grupo | contraste (teste) |
| 8 | T8 efeitos na UI | 1 d | `R4<` na célula, soando | sincronia grade/esteira/partitura (resolvida por `apply` único) |
| 9 | T9 polimento + docs | 0,5–1 d | capturas, guia | desempenho (+5 %) |
| 10 | T10 remover horizontal | 0,5 d | menos código | — |

Total: **≈ 8,5–10 dias** de agente, com paradas coerentes depois de 3 (editor vertical
usável), 5 (painel) e 8 (efeitos). Se o tempo apertar: T6 pode ficar em "só lista plana", T8b e
T10 ficam para depois.

### Riscos transversais

- **Chaves de cache**: tudo o que o painel de seções, o cabeçalho de colunas e as seções passam
  a mostrar entra nas chaves (`sections_key`, chave do painel, chave do cabeçalho) — senão fica
  velho. Lista em §3.6 e §6.1.
- **Dois layouts, um estado**: toda função do `GridView` que lê geometria tem de despachar pelo
  `layout`; um `#[cfg(test)]` que percorre `cell_bounds` de todas as células nos dois layouts
  (sem sobreposição, bounds dentro da seção) pega esquecimentos.
- **Probe frágil a coordenadas**: usar `cell_bounds`/hints em todos os passos novos; nunca px
  fixos.
- **Compatibilidade com o app web**: nenhum campo obrigatório novo; validar manualmente
  exportando um `.taiko.json` com fx/grupos/cores e abrindo no web (branch `web-legacy`).
- **Teclas no Linux/ABNT2**: `Scroll Lock` pode não existir em notebooks — por isso `space v f`
  e o chip; nenhum binding novo usa tecla morta.

---

## 10. Perguntas em aberto (com recomendação)

| # | Pergunta | Recomendação |
|---|---|---|
| **Q1** | A grade horizontal continua como opção (`ToggleLayout`) ou sai depois da transição (T10)? | **Sair** em T10, depois de uma semana de uso do vertical: dois layouts dobram o custo de cada feature da grade. Se a partitura e a esteira já cobrem a leitura "no tempo", a horizontal não tem papel único |
| **Q2** | Grupo só organiza (cor, recolher, tocar o grupo) ou também repete (`repeat` do grupo na ordem tocada)? | **Só organiza** (§3.5). Repetição de grupo entra, se precisar, junto com "seções por referência" (F17) |
| **Q3** | Sintaxe dos efeitos: letras legíveis (`R4<`, `D+2`) ou hex estilo Renoise (`0R04`)? | **Letras** (§4.1): legível no ensaio, cabe na paleta e no which-key; hex não traz nada aqui |
| **Q4** | Follow padrão: centrado (linha fixa a 40 %, conteúdo rola) ou por página (como hoje)? | **Centrado** tocando, com `Scroll Lock`/chip para desligar; por página fica como opção |
| **Q5** | Trechos de ensaio migram para o painel esquerdo (junto das seções) ou ficam à direita? | **Esquerda**: estrutura e navegação juntas; o inspector ganha espaço à direita |
| **Q6** | Tamanhos padrão: linha 28 px e coluna 96 px (texto 13 px)? | **Sim**, ajustáveis (20–44 / 72–144) e salvos; conferir a 1,5 m da TV do ensaio na T3 |
| **Q7** | `Tab` passa a trocar de linha (instrumento), como no Renoise, e o avanço no tempo fica com `Enter`/`Space` + edit step? | **Sim** — é a convenção de todos os trackers e libera `Tab` do papel duplicado com `Space` |

## Andamento (2026-11-18, branch `tracker`)

| Etapa | Status | Notas |
|---|---|---|
| T0, T3a | feito | editor vertical (agente) |
| T1 | feito | `wadawko-core::fx`, `ops::structure` (cores, grupos, copiar/colar), campos novos; a cor dos instrumentos prontos fica em `Song.instrumentColors` |
| T2 | feito | schedule/engine (`EventKind::Cut`, `HitEv.humanize: f32`), esteira; MIDI herda do schedule |
| T3b | feito | faixa de loop no gutter (`measure_at`/`bar_pos` por layout); seguir centrado como opção (`view::ToggleFollowCenter`) — custa +0,9 ms/frame (12 seções redesenhadas por frame), por isso o padrão continua "por página"; `view::ToggleFollow` (Scroll Lock); faixa da linha sob o mouse no overlay (0 renders de seção) |
| T4 | feito (essencial) | `Tab`/`Shift+Tab` trocam de linha, `PgUp/PgDn` compasso, `Ctrl+↑/↓` tempo, `Alt+PgUp/PgDn` seção, `Ctrl+PgUp/PgDn` 1ª/última linha, passo de edição `Alt+0–9`. Faltam V-BLOCO, `grid::Reverse`, `gf`/`gn` |
| T5 | feito | painel de seções (`Part::Sections`) com trechos |
| T6 | feito | grupos no painel; arrastar sobre uma seção entra no grupo dela; grupo e cor no cabeçalho da seção do editor (sem banda própria: a geometria não muda) |
| T7 | feito | cor de seção/grupo/instrumento; origem da cor no inspector ("herdar") |
| T8 | feito | sub-coluna fx, ações, inspector |
| T8b | parcial | partitura: rulo (`R`) e ghost (`V0–V2`); flam e corte ainda não desenham |
| extra | feito | **minimap** (`views/minimap.rs`, `Part::Minimap`) e a grade acompanhando a seleção (reveal no render + rolagem automática no arrasto) |
| T9, T10 | pendente | polimento visual geral; remover a grade horizontal (Q1, com o OK do Fernando) |
