#!/usr/bin/env python3
"""Confere (e gera os índices de) um bundle OKF v0.2 — o `docs/` do WaDAWko.

Spec: https://github.com/GoogleCloudPlatform/open-knowledge-format/blob/main/SPEC.md

Conformidade (§11):
  1. todo .md que não é index.md/log.md tem frontmatter YAML legível;
  2. todo frontmatter tem `type` não vazio;
  3. index.md e log.md presentes seguem §8 e §9.
Além do spec (política deste repo): links internos precisam resolver.

uso: python3 tools/okf-check.py docs                 # confere
     python3 tools/okf-check.py docs --write-index   # (re)gera os index.md e confere
Sem dependências além da biblioteca padrão (o frontmatter daqui usa YAML simples).
"""
import pathlib
import re
import sys

RESERVED = {"index.md", "log.md"}
OKF_VERSION = "0.2"

# ordem e descrição das pastas nos índices gerados (pastas não têm frontmatter)
FOLDERS = {
    "guias": ("Guias", "Instalar, compilar e usar o app."),
    "referencia": ("Referência", "Atalhos, linha de comando, vocabulário, formato da música e arquivos de dados."),
    "arquitetura": ("Arquitetura", "Como o código está organizado: crates e o app desktop."),
    "desenvolvimento": ("Desenvolvimento", "Testes, depuração, desempenho e como contribuir."),
    "planejamento": ("Planejamento", "Os planos do projeto, do app web ao porte nativo e ao repositório completo."),
    "material": ("Material de referência", "Arquivos externos usados no projeto (PDFs)."),
    "imagens": ("Imagens", "Capturas de tela do app usadas no README e nos guias."),
}
FIRST = ["visao-geral", "uso", "instalar-e-compilar", "atalhos", "workspace", "app", "repo", "gpui", "teclado"]
ROOT_HEADINGS = ("Comece aqui", "Pastas")


def frontmatter(text):
    """(dict, corpo) — None se não houver bloco `---` ... `---` no topo."""
    if not text.startswith("---\n"):
        return None, text
    end = text.find("\n---\n", 4)
    if end < 0:
        end = text.find("\n---", 4)
        if end < 0 or text[end + 4:].strip():
            return None, text
    block, body = text[4:end], text[end + 5:]
    data = {}
    for line in block.splitlines():
        if not line.strip() or line.startswith((" ", "\t", "-", "#")):
            continue  # valores aninhados/listas em bloco: não interessam aqui
        m = re.match(r"^([A-Za-z_][\w-]*):\s*(.*)$", line)
        if not m:
            raise ValueError(f"linha de frontmatter ilegível: {line!r}")
        v = m.group(2).strip()
        if len(v) >= 2 and v[0] == v[-1] and v[0] in "\"'":
            v = v[1:-1]
        data[m.group(1)] = v
    return data, body


def concepts(root):
    for p in sorted(root.rglob("*.md")):
        if p.name not in RESERVED:
            yield p


def sort_key(p):
    stem = p.stem
    return (FIRST.index(stem) if stem in FIRST else len(FIRST), stem)


def build_index(root, d):
    docs, subdirs = [], []
    for p in sorted(d.iterdir(), key=sort_key):
        if p.is_dir() and any(p.rglob("*")):
            subdirs.append(p)
        elif p.suffix == ".md" and p.name not in RESERVED:
            fm, _ = frontmatter(p.read_text())
            fm = fm or {}
            docs.append(f"* [{fm.get('title', p.stem)}]({p.name}) - {fm.get('description', '')}".rstrip(" -"))
        elif p.is_file() and p.suffix != ".md":
            kind = "captura de tela" if p.suffix in (".png", ".jpg", ".webp", ".gif") else "arquivo de material de referência"
            docs.append(f"* [{p.name}]({p.name}) - {kind}")
    is_root = d == root
    out = []
    if is_root:
        out += ["---", f'okf_version: "{OKF_VERSION}"', "---", ""]
    head_docs = ROOT_HEADINGS[0] if is_root else FOLDERS.get(d.name, (d.name.capitalize(),))[0]
    if docs:
        out += [f"# {head_docs}", ""] + docs + [""]
    if subdirs:
        out += [f"# {ROOT_HEADINGS[1] if is_root else 'Subpastas'}", ""]
        order = list(FOLDERS)
        for s in sorted(subdirs, key=lambda s: (order.index(s.name) if s.name in order else len(order), s.name)):
            title, desc = FOLDERS.get(s.name, (s.name, ""))
            out.append(f"* [{title}]({s.name}/) - {desc}".rstrip(" -"))
        out.append("")
    return "\n".join(out)


