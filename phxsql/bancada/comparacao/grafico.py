#!/usr/bin/env python3
"""Desenha o grafico dos tres motores a partir do JSON medido.

    python3 bancada/comparacao/grafico.py [um-milhao.json]

Escreve `comparacao-tres-motores.svg` (fragmento, para entrar no dossie) e
`comparacao-tres-motores.html` (pagina de pe, para olhar sozinha).

Tres decisoes que valem explicar:

**Um painel por fase, com escala propria.** Inserir um milhao de linhas e
buscar uma custam ordens de grandeza diferentes. Num eixo compartilhado a fase
barata vira um risco no chao e nao se le -- e ninguem precisa comparar o tempo
de inserir com o de buscar, entao o eixo compartilhado nao compra nada e custa
a legibilidade das fases rapidas.

**A dispersao aparece.** Cada barra leva o bigode de minimo a maximo. Mediana
sozinha esconde que a medida de `fsync` varia 1,2x a 1,6x entre dias -- foi
medido aqui -- e uma barra lisa afirmaria uma precisao que o numero nao tem.

**Fase nao medida vira «nao medido», nunca zero.** E a mesma regra do
`graficos.py` que ja existia: barra de altura zero se le como "foi
instantaneo", que e a mentira mais facil de publicar sem querer.

As cores sao os tokens da marca, e passaram pelo validador do skill de
visualizacao nos DOIS temas -- claro e escuro tem passos proprios, porque o
tema escuro nao e o claro invertido. Cada barra tambem leva rotulo direto, que
e a codificacao secundaria que a separacao tritan pede.
"""
import json
import sys
from pathlib import Path

BASE = Path(__file__).resolve().parent
MOTORES = [
    ("phxsql", "PhxSql", "var(--m-phx)"),
    ("mysql", "MySQL(R)", "var(--m-sql)"),
    ("sqlite", "SQLite(R)", "var(--m-lite)"),
]
# A nota de cada fase e um MOLDE: o numero de operacoes sai da medicao, e
# nao de um texto digitado. A do UPDATE dizia «trocar o valor de uma coluna»
# e estava errada -- as tres regravam a linha inteira, porque o `carga.rs`
# regrava, e trocar so uma coluna seria menos trabalho de um lado.
FASES = [
    ("inserir", "INSERT", "gravar {n} linhas, uma a uma"),
    ("buscar", "SELECT", "achar {ops} linhas pela chave, uma instrução cada"),
    ("atualizar", "UPDATE", "regravar a linha inteira de {ops} delas"),
    ("excluir", "DELETE", "apagar {ops} de vez"),
]

LARG, ALT = 460, 224
ESQ, DIR, TOPO, BASE_Y = 96, 24, 46, 54


def mil(x):
    return f"{x:,}".replace(",", ".")


def numero(v, casas):
    """Virgula decimal e ponto de milhar -- a pagina e em portugues.

    Saia com ponto decimal («9.93 s») ate esta rodada. Nao muda o valor, mas
    muda a LEITURA: quem le em portugues ve «nove mil e noventa e tres».
    """
    bruto = f"{v:,.{casas}f}"           # 1,234.56
    return bruto.replace(",", "\x00").replace(".", ",").replace("\x00", ".")


def fmt(v):
    if v is None:
        return "nao medido"
    if v >= 100:
        return f"{numero(v, 0)} s"
    if v >= 10:
        return f"{numero(v, 1)} s"
    if v >= 1:
        return f"{numero(v, 2)} s"
    return f"{numero(v * 1000, 0)} ms"


def vencedor(candidatos):
    """Quem ganhou a fase, ou None quando as faixas se cruzam.

    `candidatos` sao tuplas (nome, mediana, minimo, maximo), so dos motores
    MEDIDOS -- comparar contra fase que nao rodou daria vencedor por ausencia
    do outro.

    So ha vencedor quando a faixa do primeiro NAO cruza a do segundo. Marcar
    164 ms contra 166 ms, com as duas faixas sobrepostas (151-215 contra
    158-232), e publicar ruido da maquina como resultado.

    Mora AQUI, e o dossie a importa daqui, porque a primeira versao tinha duas
    copias da regra: consertei a do grafico e a da tabela continuou marcando
    vencedor na busca -- o documento se contradizia a dois centimetros de
    distancia.
    """
    if not candidatos:
        return None
    if len(candidatos) == 1:
        return candidatos[0][0]
    ordem = sorted(candidatos, key=lambda x: x[1])
    p1, p2 = ordem[0], ordem[1]
    teto_do_1o = p1[3] if p1[3] is not None else p1[1]
    piso_do_2o = p2[2] if p2[2] is not None else p2[1]
    return p1[0] if teto_do_1o < piso_do_2o else None


