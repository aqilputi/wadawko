---
type: Playbook
title: "Desempenho"
description: "Regras para não deixar a UI lenta (cache, redesenho, frames de animação), como medir em ms de CPU por frame e as armadilhas de build com cache compartilhado."
tags: [desenvolvimento, desempenho, gpui, medição]
status: stable
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-16T13:10:00Z
---

# Desempenho

Referência medida em 2026-11-14 (i5-1135G7 + Iris Xe, release, música "papparapa": 13 seções,
74 compassos, 5 linhas): **grade tocando 4,7 ms de CPU por frame a 60 fps; esteira 5–6 ms;
arrastar notas com o mouse parado ~4% de CPU.** Antes das otimizações: 7 ms/frame e quedas
para 55 fps.

# Regras

1. **A grade é uma view em cache** (`Entity::cached`). Só notifique a grade quando o desenho
   dela muda — cada re-render reconstrói ~2.500 elementos (1,5–2,5 ms + layout).
2. **Nunca use `window.refresh()` durante animação ou arrasto**: ele invalida *todos* os
   caches. Para redesenhar, notifique a entidade que mudou (`cx.notify()`).
3. **Peça `request_animation_frame` só enquanto algo anima de fato** (ex.: o bloco arrastado
   para quando assenta — `drag_ghost::Swing::settled`). Cada frame redesenha a raiz.
4. **A moldura (barra de cima, lateral, painel de músicas, barra de contexto e status bar) fica
   em cache** com uma chave por peça (`shell/chrome.rs`, `Shell::refresh_chrome`). Mostrou algo
   novo numa dessas peças? **Inclua na chave**, senão ela fica velha. A status bar é desenhada
   de um `StatusModel` (só texto, com `Hash`): tocando, ela redesenha quando a posição muda de
   célula, não a cada frame. A barra de contexto quebra linha (altura variável) e view em cache
   precisa de tamanho fixo: quando a chave muda (inclusive a largura da janela) ela é desenhada
   sem cache e medida; nos frames seguintes usa o cache com a altura medida.
5. **Nada de tesselar paths por frame** quando a forma se repete: pré-tessele e translade
   (pétalas da esteira, `lane.rs::paint_petal`).
6. **Evite hover em elementos grandes dentro da grade**: entrar/sair com o mouse notifica a
   view inteira.

Detalhes de cada regra: [arquitetura do app](/arquitetura/app.md#moldura-em-cache-e-animação-desempenho).

# Medir

Compare **ms de CPU por frame**, nunca CPU% puro — uma janela num workspace escondido recebe
menos frames do compositor e o CPU% varia 2× entre execuções.

```sh
cp ~/.local/share/wadawko/wadawko.db /tmp/bench.db
wadawko --db /tmp/bench.db --mute --demo playstat-grade      # ou playstat-esteira
# → STAT fim · 354 frames em 6.0 s (59 fps) · Shell::render média 0.34 ms
```

Some o CPU do processo no intervalo entre `STAT início` e `STAT fim` (amostrando
`/proc/<pid>/task/*/stat`) e divida pelos frames. `--demo drag` imprime frames por fase
(movendo / parado); `--bench N` dá histogramas de frame e paint da grade (só com a janela
visível).

# Armadilhas de build

- Dois checkouts do
  mesmo repo (ex.: um commit antigo num `git worktree` para comparar) compilando no **mesmo**
  `CARGO_TARGET_DIR` geram o **mesmo** artefato: o cargo pode "compilar" em 1 s e deixar o
  binário **do outro checkout** — inclusive o instalado em `~/.local/bin/wadawko`.
  Depois de builds assim: `touch crates/wadawko-app/src/main.rs`, recompile e confira
  (`strings target/release/wadawko | grep <texto novo>`).
- Build fria leva ~20 min (GPUI); `opt-level = 3` nas dependências já em dev.
