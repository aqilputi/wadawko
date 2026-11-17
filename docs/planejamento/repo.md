---
type: Plan
title: "Repositório completo"
description: "Fases para deixar o repositório completo: consolidar, limpar a raiz, CI, documentação, distribuição e backlog."
tags: [planejamento, repositório, ci, distribuição]
status: draft
estado: "em andamento"
---

# WaDAWko — Planejamento para deixar o repositório completo

Data: 2026-11-16 · Status: planejamento · Repositório: `github.com/aqilputi/wadawko` (privado)

"Completo" aqui = **alguém clona, compila, roda e contribui sem ajuda; o app sai
empacotado em releases; o código web antigo fica arquivado; licenças em ordem; CI
verde a cada PR; documentação de uso e de arquitetura; backlog organizado.**

---

## Andamento (atualizado em 2026-11-17)

| Fase | Situação |
|---|---|
| A — Consolidar | **feita**: branch de trabalho juntado no `main` (fast-forward), `main` padrão no GitHub, release `v0.1.0` |
| B — Limpar a raiz | **feita**: web e `spike/` fora do `main` (tag `web-final`), samples em `assets/samples/`, fixtures no core, `target/` padrão |
| C — Qualidade e CI | quase: avisos e clippy zerados, `cargo fmt` (+ `.git-blame-ignore-revs`), fmt e clippy na CI; `--demo probe` com verificações e código de saída; falta rodar o probe na CI (compositor headless) |
| D — Documentação | **feita**: README com capturas, `docs/` OKF, `CHANGELOG.md`, templates de issue/PR, `LICENSE-MIT`/`LICENSE-APACHE`/`NOTICE` |
| E — Distribuição | quase: ícone próprio, `.desktop`/metainfo com o ID `io.github.aqilputi.WaDAWko`, AppImage (`tools/appimage.sh`) no release, manifesto Flatpak para build local; falta publicar no Flathub (opcional) |

Decisões tomadas em 2026-11-16: D1 MIT OR Apache-2.0 · D2 repositório público · D3
`io.github.aqilputi.WaDAWko` · D4 web fora do `main` (tag `web-final`) · D5 `spike/` apagado ·
D6 fixtures reduzidas a duas músicas.

## 0. Onde estamos (2026-11-16, início do dia)

- **App nativo (Rust + GPUI) em uso diário** desde 13/11: workspace `crates/wadawko-*`
  (core, store, samples, audio, export, draw, notation, cli, app). Grade, partitura,
  esteira, Pessoas, Mixer, Kit e Config portados. Banco em `~/.local/share/wadawko/`.
- **Testes:** ~74 unitários no app + suítes por crate (core: round-trip JSON com as
  fixtures; audio: render com hash estável; store; cli). `--selftest` e `--demo probe`
  exercitam a janela de verdade (precisam de display).
- **Repositório:** `main` (estado inicial, 13/11), `web-legacy` (mesmo ponto, guarda o
  app Svelte), branch de trabalho `worktree-fix-topbar-sections-shortcuts` com 5 commits
  ainda **não juntados no `main`**.
- **Ainda misturado na raiz:** app web (`src/`, `server/`, `index.html`, `package*.json`,
  `vite/svelte/tsconfig`, `node_modules/` ignorado), `spike/` (protótipo GPUI — e o
  `target/` compartilhado mora nele), `samples-antigos/`, `scripts/`, `f2-new.png`.
  (Os planos e a arquitetura já foram para `docs/` — bundle OKF v0.2, 2026-11-16.)
- **Faltam:** `LICENSE`, CI, empacotamento, ícone, CHANGELOG, templates de
  issue/PR. Avisos de compilação: 2 de código morto + API depreciada na partitura.

---

## 1. Decisões que precisam do Fernando (bloqueiam partes do plano)

| # | Decisão | Opções | Recomendação |
|---|---|---|---|
| D1 | Licença do código | MIT/Apache-2.0 (dupla) · GPL-3.0 · proprietária | **MIT OR Apache-2.0** (padrão Rust; samples e fontes ficam com as licenças deles) |
| D2 | Repo público ou privado | manter privado · abrir | Privado até a Fase B; abrir quando README/licença/limpeza de dados pessoais estiverem prontos |
| D3 | ID do app (Flatpak/desktop) | `io.github.aqilputi.WaDAWko` · outro domínio | `io.github.aqilputi.WaDAWko` (não exige domínio próprio) |
| D4 | O que fazer com o app web | apagar do `main` (fica no `web-legacy`) · manter | **Apagar do `main`** + tag `web-final` |
| D5 | `spike/` | apagar · mover para `archive/` | Apagar (as conclusões estão no `/planejamento/gpui.md` §8.0) |
| D6 | Músicas pessoais no repo (`data/backup-localstorage-*.json`) | manter como fixture · trocar por fixtures sintéticas | Manter enquanto privado; antes de abrir, trocar por fixtures geradas |

