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
  loop), treino e o tema (em janela estreita, abaixo de 1200 px, a barra fica compacta: reto/swing
  vira um botão, o metrônomo mostra só o padrão e o tema vai para a paleta). Parar o mouse sobre um botão mostra o nome do comando e o atalho.
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
- **Kiai**: sub-linha de gritos (`Ctrl+Shift+K` na linha do cursor), uma sílaba por célula. A voz
  padrão é o **animalese com timbre de grito** (mais agudo, áspero, com sopro e rouquidão no
  "!"), que canta qualquer palavra. Também há **gravações reais** masculinas e femininas: a
  palavra inteira quando ela foi gravada (`ha!`, `hai!`, `iya!`, `yo!`, `sa!`, `hyo!`, `orya!`…),
  senão sílaba a sílaba pela vogal (`so re!` → "yo"/"ho" + "ei"/"hai"). Escolha
  **animalese/masc/fem** no menu da linha ("voz") ou no cartão da pessoa (página Pessoas; "da
  linha" segue a linha).
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

# Modelos e export MIDI

- **Modelos de música**: `Ctrl+K` → "Nova música: …" — Em branco, Jiuchi + 3 linhas, Ensemble 5
  (com seções e repetição), Matsuri (swing) e o exemplo Renshū.
- **MIDI** (`Ctrl+K` → "Exportar MIDI"; CLI: `wadawko-cli midi <música>`): uma faixa por linha no
  canal 10 (bateria GM), com andamento, fórmulas de compasso e marcadores de seção. Notas: ōdaiko
  35, ō-nagadō 41, nagadō 43, chū 45, okedō 47, hira 50, shime 38, borda 37, kane 56/80.

# Minimap e rolagem

- À direita do editor vertical fica o **minimap** (como o do VS Code): a música inteira em
  miniatura, com a faixa de cor de cada seção, as notas em tom suave da cor de cada linha (em
  passagens densas, a densidade por tempo), o retângulo da área
  visível e o playhead. Clique ou arraste nele para ir até ali. `Ctrl+K` → "Minimap" esconde.
- Se você **rolar ou mover o cursor tocando**, o seguir pausa (para não puxar a tela de volta
  enquanto edita); volta sozinho ao parar, ou com `Scroll Lock`.
- Tocando, a grade **segue o playhead** virando a página; `space v shift-f` (ou a paleta) troca para
  **centrado** (a linha que soa fica fixa e a música rola por baixo — mais suave, mais pesado), e
  `Scroll Lock` / `space v f` liga/desliga o seguir.
- A grade **acompanha o cursor e a seleção**: estender com `Shift+setas`, movimentos do vim,
  clicar numa seção no painel — se o cursor sair da vista, a grade rola até ele. Arrastando para
  selecionar, encostar o mouse na borda de cima ou de baixo rola sozinho e continua selecionando.

# Teclado no editor vertical

`↑`/`↓` andam no tempo e `←`/`→` entre as linhas (no vim, `j`/`k` e `h`/`l`). `Tab`/`Shift+Tab`
confirmam o que está sendo digitado e vão para a linha seguinte/anterior (como no Renoise).
`PgUp`/`PgDn` vão para o compasso anterior/seguinte, `Ctrl+↑/↓` andam por tempo, `Alt+PgUp/PgDn`
por seção e `Ctrl+PgUp/PgDn` vão para a primeira/última linha. **Passo de edição** (`Alt+0`…`Alt+9`):
quantas células o cursor desce ao confirmar uma sílaba — com passo 2 numa música em colcheias você
escreve nos tempos; passo 0 fica na célula.

# Painel de seções (à esquerda)

- A lista de **seções** da música (`Ctrl+Shift+B` mostra/esconde): quadradinho de cor, nome,
  ritornello (`2×`) e compassos; ▶ marca a que está tocando; ⟲, a que tem trecho de ensaio.
- **Clique** seleciona a seção (é a região que toca) e leva o editor até ela; `Ctrl+clique`
  junta/tira; `Shift+clique` pega o intervalo; **duplo clique** renomeia.
- **Arraste** para reordenar (leva os compassos junto); com `Alt` arrastado, clona.
- **Botão direito**: renomear, cor, ritornello ±, duplicar, copiar, colar depois, agrupar,
  desfazer o grupo, tocar só esta, remover.
- **Copiar/colar seções** (`⧉`/`⎘` no cabeçalho; vim `space s y` / `space s shift-p`): vai como texto
  pela área de transferência, então dá para colar noutra música. Leva notas, donos, kiai,
  dinâmicas e efeitos; as linhas casam pelo nome interno ou pela posição.
- **Grupos** (`⊞` no cabeçalho ou "agrupar"; vim `space s g`): juntam seções seguidas sob um
  nome (duplo clique renomeia), com cor própria herdada pelas seções sem cor, e recolhem com ▾.
  Clicar no grupo seleciona todas as seções dele. Grupos só organizam: não mudam a ordem tocada.
- **Cores**: clique no quadradinho da seção ou do grupo (8 cores, "sem cor" ou `#rrggbb`). A cor
  aparece na linha do gutter do editor vertical. A cor dos **instrumentos** fica na página **Kit**
  (cada linha herda a do instrumento, a menos que tenha cor própria no ⚙ da linha).
- Os **trechos de ensaio** ficam embaixo, no mesmo painel.
- A aba **LINHAS** (ao lado de SEÇÕES) lista os instrumentos como os cabeçalhos de faixa de uma
  DAW: cor, nome, instrumento e M/S/★. Clique numa linha para levar o cursor até ela; a linha do
  cursor abre com a cor (12 cores ou "herdar"), volume, pan, o que mostrar na grade (dinâmica,
  kiai, efeitos), pessoas e ↑/↓, nome, som e remover.

# Preencher a seleção

Selecione células (ou deixe só o cursor: vale o compasso dele na linha) e use `Ctrl+Shift+I`
(paleta filtrada em "Preencher") ou, no vim, `space e …`: **a cada 2** (`2`), **a cada 4** (`4`),
**contratempo** (`c`), **euclidiano 3 em 8** (`3`) e **5 em 8** (`5`), **oroshi** (`o`, golpes cada
vez mais próximos, terminando em acento). Escreve a sílaba da célula do cursor (vazia: `don`; no
contratempo, `ka`), só nas células vazias, num passo de desfazer.

# Efeitos por célula

Como os comandos de um tracker, curtos e legíveis, até 3 por célula (ex.: `R4< D+1`):

| Comando | Faz |
|---|---|
| `V0`–`V9` | força × n/10 (`V0` = ghost) |
| `R2`–`R9`, `R4<`, `R4>` | rulo: repete o golpe n vezes na célula, mãos alternando; `<`/`>` crescendo/decrescendo |
| `F` (`F1`–`F3`) | flam: um golpe de apoio antes, na outra mão |
| `D+1`…`D+5`, `D-1`…`D-5` | atrasa/antecipa em décimos da célula |
| `X` (`X1`–`X9`) | corte: abafa a pele no meio da célula (nota curta) |
| `H0`–`H9` | humanização (`H0` = exato, para uníssonos) |

No editor vertical a coluna **fx** aparece ao lado da nota nas linhas que têm efeito (ou em todas:
`Ctrl+K` → "Efeitos: mostrar a coluna", vim `space v x`); clique nela para digitar. Atalhos na
célula ou na seleção: `Ctrl+Shift+R` rulo (2 → 3 → 4 → 6 → 8 → nenhum), `Ctrl+Shift+F` flam,
`Ctrl+Shift+.`/`,` atrasa/antecipa, `Ctrl+Shift+X` corte, `Ctrl+Shift+V` mais fraco, `Ctrl+Shift+0`
limpa, `Ctrl+Shift+Y` digita; no vim, `space f …`. O inspector mostra os efeitos da célula com
botões prontos. O áudio, a esteira e o MIDI tocam os efeitos; a partitura ainda não os desenha.

# Tocar e ensaiar

- `Space` toca/para (no modo simples, quando não está digitando). A grade acompanha o
  playhead; as células acendem quando a nota soa.
- **Loop**, **metrônomo**, **count-in** e **reto/swing** na barra de cima.
- **Treino**: começa numa % do BPM e acelera a cada volta. Com o treino ligado, os chips da barra
  ajustam: % inicial, passo por degrau, **voltas por degrau** (×1 → ×4: repete cada andamento N
  vezes antes de subir) e **até %** (o andamento onde para de subir — ex.: treinar até 90 %).
- **Metrônomo com acentos**: com o 🎵 ligado aparecem dois chips. O do **padrão** (`auto`,
  `ABBB`…) abre um campo: `A` = forte, `B` = fraco, `.` = mudo, um caractere por tempo (até 16,
  repete se o compasso for maior; vazio = automático, forte no 1). Ex.: `A.B.` para marcar só
  o 1 e o 3; `.B.B` para ouvir só o 2 e o 4. O chip **÷** troca a subdivisão: só os tempos →
  colcheias → tercinas → semicolcheias; o terceiro chip troca o **som** (bipe → shime, de
  madeira → kane, de metal). Volume pela paleta ("Metrônomo: volume…"). Tudo vale
  na hora, tocando, e fica salvo nas configurações.
- **Trechos de ensaio** (painel TRECHOS, à direita): com um trecho de loop marcado, `+ guardar`
  (ou `Ctrl+Shift+L`; vim: `Space t g`) pede um nome — "solo do Ōdaiko", "entrada do B". Um clique
  na lista põe aquele trecho em loop; também aparecem na paleta (`Ctrl+K` → "Trecho: …"). ✎
  renomeia, ✕ remove (Ctrl+Z volta). Ficam salvos na música e acompanham as seções.
- **Count-in a cada volta** (⏱↺, aparece com o loop ligado): antes de cada repetição toca um
  compasso de contagem, para o grupo respirar entre as voltas. Combina com o treino.
- **Contagem falada** (chip "sem voz / voz m / voz f" ao lado do count-in; vim `space t i`): o
  count-in fala *ichi, ni, san, shi/yon…* junto com os cliques.
- **Altura das linhas**: `Ctrl+Alt+↑/↓` (vim: `z k`/`z j`, `z =` volta ao padrão) — linhas
  baixas cabem mais na tela (abaixo de 32 px some o nome do instrumento). Fica salva.
- **Ensaiar como…** (`Alt+P`, chip 👤 na barra de cima; vim `space p`): escolha uma pessoa e o som
  toca **sem ela** (ela toca junto) ou **só ela**; as linhas das outras pessoas ficam esmaecidas e a
  barra de status mostra `👤 sem Ana`. Não muda a música nem entra no desfazer. `Alt+Shift+P` volta
  a todos; "Ensaiar como: ouvir sem / só" alterna.
- **Visão por pessoa** (`Alt+V`; vim `space v v`): o editor mostra só as linhas de uma pessoa
  (as outras somem, o cursor pula para a linha visível mais próxima); a barra de status mostra
  `👁 Ana`. `Alt+Shift+V` (ou "Visão: todas as linhas") volta a mostrar tudo. Não muda a música.
- **Modo projeção** (`F10`, ou `Ctrl+K` → "Modo projeção"): tela cheia em letras grandes para a TV
  do ensaio (durante o count-in, a contagem gigante 4… 3… 2… 1) — seção atual e a volta (`2/3`), compasso, os tempos acendendo, a próxima seção e o
  BPM que está soando; a lista de linhas com as cores embaixo. `Espaço` toca/para, `Esc` sai.
- **Tap tempo**: bata no **TAP** (ou `Alt+T`; no vim `Space t .`) no ritmo — da 2ª batida em
  diante o BPM segue a média das últimas 8; parar 2 s recomeça. Um `Ctrl+Z` desfaz o tap inteiro.
- **Loop por trecho** ("esses dois compassos de novo"): **arraste na faixa de cima da régua**
  do primeiro ao último compasso — o trecho fica azul, o loop liga e a música só toca ali
  (pode atravessar seções). Puxe as alças das pontas para ajustar; clique dentro do trecho
  desmarca (`Alt+clique` também); clique fora marca só aquele compasso. `Ctrl+L`
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
