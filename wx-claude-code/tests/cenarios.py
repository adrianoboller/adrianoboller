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

import csv
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
EXEMPLO_PHP = RAIZ / "exemplos/faturamento-php"


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


def c13_legado_php_de_verdade():
    """O outro exemplo inteiro: PHP procedural de 2009, sem nada de WX, ate o G0.

    Nao e o cenario 02 de novo: la o legado PHP era declarado sobre os anexos do
    exemplo WX. Aqui o projeto E PHP -- codigo-fonte no lugar de PDF, MySQL no
    lugar de HFSQL -- e foi ele que achou o G0 supondo WINDEV em oito lugares.
    """
    p = Path(tempfile.mkdtemp(prefix="wx-cen-php-"))
    shutil.copytree(EXEMPLO_PHP / "inputs", p / "inputs")
    (p / ".wx-migration").mkdir()
    r = py(SCRIPTS / "aplicar_questionario.py", "--questionario", EXEMPLO_PHP / "questionario.json",
           "--project-root", p, "--plugin-root", RAIZ)
    py(SCRIPTS / "wx_preflight.py", "--manifest", p / ".wx-migration/wx-inputs.manifest.json",
       "--allowed-evidence-root", p / "inputs", "--workspace-root", p,
       "--output", p / ".wx-migration/preflight")
    relatorios = sorted((p / ".wx-migration/preflight/runs").glob("*/report.json"))
    rel = json.loads(relatorios[-1].read_text(encoding="utf-8")) if relatorios else {}
    man = json.loads((p / ".wx-migration/wx-inputs.manifest.json").read_text(encoding="utf-8")) if r.returncode == 0 else {}
    fontes = (man.get("artifacts", {}).get("native_project_sources") or {}).get("items", [])
    proc = (p / ".wx-migration/processo-de-conversao.md").read_text(encoding="utf-8") if r.returncode == 0 else ""
    ok = (r.returncode == 0 and rel.get("status") == "CONDITIONAL" and not rel.get("errors")
          and len(fontes) >= 5 and "Rust" in proc)
    shutil.rmtree(p, ignore_errors=True)
    return ok, f"{rel.get('status')} sem erros, {len(fontes)} fontes PHP como evidência central, destino Rust"


def c14_governanca_de_ponta_a_ponta():
    """Os seis portoes novos ligados, num projeto real, na ordem em que se usam.

    Cada um ja tem teste de unidade; este prova a LIGACAO -- que e onde os
    defeitos deste projeto sempre apareceram. O caso e o que a governanca existe
    para pegar: F-GATE verde e C-GATE reprovado.
    """
    p = projeto_novo()
    aplicar(p)
    ambiente = {**os.environ, "CLAUDE_PLUGIN_ROOT": str(RAIZ)}

    def rodar(script, *args):
        return subprocess.run([sys.executable, str(SCRIPTS / script), "--project-root", str(p), *args],
                              capture_output=True, text=True, timeout=300, env=ambiente)

    # 1. as restricoes que o questionario ja implica, pedidas
    rodar("constraints.py", "semear", "--aplicar")
    # 2. uma restricao do projeto que o resultado viola
    rodar("constraints.py", "criar", "--titulo", "API pública não quebra compatibilidade",
          "--severidade", "bloqueante", "--validador", "false", "--origem", "ADR-0021")
    c = json.loads(rodar("constraints.py", "--json", "c-gate").stdout)
    # 3. F-GATE verde: o golden bate inteiro
    rel = p / "comp.json"
    rel.write_text(json.dumps({"total": 6, "passaram": 6,
                               "casos": [{"id": f"C{i}", "passou": True} for i in range(6)]}), encoding="utf-8")
    f = json.loads(rodar("evidencia.py", "--json", "do-golden", str(rel)).stdout)
    # 4. o contrato ativo sai das duas coisas
    decisoes = p / ".wx-migration/decisoes"
    decisoes.mkdir(parents=True, exist_ok=True)
    (decisoes / "DEC-0001.md").write_text("# DEC-0001 — Banco\n- Status: superseded\n- Decisão: MySQL\n", encoding="utf-8")
    (decisoes / "DEC-0002.md").write_text("# DEC-0002 — Banco\n- Status: approved\n- Decisão: PostgreSQL\n", encoding="utf-8")
    contrato = json.loads(rodar("contrato.py", "--json", "gerar").stdout)
    # 5. efeito conferido no mundo, nao no codigo de saida
    e = json.loads(rodar("efeito.py", "--json", "conferir", "--acao", "gerar o contrato",
                         "--esperado", "arquivo-existe", "--alvo", ".wx-migration/contrato-ativo.md").stdout)
    # 6. o QA nao conserta o que deveria detectar
    (p / ".wx-migration/papel-da-sessao").write_text("qa\n", encoding="utf-8")
    pedido = json.dumps({"tool_name": "Write", "tool_input": {"file_path": "src/api.rs"}, "cwd": str(p)})
    amb = {k: v for k, v in ambiente.items() if k != "WX_PAPEL"}
    hook = subprocess.run([sys.executable, str(RAIZ / "hooks/papel_da_sessao.py")],
                          input=pedido, capture_output=True, text=True, env=amb)

    ok = (c["c_gate"] == "REPROVADO" and "CONST-0006" in c["bloqueantes"]
          and f["estado"] == "verificado"
          and [d["id"] for d in contrato["decisoes_vigentes"]] == ["DEC-0002"]
          and e["resultado"] == "verificado"
          and "deny" in hook.stdout)
    shutil.rmtree(p, ignore_errors=True)
    return ok, "F-GATE verde e C-GATE reprovado no mesmo projeto; contrato só com a decisão vigente; QA barrado no produto"


