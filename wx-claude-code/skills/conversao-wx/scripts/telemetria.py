#!/usr/bin/env python3
"""Telemetria em OTLP/JSON, gerada do registro de operacoes -- sem crate, sem agente.

Por que assim: o `registro.py` ja grava toda operacao com instante, duracao e
codigo de saida, e o `uso_de_tokens.py` ja le o gasto real. Nao falta MEDICAO;
falta o FORMATO que a area de infraestrutura do cliente ja consome. Este script
traduz o que existe para OpenTelemetry, sem trazer SDK nenhum.

Tres decisoes que valem mais que o codigo:

  ARQUIVO POR PADRAO. A saida vai para `.wx-migration/telemetria/`, no disco do
  cliente. Enviar para fora e opcional, explicito e para um endereco que o dono
  escreve -- o Sovereign Mode do projeto e o que abre banco e governo, e
  telemetria que sai sozinha mata isso.

  NADA DE CONTEUDO. Vao nomes de operacao, tempos e codigos. Nao vai argumento,
  caminho de arquivo do cliente nem trecho de codigo: os mesmos campos que o
  registro ja omite continuam omitidos aqui. Telemetria e o segundo lugar onde
  segredo vaza sem ninguem perceber -- o primeiro e o log.

  SPAN E O QUE ACONTECEU. Um span por operacao registrada, com o pai sendo a
  sessao. Nada e sintetizado para "ficar bonito no grafico": operacao que o
  registro nao tem nao vira span.

Uso:
  telemetria.py exportar [--saida ARQ] [--dias 7]
  telemetria.py enviar --endereco http://127.0.0.1:4318 [--dias 7]
  telemetria.py resumo
"""
from __future__ import annotations

import argparse
import json
import sys
import urllib.error
import urllib.request
from datetime import date, datetime, timedelta, timezone
from pathlib import Path

# O que pode virar atributo. Lista fechada, de propósito: campo novo no registro
# nao vaza para a telemetria sem alguem escrever aqui.
ATRIBUTOS_PERMITIDOS = ("operacao", "codigo", "ms")


def linhas(raiz: Path, dias: int) -> list[dict]:
    pasta = raiz / ".wx-migration" / "logs"
    if not pasta.is_dir():
        return []
    limite = date.today() - timedelta(days=dias)
    achadas = []
    for arq in sorted(pasta.glob("plugin-*.jsonl")):
        try:
            dia = date.fromisoformat(arq.stem.replace("plugin-", ""))
        except ValueError:
            continue
        if dia < limite:
            continue
        for l in arq.read_text(encoding="utf-8").splitlines():
            if not l.strip():
                continue
            try:
                achadas.append(json.loads(l))
            except json.JSONDecodeError:
                continue
    return achadas


def nano(iso: str) -> int:
    try:
        d = datetime.fromisoformat(iso)
    except ValueError:
        return 0
    if d.tzinfo is None:
        d = d.replace(tzinfo=timezone.utc)
    return int(d.timestamp() * 1_000_000_000)


def hexid(texto: str, tamanho: int) -> str:
    import hashlib
    return hashlib.sha256(texto.encode()).hexdigest()[:tamanho]


def spans(raiz: Path, dias: int) -> dict:
    """Traduz o registro em OTLP/JSON. Um trace por dia, um span por operacao."""
    itens = linhas(raiz, dias)
    por_dia: dict[str, list] = {}
    for i in itens:
        instante = i.get("instante") or i.get("quando") or ""
        dia = instante[:10] or "sem-data"
        # o registro grava o campo `ms`; adivinhar o nome dava span de duracao
        # zero em tudo, e um grafico de latencia todo no chao e pior que nenhum
        dur_ms = round(float(i.get("ms", 0)))
        inicio = nano(instante)
        atributos = []
        for chave in ATRIBUTOS_PERMITIDOS:
            if chave in i and i[chave] not in (None, ""):
                v = i[chave]
                atributos.append({"key": f"wx.{chave}",
                                  "value": {"stringValue": str(v)} if not isinstance(v, int)
                                  else {"intValue": str(v)}})
        codigo = i.get("codigo") or 0
        por_dia.setdefault(dia, []).append({
            "traceId": hexid(f"{raiz}:{dia}", 32),
            "spanId": hexid(f"{instante}:{i.get('operacao', '')}:{len(por_dia.get(dia, []))}", 16),
            "name": i.get("operacao", "operacao"),
            "kind": 1,
            "startTimeUnixNano": str(inicio),
            "endTimeUnixNano": str(inicio + int(dur_ms) * 1_000_000),
            "attributes": atributos,
            "status": {"code": 2 if codigo else 1},
        })
    return {
        "resourceSpans": [{
            "resource": {"attributes": [
                {"key": "service.name", "value": {"stringValue": "wx-claude-code"}},
                {"key": "wx.projeto", "value": {"stringValue": raiz.name}},
            ]},
            "scopeSpans": [{
                "scope": {"name": "wx-claude-code/registro"},
                "spans": s,
            } for s in por_dia.values()],
        }],
        "_medido": {"operacoes": len(itens), "dias": len(por_dia)},
    }


