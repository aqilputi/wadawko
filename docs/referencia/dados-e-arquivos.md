---
type: Reference
title: "Dados e arquivos"
description: "Onde o WaDAWko guarda o banco de músicas, as configurações e o keymap do usuário, e para onde vão os exports."
tags: [referência, dados, configuração, xdg]
status: stable
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-16T13:10:00Z
fontes: [crates/wadawko-store/src/db.rs, crates/wadawko-app/src/keys/settings.rs, crates/wadawko-app/src/keys/keymap.rs, crates/wadawko-app/src/shell/io.rs]
---

# Dados e arquivos

| O quê | Onde | Formato |
|---|---|---|
| Banco de músicas | `$XDG_DATA_HOME/wadawko/wadawko.db` (normalmente `~/.local/share/wadawko/wadawko.db`) | SQLite, tabela `kv` (o mesmo esquema do app web) |
| Configurações | `~/.config/wadawko/settings.json` | JSON: `vim`, `av_offset_ms`, metrônomo e count-in a cada volta |
| Keymap do usuário | `~/.config/wadawko/keymap.json` | JSON no formato do Zed, somado ao padrão |
| Exports | diálogo do sistema (portal); sem portal, `$XDG_DOWNLOAD_DIR` ou `~/Downloads` | `.taiko.json`, `.wav`, `.mp3`, `.svg`, `.png` |

# Banco

- Chaves: `wadawko:workspace` (lista de músicas e música atual), `wadawko:project:<id>` (uma por
  música — o JSON descrito em [formato da música](/referencia/formato-da-musica.md)),
  `wadawko:theme` (tema) e `wadawko:native:settings` (preferências só do app nativo).
- **Autosave** 400 ms depois da última edição; desfazer/refazer guardam até 100 passos (na memória).
- Sem banco, o app cria um com a música de exemplo. Para trazer músicas de outro banco
  (o `data/wadawko.db` do app web, ou o de outro PC) ou de um backup `.json`: **Config →
  importar músicas** (ou `Ctrl+K` "importar músicas") — junta sem sobrescrever nada.
- Backup: copie o arquivo `.db` com o app fechado (ou use `wadawko-cli export` por música).

# settings.json

```json
{
  "vim": false,
  "av_offset_ms": 0,
  "metro_pattern": "auto",
  "metro_subdiv": 1,
  "metro_gain": 1.0,
  "count_in_loop": false
}
```

- `vim` — liga o modo vim na grade. **Sem o arquivo, o padrão é `true`.**
- `av_offset_ms` — atraso extra do visual em relação ao som (positivo = luz mais tarde).
  Ajustável em Config → Sincronia.
- `metro_pattern` — acentos do metrônomo, um caractere por tempo: `A` forte, `B` fraco, `.`
  mudo (até 16; repete ciclicamente). `auto` = forte no tempo 1 e fraco nos demais.
- `metro_subdiv` — cliques por tempo: 1, 2 (colcheias), 3 (tercinas) ou 4 (semicolcheias).
- `metro_gain` — volume do metrônomo (0 a 2; 1 = padrão).
- `count_in_loop` — um compasso de contagem antes de cada volta do loop.
- Campos ausentes ficam no padrão; o app reescreve o arquivo inteiro ao mudar qualquer um.

# keymap.json

Lista de blocos `{ "context": …, "bindings": { "tecla": "comando" } }`, igual ao
[keymap padrão](https://github.com/aqilputi/wadawko/blob/main/crates/wadawko-app/assets/keymap-default.json).
Crie um esqueleto pela paleta ("Criar keymap do usuário") e recarregue com "Recarregar keymap".
Conflitos aparecem em Config. Comandos e teclas atuais: [atalhos](/referencia/atalhos.md).
