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

Cada versão publica em [Releases](https://github.com/aqilputi/wadawko/releases) (gerados pela CI):

- **AppImage** (`tools/appimage.sh`): um arquivo só, sem instalar —
  `chmod +x WaDAWko-*-x86_64.AppImage && ./WaDAWko-*-x86_64.AppImage`.
- **`.tar.gz`** (`tools/package.sh`): instala em `~/.local` com atalho e ícone no menu —
  ```sh
  tar xzf wadawko-*-linux-x86_64.tar.gz
  cd wadawko-*-linux-x86_64 && ./install.sh
  ```

Os dois são compilados no Ubuntu 24.04: precisam de **glibc ≥ 2.39**.

**Flatpak**: há um manifesto em `assets/linux/flatpak/io.github.aqilputi.WaDAWko.yml` para build
local (instruções no topo do arquivo; o build baixa os crates, então ainda não serve para o
Flathub). Para compilar do código, siga abaixo.

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

Atalho e ícone no menu de aplicativos (o ID do app é `io.github.aqilputi.WaDAWko`):

```sh
install -Dm644 assets/linux/io.github.aqilputi.WaDAWko.desktop -t ~/.local/share/applications/
cp -r assets/linux/icons/hicolor ~/.local/share/icons/
rm -f ~/.local/share/applications/wadawko.desktop      # atalho antigo, se houver
```

Com o link simbólico, cada `cargo build --release` atualiza o app instalado (feche e abra).

# Primeira execução e migração do banco

O banco fica em `~/.local/share/wadawko/wadawko.db` ([dados e arquivos](/referencia/dados-e-arquivos.md)).
Na primeira vez o app cria o banco com uma música de exemplo e avisa como trazer as suas.

Para trazer as músicas do app web (o banco `data/wadawko.db` de um checkout antigo), de outro
PC ou de um backup do `localStorage` (`.json`): **Config → importar músicas de outro banco ou
backup…** (ou `Ctrl+K` → "importar músicas"). A origem é só lida; músicas novas entram e as
que já existem ficam como estão.

Pela linha de comando, antes de abrir o app pela primeira vez, o banco inteiro também pode ser
copiado (`VACUUM INTO`, a origem é só lida):

```sh
wadawko --migrate-from <checkout-antigo>/data/wadawko.db
```

Ou um backup JSON: `wadawko-cli import-backup arquivo.json` ([linha de comando](/referencia/linha-de-comando.md)).

# Conferir

```sh
wadawko --db /tmp/copia.db --mute --selftest
```

abre a janela, toca, edita, desfaz/refaz e grava, imprimindo linhas `TESTE …` — sempre numa
**cópia** do banco (ver [testes e depuração](/desenvolvimento/testes-e-depuracao.md)).
