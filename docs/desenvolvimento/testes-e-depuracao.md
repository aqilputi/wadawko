---
type: Playbook
title: "Testes e depuração"
description: "Como rodar os testes, testar a janela de verdade com --selftest e --demo probe sem tocar no banco real, e as armadilhas de testar input sintético."
tags: [desenvolvimento, testes, depuração, probe]
status: stable
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-16T13:10:00Z
---

# Testes e depuração

# Steps

1. **Testes sem janela** — rápidos, rodam em qualquer lugar:
   ```sh
   cargo test --workspace
   ```
   O que cada suíte cobre: [workspace de crates](/arquitetura/workspace.md#testes).
2. **Sempre numa cópia do banco** antes de abrir a janela em teste:
   ```sh
   cp ~/.local/share/wadawko/wadawko.db /tmp/teste.db
   ```
   `--selftest` e os demos `probe`, `drag`, `kit-new` **gravam**.
3. **Autoteste da janela** — abre, pinta, toca 2 s, edita, desfaz/refaz, grava e sai:
   ```sh
   wadawko --db /tmp/teste.db --mute --selftest
   ```
   Saída esperada: linhas `TESTE música`, `TESTE áudio: pronto=true`, `TESTE frames tocando
   … (≈60 fps)`, `TESTE edição`, `TESTE gravação … erro=None`.
4. **Probe de interação** — simula mouse e teclado pelo caminho real do GPUI (`dispatch_event`
   / `dispatch_keystroke`: hit-test, foco, keymap) e imprime o efeito de cada passo:
   ```sh
   wadawko --db /tmp/teste.db --mute --demo probe | grep PROBE
   ```
   Cobre: clique e digitação, Tab, Ctrl+C, arrastar para selecionar, pegar/arrastar notas
   (com e sem Alt, Esc, soltar fora), seleção de seções (clique e Ctrl+clique), barra da seção
   apertada, barra de cima em cache, Ctrl+Space, rolagem com cabeçalho fixo e a paleta.
   Ao mudar interação, **acrescente um passo no `run_probe` de `crates/wadawko-app/src/main.rs`**.
5. **Sem tela** (não abre janela na sua tela; útil em reunião ou na CI): `tools/headless.sh`
   sobe um weston headless com Xwayland e roda o app nele, com o `settings.json` numa pasta
   temporária:
   ```sh
   tools/headless.sh --db /tmp/teste.db --mute --demo probe | grep PROBE
   tools/headless.sh --db /tmp/teste.db --mute --demo hoverstat
   ```
   Renderiza em software (~10 fps): bom para comportamento e para comparar versões.
5. **Página de atalhos** — depois de mudar teclas ou títulos de comandos:
   ```sh
   python3 tools/gen-atalhos.py          # regenera docs/referencia/atalhos.md
   python3 tools/gen-atalhos.py --check  # falha se estiver desatualizada
   ```
6. **Documentação** — confira o bundle OKF: `python3 tools/okf-check.py docs`.

# Armadilhas do input sintético

- **Ctrl+C sintético não chega na área de transferência do Wayland**: o GPUI só assume o
  clipboard com o *serial* de um evento real. No probe, julgue a cópia pelo aviso "copiado"
  e pelos registradores internos; para o sistema, teste à mão.
- **Coordenadas mudam com o layout**: trocar de modo, mover o cursor (a grade rola para o
  lado) ou rolar a página muda onde as células estão — recalcule com `cell_bounds` depois de
  cada mudança e deixe passos que rolam a grade para o fim do probe.
- **Só o que está na tela recebe evento**: célula fora da janela não tem hitbox.
- **Cliques precisam de um frame entre apertar e soltar** em alguns elementos — use uma espera
  curta (`wait(cx, 60)`) entre `MouseDown` e `MouseUp`.
- **Registro de alvos (Ctrl+Space) guarda posições antigas** de botões que sumiram: limpe
  `keys::HINTS` e force um redesenho antes de checar posições.
- **Janela escondida** (outro workspace) recebe poucos frames do compositor: animações e
  medições ficam lentas — não é travamento.
