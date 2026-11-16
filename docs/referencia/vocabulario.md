---
type: Reference
title: "Vocabulário kuchi shōga"
description: "As sílabas que a grade entende (golpe, zona e mão de cada uma), as regras de escrita e as frases e kiai prontos."
tags: [referência, kuchi-shoga, vocabulário, frases]
status: stable
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-16T13:10:00Z
fontes: [crates/wadawko-core/src/vocab.rs, crates/wadawko-core/src/phrase.rs, crates/wadawko-core/src/kyai.rs]
---

# Vocabulário kuchi shōga

Fonte da verdade: `crates/wadawko-core/src/vocab.rs` (se divergir desta página, vale o código).
Zona **couro** = pele (centro); **borda** = fuchi. Mão: D = direita, E = esquerda.
Sílabas de dois golpes dividem a célula ao meio (numa célula de colcheia viram semicolcheias).

# Sílabas

| Sílaba | Golpe | Zona | Mão |
|---|---|---|---|
| `DON` | batida forte no centro | couro | D |
| `don` | batida no centro | couro | D |
| `KON` | batida forte no centro | couro | E |
| `kon` | batida no centro | couro | E |
| `doron` | as duas mãos ao mesmo tempo (uníssono forte) | couro | D+E |
| `do` | batida curta | couro | D |
| `ko` | batida curta | couro | E |
| `doko` | duas batidas rápidas | couro | D, E |
| `tsu` | ghost note, quase silêncio | couro | D |
| `ku` | ghost note, quase silêncio | couro | E |
| `tsuku` | duas ghost | couro | D, E |
| `ten` | shime, batida | couro | D |
| `teke` | shime, duas rápidas | couro | D, E |
| `KA` | borda forte | borda | D |
| `ka` | borda (fuchi) | borda | D |
| `ra` | borda (fuchi) | borda | E |
| `kara` | duas na borda | borda | D, E |
| `karan` | borda com as duas mãos ao mesmo tempo | borda | D+E |
| `su` | pausa marcada | — | — |

# Regras de escrita

- **Compostas** se quebram em sílabas do vocabulário: `dokodoko` = `doko` + `doko`.
- **Alongamento** falado é absorvido e conta como duração: `dooon` casa com `don` e ocupa mais.
- **CAIXA ALTA** numa sílaba sem variante maiúscula = acento genérico.
- **`!` no fim** = sforzando (acento forte daquele golpe).
- Palavra que não casa com nada é recusada ("não é kuchi shōga conhecido").

# Frases prontas

Pela paleta (`Ctrl+K` → "Inserir frase: …"), escritas a partir do cursor.

| Frase | Sílabas | Observação |
|---|---|---|
| cavalinho | don doko don doko don doko don doko | galope: don no tempo, doko no contratempo (1 compasso) |
| dondoko | don doko don don | padrão dondoko clássico (1 compasso) |
| jiuchi dontsuku | DON tsuku DON tsuku | jiuchi de festival, matsuri-jiuchi (1 compasso) |
| tontsuton | don su tsu kon su ku don su tsu kon su ku don su don su | jiuchi tontsuton (2 compassos, meio-tempo por palavra) |
| matsuri | DON su KON su DON KARA KA RA don KON su kon DON KARA KA RA | tema do Matsuri (2 compassos, meio-tempo por palavra) |
| jiuchi shime | ten teke ten teke | jiuchi básico do shime (1 compasso) |
| rulo dokodoko | dokodoko dokodoko dokodoko dokodoko | rulo contínuo (1 compasso, 4 golpes por tempo) |

# Kiai prontos

Paleta → "Inserir kiai: …", na sub-linha de kiai da linha do cursor:
`sa!`, `sei!`, `seiya!`, `ha!`, `hai!`, `sore!`, `soriya!`, `hyia!`, `soryia!`,
`soresoresoresore!`, `soresoresoresore! soryia!`. Qualquer texto também vale — o
sintetizador silaba e canta.
