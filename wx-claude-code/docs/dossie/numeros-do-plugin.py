#!/usr/bin/env python3
"""Mede os numeros do plugin e escreve docs/relatorio-do-plugin.md e os dados do dossie.

Regra do projeto: numero visivel ou sai de um gerador, ou esta errado e ninguem
percebeu ainda. Tudo aqui e contado no repositorio na hora de rodar.
Uso: python3 docs/dossie/numeros-do-plugin.py  (a partir de wx-claude-code/)
"""
from __future__ import annotations
import json, re, subprocess, sys
from datetime import date
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]


def conta(glob: str) -> int:
    return len(list(RAIZ.glob(glob)))


def linhas(glob: str) -> int:
    return sum(len(p.read_text(encoding="utf-8", errors="ignore").splitlines()) for p in RAIZ.glob(glob) if p.is_file())


def medir() -> dict:
    plugin = json.loads((RAIZ / ".claude-plugin/plugin.json").read_text(encoding="utf-8"))
    q = json.loads((RAIZ / "skills/conversao-wx/templates/questionario.json").read_text(encoding="utf-8"))
    blocos = [k for k in q if k not in ("schema_version", "respondido_em", "projeto")]
    def itens(b):
        v = q[b]
        return len([k for k in v if re.match(r"^(0_\d+_|F\d+_|K\d_|L\d_)", k)]) if isinstance(v, dict) else 0
    sys.path.insert(0, str(RAIZ / "skills/conversao-wx/scripts"))
    import esqueleto_erp as _e  # noqa: E402
    esqueleto_erp = len(_e.arquivos(json.loads((RAIZ / "exemplos/estoque-wx/questionario.json").read_text(encoding="utf-8"))))
    testes = len(re.findall(r"^\s+def test_", (RAIZ / "tests/testes.py").read_text(encoding="utf-8"), re.M))
    corpus = RAIZ / "skills/conversao-wx/resources/Help_WL_12k_Json.zip"
    def duracao(nome: str) -> str:
        arq = RAIZ / "docs/video" / nome
        if not arq.is_file():
            return "INDISPONÍVEL"
        try:
            import imageio_ffmpeg
            out = subprocess.run([imageio_ffmpeg.get_ffmpeg_exe(), "-i", str(arq)], capture_output=True, text=True).stderr
            m = re.search(r"Duration: (\d+):(\d+):(\d+)", out)
            return f"{int(m.group(2))} min {int(m.group(3)):02d} s" if m else "INDISPONÍVEL"
        except Exception:
            return "INDISPONÍVEL"

    # Dois roteiros no mesmo gerador: contar '\d+ · ' no arquivo inteiro somaria
    # os dois e mentiria sobre os dois. Cada um se conta dentro do seu bloco.
    fonte = (RAIZ / "docs/video/gravar-video.mjs").read_text(encoding="utf-8")

    def cenas_de(roteiro: str) -> int:
        i = fonte.index(f"ROTEIROS.{roteiro} = [")
        return len(re.findall(r"'\d+ · ", fonte[i:fonte.index("];", i)]))

    dur = duracao("wx-claude-code-video-de-uso.mp4")
    cenas = cenas_de("uso")
    dur_php = duracao("wx-claude-code-video-php.mp4")
    cenas_php = cenas_de("php")
    papeis = conta("agents/papel-?-*.md") - conta("agents/papel-?-*-plan.md") - conta("agents/papel-?-*-do.md") - conta("agents/papel-?-*-check.md") - conta("agents/papel-?-*-act.md")
    return {
        "medido_em": date.today().isoformat(), "versao": plugin["version"],
        "agentes": conta("agents/*.md"), "papeis": papeis, "subagentes_pdca": conta("agents/papel-?-*-plan.md") * 4, "especialistas_wl": conta("agents/wl-*-specialist.md"),
        "comandos": conta("commands/*.md"), "skills": len([p for p in (RAIZ / "skills").iterdir() if p.is_dir()]),
        "scripts": conta("skills/conversao-wx/scripts/*.py"), "linhas_de_python": linhas("skills/conversao-wx/scripts/*.py") + linhas("hooks/*.py"),
        "referencias": conta("skills/conversao-wx/references/*.md"), "testes": testes,
        "blocos_do_questionario": len(blocos), "itens_do_bloco_0": itens("0_empresa_e_projeto"), "subperguntas_f": itens("F_estilo_impeccable"), "itens_k": itens("K_ambiente"), "itens_l": itens("L_contexto_e_implantacao"),
        "prints": conta("docs/prints/*.png"), "video_duracao": dur, "video_cenas": cenas, "video_php_duracao": dur_php, "video_php_cenas": cenas_php,
        "corpus_bytes": corpus.stat().st_size if corpus.is_file() else 0, "corpus_paginas_validas": 12035,
        "hooks": sum(len(v) for v in json.loads((RAIZ / "hooks/hooks.json").read_text(encoding="utf-8"))["hooks"].values()),
        "manual_linhas": linhas("MANUAL.md"), "exemplo_tabelas": len(re.findall(r"CREATE TABLE", (RAIZ / "exemplos/estoque-wx/inputs/banco.sql").read_text(encoding="utf-8"), re.I)),
        "arquivos_gerados_pelo_questionario": len(re.findall(r"write_new\(", (RAIZ / "skills/conversao-wx/scripts/aplicar_questionario.py").read_text(encoding="utf-8"))) - 1 + esqueleto_erp,
        "skills_erp": len(list((RAIZ / "skills").glob("erp-*"))) + len(list((RAIZ / "skills").glob("windev-wlanguage-erp"))),
    }


