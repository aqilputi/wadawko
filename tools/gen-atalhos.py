#!/usr/bin/env python3
"""Gera docs/referencia/atalhos.md a partir do keymap padrão e dos títulos dos comandos.

Fontes (as mesmas que o app usa):
  crates/wadawko-app/assets/keymap-default.json   — teclas por contexto
  crates/wadawko-app/src/keys/registry.rs         — títulos (SPECS, title_for, PREFIX_TITLES)

uso: python3 tools/gen-atalhos.py            (escreve o arquivo)
     python3 tools/gen-atalhos.py --check    (falha se o arquivo estiver desatualizado)
"""
import json
import pathlib
import re
import sys
from datetime import datetime, timezone

ROOT = pathlib.Path(__file__).resolve().parent.parent
KEYMAP = ROOT / "crates/wadawko-app/assets/keymap-default.json"
REGISTRY = ROOT / "crates/wadawko-app/src/keys/registry.rs"
OUT = ROOT / "docs/referencia/atalhos.md"

CONTEXTS = [  # (contexto do keymap, título da seção, explicação)
    (None, "Em qualquer lugar", "Valem em todas as páginas, com ou sem vim."),
    ("Workspace", "Editor", "Valem na área do editor (grade, partitura, esteira), com ou sem vim."),
    ("Workspace && mode == simple", "Editor — modo simples (vim desligado)", "O modo padrão para quem não usa vim: digite as sílabas direto nas células."),
    ("Workspace && (mode == normal || mode == visual || mode == vline)", "Vim — normal e visual", "Com o vim ligado (Configurações ou `keys::ToggleVim`)."),
    ("Workspace && (mode == normal || mode == visual || mode == vline) && count", "Vim — com contagem digitada", ""),
    ("Workspace && (mode == normal || mode == visual || mode == vline) && !count", "Vim — sem contagem", ""),
    ("Workspace && mode == normal", "Vim — só no modo normal", ""),
    ("Workspace && (mode == visual || mode == vline)", "Vim — só no visual", ""),
    ("Workspace && mode == insert", "Vim — inserção", ""),
    ("Palette", "Paleta de comandos (Ctrl+K)", "Com a paleta aberta. Tab/Shift+Tab e ↑/↓ andam na lista, Enter executa, Esc fecha."),
]


def titles():
    src = REGISTRY.read_text()
    t = {}
    for m in re.finditer(r'spec!\("([^"]+)",\s*"([^"]+)"', src):
        t[m.group(1)] = m.group(2)
    body = src[src.index("pub fn title_for"):]
    body = body[: body.index("\n}\n")]
    for m in re.finditer(r'((?:"[^"]+"\s*\|?\s*)+)=>\s*"([^"]+)"', body):
        for name in re.findall(r'"([^"]+)"', m.group(1)):
            t.setdefault(name, m.group(2))
    prefixes = dict(re.findall(r'\("([^"]+)",\s*"([^"]+)"\)', src[src.index("PREFIX_TITLES"):]))
    return t, prefixes


def pretty(keys):
    def one(k):
        parts = k.split("-") if k not in ("-", "+") else [k]
        if len(parts) > 1 and parts[-1] == "":  # "ctrl--" → tecla "-"
            parts = parts[:-2] + ["-"]
        names = {"ctrl": "Ctrl", "shift": "Shift", "alt": "Alt", "cmd": "Cmd", "space": "Space", "enter": "Enter", "escape": "Esc", "tab": "Tab",
                 "backspace": "Backspace", "delete": "Delete", "left": "←", "right": "→", "up": "↑", "down": "↓", "home": "Home", "end": "End",
                 "pageup": "PgUp", "pagedown": "PgDn"}
        out = [names.get(p, p) for p in parts]
        # com modificador, a letra vai em maiúscula (Ctrl+K); sozinha fica como está (no vim, g ≠ G)
        if len(out) > 1 and len(out[-1]) == 1 and out[-1].isalpha():
            out[-1] = out[-1].upper()
        return "+".join(out)
    return " ".join(f"`{one(k)}`" for k in keys.split(" "))


def render():
    keymap = json.loads(KEYMAP.read_text())
    t, prefixes = titles()
    by_ctx = {b["context"]: b["bindings"] for b in keymap}
    known = {c for c, _, _ in CONTEXTS}
    at = datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    out = [
        "---",
        "type: Reference",
        'title: "Atalhos de teclado"',
        'description: "Todas as teclas do keymap padrão do app, por contexto, com o título de cada comando."',
        "tags: [referência, teclado, atalhos]",
        "generated:",
        "  by: process:tools/gen-atalhos.py",
        f"  at: {at}",
        "fontes: [crates/wadawko-app/assets/keymap-default.json, crates/wadawko-app/src/keys/registry.rs]",
        "---",
        "",
        "# Atalhos de teclado",
        "",
        "> Página **gerada** por `tools/gen-atalhos.py` a partir do keymap padrão — não edite à mão;",
        "> rode o script depois de mudar teclas ou títulos (`--check` acusa quando está desatualizada).",
        "",
        "Para mudar uma tecla, crie `~/.config/wadawko/keymap.json` no mesmo formato do",
        "[keymap padrão](https://github.com/aqilputi/wadawko/blob/main/crates/wadawko-app/assets/keymap-default.json)",
        "(paleta → *Criar keymap do usuário*); ele é somado ao padrão e conflitos aparecem na página de Configurações.",
        "Todo comando também pode ser executado pela paleta (`Ctrl+K`), mesmo sem tecla.",
        "Para os conceitos por trás (modos, seleção, arrasto), veja o [guia de uso](/guias/uso.md).",
        "",
    ]
    for ctx, title, expl in CONTEXTS + [(c, f"Contexto `{c}`", "") for c in by_ctx if c not in known]:
        binds = by_ctx.get(ctx)
        if not binds:
            continue
        out += [f"## {title}", ""]
        if expl:
            out += [expl, ""]
        out += ["| Tecla | Comando | Ação |", "|---|---|---|"]
        for keys, action in binds.items():
            label = t.get(action, "")
            out.append(f"| {pretty(keys)} | `{action}` | {label} |")
        out.append("")
    if prefixes:
        out += ["## Prefixos (which-key)", "", "Apertar o prefixo e esperar mostra as continuações possíveis.", "", "| Prefixo | Grupo |", "|---|---|"]
        out += [f"| {pretty(p)} | {g} |" for p, g in prefixes.items()]
        out.append("")
    return "\n".join(out)


def strip_stamp(s):
    return re.sub(r"\n  at: [^\n]+", "", s)


if __name__ == "__main__":
    new = render()
    if "--check" in sys.argv:
        cur = OUT.read_text() if OUT.exists() else ""
        if strip_stamp(cur) != strip_stamp(new):
            print(f"{OUT.relative_to(ROOT)} está desatualizado: rode python3 tools/gen-atalhos.py", file=sys.stderr)
            sys.exit(1)
        print("atalhos.md em dia")
    else:
        OUT.parent.mkdir(parents=True, exist_ok=True)
        OUT.write_text(new)
        print(f"escrito {OUT.relative_to(ROOT)}")