def painel(chave, titulo, nota, dados):
    """Um painel: tres barras horizontais com bigode de minimo a maximo."""
    linhas = []
    for mid, rotulo, cor in MOTORES:
        d = (dados or {}).get(mid) or {}
        linhas.append((rotulo, cor, d.get("mediana_s"), d.get("min_s"), d.get("max_s")))

    medidos = [x for x in linhas if x[2] is not None]
    teto = (max(x[2] for x in medidos) * 1.35) if medidos else 1.0
    util = LARG - ESQ - DIR
    alt_barra, passo = 34, 62

    melhor = vencedor([(x[0], x[2], x[3], x[4]) for x in medidos])

    p = [
        f'<text x="0" y="16" class="tit">{titulo}</text>',
        f'<text x="0" y="34" class="nota">{nota}</text>',
    ]
    for i, (rotulo, cor, med, mn, mx) in enumerate(linhas):
        y = TOPO + i * passo
        p.append(f'<text x="0" y="{y + 15}" class="eixo">{rotulo}</text>')
        if med is None:
            p.append(f'<text x="{ESQ}" y="{y + 15}" class="ausente">nao medido</text>')
            continue
        w = max(3, util * med / teto)
        vitoria = ' vencedor' if rotulo == melhor else ''
        p.append(
            f'<rect x="{ESQ}" y="{y}" width="{w:.1f}" height="{alt_barra}" rx="4" '
            f'fill="{cor}" class="barra{vitoria}"/>'
        )
        fim_do_rotulo = ESQ + w
        if mn is not None and mx is not None and mx > mn:
            x1 = min(ESQ + util * mn / teto, ESQ + util)
            estourou = mx > teto
            x2 = ESQ + util if estourou else ESQ + util * mx / teto
            ym = y + alt_barra / 2
            p.append(
                f'<line x1="{x1:.1f}" y1="{ym}" x2="{x2:.1f}" y2="{ym}" class="bigode"/>'
                f'<line x1="{x1:.1f}" y1="{ym - 6}" x2="{x1:.1f}" y2="{ym + 6}" class="bigode"/>'
            )
            if estourou:
                # Seta em vez de traco: o bigode nao termina ali, ele foi
                # CORTADO. Traco no fim leria como «o maximo e este».
                p.append(
                    f'<path d="M{x2 - 7:.1f},{ym - 6} L{x2:.1f},{ym} '
                    f'L{x2 - 7:.1f},{ym + 6}" class="bigode" fill="none"/>'
                )
            else:
                p.append(
                    f'<line x1="{x2:.1f}" y1="{ym - 6}" x2="{x2:.1f}" y2="{ym + 6}"'
                    f' class="bigode"/>'
                )
            fim_do_rotulo = max(fim_do_rotulo, x2)
        # O rotulo vem DEPOIS do bigode, nunca por cima dele -- e se nao
        # couber ali, vem DENTRO da barra, alinhado a direita. A primeira
        # versao so empurrava para a direita, e o texto saia cortado pela
        # borda do painel justamente nas linhas mais interessantes.
        rotulo_valor = fmt(med)
        if mx is not None and mx > teto:
            rotulo_valor += f" (pico {fmt(mx)})"
        largura = len(rotulo_valor) * 7.3          # 13px semibold, medido no desenho
        if fim_do_rotulo + 8 + largura <= LARG - 2:
            p.append(
                f'<text x="{fim_do_rotulo + 8:.1f}" y="{y + 22}" class="valor">'
                f'{rotulo_valor}</text>'
            )
        else:
            p.append(
                f'<text x="{ESQ + w - 8:.1f}" y="{y + 22}" class="valor dentro"'
                f' text-anchor="end">{rotulo_valor}</text>'
            )

    aria = "; ".join(f"{r} {fmt(m)}" for r, _, m, _, _ in linhas)
    return (
        f'<figure class="fig">\n'
        f'  <svg viewBox="0 0 {LARG} {ALT}" role="img" '
        f'aria-label="{titulo}, {nota}: {aria}">\n    '
        + "\n    ".join(p)
        + f'\n  </svg>\n</figure>'
    )


