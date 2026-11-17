---
type: Plan
title: "Design estilo DAW e features de DAW"
description: "Pesquisa de UX e linguagem visual de DAWs (Bitwig e outros), sistema de design escuro de cantos retos para o app, backlog priorizado de features de DAW e plano de implementação em etapas."
tags: [planejamento, design, daw, ux, tema, mixer, features]
status: draft
estado: "pesquisa e plano prontos (2026-11-17); implementação a começar"
---

# WaDAWko — design estilo DAW e features de DAW

Data: 2026-11-17 · Status: plano · Pedido: *"features para uma verdadeira DAW, e uma
estilização com menos curvas, com design parecendo o Bitwig Studio: pesquisa de UX de DAW e
de features que faltam; planejar e implementar."*

Este documento tem quatro partes: (1) o resumo da pesquisa, (2) a especificação do sistema
de design "estilo DAW" (tokens, componentes, layout), (3) o backlog de features priorizado
com o **corte para implementar agora**, e (4) o plano de implementação em etapas pequenas e
verificáveis. O agente implementador segue a parte 4; as partes 2 e 3 são a referência.

Contexto do app: [visão geral](/visao-geral.md), [guia de uso](/guias/uso.md),
[arquitetura do app](/arquitetura/app.md). Backlogs anteriores que este plano absorve:
[redesign web §9](/planejamento/redesign-web.md), [gpui §1.2](/planejamento/gpui.md) e a
Fase F do [plano do repositório](/planejamento/repo.md). Telas atuais:
[grade](/imagens/grade.png), [partitura](/imagens/partitura.png), [esteira](/imagens/esteira.png),
[paleta](/imagens/paleta.png), [mixer](/imagens/mixer.png).

> **Original, não cópia.** O resultado é um design *inspirado* na linguagem das DAWs (cinzas
> neutros, cantos retos, densidade, cor por trilha, medidores). Nenhum logo, nome, ícone ou
> asset da Bitwig ou de outra DAW é copiado; os valores de cor abaixo são nossos.

---

## 1. Resumo da pesquisa

Feita em 2026-11-17 com dois levantamentos: a **linguagem visual** das DAWs (paletas extraídas
dos arquivos de tema reais do Bitwig 5.x/6.1, Ableton Live 12.1, Reaper 6/7 e Renoise, mais os
manuais e capturas oficiais) e a **UX e as features** de DAWs e de notadores/editores de percussão.
Valores hex sem "≈" vêm dos arquivos de tema; com "≈", de capturas.

### 1.1 Linguagem visual — o que dá a "cara" de cada DAW

| DAW | Cinzas | Acento | Cor por trilha | Cantos | Em uma frase |
|---|---|---|---|---|---|
| **Bitwig 4/5** | neutros puros (R=G=B) em patamares **invertidos**: janela/régua `#2E2E2E`, painéis e track headers `#535353`, arranger `#8F8F8F`, piano roll `#A6A6A6`, "buracos" `#0D0D0D`; display do transporte `#1D1D1D` | **um laranja só, `#FF8713`, que significa "ligado"** (play, loop, metrônomo, mute, aba ativa, arco do knob `#FF7D0F`); azul `#3EBAFF` reservado ao texto do display e ao playhead; solo amarelo `#CFC500`; record vermelho; seleção **branca** | listra ≈4 px à esquerda do header no arranger; **header inteiro** preenchido no mixer; clip sólido na cor com nome escuro | botões/clips 2–3 px; painéis e abas retos; knobs planos sem relevo; faders são barras preenchidas | "superfície cinza fosca com pontos laranja" |
| **Bitwig 6** | escala `Grey 0–6`: `#0F0F0F #1A1A1A #2626 26 #323232 #3D3D3D #494949 #5A5A5A`; painel `#464646`; texto `#D2D2D2` (não mais branco) | `On #D8720F`, `Accent #FF8713`, medidor até `#FF6900` | igual | igual; grade mais sutil, preenchimentos em gradiente nos loops | mesma assinatura, mais escura |
| **Ableton Live 12** (Default Dark) | neutros em 7 níveis: `#181818 #1E1E1E #242424 #2A2A2A #363636 #3E3E3E #464646`; texto `#B5B5B5` | âmbar `#FFAD56` para tudo que está ligado (texto escuro `#070707`); play verde `#00D38D`; valores em ciano `#03C3D5`; solo/cue azul `#3C6AB6`; seleção ciano claro `#B0DDEB` | 16 cores automáticas; **preenche o título da trilha e o clip inteiro** | botões 3–4 px e alguns *pills*; clips e títulos retos | "ligado em âmbar, valor em ciano, blocos de cor cheia" |
| **Renoise** | `#161616` fundo, `#464646` painéis com bisel de 1 px, pattern `#151515`; texto `#D7D7D7`/`#999999` | laranja-tijolo `#F16A32` (seleção, botão ativo, linha tocando `#994000`) + verde `#2A905E` (sliders, MIDI) | 16 cores; barra no **topo** do cabeçalho da trilha | retos; único relevo discreto dos cinco | "tracker preto onde a cor é informação" |
| **Reaper** (Default 6/7) | `#333333` janela/transporte, arrange `#454545`, TCP selecionado `#818989` (único cinza tingido) | teal/mint `#1ABC98` (toolbar on), medidores `#00BFBF→#00FE95`, cursor de play amarelo `#EFC852` | tinge TCP/MCP e items; items claros com picos pretos | botões ≈3 px com leve relevo; transporte redondo | "caixa de ferramentas cinza-chumbo que esconde controles até precisar" |
| **FL Studio 21** | slate azulado ≈`#3A4751` / `#2E3A42` / `#1F292F` | laranja-âmbar ≈`#F0A030` + lima ≈`#A6E04B` (LEDs, medidores) | cabeçalho e clips preenchidos (dessaturados) com barra de título escura | 4–6 px, brilhos, gradientes, multi-janela | "o único que ainda abraça o skeuomorfismo" |

Traços transversais que valem para nós: (a) cinzas **neutros** (só o FL tinge); (b) **uma** cor
de "ligado", usada de forma consistente; (c) seleção clara (branca ou ciano), nunca a cor de ação;
(d) cor por trilha nos cabeçalhos, com o conteúdo desenhado em tom mais escuro da mesma cor;
(e) medidores em gradiente verde → amarelo → laranja → vermelho com pico retido; (f) grade do
arranger desenhada **escurecendo** o fundo (Bitwig: compasso preto 39 %, batida preto 8 %), não
com linhas claras; (g) cabeçalhos de painel de 22–24 px, título bold de 12 px, abas como ícones
monocromáticos (ativo = cor de "ligado"); (h) hierarquia de texto por **alfa** (100/70/47/24 %)
em vez de por tamanho; (i) densidade: track header ≈44 px (meia altura 22), transporte ≈40 px,
fonte ≈11–12 px; (j) o display do transporte do Bitwig é um campo escuro `#1D1D1D` com texto
azul — a única "tela" dentro da UI; (k) o inspector do Bitwig é fixo à esquerda e o browser à
direita; os painéis não se movem, só ligam/desligam pelo rodapé.

Decisão para o WaDAWko: a escala de cinzas segue a **ordem** do Bitwig (moldura mais escura →
painéis → área de trabalho mais clara) nos **níveis do Bitwig 6** (mais escuros, confortáveis à
noite — usuários do 4/5 reclamam do arranger claro demais); a cor de "ligado" é o vermelho quente
da marca (couro) no papel do laranja do Bitwig; azul para seleção, borda (fuchi) e display, como o
"hitech" deles; M âmbar / S azul como no Live. Detalhes na parte 2.

### 1.2 UX de DAW — princípios adotados

