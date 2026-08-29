#!/usr/bin/env python3
"""Gera a pagina «O que voce pediu» a partir do docs/PENDENCIAS.md.

    python3 docs/dossie/pagina-dos-pedidos.py [saida.html]

A pagina nao se digita, e a razao e a mesma do selo do dossie: numero
digitado a mao envelhece calado. Aqui a lista tem 129 linhas e tres
contadores -- mantida a mao, ela estaria errada no dia seguinte.

A fonte da verdade e uma so: `docs/PENDENCIAS.md`. Mexeu la, rode isto.
"""

import html
import json
import pathlib
import re
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]
FONTE = RAIZ / "docs" / "PENDENCIAS.md"
PADRAO = RAIZ / "docs" / "dossie" / "pedidos.html"

# O estado vem do emoji da primeira coluna da tabela.
ESTADOS = {
    "☑️": ("feito", "Feito"),
    "◐": ("parcial", "Parcial"),
    "☐": ("planejado", "Planejado"),
}

LINHA = re.compile(r"^\|\s*(☑️|◐|☐)\s*\|\s*(\d+)\s*\|\s*(.*?)\s*\|\s*(.*?)\s*\|\s*$")


def marcar(t):
    """O pouco de Markdown que as celulas usam, virando HTML.

    Escapa ANTES de converter: senao um `<` do texto viraria tag.
    """
    t = html.escape(t)
    t = re.sub(r"`([^`]+)`", r"<code>\1</code>", t)
    t = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", t)
    t = re.sub(r"(?<!\*)\*([^*]+)\*(?!\*)", r"<em>\1</em>", t)
    return t


def ler():
    itens = []
    for l in FONTE.read_text(encoding="utf-8").split("\n"):
        m = LINHA.match(l)
        if m:
            classe, rotulo = ESTADOS[m.group(1)]
            itens.append(
                {
                    "classe": classe,
                    "rotulo": rotulo,
                    "n": int(m.group(2)),
                    "pedido": marcar(m.group(3)),
                    "estado": marcar(m.group(4)),
                }
            )
    if not itens:
        raise SystemExit("nenhuma linha reconhecida em PENDENCIAS.md")
    ns = [i["n"] for i in itens]
    if len(set(ns)) != len(ns):
        raise SystemExit("ha numero de pedido repetido em PENDENCIAS.md")
    return itens


