#!/usr/bin/env python3
"""Testes de regressao do plugin WX Claude Code. Rode: python3 tests/testes.py

Cada teste exercita um script de verdade num diretorio temporario e confere
a saida. Provas que antes eram manuais (questionario -> manifesto -> pre-flight,
roteador de modelos, PDCA, kanban, golden, hook do G0) viram regressao aqui.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[1]
SCRIPTS = RAIZ / "skills" / "conversao-wx" / "scripts"
EXEMPLO = RAIZ / "exemplos" / "estoque-wx"


def run(*args: str, entrada: str | None = None, cwd: Path | None = None) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, *map(str, args)], input=entrada, capture_output=True, text=True, cwd=cwd)


class Questionario(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        shutil.copytree(EXEMPLO / "inputs", self.tmp / "inputs")
        (self.tmp / ".wx-migration").mkdir()
        shutil.copy(EXEMPLO / "questionario.json", self.tmp / ".wx-migration" / "questionario.json")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_aplica_e_nao_sobrescreve(self):
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("modo=inventory", r.stdout)
        m = json.loads((self.tmp / ".wx-migration/wx-inputs.manifest.json").read_text())
        self.assertEqual(m["artifacts"]["sql_scripts"]["status"], "provided")
        self.assertEqual(m["evidence_root"], "../inputs")
        self.assertTrue((self.tmp / "DESIGN.md").exists())
        design = (self.tmp / "DESIGN.md").read_text()
        for secao in ("Grids (F3", "Formulários (F4", "Números, datas e moeda (F5", "Acessibilidade (F8"):
            self.assertIn(secao, design)
        self.assertIn("F2 novo", design); self.assertIn("Edição na célula: sim", design)
        self.assertIn("| selecionar | SELECIONAR REGISTRO | check | #1F5FBF |", design)
        self.assertIn("Barra da grade: acima da grade, alinhada à direita", design)
        self.assertIn("«Confirma a exclusão do registro?»", design)
        self.assertIn("Tipo: textura", design)
        self.assertIn("Modo Impeccable: **Operate**", (self.tmp / "PRODUCT.md").read_text())
        self.assertIn("Estilo de resposta", (self.tmp / "CLAUDE.md").read_text())
        # bloco 0: empresa, governanca para o PMO e entrega sem senha
        empresa = (self.tmp / ".wx-migration/empresa.md").read_text()
        self.assertIn("Boller Sistemas Ltda", empresa); self.assertIn("Blumenau - SC", empresa)
        self.assertIn("marca/logotipo-estoque.svg (provided)", empresa); self.assertIn("| Maria Souza | Diretora comercial |", empresa)
        self.assertIn("**Prazo final de entrega: 2026-12-18**", (self.tmp / ".wx-migration/pmo/cronograma.md").read_text())
        self.assertIn("e1 --> e2", (self.tmp / ".wx-migration/pmo/fluxograma.md").read_text())
        self.assertIn("| RSK-002 |", (self.tmp / ".wx-migration/pmo/riscos.md").read_text())
        entrega = json.loads((self.tmp / ".wx-migration/entrega.json").read_text())
        self.assertEqual(entrega["credencial_ref"], "GITHUB_TOKEN"); self.assertEqual(entrega["github"]["usuario"], "adrianoboller")
        self.assertNotIn("abc", json.dumps(entrega))
        r2 = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r2.stdout.count("SKIPPED"), 14)

    def test_senha_em_texto_puro_e_recusada(self):
        q = json.loads((self.tmp / ".wx-migration/questionario.json").read_text())
        q["0_empresa_e_projeto"]["0_15_github"]["senha"] = "abc123"
        (self.tmp / ".wx-migration/questionario.json").write_text(json.dumps(q))
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 2)
        self.assertIn("0_15_github.senha", r.stderr)
        self.assertNotIn("abc123", r.stderr + r.stdout)
        self.assertFalse((self.tmp / ".wx-migration/entrega.json").exists())

    def test_pmo_iniciar_le_prazo_e_marcos_do_bloco_0(self):
        run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "iniciar", "--aprovador", "A")
        plano = json.loads((self.tmp / ".wx-migration/pmo/plano.json").read_text())
        self.assertEqual(plano["prazo_final"], "2026-12-18"); self.assertEqual(plano["gates"]["G4"]["previsto_para"], "2026-10-30")
        st = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "status").stdout
        self.assertIn("Prazo final de entrega: 2026-12-18", st); self.assertIn("180000 BRL", st)

    def test_status_provided_sem_arquivo_e_recusado(self):
        q = json.loads((self.tmp / ".wx-migration/questionario.json").read_text())
        q["A_sql"]["arquivos"] = []
        (self.tmp / ".wx-migration/questionario.json").write_text(json.dumps(q))
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 2)
        self.assertIn("provided", r.stderr)

    def test_preflight_do_exemplo_nao_bloqueia_por_schema(self):
        run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        r = run(SCRIPTS / "wx_preflight.py", "--manifest", self.tmp / ".wx-migration/wx-inputs.manifest.json", "--allowed-evidence-root", self.tmp / "inputs", "--workspace-root", self.tmp, "--output", self.tmp / ".wx-migration/preflight")
        # wx_preflight.py devolve 0 para READY e 2 para CONDITIONAL; BLOCKED e erro de schema tambem dao != 0,
        # por isso o status e conferido na saida, nao no codigo.
        self.assertIn('"status": "CONDITIONAL"', r.stdout, r.stderr)
        rel = sorted((self.tmp / ".wx-migration/preflight/runs").glob("*/report.md"))[-1].read_text()
        for codigo in ("INVALID_SIGNATURE", "SQL_METADATA_MISSING", "EVIDENCE_ROOT", "SCHEMA"):
            self.assertNotIn(codigo, rel, f"{codigo} apareceu no pré-flight do exemplo")


class Roteador(unittest.TestCase):
    def rotear(self, *args):
        r = run(SCRIPTS / "rotear_modelo.py", *args)
        return json.loads(r.stdout), r.returncode

    def test_classes_e_escaladas(self):
        d, _ = self.rotear("--classe", "mecanica"); self.assertEqual(d["modelo"], "haiku")
        d, _ = self.rotear("--classe", "analise", "--sinal", "fiscal"); self.assertEqual(d["modelo"], "opus")
        d, _ = self.rotear("--classe", "mecanica", "--sinal", "conflito"); self.assertEqual(d["modelo"], "sonnet")
        d, _ = self.rotear("--classe", "revisao", "--sinal", "padrao-aprovado"); self.assertEqual((d["modelo"], d["effort"]), ("opus", "max"))
        d, _ = self.rotear("--classe", "decisao", "--indisponivel", "opus"); self.assertEqual(d["modelo"], "sonnet"); self.assertTrue(any("fallback" in m for m in d["motivos"]))

    def test_orcamento_rebaixa_e_bloqueia(self):
        tmp = Path(tempfile.mkdtemp())
        try:
            run(SCRIPTS / "pmo.py", "--project-root", tmp, "iniciar")
            orc = tmp / ".wx-migration/pmo/orcamento.json"
            o = json.loads(orc.read_text()); o["gates"]["G1"]["tokens_previstos"] = 1000; orc.write_text(json.dumps(o))
            run(SCRIPTS / "pmo.py", "--project-root", tmp, "gastar", "--gate", "G1", "--modelo", "sonnet", "--tokens", "850")
            d, rc = self.rotear("--project-root", tmp, "--classe", "decisao", "--gate", "G1")
            self.assertEqual((d["modelo"], rc), ("sonnet", 0))
            run(SCRIPTS / "pmo.py", "--project-root", tmp, "gastar", "--gate", "G1", "--modelo", "opus", "--tokens", "200")
            d, rc = self.rotear("--project-root", tmp, "--classe", "analise", "--gate", "G1")
            self.assertEqual((d["estado"], rc), ("BLOQUEADO", 3))
        finally:
            shutil.rmtree(tmp, ignore_errors=True)


class PMO(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        (self.tmp / ".wx-migration").mkdir()
        (self.tmp / ".wx-migration/traceability.csv").write_text(
            "trace_id,kind,source_artifact,source_locator,source_sha256,legacy_symbol,rule_summary,decision_id,target_component,target_file,target_symbol,test_id,test_file,expected,actual,target_commit,test_result_ref,approved_by,approved_at,status,confidence,notes\n"
            + "".join(f"BR-{i:03d},business_rule,,,,,regra {i},,,,,,,,,,,,,implemented,,\n" for i in range(1, 9))
            + "BR-009,business_rule,,,,,regra 9,,,,,,,,,,,Adriano,,accepted,,\n")
        run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "iniciar", "--aprovador", "Adriano")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_pdca_infrutifero_exige_proxima_e_grava_na_base(self):
        run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "pdca", "abrir", "--gate", "G4", "--hipotese", "h1", "--medida", "m", "--criterio", ">= 1,5x")
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "pdca", "fechar", "--id", "PDCA-001", "--resultado", "infrutifero", "--medido", "1,06x", "--aprendizado", "a")
        self.assertEqual(r.returncode, 2); self.assertIn("proxima", r.stderr)
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "pdca", "fechar", "--id", "PDCA-001", "--resultado", "infrutifero", "--medido", "1,06x", "--aprendizado", "a", "--proxima", "h2")
        self.assertEqual(r.returncode, 0, r.stderr)
        base = (self.tmp / ".wx-migration/pmo/base_de_conhecimento.md").read_text()
        self.assertIn("| PDCA-001 |", base); self.assertIn("infrutifero", base)
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "pdca", "fechar", "--id", "PDCA-001", "--resultado", "frutifero", "--medido", "x", "--aprendizado", "a")
        self.assertEqual(r.returncode, 2)

    def test_kanban_marca_wip_estourado(self):
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "kanban")
        self.assertIn("Em andamento (8 / WIP 6)  **WIP ESTOURADO**", r.stdout)

    def test_sprint_uma_por_vez_e_resumo(self):
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "sprint", "abrir", "--nome", "s1", "--objetivo", "o", "--gate", "G4", "--item", "BR-009", "--item", "BR-001")
        self.assertEqual(r.returncode, 0, r.stderr)
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "sprint", "abrir", "--nome", "s2", "--objetivo", "o", "--gate", "G4")
        self.assertEqual(r.returncode, 2)
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "sprint", "fechar", "--decisao", "CONDITIONAL")
        self.assertIn("prontos 1/2", r.stdout)
        resumo = next((self.tmp / ".wx-migration/pmo/sprints").glob("sprint-01-*.md")).read_text()
        self.assertIn("## 12. Retrospectiva", resumo); self.assertIn("BR-001", resumo)

    def test_backlog_com_papel_e_entrega_zipada(self):
        import zipfile
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "sprint", "abrir", "--nome", "s1", "--objetivo", "o", "--gate", "G5", "--item", "BR-001:B", "--item", "BR-009:F")
        self.assertEqual(r.returncode, 0, r.stderr)
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "sprint", "abrir", "--nome", "x", "--objetivo", "o", "--gate", "G5", "--item", "BR-002:Z")
        self.assertEqual(r.returncode, 2)
        kb = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "kanban").stdout
        self.assertIn("[B engenheiro] `BR-001`", kb); self.assertIn("[F prova-real] `BR-009`", kb); self.assertIn("[sem papel] `BR-002`", kb)
        run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "sprint", "fechar", "--decisao", "APPROVED")
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "entregar", "--sprint", "1", "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 0, r.stderr)
        z = next((self.tmp / ".wx-migration/pmo/entregas").glob("sprint-01-G5-*.zip"))
        nomes = zipfile.ZipFile(z).namelist()
        for n in ("sprint-01/resumo-da-sprint.md", "sprint-01/tecnicas-aplicadas.md", "sprint-01/base-de-conhecimento.md", "sprint-01/ferramentas.md", "sprint-01/kanban.md", "sprint-01/backlog.md"):
            self.assertIn(n, nomes)
        ferr = zipfile.ZipFile(z).read("sprint-01/ferramentas.md").decode()
        self.assertIn("pmo.py", ferr); self.assertIn("golden.py", ferr)
        tec = zipfile.ZipFile(z).read("sprint-01/tecnicas-aplicadas.md").decode()
        self.assertIn("## PDCA", tec); self.assertIn("Fonte:", tec)

    def test_status_sem_numero_inventado(self):
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "status")
        self.assertIn("1/9 = 11.1%", r.stdout)
        self.assertIn("| G1 | 0 | 0 | — |", r.stdout)


class Golden(unittest.TestCase):
    def test_compara_com_tolerancia(self):
        tmp = Path(tempfile.mkdtemp())
        try:
            run(SCRIPTS / "golden.py", "capturar", "--casos", EXEMPLO / "inputs/dados-de-amostra/resultados-esperados.json", "--saida", tmp / "g.json")
            res = {"resultados": [{"id": "TST-BR-001-a", "resultado": 249.382}, {"id": "TST-BR-004-b", "resultado": [33.33, 33.33, 33.34]}, {"id": "TST-BR-005-b", "resultado": True}]}
            (tmp / "r.json").write_text(json.dumps(res))
            r = run(SCRIPTS / "golden.py", "comparar", "--golden", tmp / "g.json", "--resultados", tmp / "r.json", "--relatorio", tmp / "c.json")
            self.assertEqual(r.returncode, 1)
            rel = json.loads((tmp / "c.json").read_text())
            por = {c["id"]: c["passou"] for c in rel["casos"]}
            self.assertTrue(por["TST-BR-001-a"]); self.assertTrue(por["TST-BR-004-b"]); self.assertFalse(por["TST-BR-005-b"])
            self.assertEqual(rel["equivalencia"], "2/10")
        finally:
            shutil.rmtree(tmp, ignore_errors=True)


class ExtrairPdf(unittest.TestCase):
    def test_extrai_com_localizador(self):
        try:
            import pypdf  # noqa: F401
        except ImportError:
            self.skipTest("pypdf ausente")
        tmp = Path(tempfile.mkdtemp())
        try:
            shutil.copytree(EXEMPLO / "inputs", tmp / "inputs"); (tmp / ".wx-migration").mkdir()
            shutil.copy(EXEMPLO / "questionario.json", tmp / ".wx-migration/questionario.json")
            run(SCRIPTS / "aplicar_questionario.py", "--questionario", tmp / ".wx-migration/questionario.json", "--project-root", tmp, "--plugin-root", RAIZ)
            r = run(SCRIPTS / "extrair_pdf.py", "--manifest", tmp / ".wx-migration/wx-inputs.manifest.json", "--allowed-evidence-root", tmp / "inputs", "--output", tmp / ".wx-migration/evidence/pdf-text")
            self.assertEqual(r.returncode, 0, r.stderr)
            s = json.loads((tmp / ".wx-migration/evidence/pdf-text/sumario.json").read_text())
            self.assertEqual(len(s["arquivos"]), 4); self.assertEqual(s["ocr_required"], [])
            self.assertIn("CalculaDesconto", (tmp / ".wx-migration/evidence/pdf-text/estoque-codigo/0001.txt").read_text())
        finally:
            shutil.rmtree(tmp, ignore_errors=True)


class HookG0(unittest.TestCase):
    def hook(self, tmp: Path, arquivo: str):
        return run(RAIZ / "hooks/portao_g0.py", entrada=json.dumps({"tool_name": "Write", "tool_input": {"file_path": str(tmp / arquivo)}, "cwd": str(tmp)}))

    def test_nega_fora_do_wx_migration_quando_blocked(self):
        tmp = Path(tempfile.mkdtemp())
        try:
            run_dir = tmp / ".wx-migration/preflight/runs/run-1"; run_dir.mkdir(parents=True)
            (run_dir / "report.json").write_text(json.dumps({"status": "BLOCKED", "summary": {"errors": 3}}))
            r = self.hook(tmp, "src/main.rs"); self.assertIn('"deny"', r.stdout)
            r = self.hook(tmp, ".wx-migration/gaps.md"); self.assertEqual(r.stdout, "")
            (run_dir / "report.json").write_text(json.dumps({"status": "READY"}))
            r = self.hook(tmp, "src/main.rs"); self.assertEqual(r.stdout, "")
        finally:
            shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    unittest.main(verbosity=1)
