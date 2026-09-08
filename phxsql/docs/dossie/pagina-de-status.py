#!/usr/bin/env python3
"""Gera a pagina «Os dez recursos do PhxSql» a partir do docs/STATUS.md.

    python3 docs/dossie/pagina-de-status.py [saida.html] [--so-medir]

Ela carrega DOIS tipos de numero, e os separa de proposito -- e o motivo e o
mesmo do dossie dos testes: juntar medida e avaliacao sem dizer qual e qual
publica um retrato que ninguem sabe ler.

- A NOTA (0-10) de cada recurso e AVALIACAO: sai do `docs/STATUS.md`, datada,
  com as fontes nomeadas na propria linha. Quem discorda muda la e roda isto.
- O PAINEL do alto e MEDIDO: `CAPABILITIES.json` e os `resultados.json` das
  bancadas, cada numero com a data em que foi medido. Quando a data sai do
  `mtime` do arquivo e nao do proprio resultado, a pagina DIZ que saiu.
  Bancada sem arquivo aparece como NAO MEDIDA -- nunca some da pagina.

Nenhum numero desta pagina se digita.
"""

import datetime
import html
import json
import pathlib
import re
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]
FONTE = RAIZ / "docs" / "STATUS.md"
PADRAO = RAIZ / "docs" / "dossie" / "status.html"

TIPOS = {
    "construído": ("construido", "Construído"),
    "parcial": ("parcial", "Parcial"),
    "recusa medida": ("recusa", "Recusa medida"),
    "promessa": ("promessa", "Promessa"),
}
LINHA = re.compile(
    r"^\|\s*([A-J])\s*\|\s*(.*?)\s*\|\s*(\d{1,2})\s*\|\s*(.*?)\s*\|\s*(.*?)\s*\|\s*(.*?)\s*\|\s*$"
)
DATA_DA_AVALIACAO = re.compile(r"em \*\*(\d{2}/\d{2}/\d{4})\*\*")


def marcar(t):
    """O pouco de Markdown que as celulas usam, virando HTML. Escapa ANTES."""
    t = html.escape(t)
    t = re.sub(r"`([^`]+)`", r"<code>\1</code>", t)
    t = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", t)
    t = re.sub(r"(?<!\*)\*([^*]+)\*(?!\*)", r"<em>\1</em>", t)
    return t


def ler():
    md = FONTE.read_text(encoding="utf-8")
    m = DATA_DA_AVALIACAO.search(md)
    if not m:
        raise SystemExit("STATUS.md: nao diz a data da avaliacao (em **DD/MM/AAAA**)")
    data = m.group(1)
    itens = []
    for n, l in enumerate(md.split("\n"), 1):
        r = LINHA.match(l)
        if not r:
            continue
        letra, recurso, nota, tipo, como, falta = r.groups()
        if tipo not in TIPOS:
            raise SystemExit(
                f"STATUS.md:{n}: tipo {tipo!r} nao e um de {list(TIPOS)}")
        nota = int(nota)
        if not 0 <= nota <= 10:
            raise SystemExit(f"STATUS.md:{n}: nota {nota} fora de 0-10")
        itens.append({
            "letra": letra, "recurso": marcar(recurso), "nota": nota,
            "classe": TIPOS[tipo][0], "tipo": TIPOS[tipo][1],
            "como": marcar(como), "falta": marcar(falta),
        })
    letras = "".join(i["letra"] for i in itens)
    if letras != "ABCDEFGHIJ":
        raise SystemExit(f"STATUS.md: esperava as dez linhas A..J na ordem, achei {letras!r}")
    # A secao «Leitura», paragrafo a paragrafo.
    leitura = []
    if "## Leitura" in md:
        bloco = md.split("## Leitura", 1)[1].strip()
        leitura = [marcar(" ".join(p.split())) for p in bloco.split("\n\n") if p.strip()]
    return data, itens, leitura


