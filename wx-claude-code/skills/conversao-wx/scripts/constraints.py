#!/usr/bin/env python3
"""Registro de restricoes do projeto, e o portao que as confere (C-GATE).

O problema que ele resolve: as regras de um projeto de migracao vivem hoje
espalhadas em CLAUDE.md, DESIGN.md, conversion.config.json, comentario de codigo
e cabeca de gente. Regra que so existe em prosa depende de o agente lembrar --
e a bateria deste projeto ja provou tres vezes que lembrar nao e garantia.

A separacao que este script cria:

  F-GATE  "funciona?"          testes, golden master, comportamento
  C-GATE  "esta conforme?"     as restricoes do projeto, uma a uma

Nao e teoria: num levantamento publico de reparos automaticos, cerca de um
terco dos patches que passavam nos testes funcionais ainda violava restricao de
revisao. Teste verde nao e Sprint aprovada.

Cada restricao e um objeto com id, origem, escopo, severidade e -- quando da --
um VALIDADOR: um comando que sai 0 quando a regra vale. Duas pegadinhas, as
duas achadas rodando:

  * o comando roda SEM shell (nada de `|`, `&&`, `>`); para usar shell,
    escreva `sh -c "..."` de proposito;
  * `grep` sai 1 quando NAO acha. Um validador de "nao ha segredo aqui" acusaria
    violacao com o projeto limpo -- para esses, use `--inverter`, que diz que
    sair 0 significa ACHOU o problema. Regra sem validador
continua valendo, mas entra como `manual` e o C-GATE a devolve como
INCONCLUSIVA, nunca como aprovada: portao que aprova o que nao conferiu e pior
que portao nenhum.

Uso:
  constraints.py criar --titulo "..." --origem ADR-0002 --escopo "src/api/**" \\
      --severidade bloqueante [--validador "cargo test --test compat"] [--inverter]
  constraints.py listar [--json]
  constraints.py c-gate [--json]      # roda os validadores; 0 aprova
  constraints.py revogar CONST-0003 --por CONST-0007 --motivo "..."
"""
from __future__ import annotations

import argparse
import json
import re
import os
import shlex
import shutil
import subprocess
import sys
import time
from datetime import date
from pathlib import Path

SEVERIDADES = ("bloqueante", "grave", "aviso")
# INCONCLUSIVA e um resultado de primeira classe: e o que a restricao manual
# devolve, e o que um validador que estourou o tempo devolve. Ela NAO aprova.
RESULTADOS = ("aprovada", "violada", "inconclusiva", "revogada")


def arquivo(raiz: Path) -> Path:
    return raiz / ".wx-migration" / "constraints.json"


def carregar(raiz: Path) -> list[dict]:
    p = arquivo(raiz)
    if not p.is_file():
        return []
    try:
        d = json.loads(p.read_text(encoding="utf-8"))
    except json.JSONDecodeError as e:
        print(f"erro: constraints.json ilegível: {e}", file=sys.stderr)
        raise SystemExit(2) from e
    return d.get("constraints", [])