---

## 2. Fases

### Fase A — Consolidar (1 dia) · *pré-requisito de tudo*
1. Juntar o branch de trabalho no `main` (`git merge worktree-fix-topbar-sections-shortcuts`) e
   publicar o `main`. Proteger o `main` no GitHub (PR obrigatório, CI obrigatório quando existir).
2. Tag `v0.1.0-alpha` no `main` = "primeira versão nativa usável".
3. Apagar o branch de trabalho depois do merge.

**Pronto quando:** `main` no GitHub contém todo o trabalho; app instalado é compilado do `main`.

### Fase B — Limpar a raiz (1–2 dias) · depende de D4, D5
1. **Web:** `git tag web-final web-legacy`; remover do `main`: `src/`, `server/`, `index.html`,
   `package.json`, `package-lock.json`, `vite.config.ts`, `svelte.config.js`, `tsconfig.json`,
   `dist/`, `f2-new.png`, `samples-antigos/`, `scripts/` (revisar antes — algum script pode
   servir ao nativo).
2. **Samples:** mover `public/samples/` → `assets/samples/` e ajustar os `include_bytes!`/
   `include_str!` de `crates/wadawko-samples/src/embedded.rs` (hoje apontam para `public/`).
3. **Spike:** apagar `spike/` e trocar `.cargo/config.toml` para `target-dir = "target"`
   (custa **uma** build fria de ~20 min). Atualizar `~/.local/bin/wadawko` para o novo caminho.
