---
type: User Guide
title: "Guia de uso"
description: "Como escrever, organizar, tocar e exportar uma música no WaDAWko: grade, seções, notas, pessoas, mixer, kit e exportação."
tags: [guia, uso, grade, playback, exportação]
status: stable
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-16T13:10:00Z
---

# Guia de uso

Todas as teclas citadas aqui estão na [referência de atalhos](/referencia/atalhos.md); qualquer
comando também roda pela **paleta** (`Ctrl+K`): digite parte do nome e Enter.

# A janela

- **Barra lateral** (`Ctrl+B` mostra/esconde): Editor, Pessoas, Mixer, Kit, Config.
- **Barra de cima**: à esquerda o título da música (clique para renomear); no centro o
  **transporte** — ⏮ ir ao início, ▶/■, ↺ loop, o mostrador de posição (`c12 · 3.2`: compasso ·
  tempo.subdivisão do cursor; tocando, do playhead) e o BPM (roda do mouse ±1, clique para
  digitar) com o **TAP** (bata no ritmo para marcar o BPM), reto/swing; à direita metrônomo
  (ligado, mostra o padrão de acentos e a subdivisão), count-in, count-in a cada volta (com
  loop), treino e o tema. Parar o mouse sobre um botão mostra o nome do comando e o atalho.
- **Painel da direita**: o **inspector** (propriedades da seção, do compasso, da linha e da
  célula do cursor — edite ali mesmo: ritornello, compassos, fórmula, dinâmica, andamento,
  M/S/★, volume, pan, pessoas, sfz/acento) e as músicas do banco (`+ nova`, trocar, renomear,
  duplicar, excluir) com exportar/importar. Num slider do inspector, clique (ou salte com
  `Ctrl+Space`) e use as setas para ajustar de passo em passo; `Esc` ou um clique na grade
  devolve as setas ao cursor.
- **Barra de status**: modo (SIMPLES ou modos do vim), posição do cursor, BPM, estado do
  playback, trecho de loop (`↺ c2–c3`) e avisos.

No Editor há três abas — **grade**, **partitura** e **esteira** (`Ctrl+1/2/3`). A barra das
abas e a barra de ferramentas ficam fixas no topo enquanto a grade rola.

# Escrever na grade

Cada **linha** é um instrumento; as colunas são as células do compasso (a subdivisão —
colcheia, semicolcheia ou fusa — vale para a música toda e fica na barra de ferramentas).

- **Digitar**: clique numa célula e escreva a sílaba (`don`, `doko`, `ka`…). `Space`, `Enter`
  ou `Tab` gravam e vão para a próxima célula; `Shift+Tab` grava e volta. Sílabas válidas e
  frases prontas: [vocabulário](/referencia/vocabulario.md).
  - Palavras compostas se quebram sozinhas (`dokodoko` = doko+doko); alongar (`dooon`) dura mais;
    CAIXA ALTA acentua; `!` no fim = sforzando.
- **Apagar**: botão direito na célula, `Backspace` (anterior) ou `Delete` (seleção).
- **Frases e kiai prontos**: paleta → "Inserir frase: cavalinho", "Inserir kiai: sei!" —
  escrevem a partir do cursor.
- **Fórmula de compasso**: na barra de ferramentas, `compasso − 4/4 +` muda o compasso do cursor
  (ou todos os compassos selecionados) — ex.: 4/4 → 5/4. Aumentar acrescenta tempos vazios no fim;
  diminuir corta os últimos tempos (`Ctrl+Z` desfaz). Cada compasso toca com a sua duração
  (metrônomo, esteira e partitura acompanham). Também pela paleta: "Compasso: +1 tempo".
