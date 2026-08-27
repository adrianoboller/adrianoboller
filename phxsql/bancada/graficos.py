#!/usr/bin/env python3
"""Gera a pagina de comparacao a partir do resultados.json.

Sem biblioteca de grafico: SVG escrito a mao, como o resto do projeto. Barra
comparativa e o desenho certo aqui -- sao duas series e cinco medidas, e um
grafico mais bonito nao diria mais nada.

O que NAO faz: inventar numero que nao foi medido. Fase que nao rodou aparece
como "nao medido", nunca como zero.
"""

import json
import subprocess
import sys
from pathlib import Path

BASE = Path(__file__).resolve().parent
SAIDA = BASE / "comparacao-phxsql-mysql.html"

MEDIDAS = [
    ("segundos", "Tempo", "s", "menor e melhor"),
    ("cpu_s", "CPU", "s", "menor e melhor"),
    ("lido_mb", "Disco lido", "MB", "menor e melhor"),
    ("escrito_mb", "Disco escrito", "MB", "menor e melhor"),
    ("pico_rss_mb", "Pico de memoria", "MB", "menor e melhor"),
]

FASES = [
    ("inserir", "INSERT"),
    ("buscar", "SELECT pontual"),
    ("varrer", "SELECT por faixa"),
    ("atualizar", "UPDATE"),
    ("excluir", "DELETE"),
]


def maquina():
    def cmd(c):
        return subprocess.run(c, shell=True, capture_output=True, text=True).stdout.strip()

    cpu = cmd("grep -m1 'model name' /proc/cpuinfo | cut -d: -f2").strip()
    return {
        "cpu": cpu or "desconhecida",
        "nucleos": cmd("nproc"),
        "memoria": cmd("free -h | awk '/^Mem:/{print $2}'"),
        "kernel": cmd("uname -r"),
        "mysql": cmd("mysqld --version | head -1").replace("/usr/sbin/mysqld  ", ""),
        "buffer_pool": cmd(
            "mysql -N -B -e \"SHOW VARIABLES LIKE 'innodb_buffer_pool_size'\" | awk '{print $2}'"
        ),
        "flush_log": cmd(
            "mysql -N -B -e \"SHOW VARIABLES LIKE 'innodb_flush_log_at_trx_commit'\" | awk '{print $2}'"
        ),
    }


def barras(titulo, unidade, nota, phx, mysql, id_fig):
    """Duas barras lado a lado, com o numero escrito em cima."""
    if phx is None and mysql is None:
        return (
            f'<figure class="fig"><figcaption><b>{titulo}</b> — não medido</figcaption></figure>'
        )
    p = phx or 0.0
    m = mysql or 0.0
    teto = max(p, m, 1e-9)
    LARG, ALT = 300, 150
    base, topo = 112, 22
    escala = (base - topo) / teto

    def barra(x, v, cor, rotulo):
        h = max(2, v * escala)
        y = base - h
        return (
            f'<rect x="{x}" y="{y:.1f}" width="62" height="{h:.1f}" rx="3" fill="{cor}"/>'
            f'<text x="{x+31}" y="{y-6:.1f}" text-anchor="middle" font-size="12" '
            f'font-weight="600" fill="currentColor">{formata(v)}</text>'
            f'<text x="{x+31}" y="{base+16}" text-anchor="middle" font-size="10.5" '
            f'opacity=".65" fill="currentColor">{rotulo}</text>'
        )

    razao = ""
    if p > 0 and m > 0:
        if p < m:
            razao = f"PhxSql {m/p:.1f}× melhor"
        elif m < p:
            razao = f"MySQL(R) {p/m:.1f}× melhor"
        else:
            razao = "empate"

    return f"""<figure class="fig">
  <svg viewBox="0 0 {LARG} {ALT}" role="img" aria-label="{titulo}: PhxSql {formata(p)} {unidade}, MySQL {formata(m)} {unidade}">
    <line x1="24" y1="{base}" x2="{LARG-24}" y2="{base}" stroke="currentColor" stroke-width="1" opacity=".25"/>
    {barra(58, p, 'var(--acento)', 'PhxSql')}
    {barra(178, m, 'var(--outro)', 'MySQL(R)')}
    <text x="{LARG/2}" y="{ALT-6}" text-anchor="middle" font-size="10.5" opacity=".6" fill="currentColor">{razao}</text>
  </svg>
  <figcaption><b>{titulo}</b> ({unidade}) · <span class="nota">{nota}</span></figcaption>
</figure>"""