# ---------------------------------------------------------------- medidas

def num(v):
    """1234567 -> 1.234.567; 2.7 -> 2,7 -- o formato das outras paginas."""
    if isinstance(v, bool):
        return "sim" if v else "nao"
    if isinstance(v, int):
        return f"{v:,}".replace(",", ".")
    if isinstance(v, float):
        s = f"{v:,.1f}".replace(",", "X").replace(".", ",").replace("X", ".")
        return s
    return str(v)


def data_br(iso):
    """'2026-09-08 03:00:11' ou '2026-09-08T14:58:08' ou '2026-09-07' -> 08/09/2026."""
    m = re.match(r"(\d{4})-(\d{2})-(\d{2})", str(iso))
    return f"{m.group(3)}/{m.group(2)}/{m.group(1)}" if m else str(iso)


def mtime_br(p):
    return datetime.datetime.fromtimestamp(p.stat().st_mtime).strftime("%d/%m/%Y")


def ficha(v, rotulo, quando, fonte, classe=""):
    return {"v": v, "r": rotulo, "q": quando, "f": fonte, "c": classe}


def nao_medida(rotulo, fonte, comando):
    return ficha("—", rotulo, f"NÃO MEDIDA · rode <code>{html.escape(comando)}</code>",
                 fonte, "ausente")


def medir():
    fichas = []
    cap = RAIZ / "CAPABILITIES.json"
    if cap.exists():
        c = json.loads(cap.read_text(encoding="utf-8"))
        q = f"medido em {data_br(c['medido_em'])}"
        f = "CAPABILITIES.json"
        fichas += [
            ficha(num(c["testes"]), "testes, na suíte inteira", q, f),
            ficha(num(c["operacoes"]), "operações do protocolo", q, f),
            ficha(num(c["linhas_rust"]), "linhas de Rust", q, f),
            ficha(num(c["dependencias_externas"]), "dependências externas", q, f),
        ]
        versao = c["versao"]
    else:
        fichas.append(nao_medida("testes, na suíte inteira", "CAPABILITIES.json",
                                 "python3 docs/dossie/numeros-do-projeto.py"))
        versao = "?"

    # Os testes da crate SQL, contados AGORA no fonte: a linha G os cita.
    crate = RAIZ / "crates" / "phxsql-sql" / "src"
    n_sql = sum(p.read_text(encoding="utf-8").count("#[test]") for p in crate.glob("*.rs"))
    fichas.append(ficha(num(n_sql), "testes na crate SQL",
                        f"contados hoje, {datetime.date.today():%d/%m/%Y}",
                        "crates/phxsql-sql"))

    g = RAIZ / "bancada" / "gestao" / "resultados.json"
    if g.exists():
        d = json.loads(g.read_text(encoding="utf-8"))
        est = [l["estado"] for l in d["linhas"]]
        v = f"{est.count('ok')} ok · {est.count('parcial')} parcial · {est.count('planejado')} planejado"
        fichas.append(ficha(v, "gestão do banco, pelo efeito",
                            f"medido em {data_br(d['quando'])}", "bancada/gestao/resultados.json",
                            "larga"))
    else:
        fichas.append(nao_medida("gestão do banco", "bancada/gestao/resultados.json",
                                 "python3 bancada/gestao/medir.py"))

    r = RAIZ / "bancada" / "replicacao" / "resultados.json"
    if r.exists():
        d = json.loads(r.read_text(encoding="utf-8"))
        q = f"medido em {data_br(d['quando'])}"
        f = "bancada/replicacao/resultados.json"
        fichas += [
            ficha(num(d["master_linhas_s"]), "linhas/s no master, replicando", q, f),
            ficha(num(d["replica_eventos_s"]), "eventos/s na réplica", q, f),
            ficha(num(d["alcance_s"]) + " s", "alcance da réplica", q, f),
            ficha("sim" if d["iguais_no_fim"] else "NÃO", "réplicas iguais no fim (SHA-256)", q, f),
        ]
    else:
        fichas.append(nao_medida("replicação", "bancada/replicacao/resultados.json",
                                 "python3 bancada/replicacao/medir.py"))

    k = RAIZ / "bancada" / "cluster" / "resultados.json"
    if k.exists():
        d = json.loads(k.read_text(encoding="utf-8"))
        # O resultado do cluster NAO traz a data: ela sai do arquivo, e a
        # pagina diz isso -- data que parece do resultado e nao e engana.
        q = f"data do arquivo: {mtime_br(k)}"
        f = "bancada/cluster/resultados.json"
        fichas += [
            ficha(num(d["promocao_s"]) + " s", "promoção após matar o master", q, f),
            ficha("sim" if d["no3_nao_promoveu"] else "NÃO", "a minoria não se elegeu", q, f),
            ficha(num(len(d["falhas"])), "falhas na bancada do cluster", q, f),
        ]
    else:
        fichas.append(nao_medida("cluster", "bancada/cluster/resultados.json",
                                 "python3 bancada/cluster/provar.py"))
    return versao, fichas