- **Mudança de andamento** (rallentando/accelerando): clique no `♩` da faixa de dinâmica do
  compasso onde a mudança começa. No popover, **alvo** ajusta o BPM a atingir (±1/±5) e
  **chegar em** diz em quantos compassos (a rampa é linear; "na hora" troca de uma vez — "a
  tempo"). A faixa mostra `rall. → 90 ·2c`; a partitura, `rall. → 90`. Pela paleta: "Andamento:
  rallentando" / "accelerando" (±10 BPM; com seleção, a rampa dura os compassos selecionados),
  "a tempo" (volta ao BPM da música) e "remover". O campo BPM da barra de cima continua sendo o
  da música; tocando, o `♪ 110` ao lado mostra o que está soando. O treino escala o mapa inteiro.
- **Modos** (barra de ferramentas, ou `Ctrl+Shift+S`): **editar** (padrão: digitar) e
  **selecionar**. Com o **vim** ligado (Config), a grade usa normal/inserção/visual.

## Selecionar, copiar e mover notas

- **Arrastar a partir de uma célula vazia** seleciona um bloco (várias linhas e compassos);
  `Shift+clique` estende.
- **Copiar/recortar/colar**: `Ctrl+C`, `Ctrl+X`, `Ctrl+V` (no modo simples). A seleção continua
  depois de copiar. O que é copiado vai também para a área de transferência do sistema (texto
  separado por tabulações).
- **Pegar e arrastar**: aperte **sobre uma nota** (ou sobre a seleção) e arraste — o bloco
  "descola" e segue o mouse, balançando, e cai onde o tracejado mostra. **Com `Alt`, copia**
  em vez de mover. `Esc` no meio cancela; soltar fora da grade larga no último destino.
- Com seleção, `Alt+setas` movem o bloco e `Alt+Shift+setas` o clonam.

## Seções

Cada seção tem uma barra com: ⠿ (arrastar para reordenar), nome (✎ renomeia), ritornello
(−/+ e `2×`), ▾ (recolher), compassos (−/+), ⧉ clonar e ✕ remover. Em janelas estreitas a
barra encolhe (some primeiro o texto, depois os botões de compasso — que seguem na paleta).

- **Clique na barra** seleciona **só** aquela seção para tocar (região do playback).
  **`Ctrl+clique`** junta ou tira seções; clicar na única selecionada limpa.
- `+ seção` (barra de ferramentas) cria uma seção nova no fim.

## Linhas, dinâmicas e kiai

- `+ linha` cria uma linha com o instrumento escolhido ao lado; em cada linha: ⚙ (nome,
  instrumento, **cor**, pessoas), **M**udo, **S**olo, ★ (linha principal da partitura) e ×.
- **Cor da linha**: a listra à esquerda do nome (e o cabeçalho do strip no mixer, o anel do
  tambor na esteira). Cada instrumento tem uma cor padrão; no ⚙ escolha uma das 12 ou volte ao
  padrão — ou "Linha: próxima cor" na paleta.
- **Dinâmica**: a faixa fina acima das células — clique para escolher pp…ff; `<` e `>` marcam
  crescendo/diminuendo. Há dinâmica geral do compasso e, por linha, a sub-linha de dinâmica.
- **Kiai**: sub-linha de gritos (`Ctrl+Shift+K` na linha do cursor) — o texto é cantado pelo
  sintetizador de voz na hora certa.
- **Régua** (números 1 e 2 e…): clique para escolher de onde tocar; arraste para rolar. A
  **faixa fina em cima** da régua é a do loop por trecho (abaixo).
- **Zoom**: `Ctrl+roda` ou os botões 🔍−/🔍+.

# Pessoas, Mixer e Kit

- **Pessoas**: cada pessoa tem nome, cor, **caneta** e a lista dos instrumentos que toca; uma
  linha marcada por 2+ pessoas fica **compartilhada** — escreva com a caneta de cada uma (ou
  selecione notas e reatribua) e cada nota soa e se colore pela sua dona.
- **Mixer**: um strip por linha (cabeçalho na cor da linha, fader com escala de dB,
  **medidor** ao lado — verde até −18 dB, âmbar até −6, vermelho até 0, com a linha do pico
  retido —, pan, M/S), um barramento por pessoa (com medidor) e o strip **MASTER** à direita
  (volume de saída, medidor L/R e LED de clip, que acende quando o som encosta no teto).
  Faders: arraste, duplo clique volta ao padrão, setas ajustam o slider em foco. Na grade, um
  mini-medidor aparece sob o nome de cada linha enquanto toca.
- **Kit**: instrumentos próprios a partir de um base (shime, chū, ōdaiko, okedō, nagadō…), com
  couro e borda ajustados separadamente (pitch, ganho, decaimento, overtones, filtro) e ▶ para
  ouvir. Os prontos podem ser duplicados para editar.

# Tocar e ensaiar

- `Space` toca/para (no modo simples, quando não está digitando). A grade acompanha o
  playhead; as células acendem quando a nota soa.
- **Loop**, **metrônomo**, **count-in** e **reto/swing** na barra de cima.
- **Treino** (🏃): começa numa % do BPM e acelera a cada volta (% inicial e passo editáveis).
- **Metrônomo com acentos**: com o 🎵 ligado aparecem dois chips. O do **padrão** (`auto`,
  `ABBB`…) abre um campo: `A` = forte, `B` = fraco, `.` = mudo, um caractere por tempo (até 16,
  repete se o compasso for maior; vazio = automático, forte no 1). Ex.: `A.B.` para marcar só
  o 1 e o 3; `.B.B` para ouvir só o 2 e o 4. O chip **÷** troca a subdivisão: só os tempos →
  colcheias → tercinas → semicolcheias. Volume pela paleta ("Metrônomo: volume…"). Tudo vale
  na hora, tocando, e fica salvo nas configurações.
- **Count-in a cada volta** (⏱↺, aparece com o loop ligado): antes de cada repetição toca um
  compasso de contagem, para o grupo respirar entre as voltas. Combina com o treino.
- **Tap tempo**: bata no **TAP** (ou `Alt+T`; no vim `Space t .`) no ritmo — da 2ª batida em
  diante o BPM segue a média das últimas 8; parar 2 s recomeça. Um `Ctrl+Z` desfaz o tap inteiro.
- **Loop por trecho** ("esses dois compassos de novo"): **arraste na faixa de cima da régua**
  do primeiro ao último compasso — o trecho fica azul, o loop liga e a música só toca ali
  (pode atravessar seções). Puxe as alças das pontas para ajustar; clique dentro do trecho
  liga/desliga o loop; `Alt+clique` limpa; clique fora marca só aquele compasso. `Ctrl+L`
  faz o trecho da seleção (ou do compasso do cursor); no vim, `[` e `]` põem o início e o fim
  no compasso do cursor. O trecho aparece na barra de ferramentas (`↺ c2–c3 ✕` — clique
  limpa) e na barra de status. Tocando, mudar o trecho não reinicia: o playhead continua se
  ainda estiver dentro, senão vai ao início dele.
- **Região**: com seções selecionadas, só elas tocam (e o loop fica nelas); o trecho de loop,
  se houver, tem precedência.
- **Esteira**: a música como discos vindo em direção a cada tambor, com pétalas a cada golpe.
- Se a luz das células vier antes/depois do som, ajuste **Config → Sincronia** (ou digite
  `:offset 40` com o vim).

# Exportar e importar

- **Música**: `Ctrl+E` exporta `.taiko.json`; `Ctrl+O` importa (vira música nova). Formato:
  [formato da música](/referencia/formato-da-musica.md).
- **Áudio**: `Ctrl+Shift+E` (MP3, precisa do `ffmpeg`) ou "wav" no painel — render com o mesmo
  motor do playback.
- **Partitura**: paleta → "Exportar partitura (SVG/PNG)" ou "Imprimir partitura / PDF".
- Sem portal de arquivos do desktop, os exports vão direto para `~/Downloads`.

Tudo é salvo sozinho (autosave); `Ctrl+Z`/`Ctrl+Shift+Z` desfazem/refazem até 100 passos.

# Navegar só com o teclado

- `Ctrl+K` — paleta de comandos e músicas.
- `Ctrl+Space` — rótulos sobre botões, linhas e células: digite o rótulo para "clicar".
- `Ctrl+/` — mostra o que dá para apertar aqui; prefixos como `Space` mostram as continuações.
- Teclas próprias: `~/.config/wadawko/keymap.json` ([dados e arquivos](/referencia/dados-e-arquivos.md)).
