---
type: Overview
title: "WaDAWko — visão geral"
description: "O que é o WaDAWko, para quem é, o que ele faz hoje e como o repositório está organizado."
tags: [visão-geral, produto]
status: stable
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-16T13:10:00Z
---

# WaDAWko — visão geral

**WaDAWko** é um editor de partituras e "DAW" para grupos de **taiko** (tambores japoneses).
As músicas são escritas em **kuchi shōga** — as sílabas faladas que os grupos usam para
ensinar ritmos (`don`, `doko`, `ka`, `tsuku`…) — numa grade por linha de instrumento, e o app
toca o resultado com samples reais, mostra a partitura e anima uma "esteira" de notas para
ensaio.

O projeto nasceu como app web (Svelte, 2026-07) e foi reescrito como **app desktop nativo em
Rust + GPUI** (2026-11). O app web está arquivado no branch `web-legacy`.

# O que dá para fazer

- **Escrever** a música digitando sílabas na grade, por seções (com ritornello), compassos,
  subdivisão (colcheia, semicolcheia, fusa), dinâmicas por compasso e por linha, crescendo/
  diminuendo, kiai (gritos) e sforzando. Frases do repertório inseridas pela paleta.
- **Organizar** quem toca o quê: [pessoas](/planejamento/pessoas.md) com cor e caneta, linhas
  compartilhadas, mixer com barramento por pessoa.
- **Ouvir**: playback com loop, metrônomo, count-in, swing, treino com aceleração por volta e
  região (seções selecionadas); instrumentos ajustáveis no [Kit](/planejamento/instrumentos.md).
- **Ver**: grade, partitura (notação própria sobre a fonte Bravura) e esteira com pétalas.
- **Exportar**: música em JSON, áudio em WAV/MP3, partitura em SVG/PNG/impressão A4.
- **Operar pelo teclado**: paleta de comandos, saltos por rótulo, which-key e modo vim
  opcional — ver [atalhos](/referencia/atalhos.md).

Passo a passo de uso: [guia de uso](/guias/uso.md). Instalar e compilar:
[instalar e compilar](/guias/instalar-e-compilar.md).

# Organização do repositório

| Caminho | O que é |
|---|---|
| `crates/` | Workspace Rust — um crate por responsabilidade ([mapa](/arquitetura/workspace.md)) |
| `docs/` | Esta documentação, um bundle [OKF v0.2](https://github.com/GoogleCloudPlatform/open-knowledge-format) |
| `public/samples/` | Samples `.ogg` e créditos — embutidos no binário ([créditos](/creditos.md)) |
| `assets/fonts/` | Fontes embutidas (Zen Kaku Gothic New, Shippori Mincho, Bravura) |
| `tools/` | Scripts do repositório (gerar a página de atalhos, checar o bundle OKF) |
| `data/` | Fixtures de teste (backup das músicas do app web) |
| `spike/`, `src/`, `server/`… | Protótipo GPUI e app web antigos — saem do `main` na [fase B do plano](/planejamento/repo.md) |

# Estado

App nativo em uso diário desde 2026-11-13. O que falta para o repositório ficar completo
(licença, CI, empacotamento, limpeza da raiz) está em [repositório completo](/planejamento/repo.md);
o histórico das decisões está nos [planos](/planejamento/).