CABECA = """<title>Os {n} pedidos do PhxSql</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@400;500;600;700&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=IBM+Plex+Mono:wght@400;500;600&display=swap">
<style>
:root{
  --papel:#fbf9f7; --papel-2:#f3efec; --papel-3:#e9e3de;
  --tinta:#1a1210; --tinta-2:#4a3f3a; --tinta-3:#7a6d66;
  --linha:#ded6d0;
  --acento:#c63c0a;
  --feito:#2f7a3e; --parcial:#8a6a1f; --planejado:#7a6d66;
  --sombra:0 1px 2px rgba(26,18,16,.06),0 8px 24px rgba(26,18,16,.05);
}
@media (prefers-color-scheme:dark){
  :root:not([data-theme="light"]){
    --papel:#040814; --papel-2:#0a1122; --papel-3:#131c31;
    --tinta:#dde2eb; --tinta-2:#a8b0c0; --tinta-3:#7c8598;
    --linha:#1e2940;
    --acento:#ff8a1c;
    --feito:#5cbf74; --parcial:#d5a83c; --planejado:#7c8598;
    --sombra:0 1px 2px rgba(0,0,0,.4),0 8px 24px rgba(0,0,0,.3);
  }
}
:root[data-theme="dark"]{
  --papel:#040814; --papel-2:#0a1122; --papel-3:#131c31;
  --tinta:#dde2eb; --tinta-2:#a8b0c0; --tinta-3:#7c8598;
  --linha:#1e2940;
  --acento:#ff8a1c;
  --feito:#5cbf74; --parcial:#d5a83c; --planejado:#7c8598;
  --sombra:0 1px 2px rgba(0,0,0,.4),0 8px 24px rgba(0,0,0,.3);
}
*{box-sizing:border-box}
body{
  margin:0;background:var(--papel);color:var(--tinta);
  font-family:"Source Serif 4",Georgia,"Times New Roman",serif;
  font-size:16px;line-height:1.55;
  -webkit-font-smoothing:antialiased;
}
h1,h2,h3,.rotulo,.pino,.filtros button{font-family:"Exo 2","Helvetica Neue",Arial,sans-serif}
code,.mono,.num{font-family:"IBM Plex Mono",ui-monospace,Menlo,monospace}
code{
  font-size:.86em;background:var(--papel-2);
  padding:1px 4px;border-radius:3px;color:var(--tinta-2);
}
.envelope{max-width:1080px;margin:0 auto;padding:0 20px 80px}

header{padding:52px 0 28px;border-bottom:1px solid var(--linha)}
.rotulo{
  font-size:10.5px;letter-spacing:.18em;text-transform:uppercase;
  color:var(--acento);font-weight:600;margin-bottom:12px;
}
h1{
  font-size:clamp(30px,5vw,46px);font-weight:700;line-height:1.08;
  margin:0 0 14px;letter-spacing:-.015em;text-wrap:balance;
}
h1 .x{color:var(--acento)}
.chamada{
  max-width:64ch;color:var(--tinta-2);font-size:17px;margin:0 0 4px;
}

.placar{
  display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));
  gap:10px;margin:30px 0 0;
}
.placar .c{
  border:1px solid var(--linha);border-radius:6px;padding:14px 16px;
  background:var(--papel-2);
}
.placar .v{
  font-family:"Exo 2",sans-serif;font-size:34px;font-weight:700;
  line-height:1;font-variant-numeric:tabular-nums;
}
.placar .r{
  font-family:"IBM Plex Mono",monospace;font-size:10px;
  letter-spacing:.13em;text-transform:uppercase;color:var(--tinta-3);
  margin-top:7px;
}
.placar .feito .v{color:var(--feito)}
.placar .parcial .v{color:var(--parcial)}
.placar .planejado .v{color:var(--planejado)}

h2{
  font-size:22px;font-weight:600;margin:52px 0 6px;letter-spacing:-.01em;
  scroll-margin-top:74px;
}
h2 + .sub{color:var(--tinta-3);font-size:15px;margin:0 0 20px;max-width:64ch}

.barra{
  position:sticky;top:0;z-index:5;background:var(--papel);
  border-bottom:1px solid var(--linha);
  padding:12px 0;margin:0 0 4px;
  display:flex;flex-wrap:wrap;gap:10px;align-items:center;
}
.filtros{display:flex;flex-wrap:wrap;gap:6px}
.filtros button{
  font-size:12.5px;font-weight:500;cursor:pointer;
  background:none;color:var(--tinta-2);
  border:1px solid var(--linha);border-radius:999px;
  padding:5px 13px;
}
.filtros button:hover{border-color:var(--acento);color:var(--acento)}
.filtros button[aria-pressed="true"]{
  border-color:var(--acento);color:var(--acento);
  background:color-mix(in srgb,var(--acento) 9%,transparent);
}
.filtros button:focus-visible,input:focus-visible{
  outline:2px solid var(--acento);outline-offset:2px;
}
input[type="search"]{
  font-family:"Source Serif 4",Georgia,serif;font-size:14px;
  background:var(--papel-2);color:var(--tinta);
  border:1px solid var(--linha);border-radius:5px;
  padding:6px 11px;min-width:190px;flex:1;max-width:280px;
}
.conta{
  font-family:"IBM Plex Mono",monospace;font-size:11px;
  color:var(--tinta-3);margin-left:auto;white-space:nowrap;
}

.rolo{overflow-x:auto;-webkit-overflow-scrolling:touch}
table{border-collapse:collapse;width:100%;min-width:660px}
/* Cabecalho NAO grudento: `.rolo` tem `overflow-x:auto`, e isso faz dele um
   contexto de rolagem proprio -- o `position:sticky` do `thead` passava a se
   medir por ele e caia POR CIMA da primeira linha. Quem gruda e a barra de
   filtro, que e onde esta a contagem. */
thead th{
  font-family:"IBM Plex Mono",monospace;font-weight:500;
  font-size:10px;letter-spacing:.13em;text-transform:uppercase;
  color:var(--tinta-3);text-align:left;
  padding:12px 12px 8px;border-bottom:1px solid var(--linha);
  background:var(--papel);
}
tbody td{
  padding:14px 12px;border-bottom:1px solid var(--linha);
  vertical-align:top;font-size:15px;
}
tbody tr:hover td{background:var(--papel-2)}
td.n{
  font-size:12px;color:var(--tinta-3);text-align:right;
  font-variant-numeric:tabular-nums;width:44px;white-space:nowrap;
}
td.p{width:34%;color:var(--tinta)}
td.e{color:var(--tinta-2);font-size:14.5px}
th.st,td.st{width:104px}

/* A forma tambem carrega o estado, e nao so a cor: cheio, meio, vazio. */
.pino{
  display:inline-flex;align-items:center;gap:6px;
  font-size:11px;font-weight:600;letter-spacing:.03em;white-space:nowrap;
}
.pino::before{
  content:"";width:9px;height:9px;border-radius:50%;
  border:1.5px solid currentColor;flex:none;
}
.pino.feito{color:var(--feito)}
.pino.feito::before{background:currentColor}
.pino.parcial{color:var(--parcial)}
.pino.parcial::before{background:linear-gradient(90deg,currentColor 50%,transparent 50%)}
.pino.planejado{color:var(--planejado)}

.vazio{padding:38px 12px;color:var(--tinta-3);text-align:center;font-style:italic}

.nota{
  border-left:3px solid var(--acento);background:var(--papel-2);
  padding:14px 18px;border-radius:0 5px 5px 0;margin:26px 0;
  font-size:15px;color:var(--tinta-2);max-width:66ch;
}
.nota .t{
  display:block;font-family:"Exo 2",sans-serif;font-weight:600;
  color:var(--tinta);font-size:14px;margin-bottom:5px;
}
footer{
  margin-top:56px;padding-top:22px;border-top:1px solid var(--linha);
  color:var(--tinta-3);font-size:13.5px;max-width:66ch;
}
@media (prefers-reduced-motion:reduce){*{transition:none!important;animation:none!important}}
</style>"""


