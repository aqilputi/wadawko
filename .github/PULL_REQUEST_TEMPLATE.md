## O que muda

<!-- Resumo em poucas linhas e o porquê. Link para a issue, se houver: "Fecha #123". -->

## Como foi testado

- [ ] `cargo fmt --all`, `cargo test --workspace` e `cargo clippy --workspace --all-targets` sem avisos
- [ ] mudou interação: `wadawko --demo probe --db <cópia do banco>` (com um passo novo para o caso)
- [ ] mudou teclas ou títulos de comandos: `python3 tools/gen-atalhos.py`
- [ ] mexeu em `docs/`: `python3 tools/okf-check.py docs`
- [ ] mexeu em desenho/animação: conferi as regras de desempenho (`docs/desenvolvimento/desempenho.md`)

## Checklist

- [ ] `CHANGELOG.md` atualizado (seção `[Não lançado]`), se o usuário percebe a mudança
- [ ] documentação de uso/formato atualizada, se for o caso
- [ ] o JSON da música continua compatível (campos novos opcionais; músicas antigas abrem igual)
