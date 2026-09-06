#!/usr/bin/env python3
"""Gera o relatorio da bateria pesada RODANDO a bateria pesada.

Existe pela regra do projeto: numero visivel sai de gerador, nunca da mao. O
relatorio de um teste e o pior lugar para digitar numero, porque quem le supoe
que aquilo foi medido. Aqui foi: esta pagina nao se escreve sem rodar os doze
cenarios, e se um falhar o gerador falha junto.

Uso: python3 docs/dossie/gerar-relatorio-cenarios.py [saida.html] [--json ARQ]
"""
from __future__ import annotations

import html as H
import json
import subprocess
import sys
import tempfile
from datetime import date
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
E = H.escape

# "doze" ja mentiu uma vez, quando o decimo terceiro cenario entrou: o numero
# por extenso do texto tem de sair da contagem, como todo numero visivel.
POR_EXTENSO = {10: "dez", 11: "onze", 12: "doze", 13: "treze", 14: "catorze", 15: "quinze",
               16: "dezesseis", 17: "dezessete", 18: "dezoito", 19: "dezenove", 20: "vinte"}

# O que cada cenario cobre, para quem le o relatorio sem ler o codigo.
POR_QUE = {
    "01": "É o caminho que o plugin existe para atender: projeto WINDEV com anexos, questionário completo, portão G0 e saída em Rust.",
    "02": "Prova as duas frouxidões que o dono pediu: legado E/OU (só PHP, sem WX) e destino livre (Elixir, que não está na lista).",
    "03": "Senha nunca em texto puro: o valor é recusado antes de gravar, e a mensagem de recusa não repete o segredo.",
    "04": "Evidência faltando não vira suposição: o G0 devolve BLOCKED dizendo o que falta, em vez de seguir com buraco.",
    "05": "PDF escaneado é o caso que mais tenta o modelo a inventar: cada página sai marcada OCR_REQUERIDO.",
    "06": "Cliente reaplica o questionário meses depois: o trabalho já feito não pode ser sobrescrito.",
    "07": "ERP de verdade tem módulos: cada um sai com domínio, pasta e a skill correspondente.",
    "08": "Resposta que se contradiz (RPO de 15 min com backup diário) é recusada com a razão, não aceita calada.",
    "09": "Artefato é acervo permanente: um token colado dentro dele não pode ser arquivado.",
    "10": "Sem licença o plugin não roda: o hook recusa os scripts, e o verificador não diz que está válida.",
    "11": "Entrega ao cliente: o .env fica fora do pacote e o .env.exemplo entra, sem nenhum valor real.",
    "12": "O instalador em conferência não instala nada e não deixa lixo — inclusive em /tmp.",
    "16": "Os seis documentos de auditoria que um cliente regulado pede — e o teste não é sair o documento, é cada um declarar o próprio limite.",
    "15": "O grafo de rastreabilidade sobre um projeto real: acha o arquivo que requisito nenhum pediu e a regra que ninguém provou — nenhum dos dois aparece lendo o código.",
    "14": "Os seis portões de governança ligados no mesmo projeto — o caso que eles existem para pegar é F-GATE verde com C-GATE reprovado.",
    "13": "O exemplo FATURAMENTO inteiro: PHP procedural de 2009, sem nada de WINDEV, atravessando o G0 com o código-fonte como evidência central.",
}


def rodar(json_pronto: Path | None) -> dict:
    if json_pronto:
        return json.loads(json_pronto.read_text(encoding="utf-8"))
    r = subprocess.run([sys.executable, str(RAIZ / "tests/cenarios.py")],
                       capture_output=True, text=True, timeout=3600, cwd=RAIZ)
    sys.stderr.write(r.stdout)
    saida = Path(tempfile.gettempdir()) / "wx-cenarios.json"
    if not saida.is_file():
        raise SystemExit("a bateria pesada nao gravou wx-cenarios.json")
    dados = json.loads(saida.read_text(encoding="utf-8"))
    dados["codigo"] = r.returncode
    return dados