def gravar(raiz: Path, itens: list[dict]) -> None:
    p = arquivo(raiz)
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps({"schema_version": "1.0", "atualizado_em": date.today().isoformat(),
                             "constraints": itens}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    escrever_indice(raiz, itens)


def escrever_indice(raiz: Path, itens: list[dict]) -> None:
    L = ["# Restrições do projeto (C-GATE)", "",
         f"Gerado por `constraints.py` em {date.today().isoformat()}. Não se edita à mão.", "",
         "A Sprint só é aprovada com **F-GATE e C-GATE**: funcionar não é o mesmo que "
         "estar conforme. Restrição sem validador continua valendo, mas o portão a "
         "devolve como INCONCLUSIVA — nunca como aprovada.", "",
         "| id | severidade | título | escopo | validador | origem | estado |",
         "| --- | --- | --- | --- | --- | --- | --- |"]
    for c in itens:
        L.append(f"| {c['id']} | {c['severidade']} | {c['titulo']} | `{c.get('escopo', '')}` | "
                 f"{'`' + c['validador'] + '`' if c.get('validador') else 'manual'} | "
                 f"{c.get('origem', '')} | {c.get('estado', 'ativa')} |")
    if not itens:
        L.append("| — | — | nenhuma restrição registrada ainda | — | — | — | — |")
    (raiz / ".wx-migration" / "constraints.md").write_text("\n".join(L) + "\n", encoding="utf-8")


def criar(args, raiz: Path) -> int:
    if not (raiz / ".wx-migration").is_dir():
        print("erro: rode dentro de um projeto com .wx-migration/", file=sys.stderr)
        return 2
    itens = carregar(raiz)
    usados = [int(m.group(1)) for c in itens if (m := re.match(r"CONST-(\d+)$", c["id"]))]
    ident = f"CONST-{max(usados, default=0) + 1:04d}"
    itens.append({
        "id": ident,
        "titulo": args.titulo,
        "origem": args.origem or "",
        "escopo": args.escopo or "**",
        "severidade": args.severidade,
        "validador": args.validador or "",
        "inverter": bool(args.inverter),
        "requisito": args.requisito or "",
        "estado": "ativa",
        "criada_em": date.today().isoformat(),
        "supersede": "",
        "motivo_da_revogacao": "",
    })
    gravar(raiz, itens)
    print(f"{ident} {args.severidade}: {args.titulo}"
          + ("" if args.validador else "  (sem validador: entra como manual, o C-GATE devolve INCONCLUSIVA)"))
    return 0


def revogar(args, raiz: Path) -> int:
    """Revogar preserva o historico: a restricao fica, com estado e motivo."""
    itens = carregar(raiz)
    alvo = next((c for c in itens if c["id"] == args.id), None)
    if alvo is None:
        print(f"erro: {args.id} não existe", file=sys.stderr)
        return 2
    alvo["estado"] = "revogada"
    alvo["motivo_da_revogacao"] = args.motivo
    alvo["supersede"] = args.por or ""
    gravar(raiz, itens)
    print(f"{args.id} revogada" + (f", superada por {args.por}" if args.por else ""))
    return 0


def listar(args, raiz: Path) -> int:
    itens = carregar(raiz)
    if args.json:
        print(json.dumps(itens, ensure_ascii=False))
        return 0
    if not itens:
        print("nenhuma restrição registrada")
        return 0
    for c in itens:
        marca = "" if c.get("estado") == "ativa" else f"  [{c.get('estado')}]"
        val = c.get("validador") or "manual"
        print(f"{c['id']}  {c['severidade']:<11} {c['titulo']}{marca}")
        print(f"            escopo {c.get('escopo', '')} · validador: {val}")
    return 0


def expandir(comando: str) -> str:
    """`${CLAUDE_PLUGIN_ROOT}` no validador aponta para o plugin, nao para o projeto.

    Sem isto, a restricao semeada citava `skills/conversao-wx/scripts/golden.py`
    relativo a raiz do PROJETO -- onde esse arquivo nunca esta.
    """
    raiz_plugin = os.environ.get("CLAUDE_PLUGIN_ROOT") or str(Path(__file__).resolve().parents[3])
    return (comando.replace("${CLAUDE_PLUGIN_ROOT}", raiz_plugin)
                   .replace("$CLAUDE_PLUGIN_ROOT", raiz_plugin))


def programa_ausente(comando: str, raiz: Path) -> str:
    """Devolve o caminho que falta, ou string vazia quando da para rodar."""
    try:
        partes = shlex.split(comando)
    except ValueError as e:
        return f"comando mal formado ({e})"
    if not partes:
        return "comando vazio"
    if shutil.which(partes[0]) is None and not (raiz / partes[0]).is_file():
        return partes[0]
    for arg in partes[1:]:
        if arg.endswith((".py", ".sh", ".ps1")) and not Path(arg).is_absolute():
            if not (raiz / arg).is_file() and not Path(arg).is_file():
                return arg
        elif arg.endswith((".py", ".sh", ".ps1")) and not Path(arg).is_file():
            return arg
    return ""


def c_gate(args, raiz: Path) -> int:
    """Roda os validadores das restricoes ativas e devolve o veredito do C-GATE.

    Codigos: 0 aprovado; 1 violada em severidade bloqueante; 2 erro de uso.
    INCONCLUSIVA nao reprova sozinha, mas aparece no resumo e no relatorio --
    quem aprova a Sprint precisa ver o que ninguem conferiu.
    """
    itens = carregar(raiz)
    ativas = [c for c in itens if c.get("estado") == "ativa"]
    linhas = []
    for c in ativas:
        if not c.get("validador"):
            linhas.append({"id": c["id"], "titulo": c["titulo"], "severidade": c["severidade"],
                           "resultado": "inconclusiva", "motivo": "sem validador automático (manual)",
                           "ms": 0})
            continue
        t0 = time.monotonic()
        comando = expandir(c["validador"])
        faltando = programa_ausente(comando, raiz)
        if faltando:
            # rodou e deu erro NAO e o mesmo que nao ter rodado. Um validador que
            # aponta para arquivo inexistente devolvia "violada" -- ou seja,
            # acusava o projeto de quebrar uma regra que ninguem conferiu.
            linhas.append({"id": c["id"], "titulo": c["titulo"], "severidade": c["severidade"],
                           "resultado": "inconclusiva",
                           "motivo": f"validador não existe: {faltando}", "ms": 0})
            continue
        try:
            r = subprocess.run(shlex.split(comando), cwd=raiz, capture_output=True,
                               text=True, timeout=args.timeout)
            saida = (r.stdout + r.stderr).strip().splitlines()
            # `grep` sai 1 quando NAO acha -- entao um validador de "nao existe
            # segredo aqui" acusa violacao justamente quando o projeto esta
            # limpo. Com --inverter, sair 0 significa que o validador ACHOU o
            # problema. E flag explicita porque adivinhar isso pelo texto do
            # comando seria magica, e magica em portao e o pior tipo de defeito.
            achou = (r.returncode == 0)
            vale = (not achou) if c.get("inverter") else achou
            linhas.append({"id": c["id"], "titulo": c["titulo"], "severidade": c["severidade"],
                           "resultado": "aprovada" if vale else "violada",
                           "motivo": (saida[-1][:200] if saida else f"código {r.returncode}"),
                           "ms": round((time.monotonic() - t0) * 1000)})
        except subprocess.TimeoutExpired:
            linhas.append({"id": c["id"], "titulo": c["titulo"], "severidade": c["severidade"],
                           "resultado": "inconclusiva", "motivo": f"validador estourou {args.timeout}s",
                           "ms": round((time.monotonic() - t0) * 1000)})
        except (OSError, ValueError) as e:
            linhas.append({"id": c["id"], "titulo": c["titulo"], "severidade": c["severidade"],
                           "resultado": "inconclusiva", "motivo": f"validador não executou: {e}",
                           "ms": round((time.monotonic() - t0) * 1000)})
    violadas = [l for l in linhas if l["resultado"] == "violada"]
    bloqueantes = [l for l in violadas if l["severidade"] == "bloqueante"]
    inconclusivas = [l for l in linhas if l["resultado"] == "inconclusiva"]
    veredito = "REPROVADO" if bloqueantes else ("APROVADO_COM_RESSALVA" if violadas or inconclusivas else "APROVADO")
    resumo = {"c_gate": veredito, "total": len(linhas), "violadas": [l["id"] for l in violadas],
              "bloqueantes": [l["id"] for l in bloqueantes],
              "inconclusivas": [l["id"] for l in inconclusivas], "itens": linhas}
    destino = raiz / ".wx-migration" / "c-gate.json"
    destino.parent.mkdir(parents=True, exist_ok=True)
    destino.write_text(json.dumps(resumo, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if args.json:
        print(json.dumps(resumo, ensure_ascii=False))
    else:
        print(f"C-GATE: {veredito}  ({len(linhas)} restrições ativas)")
        for l in linhas:
            print(f"  {l['resultado'].upper():<13} {l['id']} {l['titulo']}  {l['motivo']}")
        if inconclusivas:
            print(f"\n  {len(inconclusivas)} inconclusiva(s): ninguém conferiu. Isso não é aprovação.")
    return 1 if bloqueantes else 0


# As restricoes que o proprio questionario ja implica. Nao entram sozinhas: a
# regra da casa e que guarda nova entra PEDIDA. `semear` propoe, o dono aceita.
def semear(args, raiz: Path) -> int:
    cfg_p = raiz / ".wx-migration" / "conversion.config.json"
    if not cfg_p.is_file():
        print("erro: sem .wx-migration/conversion.config.json; rode o questionário antes", file=sys.stderr)
        return 2
    cfg = json.loads(cfg_p.read_text(encoding="utf-8"))
    fid = cfg.get("fidelity", {})
    propostas: list[dict] = []
    if not fid.get("allowed_modernizations"):
        propostas.append({
            "titulo": "Nenhuma modernização não aprovada: o destino reproduz o comportamento do legado",
            "origem": "conversion.config.json fidelity.allowed_modernizations = []",
            "escopo": "**", "severidade": "bloqueante", "validador": ""})
    if fid.get("data_behavior") == "identical":
        propostas.append({
            "titulo": "Resultado numérico e de dados idêntico ao legado, dentro da tolerância declarada",
            "origem": "conversion.config.json fidelity.data_behavior = identical",
            # sem validador de proposito: o comando do golden depende do projeto
            # (qual binario recebe os casos), e um validador que so roda `--help`
            # aprovaria sem conferir nada -- que e exatamente o que este portao
            # existe para impedir. O dono aponta o comando real com --validador.
            "escopo": "**", "severidade": "bloqueante", "validador": ""})
    if fid.get("ui") in ("behavioral", "identical"):
        propostas.append({
            "titulo": "Fluxo de tela e atalhos preservados conforme a letra F do questionário",
            "origem": "conversion.config.json fidelity.ui",
            "escopo": "**", "severidade": "grave", "validador": ""})
    propostas.append({
        "titulo": "Nenhum segredo em texto puro no repositório nem em log",
        "origem": "regra do projeto (CLAUDE.md)", "escopo": "**",
        "severidade": "bloqueante", "validador": ""})
    propostas.append({
        "titulo": "Toda regra de negócio convertida cita arquivo e localizador do legado",
        "origem": "regra do projeto (CLAUDE.md)", "escopo": "**",
        "severidade": "grave",
        "validador": "python3 ${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/validate_traceability.py .wx-migration/traceability.csv"})

    itens = carregar(raiz)
    ja = {c["titulo"] for c in itens}
    novas = [x for x in propostas if x["titulo"] not in ja]
    if not args.aplicar:
        print(f"{len(novas)} restrição(ões) a propor (nenhuma gravada; use --aplicar):")
        for x in novas:
            print(f"  {x['severidade']:<11} {x['titulo']}")
            print(f"              origem: {x['origem']}"
                  + ("" if x["validador"] else "  · sem validador: entraria como manual"))
        return 0
    for x in novas:
        ns = argparse.Namespace(titulo=x["titulo"], origem=x["origem"], escopo=x["escopo"],
                                severidade=x["severidade"], validador=x["validador"],
                                inverter=x.get("inverter", False), requisito="")
        criar(ns, raiz)
    if not novas:
        print("nada a semear: as restrições implicadas pelo questionário já estão no registro")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--project-root", default=".")
    p.add_argument("--json", action="store_true")
    sub = p.add_subparsers(dest="cmd", required=True)

    c = sub.add_parser("criar", help="registra uma restrição")
    c.add_argument("--titulo", required=True)
    c.add_argument("--origem", help="ADR-0002, requisito, norma, decisão em ata…")
    c.add_argument("--escopo", help="glob dos arquivos que a restrição alcança")
    c.add_argument("--severidade", required=True, choices=SEVERIDADES)
    c.add_argument("--validador", help="comando que sai 0 quando a regra vale")
    c.add_argument("--inverter", action="store_true",
                   help="o validador sai 0 quando ACHA o problema (caso do grep, que sai 1 sem achar)")
    c.add_argument("--requisito")

    sub.add_parser("listar", help="lista as restrições")

    s = sub.add_parser("semear", help="propõe as restrições que o questionário já implica")
    s.add_argument("--aplicar", action="store_true", help="grava; sem isto apenas propõe")

    g = sub.add_parser("c-gate", help="roda os validadores; sai 1 se violar bloqueante")
    g.add_argument("--timeout", type=int, default=300)

    r = sub.add_parser("revogar", help="revoga sem apagar o histórico")
    r.add_argument("id")
    r.add_argument("--motivo", required=True)
    r.add_argument("--por", help="id da restrição que a substitui")

    args = p.parse_args()
    raiz = Path(args.project_root).resolve()
    return {"criar": criar, "listar": listar, "c-gate": c_gate, "revogar": revogar,
            "semear": semear}[args.cmd](args, raiz)


try:
    import registro
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