def c15_grafo_acha_a_lacuna():
    """O grafo sobre um projeto de verdade: a matriz preenchida e o que falta nela.

    O caso e o do dia a dia: alguem escreveu um arquivo que requisito nenhum
    pediu, e uma regra ficou sem teste. Nenhum dos dois aparece lendo o codigo.
    """
    p = projeto_novo()
    aplicar(p)
    (p / "src/regras").mkdir(parents=True, exist_ok=True)
    for arq in ("src/regras/desconto.rs", "src/relatorio_que_ninguem_pediu.rs"):
        (p / arq).write_text("pub fn x() {}\n", encoding="utf-8")
    matriz = p / ".wx-migration/traceability.csv"
    cab = matriz.read_text(encoding="utf-8").splitlines()[0].split(",")
    with matriz.open("w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=cab)
        w.writeheader()
        w.writerow({**{c: "" for c in cab}, "trace_id": "BR-001", "kind": "business_rule",
                    "target_file": "src/regras/desconto.rs", "rule_summary": "teto de desconto",
                    "status": "implemented"})
    r = py(SCRIPTS / "grafo.py", "--project-root", p, "--json", "conferir")
    a = json.loads(r.stdout)["achados"] if r.stdout else {}
    ok = (r.returncode == 1
          and a.get("codigo_sem_requisito") == ["src/relatorio_que_ninguem_pediu.rs"]
          and [x["trace_id"] for x in a.get("requisito_sem_teste", [])] == ["BR-001"])
    shutil.rmtree(p, ignore_errors=True)
    return ok, "achou o arquivo que ninguém pediu e a regra sem teste, sem inventar aresta"


def c16_entrega_auditavel():
    """Os seis itens de auditoria ligados, na ordem em que um cliente regulado pede.

    O caso e a mesa de compras de banco: "de onde veio isto, quem decidiu o que,
    o que a sprint provava e o que a maquina prova". O que importa aqui nao e so
    cada documento sair -- e cada um DECLARAR o proprio limite, que e o que
    separa documento auditavel de documento decorativo.
    """
    p = projeto_novo()
    aplicar(p)
    ambiente = {**os.environ, "CLAUDE_PLUGIN_ROOT": str(RAIZ)}

    def rodar(script, *args):
        return subprocess.run([sys.executable, str(SCRIPTS / script), "--project-root", str(p), *args],
                              capture_output=True, text=True, timeout=600, env=ambiente)

    (p / "src").mkdir(exist_ok=True)
    (p / "src/regra.rs").write_text("pub fn r() {}\n", encoding="utf-8")
    rodar("procedencia.py", "--plugin-root", str(RAIZ), "tudo")
    slsa = json.loads((p / ".wx-migration/procedencia/slsa-provenance.json").read_text(encoding="utf-8"))
    bom = json.loads((p / ".wx-migration/procedencia/bom-cyclonedx.json").read_text(encoding="utf-8"))
    rodar("replay.py", "capturar", "--id", "DEC-0002", "--titulo", "Arredondamento",
          "--escolhida", "centavos em i64", "--alternativa", "f64",
          "--fonte", ".wx-migration/conversion.config.json")
    rp = json.loads(rodar("replay.py", "--json", "reconferir").stdout)
    rodar("gemeo.py", "fotografar", "--sprint", "SP00001")
    foto = json.loads((p / ".wx-migration/gemeos/SP00001.json").read_text(encoding="utf-8"))
    tel = rodar("telemetria.py", "--json", "exportar")
    spans = json.loads((p / ".wx-migration/telemetria/otlp-spans.json").read_text(encoding="utf-8"))
    at = json.loads(rodar("identidade.py", "--json", "atestado").stdout)

    ok = (slsa["predicate"]["_limites"]["nivel_slsa"] == "INDISPONÍVEL"
          and bom["bomFormat"] == "CycloneDX"
          and rp["pior"] == "estavel"
          and len(foto["hash"]) == 64
          and tel.returncode == 0 and spans["resourceSpans"]
          and at["_limites"]["isto_nao_e"] == "attestation"
          and str(p) not in json.dumps(spans))
    shutil.rmtree(p, ignore_errors=True)
    return ok, "SLSA, BOM, decisão, gêmeo, OTLP e atestado — cada um declarando o próprio limite"


# O C++ do cenario 17 mora aqui, e nao num exemplo novo: sao vinte linhas, e um
# terceiro projeto de exemplo custaria megabytes no pacote para provar a mesma
# coisa que estas vinte provam -- que legado sem WX e sem PHP atravessa o G0.
CPP_DESCONTO = """// Sistema COMERCIAL — regras de precificacao. Delphi virou C++ em 2011.
#include "desconto.h"
#include <stdexcept>
#include <cmath>

// BR-201: desconto maximo por faixa de cliente.
// A (atacado) 30%, B (revenda) 20%, demais 10%.
// Acima do teto o pedido NAO fecha — decisao do comite de precos, 2014.
double CalculaDesconto(double subtotal, double percentual, char faixa) {
    double teto;
    switch (faixa) {
        case 'A': teto = 30.0; break;
        case 'B': teto = 20.0; break;
        default:  teto = 10.0; break;
    }
    if (percentual > teto) {
        throw std::domain_error("desconto acima do teto da faixa");
    }
    return std::round(subtotal * percentual) / 100.0;
}
"""


def c17_legado_cpp():
    """Origem que nao e WX nem PHP: C++17, com o fonte como evidencia central.

    O legado e E/OU e o destino e livre, mas ate a 3.34.0 isso so tinha sido
    medido com PHP. C++ e o caso que prova o `legado_outra`: nenhum produto WX,
    nenhum PDF, e o G0 tem de passar julgando o codigo-fonte.
    """
    p = Path(tempfile.mkdtemp(prefix="wx-cen-cpp-"))
    fonte = p / "inputs/legado-cpp/src"
    fonte.mkdir(parents=True)
    (fonte / "desconto.cpp").write_text(CPP_DESCONTO, encoding="utf-8")
    (fonte / "desconto.h").write_text("#pragma once\ndouble CalculaDesconto(double, double, char);\n", encoding="utf-8")
    (p / "inputs/banco.sql").write_text(
        "CREATE TABLE cliente (ID INT PRIMARY KEY, FAIXA CHAR(1));\n", encoding="utf-8")
    shutil.copytree(EXEMPLO_PHP / "inputs/marca", p / "inputs/marca")
    (p / ".wx-migration").mkdir()

    q = json.loads((EXEMPLO_PHP / "questionario.json").read_text(encoding="utf-8"))
    pr = q["projeto"]
    pr.update({"nome": "COMERCIAL", "produtos": ["outra"], "principal": "outra"})
    pr["legado_php"] = {"tem": False, "raiz": "", "versao": "", "framework": "", "estilo": ""}
    pr["legado_outra"] = {"linguagem": "C++", "versao": "C++17", "framework": "nenhum",
                          "raiz": "./inputs/legado-cpp",
                          "observacao": "migrado de Delphi em 2011; sem teste automatizado"}
    q["A_sql"]["arquivos"] = ["banco.sql"]
    q["H_backend"].update({"perfil": "rust", "linguagem": "Rust", "framework": "Axum"})
    (p / "questionario.json").write_text(json.dumps(q, ensure_ascii=False), encoding="utf-8")

    r = py(SCRIPTS / "aplicar_questionario.py", "--questionario", p / "questionario.json",
           "--project-root", p, "--plugin-root", RAIZ)
    py(SCRIPTS / "wx_preflight.py", "--manifest", p / ".wx-migration/wx-inputs.manifest.json",
       "--allowed-evidence-root", p / "inputs", "--workspace-root", p,
       "--output", p / ".wx-migration/preflight")
    rels = sorted((p / ".wx-migration/preflight/runs").glob("*/report.json"))
    rel = json.loads(rels[-1].read_text(encoding="utf-8")) if rels else {}
    man = json.loads((p / ".wx-migration/wx-inputs.manifest.json").read_text(encoding="utf-8")) if r.returncode == 0 else {}
    fontes = (man.get("artifacts", {}).get("native_project_sources") or {}).get("items", [])
    ok = (r.returncode == 0 and rel.get("status") == "CONDITIONAL" and not rel.get("errors")
          and any(i["path"].endswith("desconto.cpp") for i in fontes))
    shutil.rmtree(p, ignore_errors=True)
    return ok, f"{rel.get('status')} sem erros; {len(fontes)} fontes C++ como evidência central"


def c18_destino_php():
    """Destino PHP a partir de WLanguage: o processo sai em PHP, nao em Rust.

    O destino livre so tinha sido medido indo PARA Rust e Elixir. PHP e o caso
    que fecha o outro lado: o mesmo legado WX, trocando so a letra H.
    """
    def mexer(q):
        q["H_backend"].update({"perfil": "php", "linguagem": "PHP",
                               "framework": "Slim", "banco": "MySQL 8"})
    p = projeto_novo(mexer)
    r = aplicar(p)
    proc = (p / ".wx-migration/processo-de-conversao.md").read_text(encoding="utf-8") if r.returncode == 0 else ""
    ok = (r.returncode == 0 and "## Backend: PHP" in proc
          and "strict_types" in proc and "HReadSeek" in proc)
    shutil.rmtree(p, ignore_errors=True)
    return ok, "letra H em PHP: processo com strict_types e o mapa HReadSeek → repositório"


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
    ("13 legado PHP de verdade", c13_legado_php_de_verdade, "projeto sem nada de WX atravessa o G0"),
    ("14 governança ligada", c14_governanca_de_ponta_a_ponta, "os seis portões novos funcionando juntos"),
    ("15 grafo acha a lacuna", c15_grafo_acha_a_lacuna, "código sem requisito e requisito sem teste"),
    ("16 entrega auditável", c16_entrega_auditavel, "os seis documentos de auditoria, com limites"),
    ("17 legado C++", c17_legado_cpp, "origem que não é WX nem PHP atravessa o G0"),
    ("18 destino PHP", c18_destino_php, "o destino livre também aponta para PHP"),
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
