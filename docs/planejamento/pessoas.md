---
type: Plan
title: "Entidade Pessoa (agrupamento de instrumentos)"
description: "Pessoas que tocam um ou mais instrumentos: cor, caneta, barramento de mixagem e linhas compartilhadas."
tags: [planejamento, pessoas, mixer]
status: stable
estado: "implementado (web e nativo)"
---

# Planejamento — Entidade "Pessoa" (agrupamento de instrumentos)

## Objetivo

Introduzir a entidade **Pessoa**, que toca um ou mais instrumentos. O agrupamento
por pessoa deve se refletir na **grade**, **partitura**, **esteira**, **mixer** e
**playback**. Um instrumento pode ser **compartilhado por 2 pessoas** — e o
compartilhamento é **por nota**: cada nota de um instrumento compartilhado é
tocada por uma pessoa específica.

## Decisões (travadas com o usuário)

1. **Pessoa = submix de áudio** (barramento por pessoa: mudo/solo/volume de grupo).
2. **Gestão em painel "Pessoas" dedicado** (criar pessoa, cor, marcar instrumentos).
3. **Cor da pessoa = moldura do grupo** para instrumentos não compartilhados
   (célula/nota/disco mantêm couro=vermelho / fuchi=azul). Em instrumento
   **compartilhado**, a nota ganha **hint da cor do dono** sobre o couro/fuchi.
4. **Relação N:N** instrumento↔pessoa. Layout de compartilhado = **linha única com
   colchetes multicor** (sem duplicar linha, sem reordenar tracks).
5. **Compartilhamento por nota**: cada nota roteia pro submix do seu dono. Mutar
   uma pessoa **silencia e deixa transparentes** só as notas dela.
6. **Atribuição via "caneta de pessoa ativa"**: seletor na toolbar define o dono
   das notas digitadas; reatribuição = selecionar nota(s) + clicar no chip da pessoa.

## Referências de UX

- **Dorico "Players"**: um player possui vários instruments, agrupados por
  chave/colchete e rotulados pelo player. Modelo direto do conceito Pessoa.
- **Folder/group tracks de DAW** (Reaper folder, Logic track stack, Ableton group):
  header colorido, mudo/solo de grupo, submix. Base pro mixer e submix.
- Sobreposição de grupos (instrumento compartilhado) → "trilhos" verticais
  coloridos no gutter (estilo colchetes aninhados de anotação), 1 trilho por
  pessoa, aceso nas linhas que ela toca. Linha compartilhada acende 2 trilhos.

---

## Modelo de dados (`src/lib/types.ts`)

Tudo opcional → músicas antigas carregam sem migração; `version` continua `1`.

```ts
export interface Person {
  id: string
  name: string
  color?: string        // hex; default = ciclo de paleta ao criar
}

export interface Track {
  // ...campos atuais...
  personIds?: string[]  // pessoas que tocam este instrumento; ausente/[] = sem pessoa
}

export interface Measure {
  // ...campos atuais...
  /** paralelo a cells; personId por célula. Só usado em track compartilhada. */
  cellOwners?: Record<string, (string | null)[]>
}

export interface Song {
  // ...campos atuais...
  people?: Person[]     // ordem das pessoas (define ordem dos trilhos)
}
```

**Regra do dono efetivo de uma célula** (`cellOwner(track, meas, ci)`):
- track com 0 pessoas → `null`
- track com 1 pessoa → sempre essa pessoa (ignora `cellOwners`)
- track compartilhada → `cellOwners[trackId][ci]`; fallback = `personIds[0]`
  se ausente ou apontando pra pessoa que saiu de `personIds` (stale).

---

## Store (`src/lib/store.svelte.ts`)

Estado novo:
- `store.activePersonId: string | null` — caneta ativa (dono de notas novas).

Helpers/derivados:
- `people()` / acesso a `song.people`.
- `cellOwner(trackId, mi, ci): string | null` — regra acima.
- `personById(id)`, `personColor(id)`.
- `railsForTracks(): { person: Person; rows: boolean[] }[]` — pra cada pessoa, quais
  linhas (na ordem de `song.tracks`) ela toca. Consumido pelas 3 views pra desenhar
  o gutter de trilhos. Suporta sobreposição/não-contiguidade nativamente.
- `isShared(track): boolean` = `(track.personIds?.length ?? 0) >= 2`.

CRUD:
- `addPerson(name?)` — cor do próximo slot da paleta.
- `renamePerson(id, name)`, `setPersonColor(id, hex)`.
- `deletePerson(id)` — remove de todos os `track.personIds` e de `cellOwners`
  (colapsa donos órfãos); **não** apaga tracks.
- `toggleTrackPerson(trackId, personId)` — adiciona/remove pertença; ao virar
  não-compartilhada (1 dono), pode limpar `cellOwners` da track.
- `assignCellOwner(trackId, mi, ci, personId)` — grava dono da célula (só em
  compartilhada); usado pela caneta e pela reatribuição.
- `setCellsOwner(cells[], personId)` — reatribuição em bloco (modo select).

Manutenção existente:
- `addTrack`: `personIds` default `[]`.
- `removeTrack`: também apagar `meas.cellOwners[trackId]`.
- Escrita de célula (`setCell`/`fillSequential`/`fillPhraseBeats`): se track
  compartilhada e há `activePersonId`, grava também `cellOwners[ci] = activePersonId`.

Undo/redo e autosave já serializam `store.song` inteiro (App.svelte) → `people` e
`cellOwners` entram de graça.

---

## Gutter de trilhos (mecanismo comum às 3 views)

À esquerda da coluna de nomes: N colunas estreitas, 1 por pessoa, na ordem de
`song.people`. Cada trilho é uma linha vertical na cor da pessoa, **acesa** apenas
nas linhas que ela toca; rótulo (nome) no topo do segmento. Linha compartilhada =
2 trilhos acesos lado a lado. Resolve sobreposição sem duplicar linha nem reordenar.

