#!/usr/bin/env python3
"""Gera o diagrama de WORKFLOW: quem faz o que, e onde a coisa espera gente.

O fluxograma (`gerar-fluxo.py`) responde «por onde a coisa passa». Este responde
outra pergunta, que o outro nao responde e por isso merece desenho proprio:
**quem** faz cada passo, e em que ponto o trabalho PARA esperando uma pessoa.

Quatro raias, porque sao quatro responsabilidades que nao se misturam -- e uma
delas e regra do plugin, nao enfeite: quem valida nao conserta o que detecta.

  cliente / softhouse   entrega evidencia, responde, homologa
  engenheiro            converte, com o plugin
  QA independente       prova, e so prova
  aprovador             decide o que so gente decide

O que o desenho marca de proposito sao as ESPERAS: losango de aprovacao humana.
Um workflow que so mostra caixas felizes esconde justamente onde o cronograma
para. Numeros medidos do repositorio, como no fluxograma.

Uso: python3 docs/dossie/gerar-workflow.py [saida.html]
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
L, ALT = 1400, 622

RAIAS = [
    ("cliente · softhouse", "var(--az)", 64),
    ("engenheiro de conversão", "var(--vd)", 216),
    ("QA independente", "var(--am)", 400),
    ("aprovador", "var(--a)", 548),
]


def medir() -> dict:
    v = json.loads((RAIZ / ".claude-plugin/plugin.json").read_text(encoding="utf-8"))["version"]
    q = subprocess.run([sys.executable, str(RAIZ / "skills/conversao-wx/scripts/listar_perguntas.py"),
                        "--json"], capture_output=True, text=True)
    testes = len(re.findall(r"^\s+def test_", (RAIZ / "tests/testes.py").read_text(encoding="utf-8"), re.M))
    return {"versao": v, "perguntas": len(json.loads(q.stdout)) if q.returncode == 0 else 0,
            "comandos": len(list((RAIZ / "commands").glob("*.md"))), "testes": testes}


def caixa(x, y, w, h, titulo, sub, cor, tracejada=False) -> str:
    t = ' stroke-dasharray="5 4"' if tracejada else ""
    s = [f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="7" fill="var(--p)" stroke="{cor}" '
         f'stroke-width="1.5"{t}/>',
         f'<text x="{x + w / 2}" y="{y + 19}" text-anchor="middle" fill="currentColor" '
         f'font-size="12" font-weight="600">{E(titulo)}</text>']
    for i, linha in enumerate(sub.split("\n") if sub else []):
        s.append(f'<text x="{x + w / 2}" y="{y + 35 + i * 12}" text-anchor="middle" fill="var(--m)" '
                 f'font-size="10">{E(linha)}</text>')
    return "".join(s)


def espera(x, y, texto, cor) -> str:
    """O losango marca onde o trabalho PARA esperando uma pessoa decidir."""
    return (f'<path d="M{x} {y - 26} L{x + 92} {y} L{x} {y + 26} L{x - 92} {y} Z" fill="var(--p)" '
            f'stroke="{cor}" stroke-width="1.5"/>'
            f'<text x="{x}" y="{y + 4}" text-anchor="middle" fill="currentColor" font-size="11" '
            f'font-weight="600">{E(texto)}</text>')


def seta(x1, y1, x2, y2, rotulo="", cor="currentColor", tracejada=False) -> str:
    t = ' stroke-dasharray="5 4"' if tracejada else ""
    s = [f'<path d="M{x1} {y1} L{x2} {y2}" fill="none" stroke="{cor}" stroke-width="1.4"{t} '
         'marker-end="url(#pw)"/>']
    if rotulo:
        s.append(f'<text x="{(x1 + x2) / 2}" y="{(y1 + y2) / 2 - 5}" text-anchor="middle" '
                 f'fill="var(--m)" font-size="10">{E(rotulo)}</text>')
    return "".join(s)


def desenho(m: dict) -> str:
    p, frente = [], []
    p.append('<defs><marker id="pw" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" '
             'markerHeight="7" orient="auto-start-reverse">'
             '<path d="M0 0 L10 5 L0 10 z" fill="currentColor"/></marker></defs>')

    # as raias, com o nome sobre fundo para as setas nao cruzarem a palavra
    for nome, cor, y in RAIAS:
        p.append(f'<path d="M170 {y - 46} H{L - 14}" stroke="var(--l)" stroke-width="1"/>')
        p.append(f'<rect x="14" y="{y - 22}" width="148" height="30" rx="5" fill="none" '
                 f'stroke="{cor}" stroke-width="1.5"/>')
        p.append(f'<text x="88" y="{y - 2}" text-anchor="middle" fill="{cor}" font-size="11" '
                 f'font-weight="700">{E(nome)}</text>')
    p.append(f'<path d="M170 {RAIAS[-1][2] + 60} H{L - 14}" stroke="var(--l)" stroke-width="1"/>')

    AZ, VD, AM, VM = (r[1] for r in RAIAS)
    yc, ye, yq, ya = (r[2] for r in RAIAS)

    # ---- cliente: entrega e responde
    p.append(caixa(186, yc - 34, 190, 62, "entrega as evidências",
                   f"SQL, PDFs ou código;\nartefatos um a um", AZ))
    p.append(caixa(400, yc - 34, 190, 62, "responde o questionário",
                   f"{m['perguntas']} itens, com id;\npode parar e retomar", AZ))
    p.append(seta(376, yc, 400, yc))

    # ---- engenheiro: converte
    p.append(seta(495, yc + 28, 495, ye - 34, "aplica"))
    p.append(caixa(400, ye - 34, 190, 62, "G0 · pré-flight",
                   "inventário com hash;\nBLOCKED trava o código", VD))
    passos = [("G1–G3 estrutura", "inventário, arquitetura,\ndados"),
              ("G4 piloto", "uma vertical inteira,\ncom golden master"),
              ("G5 ondas", "módulo a módulo,\ntelas pelo Impeccable")]
    x = 620
    for titulo, sub in passos:
        p.append(caixa(x, ye - 34, 190, 62, titulo, sub, VD))
        p.append(seta(x - 30, ye, x, ye))
        x += 220

    # ---- QA: prova, e so prova
    for origem, texto, sub in ((715, "F-GATE", "funciona?"), (935, "C-GATE", "conforme?"),
                               (1155, "evidência e grafo", "o que não foi provado\nfica escrito")):
        p.append(caixa(origem - 95, yq - 34, 190, 62, texto, sub, AM))
        p.append(seta(origem, ye + 28, origem, yq - 34))
    # o reprovado volta para o engenheiro -- e QA nao conserta
    p.append(f'<path d="M1250 {yq + 28} V{yq + 52} H602 V{ye + 8} H616" fill="none" stroke="{VM}" '
             'stroke-width="1.4" stroke-dasharray="5 4" marker-end="url(#pw)"/>')
    frente.append(f'<rect x="700" y="{yq + 42}" width="470" height="15" fill="var(--p)"/>'
                  f'<text x="935" y="{yq + 53}" text-anchor="middle" fill="{VM}" font-size="10">'
                  'reprovado volta para quem escreveu — o QA não conserta o que detecta</text>')

    # ---- aprovador: as duas esperas
    p.append(espera(495, ya, "aprova o G0?", VM))
    p.append(seta(495, ye + 28, 495, ya - 26, "", VM))
    p.append(espera(1250, ya, "homologa?", VM))
    p.append(seta(1250, yq + 28, 1250, ya - 26, "", VM))
    frente.append(f'<text x="640" y="{ya + 5}" fill="var(--m)" font-size="10">'
                  'aqui o trabalho PARA: nenhum gate avança sem decisão humana registrada</text>')

    # ---- fim
    p.append(caixa(1210, yc - 34, 176, 62, "recebe a entrega",
                   "sete pastas, SHA-256,\nprocedência e BOM", AZ))
    p.append(f'<path d="M1298 {ya - 26} V{ya - 60} H1380 V{yc + 28}" fill="none" stroke="{AZ}" '
             'stroke-width="1.4" marker-end="url(#pw)"/>')

    return (f'<svg viewBox="0 0 {L} {ALT}" role="img" aria-label="Workflow do WX Claude Code em '
            'quatro raias: o cliente entrega evidências e responde o questionário; o engenheiro '
            'roda o G0 e converte pelos gates; o QA independente prova e devolve ao engenheiro '
            'quando reprova, sem consertar; e o aprovador decide o G0 e a homologação, que é onde '
            'o trabalho para esperando uma pessoa">'
            + "".join(p) + "".join(frente) + "</svg>")


def pagina(m: dict) -> str:
    hoje = date.today()
    meses = ("janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto",
             "setembro", "outubro", "novembro", "dezembro")
    papeis = [
        ("cliente · softhouse", "Entrega evidência, responde o questionário, homologa e recebe.",
         "Não escreve código do destino. O que ele entrega vira hash no inventário do G0."),
        ("engenheiro de conversão", "Converte com o plugin, gate a gate.",
         "Escreve o produto. Com <code>WX_PAPEL</code> ausente, é o papel padrão e nada muda."),
        ("QA independente", "Prova: F-GATE, C-GATE, evidência, grafo, efeito.",
         "Com <code>WX_PAPEL=qa</code> o hook recusa que ele escreva o produto que valida — "
         "quem valida não conserta o que detecta."),
        ("aprovador", "Decide o que só gente decide: liberar o G0 e homologar.",
         "É o nome registrado no item 0.16 do questionário; o kickoff o cita."),
    ]
    linhas = "".join(f"<tr><td><b>{E(a)}</b></td><td>{E(b)}</td><td>{c}</td></tr>" for a, b, c in papeis)
    return f"""<title>Workflow do WX Claude Code</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@500;700;800&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=JetBrains+Mono:wght@400;600&display=swap">
