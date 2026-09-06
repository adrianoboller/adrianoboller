#!/usr/bin/env python3
"""Gera a pagina de referencia dos comandos e das perguntas do plugin.

Le o front-matter de cada commands/*.md e a saida do listar_perguntas.py: a
pagina nao pode discordar do que existe, porque sai do que existe.

Uso: python3 docs/dossie/gerar-comandos.py [saida.html]
"""
from __future__ import annotations

import html as H
import json
import re
import subprocess
import sys
from datetime import date
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
E = H.escape

# Ordem de uso, nao alfabetica: quem le a folha quer saber por onde comecar.
ORDEM = ["questionario", "pergunta", "progresso", "comandos", "artefato", "pdf", "preflight", "converter", "interface",
         "estilo-telas", "golden", "constraints", "evidencia", "efeito", "grafo", "procedencia", "replay", "gemeo",
         "pmo", "equipe", "contrato", "telemetria", "identidade", "log", "ambiente", "help-wl", "rag",
         "exportar", "zelador", "licenca", "laudo-tokens"]
GRUPO = {
    "questionario": "Começar", "pergunta": "Começar", "progresso": "Começar", "comandos": "Começar",
    "artefato": "Entrada", "pdf": "Entrada", "preflight": "Entrada",
    "converter": "Converter", "interface": "Converter", "estilo-telas": "Converter", "golden": "Converter",
    "constraints": "Provar", "evidencia": "Provar", "efeito": "Provar", "grafo": "Provar", "procedencia": "Provar", "replay": "Provar", "gemeo": "Provar",
    "pmo": "Governar", "equipe": "Governar", "contrato": "Governar", "log": "Governar",
    "telemetria": "Governar", "identidade": "Governar",
    "ambiente": "Apoio", "help-wl": "Apoio", "rag": "Apoio",
    "exportar": "Entregar", "zelador": "Entregar", "licenca": "Entregar", "laudo-tokens": "Entregar",
}
CORES = {"Começar": "--a", "Entrada": "--a2", "Converter": "--ok", "Provar": "--a2",
         "Governar": "--roxo", "Apoio": "--m", "Entregar": "--am"}


def front(p: Path) -> dict:
    t = p.read_text(encoding="utf-8")
    d = {}
    for campo in ("description", "argument-hint"):
        m = re.search(rf'^{campo}:\s*"?(.*?)"?\s*$', t, re.M)
        if m:
            d[campo] = m.group(1)
    return d


