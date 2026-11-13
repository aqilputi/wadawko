# Planejamento — Criador de Instrumentos (Kit)

## Objetivo

Sessão onde o usuário cria instrumentos a partir de um **base** (shime, chū, ōdaiko…)
e ajusta parâmetros para variar o som — shime mais agudo, outro mais grave, outro
com mais overtones, etc. A seleção de instrumentos da interface principal passa a
listar **essas defs** (builtins + custom). Deletar um instrument custom **não mata**
a linha que o usa: ela volta ao som base.

## Decisões (travadas)

1. **Escopo por música**: defs custom em `Song.instruments`; entram no export/import.
2. **Parâmetros**: pitch (agudo/grave), timbre (filtro cutoff), overtones (ressonância),
   decaimento e ganho.
3. **Delete em uso** → cai no som base, linha vive (notas intactas).

## Modelo de dados (`types.ts`)

Params **separados por zona**: couro (`head`) e borda/fuchi (`rim`) têm ajustes
próprios (pitch, ganho, timbre, overtones, decay), e cada zona pode ser ouvida
isolada no editor. Há **presets** builtin bem distintos (agudo, grave, anelado…).

```ts
export interface ZoneParams {
  pitch?: number                // semitons; 0 = base → playbackRate = 2^(pitch/12)
  gain?: number                 // trim em dB
  tone?: { type: 'lowpass' | 'highpass' | 'bandpass'; cutoff: number; q: number }
  overtones?: number            // 0..1 → soma no Q (ring)
  decay?: number                // 0..1 → encurta(<0.5)/alonga(>0.5) o sample
}

export interface InstrumentDef {
  id: string
  name: string
  base: InstrumentId            // voz/sample base
  builtin?: boolean             // defs padrão (constantes no código: bases + presets)
  head?: ZoneParams             // couro
  rim?: ZoneParams              // borda (fuchi)
}

export interface Track {
  // ...
  instrument: InstrumentId      // BASE / fallback (mantido)
  instrumentId?: string         // ref à def (builtin id ou custom id)
}

export interface Song {
  // ...
  instruments?: InstrumentDef[] // só as CUSTOM; builtins são constantes
}
```

- **Builtins** = constante `BUILTIN_INSTRUMENTS` no código (os 7 atuais: id = base,
  name = label de `INSTRUMENTS`, base = id, `builtin: true`). Não vão pro JSON.
- **Paleta** = `[...BUILTIN_INSTRUMENTS, ...(song.instruments ?? [])]`.
- **Resolução** `resolveInstrument(track)`: acha a def por `track.instrumentId` na
  paleta; se faltar (deletada / música antiga), **fallback** para um builtin de
  `track.instrument`. Nunca retorna null → a linha sempre soa.
- **Migração**: nada. Músicas antigas não têm `instrumentId` → resolvem pelo base.

## Store (`store.svelte.ts`)

- `paletteInstruments()` → builtins + custom.
- `resolveInstrument(track): InstrumentDef`.
- `instrumentLabel(track): string` (nome da def resolvida) — usado nos rótulos.
- CRUD: `addInstrument(base, name?)` (cria custom a partir do base), `updateInstrument(id, patch)`,
  `renameInstrument`, `duplicateInstrument`, `deleteInstrument(id)`.
  - `deleteInstrument`: remove de `song.instruments`; linhas com esse `instrumentId`
    ficam como estão → `resolveInstrument` cai no base (linha vive). Opcional:
    limpar `instrumentId` das linhas afetadas pra refletir o fallback na UI.
- `addTrack(defId)`: seta `instrument = def.base` e `instrumentId = def.id`; nome
  da linha = nome da def.

## Áudio (`audio.ts`)

Hoje `makeVoice(instrument, out)` usa `SAMPLE_FILES[instrument]` + `TUNING[instrument]`.
Passa a `makeVoice(def, out)`:

- Base escolhe `SAMPLE_FILES[def.base]` / `TUNING[def.base]`.
- **Pitch**: `rate *= 2^(def.pitch/12)` sobre `headRate`/`rimRate`.
- **Ganho**: soma `def.gain` (dB) ao ganho da voz.
- **Timbre/overtones**: insere `Tone.Filter({ type: def.tone.type, frequency: cutoff, Q: cutoff*overtones })`
  na cadeia `src → gain → filtro → out`. Sem `tone` → sem filtro (idêntico a hoje).
- **Decay**: `ToneBufferSource` com `fadeOut` e/ou `stop(time + bufferDur * f(decay))`.
- `def` sem overrides (builtin puro) = som atual (compat total).

Engine: `ensureVoice(track, ownerId)` resolve a def do track e passa pra `makeVoice`.
Chave de voz/canal continua por `channelKey` (track/pessoa) — inalterado. Editar
params de uma def em uso deve **recriar** as vozes daquele instrumento (invalidar
cache de voz por def, ou rebuild no próximo `schedule`).

Preview: botão "tocar" no editor usa `engine.previewInstrument(def)` (fora do transport).

## UI — página Kit (`KitPage.svelte`)

- Ativa o slot **🥁 Kit** no `NavRail` (hoje inativo); `store.page = 'kit'`.
- Lista de instrumentos (builtins marcados; custom com editar/duplicar/excluir).
- "+ novo": escolhe o base → cria custom (params = default do base).
- Editor de params: sliders (pitch −24..+24 st, cutoff, ressonância 0..1, decay 0..1,
  ganho −12..+12 dB) + seletor de tipo de filtro + botão ▶ preview.
- Builtins: não editáveis/deletáveis (ou "duplicar pra editar").

## Paleta principal usa as defs

- `ContextToolbar` "+ linha": `<select>` lista `paletteInstruments()`; `addTrack(defId)`.
- Rótulos que hoje usam `INSTRUMENTS[track.instrument]` passam a usar
  `instrumentLabel(track)`: `Grid.svelte`, `Lane.svelte`, `MixerPage.svelte`,
  `PeoplePanel.svelte`. (`Score` usa `track.name`, sem mudança.)

## Fases

0. **Modelo & store**: tipos, `BUILTIN_INSTRUMENTS`, `resolveInstrument`, CRUD, `addTrack(defId)`.
1. **Áudio**: `makeVoice(def)`, filtro/pitch/gain/decay, `previewInstrument`, invalidação de voz ao editar.
2. **Kit page**: NavRail + `KitPage.svelte` (lista, criar, editor de params, preview, delete-fallback).
3. **Paleta**: ContextToolbar + rótulos via `instrumentLabel`.
4. **Build & check**.

## Casos de borda

- Música antiga (sem `instrumentId`): resolve pelo base. ✓
- Instrument deletado em uso: linha vive, som = base; opcional limpar `instrumentId`.
- Export/import: `song.instruments` viaja junto; builtins existem em qualquer lugar (constantes).
- Editar params ao vivo tocando: recriar vozes do instrumento (glitch mínimo aceitável).
- Nome duplicado: permitido (id é único); UI pode avisar.
