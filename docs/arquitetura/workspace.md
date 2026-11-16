---
type: Architecture
title: "Workspace de crates"
description: "Os nove crates do app nativo, o que cada um faz, quem depende de quem e onde ficam os testes."
tags: [arquitetura, crates, rust]
status: stable
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-16T13:10:00Z
fontes: [Cargo.toml, crates/*/Cargo.toml]
---

# Workspace de crates

O app nativo é um workspace Cargo (`Cargo.toml` na raiz, `crates/*`). A divisão segue o
[plano do porte](/planejamento/gpui.md) (§3.1): tudo que não é janela fica fora do app e é
testável sem display.

| Crate | Responsabilidade | Depende de |
|---|---|---|
| `wadawko-core` | Modelo `Song` (serde, round-trip do JSON do web), vocabulário, parser de kuchi shōga, timeline (ordem de toque, dinâmicas), kiai e **todas as operações de edição** como funções puras | — |
| `wadawko-store` | Persistência: SQLite `kv`, workspace/projetos, autosave em segundo plano, histórico (desfazer), migração para XDG, import de backup | core |
| `wadawko-samples` | Os `.ogg` embutidos (`public/samples/`), decodificados com symphonia; créditos | — |
| `wadawko-audio` | Motor em tempo real (cpal + thread RT sem alocação): sampler com camadas/round robin, EQ, choke, mixer por linha e pessoa, metrônomo, kiai, transporte (loop, região, count-in, treino), render offline | core, samples |
| `wadawko-export` | WAV (16/24 bits) e MP3 (via `ffmpeg`), limiter; binário `wadawko-render` | core, samples, audio |
| `wadawko-draw` | Desenho 2D independente de backend (`Canvas2D`): SVG, tiny-skia, gravação; métricas SMuFL/Bravura | — |
| `wadawko-notation` | Engraving da partitura de taiko sobre `Canvas2D` | core, draw |
| `wadawko-cli` | Linha de comando: list, export, render, import, import-backup ([referência](/referencia/linha-de-comando.md)) | core, store, samples, audio, export |
| `wadawko-app` | O app desktop (GPUI): shell, grade, partitura, esteira, Pessoas, Mixer, Kit, teclado ([arquitetura do app](/arquitetura/app.md)) | todos menos cli |

# Fluxo de uma edição

1. A UI chama `AppState::edit(cx, |song| ops::…)` — sempre uma função de `wadawko_core::ops`.
2. O estado anterior vai para o histórico (`wadawko-store`), a música nova para o autosave.
3. O `wadawko-audio` recompila o schedule (debounce de 30 ms) e troca no thread de áudio sem
   parar o som.
4. As views observam o estado e se redesenham (grade em cache — ver [desempenho](/desenvolvimento/desempenho.md)).

# Testes

| Onde | O que cobre |
|---|---|
| `crates/wadawko-core/tests/` | operações, parser, timeline; round-trip byte a byte contra as fixtures do app web; golden contra o TS |
| `crates/wadawko-audio/tests/` | schedule (ordem, ritornello, região, swing, mudo/solo, camadas), motor, render offline e **caminho de tempo real sem alocação** |
| `crates/wadawko-export/tests/`, `wadawko-samples/tests/` | WAV/MP3 e banco de samples |
| `crates/wadawko-store/tests/` | banco, autosave, migração, backup (algumas usam `data/`) |
| `crates/wadawko-cli/tests/` | comandos de ponta a ponta |
| `crates/wadawko-notation/tests/`, `wadawko-draw/tests/` | engraving e backends de desenho |
| `crates/wadawko-app` (`#[cfg(test)]`) | lógica pura das views (seleção, arrasto, física do balanço, keymap sem conflitos, títulos de todos os atalhos) |
| `wadawko --selftest`, `--demo probe` | a janela de verdade ([testes e depuração](/desenvolvimento/testes-e-depuracao.md)) |

`cargo test --workspace` roda tudo que não precisa de display.
