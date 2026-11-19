# Changelog

Formato: [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) · versões: [SemVer](https://semver.org/lang/pt-BR/).

## [Não lançado]

### Adicionado
- Aba **LINHAS** no painel da esquerda (cabeçalhos de faixa com o que o inspector mostra).
- **Minimap** à direita do editor vertical (clique/arraste para rolar) e a grade **acompanha a seleção**
  (teclado e arrasto com rolagem automática nas bordas).
- **Efeitos por célula** estilo tracker: rulo, flam, atraso, corte, força e humanização (`R4< D+1`),
  com coluna própria no editor vertical, atalhos e inspector; tocam no áudio, na esteira e no MIDI.
- **Painel de seções** à esquerda: seleção, renomear, arrastar para reordenar (Alt clona), menu de
  contexto, copiar/colar seções entre músicas, **grupos** recolhíveis e **cores** de seção e grupo;
  os trechos de ensaio passaram para ele. **Cor por instrumento** na página Kit.
- **Modo projeção** (`F10`): seção, volta, compasso, tempos, próxima seção e BPM em letras grandes.
- **Metrônomo com som de shime ou kane** (além do bipe): chip na barra de cima.
- **Export MIDI** (`.mid`, uma faixa por linha, bateria GM; também `wadawko-cli midi`).
- **Modelos de música** na paleta: Em branco, Jiuchi + 3 linhas, Ensemble 5, Matsuri (swing).
- **Altura das linhas ajustável** (`Ctrl+Alt+↑/↓`, vim `z k`/`z j`/`z =`), salva nas configurações.
- **Trechos de ensaio com nome** (painel TRECHOS): guarde o trecho de loop com um nome e volte a
  ele com um clique ou pela paleta. Campo opcional `practice` na música.
- **Metrônomo com acentos**: padrão por tempo (`A` forte, `B` fraco, `.` mudo — ex.: `A.B.`),
  subdivisão (colcheias, tercinas, semicolcheias) e volume; chips na barra de cima quando o
  metrônomo está ligado, salvo nas configurações. O count-in segue os acentos e a fórmula do
  compasso.
- **Count-in a cada volta do loop** (⏱↺): um compasso de contagem antes de cada repetição.
- **Tap tempo**: botão TAP no mostrador, `Alt+T` (vim: `Space t .`).
- **Tooltips** nos botões com o nome do comando e o atalho atual.
- Sliders do **inspector** respondem às setas (clique ou `Ctrl+Space` no slider; `Esc` ou
  clique na grade solta).
- **Cor por trilha**: cada linha tem uma cor (padrão pelo instrumento base; 12 cores na paleta
  do ⚙ da linha, ou "Linha: próxima cor" na paleta de comandos) — listra na lista de linhas,
  cabeçalho do strip no mixer, anel do tambor na esteira. Campo opcional `color` na linha.
- **Loop por trecho**: arraste na faixa de cima da régua para repetir só aqueles compassos
  (alças nas pontas; clique dentro liga/desliga o loop; `Alt+clique` limpa). `Ctrl+L` faz o
  trecho da seleção (ou do compasso do cursor); no vim, `[` e `]` marcam início e fim. O trecho
  aparece na régua, na barra de ferramentas (`↺ c2–c3 ✕`) e na barra de status.
- **Medidores de nível** no mixer (por linha, por pessoa e **strip master** com volume de saída,
  L/R e LED de clip) e mini-medidores sob o nome de cada linha na grade, com pico retido.
- **Mostrador de posição** no transporte (`c12 · 3.2`, do playhead enquanto toca) e ⏮.
- **Inspector** no painel direito: propriedades da seção, do compasso, da linha e da célula do
  cursor, editáveis ali (ritornello, compassos, fórmula, dinâmica, andamento, M/S/★, volume,
  pan, pessoas, sfz/acento).
- **Fórmula de compasso por compasso** (ex.: 4/4 → 5/4): `compasso − 4/4 +` na barra de
  ferramentas e "Compasso: ±1 tempo" na paleta, no compasso do cursor ou nos selecionados.
- **Mudança de andamento por compasso** (rallentando, accelerando, "a tempo"): chip `♩` na
  faixa de dinâmica de cada compasso abre o ajuste do BPM alvo (±1/±5) e de em quantos
  compassos chegar lá (ou na hora). Pela paleta: "Andamento: rallentando/accelerando" (±10 BPM,
  ao longo dos compassos selecionados), "a tempo" e "remover". O áudio, o metrônomo, o count-in,
  o playhead, a esteira (as notas se afastam no rall.) e a exportação seguem o mapa de
  andamento; a barra de cima mostra `♪ 110` enquanto soa outro BPM; a partitura mostra `rall. → 90`. O
  treino continua valendo: escala o mapa inteiro. Campo novo `tempo` no compasso.

- **Importar músicas de outro banco ou backup** (Config → importar músicas, ou `Ctrl+K`): um
  `wadawko.db` (do app web ou de outra instalação) ou um backup `.json`; junta sem sobrescrever.
  Na primeira execução o app avisa onde fica.
- Licença do código: **MIT OR Apache-2.0** (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`).

### Mudado
- **Identidade visual nova**: tema escuro com grafites azulados, rosa e ciano; fonte Inconsolata;
  ícones próprios; botões com relevo (gradiente, brilho, sombra; aceso com brilho rosa).
- Minimap mais calmo: cores esmaecidas, 1 px entre células e densidade por tempo nas passagens rápidas.
- Barra de cima **compacta em janela estreita** (< 1200 px): nada mais se sobrepõe ao transporte.
- **Desempenho**: cada seção da grade é desenhada em cache; passar o mouse num botão
  re-renderiza só aquela seção (≈11× menos trabalho por hover).
- **Visual "estilo DAW"**: tema escuro por padrão (bancos sem tema salvo), cinzas neutros em
  níveis, cantos retos, transporte no centro da barra de cima, barra lateral estreita (64 px),
  painel direito de 280 px com cabeçalhos de painel (INSPECTOR, MÚSICAS), grade mais densa
  (células que escurecem nos tempos, barra de compasso escura), M âmbar / S azul, cartões do
  Mixer, Kit, Pessoas e Config com cabeçalho. Ver [docs/planejamento/daw.md](docs/planejamento/daw.md).
- Repositório: o app web e o protótipo `spike/` saíram do `main` (ficam na tag `web-final`);
  samples em `assets/samples/`; os builds vão para `target/` (antes `spike/target/`).
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