Implementado em HTML (grade, esteira, e coluna de labels da partitura) alinhado às
alturas de linha de cada view. VexFlow `StaveConnector` tipo BRACKET por run
contíguo é um extra opcional na partitura (os trilhos já comunicam o grupo).

---

## Fases

Cada fase compila e roda isolada.

### Fase 0 — Modelo & store
`types.ts`, `store.svelte.ts`. Tipos, `activePersonId`, helpers, CRUD, integração
na escrita de célula, limpeza no `removeTrack`. Sem UI. Base de tudo.

### Fase 1 — Painel "Pessoas"
Novo `PeoplePanel.svelte`; entrada no `NavRail.svelte` (hoje tem slot "Kit"/"Config"
inativos — ativar um "Pessoas"); wiring em `App.svelte` (`store.page`).
- Lista de pessoas: adicionar, renomear, swatch de cor (`<input type=color>`), excluir.
- Por pessoa: checklist de instrumentos (tracks) → `toggleTrackPerson`. Instrumento
  marcado por 2 pessoas = compartilhado.
- Mostra estado de compartilhamento (badge "compart."). Desbloqueia atribuição.

### Fase 2 — Grade
`Grid.svelte`, `ContextToolbar.svelte`.
- Gutter de trilhos à esquerda dos cabeçalhos (dentro de cada bloco-seção; o
  agrupamento por pessoa é vertical, ortogonal às seções).
- Hint de dono na célula de track compartilhada: canto/sublinhado na cor do dono.
- Nota cujo dono está mutado → conteúdo transparente (`opacity`).
- Caneta: seletor de pessoa ativa na toolbar; notas digitadas em track
  compartilhada nascem com esse dono. Modo select: selecionar células + clicar
  chip da pessoa reatribui (`setCellsOwner`).
- `trackIndex === índice do array` **preservado** (linha única, sem reordenar) →
  select/move atual (Grid 283–355) não muda.

### Fase 3 — Partitura
`score.ts`, `Score.svelte`.
- Gutter de trilhos na coluna `.score-labels`; nome da pessoa tinge o rótulo.
- Nota de track compartilhada: notehead/anotação na cor do dono; dono mutado →
  `opacity` no elemento SVG (reusar `HighlightMap`/estrutura de acesso ao SVG).
- Opcional: `StaveConnector` BRACKET por run contíguo da pessoa.

### Fase 4 — Esteira
`Lane.svelte`, `lane.ts`.
- `noteEvents` passa a expor o `ownerId` de cada evento (via `cellOwner`).
- Gutter/faixa de grupo à esquerda das lanes; disco de track compartilhada ganha
  anel na cor do dono; dono mutado → disco transparente.

### Fase 5 — Áudio submix & Mixer
`audio.ts`, `App.svelte` (effects de mix), `MixerPage.svelte`.

**Arquitetura de sinal (alvo):**
- `personBuses: Map<personId, Tone.Channel>` → master. Carregam volume da pessoa;
  mudo/solo resolvidos entre pessoas.
- Refatorar vozes: hoje `makeVoice(instrument, out)` fixa a saída. Passar a
  **saída por golpe**: `voice.head(out, time, vel, gain?, pan?)`. Vozes viram
  por-instrumento (buffers independem da saída; `playLayered` já cria `Gain` por
  golpe). Assim um golpe roteia pro barramento certo.
- **Track não compartilhada (0–1 pessoa):** mantém `Tone.Channel` por track
  (M/S/vol/pan atuais); saída conecta ao barramento do dono (se houver) senão master.
- **Track compartilhada:** cada golpe roteia direto pro barramento do **dono da
  célula**. M/S/vol/pan da track aplicados no schedule (skip se muda) e como
  `gain`/`pan` por golpe.

**Resolução mudo/solo (por golpe):**
- Track audível = `!track.muted && (nenhuma track em solo || track.solo)`.
- Pessoa audível = `!person.muted && (nenhuma pessoa em solo || person.solo)`.
- Golpe soa sse `trackAudível && (dono == null || pessoaAudível)`.
- Golpe silenciado pela regra da pessoa → também renderiza transparente nas views
  (Fases 2–4 já leem o estado de mudo da pessoa).

**Mixer:** strips agrupadas por pessoa com header colorido; strip de barramento da
pessoa (vol/M/S) + strips das tracks dentro. Instrumento compartilhado aparece 1
vez, marcado como compartilhado.

---

## Casos de borda

- Import de música antiga (sem `people`): tudo sem pessoa; views funcionam,
  gutter vazio; playback = comportamento atual (track → master).
- Excluir pessoa: limpa `personIds` e `cellOwners`; tracks e notas permanecem.
- Compartilhada → 1 dono (removeu pessoa): vira não-compartilhada; `cellOwners`
  colapsa pro dono restante.
- `cellOwners` apontando pra pessoa fora de `personIds`: fallback `personIds[0]`.
- Nota sem dono em track compartilhada: fallback `personIds[0]`.
- `removeTrack`: apagar `cellOwners[trackId]` junto de `cells[trackId]`.

## Riscos

- **Fase 5 (áudio)** é a mais arriscada: refator de roteamento de voz + duas
  camadas de mudo/solo (track e pessoa). Fazer por último; as views (2–4)
  independem dela (leem só o estado de mudo, não o grafo de áudio).
- Alinhamento do gutter de trilhos entre views (alturas de linha diferentes):
  cada view desenha seu próprio gutter alinhado; conceito compartilhado, CSS local.
- Sobreposição de trilhos com muitas pessoas: limitar largura do gutter / rolar.