1. **Usabilidade e consistência antes de "parecer hardware"**: sliders alinhados mostram mais de
   relance que knobs; a informação existe em dois formatos (relação visual + número exato sob
   demanda) — [Kevin Perry, SoS](https://www.soundonsound.com/people/kevin-perry-radical-redesign-modern-digital-audio-software).
2. **Um inspector único que segue a seleção**; ações ficam no menu/paleta —
   [Bitwig, Inspector](https://bitwig.com/userguide/latest/meet_inspector_panel).
3. **Duas vistas sobre o mesmo modelo** (Session/Arrangement no Live; grade/partitura/esteira aqui);
   trocar de vista nunca muda o som — [Live 12, conceitos](https://www.ableton.com/en/live-manual/12/live-concepts/).
4. **Feedback sonoro imediato ao editar** (Hydrogen toca a nota ao inserir; o Live "está sempre
   ouvindo") — [Hydrogen cap. 9](https://hydrogen-music.org/documentation/manual/manual_en_chunked/ch09.html).
5. **Navegação quantizada e zoom pelo teclado**; *follow* que pausa ao editar —
   [Live, Arrangement](https://www.ableton.com/en/live-manual/12/arrangement-view/).
6. **Eficiência é o fator central**: aceleradores, visibilidade e funcionalidade são os problemas
   "vermelhos" (param o trabalho) — [McGrath 2020, Goldsmiths](https://research.gold.ac.uk/id/eprint/29312).
7. **Densidade ajustável / progressive disclosure**; painéis contextuais no lugar de barras
   permanentes — [case study Live](https://nenadmilosevic.co/?p=1506),
   [dj.studio](https://dj.studio/blog/minimalist-vs-customizable-daw-interfaces).
8. **Undo visível e edição não destrutiva** (Undo History do Live/Reaper; padrões por referência
   com "make unique") — [NN/g](https://www.nngroup.com/articles/ten-usability-heuristics/).
9. **Sem tela em branco** para o iniciante (presets e desafios do Groove Pizza), mas sem urgência —
   [MusEDLab](https://wp.nyu.edu/musedlab/?p=73).
10. **Tema escuro**: o conteúdo é o mais brilhante; elevação por luminância, nunca branco puro
    para texto; cores de status reservadas — [guia Camo](https://blakecrosley.com/guides/design/camo).
11. **Visibilidade de status e estrutura**: seção atual, próximo ritornello e andamento sempre
    visíveis (Sections do Bitwig, Director do Hydrogen) —
    [Bitwig, playback](https://www.bitwig.com/userguide/latest/playing_back_the_arranger/).
12. **Ferramentas de estudo** são o estado da arte para ensaio: loop por arraste, rampa de
    velocidade com nº de passagens por passo, count-in por passagem e **listas de trechos
    nomeados** — [Soundslice](https://www.soundslice.com/help/en/player/basic/286/speed-training/),
    [Transcribe!](https://www.seventhstring.com/xscribe/overview.html).

### 1.3 Features — o que o levantamento mostrou

- Convenções que vale copiar tal qual: `Ctrl+L` = loop da seleção (Live); padrão de acentos do
  metrônomo como `ABBB` (Reaper); tap tempo com média móvel (Hydrogen `\`); mudança de fórmula de
  compasso pela régua/inspector (Bitwig); templates de projeto nomeados (Live/Bitwig).
- Medidores: peak + RMS com pico retido e campo numérico (Live); VU por strip e no master
  (Hydrogen); escala 0/6/12/18/24/30/36/48/∞ (Bitwig "Big Meters").
- Padrões por referência (aliases do Renoise, Pattern Clips do FL, *virtual patterns* do
  Hydrogen) são o caminho natural das nossas seções — mas mudam o formato (fica no backlog como G).
- Notadores de percussão: MuseScore 4.5 tem painel de *drum pads* com prévia e teclas A–H;
  sticking é texto R/L; rolls são tremolo. Dorico separa "grid" e "single-line". Guitar Pro tem
  ghost notes entre parênteses e acentos com velocity automática. O TJA (Taiko no Tatsujin) é texto
  onde o número de caracteres do compasso define a subdivisão e `#SENOTECHANGE` troca o rótulo de
  kuchi shōga — a nossa grade já é mais expressiva (mãos, dinâmicas, kiai).
- Não há editor dedicado de kuchi shōga por instrumento no mercado (busca em pt/en/ja só retorna
  ferramentas TJA, o DoDon para iPad e um soundfont + `.drm` para MuseScore): o nicho continua vazio.

### 1.4 Fontes consultadas

Bitwig: [guia do usuário](https://www.bitwig.com/userguide/latest/) —
[anatomia da janela](https://www.bitwig.com/userguide/latest/anatomy_of_the_bitwig_studio_window/),
[corpo](https://www.bitwig.com/userguide/latest/the_window_body/),
[transporte](https://www.bitwig.com/userguide/latest/the_window_menus_transport_area/),
[rodapé](https://www.bitwig.com/userguide/latest/the_window_footer/),
[arrange e trilhas](https://www.bitwig.com/userguide/latest/the_arrange_view_and_tracks/),
[trilhas e cores](https://www.bitwig.com/userguide/latest/intro_to_tracks/),
[mixer](https://www.bitwig.com/userguide/latest/the_mix_view/),
[clip launcher](https://www.bitwig.com/userguide/latest/the_clip_launcher/),
[inspector](https://bitwig.com/userguide/latest/meet_inspector_panel),
[dashboard](https://www.bitwig.com/userguide/latest/the_dashboard/),
[export de áudio](https://www.bitwig.com/userguide/latest/exporting_audio);
paletas reais: [bitwig-theme-editor-helper](https://github.com/polarity/bitwig-theme-editor-helper)
(tema 5.x com 248 chaves e 6.1.4), [bitwig-theme-editor-webui](https://github.com/Berikai/bitwig-theme-editor-webui),
[bitwig-theme-manager](https://github.com/DJZeroAction/bitwig-theme-manager),
[KVR: temas do Bitwig](https://www.kvraudio.com/forum/viewtopic.php?t=611222),
[Bitwish: theme support](https://bitwish.top/t/theme-skin-support/555);
reviews: [SoS 5.1](https://www.soundonsound.com/reviews/bitwig-studio-5-1),
[SoS 6](https://www.soundonsound.com/reviews/bitwig-studio-6),
[Audiofanzine 6](https://en.audiofanzine.com/general-sequencer/bitwig/bitwig-studio-6-studio/editorial/reviews/test/testing-bitwig-studio-6.html),
[CDM: temas](https://cdm.link/2024/07/bitwig-themes/).
Ableton Live 12: [primeiros passos (tema)](https://www.ableton.com/en/live-manual/12/first-steps/),
[conceitos](https://www.ableton.com/en/live-manual/12/live-concepts/),
[arrangement](https://www.ableton.com/en/live-manual/12/arrangement-view/),
[session](https://www.ableton.com/en/live-manual/12/session-view/),
[mixing](https://www.ableton.com/en/live-manual/12/mixing/),
[gravação](https://www.ableton.com/en/live-manual/12/recording-new-clips/),
[grooves](https://www.ableton.com/en/live-manual/12/using-grooves/),
[comping](https://www.ableton.com/en/live-manual/12/comping/),
[arquivos e sets](https://www.ableton.com/en/live-manual/12/managing-files-and-sets/),
[tema Default Dark (.ask)](https://github.com/phixion/ableton-themes),
[CDM: temas do Live 12](https://cdm.link/2024/05/makeover-live-12-themes/).
Reaper: [Default 6.0 (.ReaperTheme)](https://github.com/johnnovak/configs),
[Default 7.0](https://github.com/Conceptual-Machines/reaper-theme-builder),
[descrição das variáveis](https://github.com/ReaTeam/ReaScripts),
[reaper.blog: Reaper 6](https://reaper.blog/2019/12/reaper-6-faq/),
[Production Expert: Reaper 7](https://www.production-expert.com/production-expert-1/reaper-7-we-take-a-look-at-new-features),
[SoS: arranjo no Reaper](https://www.soundonsound.com/techniques/power-arranging-reaper),
[metrônomo do Reaper](https://ProRec.com/?p=3592).
Renoise: [GUI](https://tutorials.renoise.com/wiki/GUI_Customisation),
[tela principal](https://tutorials.renoise.com/wiki/Main_Screen),
[pattern editor](https://tutorials.renoise.com/wiki/Pattern_Editor),
[pattern matrix](https://tutorials.renoise.com/wiki/Pattern_Matrix),
[groove](https://tutorials.renoise.com/wiki/Groove_Settings),
[tema Default (.xrnc)](https://github.com/esaruoho/paketti),
[fórum: tema padrão](https://forum.renoise.com/t/about-the-default-colortheme/19676).
FL Studio: [temas](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/envsettings_themes.htm),
[toolbar](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/toolbar_panels.htm),
[playlist](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist.htm),
[mixer](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/mixer.htm),
[pattern clips](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist_patterns.htm).
UX: [Kevin Perry (SoS)](https://www.soundonsound.com/people/kevin-perry-radical-redesign-modern-digital-audio-software),
[McGrath 2020](https://research.gold.ac.uk/id/eprint/29312),
[redesign do Live (case study)](https://nenadmilosevic.co/?p=1506),
[tese HAW Hamburg (onboarding no Cubase)](https://reposit.haw-hamburg.de/bitstream/20.500.12738/19701/1/BA_User_Experience_in_Musiksoftware.pdf),
[NN/g heurísticas](https://www.nngroup.com/articles/ten-usability-heuristics/),
[dj.studio](https://dj.studio/blog/minimalist-vs-customizable-daw-interfaces),
[Camo: dark UI](https://blakecrosley.com/guides/design/camo),
[MusEDLab](https://wp.nyu.edu/musedlab/?p=73),
[Ethan Hein](https://ethanhein.substack.com/p/learning-music-from-ableton).
Percussão e estudo: [Hydrogen manual](https://hydrogen-music.org/documentation/manual/manual_en_chunked/ch08.html),
[MuseScore: percussão](https://handbook.musescore.org/idiomatic-notation/percussion/inputting-percussion-notation),
[MuseScore: sticking e rolls](https://musescore.org/en/print/book/export/html/330561),
[Dorico: kits](https://www.steinberg.help/r/dorico-pro/6.1/en/dorico/topics/notation_reference/notation_reference_unpitched_percussion/notation_reference_unpitched_percussion_kit_presentation_types_r.html),
[Guitar Pro 8: bateria](https://www.guitar-pro.com/docs/gp8/score/drums),
[spec TJA](https://gist.github.com/KatieFrogs/e000f406bbc70a12f3c34a07303eec8b),
[TJAPlayer3: tja](https://iepiweidieng.github.io/TJAPlayer3/tja),
[kuchi shōga](https://en.wikipedia.org/wiki/Kuchi_sh%C5%8Dga),
[DoDon](https://apps.apple.com/us/app/dodon/id6753098478),
[soundfont kumidaiko](https://www.polyphone.io/fr/soundfonts/unpitched-percussion/745-japanese-taiko-kumidaiko-soundfont-v1),
[Soundslice: speed training](https://www.soundslice.com/help/en/player/basic/286/speed-training/),
[Soundslice: practice lists](https://www.soundslice.com/blog/214/introducing-practice-lists/),
[Transcribe!](https://www.seventhstring.com/xscribe/overview.html),
[Anytune](https://apps.apple.com/us/app/anytune-pro/id427637607),
[Groove Pizza](https://apps.musedlab.org/groovepizza/), [Drumbit](https://drumbit.app/).

Lacunas: o Bitwig não publica a fonte nem os valores oficiais (vêm dos arquivos de tema extraídos
pela comunidade); as páginas do Dorico bloqueiam acesso automatizado; não existe uma lista
canônica de "heurísticas de DAW" — McGrath 2020 é o mais próximo.

---

## 2. Sistema de design "estilo DAW"

Especificação implementável. Tudo o que está aqui vira código em `crates/wadawko-app/src/theme.rs`
(tokens), `ui.rs` (helpers) e nas views. Onde um valor atual é citado, ele vem do código de hoje.

### 2.1 Princípios (o que dá a "cara" de DAW)

1. **Cinzas neutros em níveis** (sem tinta azulada como hoje: `#1d212c`), na ordem do Bitwig:
   **moldura mais escura → painéis → área de trabalho mais clara**, nos níveis (escuros) do
   Bitwig 6. O olho entende profundidade pela luminosidade, não por sombras nem bordas
   arredondadas. A grade se desenha **escurecendo** o fundo (linhas de tempo e de compasso mais
   escuras que a célula), como o arranger do Bitwig.
2. **Cantos retos**: painéis, cartões, células, abas e popovers com raio **0**; controles
   clicáveis (botão, chip, campo, segmento) com raio **2 px**. Pílulas (`999`) deixam de existir.
3. **Uma cor quente de ação** (o vermelho "couro" da marca, aquecido) usada só para o que está
   *ligado* ou *soando*: play, loop, cursor, notas de couro. Azul "fuchi" para seleção, notas
   de borda e solo. Tudo o mais é cinza.
4. **Cor por trilha** nos cabeçalhos (listra lateral de 4 px na grade; cabeçalho preenchido no mixer), nunca no fundo das
   células — o conteúdo continua legível.
5. **Densidade**: fontes de 10–12 px, alturas de 22–28 px na moldura, linhas da grade de 36 px.
6. **Cabeçalhos de painel** de 22 px em caixa alta de 10 px, fundo mais escuro que o painel.
7. **Transporte centralizado** na barra de cima, com mostrador de posição.
8. **Estados planos**: ligado = preenchido com a cor; desligado = cinza `paper_raise`;
   desabilitado = texto `ink_faint` sem hover; hover = `paper_hover`. Sem sombras, sem gradientes.
9. **Medidores e cores** seguem convenções de DAW: verde → âmbar → vermelho; M âmbar, S azul.

### 2.2 Tokens de cor

Campos novos no `struct Theme` estão marcados com **(novo)**. Os existentes mantêm o nome,
para que o código atual compile sem mudanças; só os valores trocam. `on_accent` passa a ser
**escuro** no tema escuro (texto escuro sobre botão aceso, como lâmpada) — conferir contraste
no teste da etapa E1.

| Token | Uso | Escuro (**padrão novo**) | Claro |
|---|---|---|---|
| `chrome` **(novo)** | moldura: barra de cima, nav rail, painel direito, barra de abas/contexto, status bar — o nível mais escuro | `#1c1c1c` | `#f6f6f6` |
| `header` **(novo)** | faixa de cabeçalho de painel (MÚSICAS, INSPECTOR, grupo do mixer) | `#161616` | `#e9e9e9` |
| `display` **(novo)** | "tela" do transporte (posição, BPM): campo escuro com texto `accent2`, como o display do Bitwig | `#0f0f0f` | `#e3e3e3` |
| `bg` | área de trabalho: fundo da grade, páginas, esteira | `#262626` | `#ececec` |
| `paper` | cartões, popovers, cabeçalho de seção, strips do mixer, paleta | `#303030` | `#ffffff` |
| `paper_raise` | controles em repouso: botão, chip, campo, segmento inativo | `#3b3b3b` | `#efefef` |
| `paper_hover` | hover / pressionado | `#474747` | `#e2e2e2` |
| `cell_empty` | célula vazia da grade | `#2b2b2b` | `#e6e6e6` |
| `cell_filled` **(novo)** | célula com nota (hoje usa `paper`) | `#363636` | `#ffffff` |
| `ink` | texto principal, nomes, sílabas (não é branco puro) | `#e6e6e6` | `#1f1f1f` |
| `ink_soft` | texto secundário, rótulos, ícones (≈70 %) | `#a6a6a6` | `#5a5a5a` |
| `ink_faint` | legendas, desabilitado, contagem "e & a" (≈45 %) | `#707070` | `#9a9a9a` |
| `line` | hairlines: bordas de painel, divisores | `#3a3a3a` | `#d9d9d9` |
| `line_strong` | borda de controle, trilho de slider | `#525252` | `#bdbdbd` |
| `line_beat` **(novo)** | linha de início de tempo na grade — **escurece** a célula (Bitwig: preto 8 %) | `#1a1a1a` | `#d2d2d2` |
| `line_bar` **(novo)** | barra de compasso na grade, 2 px (hoje `ink`, 3 px; Bitwig: preto 39 %) | `#0f0f0f` | `#9a9a9a` |
| `accent` | ação principal, **estado "ligado"** (play, loop, metrônomo, treino), cursor, nota de couro, kiai | `#ee6b5a` | `#d9503c` |
| `accent_wash` | fundo suave do acento (chip de andamento, toast) | `#4a2c27` | `#fbe3de` |
| `accent2` | seleção, nota de borda (fuchi), solo, faixa de loop, texto do display | `#5a9be0` | `#2f6fb0` |
| `accent2_wash` | fundo suave da seleção | `#253a52` | `#dfeafa` |
| `on_accent` | texto sobre `accent`/`accent2`/cor de trilha preenchidos | `#141414` | `#ffffff` |
| `playhead` **(novo)** | linha do playhead | `#f4f4f4` | `#1f1f1f` |
| `warn` **(novo)** | mudo ligado, aviso, medidor meio | `#e2b53e` | `#b8860b` |
| `danger` **(novo)** | clip, erro de gravação, excluir | `#ff4a4a` | `#d32f2f` |
| `meter_lo` **(novo)** | medidor até −18 dB | `#7fc53a` | `#2e9e52` |
| `meter_mid` **(novo)** | medidor −18..−6 dB | = `warn` | = `warn` |
| `meter_hi` **(novo)** | medidor −6..0 dB | = `accent` | = `accent` |

Contrastes (WCAG, calculados): `ink`/`bg` 11,7 · `ink_soft`/`paper` 5,3 · `on_accent`/`accent`
6,2 · `on_accent`/`accent2` 6,5. No tema claro `on_accent` continua branco (acentos mais escuros).

Observações:

- O **tema escuro vira o padrão** para bancos sem tema salvo (`main.rs:164`: `Ok(None)` → escuro).
  `--light`/`--dark` e o botão da barra continuam valendo.
- `lane.rs:540` tem `fuchi` fixo em `#2f6fb0`: passa a usar `th.accent2`.
- A partitura (`score.rs:77`) já deriva o tema da notação dos tokens; com os valores novos ela
  fica cinza-neutro sem outra mudança (checar `wadawko_notation::Theme::dark()` — `kyai` fixo em
  `#d6452b` pode ficar).
- **Cores por trilha** (`TRACK_COLORS`, 12, em `wadawko-core::model`, ao lado de `PERSON_COLORS`;
  escolhidas para contrastar entre si no cinza `#1e1e1e` e no `#ececec`):
  `#e8603f` coral · `#f08a2e` laranja · `#e6c33f` amarelo · `#8dc152` lima · `#2fae6b` verde ·
  `#2cb7a4` teal · `#3d9be9` azul · `#6f7ce8` índigo · `#a35ad6` roxo · `#e0569b` magenta ·
  `#b08968` castanho · `#8d99a6` ardósia. Linha sem cor definida usa a cor do **instrumento
  base** (determinística: shime → amarelo, chū → coral, nagadō → laranja, ō-nagadō → castanho,
  okedō → teal, ōdaiko → roxo, hira → verde, kane → azul, custom → ardósia). Uma pessoa continua
  com a sua cor própria (`PERSON_COLORS`) nos trilhos e nas notas de linha compartilhada.

### 2.3 Tokens de forma, espaço e tipo

Constantes em `ui.rs` (raios) e nas views (alturas). Os valores atuais estão entre parênteses.

| Token | Valor | Onde |
|---|---|---|
| `R0` raio de superfície | **0 px** | painéis, cartões, popovers, paleta, abas, cabeçalho de seção, células, barras de nota, cursor, seleção, strips, halo do flash |
| `R1` raio de controle | **2 px** | botão, chip, kbd, campo, segmento, trilho/cap do slider, medidor, swatch de cor, M/S |
| pílulas (`rounded(px(999))`) | → `R1` | `context_toolbar.rs:89` (caneta), `right_panel.rs:103` (+ nova) |
| borda | 1 px | tudo; foco = 1 px `accent2`; nunca 2 px exceto cursor da grade |
| sombra | nenhuma | popovers/paleta: `shadow_sm` + borda `line_strong` (hoje `shadow_lg`) |
| barra de cima | **44 px** (57) | `topbar.rs`, `shell/mod.rs:728` |
| nav rail | **64 px** (200) | `nav_rail.rs`, `shell/mod.rs:727` |
| faixa de abas do editor | 28 px | `view_tabs` |
| barra de contexto | 34 px, uma linha, quebra em 2 só se faltar largura | `context_toolbar.rs` |
| cabeçalho de painel | 22 px, texto 10 px caixa alta, espaçamento 0.06 em | `ui::panel_header` (novo) |
| painel direito | **280 px** (260) | `right_panel.rs`, `shell/mod.rs:813` |
| status bar | **22 px** (26) | `status_bar.rs`, `shell/mod.rs:814` |
| botão padrão | h 26, px 10, texto 12 (h 32, px 12, texto 13) | `ui::btn_bare` |
| botão pequeno | h 22, px 8, texto 11 | `ui::small_btn`, `widgets::sbtn` |
| M / S / ★ | 20×16, texto 10 bold | `widgets::mini`, `small_btn` na grade |
| chip | h 20, px 8, texto 11 | `widgets::chip`, `render.rs::chip` |
| kbd | h 16, texto 10 | `ui::kbd` |
| grade: `SEC_HEAD_H` | **28** (40) | `grid/mod.rs` |
| grade: `SEC_DYN_H` | **22** (26) | idem |
| grade: `TIMELINE_H` (régua) | **22** (30), os 6 px de cima = faixa de loop | idem |
| grade: `CELL_H` | **36** (42) | idem |
| grade: `TDYN_H` / `KYAI_H` | 20 / 22 (22 / 24) | idem |
| grade: `ROW_PAD` / `SEC_GAP` | 6 / 20 (12 / 34) | idem |
| grade: `LABEL_W` | 216 (igual) | idem |
| grade: barra de compasso | 2 px `line_bar` (3 px `ink`) | `paint.rs` |
| grade: separador de tempo | 1 px `line_beat` (hoje 2 px `line_strong`); subdivisão sem linha — a fresta de 1 px entre células mostra o `bg` | `paint.rs` |
| mixer: strip | 84 px de largura, fader 160 px | `mixer.rs` |
| fonte UI | Zen Kaku Gothic New 12 px; rótulos 11; legendas 10 | raiz |
| fonte de título | Shippori Mincho bold **16 px** (22) só no título da música e nos títulos de página | `topbar.rs`, `widgets::page_title` |
| números do transporte | 13 px bold tabular, `accent2` sobre `display` | BPM, posição |
| sílabas na célula | 12 px (`cw ≥ 44`) / 11 px | `paint.rs` |

### 2.4 Componentes

Cada linha diz como fica em cada estado. "Preenchido" = fundo na cor, texto `on_accent`.

| Componente | Normal | Hover | Ligado / ativo | Desabilitado | Notas |
|---|---|---|---|---|---|
| **Botão** (`ui::btn`) | `paper_raise`, borda `line_strong`, texto `ink`, R1 | `paper_hover` | — | texto `ink_faint`, sem hover, sem cursor | h 26 |
| **Botão primário** (play, + nova) | `accent` preenchido | `accent` 90 % | — | — | play tocando: `accent` preenchido + "■"; parado: `paper_raise` com "▶" em `accent` |
| **Toggle** (`ui::atoggle`: loop, metrônomo, count-in, treino) | como botão | `paper_hover` | **`accent` preenchido**, texto `on_accent` | | hoje liga em azul-wash; passa a "lâmpada" |
| **Segmented** (`ui::segmented`) | fundo `paper_raise`, borda `line_strong`, R1; segmentos texto `ink_soft` | segmento `paper_hover` | segmento ativo `paper_hover` + texto `ink` + **linha inferior 2 px** `accent` (ou `accent2` se `blue`) | | deixa de preencher o segmento inteiro de cor |
| **Chip** (`widgets::chip`, `render.rs::chip`) | `paper_raise`, borda `line`, R1, 11 px | `paper_hover` | borda e texto `accent2`, fundo `accent2_wash` | | |
| **Chip de pessoa (caneta)** | igual chip + bolinha 8 px na cor | | fundo = cor 18 %, borda = cor | | sem pílula |
| **Abas do editor** | texto `ink_soft` 12 px, h 28, sem fundo | texto `ink` | texto `ink` + **sublinhado 2 px `accent`** | | sobre `chrome`; faixa com `border_b` `line` |
| **Cabeçalho de painel** (`ui::panel_header`) | h 22, fundo `header`, texto 10 px caixa alta `ink_soft`, botões à direita (h 18) | | | | MÚSICAS, INSPECTOR, grupo do mixer, cartões de Kit/Config |
| **Popover / menu** | `paper`, borda `line_strong`, R0, `shadow_sm`, p 8 | | | | título em cabeçalho de painel |
| **Paleta Ctrl+K / which-key** | `paper`, R0, borda `line_strong`; linha selecionada `accent2_wash` + barra esquerda 2 px `accent2`; grupo 10 px caixa alta `ink_faint` | | | | |
| **Nav rail** | 64 px, itens 56×52 empilhados: glifo 18 px + rótulo 10 px, texto `ink_soft`, sem borda | `paper_hover` | texto `ink` + **barra esquerda 2 px `accent`** (sem wash) | | glifos monocromáticos (`▦ ◉ ≡ ◎ ⚙`) até haver `AssetSource` para SVG; `Ctrl+B` esconde |
| **Barra de cima** | h 44, `chrome`, `border_b` `line`; esquerda: marca 24 px (`accent`, "太" 14 px) + ☰ + título 16 px Mincho; **centro: transporte**; direita: metrônomo, count-in, treino (+chips), tema | | | | ver layout 2.6 |
| **Transporte** (centro) | `⏮` ir ao início · `▶/■` (primário, w 72) · `↺` loop (toggle) · **display** (`display`, R1, h 26): `c12 · 3.2` (posição: compasso · tempo.sub; tocando mostra o audível) e `180` BPM (roda = ±1; clique = digitar) em 13 px bold tabular `accent2` · `♪ 110` quando soa outro BPM · reto/swing (segmented) | | | | o display é a única "tela" da UI, como no Bitwig |
| **Status bar** | h 22, `chrome`, `border_t` `line`, 11 px `ink_soft`; segmentos separados por divisor 1 px `line` (não "·") | | chip de modo: h 16, R1 | | SIMPLES `paper_raise`; NORMAL `ink`/`chrome`; INSERT `accent`; VISUAL `accent2` |
| **Cabeçalho de seção** | h 28, `paper`, R0, `border_b` `line`; ⠿ · nome 12 px bold · `2×` · `8 compassos` · botões h 20 que **aparecem em hover** (⧉, ✕, −/+) | `paper_hover` | região selecionada: `accent2_wash` + barra esquerda 2 px `accent2`; seção do cursor: nome em `ink` (hoje `accent`) | | clique seleciona região (igual) |
| **Faixa de dinâmica / andamento** | h 22; chip `pp…ff` em `paper_raise` R1 (marcado: `accent2_wash`); `<` `>` 16×16 R1 (ligado `accent2` preenchido); chip ♩ R1 (marcado `accent_wash`); cunha `accent2` 1 px | | | | |
| **Régua** | h 22; números de tempo 10 px `ink` (forte) / `ink_faint`; linhas verticais de tempo descem 4 px; **faixa de loop** nos 6 px de cima: vazia = `line` 1 px; com loop = `accent2` 60 % + alças 6×6 nas pontas; clique = playhead (igual hoje); arraste no corpo = rolar (igual); **arraste na faixa de loop = definir loop** | | | | ver F2 |
| **Rótulo de linha** (coluna esquerda) | **listra de cor 4 px** na borda esquerda com a altura da linha (como o track header do Bitwig); nome 12 px bold `ink`; instrumento 10 px `ink_faint`; `⚙` `M` `S` `★` `×` como 20×16 R1; mini-medidor 3 px sob o nome (só tocando) | | **M ligado `warn`**, S `accent2`, ★ `accent` (todos preenchidos, texto `on_accent`) | linha inaudível: nome `ink_faint` | trilhos de pessoa continuam à esquerda (3 px cada, na cor da pessoa) |
| **Célula vazia** | `cell_empty`; início de tempo = 1 px `line_beat` (mais escuro); subdivisão = fresta de 1 px mostrando `bg` | | | | |
| **Célula com nota** | `cell_filled`; barra de golpe no rodapé h 10, R0, `accent` (couro) / `accent2` (borda), alfa 0.45–1 por velocidade (igual); sílaba 12 px `ink` em cima; sfz = quadrado 4×4 `accent` no canto; nota de dono = faixa 3 px no topo na cor da pessoa | | | nota em mudo: véu `bg` 60 % (igual) | sílaba inválida: contorno 1 px `danger` |
| **Cursor** | contorno 2 px `accent`, R0; modo normal/visual: + wash `accent` 18 % | | editando: caixa `accent_wash` + texto 13 px | | |
| **Seleção** | `accent2` 16 % + contorno 1 px `accent2`, R0 | | arraste de bloco: destino `ink` 12 % (mover) / `accent` 12 % (copiar) | | |
| **Célula tocando (flash)** | contorno 1 px `playhead` + wash da cor do golpe 35 %, R0, sem halo | | | | hoje: halo 5 px R7 + borda 3 px |
| **Playhead** | linha 1 px `playhead` + triângulo 10 px `accent` na régua | | | | |
| **Esteira** | fundo `bg`; cada lane: `cell_empty` com divisores `line`, R0 (hoje cantos 6 px à direita); tambor: face `paper_hover`, anel `ink`; discos e pétalas iguais (conteúdo) | | | | `fuchi` → `accent2` |
| **Partitura** | papel `bg`, pauta `ink`, playhead `accent`; cartão R0 com borda `line` | | | | |
| **Mixer — strip** | w 84, `paper`, R0, `border_r` `line`; topo: **cabeçalho de 22 px preenchido com a cor da trilha** (nome 11 px bold em `on_accent` escuro — como o mixer do Bitwig com "fill header"), instrumento 10 px abaixo; **fader** vertical 160 px: trilho 4 px `line_strong`, preenchido `ink_soft`, cap **retangular 12×22** `paper_hover` com linha central 1 px `ink`; escala de dB (0, −6, −12, −24) em 9 px `ink_faint` à esquerda; **medidor** 6 px à direita do fader (verde/âmbar/vermelho, pico 1 px `ink` que cai); valor `−6 dB` 10 px; pan: trilho horizontal 4 px com marca central, thumb 6×12; `M S` 20×16 | | mudo: strip esmaecido (igual) | | |
| **Mixer — grupo de pessoa** | cabeçalho de painel com swatch 10×10 na cor, nome, barramento (fader horizontal + M/S) | | | | R0, sem barra lateral de 4 px (vai para o cabeçalho) |
| **Mixer — master** (novo) | strip à direita, w 96: fader de saída (`set_master_gain`), medidor L/R 2×6 px, LED de clip 8×8 `danger` (acende 1 s) | | | | |
| **Slider horizontal** (Pessoas, Kit, Config) | trilho 4 px `line_strong`, preenchido `accent2`, thumb 6×14 R1 | | foco: contorno 1 px `accent2` | | thumb deixa de ser redondo |
| **Cartão** (Kit, Config, Pessoas) | `paper`, borda `line`, R0, cabeçalho de painel | | | | |
| **Toast / export** | chip h 18 R1: `accent_wash`/`accent` (export), `accent2_wash`/`accent2` (aviso), `danger` 20 % (erro) | | | | status bar e painel |

### 2.5 Medidor de nível (`ui::meter`)

- Entrada: nível linear (0..1+) e pico retido (0..1+); desenha vertical (w 6) ou horizontal (h 3).
- Escala em dB: 0 dB = topo; −60 dB = base; mapa `y = (db + 60) / 60` (piso em −60).
- Três bandas pintadas como `quad`s R1: `meter_lo` até −18 dB, `meter_mid` até −6 dB,
  `meter_hi` até 0; acima de 0 dB o LED de clip (`danger`) acende por 1 s. (Bitwig e Live usam um
  gradiente verde → amarelo → laranja → vermelho; três bandas dão o mesmo sinal com `quad`s.)
- Pico retido: linha de 1 px `ink` que cai; valor numérico do pico só no inspector/mixer master.
- Balística (na UI, em `AppState::poll`): subida instantânea, queda 36 dB/s; pico retido 1,5 s
  e cai a 12 dB/s.
- Fundo do medidor: `cell_empty`; borda 1 px `line`.

### 2.6 Layout

```
┌────┬──────────────────────────────────────────────────────────────┬──────────────┐
│    │ 太 ☰  Sample music      ⏮ ▶ ↺  c12·3.2  BPM 180  reto|swing   🎵 ⏱ 🏃  ☾ │  ← 44 px, chrome
│ ▦  ├──────────────────────────────────────────────────────────────┼──────────────┤
│Edi │ grade  partitura  esteira            (abas sublinhadas)      │ INSPECTOR    │  ← 28 px, chrome
│tor ├──────────────────────────────────────────────────────────────┤ seção · c12  │
│    │ editar|selecionar  colcheia|…  compasso −4/4+  +seção  +linha│ Shime (jiuchi)│  ← 34 px, chrome
│ ◉  ├──────────────────────────────────────────────────────────────┤ …            │
│Pes │ ▌Seção 1  2×  8 compassos                         (hover: ⧉✕)│──────────────│
│    │ dinâmica  mf ⌄ ♩          <  >  │ mp ⌄                        │ MÚSICAS  +nova│  ← cabeçalho 22 px
│ ≡  │ ░░░░ loop ░░░░ 1  e  2  e  3  e  4  e │ 1  e  2  e             │ ▌Sample music │
│Mix │ ▌Shime   ⚙ M S ★ ×  ┃do ko do ko…                             │  yagyo        │
│    │ ▌Chū 1   ⚙ M S ★ ×  ┃DON   KU…                                │              │
│ ◎  │ …                                                            │ exportar imp.│
│Kit ├──────────────────────────────────────────────────────────────┴──────────────┤
│ ⚙  │ SIMPLES │ Seção 1 · c1 · 1.1 · Shime │ 180 BPM │ parado │ ↺ c3–c6 │ gravado │  ← 22 px, chrome
└────┴─────────────────────────────────────────────────────────────────────────────┘
   64 px                                                                  280 px
```

O que muda em relação a hoje:

1. **Transporte no centro** da barra de cima (play, loop, posição, BPM, swing); metrônomo,
   count-in, treino e tema ficam à direita. O título da música fica à esquerda, menor.
2. **Nav rail estreita** (64 px, glifo + rótulo). Ganha 136 px de largura para a grade.
3. **Abas e barra de contexto** ficam sobre `chrome` como faixas planas (sem cartão arredondado),
   continuam fixas no topo do editor.
4. **Painel direito** com dois blocos empilhados, cada um com cabeçalho de painel:
   **INSPECTOR** (contextual; altura máxima 45 % do painel, rola) e **MÚSICAS** (o resto, com
   os botões de exportar/importar no rodapé). Sem inspector implementado, o bloco mostra só
   "seção · compasso · linha · célula" do cursor em texto (etapa F4 completa).
5. **Lista de linhas** à esquerda da grade ganha barra de cor, M/S/★ quadrados e mini-medidor.
6. **Mixer** ganha o strip master e medidores; strips e grupos ficam planos.
7. **Status bar** com divisores e o trecho do loop (`↺ c3–c6`).

Tudo o mais (which-key, hints `Ctrl+Space`, paleta, prompt de texto, páginas Pessoas/Kit/Config)
só troca de tokens e raios.

---

## 3. Backlog de features de DAW

Valor = para um grupo de taiko que ensaia junto (alto/médio/baixo). Esforço neste código:
**P** (horas), **M** (um dia), **G** (vários dias). Arquivos relativos a `crates/`.

| # | Feature | O que é | Valor | Esforço | Onde mexe |
|---|---|---|---|---|---|
| F1 | **Cor por trilha** | cor própria da linha (padrão pelo instrumento), na lista, no mixer, na esteira e no inspector | alto | **P** | `wadawko-core/src/model.rs` (`Track.color`, `TRACK_COLORS`), `ops/tracks.rs`; `wadawko-app/src/views/grid/render.rs`, `mixer.rs`, `lane.rs`, `keys/*` |
| F2 | **Loop por arraste na régua + loop da seleção** | trecho de loop em compassos independente das seções; alças; `Loop: seleção`, `[`/`]` no cursor | alto | **M** | `wadawko-app/src/state.rs` (`Transport.range`), `views/grid/{mod,paint,render}.rs`, `context_toolbar.rs`, `status_bar.rs`, `keys/{actions,registry}.rs`, `assets/keymap-default.json`; `wadawko-core/src/ops/sections.rs` (precedência da região) |
| F3 | **Medidores de nível** por linha, por pessoa e master, com clip | ver o que soa e quem está alto; essencial para mixar | alto | **M** | `wadawko-audio/src/engine.rs` (picos por canal/bus/master), `controller.rs`; `wadawko-app/src/state.rs` (`poll`, balística), `views/widgets.rs` (`meter`), `mixer.rs`, `shell/mod.rs` (overlay dos mini-medidores) |
| F4 | **Inspector contextual** | painel com as propriedades da seção, compasso, linha e célula do cursor; edita sem popover | alto | **M** | `wadawko-app/src/views/inspector.rs` (novo), `right_panel.rs`, `shell/chrome.rs` (chave), `shell/mod.rs` |
| F5 | **Mostrador de posição** no transporte (`c12 · 3.2` e `mm:ss`) | saber onde está sem olhar a grade | alto | **P** | `topbar.rs`, `state.rs` (`playhead`/`cursor`) |
| F6 | **Strip master** (volume de saída, medidor L/R, clip) | hoje não há controle de saída na UI (`set_master_gain` existe) | médio | **P** | `mixer.rs`, `state.rs` |
| F7 | **Metrônomo com acentos** (padrão de tempos forte/fraco/mudo tipo `ABBB` do Reaper, som de shime/kane, subdivisão) | o jiuchi do ensaio; o clique atual é um só | alto | **M** | `wadawko-audio/src/engine.rs` (voz do metrônomo), `state.rs::Transport`, `topbar.rs` (popover) |
| F7b | **Lista de trechos de ensaio** (loops salvos com nome, como as *practice lists* do Soundslice) | "o solo do Ōdaiko", "a entrada do B" — um clique e está em loop com treino | alto | **M** (depende de F2) | `wadawko-core/src/model.rs` (`Song.practice: Vec<{name, start_bar, end_bar}>`), `right_panel.rs`/inspector, paleta |
| F7c | **Count-in a cada volta do loop** (Soundslice) | o grupo respira entre as repetições | médio | **P** | `wadawko-audio/src/engine.rs` (preroll no wrap), `Transport` |
| F7d | **Modo projeção** (seção atual, próximo compasso, andamento em letras grandes — o *Director* do Hydrogen) | projetar na TV do ensaio | médio | **M** | `views/director.rs` (novo), `Page` |
| F8 | **Tap tempo** | marcar o BPM batendo | médio | **P** | `topbar.rs`, `keys/*` |
| F9 | **Templates de música** ("jiuchi + 3 linhas", "ensemble 5") | começar rápido | médio | **P** | `wadawko-core/src/demo.rs`, `wadawko-app/src/state.rs::new_project`, paleta |
| F10 | **Export MIDI** | levar a música para outra DAW/notador (um canal por linha, nota por zona) | médio | **M** | `wadawko-export` (+ crate `midly`), `wadawko-cli`, paleta |
| F11 | **Export de stems** (um WAV por linha/pessoa) | ensaiar com "menos um" | médio | **M** | `wadawko-audio/src/offline.rs`, `wadawko-app/src/shell/io.rs` |
| F12 | **Humanização ajustável** (timing/ganho, 0–100 %) | hoje é constante no engine | médio | **P** | `wadawko-audio/src/schedule.rs` (`CompileOptions`), `settings.rs` |
| F13 | **EQ de 3 bandas por linha** | os strips já têm cadeia de biquads por zona (`FilterChain`) | médio | **M** | `wadawko-audio/src/{schedule,engine}.rs`, `mixer.rs` |
| F14 | **Tooltips com atalho** nos botões | descobrir o teclado | médio | **P** | `ui.rs` (`abtn` lê `registry`) |
| F15 | **Mapa da música** (seções como blocos coloridos numa linha; clique = ir) | navegar música longa | médio | **M** | `views/overview.rs` (novo), `grid/mod.rs` (`ensure_visible`) |
| F16 | **Altura de linha ajustável / zoom vertical** | músicas com 10+ linhas | médio | **P** | `grid/mod.rs` (`CELL_H` → campo), `keys/*` |
| F17 | **Seções por referência / arranger** ("A A B A" sem clonar) | reuso de padrões (Renoise/FL) | alto | **G** | `wadawko-core` (modelo + `timeline::play_order` + ops), grade, partitura, export — muda o formato |
| F18 | **Dinâmica por célula** (acento/ghost por nota além de CAIXA ALTA/`!`) | expressão fina; planejado em `/planejamento/gpui.md` §1.2 | médio | **M** | `wadawko-core` (`Measure.cell_dynamics`), `schedule.rs`, `paint.rs` |
| F19 | **Automação desenhável de volume por linha** | curvas livres; as dinâmicas por compasso + hairpins já são "automação em degraus" | baixo | **G** | modelo, `schedule.rs`, lane nova na grade |
| F20 | **Automação de tempo desenhável** | rall./accel. por compasso já cobrem | baixo | **M** | — |
| F21 | **Marcadores/locators nomeados** | seções já cumprem o papel (`(`/`)`, saltos) | baixo | **M** | modelo, régua, `keys/*` |
| F22 | **MIDI in** (pads/e-drum para digitar golpes) | entrada ao vivo | médio | **G** | crate `midir`, `engine.rs`, `grid/mod.rs` |
| F23 | **Gravação de áudio** | gravar o grupo tocando por cima | baixo | **G** | `wadawko-audio` (input), modelo |
| F24 | **Reverb/efeitos de envio** | ambiente | baixo | **G** | engine (DSP novo) |
| F25 | **Swing por trilha** | raro em taiko | baixo | **M** | modelo, `swing_warp` |
| F26 | **Histórico de undo visual** | — | baixo | **M** | `wadawko-store::History`, painel |
| F27 | **Painéis destacáveis / multi-monitor** | — | baixo | **G** | janelas GPUI |
| F28 | Grupos de trilhas (pastas) | pessoas já agrupam | baixo | — | — |
| F29 | Punch in/out, comping, freeze, time-stretch, follow actions | não se aplicam (não há gravação/áudio linear) | — | — | — |

### 3.1 Corte para implementar AGORA (esta noite)

O redesign (parte 2) é obrigatório e vem primeiro — é o que o Fernando vai ver de manhã. Com ele
entram **três features** de alto valor e esforço P/M, sem dependências novas, nesta ordem:

1. **F1 Cor por trilha** (P). É o que mais muda a "cara de DAW" por hora de trabalho: a lista de
   linhas, o mixer e a esteira passam a ter identidade por instrumento. Só um campo opcional no
   JSON (`color`), compatível com arquivos antigos.
2. **F2 Loop por arraste na régua + loop da seleção** (M). Hoje só se repete por seção inteira;
   ensaio real é "esses 2 compassos de novo". O engine já aceita `CompileOptions.region` em
   compassos — falta a UI. Vem com `Loop: seleção` na paleta e `[`/`]` para o teclado.
3. **F3 Medidores de nível** (M). Um mixer sem medidor não é um mixer; o engine já soma por canal
   e por barramento em blocos de 128 frames — capturar o pico é uma linha por estágio. Dá vida à
   tela enquanto toca (mini-medidor por linha) sem custo de re-render (desenho no overlay).

**Se sobrar tempo**: **F4 Inspector** (M), começando pela versão mínima (texto do contexto +
botões já existentes para seção e linha). **F5 Mostrador de posição** entra junto com o
transporte centralizado (etapa E3), porque é parte do layout.

Deixados de fora e por quê: F7 metrônomo com acentos (valor alto, mas mexe na voz do engine e
precisa de popover de configuração — fica para a próxima noite); F17 arranger (G, muda o formato);
F10 MIDI (depende de crate novo).

---

## 4. Plano de implementação

Regras para o agente implementador:

- Trabalhe no branch `design-daw`. Commits pequenos, um por etapa, mensagem em português.
- **Nunca** `window.refresh()` em animação ([regras de desempenho](/arquitetura/app.md)); o que
  anima vai no overlay do Shell ou numa view com `request_animation_frame` própria.
- Antes de começar, uma cópia do banco: `cp ~/.local/share/wadawko/wadawko.db /tmp/teste.db`
  ([testes e depuração](/desenvolvimento/testes-e-depuracao.md)). Todos os comandos abaixo
  usam `--db /tmp/teste.db --mute`.
- **Verificação padrão de cada etapa** (abreviada como *VP* abaixo):
  `cargo fmt && cargo clippy --workspace -- -D warnings && cargo test --workspace` e
  `cargo run --release -p wadawko-app -- --db /tmp/teste.db --mute --demo probe | grep -c FALHOU`
  deve ser 0 (o probe sai com código ≠ 0 em falha).
- **Captura de tela**: `cargo run --release -p wadawko-app -- --db /tmp/teste.db --mute --dark --demo MODO`
  (modos `play`, `mixer`, `esteira`, `partitura`, `palette`, `menu`, `dyn`) e capture a janela
  com a ferramenta do compositor (`grim -g "$(slurp)"` no wlroots/Hyprland, `spectacle -a` no KDE).
  Guarde em `/tmp/shots/<etapa>-<modo>.png` e **olhe a imagem** (ferramenta Read) antes de
  seguir. No fim, refaça `docs/imagens/*.png` com os mesmos modos (o README as embute).
- Ao mudar teclas ou títulos: `python3 tools/gen-atalhos.py`; ao mudar docs:
  `python3 tools/okf-check.py docs --write-index`.

### E0 — Linha de base (15 min)

1. VP no estado atual; capture `play`, `mixer`, `esteira` como linha de base em `/tmp/shots/e0-*.png`.
2. Rode `--demo playstat-grade` e anote "ms de CPU por frame" (comparar no fim; limite +5 %).

### E1 — Tokens (theme.rs, ui.rs) · 45 min

1. `theme.rs`: acrescente os campos novos da tabela 2.2 (`chrome`, `header`, `display`,
   `cell_filled`, `line_beat`, `line_bar`, `playhead`, `warn`, `danger`, `meter_lo`, `meter_mid`,
   `meter_hi`) e troque **todos** os valores de `dark()` e `light()` pelos da tabela. `on_accent`
   escuro no tema escuro.
2. `ui.rs`: `pub const R0: f32 = 0.0; pub const R1: f32 = 2.0;` e helpers novos:
   `panel_header(title, th) -> Div` (22 px, `header`, caixa alta 10 px), `tab(id, label, on, th, action)`
   (texto + sublinhado), `icon_btn(id, glyph, th, action)` (26×26), `meter(level, peak, vertical, th)`
   (2.5). `btn_bare`: h 26, px 10, texto 12, R1. `kbd`: h 16, texto 10. `small_btn`: 20×16, R1.
3. `main.rs:164`: `Ok(None)` → escuro (`unwrap_or(true)` na ausência de tema salvo).
4. **Teste** em `theme.rs`: para cada tema, contraste WCAG (luminância relativa) `ink`/`bg` ≥ 7,
   `ink_soft`/`paper` ≥ 4.5, `on_accent`/`accent` ≥ 4.5; e que nenhum token tem alfa < 1.
5. **Pronto quando**: VP passa; o app abre escuro num banco novo; o botão ☀/☾ alterna.

> **Decisões na implementação (2026-11-17)**: no tema claro, `accent` ficou `#c94531`
> (o `#d9503c` da tabela dava 4,07 de contraste com o texto branco; 4,80 agora) e `warn`
> ficou `#946808` (o `#b8860b` dava 3,25 com o "M" branco; 4,95 agora). O teste de
> contraste cobre também `on_accent`/`warn`. `tokens()`, `luminance` e `contrast` são
> `cfg(test)`. O `fuchi` fixo da esteira já passou a `th.accent2` nesta etapa.

### E2 — Raios (mecânico) · 30 min

1. Troque as ~56 chamadas `rounded(px(N))` em `crates/wadawko-app/src/**` por `rounded(px(ui::R0))`
   (superfícies) ou `rounded(px(ui::R1))` (controles), conforme a tabela 2.3. Casos: `kit.rs:16`
   e `people.rs:16` (cartões → R0), `nav_rail.rs:8` (→ R0), `right_panel.rs` 8/999 (→ R0/R1),
   `context_toolbar.rs` 10/5/999 (→ R0/R1/R1), `status_bar.rs` 4 (→ R1), `palette.rs` 12/8/6
   (→ R0/R1/R1), `whichkey.rs` 10 (→ R0), `settings.rs` 6 (→ R1), `score.rs` 10 (→ R0),
   `mixer.rs` 10/3 (→ R0/R1), `widgets.rs` 12/5/6 (→ R1), `grid/render.rs` 10/8/6/5/4/1.5 (→ R0 para
   popover e cabeçalho; R1 para chips/botões; trilho de pessoa 0).
2. `grid/paint.rs`: o helper `rounded(rect, raio, cor)` passa a receber `ui::R1` nos chips e `0.0`
   nas barras de golpe (troque o `Corners {2,2,3,3}` por `px(0.0)`), no sfz e no cursor (`px(0.0)`).
   `widgets.rs::slider`: thumb retangular (6×14 horizontal, 12×22 vertical), R1; anel de foco → contorno.
   `shell/mod.rs` (overlay): halo do flash removido; flash = contorno 1 px `playhead` + wash 35 %.
   `lane.rs:612`: cantos do campo → 0.
3. **Pronto quando**: `grep -rn 'rounded(px(' crates/wadawko-app/src | grep -v 'ui::R'` retorna
   vazio; VP passa; capture `play` e compare com E0.

### E3 — Moldura: barra de cima, nav rail, abas, contexto, status, painel · 2 h

1. `topbar.rs`: h 44; esquerda (marca 24 px, ☰, título 16 px); **centro** (`div().flex_1()` com
   `justify_center`): `⏮` (`transport::PlayFromStart` já existe → cria `transport::GoToStart` que só
   faz `seek_cell(0)` sem tocar), play/stop primário, `↺` toggle, **mostrador de posição** (F5:
   `c{m+1} · {beat}.{sub}` do playhead audível quando toca, senão do cursor da grade) e o campo
   BPM, ambos num **display** `th.display` R1 h 26 com texto 13 px bold `accent2` (w 88 + w 56),
   `♪ 110`, reto/swing; direita: metrônomo, count-in,
   treino + chips, tema. `shell/mod.rs:728`: altura 44. Chave de cache (`refresh_chrome`): inclua
   a posição mostrada (arredondada à célula) para a barra atualizar tocando.
2. `nav_rail.rs`: 64 px, itens empilhados (glifo + rótulo 10 px), ativo com barra esquerda 2 px.
   `shell/mod.rs:727`: largura 64.
3. `topbar.rs::view_tabs` → `ui::tab` com sublinhado; o cabeçalho do editor (`shell/mod.rs:735–760`)
   passa a `bg(th.chrome)` sem padding lateral extra, abas em faixa de 28 px com `border_b`.
4. `context_toolbar.rs`: sem cartão (`rounded`/borda fora), h 34, `bg(th.chrome)`, `border_b`;
   `segmented` novo (sublinhado); caneta e região como chips R1.
5. `status_bar.rs`: h 22; divisores `div().w(px(1)).h(px(12)).bg(th.line)` no lugar de "·";
   `shell/mod.rs:814`: 22.
6. `right_panel.rs`: 280 px; `ui::panel_header("MÚSICAS", …)` com `+ nova` (h 18) à direita;
   cartões de música R0 com barra esquerda 2 px `accent2` na atual; rodapé de exportar/importar.
   Acima, um bloco `INSPECTOR` provisório (cabeçalho + uma linha de texto com o contexto do cursor:
   "Seção 1 · c3 · 2.1 · Shime"), que F4 substitui. `shell/mod.rs:813`: 280.
7. `keys/palette.rs`, `keys/whichkey.rs`, `grid/render.rs::popover`: `paper`, R0, borda
   `line_strong`, `shadow_sm`; linha selecionada da paleta com barra esquerda `accent2`.
8. **Pronto quando**: VP passa (o probe clica na barra de cima em cache — se mudar a posição do
   botão de play, ajuste o passo correspondente de `run_probe`); capture `play`, `palette`,
   `menu`; o mostrador de posição anda tocando.

> **Decisões na implementação (2026-11-17)**: o loop no transporte é só o glifo `↺` (30 px) para
> caber ao centro; o display traz o rótulo "BPM" em 9 px antes do número; `transport::GoToStart`
> ("Ir ao início (sem tocar)") faz `start_cell = 0` + `seek_cell(0)`, sem tecla padrão; as abas
> têm ids `tab-grade`/`tab-partitura`/`tab-esteira`; o bloco INSPECTOR provisório mostra o mesmo
> texto de posição da status bar (`StatusModel::pos_text`), que entrou na chave de cache do
> painel; o which-key foi reposicionado para a rail de 64 px e o painel de 280 px. Os ids que o
> probe usa (`hamb`, `loop`, `title`, `play`, `bpm`) não mudaram.

### E4 — Grade: constantes, pintura, rótulos, régua · 2 h

1. `grid/mod.rs`: `SEC_HEAD_H 28`, `SEC_DYN_H 22`, `TIMELINE_H 22`, `CELL_H 36`, `TDYN_H 20`,
   `KYAI_H 22`, `ROW_PAD 6`, `SEC_GAP 20`. `Rows`, `hit`, `cell_bounds`, `visible_cells` já
   derivam disso — nada mais muda. Rode o probe: ele usa `cell_bounds`, então se adapta.
2. `grid/paint.rs`: células `cell_empty`/`cell_filled`; início de tempo 1 px `line_beat`
   (hoje 2 px `line_strong`, mais claro — passa a escurecer); subdivisão sem linha; barra de
   compasso 2 px `line_bar`; barras de golpe R0 h 10; sfz 4×4; cursor/seleção/flash conforme 2.4;
   números da régua 10 px.
3. `grid/render.rs`: cabeçalho de seção h 28 plano, botões h 20 **visíveis só em hover**
   (`group_hover` do gpui ou um `hovered: Option<usize>` na view com `on_hover`); região
   selecionada = `accent2_wash` + barra 2 px; rótulo de linha com **listra de cor 4 px** (usa
   `track_color` de F1 — até lá, `ink_faint`), M/S/★/× em `small_btn` 20×16 com M `warn`,
   S `accent2`, ★ `accent`.
4. **Pronto quando**: VP passa; capture `play` e `dyn`; `--demo playstat-grade` dentro de +5 %.

> **Decisões na implementação (2026-11-17)**: os botões do cabeçalho ficam com opacidade 0 fora
> do hover (continuam no layout, como alvo de salto e para o probe) e aparecem com
> `GridView::hover_sec` (`on_hover` na barra, notifica só a grade); o ▾ de recolher e o `N×`
> ficam sempre visíveis. A listra de 4 px usa `ink_faint` até a F1; os trilhos de pessoa vêm
> 6 px à direita dela. Os botões de hairpin mantiveram 18 px (o hit test `dyn_part` já os
> espera assim). A régua já pinta a hairline da faixa de loop (6 px de cima) e marcas de tempo
> de 4 px no rodapé; sílaba inválida usa `danger`. `playstat-grade` (debug, janela em tela
> cheia): 6,15–6,91 ms/frame antes → 6,64–6,69 depois.

### F1 — Cor por trilha · 1 h

1. `wadawko-core/src/model.rs`: `Track.color: Option<String>` (`#[serde(default, skip_serializing_if)]`),
   `pub const TRACK_COLORS: [&str; 12]`, `pub fn default_track_color(base: InstrumentId) -> &'static str`
   (tabela de 2.2). `ops/tracks.rs`: `set_track_color(song, id, Option<&str>)`, `cycle_track_color`.
   **Testes**: round-trip JSON com e sem `color`; `default_track_color` cobre todos os `InstrumentId`.
2. `wadawko-app`: `theme.rs::track_color(song, track, th) -> Hsla` (campo → padrão → `parse_color`).
   Usar em: `grid/render.rs` (listra 4 px do rótulo), `mixer.rs` (cabeçalho do strip preenchido,
   texto `on_accent`), `lane.rs` (anel externo do tambor na cor, 2 px), `grid/render.rs::row_menu`
   (linha "cor" com 12 swatches 14×14 R1 + "padrão").
3. Ação `track::CycleColor` (`keys/actions.rs`, `registry.rs`: "Linha: próxima cor", grupo
   Estrutura; sem tecla padrão). `python3 tools/gen-atalhos.py`.
4. **Pronto quando**: VP passa; capture `play`, `mixer`, `esteira`; abrir uma música antiga mostra
   cores por instrumento sem editar o arquivo.

> **Decisões na implementação (2026-11-17)**: `track_color(track)` não precisa da música nem do
> tema (as 12 cores foram escolhidas para os dois fundos); `Track::color_or_default()` dá a cor
> efetiva no core. O roxo da paleta ficou `#ab63d9` (o `#a35ad6` da tabela dava 4,38 de contraste
> com o texto escuro do cabeçalho do strip; teste `track_colors_carry_dark_text` garante ≥ 4,5
> para as 12). O texto sobre a cor da linha é sempre escuro (`Theme::on_track_color()`), nos dois
> temas. Na esteira a cor é um anel de 2 px por fora do aro `ink` do tambor (esmaecido quando a
> linha não soa). Os swatches do popover ⚙ são alvos de salto (`col-{t}-{i}`), e o probe usa
> isso: abre o ⚙ pela ação `track::Settings`, clica num swatch e confere `track.color` + Ctrl+Z.

### F2 — Loop por arraste na régua e loop da seleção · 2 h

1. `state.rs`: `Transport.range: Option<Region>`; `AppState::set_range(Option<Region>, cx)` →
   `sync_audio`. Em `sync_audio` (`state.rs:293`), a região passa a ser
   `tp.range.or_else(|| selected_region(&song, &tp.selected))`. `set_range(Some)` liga o loop
   (`set_loop(true)`) e, se tocando, faz `seek` ao início do trecho se o playhead estiver fora.
   **Teste** puro: `effective_region(range, selected)` precedência.
2. `grid/mod.rs`: `Hit::Timeline` ganha a variante `Hit::LoopLane(m)` quando `y < 6.0` dentro
   da régua; `mouse_down` nela inicia `loop_drag: Option<(usize, usize)>` (compasso de origem,
   atual); `mouse_move` atualiza; `mouse_up` chama `set_range(Some(Region{start_bar: min,
   end_bar: max + 1}))`; clique sem arrasto na faixa com loop existente = alterna `looping`;
   clique com `Alt` = limpa. Alças: se o clique cair a ≤ 6 px de uma ponta do trecho, arrasta só
   aquela ponta. **Teste** puro: `range_from_drag(a, b)` e `grab_handle(x, x0, x1)`.
3. `grid/paint.rs`: nos 6 px de cima da régua, para cada compasso no trecho, pinta `accent2`
   60 %; alças 6×6 `accent2` nas pontas; trecho em arrasto pinta com 35 %.
4. Ações: `transport::LoopSelection` ("Loop: seleção" — compassos da seleção ou do cursor;
   teclas `space t l` no vim e `ctrl-l` no simples — a mesma tecla do Live), `transport::LoopStartHere`
   / `LoopEndHere` (`[` / `]` no modo normal), `transport::ClearRegion` passa a limpar também o trecho.
   `context_toolbar.rs`: chip `↺ c3–c6 ✕` (clique = limpar); `status_bar.rs`: `↺ c3–c6`
   (entra no `StatusModel`). `python3 tools/gen-atalhos.py`.
5. **Probe**: passo novo em `run_probe` (`main.rs`): arrastar na faixa de loop do compasso 2 ao 3
   e verificar `tp.range == Some(Region{1, 3})` e `tp.looping`; `Alt+clique` limpa.
6. **Pronto quando**: VP passa com o passo novo; tocando com loop de 2 compassos, o playhead
   volta no lugar certo (ouça com `--mute` desligado uma vez); capture `play`.

> **Decisões na implementação (2026-11-17)**: a faixa de loop tem 8 px de alvo (`LOOP_LANE_H`;
> a banda pintada tem 6) — 6 px era pequeno demais para o mouse. O arrasto é acompanhado pelo
> Shell em toda a janela (overlay com `on_mouse_event`, como o bloco arrastado): dá para
> arrastar da régua de uma seção até a de outra (trecho atravessa seções). Sem arrasto: clique
> fora do trecho marca só aquele compasso; dentro, alterna o loop; Alt+clique limpa. Banda a
> 60 % com loop ligado, 25 % desligado; prévia do arrasto a 35 %. `set_range` mantém o playhead
> onde está se ele ainda cai no trecho novo, senão `seek` ao início dele (não reinicia o play).
> Teclas: `ctrl-l` em todos os modos (o `space t l` já era o toggle do loop, então o chord ficou
> `space t shift-l`); `[`/`]` só no normal/visual. `transport::ClearRegion` limpa o trecho e as
> seções. Modo `--demo loop` para captura. Probe: 3 passos novos (arrasto c2→c3 + Alt+clique;
> `Loop: seleção` + limpar; playhead fica dentro do trecho tocando) — a janela do probe é estreita,
> então o passo aproxima o zoom e rola até o compasso 2 antes de arrastar.

### F3 — Medidores de nível · 2 h

1. `wadawko-audio/src/engine.rs`: `pub struct Meters { chan: [AtomicU32; MAX_CHANNELS],
   bus: [AtomicU32; MAX_BUSES], master: [AtomicU32; 2], clip: AtomicBool }` (bits de `f32` ≥ 0 —
   `fetch_max` em `u32` preserva a ordem). Em `render_block`: estágio 2, por canal, pico de
   `|x·g|` do bloco (`chan[s/2]`); estágio 3, por barramento; estágio 5, `master` depois do
   soft clip e `clip` se `|l|` ou `|r|` ≥ 0.999 **antes** do clip. Sem alocação, sem lock.
   `Controller::meters() -> &Arc<Meters>`; a UI lê com `swap(0)` (pico desde a última leitura).
   **Teste**: compila uma música de um golpe, roda `process` por 0,2 s, `meters.chan[slot] > 0`
   e `master > 0`; render offline não muda (hash estável).
2. `state.rs`: `pub levels: Levels { chan: Vec<(f32 /*nível*/, f32 /*pico*/, Instant)>, bus,
   master: [(f32, f32); 2], clip_at: Option<Instant> }`; em `poll()`, lê os medidores e aplica a
   balística de 2.5. Mapa linha → slot: `schedule.channel_slot(&channel_key(track, owner))`
   (linha compartilhada: máximo entre os donos).
3. `widgets.rs::meter` (ou `ui::meter`); `mixer.rs`: medidor 6 px ao lado do fader, strip
   **master** (F6, fader de saída via `set_master_gain`, L/R, LED de clip); barramento da pessoa
   com medidor horizontal. O Mixer passa a pedir `request_animation_frame` enquanto toca.
4. Mini-medidor por linha na grade: desenhado no **overlay** do Shell (`shell/mod.rs`, junto dos
   flashes) com a geometria da grade (`geom.sections` + `rows.y`), 3 px sob o nome; nunca dentro
   da grade em cache.
5. **Pronto quando**: VP passa; capture `mixer` tocando e `play`; `--demo playstat-grade` e
   `--demo playstat-esteira` dentro de +5 % do E0.

> **Decisões na implementação (2026-11-17)**: `Meters` vive no `Engine` (criado em
> `with_clock`) e o `Controller` guarda o mesmo `Arc` (`meters()`); o pico por canal é medido
> já com o ganho/pan aplicados (|x·g|, as duas zonas somam no mesmo slot) e o do master depois
> do soft clip; o LED usa 0,999 antes do soft clip. A UI lê com `Meters::take` (swap 0) em
> `AppState::poll` e aplica a balística em `state::Level` (36 dB/s; pico retido 1,5 s e cai a
> 12 dB/s; piso −60 dB) — teste `level_rises_instantly_and_falls_36_db_per_second`. O Shell
> pede frames enquanto `levels.active()` (os medidores caem depois de parar). Os mini-medidores
> da grade são pintados no overlay com `ui::paint_meter` (3 px sob o nome, largura =
> `LABEL_W − label_pad − 150`; `Geom.label_pad` vem do render da grade). O mixer já ficou no
> visual da E5 (strip 84 px, fader 160 com escala de dB e medidor de 6 px, cabeçalho de painel
> por grupo, strip **master** com fader de saída (`AppState::master_db`, não é edição da
> música), medidor L/R e LED de clip). Teste do engine:
> `meters_report_channel_bus_and_master_peaks` (o clip precisa de 8 linhas iguais a +6 dB — o
> compressor do master segura uma só). Probe: "medidores: master e linha mostram nível
> tocando" (20 verificações). `playstat-grade` 5,9–7,1 ms/frame (antes 6,0–6,8) e
> `playstat-esteira` 5,6–6,2 (antes 6,2): sem regressão.

### F4 — Inspector (se sobrar tempo) · 2 h

1. `views/inspector.rs` (`impl Shell { fn inspector(&self, th, cx) -> Div }`): blocos
   **SEÇÃO** (nome + ✎, `2×` −/+, compassos −/+, ⧉, ✕, "só esta" — tudo com `abtn` das ações
   `section::*`), **COMPASSO** (`4/4` −/+, chips pp…ff, `<` `>`, ♩ rall./accel./a tempo),
   **LINHA** (nome, instrumento ▾, cor, M S ★, volume/pan com `widgets::slider`, dinâmica própria,
   kiai, pessoas), **CÉLULA** (sílaba, golpes resolvidos "D couro · E borda", dono, sfz, acento).
   Fonte: cursor da grade + `tp.selected` + `revision`.
2. `right_panel.rs`: substitui o bloco provisório; `shell/chrome.rs` chave do `Panel` inclui
   cursor, seleção, `revision`.
3. **Pronto quando**: VP passa; capture `play` com o cursor numa célula; editar pelo inspector
   desfaz com `Ctrl+Z`.

### E5 — Mixer, Pessoas, Kit, Config (restyle) · 1 h

Strips e grupos conforme 2.4 (planos, cap retangular, escala de dB); cartões com
`ui::panel_header`; sliders com thumb retangular. Capture `mixer`, `pessoas`, `kit`, `config`.

### E6 — Esteira e partitura · 30 min

`lane.rs`: `fuchi` → `th.accent2`, lanes R0, divisores `line`; `score.rs`: cartão R0. Capture
`esteira`, `partitura`.

### E7 — Fechamento · 45 min

1. Refaça `docs/imagens/{grade,partitura,esteira,paleta,mixer}.png` (tema escuro, mesmos modos).
2. `CHANGELOG.md` ("Não lançado" → Adicionado: cor por trilha, loop por trecho, medidores,
   master, mostrador de posição[, inspector]; Mudado: tema escuro padrão, visual plano).
3. Docs: [guia de uso](/guias/uso.md) (loop por arraste, cor, medidores, transporte),
   [arquitetura do app](/arquitetura/app.md) (tokens novos, `ui::R0/R1`, medidores no overlay),
   `docs/log.md`; `python3 tools/okf-check.py docs --write-index`; `python3 tools/gen-atalhos.py`.
4. VP final + `--demo playstat-*` comparado com E0.

### Ordem e tempo

| Ordem | Etapa | Tempo | Entrega visível |
|---|---|---|---|
| 1 | E0 linha de base | 0:15 | capturas de referência |
| 2 | E1 tokens | 0:45 | app escuro neutro |
| 3 | E2 raios | 0:30 | cantos retos |
| 4 | E3 moldura | 2:00 | transporte central, rail estreita, abas, painel |
| 5 | E4 grade | 2:00 | grade densa, rótulos com M/S quadrados |
| 6 | F1 cor por trilha | 1:00 | barras de cor na lista, mixer, esteira |
| 7 | F2 loop por trecho | 2:00 | arraste na régua, `[`/`]`, chip na barra |
| 8 | F3 medidores | 2:00 | mixer com medidores e master, mini-medidores |
| 9 | E5 + E6 restyle | 1:30 | mixer/kit/pessoas/esteira/partitura |
| 10 | E7 fechamento | 0:45 | capturas, changelog, docs |
| (11) | F4 inspector | 2:00 | painel contextual |

Total sem o inspector: ~12 h 45. Se o tempo apertar, a ordem garante que cada parada deixa o app
coerente: depois de 5 (E4) já é "o novo visual"; 6–8 são as features; 9–10 fecham.

## Riscos

- **Chaves de cache da moldura** (`refresh_chrome`): tudo que a barra de cima, o painel ou a
  status bar passam a mostrar (posição, loop, cores) precisa entrar na chave, senão fica velho.
- **Probe e coordenadas**: as alturas novas movem tudo; o probe usa `cell_bounds` e os hints, mas
  passos com coordenadas fixas (barra de cima) precisam de ajuste.
- **Medidores no thread de áudio**: só `fetch_max`/`store` em atômicos; nada de `Vec` nem
  `Mutex`. Medir com `--demo playstat-*`.
- **`on_accent` escuro**: conferir o texto do botão de play e dos chips de modo no tema claro
  (lá continua branco).