def cabeca(n):
    return CABECA.replace("{n}", str(n))


def corpo(itens):
    n = len(itens)
    contas = {c: sum(1 for i in itens if i["classe"] == c) for c in ("feito", "parcial", "planejado")}
    abertos = [i for i in itens if i["classe"] != "feito"]

    if abertos:
        estados = []
        if contas["parcial"]:
            estados.append(f"{contas['parcial']} pela metade")
        if contas["planejado"]:
            estados.append(f"{contas['planejado']} sem começar")
        abertura = (
            f"Os {len(abertos)} que não fecharam ({' e '.join(estados)}), com o "
            f"motivo de cada um. O motivo importa mais que o estado: uns esperam "
            f"trabalho, outros esperam uma decisão sua, e outros esperam coisa "
            f"de fora deste repositório."
        )
    else:
        abertura = "Nenhum pedido em aberto."

    def linhas(ls):
        return "\n".join(
            f'      <tr data-e="{i["classe"]}">'
            f'<td class="n mono">{i["n"]}</td>'
            f'<td class="st"><span class="pino {i["classe"]}">{i["rotulo"]}</span></td>'
            f'<td class="p">{i["pedido"]}</td>'
            f'<td class="e">{i["estado"]}</td></tr>'
            for i in ls
        )

    return f"""<div class="envelope">
<header>
  <div class="rotulo">PhxSql · o dossiê do que foi pedido</div>
  <h1>Os {n} pedidos, e o<br>que existe de cada um</h1>
  <p class="chamada">Uma linha por pedido seu, na ordem em que você pediu. O
  estado é <strong>medido contra o código</strong>, não contra a lembrança — foi
  assim que a chave estrangeira saiu de «pronto» para «parcial», e o Centro de
  Controle de «pronto» para «só navega».</p>

  <div class="placar">
    <div class="c"><div class="v">{n}</div><div class="r">pedidos</div></div>
    <div class="c feito"><div class="v">{contas['feito']}</div><div class="r">feitos</div></div>
    <div class="c parcial"><div class="v">{contas['parcial']}</div><div class="r">parciais</div></div>
    <div class="c planejado"><div class="v">{contas['planejado']}</div><div class="r">planejados</div></div>
  </div>
</header>

<h2>O que está aberto</h2>
<p class="sub">{abertura}</p>
<div class="rolo">
  <table>
    <thead><tr><th class="n">#</th><th class="st">estado</th><th>o que você pediu</th><th>onde está</th></tr></thead>
    <tbody>
{linhas(abertos)}
    </tbody>
  </table>
</div>

<div class="nota">
  <span class="t">Por que «parcial» e não «feito»</span>
  Meio caminho andado continua sendo meio caminho, e a lista diz <em>qual</em>
  metade — porque é a metade que falta que decide se o pedido serve para alguma
  coisa hoje. O estado sai da primeira coluna do
  <code>docs/PENDENCIAS.md</code>, que é medido contra o código; esta página
  não tem opinião própria sobre nenhum item.
</div>

<h2 id="todos">Os {n}, na ordem em que você pediu</h2>
<p class="sub">A numeração é a sequência real dos seus pedidos — ela diz
<em>quando</em> cada coisa foi pedida, e é por isso que vale mantê-la.</p>

<div class="barra">
  <div class="filtros" role="group" aria-label="Filtrar por estado">
    <button type="button" data-f="todos" aria-pressed="true">Todos</button>
    <button type="button" data-f="feito" aria-pressed="false">Feitos</button>
    <button type="button" data-f="parcial" aria-pressed="false">Parciais</button>
    <button type="button" data-f="planejado" aria-pressed="false">Planejados</button>
  </div>
  <input type="search" id="busca" placeholder="procurar…" aria-label="Procurar nos pedidos">
  <span class="conta" id="conta"></span>
</div>

<div class="rolo">
  <table id="tudo">
    <thead><tr><th class="n">#</th><th class="st">estado</th><th>o que você pediu</th><th>o que existe hoje</th></tr></thead>
    <tbody>
{linhas(itens)}
    </tbody>
  </table>
  <div class="vazio" id="vazio" hidden>Nenhum pedido bate com isso.</div>
</div>

<footer>
  Gerado de <code>docs/PENDENCIAS.md</code> por
  <code>docs/dossie/pagina-dos-pedidos.py</code> — a lista não se digita, pela
  mesma razão que o selo do dossiê não se digita. O dossiê técnico, com o
  formato byte a byte e a bancada medida, é a outra página.
</footer>
</div>

<script>
(function(){{
  const linhas = Array.from(document.querySelectorAll('#tudo tbody tr'));
  const botoes = Array.from(document.querySelectorAll('.filtros button'));
  const busca  = document.getElementById('busca');
  const conta  = document.getElementById('conta');
  const vazio  = document.getElementById('vazio');
  let filtro = 'todos';

  // Sem isto, quem digita «indice» nao acha «indice» com acento -- e em
  // portugues isso e a busca falhando calada, nao uma sutileza.
  const achatar = t => t.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase();
  linhas.forEach(tr => tr.dataset.busca = achatar(tr.textContent));

  function aplicar(){{
    const q = achatar(busca.value.trim());
    let vistos = 0;
    for (const tr of linhas) {{
      const okEstado = filtro === 'todos' || tr.dataset.e === filtro;
      const okTexto  = !q || tr.dataset.busca.includes(q);
      const mostra = okEstado && okTexto;
      tr.hidden = !mostra;
      if (mostra) vistos++;
    }}
    conta.textContent = vistos + ' de ' + linhas.length;
    vazio.hidden = vistos !== 0;
  }}

  botoes.forEach(b => b.addEventListener('click', () => {{
    filtro = b.dataset.f;
    botoes.forEach(o => o.setAttribute('aria-pressed', String(o === b)));
    aplicar();
  }}));
  busca.addEventListener('input', aplicar);
  aplicar();
}})();
</script>"""


