---
type: Schema
title: "Formato da música (Song v1)"
description: "O JSON de uma música — o mesmo no banco, nos arquivos .taiko.json e no app web antigo: campos de música, linhas, compassos, seções, pessoas e instrumentos."
tags: [referência, formato, json, modelo]
status: stable
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-16T13:10:00Z
fontes: [crates/wadawko-core/src/model.rs, src/lib/types.ts]
---

# Formato da música (Song v1)

Fonte da verdade: `crates/wadawko-core/src/model.rs` (porte de `src/lib/types.ts` do app web).
Chaves em **camelCase**; campos opcionais ausentes não são gravados; **campos desconhecidos
fazem round-trip** (preservados intactos), então arquivos de versões futuras não perdem dados.

# Schema

## Música (`Song`)

| Campo | Tipo | Notas |
|---|---|---|
| `version` | número | sempre `1` |
| `title` | texto | |
| `tempo` | número | BPM |
| `swing` | número? | 0 = reto |
| `tracks` | `Track[]` | linhas, na ordem da grade |
| `measures` | `Measure[]` | compassos da música inteira |
| `sections` | `Section[]`? | seções por compasso inicial |
| `people` | `Person[]`? | |
| `instruments` | `InstrumentDef[]`? | instrumentos custom do Kit |
| `mainTrackIds` / `mainTrackId` | texto[]? / texto? | linhas principais (★) da partitura |

## Linha (`Track`)

| Campo | Tipo | Notas |
|---|---|---|
| `id`, `name` | texto | |
| `instrument` | id do sample base | `shime`, `chu`, `hira`, `nagado`, `onagado`, `okedo`, `odaiko`, `kane` |
| `instrumentId` | texto? | instrumento do Kit (custom ou pronto) |
| `muted`, `solo` | bool | |
| `volume`, `pan` | número? | dB; −1…1 |
| `personIds` | texto[]? | 2+ pessoas = linha compartilhada |
| `kyai` | objeto? | voz do kiai da linha |

## Compasso (`Measure`)

| Campo | Tipo | Notas |
|---|---|---|
| `beats` | número | tempos do compasso — a fórmula `beats`/4 (4 = 4/4, 5 = 5/4); o compasso dura `beats` tempos |
| `subdivision` | número | células por tempo (2, 4, 8) |
| `cells` | `{ trackId: (texto\|null)[] }` | a sílaba de cada célula, por linha |
| `cellOwners` | igual a `cells`? | dona (pessoa) de cada nota em linha compartilhada |
| `dynamic` | `pp…ff`? | dinâmica geral |
| `hairpin` | `cresc`/`dim`? | |
| `trackDynamics` | mapa por linha? | dinâmica/hairpin por linha |
| `kyai` | igual a `cells`? | textos de kiai por célula |

## Seção, pessoa e instrumento

- `Section`: `id`, `name`, `start` (índice do compasso), `repeat` (ritornello, padrão 1).
- `Person`: `id`, `name`, `color` (`#rrggbb`), `muted`, `solo`, `volume`.
- `InstrumentDef`: `id`, `name`, `base` (sample), `builtin`, `head`/`rim` com `pitch`
  (semitons), `gain` (dB), `decay` (0…1, 0,5 = natural), `overtones` (0…1) e `tone`
  (`{ type: lowpass|highpass|bandpass, cutoff, q }`).

# Examples

Uma linha, um compasso de 2 tempos em colcheias:

```json
{
  "version": 1,
  "title": "Exemplo",
  "tempo": 120,
  "tracks": [{ "id": "t1", "name": "Chū", "instrument": "chu", "muted": false, "solo": false }],
  "measures": [{ "beats": 2, "subdivision": 2, "cells": { "t1": ["don", "doko", null, "ka"] } }]
}
```