<style>
:root{{--g:#FBFAF7;--p:#FFFFFF;--i:#14161F;--m:#6B6F82;--l:#E4E2DB;--a:#C63C0A;--az:#1F5FBF;--vd:#1F7A4D;--am:#9A6B00}}
*{{box-sizing:border-box}}body{{margin:0;background:var(--g);color:var(--i);font-family:"Source Serif 4",Georgia,serif;font-size:14.5px;line-height:1.55}}
.wrap{{max-width:1460px;margin:0 auto;padding:34px 26px 56px}}
h1,h2{{font-family:"Exo 2","Segoe UI",sans-serif;margin:0}}
h1{{font-size:34px;font-weight:800;letter-spacing:-.5px;line-height:1.1;margin-top:6px}}
h2{{font-size:19px;font-weight:700;margin:32px 0 10px}}
.eyebrow{{font-family:"Exo 2",sans-serif;font-size:12px;letter-spacing:.14em;text-transform:uppercase;color:var(--a);font-weight:700}}
.lead{{max-width:74ch;color:var(--m);margin:12px 0 0;font-size:16px}}
header{{border-bottom:2px solid var(--i);padding-bottom:16px}}
.quadro{{background:var(--p);border:1px solid var(--l);padding:14px;margin-top:20px;overflow-x:auto}}
svg{{display:block;min-width:1120px;width:100%;height:auto;color:var(--i)}}
table{{border-collapse:collapse;width:100%;font-size:13.5px;background:var(--p);border:1px solid var(--l)}}
th{{font-family:"Exo 2",sans-serif;font-size:11px;letter-spacing:.08em;text-transform:uppercase;text-align:left;color:var(--m);padding:9px 10px;border-bottom:2px solid var(--l)}}
td{{padding:9px 10px;border-bottom:1px solid var(--l);vertical-align:top}}
code{{font-family:"JetBrains Mono",monospace;font-size:12px;background:#F2F0EA;padding:1px 4px}}
.nota{{color:var(--m);font-size:13.5px;max-width:80ch;margin-top:14px}}
</style>
<div class="wrap">
<header>
 <div class="eyebrow">WX Claude Code {E(m['versao'])} · workflow · {hoje.day} de {meses[hoje.month - 1]} de {hoje.year}</div>
 <h1>Quem faz o quê — e onde o trabalho espera gente</h1>
 <p class="lead">O fluxograma mostra <b>por onde a coisa passa</b>. Este mostra <b>quem</b> faz cada
 passo e em que ponto o cronograma para: os dois losangos são decisões humanas, e nenhum gate
 avança sem elas. Página gerada; os números saem do repositório.</p>
</header>
<div class="quadro">{desenho(m)}</div>
<h2>As quatro raias</h2>
<table><thead><tr><th>papel</th><th>o que faz</th><th>o que o plugin garante</th></tr></thead>
<tbody>{linhas}</tbody></table>
<p class="nota">A separação entre engenheiro e QA não é organograma: é o hook
<code>papel_da_sessao.py</code>. Sem papel declarado <b>nada muda</b> — a guarda entra pedida, não
imposta, porque proteção que quebra quem já usava não é proteção, é estrago.</p>
</div>
"""


def main() -> int:
    saida = Path(sys.argv[1]) if len(sys.argv) > 1 else RAIZ / "docs/workflow.html"
    m = medir()
    saida.write_text(pagina(m), encoding="utf-8")
    print(f"ok {saida} ({len(RAIAS)} raias, {m['perguntas']} perguntas, {m['comandos']} comandos)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