def main():
    origem = Path(sys.argv[1]) if len(sys.argv) > 1 else BASE / "um-milhao.json"
    if not origem.exists():
        sys.exit(
            f"nao achei {origem}.\n"
            "O grafico sai da medicao, e nao ha medicao ainda -- rode a bancada\n"
            "antes. Desenhar sem o numero seria inventar o numero."
        )
    d = json.loads(origem.read_text(encoding="utf-8"))
    fases = d.get("fases") or {}
    n = d.get("linhas", 0)

    ops = d.get("operacoes_por_fase_pontual", 0)
    paineis = "\n".join(
        painel(c, t, nt.format(n=mil(n), ops=mil(ops)), fases.get(c))
        for c, t, nt in FASES
    )
    legenda = " ".join(
        f'<span class="leg"><i style="background:{cor}"></i>{rot}</span>'
        for _, rot, cor in MOTORES
    )
    ress = "".join(f"<li>{r}</li>" for r in (d.get("ressalvas") or []))
    dur = d.get("durabilidade") or {}
    dur_txt = " · ".join(f"<b>{k}</b>: {v}" for k, v in dur.items())

    (BASE / "comparacao-tres-motores.svg").write_text(paineis, encoding="utf-8")

    pag = f"""<!doctype html><meta charset="utf-8">
<title>PhxSql x MySQL(R) x SQLite(R) -- {n:,} linhas</title>
<style>
:root{{--papel:#fbf9f7;--tinta:#1a1210;--tinta-2:#4a3f3a;--tinta-3:#7a6d66;
--linha:#ded6d0;--m-phx:#c63c0a;--m-sql:#1f5c93;--m-lite:#37702e}}
@media (prefers-color-scheme:dark){{:root:not([data-theme="light"]){{
--papel:#040814;--tinta:#dde2eb;--tinta-2:#a8b0c0;--tinta-3:#7c8598;
--linha:#1e2940;--m-phx:#d9741c;--m-sql:#4287cf;--m-lite:#54a84c}}}}
body{{margin:0;padding:32px;background:var(--papel);color:var(--tinta);
font:15px/1.6 system-ui,sans-serif;max-width:1100px}}
h1{{font-size:22px;margin:0 0 4px}}
.sub{{color:var(--tinta-2);margin:0 0 20px}}
.grade{{display:grid;grid-template-columns:repeat(auto-fit,minmax(360px,1fr));gap:24px}}
.fig{{margin:0;border:1px solid var(--linha);border-radius:10px;padding:12px}}
.fig svg{{width:100%;height:auto;display:block}}
.tit{{font:600 15px system-ui;fill:var(--tinta)}}
.nota{{font:12px system-ui;fill:var(--tinta-3)}}
.eixo{{font:13px system-ui;fill:var(--tinta-2)}}
.valor{{font:600 13px system-ui;fill:var(--tinta);font-variant-numeric:tabular-nums}}
.valor.dentro{{fill:#fff}}
.ausente{{font:italic 13px system-ui;fill:var(--tinta-3)}}
.bigode{{stroke:var(--tinta-3);stroke-width:2}}
.barra.vencedor{{stroke:var(--tinta);stroke-width:2}}
.leg{{margin-right:16px;color:var(--tinta-2);font-size:13px}}
.leg i{{display:inline-block;width:11px;height:11px;border-radius:3px;
margin-right:6px;vertical-align:-1px}}
.ress{{margin-top:24px;border-left:3px solid var(--m-phx);padding:4px 0 4px 14px}}
.ress h2{{font-size:15px;margin:0 0 6px}}
.ress li{{color:var(--tinta-2);margin-bottom:4px}}
</style>
<h1>PhxSql &times; MySQL&reg; &times; SQLite&reg;</h1>
<p class="sub">tabela de {n:,} linhas &middot; menor &eacute; melhor &middot; cada painel
tem escala pr&oacute;pria &middot; o bigode vai do m&iacute;nimo ao m&aacute;ximo das rodadas, e a
seta diz que ele foi cortado no fim do painel &middot; o contorno s&oacute; marca vencedor
quando as faixas <em>n&atilde;o</em> se cruzam</p>
<p>{legenda}</p>
<div class="grade">
{paineis}
</div>
<div class="ress">
<h2>Durabilidade de cada um durante a medida</h2>
<p class="sub">{dur_txt}</p>
<h2>O que estes n&uacute;meros n&atilde;o dizem</h2>
<ul>{ress}</ul>
</div>
""".replace("{n:,}".format(n=n), f"{n:,}".replace(",", "."))
    (BASE / "comparacao-tres-motores.html").write_text(pag, encoding="utf-8")
    print(f"grafico gerado de {origem.name}: {n} linhas, {len(FASES)} fases")
    for c, t, _ in FASES:
        got = [m for m, _, _ in MOTORES if ((fases.get(c) or {}).get(m) or {}).get("mediana_s") is not None]
        print(f"  {t:<8} medido para: {', '.join(got) if got else 'NINGUEM'}")


if __name__ == "__main__":
    main()
