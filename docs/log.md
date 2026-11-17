# Histórico da documentação

## 2026-11-17

**Criação** do plano [design estilo DAW e features de DAW](/planejamento/daw.md): pesquisa da
linguagem visual do Bitwig (e Live, Renoise, Reaper, FL) e de UX/features de DAWs e notadores de
percussão; sistema de design escuro de cantos retos (tokens, componentes, layout com transporte
central, nav rail estreita, inspector); backlog de features priorizado com o corte para
implementar agora (cor por trilha, loop por trecho na régua, medidores de nível) e plano de
implementação em etapas verificáveis.

**Atualização**: licença (MIT OR Apache-2.0) em créditos; repositório sem app web e sem
`spike/` (visão geral, instalar e compilar, desempenho, arquitetura); importar músicas de outro
banco ou backup pela Config (dados e arquivos, instalar e compilar); AppImage, Flatpak e ícone
(instalar e compilar); `cargo fmt`/clippy no fluxo de contribuição; quadro do
[plano do repositório](/planejamento/repo.md) atualizado.

## 2026-11-16

**Atualização**: fixtures de teste reduzidas a duas músicas — "yagyo" e "Sample music" —, e
capturas de tela refeitas só com elas.

**Atualização**: capturas de tela em `imagens/` (usadas no README), pacote de release e CI
documentados em [instalar e compilar](/guias/instalar-e-compilar.md) e
[contribuir](/desenvolvimento/contribuir.md); quadro de andamento no
[plano do repositório](/planejamento/repo.md).

**Criação** do bundle OKF v0.2 em `docs/`: os planos (`PLANEJAMENTO-*.md`) foram para
`planejamento/` e a arquitetura do app (`crates/wadawko-app/ARQUITETURA.md`) para
`arquitetura/app.md`, todos com frontmatter; caminhos absolutos trocados por caminhos do repo.
Novos: visão geral, guias (instalar e compilar, uso), referência (atalhos — gerada —, linha de
comando, vocabulário, formato da música, dados e arquivos), arquitetura do workspace,
desenvolvimento (testes e depuração, desempenho, contribuir) e créditos. Ferramentas:
`tools/okf-check.py` (conformidade + índices) e `tools/gen-atalhos.py`.

## 2026-11-14

**Atualização** do `ARQUITETURA.md` (hoje `arquitetura/app.md`): moldura em cache e regras de
desempenho.

## 2026-11-10

**Criação** dos planos do app nativo (`planejamento/gpui.md`) e da UX por teclado
(`planejamento/teclado.md`).

## 2026-07-04

**Criação** do plano de redesign do app web (`planejamento/redesign-web.md`) e, em seguida, dos
planos de instrumentos e pessoas.

## 2026-07-02

**Criação** do planejamento inicial (`planejamento/inicial.md`).