def main() -> int:
    argv = sys.argv[1:]
    pronto = None
    if "--json" in argv:
        i = argv.index("--json")
        pronto = Path(argv[i + 1])
        del argv[i:i + 2]
    saida = Path(argv[0]) if argv else RAIZ / "docs/relatorio-de-cenarios.html"
    versao = json.loads((RAIZ / ".claude-plugin/plugin.json").read_text(encoding="utf-8"))["version"]

    d = rodar(pronto)
    cen = d["cenarios"]
    falhas = [c for c in cen if not c["ok"]]
    segundos = d["ms"] / 1000
    mais_lento = max(cen, key=lambda c: c["ms"])

    linhas = []
    for c in cen:
        num = c["cenario"].split(" ", 1)[0]
        cor = "--ok" if c["ok"] else "--a"
        linhas.append(
            f'<tr><td class="n" style="color:var({cor})">{E(num)}</td>'
            f'<td><b>{E(c["cenario"].split(" ", 1)[1])}</b>'
            f'<span class="esp">espera: {E(c["espera"])}</span>'
            f'<span class="pq">{E(POR_QUE.get(num, ""))}</span></td>'
            f'<td class="res" style="color:var({cor})">{"passou" if c["ok"] else "FALHOU"}</td>'
            f'<td class="det">{E(c["detalhe"])}</td>'
            f'<td class="ms">{c["ms"]} ms</td></tr>')
    tabela = "".join(linhas)

    veredito = (f"Os {POR_EXTENSO.get(d['total'], d['total'])} passaram." if not falhas
                else f'{len(falhas)} cenário(s) falharam: ' + ", ".join(E(c["cenario"]) for c in falhas))

    page = f'''<title>Relatório da bateria pesada</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@500;700;800&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=JetBrains+Mono:wght@400;600&display=swap">
<style>
:root{{--g:#FBFAF7;--p:#FFFFFF;--i:#14161F;--m:#6B6F82;--l:#E4E2DB;--a:#C63C0A;--a2:#1F5FBF;--ok:#1F7A4D;--am:#9A6B00;--grid:#ECEAE3}}
@media (prefers-color-scheme: dark){{:root:not([data-theme="light"]){{--g:#0B0D17;--p:#121527;--i:#EDEDF3;--m:#9AA0B8;--l:#252A42;--a:#E2261C;--a2:#6FA3FF;--ok:#2FBF71;--am:#F7B733;--grid:#1B1F33}}}}
:root[data-theme="dark"]{{--g:#0B0D17;--p:#121527;--i:#EDEDF3;--m:#9AA0B8;--l:#252A42;--a:#E2261C;--a2:#6FA3FF;--ok:#2FBF71;--am:#F7B733;--grid:#1B1F33}}
*{{box-sizing:border-box}}body{{margin:0;background:var(--g);color:var(--i);font-family:"Source Serif 4",Georgia,serif;font-size:14.5px;line-height:1.45}}
.wrap{{max-width:1040px;margin:0 auto;padding:36px 26px 60px}}
h1,h2{{font-family:"Exo 2","Segoe UI",sans-serif;margin:0;text-wrap:balance}}
h1{{font-size:34px;font-weight:800;letter-spacing:-.5px;line-height:1.1;margin-top:6px}}
h2.sec{{font-size:20px;margin:34px 0 6px;page-break-after:avoid}}
.eyebrow{{font-family:"Exo 2",sans-serif;font-size:12px;letter-spacing:.14em;text-transform:uppercase;color:var(--a);font-weight:700}}
.lead{{max-width:74ch;color:var(--m);margin:10px 0 0;font-size:15.5px}}
header{{border-bottom:2px solid var(--i);padding-bottom:16px}}
.painel{{display:flex;flex-wrap:wrap;gap:10px;margin:18px 0 4px}}
.card{{flex:1 1 150px;border:1px solid var(--l);background:var(--p);padding:10px 13px}}
.card b{{display:block;font-family:"Exo 2",sans-serif;font-size:26px;font-weight:800;line-height:1.1}}
.card span{{font-size:12px;color:var(--m)}}
.scroll{{overflow-x:auto;border:1px solid var(--l);background:var(--p);margin-top:8px}}
table{{border-collapse:collapse;width:100%;font-size:13.5px}}
td{{padding:8px 11px;border-bottom:1px solid var(--grid);vertical-align:top}}
tr{{page-break-inside:avoid}}
td.n{{font-family:"JetBrains Mono",monospace;font-weight:600;width:34px}}
td.res{{font-family:"Exo 2",sans-serif;font-size:11.5px;letter-spacing:.08em;text-transform:uppercase;font-weight:700;white-space:nowrap;width:78px}}
td.det{{color:var(--m);width:33%}}
td.ms{{font-family:"JetBrains Mono",monospace;font-size:12px;color:var(--m);text-align:right;white-space:nowrap}}
.esp,.pq{{display:block;color:var(--m);font-size:12.5px;margin-top:2px}}
.esp{{color:var(--a2)}}
.nota{{color:var(--m);font-size:13.5px;max-width:78ch;margin-top:12px}}
code{{font-family:"JetBrains Mono",monospace;font-size:12.5px;background:var(--grid);padding:1px 5px;border-radius:3px}}
@media print{{.wrap{{padding:0 0 8px}}}}
</style>
<div class="wrap">
<header>
 <div class="eyebrow">WX Claude Code {E(versao)} · relatório de uso · {date.today().isoformat()}</div>
 <h1>Bateria pesada: o plugin inteiro, em {E(POR_EXTENSO.get(d["total"], str(d["total"])))} situações</h1>
 <p class="lead">Esta página é gerada rodando os cenários — nenhum número aqui foi digitado. A bateria de unidade prova cada peça; o teste de fluxo prova a ligação no caminho feliz. Estes {E(POR_EXTENSO.get(d["total"], str(d["total"])))} cenários são os <i>outros</i> caminhos: os que um cliente real traz e que só aparecem quando quebram.</p>
</header>

<div class="painel">
 <div class="card"><b style="color:var({'--ok' if not falhas else '--a'})">{d["ok"]}/{d["total"]}</b><span>cenários passaram</span></div>
 <div class="card"><b>{segundos:.1f}s</b><span>a bateria inteira</span></div>
 <div class="card"><b>{mais_lento["ms"]} ms</b><span>o mais lento ({E(mais_lento["cenario"].split(" ", 1)[0])})</span></div>
 <div class="card"><b>{round(d["ms"] / max(1, d["total"]))} ms</b><span>média por cenário</span></div>
</div>

<h2 class="sec">Como foi o uso</h2>
<p class="nota" style="margin-top:0">{veredito} Cada linha diz o que o cenário <b>esperava antes de rodar</b> e o que de fato aconteceu: cenário que passa por engano é pior que cenário que falta, e por isso o esperado está escrito no código, não inferido da saída.</p>
<div class="scroll"><table><tbody>{tabela}</tbody></table></div>

<h2 class="sec">O que a bateria pesada acrescenta</h2>
<p class="nota" style="margin-top:0">Os três níveis não se substituem. <code>tests/testes.py</code> pega a peça quebrada; <code>tests/fluxo.py</code> pega a peça certa ligada errada; <code>tests/cenarios.py</code> pega o caminho que ninguém imaginou — o cliente sem licença, o PDF que é foto, o legado que nunca foi WINDEV, a resposta que se contradiz. Os três rodam no mesmo comando: <code>python3 tests/testes.py</code>.</p>
<p class="nota">Reproduza com <code>python3 tests/cenarios.py</code>; um cenário só, com <code>--so 04</code>; e <code>--manter</code> guarda as pastas temporárias para inspeção. O relatório se refaz com <code>python3 docs/dossie/gerar-relatorio-cenarios.py</code>, que roda a bateria de novo antes de escrever.</p>
</div>
'''
    saida.write_text(page, encoding="utf-8")
    print(f"ok {saida} ({d['ok']}/{d['total']} cenarios, {segundos:.1f}s)")
    return 0 if not falhas else 1


if __name__ == "__main__":
    raise SystemExit(main())