# ----------------------------------------------------------------- pagina

CSS = """
:root{
  --papel:#fbf9f7; --papel-2:#f3efec; --papel-3:#e9e3de;
  --tinta:#1a1210; --tinta-2:#4a3f3a; --tinta-3:#7a6d66;
  --linha:#ded6d0; --acento:#c63c0a;
  --construido:#2f7a3e; --parcial:#8a6a1f; --recusa:#5b6470; --promessa:#c63c0a;
  --ausente:#8a6a1f;
}
@media (prefers-color-scheme:dark){
  :root:not([data-theme="light"]){
    --papel:#040814; --papel-2:#0a1122; --papel-3:#131c31;
    --tinta:#dde2eb; --tinta-2:#a8b0c0; --tinta-3:#7c8598;
    --linha:#1e2940; --acento:#ff8a1c;
    --construido:#5cbf74; --parcial:#d5a83c; --recusa:#8e9ab0; --promessa:#ff8a1c;
    --ausente:#d5a83c;
  }
}
:root[data-theme="dark"]{
  --papel:#040814; --papel-2:#0a1122; --papel-3:#131c31;
  --tinta:#dde2eb; --tinta-2:#a8b0c0; --tinta-3:#7c8598;
  --linha:#1e2940; --acento:#ff8a1c;
  --construido:#5cbf74; --parcial:#d5a83c; --recusa:#8e9ab0; --promessa:#ff8a1c;
  --ausente:#d5a83c;
}
*{box-sizing:border-box}
body{margin:0;background:var(--papel);color:var(--tinta);
  font-family:"Source Serif 4",Georgia,"Times New Roman",serif;font-size:16px;line-height:1.55;
  -webkit-font-smoothing:antialiased}
h1,h2,h3,.rotulo,.pino,.nota-n,.regua .l{font-family:"Exo 2","Helvetica Neue",Arial,sans-serif}
code,.mono,.num,.placar .q{font-family:"IBM Plex Mono",ui-monospace,Menlo,monospace}
code{font-size:.86em;background:var(--papel-2);padding:1px 4px;border-radius:3px;color:var(--tinta-2)}
.envelope{max-width:1120px;margin:0 auto;padding:0 20px 80px}
header{padding:52px 0 28px;border-bottom:1px solid var(--linha)}
.rotulo{font-size:10.5px;letter-spacing:.18em;text-transform:uppercase;color:var(--acento);font-weight:600;margin-bottom:12px}
h1{font-size:clamp(30px,5vw,46px);font-weight:700;line-height:1.08;margin:0 0 14px;letter-spacing:-.015em;text-wrap:balance}
h1 .x{color:var(--acento)}
.chamada{max-width:66ch;color:var(--tinta-2);font-size:17px;margin:0 0 4px}
h2{font-size:22px;font-weight:600;margin:52px 0 6px;letter-spacing:-.01em}
h2 + .sub{color:var(--tinta-3);font-size:15px;margin:0 0 20px;max-width:66ch}

.placar{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:10px;margin:30px 0 0}
.placar .c{border:1px solid var(--linha);border-radius:6px;padding:14px 16px;background:var(--papel-2);display:flex;flex-direction:column;gap:6px}
.placar .c.larga{grid-column:span 2}
.placar .v{font-family:"Exo 2",sans-serif;font-size:30px;font-weight:700;line-height:1.05;font-variant-numeric:tabular-nums;letter-spacing:-.01em}
.placar .larga .v{font-size:20px}
.placar .r{font-family:"IBM Plex Mono",monospace;font-size:10px;letter-spacing:.13em;text-transform:uppercase;color:var(--tinta-3)}
.placar .q{font-size:10.5px;color:var(--tinta-3);margin-top:auto}
.placar .ausente .v{color:var(--ausente);font-size:22px}
.placar .ausente .q{color:var(--ausente)}

/* A regua: uma barra por recurso, 0-10. A FORMA carrega o tipo, nao so a cor:
   cheia = construido, meia = parcial, hachurada = recusa medida, so contorno
   = promessa. Numero depois do traco, nunca por cima dele. */
.regua{list-style:none;margin:18px 0 0;padding:0;display:grid;gap:9px}
.regua li{display:grid;grid-template-columns:2.2em minmax(11ch,18ch) 1fr 2.6em;gap:12px;align-items:center}
.regua .k{font-family:"IBM Plex Mono",monospace;font-size:12px;color:var(--tinta-3);text-align:right}
.regua .l{font-size:14px;font-weight:500;color:var(--tinta)}
.regua .t{height:14px;background:var(--papel-3);border-radius:3px;position:relative;overflow:hidden}
.regua .b{position:absolute;inset:0 auto 0 0;border-radius:3px}
.regua .construido .b{background:var(--construido)}
.regua .parcial .b{background:linear-gradient(90deg,var(--parcial) 0 100%);opacity:.75}
.regua .recusa .b{background:repeating-linear-gradient(135deg,var(--recusa) 0 4px,transparent 4px 8px)}
.regua .promessa .b{background:transparent;border:2px solid var(--promessa)}
.regua .n{font-family:"Exo 2",sans-serif;font-weight:700;font-size:16px;font-variant-numeric:tabular-nums;text-align:right}
.legenda{display:flex;flex-wrap:wrap;gap:14px 22px;margin:16px 0 0;font-size:13px;color:var(--tinta-2)}
.legenda span{display:inline-flex;align-items:center;gap:7px}
.legenda i{display:inline-block;width:22px;height:10px;border-radius:2px}
.legenda .construido i{background:var(--construido)}
.legenda .parcial i{background:var(--parcial);opacity:.75}
.legenda .recusa i{background:repeating-linear-gradient(135deg,var(--recusa) 0 3px,transparent 3px 6px)}
.legenda .promessa i{border:2px solid var(--promessa);background:transparent;height:8px}

.rolo{overflow-x:auto;-webkit-overflow-scrolling:touch}
table{border-collapse:collapse;width:100%;min-width:760px}
thead th{font-family:"IBM Plex Mono",monospace;font-weight:500;font-size:10px;letter-spacing:.13em;text-transform:uppercase;color:var(--tinta-3);text-align:left;padding:12px 12px 8px;border-bottom:1px solid var(--linha)}
tbody td{padding:16px 12px;border-bottom:1px solid var(--linha);vertical-align:top;font-size:14.5px}
tbody tr:hover td{background:var(--papel-2)}
td.k{font-family:"IBM Plex Mono",monospace;font-size:12px;color:var(--tinta-3);width:2.4em;white-space:nowrap}
td.rc{width:15%;color:var(--tinta)}
td.rc .nome{font-family:"Exo 2",sans-serif;font-weight:600;font-size:15px;display:block;margin-bottom:6px}
td.co,td.fa{color:var(--tinta-2)}
.nota-n{display:inline-block;font-weight:700;font-size:15px;font-variant-numeric:tabular-nums;margin-right:8px}
.pino{display:inline-flex;align-items:center;gap:6px;font-size:11px;font-weight:600;letter-spacing:.03em;white-space:nowrap}
.pino::before{content:"";width:9px;height:9px;border-radius:50%;border:1.5px solid currentColor;flex:none}
.pino.construido{color:var(--construido)}.pino.construido::before{background:currentColor}
.pino.parcial{color:var(--parcial)}.pino.parcial::before{background:linear-gradient(90deg,currentColor 50%,transparent 50%)}
.pino.recusa{color:var(--recusa)}.pino.recusa::before{background:repeating-linear-gradient(135deg,currentColor 0 2px,transparent 2px 4px)}
.pino.promessa{color:var(--promessa)}

.nota{border-left:3px solid var(--acento);background:var(--papel-2);padding:14px 18px;border-radius:0 5px 5px 0;margin:26px 0;font-size:15px;color:var(--tinta-2);max-width:68ch}
.nota .t{display:block;font-family:"Exo 2",sans-serif;font-weight:600;color:var(--tinta);font-size:14px;margin-bottom:5px}
.leitura p{max-width:68ch;color:var(--tinta-2)}
footer{margin-top:56px;padding-top:22px;border-top:1px solid var(--linha);color:var(--tinta-3);font-size:13.5px;max-width:68ch}
@media (max-width:640px){.regua li{grid-template-columns:2em 1fr 2.4em}.regua .l{grid-column:2}.regua .t{grid-column:1/4}.placar .c.larga{grid-column:span 1}}
@media (prefers-reduced-motion:reduce){*{transition:none!important;animation:none!important}}
"""