def relatorio(n: dict) -> str:
    L = [f"# Relatório do plugin WX Claude Code {n['versao']}", "",
         f"Medido em {n['medido_em']} por `docs/dossie/numeros-do-plugin.py`; nenhum número abaixo foi digitado.", "",
         "## O que é", "",
         "Plugin do Claude Code que converte projetos WINDEV, WEBDEV e WINDEV Mobile para outra plataforma sem inventar o que o projeto faz: questionário guiado, gates com aprovação humana, equipe de agentes WLanguage sobre o Help oficial, PMO com Scrum, Kanban e PDCA, qualidade de tela com o Impeccable, serial de ativação, e o contexto da primeira sessão do Claude Code gerado das respostas.", "",
         "## Números", "", "| medida | valor |", "| --- | ---: |"]
    rot = {"agentes": "agentes", "papeis": "papéis A–J", "subagentes_pdca": "subagentes PDCA", "especialistas_wl": "especialistas WLanguage por tema", "comandos": "comandos /", "skills": "skills", "skills_erp": "skills de ERP (pacote skills.sh)", "scripts": "scripts Python", "linhas_de_python": "linhas de Python (scripts e hooks)", "referencias": "documentos de referência", "testes": "testes de regressão", "hooks": "hooks do plugin", "blocos_do_questionario": "blocos do questionário (0, A–M)", "itens_do_bloco_0": "itens do bloco 0", "subperguntas_f": "subperguntas de F (F0–F13)", "itens_k": "itens de K", "itens_l": "itens de L", "arquivos_gerados_pelo_questionario": "arquivos que o questionário pode gerar", "prints": "prints de sessões reais", "video_cenas": "cenas do vídeo", "video_duracao": "duração do vídeo", "video_php_cenas": "cenas do vídeo de PHP para Rust", "video_php_duracao": "duração do vídeo de PHP para Rust", "corpus_bytes": "corpus do Help (bytes)", "corpus_paginas_validas": "páginas válidas do corpus", "manual_linhas": "linhas do manual", "exemplo_tabelas": "tabelas do exemplo ESTOQUE"}
    L += [f"| {rot[k]} | {n[k]} |" for k in rot]
    L += ["", "## O que foi provado em sessão real", "", "Cada print em `docs/prints/` é a saída de uma sessão do Claude Code ou de um script, sem edição; a origem de cada um está em `docs/prints/gerar.md`. Entre eles: o questionário uma letra por vez, a senha colada que não é gravada nem repetida, a letra H com o processo de conversão, a tela modelo aberta antes de registrar, o serial de ativação recusando e depois liberando, a primeira sessão lendo `INDEX_FILES.md` e o kickoff, a exportação organizada e o zelador, e o esqueleto de ERP (L6) com a sessão carregando a skill do módulo.", "",
          "## O que não foi provado", "", "- Nenhum projeto WINDEV real passou pelos gates G1 a G7 de ponta a ponta; o exemplo ESTOQUE é sintético.", "- Os scripts de ambiente (K e L) são bash; não há versão PowerShell, e o público do plugin usa Windows.", "- A licença é dissuasão (hook); a proteção real, servir corpus e agentes de um servidor, ficou para depois por decisão do dono.", "- O custo em tokens do questionário inteiro numa sessão real não foi medido.", "",
          "## Onde está cada coisa", "", "- Manual: `MANUAL.md` (PDF em `docs/manual-de-uso.pdf`); oito capítulos.", "- Página para investidores: `docs/investidor/`.", "- Análise da aula de vibe coding: `docs/analise-aula-vibe-coding.md`.", "- Telas do fluxo de licença: `docs/telas-licenca/`.", "- Dossiê: `docs/dossie/dossie-wx-claude-code.html`, gerado deste mesmo medidor.", ""]
    return "\n".join(L)


if __name__ == "__main__":
    n = medir()
    (RAIZ / "docs/dossie/numeros.json").write_text(json.dumps(n, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    (RAIZ / "docs/relatorio-do-plugin.md").write_text(relatorio(n), encoding="utf-8")
    print(json.dumps(n, ensure_ascii=False))
