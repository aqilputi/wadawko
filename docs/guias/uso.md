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
- **Barra de cima**: título da música (clique para renomear), BPM, reto/swing, loop,
  metrônomo, count-in, treino e tocar/parar.
- **Painel da direita**: as músicas do banco (`+ nova`, trocar, renomear, duplicar, excluir) e
  os botões de exportar/importar.
- **Barra de status**: modo (SIMPLES ou modos do vim), posição do cursor, BPM, estado do
  playback e avisos.

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
  instrumento, pessoas), **M**udo, **S**olo, ★ (linha principal da partitura) e ×.
- **Dinâmica**: a faixa fina acima das células — clique para escolher pp…ff; `<` e `>` marcam
  crescendo/diminuendo. Há dinâmica geral do compasso e, por linha, a sub-linha de dinâmica.
- **Kiai**: sub-linha de gritos (`Ctrl+Shift+K` na linha do cursor) — o texto é cantado pelo
  sintetizador de voz na hora certa.
- **Régua** (números 1 e 2 e…): clique para escolher de onde tocar; arraste para rolar.
- **Zoom**: `Ctrl+roda` ou os botões 🔍−/🔍+.

# Pessoas, Mixer e Kit

- **Pessoas**: cada pessoa tem nome, cor, **caneta** e a lista dos instrumentos que toca; uma
  linha marcada por 2+ pessoas fica **compartilhada** — escreva com a caneta de cada uma (ou
  selecione notas e reatribua) e cada nota soa e se colore pela sua dona.
- **Mixer**: volume/pan/mudo/solo por linha e um barramento por pessoa. Sliders: arraste,
  duplo clique volta ao padrão, setas ajustam o slider em foco.
- **Kit**: instrumentos próprios a partir de um base (shime, chū, ōdaiko, okedō, nagadō…), com
  couro e borda ajustados separadamente (pitch, ganho, decaimento, overtones, filtro) e ▶ para
  ouvir. Os prontos podem ser duplicados para editar.

# Tocar e ensaiar

- `Space` toca/para (no modo simples, quando não está digitando). A grade acompanha o
  playhead; as células acendem quando a nota soa.
- **Loop**, **metrônomo**, **count-in** e **reto/swing** na barra de cima.
- **Treino** (🏃): começa numa % do BPM e acelera a cada volta (% inicial e passo editáveis).
- **Região**: com seções selecionadas, só elas tocam (e o loop fica nelas).
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
