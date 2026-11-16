# Changelog

Formato: [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) · versões: [SemVer](https://semver.org/lang/pt-BR/).

## [Não lançado]

### Adicionado
- **Fórmula de compasso por compasso** (ex.: 4/4 → 5/4): `compasso − 4/4 +` na barra de
  ferramentas e "Compasso: ±1 tempo" na paleta, no compasso do cursor ou nos selecionados.
- **Mudança de andamento por compasso** (rallentando, accelerando, "a tempo"): chip `♩` na
  faixa de dinâmica de cada compasso abre o ajuste do BPM alvo (±1/±5) e de em quantos
  compassos chegar lá (ou na hora). Pela paleta: "Andamento: rallentando/accelerando" (±10 BPM,
  ao longo dos compassos selecionados), "a tempo" e "remover". O áudio, o metrônomo, o count-in,
  o playhead e a exportação seguem o mapa de andamento; a partitura mostra `rall. → 90`. O
  treino continua valendo: escala o mapa inteiro. Campo novo `tempo` no compasso.

### Mudado
- Cada compasso passa a durar os seus `beats` tempos no áudio, metrônomo, kiai, esteira e
  playhead. Antes todo compasso ocupava 4 tempos (herança do app web): um 5/4 sobrepunha o
  compasso seguinte e um 3/4 deixava um tempo de silêncio. Músicas só em 4/4 não mudam.

## [0.1.0] - 2026-11-16

Primeira versão do **app nativo** (Rust + GPUI), em uso diário desde 2026-11-13. Substitui o
app web (Svelte), que fica no branch `web-legacy`.

### Adicionado
- **Grade** de kuchi shōga por linha de instrumento: seções com ritornello, compassos,
  subdivisão (colcheia/semicolcheia/fusa), dinâmicas por compasso e por linha, crescendo/
  diminuendo, kiai, sforzando; frases e kiai prontos pela paleta.
- **Notas na mão**: arrastar a partir de célula vazia seleciona; pegar uma nota (ou a seleção)
  e arrastar move, com `Alt` copia — o bloco segue o mouse balançando; `Esc` cancela.
- **Seções**: clique na barra seleciona só ela para tocar; `Ctrl+clique` junta; a barra se
  adapta a janelas estreitas.
- **Playback** com samples reais (camadas, round robin, EQ por instrumento), loop, metrônomo,
  count-in, swing, treino com aceleração por volta e região; esteira com pétalas; partitura
  própria (fonte Bravura) com export SVG/PNG/impressão.
- **Pessoas** (cor, caneta, linhas compartilhadas), **Mixer** (linha e barramento por pessoa)
  e **Kit** (instrumentos próprios com couro e borda ajustáveis).
- **Teclado**: paleta `Ctrl+K`, saltos por rótulo `Ctrl+Space`, which-key, vim opcional,
  keymap do usuário em JSON; `Tab` grava e anda de célula.
- **Exportar/importar** música em JSON, áudio em WAV/MP3; `wadawko-cli` (list, export, render,
  import, import-backup); migração do banco do app web (`--migrate-from`).
- Documentação em `docs/` (bundle OKF v0.2), CI e pacote de release para Linux.

### Desempenho
- Moldura da janela em cache, follow do playhead sem pulo duplo, pétalas pré-tesseladas e
  arrasto sem frames parado: tocando, 7 → 4,7 ms de CPU por frame a 60 fps.

[0.1.0]: https://github.com/aqilputi/wadawko/releases/tag/v0.1.0
