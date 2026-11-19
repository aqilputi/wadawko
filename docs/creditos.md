---
type: Reference
title: "Créditos e licenças de terceiros"
description: "De onde vêm os samples de taiko e as fontes embutidas no app, e as obrigações de cada licença (CC BY-SA, CC BY, CC0, OFL)."
tags: [créditos, licenças, samples, fontes]
status: stable
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-16T13:10:00Z
fontes: [assets/samples/CREDITS.md, assets/fonts/]
---

# Créditos e licenças de terceiros

O binário do app **embute** os samples e as fontes abaixo. As licenças deles valem para esses
arquivos — não para o código do WaDAWko, que é **MIT OR Apache-2.0** (`LICENSE-MIT`,
`LICENSE-APACHE` e o resumo em `NOTICE`, na raiz do repositório). O app mostra os créditos em **Config → Banco e
créditos**.

# Samples (`assets/samples/`)

Texto completo e o mapeamento arquivo a arquivo: `assets/samples/CREDITS.md` (é esse arquivo
que o app embute e exibe).

| Origem | Autor | Licença | Usado em |
|---|---|---|---|
| [SCC Taiko Drums](https://github.com/floe-audio/Taiko-Drums) | S. Christian Collins e Subaqueous | **CC BY-SA 4.0** | chū, nagadō, ō-nagadō, ōdaiko (couro e borda), borda do shime |
| [taikoset1_ste58](https://freesound.org/people/whatsanickname4u/sounds/127337/) | whatsanickname4u (Dave Thayer) | CC BY 3.0 | shime (couro), okedō |
| [Gongo pequeno de santuário](https://freesound.org/people/RutgerMuller/sounds/364847/) | RutgerMuller | CC0 | kane |
| [Freesound](https://freesound.org) (vários autores, lista em `assets/samples/CREDITS.md`) | WelvynZPorterSamples, Vilaa2404, Reitanna, Alivvie, dobroide e outros | CC0 / CC BY 3.0 / CC BY 4.0 | vozes dos kiai e da contagem falada (`kiai/`) |

- **CC BY-SA 4.0**: os samples derivados continuam CC BY-SA — quem redistribuir precisa dar
  crédito e manter a mesma licença **nesses arquivos**. A share-alike não se estende ao código.
- **CC BY 3.0 / 4.0**: exigem crédito ao autor.

# Fontes (`assets/fonts/`)

| Fonte | Uso | Licença |
|---|---|---|
| Inconsolata | interface | SIL OFL 1.1 (`OFL-Inconsolata.txt`) |
| Zen Kaku Gothic New | partitura e caracteres japoneses | SIL OFL 1.1 (`OFL-ZenKakuGothicNew.txt`) |
| Shippori Mincho | logo 太 e partitura | SIL OFL 1.1 (`OFL-ShipporiMincho.txt`) |
| Bravura (SMuFL) | símbolos da partitura | SIL OFL 1.1 (`OFL-Bravura.txt`, `FONTLOG-Bravura.txt`) |

A OFL permite embutir e redistribuir; os textos das licenças devem acompanhar as fontes (já
estão na pasta).

# Referências

- [Uzume Taiko — notas de notação](/material/) — PDF de referência usado no desenho da partitura.

## Inconsolata — SIL OFL 1.1

Fonte da interface (identidade visual de 2026-11-18): **Inconsolata**, de Raph Levien e The
Inconsolata Project Authors (https://github.com/googlefonts/Inconsolata), licença SIL Open Font
License 1.1 (`assets/fonts/OFL-Inconsolata.txt`). Pesos Regular, Medium, SemiBold e Bold.

## Ícones

Os ícones da interface (`crates/wadawko-app/assets/icons/*.svg`) foram desenhados para o
WaDAWko (traço de 2 px em 24×24) e seguem a licença do código (MIT OR Apache-2.0).

