# Taiko-fu — Planejamento Inicial

> Editor web de músicas de taiko em notação de grade (kuchi shōga), com playback.
> Para o grupo Sukeroku. Nome "taiko-fu" (太鼓譜, "partitura de taiko") é provisório.

Data: 2026-07-02 (rev. 2: dinâmicas + engines de playback) · Status: planejamento, pré-código

---

## 1. Problema

Música de taiko não usa partitura ocidental. A transmissão é oral via **kuchi shōga**
(口唱歌): sílabas que representam cada golpe — *don* (batida cheia no centro),
*doko/doro* (duas batidas rápidas), *tsu* (batida fantasma/suave), *ka* (borda/fuchi),
*kara* (duas na borda), *su* (pausa). Muitos grupos codificam a mão junto:
mão direita = don/tsu/ka, mão esquerda = kon/ku/ra.

Quando grupos documentam por escrito, usam **grades**: uma linha por instrumento,
uma coluna por subdivisão do tempo, sílaba dentro da célula (exemplo real: handout
da [Uzume Taiko](https://www.uzume.com/sites/default/files/2022-10/Taiko%20Notes%20&%20Notation.pdf) —
grade `1 & 2 & 3 & 4 &`, MAIÚSCULA = forte, minúscula = suave). Hoje isso é feito
em Google Docs/planilha, sem playback, sem validação rítmica, sem compartilhamento
estruturado.

Estilo Sukeroku agrega ainda coreografia (giros de bachi, poses, naname-dai),
transmitida só oralmente — a notação ideal captura ritmo + mão + movimento.

## 2. O que já existe (pesquisa 2026-07)

| Ferramenta | O que é | Por que não resolve |
|---|---|---|
| [Wagaku Hitosuji](https://jonkara.com/soft/wagaku/) | App japonês freemium p/ instrumentos japoneses, com modo taiko | Notação derivada de pauta (1 linha, notas acima/abaixo = mão D/E), não é grade nem sílabas; UI só em japonês |
| [Groove Scribe](https://github.com/montulli/GrooveScribe) | Grade de bateria web, open source, playback, linha de sticking R/L, URL compartilhável | Semântica de drum-set (bumbo/caixa/chimbal), sem sílabas, kit fixo |
| [abc-taiko](https://github.com/marceltn/abc-taiko) | Protótipo open source: sílabas kuchi shōga → notação ABC | Pré-alpha abandonado (~2023), sem playback, voz única |
| [WadaikoNoteEditor](https://github.com/kanamechanlover/WadaikoNoteEditor) | Tentativa de editor de taiko | Mock HTML, 2 commits, morto |
| MuseScore + soundfont de taiko | O que grupos japoneses realmente usam ([tutorial](https://musescore.org/en/node/8341)) | Pauta ocidental, exatamente o que queremos evitar |
| [Kuchishoga Periodic Table](https://www.taikosource.org/song-database/composition-tools) (LA Taiko Institute, CC BY-SA) | Sistema gráfico de notação p/ sons **e movimentos** | É PDF/documento, não software — mas é a melhor referência de vocabulário p/ o nosso design |
| [TJA Editor](https://keishispl.github.io/tja-editor/) / taiko-web | Editores de chart do jogo Taiko no Tatsujin | Formato de jogo (don/ka numa timeline), não serve p/ ensemble; mas prova o modelo linha+subdivisão em web |
| [kaDON](https://kadon.com/) | Plataforma de ensino de taiko (vídeo, assinatura) | Não tem editor — confirma demanda por taiko digital |

**Lacuna confirmada** (buscas em EN + JA, web + GitHub): não existe app web que
combine grade + sílabas nativas (don/ka/doko/tsu/su) + sticking + multi-instrumento +
playback. Concorrência real: zero.

## 3. Visão do produto

Página web estática, grátis, offline-capaz. Três promessas:

1. **Documentação** — escrever música como o grupo já pensa: grade de sílabas,
   uma linha por instrumento (shime, chū-daiko, ōdaiko, kane...), seções nomeadas
   (jiuchi, tema A, solo, kiai marcados). Imprime bonito em A4 paisagem.
2. **Playability** — apertar play e ouvir a música com samples reais de taiko;
   loop de trecho pra ensaio, mudar andamento, mutar/solar linhas, count-in.
3. **Compartilhamento** — música inteira vira URL (sem servidor) ou arquivo
   `.json` versionado no drive/git do grupo.

Não-objetivos (v1): colaboração em tempo real, mobile-first de edição (leitura
no celular sim), MusicXML. Notação de pauta existe só como **visão read-only**
(ver abaixo) — a grade é o único editor.

**Visão partitura (implementada no protótipo):** render read-only via VexFlow
seguindo a convenção de partitura de taiko (Wagaku Hitosuji/MuseScore de grupos):
pauta de 1 linha por tambor, cabeça normal = pele, cabeça × = borda, acento >,
sílaba kuchi shōga embaixo da nota (compostas quebradas por golpe: do-ko),
**mão direita acima da linha / esquerda abaixo** (vocabulário carrega a mão:
don/ka/tsu = dir., kon/ra/ku = esq., compostas alternam D-E).
Serve p/ percussionista de formação ocidental e impressão alternativa.

## 4. Conceitos de domínio (modelo mental)

- **Música** → metadados (título, autor, BPM base) + lista de **seções**.
- **Seção** → nome + compassos; cada compasso tem nº de tempos e **subdivisão**
  (semínima/colcheia/semicolcheia — grade de 1, 2 ou 4 células por tempo; swing depois).
- **Linha (track)** → instrumento (timbre p/ playback) + nome de exibição.
- **Célula** → golpe: sílaba (`don`, `do`, `ko`, `tsu`, `ka`, `su`...), acento
  (forte/normal/suave → maiúscula/normal/minúscula na exibição), mão (D/E/ambas).
- **Vocabulário de sílabas configurável por grupo** — decisão central do design.
  Kuchi shōga NÃO é padronizado (cada grupo/região varia; San Jose chama "kuchishoka").
  O app traz um dicionário default (baseado no Periodic Table + convenções Sukeroku)
  e o grupo edita: sílaba → som + nº de golpes + mão default.
- **Kiai** → linha de anotação por seção (não é golpe).
- **Movimento/kata** (v2) → anotação livre por célula ou compasso (texto/ícone),
  p/ coreografia Sukeroku.

### Dinâmicas e expressão (estilo MuseScore)

O app deve suportar as marcações expressivas que o MuseScore tem, adaptadas à grade:

- **Dinâmica por célula**: *pp, p, mp, mf, f, ff* + acento (>). Na exibição, a
  convenção kuchi shōga já resolve parte disso (DON maiúsculo = forte, don = normal,
  tsu minúsculo = suave) — dinâmica explícita complementa, não substitui.
- **Hairpins (cresc./decresc.)**: marcação de intervalo (célula início → célula fim)
  desenhada sob a grade, como no MuseScore. No playback vira rampa de dinâmica.
- **Dinâmica por seção/linha**: valor default herdável (seção em *f*, linha do shime
  em *mp*), célula pode sobrescrever.
- **Modelo de playback correto p/ percussão** (por que gain sozinho soa falso):
  um golpe *ff* difere de *pp* em **timbre** (ataque, brilho, harmônicos), não só
  volume. Escalar 1 sample no ganho mantém a "cara" espectral errada. Modelo:
  1. **Velocity layers** — gravações separadas por faixa de dinâmica; a dinâmica
     da célula escolhe a layer;
  2. **interpolação fina de ganho** dentro da layer (crescendo não "pula" na
     fronteira entre layers);
  3. **round robins** — takes alternados por layer, senão rulo vira metralhadora.
- Hairpin sobre sequência de golpes = re-seleção de layer por golpe ao longo da
  curva + rampa leve de ganho (`linearRampToValueAtTime`). É o que o MuseScore faz
  internamente (velocity por nota + expressão contínua).
- **SFZ como fonte de verdade dos instrumentos**: formato padrão que declara
  layers/round robins (`lovel/hivel`, `seq_position`). Não precisamos de engine SFZ
  completa — parsear o subconjunto (sample, lovel/hivel, seq, volume) e gerar um
  mapa JSON é uma tarde de trabalho. O SCC Taiko Drums já vem em SFZ pronto.

## 5. Formato de dados (fonte da verdade)

JSON custom versionado — nenhum formato existente (ABC, MML, MusicXML, tab) carrega
sílabas + mão + grade. MusicXML de percussão é notoriamente lossy; descartado.

```json
{
  "version": 1,
  "title": "Matsuri",
  "tempo": 96,
  "vocabulary": {
    "don": { "sample": "chu-don", "hand": "R" },
    "doko": { "sample": "chu-don", "hits": 2 },
    "ka":  { "sample": "chu-ka", "hand": "R" },
    "su":  { "rest": true }
  },
  "tracks": [
    { "id": "shime", "instrument": "shime-daiko", "name": "Shime (jiuchi)" },
    { "id": "chu1",  "instrument": "chu-daiko",   "name": "Chū 1" }
  ],
  "sections": [
    { "name": "Jiuchi", "repeat": 4, "dynamic": "mf", "measures": [
      { "beats": 4, "subdivision": 2,
        "cells": {
          "shime": ["ten","ten","ten","ten","ten","ten","ten","ten"],
          "chu1":  ["DON","su","DON","su","doko","don","KA","su"]
        },
        "hairpins": [
          { "track": "chu1", "from": 4, "to": 7, "type": "cresc", "target": "ff" }
        ] }
    ] }
  ]
}
```

(Formato ilustrativo; célula vira objeto `{syl, dynamic, accent, hand}` quando
precisar de mais que a string. Dinâmica herda: seção → linha → célula → hairpin.)

- `version` desde o dia 1 p/ migração.
- **Share por URL**: JSON → deflate (`CompressionStream` nativo) → base64 no
  fragment `#`. Ideia do Groove Scribe generalizada; zero backend.
- Export futuro de interoperabilidade: MIDI (cada linha = nota fixa) antes de
  qualquer MusicXML.

## 6. Arquitetura técnica

| Camada | Escolha | Por quê |
|---|---|---|
| Build/stack | **Vite + Svelte 5 + TypeScript** | Grid editor é estado→DOM intenso; Svelte compila leve, sem vDOM atrapalhando o cursor de playback. Site estático puro (GitHub Pages/Cloudflare Pages) |
| Renderização | **Grade custom em CSS Grid/DOM** — nenhuma lib de notação | VexFlow/abcjs/alphaTab/OSMD são engravers de pauta ocidental; lutaríamos contra todos. Grade DOM dá hit-testing, navegação por teclado, sílaba = texto na célula, highlight de playback = classe CSS. ~centenas de linhas |
| Playback | **Tone.js 15.x** (Transport) + seletor de velocity layer próprio | `Transport` (loop por compasso, BPM em tempo musical), `Channel` por linha p/ mute/solo, `Part` p/ count-in. Velocity no `Tone.Sampler` é só ganho ([issue #1034](https://github.com/Tonejs/Tone.js/issues/1034)) → seleção de layer/round-robin é nossa (~40 linhas sobre `Players`). Regra de ouro: sempre agendar com o `time` do callback, nunca "agora"; `Tone.start()` em gesto do usuário |
| Export de áudio | **`Tone.Offline` / OfflineAudioContext** no browser | Mesmo código de agendamento renderiza WAV mais rápido que tempo real, no CPU do usuário. Encoder WAV é ~30 linhas; MP3 via `lamejs` se precisar |
| Samples | **[SCC Taiko Drums](https://schristiancollins.com/vi-percussion.php)** (SFZ, 15 tambores, velocity layers + round robins, CC BY-SA 4.0) — mesmo material do [Floe Taiko Drums](https://github.com/floe-audio/Taiko-Drums); + Freesound CC0 p/ kane/fuchi; ou gravar os taikos do próprio grupo | SFZ já declara layers/RR prontos p/ dinâmica realista. Converter p/ `.ogg` + mapa JSON gerado do SFZ. Com layers o total cresce (~dezenas de MB) — lazy load por instrumento, precache PWA |
| Persistência | localStorage (autosave com debounce) + export/import `.taiko.json` + URL share | IndexedDB só se surgir biblioteca multi-música |
| Offline | **PWA** via vite-plugin-pwa, precache app + samples | Ensaio em ginásio sem internet; instala na home screen do celular |
| Impressão | **CSS `@media print`** na mesma grade (A4 paisagem, `break-inside: avoid` por linha de compassos) | Zero dependência; "salvar como PDF" do browser. SVG export depois, se precisar |

### 6.1 Engine de playback — pesquisa (2026-07) e veredito sobre backend

**Backend NÃO é necessário.** Tudo que o app precisa — samples com velocity layers,
crescendo, acentos, mudança de andamento, export WAV — roda no browser via Web Audio.
Backend de renderização (container com FluidSynth/sfizz) adicionaria custo de hosting,
fila, upload/download e ops, para output idêntico. Único gatilho futuro legítimo:
gerar MP3 no servidor p/ links de compartilhamento sem espera do cliente — e aí
[FluidSynth 2.5 CLI](https://www.fluidsynth.org/) (SF2→WAV, LGPL) ou
[`sfizz_render`](https://github.com/sfztools/sfizz-render) (SFZ→WAV, BSD-2) resolvem
num GitHub Action.

Opções avaliadas p/ tocar no browser:

| Engine | Tipo | Prós | Contras |
|---|---|---|---|
| **Tone.js 15.x** (MIT) — *escolhido* | Framework Web Audio | `Transport` = tempo musical, loop, BPM/ritardando de graça; `Tone.Offline` p/ export; rampas contínuas (`rampTo`) melhores da categoria | Velocity é só ganho — seleção de layer/RR é nossa (trivial) |
| [smplr](https://github.com/danigb/smplr) v1.0 (MIT, ativo 2026) | Sampler minimalista TS | Velocity layers + round robins nativos ("SFZ-like"); sem framework | Sem transport — agendamento/loop/tempo é todo nosso |
| [spessasynth_lib](https://github.com/spessasus/spessasynth_lib) v4.3 (Apache-2.0, muito ativo) | Synth SF2 completo em AudioWorklet | Semântica MIDI real: velocity splits, CC11 p/ hairpin, export offline embutido | Exige empacotar samples em SF2; round robin é gambiarra no formato SF2 |
| [alphaTab](https://alphatab.net/)/alphaSynth (MPL-2.0) | Notação+synth integrados | Único que interpreta pp–ff/hairpin direto da partitura; export WAV embutido | Notação é pauta/tab ocidental — brigaríamos com o framework; hairpin vira degraus por tempo, não rampa; sem RR |
| [js-synthesizer](https://github.com/jet2jet/js-synthesizer) (FluidSynth WASM) | Synth SF2 WASM | Paridade FluidSynth | spessasynth cobre o mesmo, mais limpo e ativo |
| [sfizz-webaudio](https://github.com/sfztools/sfizz-webaudio) (WASM) | Engine SFZ | Semântica SFZ completa (formato dos nossos samples) | Protótipo parado — não construir em cima |
| MuseScore CLI / engine | Backend score→áudio | Semântica completa de partitura | Não existe forma embeddable; GPL; export headless historicamente quebrado ([#19160](https://github.com/musescore/MuseScore/issues/19160)) |

**Decisão:** Tone.js `Transport` como relógio/transporte + sampler próprio fino
(seleção de velocity layer + round robin a partir de mapa JSON derivado do SFZ do
SCC Taiko). smplr é o plano B se Tone.js pesar. spessasynth_lib é alternativa se
preferirmos emitir MIDI e delegar interpretação — mas perde round robin, e round
robin importa muito p/ taiko (rulos).

## 7. Fases

### Fase 0 — Especificação da notação (com o grupo!) — ~1 semana
Sentar com o Sukeroku e fixar: vocabulário de sílabas do grupo (mapa sílaba→som→mão),
instrumentos usados, como escrevem hoje (fotos dos papéis/planilhas atuais).
Rascunhar 1 música conhecida no formato JSON à mão. **Maior risco do projeto é
errar aqui, não no código.**

### Fase 1 — MVP editor + playback — 2–3 semanas
- Grade editável: linhas × células, célula é input de texto (Tab/espaço avança,
  frase com espaços flui uma palavra por célula), acento, compasso 4/4,
  subdivisão selecionável colcheia/semicolcheia/fusa com reamostragem,
  múltiplas linhas.
- Célula composta: palavra colada ("dokodoko", "dondon") ocupa UMA célula e os
  golpes subdividem o tempo dela por peso de alongamento — dokodoko numa
  colcheia = 4 fusas; partitura desenha as subdivisões (quiáltera p/ 3 golpes).
- Playback Tone.js: play/stop, BPM, cursor visual na grade, mute/solo.
- Autosave localStorage, export/import JSON.
- Visão partitura read-only (VexFlow, 1 linha por tambor). *(entregue)*
- 1 música real do grupo transcrita como dogfood.

### Fase 2 — Estrutura de música — 2 semanas
- Seções nomeadas com repetição, subdivisão por compasso, compassos não-4/4.
- **Célula com span** (sílaba composta ocupando N células, estilo KA-RA do Uzume):
  timing = duração do span ÷ nº de golpes — resolve fusa (dorotsuku em 1 célula),
  sílaba atravessada e rulo lento com um mecanismo só.
- Loop de seção/trecho p/ ensaio, count-in, mudança de BPM por seção.
- Share por URL. CSS de impressão (grade e partitura).

### Fase 3 — Dinâmicas e expressão — 2 semanas
- Dinâmica por célula/linha/seção (pp–ff, herança), acentos.
- Hairpins cresc./decresc. desenhados na grade → rampa de velocity no playback.
- ~~Samples com velocity layers + round robins~~ *(antecipado: protótipo já usa
  Floe/SCC Taiko Drums em ogg — chū com 3 layers p/m/f + RR, ōdaiko/shime/borda
  com RR; kane segue sintetizado, pack não tem atarigane. Créditos CC BY-SA no
  footer + public/samples/CREDITS.md)*
- Kane com sample real (Freesound CC0) no lugar do MetalSynth.
- Export WAV via `Tone.Offline`.

### Fase 4 — Identidade taiko — 2 semanas
- Vocabulário editável (dicionário do grupo), mão D/E exibida (cor/sublinhado),
  kiai, dinâmica visual (MAIÚSC/minúsc automática por dinâmica).
- PWA offline. Samples definitivos (gravar o grupo?).

### Fase 5 — Extras (backlog)
- Anotação de movimento/kata (Sukeroku!), export MIDI, export MP3 (lamejs ou
  render server-side via GitHub Action + FluidSynth/sfizz_render), export SVG/PNG,
  biblioteca de músicas (IndexedDB), modo "leitura de ensaio" mobile,
  metrônomo/jiuchi automático de fundo, i18n (PT/EN/JA).

## 8. Riscos e decisões em aberto

- **Vocabulário não padronizado** — mitigado com dicionário configurável (Fase 3),
  mas o default da Fase 1 precisa vir do grupo (Fase 0).
- **Golpes compostos** (*doko* = 2 golpes numa célula): tratar como 1 sílaba que
  dispara 2 samples deslocados, ou forçar subdivisão maior? Decidir na Fase 0 com
  exemplos reais.
- **Licença dos samples**: CC BY-SA exige atribuição + share-alike; gravar os
  próprios taikos elimina a questão e soa como o grupo de verdade.
- **Peso dos samples com velocity layers**: dezenas de MB (vs ~2 MB de 1 layer).
  Mitigação: começar Fase 1 com 1 layer por articulação; layers completas entram
  na Fase 3 com lazy load por instrumento.
- **Notação de movimento** é terra sem dono (só o Periodic Table tenta) — chance
  de contribuição original, mas fica fora do MVP de propósito.

## 9. Referências principais

- Kuchi shōga: https://en.wikipedia.org/wiki/Kuchi_sh%C5%8Dga
- Kuchishoga Periodic Table + Compose By Numbers: https://www.taikosource.org/song-database/composition-tools
- Exemplo real de grade (Uzume): https://www.uzume.com/sites/default/files/2022-10/Taiko%20Notes%20&%20Notation.pdf
- Groove Scribe (referência de UX de grade): https://github.com/montulli/GrooveScribe
- Tone.js timing: https://github.com/Tonejs/Tone.js/wiki/Accurate-Timing
- Samples: https://schristiancollins.com/vi-percussion.php (SCC Taiko, SFZ) ·
  https://github.com/floe-audio/Taiko-Drums · https://virtualplaying.com/virtual-playing-orchestra/ ·
  https://freesound.org
- Formato SFZ (velocity layers/round robins): https://sfzformat.com
- Oedo Sukeroku Taiko: https://en.wikipedia.org/wiki/Oedo_Sukeroku_Taiko