def pagina(data_av, itens, leitura, versao, fichas, agora):
    def placar():
        out = []
        for f in fichas:
            cls = (" " + f["c"]) if f["c"] else ""
            out.append(
                f'    <div class="c{cls}"><div class="v">{f["v"]}</div>'
                f'<div class="r">{html.escape(f["r"])}</div>'
                f'<div class="q">{f["q"]} · <span title="{html.escape(f["f"])}">{html.escape(f["f"].split("/")[-1])}</span></div></div>')
        return "\n".join(out)

    def regua():
        return "\n".join(
            f'  <li class="{i["classe"]}"><span class="k">{i["letra"]}</span>'
            f'<span class="l">{i["recurso"]}</span>'
            f'<span class="t" role="img" aria-label="{i["letra"]}: nota {i["nota"]} de 10, {html.escape(i["tipo"])}">'
            f'<span class="b" style="width:{i["nota"] * 10}%"></span></span>'
            f'<span class="n">{i["nota"]}</span></li>'
            for i in itens)

    def linhas():
        return "\n".join(
            f'      <tr class="{i["classe"]}"><td class="k">{i["letra"]}</td>'
            f'<td class="rc"><span class="nome">{i["recurso"]}</span>'
            f'<span class="nota-n">{i["nota"]}/10</span><span class="pino {i["classe"]}">{i["tipo"]}</span></td>'
            f'<td class="co">{i["como"]}</td><td class="fa">{i["falta"]}</td></tr>'
            for i in itens)

    media = sum(i["nota"] for i in itens) / len(itens)
    contas = {c: sum(1 for i in itens if i["classe"] == c) for c in ("construido", "parcial", "recusa", "promessa")}
    leitura_html = "\n".join(f"<p>{p}</p>" for p in leitura)
    return f"""<title>Os dez recursos do PhxSql</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@400;500;600;700&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=IBM+Plex+Mono:wght@400;500;600&display=swap">
<style>{CSS}</style>
<div class="envelope">
<header>
  <div class="rotulo">PhxSql {html.escape(versao)} · status dos dez recursos</div>
  <h1>Dez recursos, dez notas —<br>e o que <span class="x">falta</span> em cada um</h1>
  <p class="chamada">Duas coisas nesta página, separadas de propósito. As <strong>notas</strong>
  são <em>avaliação</em>: leitura do código e dos testes de cada recurso em
  <strong>{data_av}</strong>, com as fontes nomeadas na linha. O <strong>painel</strong> é
  <em>medido</em>: cada número traz a data em que foi medido, e a bancada que não
  rodou aparece como não medida em vez de sumir.</p>

  <div class="placar">
{placar()}
  </div>
</header>

<h2>A régua</h2>
<p class="sub">Uma barra por recurso, de 0 a 10. A <strong>forma</strong> diz o que a nota
significa, e não só a cor: {contas['construido']} construídos, {contas['parcial']} parcial,
{contas['recusa']} recusa medida e {contas['promessa']} promessa. Média das dez: {num(round(media, 1))}.</p>
<ul class="regua">
{regua()}
</ul>
<div class="legenda">
  <span class="construido"><i></i>construído — existe e está provado</span>
  <span class="parcial"><i></i>parcial — existe, com a metade que falta nomeada</span>
  <span class="recusa"><i></i>recusa medida — não existe <em>porque</em> foi medido que não compensa</span>
  <span class="promessa"><i></i>promessa — a documentação prometeu mais do que o código entrega</span>
</div>

<div class="nota">
  <span class="t">Nota é avaliação; número é medida</span>
  Uma nota 8 não é «80% medido». É o que uma leitura do código e dos testes
  concluiu, na data dita, com as fontes na linha — e quem discordar muda a linha
  do <code>docs/STATUS.md</code> com o motivo, e roda o gerador. Os números do
  painel, ao contrário, ninguém digita: saem dos arquivos que as bancadas gravam.
</div>

<h2>Os dez, um a um</h2>
<p class="sub">Como está hoje, e o que falta — com o arquivo, o teste ou o documento que sustenta cada afirmação.</p>
<div class="rolo">
  <table>
    <thead><tr><th class="k">#</th><th>recurso</th><th>como está</th><th>o que falta</th></tr></thead>
    <tbody>
{linhas()}
    </tbody>
  </table>
</div>

<h2>Leitura</h2>
<div class="leitura">
{leitura_html}
</div>

<footer>
  Gerado de <code>docs/STATUS.md</code> por <code>docs/dossie/pagina-de-status.py</code>
  em {agora}. Os números do painel vêm de <code>CAPABILITIES.json</code> e dos
  <code>resultados.json</code> de <code>bancada/gestao</code>, <code>bancada/replicacao</code>
  e <code>bancada/cluster</code>, cada um com a própria data. O dossiê técnico, a
  página dos pedidos, a dos testes e a dos gráficos são as outras quatro.
</footer>
</div>
"""


def main():
    so_medir = "--so-medir" in sys.argv
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    saida = pathlib.Path(args[0]).resolve() if args else PADRAO
    data_av, itens, leitura = ler()
    versao, fichas = medir()
    agora = datetime.datetime.now().strftime("%d/%m/%Y %H:%M")
    print(f"avaliacao de {data_av}: " + ", ".join(f"{i['letra']}={i['nota']}" for i in itens))
    for f in fichas:
        print(f"  {f['r']}: {f['v']} ({re.sub('<[^>]+>', '', f['q'])})")
    if so_medir:
        return
    saida.write_text(pagina(data_av, itens, leitura, versao, fichas, agora), encoding="utf-8")
    print(f"pagina gravada: {saida}")


if __name__ == "__main__":
    main()