def main() -> int:
    saida = Path(sys.argv[1]) if len(sys.argv) > 1 else RAIZ / "docs/comandos.html"
    versao = json.loads((RAIZ / ".claude-plugin/plugin.json").read_text(encoding="utf-8"))["version"]
    arquivos = {p.stem: p for p in (RAIZ / "commands").glob("*.md")}
    faltando = set(arquivos) - set(ORDEM)
    if faltando:
        print(f"erro: comando sem lugar na ordem: {sorted(faltando)}; edite ORDEM em gerar-comandos.py", file=sys.stderr)
        return 2
    perguntas = json.loads(subprocess.run(
        [sys.executable, str(RAIZ / "skills/conversao-wx/scripts/listar_perguntas.py"), "--json"],
        capture_output=True, text=True).stdout)

    ordem_dos_grupos = [GRUPO[n] for n in ORDEM if n in arquivos]
    vistos = []
    for gr in ordem_dos_grupos:
        if gr not in vistos:
            vistos.append(gr)
        elif vistos[-1] != gr:
            print(f"erro: o grupo {gr!r} aparece em dois trechos da ORDEM; junte os comandos dele", file=sys.stderr)
            return 2
    linhas, grupo_atual = [], None
    for nome in ORDEM:
        if nome not in arquivos:
            continue
        g = GRUPO[nome]
        if g != grupo_atual:
            grupo_atual = g
            linhas.append(f'<tr class="grupo"><td colspan="3" style="color:var({CORES[g]})">{E(g)}</td></tr>')
        d = front(arquivos[nome])
        arg = d.get("argument-hint", "")
        linhas.append(f'<tr><td class="cmd">/wx-claude-code:<b>{E(nome)}</b>'
                      + (f'<span class="arg">{E(arg)}</span>' if arg else "")
                      + f'</td><td>{E(d.get("description", ""))}</td></tr>')
    tabela = "".join(linhas)

    q_linhas, bloco = [], None
    for p in perguntas:
        if p["nivel"] == 1:
            bloco = p["id"]
            q_linhas.append(f'<tr class="grupo"><td colspan="2">{E(p["id"])} · {E(p["titulo"])}</td></tr>')
        else:
            q_linhas.append(f'<tr><td class="cmd"><b>{E(p["id"])}</b></td><td>{E(p["titulo"])}</td></tr>')
    perg = "".join(q_linhas)

    page = f'''<title>Comandos do WX Claude Code</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@500;700;800&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=JetBrains+Mono:wght@400;600&display=swap">
<style>
:root{{--g:#FBFAF7;--p:#FFFFFF;--i:#14161F;--m:#6B6F82;--l:#E4E2DB;--a:#C63C0A;--a2:#1F5FBF;--ok:#1F7A4D;--am:#9A6B00;--roxo:#7A3E9D;--grid:#ECEAE3}}
@media (prefers-color-scheme: dark){{:root:not([data-theme="light"]){{--g:#0B0D17;--p:#121527;--i:#EDEDF3;--m:#9AA0B8;--l:#252A42;--a:#E2261C;--a2:#6FA3FF;--ok:#2FBF71;--am:#F7B733;--roxo:#B48CF0;--grid:#1B1F33}}}}
:root[data-theme="dark"]{{--g:#0B0D17;--p:#121527;--i:#EDEDF3;--m:#9AA0B8;--l:#252A42;--a:#E2261C;--a2:#6FA3FF;--ok:#2FBF71;--am:#F7B733;--roxo:#B48CF0;--grid:#1B1F33}}
*{{box-sizing:border-box}}body{{margin:0;background:var(--g);color:var(--i);font-family:"Source Serif 4",Georgia,serif;font-size:14.5px;line-height:1.45}}
.wrap{{max-width:980px;margin:0 auto;padding:36px 26px 60px}}
h1,h2{{font-family:"Exo 2","Segoe UI",sans-serif;margin:0;text-wrap:balance}}
h1{{font-size:34px;font-weight:800;letter-spacing:-.5px;line-height:1.1;margin-top:6px}}
h2.sec{{font-size:20px;margin:34px 0 6px;page-break-after:avoid}}
.eyebrow{{font-family:"Exo 2",sans-serif;font-size:12px;letter-spacing:.14em;text-transform:uppercase;color:var(--a);font-weight:700}}
.lead{{max-width:70ch;color:var(--m);margin:10px 0 0;font-size:15.5px}}
header{{border-bottom:2px solid var(--i);padding-bottom:16px}}
.scroll{{overflow-x:auto;border:1px solid var(--l);background:var(--p);margin-top:8px}}
table{{border-collapse:collapse;width:100%;font-size:13.5px}}
td{{padding:6px 11px;border-bottom:1px solid var(--grid);vertical-align:top}}
tr{{page-break-inside:avoid}}
tr.grupo td{{background:var(--grid);font-family:"Exo 2",sans-serif;font-size:11px;letter-spacing:.1em;text-transform:uppercase;font-weight:700;padding:5px 11px;color:var(--m)}}
td.cmd{{font-family:"JetBrains Mono",monospace;font-size:12.5px;white-space:nowrap;width:34%;color:var(--m)}}
td.cmd b{{color:var(--i);font-weight:600}}
.arg{{display:block;color:var(--a2);font-size:11.5px;margin-top:1px}}
.nota{{color:var(--m);font-size:13.5px;max-width:76ch;margin-top:12px}}
code{{font-family:"JetBrains Mono",monospace;font-size:12.5px;background:var(--grid);padding:1px 5px;border-radius:3px}}
@media print{{.wrap{{padding:0 0 8px}}}}
</style>
<div class="wrap">
<header>
 <div class="eyebrow">WX Claude Code {E(versao)} · folha de referência · {date.today().isoformat()}</div>
 <h1>Comandos e perguntas</h1>
 <p class="lead">{len([n for n in ORDEM if n in arquivos])} comandos, um por recurso, e {len(perguntas)} perguntas com id. Tudo aqui sai dos próprios arquivos do plugin: o comando é o <code>commands/&lt;nome&gt;.md</code>, e a lista de perguntas sai do modelo do questionário.</p>
</header>

<h2 class="sec">Comandos, na ordem de uso</h2>
<div class="scroll"><table><tbody>{tabela}</tbody></table></div>
<p class="nota">Invoque com <code>/wx-claude-code:&lt;nome&gt;</code> dentro de uma sessão do Claude Code. O que está em azul, sob o nome, são os argumentos aceitos.</p>

<h2 class="sec">Perguntas do questionário, por id</h2>
<p class="nota" style="margin-top:0">Qualquer uma se refaz sozinha com <code>/wx-claude-code:pergunta &lt;id&gt;</code>, que grava só aquele ramo do JSON e reaplica. As respostas ficam em <code>.wx-migration/respostas_questionario.md</code>, com índice por id.</p>
<div class="scroll"><table><tbody>{perg}</tbody></table></div>

<p class="nota">Esta folha é gerada por <code>docs/dossie/gerar-comandos.py</code> e atualizada junto das outras páginas por <code>docs/dossie/atualizar-paginas.py</code>. Comando novo sem lugar na ordem faz o gerador falhar, de propósito.</p>
</div>
'''
    saida.write_text(page, encoding="utf-8")
    print(f"ok {saida} ({len([n for n in ORDEM if n in arquivos])} comandos, {len(perguntas)} perguntas)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
