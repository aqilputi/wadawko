---
type: Playbook
title: "Instalar e compilar"
description: "Dependências de sistema, como compilar o app e a CLI, instalar o atalho no menu e migrar o banco do app web."
tags: [guia, build, instalação, linux]
status: stable
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-16T13:10:00Z
---

# Instalar e compilar

# Pacote pronto (Linux x86_64)

Cada versão publica um `.tar.gz` em
[Releases](https://github.com/aqilputi/wadawko/releases) (gerado pela CI com `tools/package.sh`):

```sh
tar xzf wadawko-*-linux-x86_64.tar.gz
cd wadawko-*-linux-x86_64 && ./install.sh      # binários em ~/.local/bin + atalho no menu
```

O pacote é compilado no Ubuntu 24.04: precisa de **glibc ≥ 2.39**. AppImage/Flatpak estão na
[fase E do plano](/planejamento/repo.md). Para compilar do código, siga abaixo.

# Requisitos

- **Linux** com Wayland ou X11 (desenvolvido em CachyOS + niri/Wayland). macOS/Windows: não testados.
- **GPU com Vulkan** (Mesa serve — Intel Iris Xe é a máquina de referência).
- **Áudio**: ALSA/PipeWire (via `cpal`).
- **Rust** estável recente (edition 2024) — `rustup`.
- Pacotes de compilação: `cmake`, `pkg-config`, headers de `alsa-lib`, `libxkbcommon`,
  `wayland`, `fontconfig`/`freetype` (os nomes variam por distro).
- **`ffmpeg`** no `PATH`, só para exportar **MP3** (WAV não precisa).

# Compilar

```sh
git clone https://github.com/aqilputi/wadawko.git
cd wadawko
cargo build --release -p wadawko-app      # o app (binário `wadawko`)
cargo build --release -p wadawko-cli      # a linha de comando (opcional)
```

A **primeira** compilação demora (~20 min numa máquina de notebook: o GPUI é grande); as
seguintes levam de segundos a ~2 min. O binário fica em `target/release/wadawko`.

> Trabalhando em outro checkout (worktree) do mesmo repositório, reaproveite o cache com
> `CARGO_TARGET_DIR=<repo>/target` — e leia as armadilhas em
> [desempenho](/desenvolvimento/desempenho.md#armadilhas-de-build).

# Instalar para uso diário

```sh
ln -sfn "$PWD/target/release/wadawko" ~/.local/bin/wadawko
```

Entrada no menu de aplicativos (`~/.local/share/applications/wadawko.desktop`):

```ini
[Desktop Entry]
Type=Application
Name=WaDAWko
Comment=Editor de partituras de taiko
Exec=wadawko
Icon=audio-x-generic
Terminal=false
Categories=AudioVideo;Audio;Music;
```

Com o link simbólico, cada `cargo build --release` atualiza o app instalado (feche e abra).

# Primeira execução e migração do banco

O banco fica em `~/.local/share/wadawko/wadawko.db` ([dados e arquivos](/referencia/dados-e-arquivos.md)).
Para trazer as músicas do app web (o banco `data/wadawko.db` de um checkout antigo):

```sh
wadawko --migrate-from <checkout-antigo>/data/wadawko.db --read-only
```

A origem é só lida (cópia consolidada com `VACUUM INTO`); depois disso abra normalmente com
`wadawko`. Um backup do `localStorage` do navegador entra pela CLI:
`wadawko-cli import-backup arquivo.json` ([linha de comando](/referencia/linha-de-comando.md)).

# Conferir

```sh
wadawko --db /tmp/copia.db --mute --selftest
```

abre a janela, toca, edita, desfaz/refaz e grava, imprimindo linhas `TESTE …` — sempre numa
**cópia** do banco (ver [testes e depuração](/desenvolvimento/testes-e-depuracao.md)).
