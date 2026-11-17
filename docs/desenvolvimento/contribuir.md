---
type: Playbook
title: "Contribuir"
description: "Fluxo de trabalho no repositório: branches, como compilar com cache, o que conferir antes de um commit e como manter esta documentação."
tags: [desenvolvimento, contribuição, git, documentação]
status: stable
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-16T13:10:00Z
---

# Contribuir

# Steps

1. **Branch**: trabalhe num branch a partir do `main` e abra um PR para o `main`
   (`github.com/aqilputi/wadawko`). O app web antigo fica na tag `web-final` (e no branch `web-legacy`) e não recebe mudanças.
2. **Compilar**: `cargo build -p wadawko-app` (dev já otimiza as dependências). Em outro
   checkout, reaproveite o cache com `CARGO_TARGET_DIR` — leia antes as
   [armadilhas de build](/desenvolvimento/desempenho.md#armadilhas-de-build).
3. **Onde mexer**: lógica de música vai em `wadawko-core::ops` (função pura + teste); a UI só
   chama `AppState::edit`. Página/aba nova, comando novo, atalho novo: siga a
   [arquitetura do app](/arquitetura/app.md).
4. **Antes do commit**:
   - `cargo fmt --all`, `cargo test --workspace` e `cargo clippy --workspace --all-targets`
     (a CI recusa código fora do `rustfmt.toml` e avisos do clippy);
   - mudou interação? rode o probe ([testes e depuração](/desenvolvimento/testes-e-depuracao.md))
     e acrescente um passo para o caso novo;
   - mudou teclas ou títulos de comandos? `python3 tools/gen-atalhos.py`;
   - mexeu em desenho/animação? confira as [regras de desempenho](/desenvolvimento/desempenho.md);
   - mexeu em `docs/`? `python3 tools/okf-check.py docs`.
5. **Issues e PRs**: há modelos em `.github/` (bug, ideia e PR, com o checklist acima).
6. **Mensagens de commit** em português, título curto + corpo explicando o porquê (veja o
   `git log`).
7. **CI** (`.github/workflows/ci.yml`): todo push/PR roda a checagem de `docs/` e dos atalhos,
   `cargo test --workspace --release`, o build release e gera o pacote como artefato da
   execução (aba Actions). A primeira execução sem cache leva bem mais (o GPUI é grande).
8. **Release**: atualize o `CHANGELOG.md` (seção `## [x.y.z] - data`), suba a versão no
   `Cargo.toml` do workspace e envie a tag — o `release.yml` testa, empacota e publica:
   ```sh
   git tag v0.2.0 && git push origin v0.2.0
   ```

# Documentação

`docs/` é um bundle **[OKF v0.2](https://github.com/GoogleCloudPlatform/open-knowledge-format/blob/main/SPEC.md)**
(Open Knowledge Format): markdown com frontmatter YAML, legível por pessoas e por agentes.

- Todo `.md` (exceto `index.md` e `log.md`) começa com frontmatter com **`type`** (obrigatório),
  `title`, `description` (uma frase), `tags` e, quando fizer sentido, `status`
  (`draft`/`stable`/`deprecated`). Texto gerado por ferramenta leva `generated: {by, at}`.
- Tipos usados aqui: `Overview`, `User Guide`, `Playbook`, `Reference`, `Schema`,
  `Architecture`, `Plan`.
- Nomes de arquivo em minúsculas, com hífen, em português (`dados-e-arquivos.md`); o ID de um
  documento é o caminho sem `.md` (`referencia/atalhos`).
- Links entre documentos: absolutos a partir de `docs/`, como `[atalhos](/referencia/atalhos.md)`.
- Cada pasta tem um `index.md` (lista `* [Título](arquivo.md) - descrição`); a raiz declara
  `okf_version: "0.2"`. Registre mudanças relevantes no `log.md`.
- `tools/okf-check.py docs` confere as regras de conformidade do spec (§11) e os links internos.