def exportar(args, raiz: Path) -> int:
    d = spans(raiz, args.dias)
    medido = d.pop("_medido")
    alvo = Path(args.saida) if args.saida else raiz / ".wx-migration" / "telemetria" / "otlp-spans.json"
    alvo.parent.mkdir(parents=True, exist_ok=True)
    alvo.write_text(json.dumps(d, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if args.json:
        print(json.dumps({"escrito": str(alvo), **medido}, ensure_ascii=False))
    else:
        print(f"escrito {alvo}")
        print(f"  {medido['operacoes']} operações em {medido['dias']} dia(s), OTLP/JSON")
        print("  fica no disco: enviar para fora é `enviar`, com endereço explícito")
    return 0


def enviar(args, raiz: Path) -> int:
    """Envia para um coletor OTLP/HTTP. Explicito, e nunca o padrao."""
    d = spans(raiz, args.dias)
    medido = d.pop("_medido")
    url = args.endereco.rstrip("/") + "/v1/traces"
    corpo = json.dumps(d, ensure_ascii=False).encode()
    req = urllib.request.Request(url, data=corpo, method="POST",
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=args.timeout) as r:  # noqa: S310
            codigo = r.status
    except (urllib.error.URLError, OSError, TimeoutError) as e:
        print(f"erro: não deu para enviar a {url}: {e}", file=sys.stderr)
        return 3
    if args.json:
        print(json.dumps({"enviado": url, "http": codigo, **medido}, ensure_ascii=False))
    else:
        print(f"enviado a {url}: HTTP {codigo} · {medido['operacoes']} operações")
    return 0 if 200 <= codigo < 300 else 3


def resumo(args, raiz: Path) -> int:
    itens = linhas(raiz, args.dias)
    if args.json:
        print(json.dumps({"operacoes": len(itens)}, ensure_ascii=False))
        return 0
    if not itens:
        print("nenhuma operação registrada no período")
        return 0
    por_op: dict[str, list[int]] = {}
    for i in itens:
        por_op.setdefault(i.get("operacao", "?"), []).append(round(float(i.get("ms", 0))))
    print(f"{len(itens)} operações em {args.dias} dia(s)\n")
    print(f"{'operação':<24}{'vezes':>7}{'total ms':>11}{'pior ms':>10}")
    for op, ms in sorted(por_op.items(), key=lambda x: -sum(x[1])):
        print(f"{op:<24}{len(ms):>7}{sum(ms):>11}{max(ms):>10}")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--project-root", default=".")
    p.add_argument("--json", action="store_true")
    p.add_argument("--dias", type=int, default=7)
    sub = p.add_subparsers(dest="cmd", required=True)
    e = sub.add_parser("exportar", help="escreve OTLP/JSON no disco do cliente")
    e.add_argument("--saida")
    n = sub.add_parser("enviar", help="POST para um coletor OTLP/HTTP; explícito, nunca automático")
    n.add_argument("--endereco", required=True)
    n.add_argument("--timeout", type=int, default=10)
    sub.add_parser("resumo", help="o que gastou tempo, sem sair da máquina")
    args = p.parse_args()
    raiz = Path(args.project_root).resolve()
    return {"exportar": exportar, "enviar": enviar, "resumo": resumo}[args.cmd](args, raiz)


try:
    import registro
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