4. ~~**Docs:** planos e arquitetura para `docs/`~~ — **feito em 2026-11-16**: `docs/` é um bundle
   [OKF v0.2](https://github.com/GoogleCloudPlatform/open-knowledge-format) (ver `/index.md`).
5. ~~Trocar caminhos absolutos nos planos por caminhos do repo~~ — **feito em 2026-11-16**.
6. Fixtures: `data/backup-localstorage-*.json` → `crates/wadawko-core/tests/fixtures/`
   (ajustar `store`/`cli`/`core` tests e `golden/gen.mjs`); `data/` some do repo.

**Pronto quando:** a raiz tem só `crates/`, `assets/`, `docs/`, `Cargo.*`, `.cargo/`, arquivos
de projeto; `cargo test --workspace` passa num clone limpo.

### Fase C — Qualidade e CI (2–3 dias)
1. ~~Zerar avisos~~ — feito em 2026-11-16: código morto (`people::owner_count`, `score/surface.rs` `clear/len`),
   `ScrollWheelEvent`/`on_scroll_wheel` → `ScrollEvent`/`on_scroll` (partitura).
2. `rustfmt.toml` com o estilo atual (linhas longas, `max_width ≈ 200`) + `cargo fmt` uma vez
   (depois dos PRs abertos, para não gerar conflito). ~~`cargo clippy --workspace -D warnings`~~ —
   feito em 2026-11-16: zerado e na CI (`build.yml`).
3. **GitHub Actions** (`.github/workflows/ci.yml`), Linux:
   - `fmt` + `clippy` + `cargo test --workspace` (com `Swatinem/rust-cache` — sem cache a
     build do gpui passa de 20 min);
   - testes que precisam de janela (`--selftest`, `--demo probe`) num job separado com
     compositor headless (`weston --backend=headless` ou `sway` headless) + Mesa **lavapipe**
     (Vulkan por software). Marcar como "pode falhar" até estabilizar.
   - build de release como artefato.
   - macOS/Windows: job manual (`workflow_dispatch`), melhor esforço.
4. Transformar o `--demo probe` em teste de regressão: sair com código ≠ 0 quando uma
   checagem falhar (hoje só imprime), para o job de GUI acusar.

**Pronto quando:** todo PR roda CI; `main` sempre verde.

### Fase D — Documentação · **quase toda feita em 2026-11-16** · falta o que depende de D1
`docs/` virou um bundle OKF v0.2 com visão geral, guias, referência, arquitetura e
desenvolvimento (ver `/index.md` e `/log.md`); `tools/okf-check.py` confere a conformidade e
gera os índices.
1. ~~`README.md`~~ — feito (raiz do repo, aponta para `docs/`). Falta screenshot/GIF.
2. `LICENSE-MIT` + `LICENSE-APACHE` (D1); `NOTICE` com fontes (OFL) e samples (`CREDITS.md`,
   CC BY / CC BY-SA); `license` nos `Cargo.toml`. **(pendente — decisão D1)**; os créditos já
   estão em `/creditos.md`.
3. ~~Guia de uso e tabela de atalhos gerada~~ — feito: `/guias/uso.md` e `/referencia/atalhos.md`,
   gerada do keymap + registro por `tools/gen-atalhos.py` (`--check` para a CI).
4. ~~Guia de contribuição~~ — feito: `/desenvolvimento/contribuir.md` (+ testes e desempenho).
   Falta `CHANGELOG.md` (Keep a Changelog).
5. ~~Templates~~ — feito: `.github/ISSUE_TEMPLATE/{bug,feature}.md`, `PULL_REQUEST_TEMPLATE.md`.
6. Na CI (fase C): `python3 tools/okf-check.py docs` e `python3 tools/gen-atalhos.py --check`.

### Fase E — Distribuição (Fase 7 do `/planejamento/gpui.md`) (3–5 dias) · depende de D3
1. Ícone próprio (SVG + PNGs 48–512) no lugar do `audio-x-generic`; `.desktop` e
   AppStream `metainfo.xml` no repo (`assets/linux/`).
2. **AppImage** (linuxdeploy) — o binário já embute samples e fontes; checar Vulkan/Mesa e
   PipeWire numa instalação limpa (critério do plano original).
3. **Flatpak** (`io.github.aqilputi.WaDAWko.yml`, runtime freedesktop, permissões: áudio
   PipeWire, `xdg-download`, portal de arquivos) — publicar no Flathub é opcional.
4. Workflow de release: tag `v*` → build → AppImage + Flatpak bundle + tarball como assets
   do GitHub Release, com o trecho do CHANGELOG.
5. Primeira execução: migrar automaticamente um banco legado (hoje só com `--migrate-from`)
   e mostrar os créditos dos samples (obrigação CC BY-SA) em lugar visível.

**Pronto quando:** baixar o AppImage de um Release numa máquina limpa e tocar uma música.

### Fase F — Backlog do produto (contínuo)
Abrir como **issues** com milestones, em vez de listas em markdown:
- **Verificações manuais** que faltam: fps em X11, sincronia som × imagem a olho, 10 min tocando
  sem xrun, Ctrl+C para fora do app (área de transferência do Wayland).
- **Desempenho:** barra de contexto e status bar ainda redesenham a cada frame (~4,7 ms/frame
  tocando); grade re-renderizada em hover de botões (~2.500 elementos) — virtualizar por seção.
- **Fase 6 (bonecos)** — adiada; rig, estações, lane de movimentos, export de vídeo.
- Itens "só planejados" do `/planejamento/gpui.md` §1.2: vocabulário editável, dinâmica por
  célula, export MIDI/TJA/PDF, MIDI input, i18n, compartilhar por URL.
- Organização: labels (`bug`, `ux`, `perf`, `áudio`, `notação`, `distribuição`), milestones
  `v0.2 — distribuível` (Fases B–E) e `v1.0 — completo` (paridade + bonecos + docs).

---

## 3. Ordem sugerida e esforço

| Ordem | Fase | Esforço | Depende de |
|---|---|---|---|
| 1 | A — Consolidar | 1 dia | — |
| 2 | B — Limpar a raiz | 1–2 dias | D4, D5 |
| 3 | C — Qualidade e CI | 2–3 dias | B |
| 4 | D — Documentação | 1–2 dias | D1 (e B para os caminhos) |
| 5 | E — Distribuição | 3–5 dias | C (CI), D3 |
| — | F — Backlog | contínuo | A (issues) |

Total até "v0.2 distribuível": **~8–13 dias de trabalho**. Abrir o repositório (D2) faz
sentido ao fim da Fase D, depois de resolver D6.

## 4. Riscos

- **Build fria de ~20 min** (gpui): pesa na CI e ao apagar o `spike/` (target compartilhado).
  Mitigação: `rust-cache` na CI; trocar o `target-dir` uma vez só, num momento combinado.
- **Testes de GUI na CI** dependem de compositor headless + lavapipe; podem ser instáveis.
  Mitigação: job separado, não obrigatório até estabilizar; a lógica fica coberta por testes puros.
- **Samples CC BY-SA**: a share-alike vale para os samples, não para o código; precisa ficar
  explícito no README/NOTICE para não "contaminar" a escolha de licença.
- **Pin `gpui-kit =0.9.1`**: atualizar o gpui pode quebrar APIs (já mudaram `ScrollEvent` e
  `Tab` global). Atualizar em PR próprio, com o `--demo probe` como rede de segurança.