def write_indexes(root):
    dirs = [root] + sorted(p for p in root.rglob("*") if p.is_dir())
    for d in dirs:
        if not any(d.iterdir()):
            continue
        (d / "index.md").write_text(build_index(root, d))
        print("índice:", (d / "index.md").relative_to(root.parent))


def check(root):
    errors = []
    for p in concepts(root):
        rel = p.relative_to(root)
        try:
            fm, _ = frontmatter(p.read_text(encoding="utf-8"))
        except (UnicodeDecodeError, ValueError) as e:
            errors.append(f"{rel}: {e}")
            continue
        if fm is None:
            errors.append(f"{rel}: sem frontmatter YAML (§11.1)")
        elif not fm.get("type"):
            errors.append(f"{rel}: `type` ausente ou vazio (§11.2)")
        elif fm.get("status") and fm["status"] not in ("draft", "stable", "deprecated"):
            errors.append(f"{rel}: status inválido {fm['status']!r} (draft|stable|deprecated)")
    for idx in root.rglob("index.md"):
        rel = idx.relative_to(root)
        fm, body = frontmatter(idx.read_text())
        if fm is not None and (idx.parent != root or set(fm) - {"okf_version"}):
            errors.append(f"{rel}: index.md só pode ter frontmatter na raiz, e só `okf_version` (§8, §12)")
        for line in body.splitlines():
            if line.strip() and not line.startswith("# ") and not re.match(r"^\* \[[^\]]+\]\([^)]+\)( - .+)?$", line):
                errors.append(f"{rel}: linha fora do formato `* [Título](url) - descrição`: {line!r} (§8)")
    root_idx = root / "index.md"
    if root_idx.exists():
        fm, _ = frontmatter(root_idx.read_text())
        if not fm or fm.get("okf_version") != OKF_VERSION:
            errors.append(f'index.md: raiz deveria declarar okf_version: "{OKF_VERSION}" (§12)')
    for log in root.rglob("log.md"):
        for line in log.read_text().splitlines():
            if line.startswith("## ") and not re.match(r"^## \d{4}-\d{2}-\d{2}$", line):
                errors.append(f"{log.relative_to(root)}: título de data fora do ISO 8601 `## YYYY-MM-DD`: {line!r} (§9)")
    # links internos (política do repo; o spec só pede tolerância)
    for p in root.rglob("*.md"):
        text = re.sub(r"```.*?```", "", p.read_text(), flags=re.S)
        text = re.sub(r"`[^`\n]*`", "", text)
        for target in re.findall(r"\]\(([^)\s]+)\)", text):
            if re.match(r"^[a-z]+:", target) or target.startswith("#"):
                continue
            path = target.split("#", 1)[0]
            dest = (root / path.lstrip("/")) if path.startswith("/") else (p.parent / path)
            if not dest.exists():
                errors.append(f"{p.relative_to(root)}: link quebrado → {target}")
    return errors


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    root = pathlib.Path(args[0] if args else "docs").resolve()
    if "--write-index" in sys.argv:
        write_indexes(root)
    errs = check(root)
    n = sum(1 for _ in concepts(root))
    if errs:
        print(f"OKF v{OKF_VERSION}: {len(errs)} problema(s) em {root.name}/", file=sys.stderr)
        for e in errs:
            print("  " + e, file=sys.stderr)
        sys.exit(1)
    print(f"OKF v{OKF_VERSION}: {n} documentos conformes em {root.name}/ (índices, log e links ok)")
