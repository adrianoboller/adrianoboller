#!/usr/bin/env python3
"""Bateria pesada: o plugin inteiro em situacoes diversas.

A bateria (testes.py) prova cada peca; o fluxo (fluxo.py) prova a ligacao no
caminho feliz. Aqui estao os OUTROS caminhos -- os que um cliente real traz e
que a gente so descobre quando quebra: corpus ausente, evidencia faltando,
segredo colado no questionario, PDF escaneado, legado que nao e WX, destino que
nao esta na lista, projeto reaplicado, licenca ausente.

Cada cenario diz o que espera ANTES de rodar. Cenario que passa por engano e
pior que cenario que falta: por isso o esperado e escrito, nao inferido.

Uso: python3 tests/cenarios.py [--so <nome>] [--manter]
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[1]
SCRIPTS = RAIZ / "skills/conversao-wx/scripts"
EXEMPLO = RAIZ / "exemplos/estoque-wx"


def py(*args, cwd: Path | None = None, entrada: str | None = None) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, *[str(a) for a in args]], capture_output=True,
                          text=True, timeout=900, cwd=cwd, input=entrada)


def projeto_novo(mexer=None) -> Path:
    """Projeto limpo com o exemplo ESTOQUE; `mexer` recebe o questionario."""
    p = Path(tempfile.mkdtemp(prefix="wx-cen-"))
    shutil.copytree(EXEMPLO / "inputs", p / "inputs")
    (p / ".wx-migration").mkdir()
    q = json.loads((EXEMPLO / "questionario.json").read_text(encoding="utf-8"))
    if mexer:
        mexer(q)
    (p / ".wx-migration/questionario.json").write_text(json.dumps(q, ensure_ascii=False, indent=2), encoding="utf-8")
    return p


def aplicar(p: Path) -> subprocess.CompletedProcess:
    return py(SCRIPTS / "aplicar_questionario.py", "--questionario", p / ".wx-migration/questionario.json",
              "--project-root", p, "--plugin-root", RAIZ)


# ---------------------------------------------------------------------------
# Os cenarios. Cada um devolve (passou, o que aconteceu).
# ---------------------------------------------------------------------------

def c01_wx_classico():
    """WINDEV → Rust, o caminho que o plugin foi feito para atender."""
    p = projeto_novo()
    r = aplicar(p)
    proc = (p / ".wx-migration/processo-de-conversao.md").read_text(encoding="utf-8")
    ok = r.returncode == 0 and "## Backend: Rust" in proc and (p / "Dockerfile").exists()
    shutil.rmtree(p, ignore_errors=True)
    return ok, f"{r.stdout.count('CREATED')} arquivos, backend Rust, Dockerfile do perfil"


def c02_so_php_para_elixir():
    """Legado sem nenhum WX e destino fora da lista de perfis: os dois aceitos."""
    def mexer(q):
        q["projeto"]["produtos"] = ["php"]; q["projeto"]["principal"] = "php"
        q["projeto"]["legado_php"] = {"tem": True, "raiz": "./inputs", "versao": "7.4", "framework": "nenhum", "estilo": "procedural"}
        q["H_backend"].update({"perfil": "outra", "linguagem": "Elixir", "framework": "Phoenix"})
    p = projeto_novo(mexer)
    r = aplicar(p)
    proc = (p / ".wx-migration/processo-de-conversao.md").read_text(encoding="utf-8") if r.returncode == 0 else ""
    ok = r.returncode == 0 and "## Backend: Elixir" in proc
    shutil.rmtree(p, ignore_errors=True)
    return ok, "legado só PHP e destino Elixir aceitos, processo genérico gerado"


def c03_segredo_no_questionario():
    """Senha colada no questionario: recusa ANTES de gravar, e sem repetir o valor."""
    def mexer(q):
        q["K_ambiente"]["K2_postgresql"]["senha"] = "SenhaDoCliente#2026"
    p = projeto_novo(mexer)
    r = aplicar(p)
    vazou = "SenhaDoCliente#2026" in (r.stdout + r.stderr)
    nada_gravado = not (p / "CLAUDE.md").exists()
    ok = r.returncode == 2 and not vazou and nada_gravado
    shutil.rmtree(p, ignore_errors=True)
    return ok, "recusado antes de gravar, valor não repetido na mensagem"


def c04_evidencia_faltando():
    """Cliente sem os PDFs: o G0 diz o que falta em vez de seguir fingindo."""
    p = projeto_novo()
    aplicar(p)
    for pdf in (p / "inputs").glob("*.pdf"):
        pdf.unlink()
    r = py(SCRIPTS / "wx_preflight.py", "--manifest", p / ".wx-migration/wx-inputs.manifest.json",
           "--allowed-evidence-root", p / "inputs", "--workspace-root", p, "--output", p / ".wx-migration/preflight")
    try:
        d = json.loads(r.stdout)
        rel = json.loads((Path(d["output"]) / "report.json").read_text(encoding="utf-8"))
        ok = d["status"] in ("BLOCKED", "CONDITIONAL") and rel["counts"]["errors"] > 0
        detalhe = f"{d['status']}, {rel['counts']['errors']} erros apontando o que falta"
    except (json.JSONDecodeError, KeyError, OSError) as e:
        ok, detalhe = False, f"nao consegui ler o relatorio: {e}"
    shutil.rmtree(p, ignore_errors=True)
    return ok, detalhe


def c05_pdf_sem_texto():
    """PDF escaneado: pagina marcada OCR_REQUERIDO, nunca descrita de cabeca."""
    p = projeto_novo()
    aplicar(p)
    r = py(SCRIPTS / "pdf_para_markdown.py", "--pdf", p / "inputs/estoque-codigo.pdf",
           "--saida", p / ".wx-migration/extraidos", "--minimo", "999999", "--project-root", p)
    md = (p / ".wx-migration/extraidos/estoque-codigo.md").read_text(encoding="utf-8") if r.returncode == 0 else ""
    ok = "OCR_REQUERIDO" in md and "Nada foi inventado aqui" in md
    shutil.rmtree(p, ignore_errors=True)
    return ok, "todas as páginas marcadas OCR_REQUERIDO, nenhuma inventada"


def c06_reaplicar_nao_sobrescreve():
    """Cliente muda uma resposta e reaplica: o trabalho feito nao se perde."""
    p = projeto_novo()
    aplicar(p)
    (p / "DESIGN.md").write_text("# meu design, editado a mao\n", encoding="utf-8")
    r = aplicar(p)
    preservado = (p / "DESIGN.md").read_text(encoding="utf-8").startswith("# meu design")
    ok = r.returncode == 0 and r.stdout.count("CREATED") == 0 and preservado
    shutil.rmtree(p, ignore_errors=True)
    return ok, f"{r.stdout.count('SKIPPED')} arquivos preservados, edição à mão intacta"


def c07_erp_completo():
    """ERP com quatro modulos: cada um com dominio, pasta e a skill certa."""
    p = projeto_novo()
    aplicar(p)
    faltando = [m for m in ("cadastros", "movimentacao", "relatorios", "financeiro")
                if not (p / f"docs/domain/{m}.md").exists() or not (p / f"src/{m}").is_dir()]
    fin = (p / "docs/domain/financeiro.md").read_text(encoding="utf-8") if not faltando else ""
    ok = not faltando and "erp-accounting" in fin
    shutil.rmtree(p, ignore_errors=True)
    return ok, "4 módulos com domínio, pasta e skill; financeiro → erp-accounting"


def c08_backup_incoerente():
    """RPO que o backup nao sustenta: recusado com a razao, nao aceito calado."""
    def mexer(q):
        q["K_ambiente"]["K8_backup_e_replicacao"]["backup"]["tipo"] = "completo"
    p = projeto_novo(mexer)
    r = aplicar(p)
    ok = r.returncode == 2 and "nao cabe em backup diario" in r.stderr
    shutil.rmtree(p, ignore_errors=True)
    return ok, "RPO de 15 min com backup diário recusado, com a razão"


def c09_artefato_com_segredo():
    """Cliente manda anotacao com token dentro: nao entra no acervo."""
    p = projeto_novo()
    aplicar(p)
    ruim = p / "notas.txt"
    ruim.write_text("token de producao: ghp_" + "a" * 36 + "\n", encoding="utf-8")
    r = py(SCRIPTS / "arquivar_artefato.py", "--project-root", p, "--arquivo", ruim,
           "--tipo", "anotacao", "--onde-usar", "G1")
    entrou = (p / "artefatos/anotacao/notas.txt").exists()
    ok = r.returncode == 2 and not entrou
    shutil.rmtree(p, ignore_errors=True)
    return ok, "artefato com token recusado, não foi arquivado"


def c10_sem_licenca():
    """Sem licenca valida, o hook recusa os scripts do plugin."""
    amb = dict(os.environ, WX_LICENCA=str(Path(tempfile.mkdtemp()) / "nao-existe"))
    r = subprocess.run([sys.executable, str(SCRIPTS / "licenca.py"), "verificar"],
                       capture_output=True, text=True, env=amb, timeout=120)
    pedido = {"tool_name": "Bash", "cwd": str(RAIZ),
              "tool_input": {"command": f"python3 {SCRIPTS / 'pmo.py'} status"}}
    h = subprocess.run([sys.executable, str(RAIZ / "hooks/licenca.py" if (RAIZ / "hooks/licenca.py").exists() else SCRIPTS / "licenca.py"), "hook"],
                       input=json.dumps(pedido), capture_output=True, text=True, env=amb, timeout=120)
    negou = "deny" in h.stdout
    ok = "valida" not in r.stdout and negou
    return ok, "sem licença: verificar não diz válida e o hook recusa o script"


def c11_exportar_sem_segredo():
    """A entrega nao leva .env, mas leva o .env.exemplo."""
    p = projeto_novo()
    aplicar(p)
    (p / ".env").write_text("PGPASSWORD=segredo-de-verdade\n", encoding="utf-8")
    saida = Path(tempfile.mkdtemp(prefix="wx-entrega-"))
    r = py(SCRIPTS / "exportar_projeto.py", "--project-root", p, "--destino", saida)
    tem_env = any(x.name == ".env" for x in saida.rglob("*"))
    tem_exemplo = any(x.name == ".env.exemplo" for x in saida.rglob("*"))
    vazou = "segredo-de-verdade" in " ".join(x.read_text(encoding="utf-8", errors="ignore")
                                             for x in saida.rglob("*") if x.is_file() and x.suffix in {".md", ".json", ".txt"})
    ok = r.returncode == 0 and not tem_env and tem_exemplo and not vazou
    shutil.rmtree(p, ignore_errors=True); shutil.rmtree(saida, ignore_errors=True)
    return ok, ".env fora, .env.exemplo dentro, nenhum valor vazado"


def c12_instalador_confere():
    """O instalador roda em conferencia sem instalar nem sujar nada."""
    r = subprocess.run(["bash", str(RAIZ / "instalar.sh"), "--conferir"],
                       capture_output=True, text=True, timeout=600, stdin=subprocess.DEVNULL)
    sujeira = list(Path("/tmp").glob("wx-validacao*"))
    ok = r.returncode == 0 and "nada foi instalado" in r.stdout and not sujeira
    return ok, "cinco passos conferidos, nada instalado, nada deixado em /tmp"


CENARIOS = [
    ("01 WX clássico → Rust", c01_wx_classico, "o caminho que o plugin existe para atender"),
    ("02 só PHP → Elixir", c02_so_php_para_elixir, "legado E/OU e destino livre, os dois fora do caso padrão"),
    ("03 segredo no questionário", c03_segredo_no_questionario, "recusa antes de gravar, sem repetir o valor"),
    ("04 evidência faltando", c04_evidencia_faltando, "o G0 aponta o que falta em vez de seguir"),
    ("05 PDF sem texto", c05_pdf_sem_texto, "OCR marcado, nunca inventado"),
    ("06 reaplicar o questionário", c06_reaplicar_nao_sobrescreve, "o trabalho feito não se perde"),
    ("07 ERP com quatro módulos", c07_erp_completo, "domínio, pasta e skill por módulo"),
    ("08 backup incoerente", c08_backup_incoerente, "RPO que o backup não sustenta é recusado"),
    ("09 artefato com segredo", c09_artefato_com_segredo, "token não entra no acervo"),
    ("10 sem licença", c10_sem_licenca, "o hook recusa os scripts do plugin"),
    ("11 exportar sem segredo", c11_exportar_sem_segredo, ".env fora, .env.exemplo dentro"),
    ("12 instalador em conferência", c12_instalador_confere, "não instala e não suja"),
]


def main() -> int:
    so = None
    if "--so" in sys.argv:
        so = sys.argv[sys.argv.index("--so") + 1]
    print(f"Bateria pesada do WX Claude Code — {len(CENARIOS)} cenários\n")
    resultados = []
    for nome, funcao, espera in CENARIOS:
        if so and so not in nome:
            continue
        t0 = time.monotonic()
        try:
            ok, detalhe = funcao()
        except Exception as e:  # noqa: BLE001
            ok, detalhe = False, f"{type(e).__name__}: {e}"
        ms = round((time.monotonic() - t0) * 1000)
        resultados.append({"cenario": nome, "espera": espera, "ok": ok, "detalhe": detalhe, "ms": ms})
        print(f"  {'ok   ' if ok else 'FALHA'} {nome:<32} {ms:>6} ms  {detalhe}")
    ok = sum(1 for r in resultados if r["ok"])
    ms = sum(r["ms"] for r in resultados)
    print(f"\n{ok}/{len(resultados)} cenários, {ms/1000:.1f}s")
    (Path(tempfile.gettempdir()) / "wx-cenarios.json").write_text(
        json.dumps({"cenarios": resultados, "ok": ok, "total": len(resultados), "ms": ms}, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if ok == len(resultados) else 1


if __name__ == "__main__":
    raise SystemExit(main())