ABRE_C = "<!-- pedidos:contagem:inicio -->"
FECHA_C = "<!-- pedidos:contagem:fim -->"


def gravar_contagem(itens):
    """Escreve a contagem DE VOLTA no PENDENCIAS.md, entre as marcas.

    A linha «123 feitos . 5 parciais . 4 planejados» ficou digitada no proprio
    arquivo que a produz, e passou rodadas errada -- com a tabela logo acima
    dizendo outra coisa. Somar aqui e escrever la e a mesma receita do
    `numeros-da-bancada.py`: quem conta e quem sabe contar.
    """
    md = FONTE.read_text(encoding="utf-8")
    i, j = md.find(ABRE_C), md.find(FECHA_C)
    if i < 0 or j < 0:
        return False
    c = {k: sum(1 for x in itens if x["classe"] == k)
         for k in ("feito", "parcial", "planejado")}
    bloco = (
        f"**{len(itens)} pedidos: {c['feito']} feitos · {c['parcial']} parciais · "
        f"{c['planejado']} planejados.**\n\n"
        "*(Gerado por `docs/dossie/pagina-dos-pedidos.py` — não conte à mão. A\n"
        "conta sai da primeira coluna da tabela acima, e é a mesma que a página\n"
        "dos pedidos mostra: se as duas discordarem, é porque alguém digitou uma\n"
        "delas.)*"
    )
    md = md[:i] + ABRE_C + "\n" + bloco + "\n" + FECHA_C + md[j + len(FECHA_C):]
    FONTE.write_text(md, encoding="utf-8")
    return True


def main():
    saida = pathlib.Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else PADRAO
    itens = ler()
    saida.write_text(cabeca(len(itens)) + "\n" + corpo(itens) + "\n", encoding="utf-8")
    contas = {c: sum(1 for i in itens if i["classe"] == c) for c in ("feito", "parcial", "planejado")}
    print(f"{len(itens)} pedidos: {contas['feito']} feitos, "
          f"{contas['parcial']} parciais, {contas['planejado']} planejados")
    print(f"pagina gravada: {saida}")
    if gravar_contagem(itens):
        print(f"contagem gravada de volta em {FONTE.name}")


if __name__ == "__main__":
    main()
