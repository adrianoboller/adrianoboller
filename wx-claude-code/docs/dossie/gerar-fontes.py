#!/usr/bin/env python3
"""Gera FONTES.md: o inventario do que existe no pacote, medido no repositorio.

Nao e uma lista escrita a mao (que envelhece calada): conta e mede na hora, e o
teste confere que o arquivo esta em dia com o que ha em disco.

Uso: python3 docs/dossie/gerar-fontes.py [saida.md]
"""
from __future__ import annotations

import hashlib
import subprocess
import sys
from datetime import date
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]

GRUPOS = [
    ("Comandos", "commands/*.md", "um por recurso; `/wx-claude-code:<nome>` invoca cada um"),
    ("Agentes", "agents/*.md", "conversão, papéis PDCA, Impeccable e a equipe prioritária"),
    ("Skills", "skills/*/SKILL.md", "conversão, PHP, PDF, laudo de tokens, Impeccable e as oito de ERP"),
    ("Scripts", "skills/conversao-wx/scripts/*.py", "o que faz o trabalho: questionário, gates, PMO, licença, RAG, registro"),
    ("Hooks", "hooks/*.py", "as guardas que rodam nos eventos do Claude Code"),
    ("Referências", "skills/conversao-wx/references/*.md", "o que os agentes leem antes de decidir"),
    ("Modelos", "skills/conversao-wx/templates/*", "questionário, CLAUDE.md e matriz que viram o projeto do cliente"),
    ("Testes", "tests/*.py", "a bateria; o validador estrito a roda"),
    ("Exemplo", "exemplos/estoque-wx/**/*", "projeto sintético que é o teste de regressão do fluxo inteiro"),
    ("Exemplo PHP", "exemplos/faturamento-php/**/*", "legado PHP procedural sintético: o segundo exemplo, sem nada de WX"),
    ("Documentos", "*.md", "manual, README, fontes e a instrução de ativação, na raiz"),
    ("Documentos de apoio", "docs/**/*.md", "relatório, análises, origens dos prints e o vídeo"),
    ("Instaladores", "instalar.*", "bash para Linux e macOS, PowerShell para Windows"),
]


def linhas_py(p: Path) -> int:
    try:
        return len(p.read_text(encoding="utf-8", errors="ignore").splitlines())
    except OSError:
        return 0


def main() -> int:
    saida = Path(sys.argv[1]) if len(sys.argv) > 1 else RAIZ / "FONTES.md"
    versao = __import__("json").loads((RAIZ / ".claude-plugin/plugin.json").read_text(encoding="utf-8"))["version"]
    L = [f"# Fontes do WX Claude Code {versao}", "",
         f"Inventário medido em {date.today().isoformat()} por `docs/dossie/gerar-fontes.py`. "
         "Não se edita à mão: rode o script depois de acrescentar arquivo, e o teste `test_fontes_md_esta_em_dia` avisa quando ele envelhece.", "",
         "| grupo | arquivos | linhas | o que é |", "| --- | ---: | ---: | --- |"]
    total_arq = total_lin = 0
    for nome, padrao, oque in GRUPOS:
        arquivos = [p for p in RAIZ.glob(padrao) if p.is_file() and "__pycache__" not in str(p)]
        lin = sum(linhas_py(p) for p in arquivos if p.suffix in {".py", ".md", ".json", ".csv", ".sh", ".ps1", ".sql"})
        total_arq += len(arquivos)
        total_lin += lin
        # o separador de milhar so no numero: trocar virgula na linha inteira
        # comeria as virgulas do texto (ja comeu uma vez)
        L.append(f"| {nome} | {len(arquivos)} | {f'{lin:,}'.replace(',', '.')} | {oque} |")
    L.append(f"| **total** | **{total_arq}** | **{f'{total_lin:,}'.replace(',', '.')}** | |")
    corpus = RAIZ / "skills/conversao-wx/resources/Help_WL_12k_Json.zip"
    L += ["", "## O que não é fonte, mas vem no pacote", "",
          f"- **Corpus do Help WLanguage**: `skills/conversao-wx/resources/Help_WL_12k_Json.zip`, "
          + (f"{corpus.stat().st_size // 1048576} MB, SHA-256 `{hashlib.sha256(corpus.read_bytes()).hexdigest()[:16]}…`. "
             if corpus.is_file() else "**ausente neste pacote** (parte 2). ")
          + "Uso privado; não é redistribuível.",
          "- **Prints e vídeo**: `docs/prints/` e `docs/video/`, saída de sessões reais, sem edição.",
          "- **PDFs**: `docs/*.pdf` e `docs/dossie/*.pdf`, gerados dos HTML do mesmo repositório.",
          "- **Marca**: `marca-wx-claude-code.png` e `licenca/chave-publica.json` (a de demonstração).", "",
          "## Como instalar", "",
          "```bash", "./instalar.sh                     # Linux e macOS", "```", "",
          "```powershell", ".\\instalar.ps1                    # Windows", "```", "",
          "Os dois fazem o mesmo caminho: pré-requisitos, corpus, validação, instalação no Claude Code e licença. "
          "`--conferir` (ou `-Conferir`) mostra o que aconteceria sem mudar nada.", "",
          "## Como conferir que o pacote está inteiro", "",
          "```bash", "python3 skills/conversao-wx/scripts/validate_plugin_bundle.py . --strict", "```", "",
          "Esperado: `valid: true`, `tests: OK`, zero erros e zero avisos. O `--strict` roda a bateria inteira.", ""]
    git = subprocess.run(["git", "log", "-1", "--format=%h %ad", "--date=short"], capture_output=True, text=True, cwd=RAIZ).stdout.strip()
    if git:
        L += [f"Último commit no momento da medição: `{git}`.", ""]
    saida.write_text("\n".join(L), encoding="utf-8")
    print(f"ok {saida} ({total_arq} arquivos, {total_lin} linhas)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
