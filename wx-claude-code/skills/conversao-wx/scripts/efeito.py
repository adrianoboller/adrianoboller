#!/usr/bin/env python3
"""Confere o EFEITO de uma acao, nao o codigo de saida dela.

A confusao que este script existe para desfazer: `comando executou` nao e
`efeito aconteceu`. Um `ALTER TABLE` que sai 0 nao prova que a coluna esta la;
um `git push` que sai 0 nao prova que o commit chegou; um gerador que sai 0 nao
prova que escreveu o arquivo. Sistema agentico erra exatamente aqui, porque le
o proprio codigo de saida como se fosse o mundo.

O fluxo e em duas partes, e a segunda e a que importa:

  1. DECLARAR   o que se espera que aconteca (a intencao, antes de agir)
  2. CONFERIR   ler o estado REAL e comparar

E o veredito tem tres valores, nao dois:

  VERIFICADO     o estado real bate com o esperado
  DIVERGENTE     o estado real existe e NAO bate -- este e o achado
  INCONCLUSIVO   nao deu para ler o estado real; nao se conclui nada

INCONCLUSIVO existir e o ponto todo: quando a conferencia falha, a resposta
honesta nao e "deu certo" nem "deu errado", e "nao sei" -- e quem aprova precisa
ver isso escrito.

Efeitos que ele sabe conferir hoje, todos sem dependencia externa:
  arquivo-existe   o caminho existe (ou nao, com --ausente)
  arquivo-contem   o arquivo contem o texto
  arquivo-hash     o arquivo tem o SHA-256 dado
  comando-diz      um comando de LEITURA devolve saida que casa com o padrao
  git-commit       o commit existe na arvore de trabalho

Uso:
  efeito.py conferir --acao "criar índice em customers" \\
      --esperado arquivo-contem --alvo database/schema.sql --valor "idx_customers_cnpj"
  efeito.py conferir --acao "..." --esperado comando-diz \\
      --comando "psql -c '\\d customers'" --valor "idx_customers_cnpj"
  efeito.py listar
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import shlex
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

TIPOS = ("arquivo-existe", "arquivo-contem", "arquivo-hash", "comando-diz", "git-commit")
# Conferencia le o mundo; nao muda o mundo. Comando de leitura, so.
PROIBIDOS = re.compile(r"\b(rm|mv|cp|dd|mkfs|truncate|shutdown|reboot|kill|chmod|chown|"
                       r"drop|delete|insert|update|alter|create|push|commit|reset|checkout)\b", re.I)


def registro_de(raiz: Path) -> Path:
    return raiz / ".wx-migration" / "efeitos.jsonl"


def conferir_arquivo_existe(raiz: Path, alvo: str, ausente: bool) -> tuple[str, str]:
    p = (raiz / alvo) if not Path(alvo).is_absolute() else Path(alvo)
    existe = p.exists()
    if ausente:
        return ("verificado", f"{alvo} não existe, como esperado") if not existe else \
               ("divergente", f"{alvo} existe, e o esperado era que não existisse")
    return ("verificado", f"{alvo} existe ({p.stat().st_size} bytes)") if existe else \
           ("divergente", f"{alvo} não existe")


def conferir_arquivo_contem(raiz: Path, alvo: str, valor: str) -> tuple[str, str]:
    p = (raiz / alvo) if not Path(alvo).is_absolute() else Path(alvo)
    if not p.is_file():
        return "inconclusivo", f"não deu para ler {alvo}: arquivo não existe"
    try:
        texto = p.read_text(encoding="utf-8", errors="replace")
    except OSError as e:
        return "inconclusivo", f"não deu para ler {alvo}: {e}"
    return ("verificado", f"{alvo} contém «{valor}»") if valor in texto else \
           ("divergente", f"{alvo} NÃO contém «{valor}»")


def conferir_arquivo_hash(raiz: Path, alvo: str, valor: str) -> tuple[str, str]:
    p = (raiz / alvo) if not Path(alvo).is_absolute() else Path(alvo)
    if not p.is_file():
        return "inconclusivo", f"não deu para ler {alvo}: arquivo não existe"
    h = hashlib.sha256(p.read_bytes()).hexdigest()
    return ("verificado", f"{alvo} tem o SHA-256 esperado") if h == valor else \
           ("divergente", f"{alvo} tem {h[:16]}…, esperado {valor[:16]}…")


def conferir_comando(raiz: Path, comando: str, valor: str, timeout: int) -> tuple[str, str]:
    if PROIBIDOS.search(comando):
        return "inconclusivo", ("comando de conferência não pode mudar o estado; "
                                "use um comando de leitura")
    try:
        r = subprocess.run(shlex.split(comando), cwd=raiz, capture_output=True,
                           text=True, timeout=timeout)
    except (OSError, ValueError, subprocess.TimeoutExpired) as e:
        return "inconclusivo", f"não deu para ler o estado: {e}"
    saida = r.stdout + r.stderr
    if not valor:
        return ("verificado", f"comando saiu 0") if r.returncode == 0 else \
               ("divergente", f"comando saiu {r.returncode}")
    try:
        casou = re.search(valor, saida) is not None
    except re.error:
        casou = valor in saida
    return ("verificado", f"a saída casa com «{valor}»") if casou else \
           ("divergente", f"a saída NÃO casa com «{valor}»: {saida.strip()[:160]}")


def conferir_git_commit(raiz: Path, valor: str, timeout: int) -> tuple[str, str]:
    try:
        r = subprocess.run(["git", "cat-file", "-t", valor], cwd=raiz,
                           capture_output=True, text=True, timeout=timeout)
    except (OSError, subprocess.TimeoutExpired) as e:
        return "inconclusivo", f"não deu para consultar o git: {e}"
    if r.returncode != 0:
        return "divergente", f"o commit {valor} não está nesta árvore"
    return ("verificado", f"{valor} é um commit desta árvore") if r.stdout.strip() == "commit" else \
           ("divergente", f"{valor} existe mas é {r.stdout.strip()}, não commit")


def conferir(args, raiz: Path) -> int:
    if args.esperado == "arquivo-existe":
        estado, detalhe = conferir_arquivo_existe(raiz, args.alvo or "", args.ausente)
    elif args.esperado == "arquivo-contem":
        estado, detalhe = conferir_arquivo_contem(raiz, args.alvo or "", args.valor or "")
    elif args.esperado == "arquivo-hash":
        estado, detalhe = conferir_arquivo_hash(raiz, args.alvo or "", args.valor or "")
    elif args.esperado == "comando-diz":
        estado, detalhe = conferir_comando(raiz, args.comando or "", args.valor or "", args.timeout)
    else:
        estado, detalhe = conferir_git_commit(raiz, args.valor or "", args.timeout)

    ficha = {
        "instante": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "acao": args.acao, "esperado": args.esperado,
        "alvo": args.alvo or args.comando or args.valor or "",
        "resultado": estado, "detalhe": detalhe,
    }
    if (raiz / ".wx-migration").is_dir():
        with registro_de(raiz).open("a", encoding="utf-8") as f:
            f.write(json.dumps(ficha, ensure_ascii=False) + "\n")
    if args.json:
        print(json.dumps(ficha, ensure_ascii=False))
    else:
        print(f"{estado.upper()}: {args.acao}")
        print(f"  {detalhe}")
        if estado == "inconclusivo":
            print("  (inconclusivo NÃO é aprovação: ninguém conseguiu ler o estado real)")
    return {"verificado": 0, "divergente": 1, "inconclusivo": 2}[estado]


def listar(args, raiz: Path) -> int:
    arq = registro_de(raiz)
    if not arq.is_file():
        print("nenhum efeito conferido ainda")
        return 0
    linhas = [json.loads(l) for l in arq.read_text(encoding="utf-8").splitlines() if l.strip()]
    if args.json:
        print(json.dumps(linhas, ensure_ascii=False))
        return 0
    for f in linhas[-args.n:]:
        print(f"{f['instante']}  {f['resultado'].upper():<13} {f['acao']}")
        print(f"                            {f['detalhe']}")
    div = sum(1 for f in linhas if f["resultado"] == "divergente")
    inc = sum(1 for f in linhas if f["resultado"] == "inconclusivo")
    print(f"\n{len(linhas)} conferências · {div} divergentes · {inc} inconclusivas")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--project-root", default=".")
    p.add_argument("--json", action="store_true")
    sub = p.add_subparsers(dest="cmd", required=True)

    c = sub.add_parser("conferir", help="lê o estado real e compara com o esperado")
    c.add_argument("--acao", required=True, help="o que foi feito, em uma linha")
    c.add_argument("--esperado", required=True, choices=TIPOS)
    c.add_argument("--alvo", help="arquivo, para os efeitos de arquivo")
    c.add_argument("--comando", help="comando de LEITURA, para comando-diz")
    c.add_argument("--valor", help="texto, padrão, hash ou commit esperado")
    c.add_argument("--ausente", action="store_true", help="espera que o alvo NÃO exista")
    c.add_argument("--timeout", type=int, default=60)

    l = sub.add_parser("listar", help="as conferências já feitas")
    l.add_argument("-n", type=int, default=20)

    args = p.parse_args()
    raiz = Path(args.project_root).resolve()
    return {"conferir": conferir, "listar": listar}[args.cmd](args, raiz)


try:
    import registro
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