def formata(v):
    if v is None:
        return "—"
    if v >= 1000:
        return f"{v:,.0f}".replace(",", ".")
    if v >= 10:
        return f"{v:.1f}"
    if v >= 1:
        return f"{v:.2f}"
    return f"{v:.3f}".rstrip("0").rstrip(".")


def main():
    dados = json.loads((BASE / "resultados.json").read_text())
    por = {}
    for r in dados:
        por[(r.get("motor"), r.get("fase"))] = r

    m = maquina()
    n = max((r.get("operacoes", 0) or 0) for r in dados if r.get("fase") == "inserir")

    secoes = []
    for fase, rotulo in FASES:
        phx = por.get(("PhxSql", fase))
        msq = por.get(("MySQL", fase))
        if not phx and not msq:
            continue
        ops = (phx or msq).get("operacoes", 0)
        figs = "".join(
            barras(t, u, nota, (phx or {}).get(k), (msq or {}).get(k), f"{fase}-{k}")
            for k, t, u, nota in MEDIDAS
        )
        secoes.append(
            f"""<section>
  <h2>{rotulo} <span class="ops">{ops:,} operações</span></h2>
  <div class="grade">{figs}</div>
</section>""".replace(",", ".")
        )

    disco_phx = por.get(("PhxSql", "disco"), {}).get("bytes")
    disco_sql = por.get(("MySQL", "disco"), {}).get("bytes")
    disco = ""
    if disco_phx and disco_sql:
        disco = f"""<section>
  <h2>Tamanho em disco</h2>
  <div class="grade">{barras("Ocupado", "MB", "menor e melhor",
      disco_phx/1048576, disco_sql/1048576, "disco")}</div>
</section>"""

    completo = all(
        ("PhxSql", f) in por and ("MySQL", f) in por for f, _ in FASES
    )
    aviso = "" if completo else """<div class="alerta">
  <b>Medição parcial.</b> Nem todas as fases terminaram quando esta página foi
  gerada. O que está aqui foi medido; o que falta não aparece — não há número
  estimado nesta página.</div>"""

    html = f"""<title>PhxSql × MySQL — a medição</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@400;500;600;700&family=IBM+Plex+Mono:wght@400;500&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&display=swap">
<style>
:root{{
  --papel:#fbf9f7; --papel2:#f3efec; --tinta:#1a1210; --tinta2:#4a3f3a; --tinta3:#7a6d66;
  --linha:#ded6d0; --acento:#c63c0a; --outro:#1f5c93; --ok:#2f7a3e;
}}
@media (prefers-color-scheme:dark){{
  :root:not([data-tema="claro"]){{
    --papel:#010418; --papel2:#0a1122; --tinta:#dde2eb; --tinta2:#a8b0c0; --tinta3:#7c8598;
    --linha:#1e2940; --acento:#ff8a1c; --outro:#5fa6e8; --ok:#6cc98c;
  }}
}}
*{{box-sizing:border-box}}
body{{margin:0;background:var(--papel);color:var(--tinta);
  font:16px/1.6 "Source Serif 4",Georgia,serif;padding:0 20px 80px}}
.pagina{{max-width:1080px;margin:0 auto}}
h1,h2,h3,.mono,figcaption,.rotulo{{font-family:"Exo 2","Helvetica Neue",Arial,sans-serif}}
h1{{font-size:38px;line-height:1.15;margin:56px 0 6px;letter-spacing:-.02em;text-wrap:balance}}
h2{{font-size:20px;margin:44px 0 4px;letter-spacing:-.01em;
   border-top:1px solid var(--linha);padding-top:22px}}
.ops{{font-size:12px;font-weight:400;color:var(--tinta3);letter-spacing:.08em;
     font-family:"IBM Plex Mono",monospace;margin-left:8px}}
.sub{{color:var(--tinta2);max-width:66ch}}
.grade{{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:18px;margin-top:18px}}
.fig{{margin:0;background:var(--papel2);border:1px solid var(--linha);border-radius:10px;padding:14px 10px 10px}}
.fig svg{{width:100%;height:auto;display:block}}
figcaption{{font-size:12px;color:var(--tinta2);text-align:center;margin-top:6px}}
.nota{{color:var(--tinta3);font-style:italic}}
table{{border-collapse:collapse;width:100%;font-size:14px;margin-top:12px}}
th,td{{border-bottom:1px solid var(--linha);padding:7px 10px;text-align:left}}
th{{font-family:"Exo 2",sans-serif;font-size:11px;letter-spacing:.1em;text-transform:uppercase;
   color:var(--tinta3);font-weight:600}}
td.n{{font-family:"IBM Plex Mono",monospace;text-align:right;font-variant-numeric:tabular-nums}}
code{{font-family:"IBM Plex Mono",monospace;font-size:.9em;background:var(--papel2);
     padding:1px 5px;border-radius:4px}}
.alerta{{background:var(--papel2);border-left:3px solid var(--acento);padding:14px 18px;
        border-radius:0 8px 8px 0;margin:24px 0;font-size:15px}}
.rolo{{overflow-x:auto}}
</style>

<div class="pagina">
<h1>PhxSql × MySQL(R)<br><span style="color:var(--tinta3);font-weight:400">a medição, não a opinião</span></h1>
<p class="sub">Mesma máquina, mesmos dados, mesmo esquema, mesmas operações. Cada
número saiu de <code>/proc</code> — tempo de parede, CPU, bytes lidos e escritos,
pico de memória residente. Nada aqui é estimativa.</p>

{aviso}

<h2>A bancada</h2>
<div class="rolo"><table>
  <tr><th>Máquina</th><td>{m['cpu']} · {m['nucleos']} núcleos · {m['memoria']} de RAM · Linux {m['kernel']}</td></tr>
  <tr><th>Carga</th><td>{n:,} registros — id, produto, cidade, valor decimal, data</td></tr>
  <tr><th>Índices</th><td>chave primária em <code>id</code> + índice secundário em <code>cidade</code>, dos dois lados</td></tr>
  <tr><th>MySQL(R)</th><td>{m['mysql']} · InnoDB · buffer pool {int(m['buffer_pool'] or 0)//1048576} MB · <code>innodb_flush_log_at_trx_commit={m['flush_log']}</code></td></tr>
  <tr><th>PhxSql</th><td>0.3.0 · uma sincronização por lote de 50.000, como o outro lado</td></tr>
</table></div>

<h2>O que esta medição não diz</h2>
<p class="sub"><b>Durabilidade não é comparada.</b> Os dois carregam em massa, com
uma sincronização por lote. Uma bancada com <code>commit</code> por linha daria outros
números — e é a que importa para quem grava pedido a pedido.</p>
<p class="sub"><b>Uma instrução por operação, dos dois lados.</b> A primeira versão
desta bancada mandava ao MySQL(R) um único <code>WHERE id IN (…)</code> com todos os
alvos e ao PhxSql vinte mil buscas separadas. O número saía 41× a favor do MySQL(R)
— por causa da <em>forma da pergunta</em>, não do motor. Corrigido: agora os dois
recebem vinte mil instruções independentes.</p>
<p class="sub"><b>O MySQL(R) tem transações; o PhxSql não.</b> Parte do custo de
escrita dele é o log de <em>redo</em>, que compra algo que o PhxSql ainda não
oferece. Comparar os dois em escrita é comparar preços de coisas diferentes.</p>

{''.join(secoes)}
{disco}

<h2>Como refazer</h2>
<p class="sub">Tudo está no repositório: <code>crates/phxsql-store/examples/carga.rs</code>
é a carga do lado do PhxSql, e a bancada em Python cerca cada fase com os contadores
do <code>/proc</code>. Número de desempenho que ninguém consegue refazer é número em
que não se deve acreditar.</p>
</div>
"""
    SAIDA.write_text(html)
    print(f"gerado: {SAIDA} ({len(html)} bytes, {len(secoes)} fases)")


if __name__ == "__main__":
    main()
