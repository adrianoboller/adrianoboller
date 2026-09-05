#!/usr/bin/env python3
"""Teste de fluxo: o caminho inteiro num projeto novo, do zero a entrega.

A bateria (testes.py) prova cada peca isolada. Este prova a LIGACAO entre elas,
que e onde os defeitos deste projeto sempre apareceram: o esqueleto ERP que
quebrou a contagem de SKIPPED, o .env.exemplo comido pela regra do .env, o
artefato declarado que nao existia. Peca certa, ligacao errada.

Roda numa pasta temporaria, com o exemplo ESTOQUE, e mede cada passo. Nao
altera o repositorio.

Uso: python3 tests/fluxo.py [--manter]
"""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[1]
SCRIPTS = RAIZ / "skills/conversao-wx/scripts"
EXEMPLO = RAIZ / "exemplos/estoque-wx"


class Fluxo:
    def __init__(self) -> None:
        self.projeto = Path(tempfile.mkdtemp(prefix="wx-fluxo-"))
        self.saida = Path(tempfile.mkdtemp(prefix="wx-entrega-"))
        self.passos: list[dict] = []

    def roda(self, *args, entrada: str | None = None) -> subprocess.CompletedProcess:
        return subprocess.run([sys.executable, *[str(a) for a in args]], capture_output=True,
                              text=True, timeout=600, input=entrada, cwd=self.projeto)

    def passo(self, nome: str, funcao, esperado: str = "") -> bool:
        t0 = time.monotonic()
        try:
            ok, detalhe = funcao()
        except Exception as e:  # noqa: BLE001 - o passo falha, o fluxo segue e relata
            ok, detalhe = False, f"{type(e).__name__}: {e}"
        ms = round((time.monotonic() - t0) * 1000)
        self.passos.append({"passo": nome, "ok": ok, "detalhe": detalhe, "ms": ms, "esperado": esperado})
        marca = "ok   " if ok else "FALHA"
        print(f"  {marca} {nome:<38} {ms:>6} ms  {detalhe}")
        return ok


