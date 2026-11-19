---
type: Plan
title: "Pesquisa de UX: o que DAWs, trackers e apps de ensaio ensinam ao WaDAWko"
description: "Pesquisa de features e padrões de UX em DAWs (Bitwig, Live, Reaper, Logic, FL), trackers (Renoise, Polyend, M8), editores de bateria e notadores (Hydrogen, EZdrummer, MuseScore, Dorico) e apps de ensaio (Soundslice, Anytune, apps de drumline), com recomendações priorizadas para compor e ensaiar taiko em grupo."
tags: [planejamento, ux, pesquisa, daw, tracker, ensaio, pessoas, backlog]
status: draft
estado: "pesquisa (2026-11-18)"
---

# WaDAWko — pesquisa de UX para compor e ensaiar taiko em grupo

Data: 2026-11-18 · Status: **pesquisa** (nada implementado) · Pedido: *"pesquise features e
padrões de UX de DAWs, trackers e editores de bateria que melhorariam a experiência do WaDAWko e
escreva recomendações priorizadas."*

Este documento parte do que **já existe** ([guia de uso](/guias/uso.md), backlog F1–F29 em
[daw.md](/planejamento/daw.md), editor vertical em [tracker.md](/planejamento/tracker.md), issues
abertas #6–#27) e só recomenda o que falta. Quando uma recomendação coincide com um item já
levantado, o número dele (F17, F26, #27…) aparece ao lado, e a prioridade é reavaliada à luz do
uso em grupo.

## Resumo executivo

1. O app já cobre bem **o compositor sozinho** (tracker, efeitos, paleta, vim) e o **ensaio
   coletivo na TV** (projeção, loop, treino, count-in). O que mais falta é **o ensaio de cada
   pessoa**: hoje ninguém consegue abrir "a minha parte", ouvir "a música sem mim" ou levar um
   áudio de estudo para casa.
2. As três recomendações de maior retorno saem de apps de ensaio, não de DAWs: **"Ensaiar como…"**
   (mute-me de um clique, do Soundslice/Drumr), **visão e partes por pessoa** (layouts do Dorico,
   partes do MuseScore) e o **pacote de ensaio** (Region Render Matrix do Reaper: trechos × pessoas).
3. Na edição, o que trackers e step sequencers têm e o WaDAWko não tem é **preencher padrões**
   (Advanced Fill do FL, Fill do Polyend, Euclidean do Live 12) — ideal para jiuchi e kizami — e
   **transformações de seleção** (Advanced Edit do Renoise: dobrar/meio tempo, espelhar).
4. Para reuso, uma **biblioteca de frases do usuário com tags** (EZdrummer, Collections do Live)
   resolve 80 % do que se espera de "seções por referência" (F17) a uma fração do custo.
5. O **roteiro de ensaio** (practice lists do Soundslice + Region Playlist do Reaper com nº de
   voltas) transforma os trechos com nome em um plano de ensaio que toca sozinho.
6. São 22 recomendações; 8 delas são **P** (até um dia). A lista do que **não** fazer fecha o
   documento: mixagem fina, síntese, probabilidade, clip launcher e IA generativa não servem a um
   grupo de taiko.

## 1. Pesquisa — o que cada fonte ensina

Resumo por tema; as fontes de [daw.md §1.4](/planejamento/daw.md) e
[tracker.md §1](/planejamento/tracker.md) não são repetidas.

### 1.1 Painel de faixas, grupos e organização

- **Logic — Track Stacks**: *folder stacks* agrupam faixas para controlá-las como uma só sem
  mudar o roteamento; *summing stacks* somam num bus
  ([Track Stacks overview](https://help.apple.com/logicpro/mac/10.4/en.lproj/lgcp9bc4b63d.html)).
- **Bitwig** tem um ícone por tipo de faixa e grupos aninhados, mas não tem busca/filtro de faixas
  — usuários pedem "search/filter channels by name"
  ([Intro to Tracks](https://www.bitwig.com/userguide/latest/intro_to_tracks),
  [Gearspace](https://gearspace.com/threads/bitwig-for-mixing.1457067/)).
- **Dorico** separa *players* de *instrumentos*: um player pode segurar vários instrumentos e o
  programa mostra as trocas ([Players](https://archive.steinberg.help/dorico_se/v4/en/dorico/topics/program_concepts/program_concepts_players_c.html),
  [Instrument changes](https://steinberg.help/dorico_se/v5/en/dorico/topics/setup_mode/setup_mode_instrument_changes_c.html));
  a apresentação do kit é **por layout** — pauta de 5 linhas na grade, uma linha por instrumento
  na parte ([kits](https://www.steinberg.help/r/dorico-elements/6.1/en/dorico/topics/notation_reference/notation_reference_unpitched_percussion/notation_reference_unpitched_percussion_kits_versus_individual_instruments_c.html)).

### 1.2 Edição rápida de ritmos

- **FL Studio — Advanced Fill Tool**: "Fill every" N passos, deslocamento e "Dice" (aleatório
  dentro dos parâmetros), aberto pelo botão direito do canal; **graph editor** por passo
  (velocity, pan, shift) ([Advanced Fill](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/stepsequencer_advancedfilltool.htm),
  [Channel Rack](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/channelrack.htm)).
- **Polyend Tracker — Fill**: preenche a área selecionada de uma vez, com modos para notas,
  instrumentos e FX, incluindo Random/Random+
  ([Tracker Essentials](https://polyend.com/wp-content/uploads/2023/11/Tracker_Essentials_Rev1.pdf),
  [fórum](https://backstage.polyend.com/t/using-random-sets-all-other-parameters-to-0/3524)).
- **Ableton Live 12 — MIDI Generators/Transformations**: Rhythm, Seed, Shape, Stacks e
  **Euclidean**; Recombine, Ornament (flam), Quantize, Time Warp
  ([SoS: generators](https://www.soundonsound.com/techniques/ableton-live-12-midi-generators),
  [SoS: transformations](https://www.soundonsound.com/techniques/ableton-live-12-midi-transformations?amp)).
  **Capture MIDI**: o Live escuta sempre e um botão transforma o que acabou de ser tocado em clip
  ([Capture MIDI](https://help.ableton.com/hc/en-us/articles/360000776450-Capture-MIDI)).
- **Renoise — Advanced Edit**: Shrink/Expand, Mirror, Quantize, Nudge, **Humanize** com máscara do
  que alterar; **aliases** na Pattern Matrix (edita uma, todas mudam)
  ([Advanced Edit](https://tutorials.renoise.com/wiki/Advanced_Edit),
  [Pattern Matrix](https://tutorials.renoise.com/wiki/Pattern_Matrix)).
- **Logic — Drummer**: região com XY de complexidade × intensidade, slider de fills, swing e
  travas ([Drummer Editor](https://help.apple.com/logicpro/mac/10.4/en.lproj/lgcp898c7532.html));
  **Step Sequencer** com comprimento e step rate por linha (polimetria)
  ([Apple](https://support.apple.com/HT211026)).
- **Hydrogen**: por instrumento, "Randomize velocity" e "Select notes" no menu lateral do pattern
  editor ([Pattern Editor](https://hydrogen-music.org/documentation/manual/manual_en_chunked/ch09.html)).
- **MuseScore 4.5 — painel de percussão**: 8 colunas de pads, cada um com atalho de teclado
  configurável; entra a nota no cursor; layouts salvos em `.drm`
  ([inputting percussion](https://handbook.musescore.org/idiomatic-notation/percussion/inputting-percussion-notation),
  [customizing](https://handbook.musescore.org/pt_br/idiomatic-notation/percussion/customizing-the-percussion-panel)).
- **EZdrummer 3**: biblioteca de grooves filtrável (estilo, fórmula, andamento, mão),
  **Tap2Find** (bata o ritmo e ele busca grooves parecidos), Bandmate (sugestão por IA, recebida
  com reservas) e a *Song Track* para montar o arranjo
  ([SoS](https://www.soundonsound.com/reviews/toontrack-ezdrummer-3),
  [Recording](https://www.recordingmag.com/resources/featured-reviews/july-2022-toontrack-ezdrummer-3/)).

### 1.3 Navegação e visão geral

- **Live — Overview e Follow**: a Overview é um mapa onde se arrasta e dá zoom; `W` ajusta a música
  à largura; o **Follow pausa quando você edita ou rola e volta ao parar/reiniciar**
  ([Arrangement View](https://www.ableton.com/en/live-manual/12/arrangement-view)).
- **Reaper — Region/Marker Manager**: lista editável de regiões, export CSV, e a **Region Render
  Matrix** (quais faixas renderizar em quais regiões, nome do arquivo `$track_$region`)
  ([tutsplus](https://music.tutsplus.com/how-to-use-reaper-markers-and-regions--cms-107731t),
  [FMOD: integração Reaper](https://www.fmod.com/docs/studio/appendix-b-reaper-integration.html)).
  **Region Playlists** (SWS): regiões em qualquer ordem, cada uma com *loop count*
  ([SoS: power arranging](https://www.soundonsound.com/techniques/power-arranging-reaper)).

### 1.4 Ensaio

- **Soundslice**: loops salvos, aumento de velocidade a cada volta, mixer para mutar/abaixar cada
  instrumento, **practice lists** com "última vez praticado"
  ([guided tour](https://www.soundslice.com/help/en/player/tips/253/guided-tour-of-learning-tools),
  [practice lists](https://www.soundslice.com/blog/214/introducing-practice-lists)); **gravar a
  si mesmo** no notebook e transformar a gravação em faixa sincronizada; alternar entre gravações
  ([performances](https://www.soundslice.com/help/en/player/notebook/334/performances/),
  [private performances](https://www.soundslice.com/blog/293/introducing-private-performances-and-direct-audio-recording/)).
- **Anytune — Step-It-Up Trainer**: tempo inicial, **tempo final**, nº de degraus, e o que fazer ao
  terminar (parar, sair do loop, recomeçar); mostra o total de voltas
  ([Anytune help](https://anytune.zendesk.com/hc/en-us/articles/360025666651)).
- **Apps de drumline**: o Drumr toca a parte e depois "você toma o lugar" com a faixa sem você; o
  Ensemble Composer tem *section muting* e loop no modo prática; o MuseScore Drumline permite isolar
  cada linha ([Drumr](https://apps.apple.com/app/id6739002178),
  [Ensemble Composer](https://apps.apple.com/app/id724167960),
  [Halftime: apps](https://halftimemag.com/features/best-apps-in-the-marching-arts.html),
  [MuseScore marching](https://musescore.org/en/marching)).
- **MuseScore — partes**: uma parte por instrumento gerada sozinha; export em PDF produz também
  "Score_And_Parts" ([Parts](https://handbook.musescore.org/basics/parts)).

### 1.5 Colaboração, descoberta e acessibilidade

- **Live 12 — browser** com tags e Collections coloridas (até 7) para achar o que é seu
  ([Browser](https://www.ableton.com/en/live-manual/12/working-with-the-browser/),
  [FAQ](https://help.ableton.com/hc/articles/11425042663708)).
- **Reaper**: janela Undo History e opção de salvar o histórico com o projeto; o SWS tem
  *snapshots* nomeados guardados no projeto
  ([changelog](https://www.cockos.com/wiki/index.php/ReaperChangelog_1.x),
  [SWS snapshots](https://sws-extension.org/snapshots.php)).
- **Groove Scribe**: cada groove vira uma URL única que abre e toca no navegador; PDF e MIDI
  ([TomTom](https://tomtommag.com/2017/02/groove-scribe-free-online-digital-drum-transcription-tool-will-change-life/)).
- **Live — Info View e Learn View**: `?` mostra um painel que descreve o que está sob o mouse;
  lições guiadas embutidas ([First Steps](https://www.ableton.com/en/live-manual/12/first-steps/),
  [FAQ navegação](https://help.ableton.com/hc/en-us/articles/12243771208092)).

## 2. Recomendações priorizadas

Ordem = valor para o grupo ÷ esforço. Esforço: **P** ≤ 1 dia, **M** 2–4 dias, **G** > 1 semana ou
muda o formato da música. Caminhos relativos a `crates/`.

| # | Feature | De onde vem | Problema que resolve (taiko) | Valor | Esforço | Onde mexeria |
|---|---|---|---|---|---|---|
| 1 | **"Ensaiar como…"**: escolher uma pessoa muta o barramento dela, destaca as linhas dela na grade/esteira/projeção e liga o count-in | [Soundslice](https://www.soundslice.com/help/en/player/tips/253/guided-tour-of-learning-tools) (mixer), [Drumr](https://apps.apple.com/app/id6739002178) ("take your place"), [Ensemble Composer](https://apps.apple.com/app/id724167960) | "toca tudo menos o meu ōdaiko": hoje exige ir ao Mixer e mutar à mão, e desfazer depois | alto | **P** | `wadawko-core/src/ops/people.rs` (mute temporário fora do histórico), `wadawko-app/src/state.rs` (`rehearse_as: Option<String>`), `views/topbar.rs` (chip), `views/people.rs`, `views/director.rs`, `views/lane.rs`, `keys/registry.rs` + paleta |
| 2 | **Preencher seleção**: sílaba a cada N células com deslocamento, padrão euclidiano (k golpes em n), alternar `don`/`ka`, **oroshi** (golpes acelerando até o rulo) | [FL Advanced Fill](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/stepsequencer_advancedfilltool.htm), [Polyend Fill](https://polyend.com/wp-content/uploads/2023/11/Tracker_Essentials_Rev1.pdf), [Live 12 Euclidean](https://www.soundonsound.com/techniques/ableton-live-12-midi-generators) | escrever 16 compassos de jiuchi ou kizami é repetitivo; o oroshi é escrito à mão célula a célula | alto | **P/M** | `wadawko-core/src/ops/cells.rs` (`fill(sel, pattern)`), `wadawko-core/src/fx.rs` (oroshi = `R` crescente), `wadawko-app/src/shell/actions.rs`, `keys/actions.rs`, `keys/palette.rs` (prompt com prévia) |
| 3 | **Visão por pessoa**: filtro "só as linhas de X" (ou de um instrumento) no editor, na partitura, na esteira e na projeção | [Dorico: layouts e players](https://www.steinberg.help/r/dorico-elements/6.1/en/dorico/topics/notation_reference/notation_reference_unpitched_percussion/notation_reference_unpitched_percussion_kits_versus_individual_instruments_c.html), pedido de filtro no [Bitwig](https://gearspace.com/threads/bitwig-for-mixing.1457067/) | com 8–12 linhas cada pessoa procura a sua; no celular/tablet do ensaio só a sua interessa | alto | **M** | `wadawko-app/src/views/grid/vgeom.rs` (colunas visíveis), `grid/mod.rs`, `views/score.rs` + `wadawko-notation/src/engrave.rs` (`EngraveOptions.tracks`), `views/lane.rs`, `views/director.rs`, `state.rs` (filtro salvo por música) |
| 4 | **Partes individuais**: exportar/imprimir a partitura de cada pessoa (um PDF por pessoa ou um com todas, nome da pessoa no cabeçalho) | [MuseScore Parts](https://handbook.musescore.org/basics/parts), [Dorico](https://www.steinberg.help/r/dorico-elements/6.1/en/dorico/topics/notation_reference/notation_reference_unpitched_percussion/notation_reference_unpitched_percussion_kits_versus_individual_instruments_c.html) | levar a parte impressa para casa / colar no tambor; hoje a impressão é sempre a grade inteira | alto | **M** (depois de #3) | `wadawko-notation/src/{engrave,export,options}.rs`, `wadawko-app/src/shell/io.rs`, `wadawko-cli/src/commands/` (`score --person`) |
| 5 | **Pacote de ensaio**: matriz trechos × pessoas → MP3 "menos eu" e "só eu", com clique opcional e nome `música_trecho_pessoa.mp3` | [Reaper Region Render Matrix](https://music.tutsplus.com/how-to-use-reaper-markers-and-regions--cms-107731t), stems = F11 (#17) | estudar em casa sem o app; o grupo recebe um arquivo por pessoa no WhatsApp | alto | **M** | `wadawko-audio/src/offline.rs` (`RenderOptions` com máscara de pessoas/linhas e região), `wadawko-app/src/shell/io.rs` (diálogo-matriz), `wadawko-cli/src/commands/render.rs` |
| 6 | **Roteiro de ensaio**: encadear trechos com nº de voltas e % de andamento, tocar em sequência; "último ensaio" por trecho | [Soundslice practice lists](https://www.soundslice.com/blog/214/introducing-practice-lists), [Reaper Region Playlists](https://www.soundonsound.com/techniques/power-arranging-reaper) | o ensaio de sábado é sempre "B ×4 a 80 %, B ×4 a 100 %, transição B→C ×3"; hoje é clicar trecho a trecho | alto | **M** | `wadawko-core/src/model.rs` (`PracticeSpot`: `reps`, `tempo_pct`, `last`), `ops/practice.rs`, `views/sections_panel.rs` (TRECHOS), `state.rs` (avança ao fim da volta), `wadawko-audio/src/engine.rs` (evento de fim de volta) |
| 7 | **Biblioteca de frases do usuário**: salvar a seleção como frase nomeada com tags (`jiuchi`, `kizami`, `oroshi`, instrumento), inserir pela paleta ou por um painel com prévia ▶ | [EZdrummer grooves](https://www.soundonsound.com/reviews/toontrack-ezdrummer-3), [Live tags/Collections](https://www.ableton.com/en/live-manual/12/working-with-the-browser/) | o repertório do grupo repete frases entre músicas; hoje só as frases embutidas aparecem | alto | **M** | `wadawko-store/src/{db,library}.rs` (tabela de frases, fora da música), `wadawko-core/src/phrase.rs`, `keys/palette.rs`, painel novo em `views/sections_panel.rs` ou `right_panel.rs` |
| 8 | **Treino com alvo**: BPM final (pode passar de 100 %), voltas por degrau e ação ao chegar (parar, manter, recomeçar); contador `volta 3/8 · 92 BPM` | [Anytune Step-It-Up](https://anytune.zendesk.com/hc/en-us/articles/360025666651) | o treino atual sobe sem limite e uma volta por degrau; o grupo quer "2 voltas em cada andamento até 120" | médio | **P** | `wadawko-audio/src/engine.rs` (`Trainer { target, reps }`), `wadawko-app/src/state.rs` (`trainer_*`), `views/topbar.rs`, `views/director.rs` |
| 9 | **Contagem falada** (`ichi, ni, san, shi` ou `sore!`) e contagem visual grande na projeção | count-in de [Soundslice](https://www.soundslice.com/help/en/player/tips/253/guided-tour-of-learning-tools); costume do taiko; voz de kiai já existe | o grupo entra pela voz do líder, não por bipe; na TV o número grande ajuda quem está longe | médio | **P** | `wadawko-audio/src/{metronome,kyai_voice,engine}.rs` (voz no preroll), `views/director.rs`, `views/topbar.rs` (chip do som) |
| 10 | **Follow que pausa ao editar** e volta ao parar/reiniciar, como no Live | [Live: Follow](https://www.ableton.com/en/live-manual/12/arrangement-view) | corrigir uma célula tocando faz a grade pular de volta ao playhead | médio | **P** | `wadawko-app/src/views/grid/mod.rs` (follow), `state.rs`, `views/status_bar.rs` (indicador "seguir pausado") |
| 11 | **Transformações da seleção**: dobrar/meio tempo, espelhar, rotacionar dentro do compasso, trocar força (`DON`↔`don`), trocar mão/zona (`don`↔`ka`), mover a seleção para outra linha | [Renoise Advanced Edit](https://tutorials.renoise.com/wiki/Advanced_Edit), [Live 12 transformations](https://www.soundonsound.com/techniques/ableton-live-12-midi-transformations?amp) | variar uma frase sem redigitar; passar a frase do chū para o shime | médio | **M** | `wadawko-core/src/ops/cells.rs`, `wadawko-core/src/vocab.rs` (mapa de trocas), `shell/actions.rs`, `keys/*`, menu de contexto em `grid/render.rs` |
| 12 | **Gravar batendo**: tocando, teclas-pad (`F`/`J` = don, `D`/`K` = ka…) gravam golpes quantizados na linha do cursor; "capturar" o que foi batido nas últimas voltas | [MuseScore percussion panel](https://handbook.musescore.org/idiomatic-notation/percussion/inputting-percussion-notation), [Live Capture MIDI](https://help.ableton.com/hc/en-us/articles/360000776450-Capture-MIDI) | o compositor pensa batendo; escrever célula a célula interrompe a ideia. Prepara o MIDI in (F22, #23) | médio | **M** | `wadawko-app/src/views/grid/mod.rs` (modo gravar), `state.rs` (tick do playhead), `wadawko-core/src/ops/cells.rs` (quantizar e gravar), `assets/keymap-default.json` |
| 13 | **Matriz seção × linha** ("quem toca onde"): blocos coloridos pela linha, vazio = descanso; clique vai, duplo clique seleciona o bloco | [Renoise Pattern Matrix](https://tutorials.renoise.com/wiki/Pattern_Matrix) | ver a orquestração da música inteira e quem descansa quando (troca de tambor, kiai, coreografia). Substitui o mapa F15 (#20), que o minimap já cobre em parte | médio | **M** | `wadawko-app/src/views/overview.rs` (novo), `views/mod.rs` (aba `Ctrl+4`), `wadawko-core/src/timeline.rs` |
| 14 | **Seções por referência** (F17, #21): "A A' B A" sem clonar; editar uma ocorrência muda todas; "desvincular" | [Renoise aliases](https://tutorials.renoise.com/wiki/Pattern_Matrix), [FL pattern clips](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist_patterns.htm) | corrigir o refrão em 4 lugares; mas #7 e o clonar atual cobrem boa parte | alto | **G** | `wadawko-core` (modelo, `timeline::play_order`, ops), grade, partitura, export — muda o formato |
| 15 | **Versões nomeadas** ("antes do festival") + histórico visual (F26, #25): lista dos passos com o título do comando, clique volta até ali | [Reaper undo history](https://www.cockos.com/wiki/index.php/ReaperChangelog_1.x), [SWS snapshots](https://sws-extension.org/snapshots.php) | experimentar um arranjo novo sem medo; voltar à versão que o grupo já sabe | médio | **M** | `wadawko-store/src/{history,db}.rs` (tabela de versões), `wadawko-app/src/views/right_panel.rs`, `keys/palette.rs` |
| 16 | **Info View**: `?` liga uma faixa na barra de status que descreve o que está sob o mouse (o que faz, atalho, exemplo); "Aprender" abre a música-exemplo com passos guiados | [Live Info View/Learn View](https://www.ableton.com/en/live-manual/12/first-steps/) | membros novos do grupo abrem o app sem saber que existe loop por trecho ou projeção; tooltips só dão o nome | médio | **P** (info) / **M** (lições) | `wadawko-app/src/views/status_bar.rs`, `ui.rs` (`with_tip` publica a descrição), `keys/registry.rs` (campo `help`), `wadawko-core/src/templates.rs` (música-lição) |
| 17 | **Escalação por seção**: quem está em qual tambor em cada seção (rodízio); a projeção mostra "próxima: Ana → ōdaiko" | [Dorico: players com vários instrumentos](https://steinberg.help/dorico_se/v5/en/dorico/topics/setup_mode/setup_mode_instrument_changes_c.html) | no taiko as pessoas trocam de tambor entre seções; hoje a pessoa é fixa na linha | médio | **G** | `wadawko-core/src/model.rs` (`Section.roster`), `ops/people.rs` (`cell_owner_of` por seção), `engrave.rs`, `views/director.rs`, `views/people.rs` |
| 18 | **Grupos de linhas** recolhíveis com M/S do grupo ("Shimes", "Ōdaikos"), como folder stack | [Logic Track Stacks](https://help.apple.com/logicpro/mac/10.4/en.lproj/lgcp9bc4b63d.html), [Bitwig grupos](https://www.bitwig.com/userguide/latest/intro_to_tracks) | 4 shimes em uníssono ocupam 4 colunas; recolher mostra uma coluna-resumo. Reavalia F28 (era baixo) | médio | **M** | `wadawko-core/src/model.rs` (`Track.group`), `ops/tracks.rs`, `grid/vgeom.rs`, `views/mixer.rs` |
| 19 | **Ajustar à tela**: uma tecla escolhe a altura de linha para caber a seção (ou a música) inteira | [Live `W` / Optimize Width](https://www.ableton.com/en/live-manual/12/arrangement-view) | projetar a seção inteira sem rolar; revisar a música de relance | baixo | **P** | `wadawko-app/src/views/grid/mod.rs` (`Rows::cell_h`), `keys/*` |
| 20 | **Gravação do ensaio** (microfone) alinhada à música, com marcação de onde o grupo adiantou/atrasou e alternância entre tomadas (F23) | [Soundslice performances](https://www.soundslice.com/help/en/player/notebook/334/performances/) | ouvir o ensaio contra a referência; ver se o grupo corre no B | médio | **G** | `wadawko-audio/src/backend.rs` (entrada), `engine.rs`, `wadawko-store` (áudio fora do JSON), `views/lane.rs` (sobreposição) |
| 21 | **Variação sugerida**: a partir de uma seção, gerar uma cópia com X % das células variadas dentro do vocabulário do instrumento (sem tocar no original) | [Logic Drummer](https://help.apple.com/logicpro/mac/10.4/en.lproj/lgcp898c7532.html) (complexidade/fills), [Live 12 Seed/Rhythm](https://www.soundonsound.com/techniques/ableton-live-12-midi-generators), [Polyend Random](https://backstage.polyend.com/t/using-random-sets-all-other-parameters-to-0/3524) | bloqueio criativo para um solo; útil, mas o compositor sabe o que quer na maior parte do tempo | baixo | **M** | `wadawko-core/src/ops/` (novo `variation.rs`, semente fixa), `vocab.rs`, paleta |
| 22 | **Busca por batida** (Tap2Find): bater um ritmo e achar a seção/frase parecida na música ou na biblioteca | [EZdrummer Tap2Find](https://www.soundonsound.com/reviews/toontrack-ezdrummer-3) | "onde está aquela frase que faz *don doko don*?" — só vale com #7 cheia | baixo | **M** | `wadawko-core` (distância entre padrões), `keys/palette.rs`, captura do #12 |

## 3. Design das cinco de maior valor/esforço

### 3.1 "Ensaiar como…" (#1)

Na barra de cima, à direita do count-in, um chip **👤 todos** abre a lista das pessoas (com a cor
de cada uma). Escolher "Ana" põe o app em **modo ensaio da Ana**: o barramento da Ana fica mudo
(estado de transporte, **não** entra no histórico nem no arquivo — é diferente do M do mixer), as
colunas dela ganham a cor dela no cabeçalho e as das outras pessoas esmaecem a ~60 %, a esteira põe
os tambores dela na frente e a projeção mostra "Ana" no canto. Um segundo chip `ouvir: sem mim /
só eu / todos` cobre os três casos do Drumr e do Soundslice. Teclas: `Alt+P` abre a escolha
(digitar filtra como na paleta), `Alt+Shift+P` volta a "todos"; vim `space p` + inicial.
Paleta: "Ensaiar como: Ana". O chip fica âmbar quando ativo, e a barra de status mostra
`sem Ana`, para ninguém esquecer o modo ligado.

### 3.2 Preencher seleção (#2)

Com uma seleção (ou a linha do cursor no compasso), `Ctrl+Shift+I` abre um popover pequeno ancorado
na seleção: campo **padrão** (texto de sílabas: `don . ka .`, repetido até o fim), **a cada** N
células com **início** (±), **euclidiano** `k/n` com rotação, e o atalho **oroshi** (escolhe o
número de golpes e a curva; escreve `don` com `R2…R8` crescente até o rulo). A prévia aparece
**na própria grade**, em tracejado, enquanto os números mudam; `Enter` aplica (um passo de
desfazer), `Esc` cancela, `Space` toca só a seleção com a prévia. Atalhos rápidos sem popover pela
paleta: "Preencher: a cada 2", "a cada 4", "contratempo". Vim: `space e` + `2`/`4`/`e`/`o`. Escreve
só em células vazias por padrão; `Shift+Enter` sobrescreve.

### 3.3 Visão por pessoa (#3) e partes individuais (#4)

No cabeçalho do editor, ao lado das abas, um seletor **visão: todos ▾** com as pessoas e os
instrumentos. Escolher uma pessoa esconde as outras colunas (o modelo não muda; o filtro é salvo
por música no `settings`), mostra uma faixa fina "visão: Ana · 3 de 9 linhas" e vale para grade,
partitura, esteira e projeção. A partitura com filtro vira a **parte**: cabeçalho com o nome e a
cor da pessoa, compassos de descanso longos comprimidos ("8 compassos de pausa", como em toda parte
orquestral). "Exportar partes (PDF)" pede: uma pessoa, todas num PDF (uma por página) ou todas em
arquivos separados; o CLI ganha `wadawko-cli score --person Ana`. Teclas: `Alt+V` abre o seletor,
`Alt+Shift+V` volta a todos. Combina com #1: "Ensaiar como Ana" oferece "mostrar só a parte da Ana".

### 3.4 Pacote de ensaio (#5)

"Exportar pacote de ensaio" abre um diálogo em **matriz**: linhas = trechos de ensaio (e "música
inteira"), colunas = pessoas; cada célula marcada vira um arquivo. Acima da matriz, três opções:
**versão** (`sem a pessoa` / `só a pessoa` / ambas), **clique** (com o metrônomo e acentos atuais,
e count-in) e **andamento** (100 % ou o % do treino, ex.: 80 %). O nome segue o padrão do Reaper,
`música_trecho_pessoa_80.mp3`, numa pasta com o nome da música; a barra de progresso mostra
"7 de 24". O render reusa o mesmo motor do playback; a máscara de pessoas é a mesma de #1.
Teclas: paleta "Exportar pacote de ensaio"; CLI `wadawko-cli render --spot "solo" --without Ana`.

### 3.5 Roteiro de ensaio (#6)

No painel TRECHOS, cada trecho ganha dois campos discretos à direita: `×4` (voltas) e `80 %`.
Arrastar os trechos define a ordem; um botão **▶ roteiro** toca o primeiro trecho o número de
voltas pedido, no andamento pedido (o count-in a cada volta continua valendo), e passa sozinho ao
próximo, com um compasso de contagem entre eles. A projeção mostra `Roteiro 2/5 · B a 80 % · volta
3/4`. Ao terminar um trecho o app grava "ensaiado em 18/11" (aparece em cinza, como no Soundslice).
O mesmo trecho pode aparecer duas vezes (B a 80 %, depois B a 100 %). Teclas: `Ctrl+Shift+Enter`
toca o roteiro, `Alt+N` (com o roteiro tocando) pula para o próximo item; vim `space t o`. O formato
ganha campos opcionais em `practice`, ignorados por versões antigas.

## 4. O que não recomendamos (e por quê)

| Item | Por quê |
|---|---|
| EQ por linha, reverb/envios, compressão, automação desenhável (F13 #19, F24 #24, F19) | o som do taiko ao vivo é o do grupo, não o do app; o app é partitura que toca. As dinâmicas e hairpins já são a automação que importa |
| Síntese, devices, hosting de VST/CLAP, sound design | o Kit (couro/borda) já cobre ajustar o timbre do tambor; o resto é DAW de produção |
| Probabilidade/chance e *trig conditions* (Bitwig Operators, Elektron, Live Chance) | um ensaio é determinístico: todo mundo tem de ouvir a mesma coisa toda vez (decisão já tomada em [tracker.md §1.4](/planejamento/tracker.md)) |
| Clip launcher / Session View / Live Loops | improviso por cenas não é como um grupo de taiko toca; a ordem das seções é fixa e ensaiada |
| Polimetria por linha (step rate e comprimento por linha, do Logic Step Sequencer) | a fórmula por compasso e a subdivisão bastam; linhas com comprimentos diferentes confundem a leitura do grupo |
| Bandmate / geração por IA a partir de áudio | biblioteca pequena e estilo muito específico; resultado pouco confiável até nos reviews. A variação simples (#21) basta |
| Piano roll, transposição, escalas | tambores sem altura; a zona (couro/borda) já é o "pitch" |
| Comping, punch in/out, time-stretch, freeze (F29) | não há áudio linear a editar (a gravação #20 é para comparar, não para editar) |
| Swing por linha (F25), humanização por linha além do `H` | o swing global e o `H0–H9` por célula cobrem o que aparece em taiko |
| Multi-cursor estilo editor de texto | a seleção em bloco + preencher (#2) + transformações (#11) fazem o mesmo com menos modos para explicar |
| Painéis destacáveis / multi-janela (F27) | a projeção (`F10`) já resolve a segunda tela; o custo em GPUI é alto |

## 5. Lacunas da pesquisa

- A Region Playlist do Reaper é do SWS (extensão), não nativa; o comportamento atual do Polyend
  Random/Random+ só aparece em fórum.
- Não achei documentação de busca/filtro de faixas no Bitwig (só o pedido de usuários) nem do Quick
  Help do Logic.
- Nenhum app pesquisado trata **rodízio de pessoas entre instrumentos por seção** (#17) como
  conceito de ensaio; o mais perto é o *player* do Dorico, que não serve para percussão sobreposta.
- Não há app de ensaio específico de taiko em grupo; os de drumline (Drumr, Ensemble Composer) são
  o análogo mais próximo, e todos confirmam o mesmo trio: **mutar a minha parte, loop, andamento**.
