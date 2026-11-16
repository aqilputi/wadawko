# WaDAWko

Editor de partituras e "DAW" para grupos de **taiko**, escrito em **kuchi shōga** — as sílabas
faladas dos tambores (`don`, `doko`, `ka`, `tsuku`…). Escreva numa grade por instrumento, ouça
com samples reais, veja a partitura e ensaie com a esteira de notas.

App desktop nativo em **Rust + [GPUI](https://www.gpui.rs/)** (Linux). O app web original
(Svelte) está no branch `web-legacy`.

```sh
cargo build --release -p wadawko-app
./spike/target/release/wadawko --migrate-from data/wadawko.db   # primeira vez, se vier do app web
```

## Documentação

Tudo está em [`docs/`](docs/index.md) — um bundle
[OKF v0.2](https://github.com/GoogleCloudPlatform/open-knowledge-format) (markdown com
frontmatter, legível por pessoas e agentes):

- [Visão geral](docs/visao-geral.md)
- [Instalar e compilar](docs/guias/instalar-e-compilar.md) · [Guia de uso](docs/guias/uso.md)
- [Atalhos](docs/referencia/atalhos.md) · [Linha de comando](docs/referencia/linha-de-comando.md) ·
  [Vocabulário](docs/referencia/vocabulario.md) · [Formato da música](docs/referencia/formato-da-musica.md)
- [Arquitetura](docs/arquitetura/workspace.md) · [Contribuir](docs/desenvolvimento/contribuir.md) ·
  [Testes](docs/desenvolvimento/testes-e-depuracao.md) · [Desempenho](docs/desenvolvimento/desempenho.md)
- [Planos e roadmap](docs/planejamento/index.md) — o que falta: [repositório completo](docs/planejamento/repo.md)

## Créditos

Samples de taiko: SCC Taiko Drums (CC BY-SA 4.0), freesound (CC BY 3.0 / CC0), animalese.js
(CC BY 4.0). Fontes: Zen Kaku Gothic New, Shippori Mincho e Bravura (SIL OFL). Detalhes em
[créditos](docs/creditos.md). A licença do código ainda será definida.
