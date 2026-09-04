#!/usr/bin/env python3
"""Registro de todas as operacoes do plugin, em JSONL, dentro do projeto.

Por que existe: sem registro, "o que este plugin fez no meu projeto?" so se
responde lendo o git, e o que ele fez FORA do git (rodou o pre-flight, negou
uma escrita, exportou, arquivou artefato) nao aparece em lugar nenhum.

Onde grava: `.wx-migration/logs/plugin-AAAA-MM-DD.jsonl`, uma linha por
operacao, com instante, script, argumentos, codigo de saida e duracao. Sem
`.wx-migration` por perto **nao grava nada**: o plugin nao suja diretorio que
nao e projeto dele.

O que nunca entra: senha, token, chave. O valor de argumento com nome suspeito
vira `<omitido>`, e qualquer texto com cara de segredo e substituido antes de
gravar -- a regra do projeto vale aqui inteira, inclusive porque log e o lugar
onde segredo costuma vazar sem ninguem perceber.

Falha de registro nunca derruba a operacao: se nao der para gravar, a operacao
segue e o silencio e o preco.

Uso:
  registro.py --project-root . resumo [--dias 7]     # o que rodou, por script
  registro.py --project-root . ver [--n 40]          # as ultimas operacoes
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
from collections import Counter
from datetime import date, datetime, timezone
from pathlib import Path

TOKEN = re.compile(r"ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,}|-----BEGIN [A-Z ]*PRIVATE KEY-----|eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{10,}")
# Nome de argumento cujo VALOR nunca se registra, mesmo que hoje ninguem passe
# segredo por linha de comando: o dia em que alguem passar, o log nao guarda.
SUSPEITO = re.compile(r"senha|password|passwd|token|secret|chave|key|credencial|serial", re.I)
OMITIDO = "<omitido>"


def raiz_do_projeto(argv: list[str] | None = None, cwd: Path | None = None) -> Path | None:
    """A raiz vem do --project-root quando existe; senao, sobe do cwd ate achar
    .wx-migration. Nenhuma das duas: nao ha projeto, e nao se grava."""
    argv = argv if argv is not None else sys.argv[1:]
    for i, a in enumerate(argv):
        if a in ("--project-root", "--projeto") and i + 1 < len(argv):
            p = Path(argv[i + 1]).expanduser()
            return p if (p / ".wx-migration").is_dir() else None
        if a.startswith("--project-root="):
            p = Path(a.split("=", 1)[1]).expanduser()
            return p if (p / ".wx-migration").is_dir() else None
    base = (cwd or Path.cwd()).resolve()
    for pasta in [base, *base.parents]:
        if (pasta / ".wx-migration").is_dir():
            return pasta
    return None


def limpar(texto: str) -> str:
    return TOKEN.sub(OMITIDO, texto)


def argumentos_seguros(argv: list[str]) -> list[str]:
    saida: list[str] = []
    omitir_proximo = False
    for a in argv:
        if omitir_proximo:
            saida.append(OMITIDO)
            omitir_proximo = False
            continue
        if a.startswith("--") and SUSPEITO.search(a.split("=", 1)[0]):
            if "=" in a:
                saida.append(a.split("=", 1)[0] + "=" + OMITIDO)
            else:
                saida.append(a)
                omitir_proximo = True
            continue
        saida.append(limpar(a))
    return saida


def registrar(operacao: str, raiz: Path | None = None, **campos) -> None:
    """Grava uma linha. Nunca levanta: registro que quebra operacao e pior que
    registro ausente."""
    try:
        raiz = raiz or raiz_do_projeto()
        if raiz is None:
            return
        pasta = Path(raiz) / ".wx-migration" / "logs"
        pasta.mkdir(parents=True, exist_ok=True)
        linha = {"instante": datetime.now(timezone.utc).isoformat(timespec="seconds"), "operacao": operacao}
        for k, v in campos.items():
            linha[k] = limpar(v) if isinstance(v, str) else v
        with (pasta / f"plugin-{date.today().isoformat()}.jsonl").open("a", encoding="utf-8") as f:
            f.write(json.dumps(linha, ensure_ascii=False) + "\n")
    except Exception:  # noqa: BLE001 - ver docstring
        pass


def envolver(arquivo: str, funcao, argv: list[str] | None = None) -> int:
    """Roda `funcao`, registra a operacao e devolve o codigo de saida.

    Registra tambem quando a funcao levanta: erro nao registrado e o que faz o
    usuario dizer "nao fiz nada" no dia seguinte."""
    nome = Path(arquivo).stem
    argv = argv if argv is not None else sys.argv[1:]
    raiz = raiz_do_projeto(argv)
    t0 = time.monotonic()
    try:
        codigo = funcao()
    except SystemExit as e:  # argparse sai por aqui
        codigo = e.code if isinstance(e.code, int) else (0 if e.code is None else 2)
        registrar(nome, raiz, argumentos=argumentos_seguros(argv), codigo=codigo,
                  ms=round((time.monotonic() - t0) * 1000, 1))
        raise
    except BaseException as e:  # noqa: BLE001 - registra e deixa subir
        registrar(nome, raiz, argumentos=argumentos_seguros(argv), codigo=1,
                  ms=round((time.monotonic() - t0) * 1000, 1), erro=f"{type(e).__name__}: {e}")
        raise
    codigo = 0 if codigo is None else codigo
    registrar(nome, raiz, argumentos=argumentos_seguros(argv), codigo=codigo,
              ms=round((time.monotonic() - t0) * 1000, 1))
    return codigo


def _linhas(raiz: Path, dias: int) -> list[dict]:
    pasta = raiz / ".wx-migration" / "logs"
    corte = time.time() - dias * 86400
    saida: list[dict] = []
    for arq in sorted(pasta.glob("plugin-*.jsonl")) if pasta.is_dir() else []:
        try:
            if arq.stat().st_mtime < corte:
                continue
            for l in arq.read_text(encoding="utf-8").splitlines():
                try:
                    saida.append(json.loads(l))
                except json.JSONDecodeError:
                    continue
        except OSError:
            continue
    return saida


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("comando", choices=["resumo", "ver"])
    p.add_argument("--project-root", type=Path, default=Path("."))
    p.add_argument("--dias", type=int, default=7)
    p.add_argument("--n", type=int, default=40)
    p.add_argument("--json", action="store_true")
    a = p.parse_args()
    raiz = a.project_root.resolve()
    if not (raiz / ".wx-migration").is_dir():
        print(f"erro: {raiz} nao e um projeto do plugin (falta .wx-migration/)", file=sys.stderr)
        return 2
    itens = _linhas(raiz, a.dias)
    if a.json:
        print(json.dumps(itens[-a.n:] if a.comando == "ver" else itens, ensure_ascii=False, indent=2))
        return 0
    if not itens:
        print(f"nenhuma operacao registrada nos ultimos {a.dias} dias")
        return 0
    if a.comando == "ver":
        for i in itens[-a.n:]:
            erro = f"  ERRO {i['erro']}" if i.get("erro") else ""
            print(f"{i.get('instante','')}  {i.get('operacao',''):<24} codigo={i.get('codigo')} {i.get('ms')}ms{erro}")
        return 0
    c = Counter(i.get("operacao", "?") for i in itens)
    falhas = Counter(i.get("operacao", "?") for i in itens if i.get("codigo"))
    tempo: dict[str, float] = {}
    for i in itens:
        tempo[i.get("operacao", "?")] = tempo.get(i.get("operacao", "?"), 0.0) + float(i.get("ms") or 0)
    largura = max(len(k) for k in c)
    print(f"{len(itens)} operacoes nos ultimos {a.dias} dias, em {raiz}\n")
    print(f"{'operacao':<{largura}}  vezes  com erro  tempo total")
    for nome, vezes in c.most_common():
        print(f"{nome:<{largura}}  {vezes:>5}  {falhas.get(nome, 0):>8}  {tempo[nome]/1000:>8.1f}s")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
