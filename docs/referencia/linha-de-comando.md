---
type: Reference
title: "Linha de comando"
description: "Opções do app `wadawko` (banco, tema, demos e autotestes) e os comandos da `wadawko-cli` (listar, exportar, renderizar, importar)."
tags: [referência, cli, opções]
status: stable
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-16T13:10:00Z
fontes: [crates/wadawko-app/src/main.rs, crates/wadawko-cli/src/args.rs]
---

# Linha de comando

As duas ferramentas imprimem a ajuda atualizada com `--help`; esta página é um resumo.

# `wadawko` — o app

| Opção | Efeito |
|---|---|
| `--db CAMINHO` | banco SQLite (padrão `~/.local/share/wadawko/wadawko.db`) |
| `--migrate-from ARQ` | se o banco não existir, copia `ARQ` para lá (a origem só é lida) |
| `--read-only` | abre sem gravar — edições ficam só na memória |
| `--samples-dir PASTA` | carrega os `.ogg` de `PASTA` em vez dos embutidos |
| `--song TÍTULO` | abre a música com esse título (busca aproximada) |
| `--dark` / `--light` | força o tema (padrão: o salvo no banco) |
| `--mute` | volume de saída 0 (o motor de áudio roda normalmente) |
| `--bench N` | toca N s com follow e zoom oscilando e imprime o relatório de frames |
| `--demo MODO` | abre num estado específico (ver abaixo) |
| `--selftest` | abre, pinta, toca, edita, desfaz/refaz, grava e sai (linhas `TESTE`) |

**Modos de `--demo`** — para screenshots, testes manuais e medições:
`palette`, `hints`, `whichkey`, `roothints`, `cmdline`, `config`, `visual`, `play`, `enter`,
`kiai`, `menu`, `dyn`, `simple`, `mixer`, `pessoas`, `kit`, `kit-new`, `esteira`, `lane`,
`partitura`, e os de teste:

- `probe` — simula mouse e teclado pelo caminho real (foco, keymap, hitboxes) e imprime o efeito
  de cada passo ([testes e depuração](/desenvolvimento/testes-e-depuracao.md));
- `drag` — anima um arrasto de notas e sai;
- `playstat-grade`, `playstat-esteira` — tocam 6 s e imprimem frames e tempo de render
  ([desempenho](/desenvolvimento/desempenho.md)).

> `--selftest` e os demos que editam (`probe`, `drag`, `kit-new`) gravam no banco: use sempre
> `--db` apontando para uma **cópia**.

# `wadawko-cli`

```
wadawko-cli [--db CAMINHO] <comando> [argumentos]
```

| Comando | O que faz |
|---|---|
| `list` | lista as músicas do banco (id, título, data, BPM, linhas, compassos) |
| `export <título\|id> [saída]` | grava uma música num arquivo `.wadawko.json` |
| `render <título\|id\|arquivo> <saída.wav\|mp3> [opções]` | renderiza em WAV ou MP3 (mesmo motor do playback) |
| `import <arquivo> [--open]` | importa um arquivo de música como música nova |
| `import-backup <arquivo> [--overwrite]` | importa um backup do `localStorage` do app web |

O banco padrão da CLI é `$WADAWKO_DB`, senão `$XDG_DATA_HOME/wadawko/wadawko.db`.
`wadawko-cli <comando> --help` mostra as opções de cada comando. Há também o binário
`wadawko-render` (crate `wadawko-export`) para render direto de arquivo.