def main() -> int:
    f = Fluxo()
    print(f"Teste de fluxo do WX Claude Code\n  projeto: {f.projeto}\n  entrega: {f.saida}\n")

    def preparar():
        shutil.copytree(EXEMPLO / "inputs", f.projeto / "inputs")
        (f.projeto / ".wx-migration").mkdir()
        shutil.copy(EXEMPLO / "questionario.json", f.projeto / ".wx-migration/questionario.json")
        return True, "exemplo ESTOQUE copiado"

    def aplicar():
        r = f.roda(SCRIPTS / "aplicar_questionario.py", "--questionario", f.projeto / ".wx-migration/questionario.json",
                   "--project-root", f.projeto, "--plugin-root", RAIZ)
        criados = r.stdout.count("CREATED")
        return r.returncode == 0 and criados > 90, f"{criados} arquivos criados"

    def contexto_da_sessao():
        faltando = [n for n in ("CLAUDE.md", "INDEX_FILES.md", "AGENTS.md", "DESIGN.md",
                                ".wx-migration/respostas_questionario.md", ".wx-migration/prompts/kickoff.md",
                                ".wx-migration/ambiente/backup-e-replicacao.md", "artefatos/LEIA-ME.md",
                                "docs/adr/0001-monolito-modular.md", "docs/skills-recomendadas.md")
                    if not (f.projeto / n).exists()]
        return not faltando, "tudo no lugar" if not faltando else f"faltou: {faltando}"

    def respostas_por_id():
        md = (f.projeto / ".wx-migration/respostas_questionario.md").read_text(encoding="utf-8")
        ids = json.loads(subprocess.run([sys.executable, str(SCRIPTS / "listar_perguntas.py"), "--json"],
                                        capture_output=True, text=True).stdout)
        faltando = [i["id"] for i in ids if f"| `{i['id']}` |" not in md]
        return not faltando, f"{len(ids)} perguntas no indice" if not faltando else f"faltou: {faltando[:5]}"

    def preflight_g0():
        r = f.roda(SCRIPTS / "wx_preflight.py", "--manifest", f.projeto / ".wx-migration/wx-inputs.manifest.json",
                   "--allowed-evidence-root", f.projeto / "inputs", "--workspace-root", f.projeto,
                   "--output", f.projeto / ".wx-migration/preflight")
        try:
            d = json.loads(r.stdout)
        except json.JSONDecodeError:
            return False, r.stderr.strip()[:120]
        rel = json.loads((Path(d["output"]) / "report.json").read_text(encoding="utf-8"))
        erros = rel["counts"]["errors"]
        return d["status"] in ("READY", "CONDITIONAL") and erros == 0, f"{d['status']}, {erros} erros, {rel['counts']['help_documents']} paginas do Help"

    def artefato():
        nota = f.projeto / "nota-da-reuniao.txt"
        nota.write_text("Venda com saldo zero bloqueia. Prazo de troca: 7 dias.\n", encoding="utf-8")
        r = f.roda(SCRIPTS / "arquivar_artefato.py", "--project-root", f.projeto, "--arquivo", nota,
                   "--tipo", "anotacao", "--onde-usar", "G1: regras ditadas pelo cliente",
                   "--questionario", f.projeto / ".wx-migration/questionario.json")
        cat = (f.projeto / "artefatos/CATALOGO.md").read_text(encoding="utf-8")
        return r.returncode == 0 and "nota-da-reuniao.txt" in cat, "arquivado com hash e catalogado"

    def hook_protege_artefato():
        pedido = {"tool_name": "Write", "cwd": str(f.projeto),
                  "tool_input": {"file_path": str(f.projeto / "artefatos/CATALOGO.md"), "content": "x"}}
        p = subprocess.run([sys.executable, str(RAIZ / "hooks/guarda_anexos_e_segredos.py")],
                           input=json.dumps(pedido), capture_output=True, text=True)
        negou = "deny" in p.stdout
        return negou, "escrita recusada pelo hook" if negou else "o hook DEIXOU escrever"

    def pdf_para_markdown():
        r = f.roda(SCRIPTS / "pdf_para_markdown.py", "--pdf", f.projeto / "inputs/estoque-codigo.pdf",
                   "--saida", f.projeto / ".wx-migration/extraidos", "--linguagem", "wlanguage",
                   "--project-root", f.projeto)
        md = f.projeto / ".wx-migration/extraidos/estoque-codigo.md"
        tem_pagina = md.is_file() and "<!-- pagina 1 -->" in md.read_text(encoding="utf-8")
        return r.returncode == 0 and tem_pagina, r.stdout.strip().split(" (")[-1].rstrip(")") if r.returncode == 0 else r.stderr[:100]

    def pmo_sprint():
        f.roda(SCRIPTS / "pmo.py", "--project-root", f.projeto, "iniciar")
        f.roda(SCRIPTS / "pmo.py", "--project-root", f.projeto, "bloco", "abrir", "--titulo", "Analise da base de dados", "--gate", "G1")
        r = f.roda(SCRIPTS / "pmo.py", "--project-root", f.projeto, "sprint", "abrir",
                   "--nome", "Inventario inicial", "--objetivo", "Levantar BR-* do modulo de vendas", "--gate", "G1")
        return r.returncode == 0, r.stdout.strip().splitlines()[-1][:90] if r.stdout.strip() else r.stderr[:90]

    def roteia_modelo():
        r = f.roda(SCRIPTS / "rotear_modelo.py", "--classe", "mecanica", "--project-root", f.projeto, "--local-no-ar", "sim")
        d = json.loads(r.stdout)
        r2 = f.roda(SCRIPTS / "rotear_modelo.py", "--classe", "decisao", "--project-root", f.projeto, "--local-no-ar", "sim")
        d2 = json.loads(r2.stdout)
        certo = d["modelo"] == "local" and d2["modelo"] == "opus"
        return certo, f"mecanica={d['modelo']}, decisao={d2['modelo']}"

    def rag():
        f.roda(SCRIPTS / "rag.py", "--project-root", f.projeto, "indexar")
        r = f.roda(SCRIPTS / "rag.py", "--project-root", f.projeto, "buscar", "backup")
        tem = "#" in r.stdout and r.returncode == 0
        return tem, "busca devolve arquivo#linha" if tem else r.stderr[:100]

    def exportar():
        r = f.roda(SCRIPTS / "exportar_projeto.py", "--project-root", f.projeto, "--destino", f.saida)
        pastas = sorted(p.name for p in f.saida.rglob("*") if p.is_dir() and p.name[:1].isdigit())
        segredo = [p.name for p in f.saida.rglob(".env")]
        return r.returncode == 0 and len(pastas) >= 7 and not segredo, f"{len(pastas)} partes, nenhum .env"

    def registro():
        logs = sorted((f.projeto / ".wx-migration/logs").glob("plugin-*.jsonl"))
        if not logs:
            return False, "nenhum log"
        linhas = [json.loads(l) for l in logs[0].read_text(encoding="utf-8").splitlines()]
        ops = {i["operacao"] for i in linhas}
        esperadas = {"aplicar_questionario", "wx_preflight", "arquivar_artefato", "pdf_para_markdown", "pmo", "rotear_modelo", "rag", "exportar_projeto"}
        faltando = esperadas - ops
        if faltando:
            return False, f"faltou registrar: {faltando}"
        # Codigo != 0 nem sempre e falha: o G0 devolve 2 para CONDITIONAL por
        # contrato, e a negativa de hook e o passo 6 fazendo o que deve. Separar
        # os dois evita mandar alguem cacar um defeito que nao existe.
        previstos = {("wx_preflight", 2), ("HOOK_guarda_anexos", 1), ("HOOK_portao_g0", 1)}
        inesperados = [f"{i['operacao']}={i['codigo']}" for i in linhas
                       if i.get("codigo") and (i["operacao"], i["codigo"]) not in previstos]
        return not inesperados, (f"{len(linhas)} operacoes, nenhum codigo inesperado"
                                 if not inesperados else f"codigo inesperado: {inesperados}")

    for nome, funcao in (
        ("0. preparar o projeto", preparar),
        ("1. aplicar o questionario", aplicar),
        ("2. contexto da primeira sessao", contexto_da_sessao),
        ("3. respostas por id", respostas_por_id),
        ("4. G0 pre-flight", preflight_g0),
        ("5. artefato do cliente", artefato),
        ("6. hook protege os artefatos", hook_protege_artefato),
        ("7. PDF vira markdown citavel", pdf_para_markdown),
        ("8. PMO: bloco e sprint", pmo_sprint),
        ("9. roteador de modelo", roteia_modelo),
        ("10. RAG do projeto", rag),
        ("11. exportar a entrega", exportar),
        ("12. registro das operacoes", registro),
    ):
        f.passo(nome, funcao)

    ok = sum(1 for p in f.passos if p["ok"])
    total = len(f.passos)
    ms = sum(p["ms"] for p in f.passos)
    print(f"\n{ok}/{total} passos, {ms/1000:.1f}s no total")
    (Path(tempfile.gettempdir()) / "wx-fluxo.json").write_text(
        json.dumps({"passos": f.passos, "ok": ok, "total": total, "ms": ms}, ensure_ascii=False, indent=2), encoding="utf-8")
    if "--manter" not in sys.argv:
        shutil.rmtree(f.projeto, ignore_errors=True)
        shutil.rmtree(f.saida, ignore_errors=True)
    else:
        print(f"\nmantidos: {f.projeto} e {f.saida}")
    return 0 if ok == total else 1


if __name__ == "__main__":
    raise SystemExit(main())
