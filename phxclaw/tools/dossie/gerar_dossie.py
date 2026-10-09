#!/usr/bin/env python3
"""Dossie do PhxClaw: a pagina INTEIRA sai daqui; nenhum numero visivel e digitado.

    python3 tools/dossie/gerar_dossie.py            # regrava o dossie da pasta (um so)
    python3 tools/dossie/gerar_dossie.py --novo     # cria dossie-phxclaw-<maior.menor>.html
    python3 tools/dossie/gerar_dossie.py --suite ARQ  # placar do cargo test de uma saida guardada

O HTML gerado NAO se edita: mexeu na prosa, mexa aqui; mexeu num numero, rode o medidor dele.
De onde sai cada numero: `numeros.py` (um leitor por fonte, com a data de cada uma) e a tabela
do rodape da propria pagina. O LEIA-ME da pasta docs/dossie diz como publicar.

O que este gerador NAO faz, e diz: nao roda `cargo test` (o placar vem da certificacao gravada
ou de `--suite ARQ`), nao roda os roteiros do Chromium (le o resultado que eles gravaram).
Tudo o que nao tem arquivo de resultado sai como NAO MEDIDO na pagina, com o comando, e na
saida sob «FEZ MENOS» -- que nao e linha de exito.

Publicado em https://claude.ai/artifact/J5emfeTgE26AapFRTPssDk (sempre a mesma URL).
"""
from __future__ import annotations

import html
import re
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
sys.path.insert(0, str(AQUI))

import numeros as N  # noqa: E402
from dossie_da_pasta import PASTA, achar, candidatos  # noqa: E402
from embutir import ASSETS, css_das_faces, data_uri, dimensoes  # noqa: E402
from numerar_figuras import numerar  # noqa: E402

RAIZ = N.RAIZ
URL = "https://claude.ai/artifact/J5emfeTgE26AapFRTPssDk"
TETO_BYTES = 450 * 1024
ESTE = "tools/dossie/gerar_dossie.py"


def e(s) -> str:
    return html.escape(str(s), quote=True)


def md(s: str) -> str:
    """O pouco de Markdown que as fontes trazem (`codigo`, **negrito**), ja escapado."""
    t = e(s.replace("⏸", "«depois da versão»"))
    t = re.sub(r"`([^`]+)`", r"<code>\1</code>", t)
    return re.sub(r"\*\*(.+?)\*\*", r"<b>\1</b>", t)


def pct(x: float) -> str:
    return N.br(x, 1) + "%"


def quando(d: dict) -> str:
    return f'<span class="dt" title="{e(d["como"])}">{e(d["texto"])}</span>'


def nao_medido(o_que: str, comando: str) -> str:
    return (f'<span class="nm">NÃO MEDIDO</span> <span class="mot">{e(o_que)} — rode '
            f'<code>{e(comando)}</code></span>')


# ---------------------------------------------------------------- estilo

def css(tok: dict) -> str:
    escuro, claro = tok["escuro"], tok["claro"]
    # Base = tema claro (o :root vale para quem nao escolheu e esta num sistema claro); os
    # tokens que o claro nao redefine (fonte, escala) vem do bloco escuro, que e o completo.
    base = {**escuro, **claro}
    dark = {k: escuro[k] for k in claro if k in escuro}
    decl = lambda d: ";".join(f"{k}:{v.strip()}" for k, v in d.items())
    return f"""{css_das_faces()}
:root{{{decl(base)};--vis-escuro:none;--vis-claro:block;color-scheme:light}}
@media (prefers-color-scheme: dark){{:root:not([data-theme="light"]){{{decl(dark)};--vis-escuro:block;--vis-claro:none;color-scheme:dark}}}}
:root[data-theme="dark"]{{{decl(dark)};--vis-escuro:block;--vis-claro:none;color-scheme:dark}}
*{{box-sizing:border-box}}
html{{-webkit-text-size-adjust:100%}}
body{{margin:0;background:var(--fundo);color:var(--texto);font:400 16px/1.6 var(--sans);overflow-wrap:break-word}}
a{{color:var(--laranja)}}
code{{font:400 .9em var(--mono);color:var(--texto-2);overflow-wrap:anywhere}}
.so-escuro{{display:var(--vis-escuro)}}.so-claro{{display:var(--vis-claro)}}
.barra{{position:sticky;top:0;z-index:5;display:flex;align-items:center;gap:12px;padding:8px 20px;
  background:var(--painel);border-bottom:1px solid var(--linha)}}
.barra img{{width:32px;height:32px}}
.barra b{{font:700 var(--fs-3) var(--sans);letter-spacing:.02em}}
.barra .vers{{font:400 var(--fs-1) var(--mono);color:var(--texto-3)}}
.botao{{margin-left:auto;font:600 var(--fs-1) var(--sans);text-transform:uppercase;letter-spacing:.06em;
  color:var(--acao-consultar);background:transparent;border:1px solid var(--acao-consultar);
  border-radius:6px;padding:8px 12px;min-height:36px;cursor:pointer}}
@media (hover:hover){{.botao:hover{{background:var(--acao-consultar);color:var(--fundo)}}}}
.botao:focus-visible,a:focus-visible{{outline:2px solid var(--laranja);outline-offset:2px}}
main{{max-width:1180px;margin:0 auto;padding:0 20px 56px}}
p,li{{max-width:74ch}}
.capa{{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,300px);gap:28px;align-items:center;padding:36px 0 8px}}
.capa .abertura img{{width:100%;max-width:300px;height:auto;margin-inline:auto}}
.selo{{display:inline-block;font:400 var(--fs-1) var(--mono);color:var(--texto-2);border:1px solid var(--linha-forte);
  border-radius:12px;padding:3px 12px}}
h1{{font:800 var(--fs-8)/1.1 var(--sans);margin:14px 0 6px;letter-spacing:.01em}}
h1 .x{{color:var(--laranja)}}
.chamada{{color:var(--texto-2);font-size:var(--fs-3)}}
.painel{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,168px),1fr));gap:12px;margin-top:22px}}
.ficha{{background:var(--painel);border:1px solid var(--linha);border-radius:8px;padding:12px 14px;min-width:0}}
.ficha .v{{font:700 var(--fs-6)/1.15 var(--sans);font-variant-numeric:tabular-nums}}
.ficha .v small{{display:block;font-size:var(--fs-2);line-height:1.3;color:var(--texto-2);font-weight:600}}
.ficha .r{{font:600 var(--fs-1) var(--sans);text-transform:uppercase;letter-spacing:.06em;color:var(--texto-2)}}
.ficha .f{{font:400 var(--fs-1) var(--mono);color:var(--texto-3);margin-top:4px;overflow-wrap:anywhere}}
.ficha.falta .v{{color:var(--laranja)}}
.indice{{margin:28px 0 0;background:var(--painel);border:1px solid var(--linha);border-radius:8px;padding:10px 16px}}
.indice summary{{font:600 var(--fs-2) var(--sans);text-transform:uppercase;letter-spacing:.06em;cursor:pointer;min-height:32px}}
.indice ol{{columns:16rem;margin:6px 0 4px;padding-left:0;list-style:none}}
.indice li{{break-inside:avoid;padding:2px 0}}
.indice .n,.rotulo .num{{font:700 var(--fs-1) var(--mono);color:var(--laranja);margin-right:8px}}
section{{margin-top:56px;min-width:0;scroll-margin-top:64px}}
.rotulo{{display:flex;align-items:center;gap:10px}}
.rotulo .traco{{flex:1;border-top:1px solid var(--linha)}}
h2{{font:700 var(--fs-6)/1.2 var(--sans);margin:8px 0 12px}}
h3{{font:700 var(--fs-4)/1.25 var(--sans);margin:28px 0 8px}}
.nota{{font-size:var(--fs-2);color:var(--texto-2)}}
.dt{{font:400 .92em var(--mono);color:var(--texto-3);white-space:nowrap}}
code.id{{white-space:nowrap}}
.mot{{color:var(--texto-2);font-size:var(--fs-2)}}
.nm{{font:700 var(--fs-1) var(--mono);color:var(--aviso);border:1px dashed var(--aviso);border-radius:4px;padding:1px 6px;white-space:nowrap}}
.tab{{overflow-x:auto;background:var(--painel);border:1px solid var(--linha);border-radius:8px;margin:10px 0}}
table{{border-collapse:collapse;width:100%;font-size:var(--fs-2)}}
th,td{{text-align:left;padding:8px 12px;border-bottom:1px solid var(--linha);vertical-align:top}}
th{{font:600 var(--fs-1) var(--sans);text-transform:uppercase;letter-spacing:.06em;color:var(--texto-2);white-space:nowrap}}
th.n{{text-align:right}}
.id{{white-space:nowrap}}
tr:last-child td{{border-bottom:0}}
td.n{{font-family:var(--mono);text-align:right;white-space:nowrap;font-variant-numeric:tabular-nums}}
.pl{{font:600 var(--fs-1) var(--mono);padding:1px 7px;border-radius:99px;border:1px solid;white-space:nowrap}}
.pl.ok{{color:var(--ok)}}.pl.ruim{{color:var(--vermelho)}}.pl.trava{{color:var(--aviso);border-style:dashed}}
.pl.neutro{{color:var(--texto-2);border-style:dotted}}.pl.vivo{{color:var(--info)}}
figure{{margin:18px 0;background:var(--painel);border:1px solid var(--linha);border-radius:8px;padding:16px;min-width:0}}
figcaption{{font-size:var(--fs-2);color:var(--texto-2);margin-top:12px;max-width:74ch}}
figcaption b{{color:var(--texto);font-weight:700}}
.barras{{display:grid;gap:8px}}
.lin{{display:grid;grid-template-columns:minmax(0,9rem) minmax(0,1fr) auto;gap:10px;align-items:center;font-size:var(--fs-2)}}
.lin .nome{{overflow-wrap:anywhere}}
.lin .val{{font:400 var(--fs-1) var(--mono);color:var(--texto-2);white-space:nowrap}}
.trilho{{display:flex;height:16px;border-radius:3px;overflow:hidden;background:var(--painel-3);min-width:0}}
.s-ag{{background:var(--laranja)}}
.s-pa{{background:repeating-linear-gradient(45deg,var(--laranja) 0 3px,transparent 3px 7px);box-shadow:inset 0 0 0 1px var(--laranja)}}
.s-na{{box-shadow:inset 0 0 0 1px var(--texto-3);background:repeating-linear-gradient(90deg,transparent 0 4px,var(--painel-2) 4px 6px)}}
.s-um{{background:var(--info)}}
.legenda{{display:flex;flex-wrap:wrap;gap:6px 16px;font-size:var(--fs-1);color:var(--texto-2);margin-top:10px}}
.legenda i{{display:inline-block;width:14px;height:10px;vertical-align:-1px;margin-right:6px;border-radius:2px}}
.kanban{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,230px),1fr));gap:12px}}
.coluna{{background:var(--painel-2);border:1px solid var(--linha);border-radius:8px;padding:10px;min-width:0}}
.coluna h4{{margin:0 0 8px;font:700 var(--fs-1) var(--sans);text-transform:uppercase;letter-spacing:.06em;display:flex;justify-content:space-between}}
.cartao{{background:var(--painel);border:1px solid var(--linha);border-left:3px solid var(--texto-3);border-radius:6px;padding:8px 10px;margin-top:8px;font-size:var(--fs-1)}}
.cartao b{{font-family:var(--mono)}}
.c-CONCLUIDA{{border-left-color:var(--ok)}}.c-EM{{border-left-color:var(--info)}}
.c-PLANEJADA{{border-left-color:var(--texto-3);border-left-style:dashed}}.c-BLOQUEADA{{border-left-color:var(--aviso);border-left-style:double;border-left-width:4px}}
.org{{display:grid;gap:10px;justify-items:center}}
.caixa{{background:var(--painel-2);border:1px solid var(--linha-forte);border-radius:8px;padding:8px 12px;min-width:0;max-width:100%;width:100%}}
.org > .caixa{{max-width:560px}}
.caixa .s{{font:700 var(--fs-1) var(--mono);color:var(--laranja);margin-right:8px}}
.caixa b{{font-weight:700}}
.caixa p{{margin:4px 0 0;font-size:var(--fs-1);color:var(--texto-2)}}
.caixa.dono{{border-color:var(--laranja)}}.caixa.int{{border-color:var(--info);border-width:2px}}
.caixa.nogo{{border-color:var(--vermelho);border-style:dashed}}.caixa.go{{border-color:var(--ok);border-width:2px}}
.seta{{color:var(--texto-3);font:700 var(--fs-3) var(--mono);line-height:1;text-align:center}}
.raias{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,200px),1fr));gap:10px;width:100%}}
.raia{{border:1px dashed var(--linha-forte);border-radius:8px;padding:8px;display:grid;gap:8px;align-content:start;min-width:0}}
.raia > span{{font:600 var(--fs-1) var(--sans);text-transform:uppercase;letter-spacing:.06em;color:var(--texto-3)}}
.passos{{counter-reset:p;display:grid;gap:8px;width:100%;max-width:640px}}
.losango{{border:2px solid var(--aviso);border-radius:8px;padding:10px 12px;text-align:center;width:100%;max-width:560px;font-weight:600}}
.dois{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,260px),1fr));gap:10px;width:100%}}
.amostras{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,150px),1fr));gap:6px}}
.amostra{{display:flex;align-items:center;gap:8px;font:400 var(--fs-1) var(--mono);min-width:0}}
.amostra .q{{display:inline-flex;width:34px;height:20px;border:1px solid var(--linha-forte);border-radius:3px;flex:none}}
.amostra .q i{{flex:1}}
.amostra span{{overflow-wrap:anywhere}}
ul.git{{list-style:none;margin:0;padding:0;display:grid;gap:6px;font-size:var(--fs-2)}}
ul.git li{{display:grid;grid-template-columns:auto auto minmax(0,1fr);gap:10px;max-width:none}}
ul.git code{{color:var(--laranja)}}
footer{{max-width:1180px;margin:56px auto 0;padding:20px;border-top:1px solid var(--linha);font-size:var(--fs-2);color:var(--texto-2)}}
@media (max-width:760px){{
  .capa{{grid-template-columns:minmax(0,1fr)}}.capa .abertura{{order:-1}}.capa .abertura img{{max-width:200px}}
  h1{{font-size:var(--fs-7)}}.lin{{grid-template-columns:minmax(0,1fr) auto}}.lin .trilho{{grid-column:1/-1;grid-row:2}}
  ul.git li{{grid-template-columns:auto minmax(0,1fr)}}ul.git li .dt{{display:none}}
  main,footer{{padding-left:14px;padding-right:14px}}.barra{{padding:8px 14px}}
}}
@media (prefers-reduced-motion: reduce){{*{{scroll-behavior:auto}}}}
@media print{{.barra,.indice,.botao{{display:none}}figure,tr,.ficha,.cartao{{break-inside:avoid}}}}
"""


# ---------------------------------------------------------------- pecas

def secao(n: int, ident: str, titulo: str, corpo: str) -> str:
    return (f'\n<section id="{ident}">\n  <div class="rotulo"><span class="num">{n:02d}</span>'
            f'<span class="traco"></span></div>\n  <h2>{e(titulo)}</h2>\n{corpo}\n</section>\n')


def figura(fid: str, corpo: str, legenda: str) -> str:
    return (f'<figure id="{fid}">{corpo}<figcaption><b class="fig-n">Figura ?.</b> '
            f'{legenda}</figcaption></figure>')


def ref(fid: str) -> str:
    return f'<a class="ref-fig" href="#{fid}">Figura ?</a>'


def tabela(cab: list[str], linhas: list[list[str]], num: set[int] = frozenset()) -> str:
    th = "".join(f'<th{" class=n" if i in num else ""}>{e(c)}</th>' for i, c in enumerate(cab))
    tr = "".join("<tr>" + "".join(f'<td{" class=n" if i in num else ""}>{c}</td>' for i, c in enumerate(l)) + "</tr>"
                 for l in linhas)
    return f'<div class="tab"><table><thead><tr>{th}</tr></thead><tbody>{tr}</tbody></table></div>'


def ficha(valor: str, rotulo: str, fonte: str, extra: str = "", cls: str = "") -> str:
    return (f'<div class="ficha {cls}"><div class="v">{valor}{f" <small>{extra}</small>" if extra else ""}</div>'
            f'<div class="r">{e(rotulo)}</div><div class="f">{e(fonte)}</div></div>')


def barra_tres(ag: int, pa: int, na: int, total: int) -> str:
    w = lambda x: f"{100 * x / total:.3f}%" if total else "0%"
    return (f'<div class="trilho" role="img" aria-label="{ag} no agente, {pa} pela metade, {na} não, de {total}">'
            f'<i class="s-ag" style="width:{w(ag)}"></i><i class="s-pa" style="width:{w(pa)}"></i>'
            f'<i class="s-na" style="width:{w(na)}"></i></div>')


def barra_um(n: int, maximo: int) -> str:
    return (f'<div class="trilho" role="img" aria-label="{n} de {maximo}"><i class="s-um" '
            f'style="width:{100 * n / maximo:.3f}%"></i></div>')


PILULA = {"passed": ("passou", "ok"), "failed": ("falhou", "ruim"), "blocked": ("bloqueado", "trava"),
          "CONCLUÍDA": ("concluída", "ok"), "EM EXECUÇÃO": ("em execução", "vivo"),
          "PLANEJADA": ("planejada", "neutro"), "BLOQUEADA": ("bloqueada (dono)", "trava"),
          "feito": ("feito", "ok"), "aberto": ("aberto", "neutro"), "bloqueado": ("bloqueado", "trava"),
          "depois": ("depois da versão", "neutro")}


def pilula(chave: str) -> str:
    rot, cls = PILULA[chave]
    return f'<span class="pl {cls}">{e(rot)}</span>'


# ---------------------------------------------------------------- montagem

# Raia de cada papel no organograma de construcao. A LISTA de papeis sai do README dos
# agentes; esta tabela so diz em que raia cada um mora. Papel sem raia, ou raia citando papel
# que nao existe, PARA -- e a chave morta, pelos dois lados.
RAIAS = [("constrói", ["B", "E"]), ("reprova", ["SEC", "F", "G", "C"]),
         ("pesquisa", ["J", "RES-subagentes", "RES-rede"]), ("registra", ["H", "I", "D"])]
CADEIA = ["A", "INT"]


def conferir_raias(papeis: dict) -> None:
    postos = CADEIA + [s for _r, ss in RAIAS for s in ss]
    sem = sorted(set(papeis) - set(postos))
    mortos = sorted(set(postos) - set(papeis))
    if sem or mortos:
        raise SystemExit(f"PARADA: organograma diverge do .claude/agents/README.md -- sem raia: {sem}; "
                         f"raia citando papel inexistente: {mortos}")


def montar(suite_arq: Path | None) -> tuple[str, list[str], dict]:
    fez_menos: list[str] = []
    v = N.versao()
    cod = N.codigo()
    g = N.git()
    cg = N.cognicoes()
    ab = N.absorcao()
    sp = N.sprints()
    ac = N.achados(sp)
    fe = N.ferramentas()
    eq = N.equipe()
    pc = N.papeis_de_construcao()
    ro = N.roteiro_do_integrador()
    pu = N.portao_unico()
    bw = N.fora_do_bwrap()
    bk = N.broker()
    gl = N.gitleaks()
    tk = N.tokens_da_marca()
    ql = N.qualificacao()
    fi = N.fidelidade()
    ce = N.certificacao()
    me = N.medidores()
    su = N.suite(suite_arq)
    fs = N.fechamento_sec(suite_arq)
    pg = N.petreas_gerais()
    ps = N.portoes_de_sprint(sp)
    conferir_raias(pc["papeis"])

    if fe["declarado"] is not None and fe["declarado"] != fe["total"]:
        raise SystemExit(f"PARADA: ferramentas.json diz total {fe['declarado']} e lista {fe['total']}")
    if eq["declarado"] is not None and eq["declarado"] != eq["total"]:
        raise SystemExit(f"PARADA: equipe.json diz total {eq['declarado']} e lista {eq['total']}")
    for f in ab["fontes"]:
        if f["sem_nome"]:
            fez_menos.append(f"fonte «{f['chave']}» sem nome de exibicao em NOMES_PRODUTO do app.js (sai a chave)")

    # ---- datas: o retrato e a medicao mais nova entre as fontes, nunca o relogio da corrida
    datas = [x["data"] for x in (ab, sp, fe, eq, pc, ro, pu, bw, tk) if x] + [g["cabeca_data"]]
    if ql["atual"]:
        datas.append(ql["atual"]["data"])
    datas += [x["data"] for x in fi]
    retrato = max(d["quando"] for d in datas)

    falta_sp = sp["total"] - sp["contagem"]["CONCLUÍDA"]
    pct_falta = 100 * falta_sp / sp["total"]
    qa = ql["atual"]

    # ================================================================ capa
    temas_txt = " · ".join(f"{t} {x['qualificadas']}/{x['telas']}" for t, x in qa["por_tema"].items()) if qa else ""
    fichas = [
        ficha(pct(ab["pct_agente"]), "absorção no agente", f"{ab['fonte']}", f"{pct(ab['pct_bibliotecas'])} c/ bibliotecas"),
        ficha(str(len(ab["fontes"])), "fontes medidas", ab["fonte"], f"{ab['itens']} capacidades"),
        ficha(f"{sp['contagem']['CONCLUÍDA']}/{sp['total']}", "sprints concluídas", sp["fonte"]),
        ficha(pct(pct_falta), "falta (sprints)", sp["fonte"], f"{falta_sp} de {sp['total']}", "falta"),
        ficha(str(fe["total"]), "ferramentas montadas", fe["fonte"], f"{fe['concedidas']} concedidas"),
        ficha(str(eq["total"]), "papéis na equipe", eq["fonte"], f"{len(eq['areas'])} macroáreas"),
        (ficha(f"{min(x['qualificadas'] for x in qa['por_tema'].values())}/{max(x['telas'] for x in qa['por_tema'].values())}",
               "telas qualificadas", qa["fonte"], f"{len(qa['por_tema'])} temas") if qa else
         ficha('<span class="nm">NÃO MEDIDO</span>', "telas qualificadas", ql["comando"])),
        (ficha(f"{ce['obrig_ok']}/{ce['obrig']}", "certificação", ce["fonte"], "obrigatórios") if ce else
         ficha('<span class="nm">NÃO MEDIDO</span>', "certificação", "python3 tools/release_certification.py")),
        ficha(N.br(cod["linhas"]), "linhas de Rust", "crates/ e apps/", f"{cod['arquivos']} arquivos"),
        ficha(str(v["membros"]), "membros do workspace", v["fonte"]),
        ficha(str(g["commits"]), "commits em phxclaw/", "git rev-list"),
        ficha(str(cg["total"]), "cognições", cg["fonte"],
              f"{cg['estados'].get('FRUTÍFERO', 0)} frutíferas"),
    ]
    def img(nome: str, cls: str) -> str:
        w, h = dimensoes(ASSETS / nome)
        return (f'<img class="{cls}" src="{data_uri(ASSETS / nome)}" alt="Fênix Phoenix, marca do PhxClaw" '
                f'width="{w}" height="{h}">')
    abert = img("abertura-escuro.png", "so-escuro") + img("abertura-claro.png", "so-claro")
    capa = f"""<header class="capa">
  <div>
    <div class="selo">Dossiê técnico · versão {e(v['versao'])} · retrato de {N.fmt_data(retrato, False)}</div>
    <h1>Phx<span class="x">Claw</span></h1>
    <p class="chamada">O agente autônomo local da Phoenix: recebe um objetivo, planeja, usa ferramentas
    num sandbox, registra cada passo em evidência encadeada e entrega arquivos. Este dossiê é o
    retrato do ramo <code>{e(g['ramo'])}</code> no commit <code>{e(g['cabeca'])}</code>, mais a árvore de
    trabalho: <b>{g['sujos']}</b> caminhos mudados e ainda não comitados ({g['sujos_novos']} novos) entram
    nos números, porque é deles que a interface e os geradores leem.</p>
  </div>
  <div class="abertura">{abert}</div>
</header>
<div class="painel">{''.join(fichas)}</div>
<p class="nota">Falta = sprints não concluídas ÷ sprints da tabela «Visão geral» do <code>{e(sp['fonte'])}</code>
= {falta_sp}/{sp['total']}. Absorção no agente = capacidades entregues pelo agente ÷ capacidades listadas,
somadas as {len(ab['fontes'])} fontes ({ab['no_agente']}/{ab['itens']}); «com bibliotecas» soma meio ponto ao que
está pela metade, pelo peso do <code>docs/absorcao/gerar_absorcao.py</code>.</p>"""

    secoes: list[tuple[str, str, str]] = []

    # ================================================================ 1 o que e
    secoes.append(("s-o-que-e", "O que é o PhxClaw", f"""
<p>Um agente autônomo que roda <b>na máquina do dono</b>, escrito em Rust: {v['membros']} membros no
workspace e {N.br(cod['linhas'])} linhas de Rust em <code>crates/</code> e <code>apps/</code>. Recebe um
objetivo pela CLI, pela API, pela interface web (PWA), pelos canais de mensagem, pelo MCP ou pelo
ACP — e todas essas portas montam o <b>mesmo</b> agente, pela mesma <code>Montagem</code>.</p>
<p>O que o separa de um assistente comum é o caminho de cada ferramenta: nada roda sem a capacidade
concedida, toda chamada passa por um <b>portão único</b> ({ref('fig-tarefa')}), o que cria processo
roda num <code>bubblewrap</code> sem rede, e segredo só chega por concessão curta do
<code>SecretBroker</code>. Cada passo vira evidência com hash encadeado, e o <code>phxclaw gonogo</code>
lê essa evidência para dar Go, NoGo ou Aguardar.</p>
<p>O trabalho de agora é <b>absorção</b>: medir, capacidade por capacidade, o que {len(ab['fontes'])} produtos
de referência fazem ({', '.join(e(f['nome']) for f in ab['fontes'])}) e trazer para dentro do agente o
que falta, em sprints com Go/NoGo do conselho de integradores.</p>"""))

    # ================================================================ 2 absorcao
    linhas_ab = "".join(
        f'<div class="lin"><span class="nome">{e(f["nome"])}</span>{barra_tres(f["no_agente"], f["parcial"], f["nao"], f["total"])}'
        f'<span class="val">{pct(f["pct_agente"])} · {f["no_agente"]}/{f["total"]}</span></div>' for f in ab["fontes"])
    leg = ('<div class="legenda"><span><i class="s-ag"></i>no agente</span><span><i class="s-pa"></i>pela metade '
           '(hachura)</span><span><i class="s-na"></i>não (só contorno)</span></div>')
    tab_ab = tabela(["fonte", "itens", "no agente", "metade", "não", "% agente", "% c/ bibl.", "lido em", "falta"],
                    [[e(f["nome"]), str(f["total"]), str(f["no_agente"]), str(f["parcial"]), str(f["nao"]),
                      pct(f["pct_agente"]), pct(f["pct_com_bibliotecas"]), e(f["lido_em"]),
                      f'<span class="mot">{e(", ".join(f["falta"]) or "—")}</span>'] for f in ab["fontes"]],
                    num={1, 2, 3, 4, 5, 6})
    secoes.append(("s-absorcao", "Absorção por fonte", f"""
<p>Cada fonte tem uma lista <b>fechada</b> de capacidades, tirada da documentação oficial com a data da
leitura (<code>docs/absorcao/fontes.json</code>), e cada capacidade tem um estado no PhxClaw com a
evidência no código (<code>phxclaw.json</code>). O <code>gerar_absorcao.py</code> cruza as duas e grava
<code>{e(ab['fonte'])}</code>, de onde esta seção e a tela de Absorção leem. A lista de fontes não é fixa:
fonte nova no arquivo entra aqui sozinha.</p>
{figura('fig-absorcao', f'<div class="barras">{linhas_ab}</div>{leg}',
        f'Capacidades de cada fonte por estado no PhxClaw. A forma separa os estados, não só a cor. '
        f'Total: {ab["no_agente"]} no agente, {ab["parcial"]} pela metade e {ab["nao"]} não, de {ab["itens"]} — '
        f'{pct(ab["pct_agente"])} no agente. Arquivo de {quando(ab["data"])}.')}
{tab_ab}
<p class="nota">Uma capacidade presente em várias fontes conta uma vez em cada uma: o total pondera cada
fonte pelo tamanho da lista dela.</p>"""))

    # ================================================================ 3 sprints
    colunas = []
    for est in N.ORDEM_ESTADOS:
        its = [i for i in sp["itens"] if i["estado"] == est]
        cls = "c-" + est.split()[0].replace("Í", "I")
        def cartao(i: dict) -> str:
            onda = f" · onda {e(i['onda'])}" if i["onda"] not in ("—", "") else ""
            det = f'<div class="mot">{md(i["detalhe"])}</div>' if i["detalhe"] and i["detalhe"] != f"onda {i['onda']}" else ""
            return f'<div class="cartao {cls}"><b>{e(i["id"])}</b>{onda}<div>{md(i["foco"])}</div>{det}</div>'
        cartoes = "".join(cartao(i) for i in its)
        colunas.append(f'<div class="coluna"><h4><span>{pilula(est)}</span><span>{len(its)}</span></h4>{cartoes}</div>')
    secoes.append(("s-sprints", "Sprints e kanban", f"""
<p>As sprints saem da tabela «Visão geral» do <code>{e(sp['fonte'])}</code> ({quando(sp['data'])}): numeração
global, sem reuso. Estado lido da última coluna; estado que o gerador não conhece <b>para</b> a corrida em
vez de cair numa coluna errada.</p>
{figura('fig-kanban', f'<div class="kanban">{"".join(colunas)}</div>',
        f'O quadro das {sp["total"]} sprints: ' + ", ".join(f'{sp["contagem"][x]} {PILULA[x][0]}' for x in N.ORDEM_ESTADOS) +
        '. A borda do cartão também muda de forma por estado (cheia, tracejada, dupla).')}
<p class="nota">Portões de toda sprint, lidos do mesmo arquivo: {'; '.join(md(p) for p in ps)}.</p>"""))

    es = N.esteira()
    lin_es = [[f'<code class="id">{e(i["id"])}</code>', md(i["texto"]), pilula(i["estado"]), f'<span class="mot">{md(i["nota"])}</span>']
              for i in es["itens"]]
    secoes[-1] = (secoes[-1][0], secoes[-1][1], secoes[-1][2] + f"""
<h3>Esteira de produção</h3>
<p>Os itens de produto (E*, U*) de <code>{e(es['fonte'])}</code> ({quando(es['data'])}), com o estado lido da
última coluna; ⏸ é «depois da versão», visível e fora da conta.</p>
{tabela(["#", "item", "estado", "nota"], lin_es)}""")

    # ================================================================ 4 organograma
    P = pc["papeis"]

    def caixa(s: str, cls: str = "") -> str:
        p = P[s]
        ag = ", ".join(p["agentes"])
        return (f'<div class="caixa {cls}"><span class="s">{e(s)}</span><b>{e(p["nome"])}</b>'
                f'<p>{md(p["porque"])}</p><p><code>{e(ag)}</code></p></div>')

    raias = "".join(f'<div class="raia"><span>{e(r)}</span>{"".join(caixa(s) for s in ss)}</div>' for r, ss in RAIAS)
    org1 = (f'<div class="org"><div class="caixa dono"><span class="s">DONO</span><b>{e(eq["humanos"][0] if eq["humanos"] else "Dono")}</b>'
            f'<p>Decide produto, prazo e credencial; é o impasse — sobe a ele só choque com pétrea, empate real ou produto.</p></div>'
            f'<div class="seta">↓</div>{caixa("A")}<div class="seta">↓</div>{caixa("INT", "int")}'
            f'<div class="seta">↓</div><div class="raias">{raias}</div></div>')
    mx = max(a["n"] for a in eq["areas"])
    org2 = "".join(f'<div class="lin"><span class="nome">{e(a["nome"])}</span>{barra_um(a["n"], mx)}'
                   f'<span class="val">{a["n"]}</span></div>' for a in eq["areas"])
    integ = eq["integrador"]
    secoes.append(("s-organograma", "Organograma", f"""
<h3>Quem constrói o PhxClaw</h3>
<p>Os papéis da construção saem das duas tabelas de <code>{e(pc['fonte'])}</code> ({quando(pc['data'])}): os
papéis de base e os acréscimos (SEC, os subagentes do pesquisador e o Integrador). O gerador só decide a
<b>raia</b> de cada um; papel sem raia, ou raia citando papel que não existe, para a corrida.</p>
{figura('fig-org-construcao', org1,
        f'{len(P)} papéis e subagentes de construção. O conselho de integradores dá o Go/NoGo; o versionador (I) só comita depois do Go; '
        'os revisores reprovam antes do commit.')}
<h3>A equipe dentro do agente</h3>
<p>Os papéis da planilha <code>{e(eq['planilha'])}</code> vivem no próprio PhxClaw: o agente os lista com
<code>team_list</code> e delega com <code>team_delegate</code>, e o delegado recebe só a interseção das
capacidades dele com as de quem delegou. Gerado por <code>{e(eq['gerado_por'])}</code> em
<code>{e(eq['fonte'])}</code> ({quando(eq['data'])}).</p>
{figura('fig-org-equipe', f'<div class="barras">{org2}</div>',
        f'{eq["total"]} papéis em {len(eq["areas"])} macroáreas. Papel humano: {e(", ".join(eq["humanos"]) or "nenhum")}. '
        f'O último da planilha, {e(integ["nome"]) if integ else "—"}, é o dono do Go/NoGo dentro do agente.')}"""))

    # ================================================================ 5 fluxos
    passos_int = "".join(f'<div class="caixa int"><span class="s">{i}</span><b>{e(p)}</b></div>'
                         for i, p in enumerate(ro["passos"], 1))
    conselho = "".join(f'<div class="caixa {"go" if c["entao"].startswith("Go") else "nogo" if c["entao"].startswith("NoGo") else ""}">'
                       f'<b>{e(c["se"])}</b><p>{e(c["entao"])}</p></div>' for c in ro["conselho"])
    onda = (f'<div class="org"><div class="caixa dono"><span class="s">DONO</span><b>Pedido</b><p>Um pedido de produto, ou a sprint seguinte do backlog.</p></div>'
            f'<div class="seta">↓</div><div class="caixa"><span class="s">J</span><b>Pesquisa decide; o dono é o impasse</b>'
            f'<p>Hipóteses escritas antes de medir, lidas no fonte e na documentação das fontes; a que venceu e a que morreu vão para o arquivo.</p></div>'
            f'<div class="seta">↓</div><div class="dois"><div class="caixa"><span class="s">B</span><b>Frentes de engenharia</b><p>Uma frente cada; mutante só em cópia; não comitam.</p></div>'
            f'<div class="caixa"><span class="s">E</span><b>Designer</b><p>Tela provada no Chromium nos dois temas.</p></div></div>'
            f'<div class="seta">↓</div><div class="caixa"><span class="s">SEC · F · G · C</span><b>Revisão</b>'
            f'<p>Cada achado volta à frente dona com cenário e prova; o conserto traz o teste que cai com o defeito reposto.</p></div>'
            f'<div class="seta">↓</div><div class="passos">{passos_int}</div>'
            f'<div class="seta">↓</div><div class="losango">Portões verdes e nenhum achado alto aberto?</div>'
            f'<div class="seta">↓</div><div class="dois">{conselho}</div>'
            f'<div class="seta">↓ Go</div><div class="caixa go"><span class="s">I</span><b>Commit e push</b>'
            f'<p>Mensagem com a decisão e o motivo; push conferido por <code>git ls-remote</code>. Depois: cognição, dossiê e base de conhecimento.</p></div></div>')
    passos_pu = "".join(f'<div class="caixa"><span class="s">{i}</span><b>{e(p["nome"])}</b><p>{e(p["efeito"])}</p></div>'
                        for i, p in enumerate(pu["passos"], 1))
    tarefa = (f'<div class="org"><div class="caixa"><b>Entradas</b><p>CLI, API, PWA, canais, MCP e ACP chamam a mesma <code>Montagem</code>.</p></div>'
              f'<div class="seta">↓</div><div class="caixa"><b>Laço do motor</b><p>O modelo pede ferramentas; modo plano só lê; pergunta pausa a tarefa até a resposta.</p></div>'
              f'<div class="seta">↓ <code>call_tool_com</code></div><div class="passos">{passos_pu}</div>'
              f'<div class="seta">↓</div><div class="caixa go"><b>Ferramenta no bubblewrap</b><p>Sem rede, presa em <code>/work</code>; '
              f'{len(bw["excecoes"])} exceções fora do sandbox, cada uma declarada em teste com o motivo ({ref("fig-bwrap")}).</p></div>'
              f'<div class="seta">↓</div><div class="caixa"><b>Evidência</b><p>Cada passo gravado na tarefa com hash encadeado; o <code>gonogo</code> lê daí.</p></div></div>')
    secoes.append(("s-fluxos", "Fluxogramas", f"""
<h3>Uma onda, do pedido ao commit</h3>
<p>Os passos do Integrador e a regra do conselho saem de <code>{e(ro['fonte'])}</code> ({quando(ro['data'])}),
não deste gerador: mudou o roteiro lá, muda o desenho aqui.</p>
{figura('fig-onda', onda,
        f'O caminho de uma onda: {len(ro["passos"])} passos do roteiro do Integrador e as {len(ro["conselho"])} regras do conselho. '
        'Um NoGo faz todos aguardarem; parecer faltando é Aguardar; Go só unânime.')}
<h3>Uma tarefa, pelo portão único</h3>
<p>A ordem dos passos sai do <b>código</b>: o gerador procura cada chamada dentro do
<code>call_tool_com</code> (<code>{e(pu['fonte'])}</code>, linha {pu['linha']}, {quando(pu['data'])}) e
desenha na ordem em que aparecem. Módulo chamado ali sem rótulo aqui, ou rótulo sem chamada lá, para a
corrida — o desenho não pode prometer portão que não existe, nem esconder um que existe.</p>
{figura('fig-tarefa', tarefa,
        f'Os {len(pu["passos"])} passos que toda chamada de ferramenta atravessa antes de rodar. Qualquer um recusa, e a recusa volta ao modelo com o motivo. '
        'A varredura de segredos do commit mora dentro do <code>git_write</code> (gitleaks), e o passo do portão impede o shell de gravar no git por fora dela.')}"""))

    # ================================================================ 6 ferramentas
    mxg = max(len(fs) for _g, fs in fe["grupos"])
    lin_g = "".join(f'<div class="lin"><span class="nome"><code>{e(gn)}</code></span>{barra_um(len(fs), mxg)}'
                    f'<span class="val">{len(fs)} · {sum(1 for f in fs if f["concedida"])} concedidas</span></div>'
                    for gn, fs in fe["grupos"])
    tab_neg = tabela(["ferramenta", "capacidade", "grupo"],
                     [[f'<code>{e(f["nome"])}</code>', f'<code>{e(f["capacidade"])}</code>', e(f["grupo"])] for f in fe["negadas"]])
    secoes.append(("s-ferramentas", "Ferramentas e capacidades", f"""
<p>A lista é a da <code>Montagem</code> desta máquina, gravada por <code>{e(fe['gerado_por'])}</code> em
<code>{e(fe['fonte'])}</code> ({quando(fe['data'])}, versão {e(fe['versao'])}). Ela varia por máquina:
{e(fe['observacao'])}.</p>
{figura('fig-ferramentas', f'<div class="barras">{lin_g}</div>',
        f'{fe["total"]} ferramentas em {len(fe["grupos"])} grupos e {fe["capacidades"]} capacidades; {fe["concedidas"]} concedidas por padrão.')}
<h3>Montadas e não concedidas por padrão</h3>
<p>Existem, mas o agente só as usa com a capacidade concedida pelo operador — {len(fe['negadas'])} ao todo.</p>
{tab_neg}"""))

    # ================================================================ 7 interface
    cores = [k for k in tk["claro"] if k in tk["escuro"] and tk["claro"][k].strip().startswith("#")]
    amostras = "".join(f'<div class="amostra"><span class="q"><i style="background:{e(tk["escuro"][k].strip())}"></i>'
                       f'<i style="background:{e(tk["claro"][k].strip())}"></i></span><span>{e(k)}</span></div>' for k in cores)
    if qa:
        tab_q = tabela(["tema", "telas", "qualificadas", "com ressalvas", "não"],
                       [[e(t), str(x["telas"]), str(x["contagem"].get("QUALIFICADA", 0)),
                         str(x["contagem"].get("QUALIFICADA COM RESSALVAS", 0)), str(x["contagem"].get("NÃO QUALIFICADA", 0))]
                        for t, x in qa["por_tema"].items()], num={1, 2, 3, 4})
        q_txt = (f'<p>Veredito atual: <b>{e(temas_txt)}</b>, com {qa["sondas_ok"]} de {qa["sondas"]} sondas aprovadas, '
                 f'lido de <code>{e(qa["fonte"])}</code> ({quando(qa["data"])}). O arquivo de resultado fica fora do git '
                 f'(<code>tests/desktop/out/</code>): em outra máquina esta linha sai NÃO MEDIDO até alguém rodar '
                 f'<code>{e(ql["comando"])}</code>.</p>{tab_q}')
    else:
        q_txt = f'<p>{nao_medido("veredito por tela", ql["comando"])}</p>'
        fez_menos.append(f"qualificacao da UI sem qualificar.json -- rode: {ql['comando']}")
    pa = ql["partida"]
    p_txt = ""
    if pa:
        p_txt = (f'<p>De onde partiu: o relatório de {quando(pa["data"])} (<code>{e(pa["fonte"])}</code>) achou '
                 + ", ".join(f'{n} {e(k.lower())}' for k, n in pa["severidade"].items())
                 + f' e deu, nas {pa["telas"]} telas, '
                 + ", ".join(f'{n} {e(k.lower())}' for k, n in pa["contagem"].items()) + '.</p>')
    tab_fi = tabela(["arquivo", "medido em", "comando", "resultado"],
                    [[f'<code>{e(x["arquivo"])}</code>', quando(x["data"]), f'<code>{e(x["comando"])}</code>',
                      "<br>".join(e(r) for r in x["resumo"])] for x in fi])
    secoes.append(("s-interface", "Interface", f"""
<p>A interface segue o <b>Style Phoenix Padrão</b> (<code>docs/ui/STYLE_PHOENIX_PADRAO.md</code>): os mesmos
nomes e valores de token do console do PhxSql, Exo 2 e IBM Plex Mono <b>locais</b>, dois temas desenhados
(escuro da marca e claro em papel quente, com o laranja escurecido por contraste) e as cores de ação só onde
há ação. Esta página usa os mesmos tokens, lidos dos blocos <code>:root</code> do
<code>{e(tk['fonte'])}</code> ({quando(tk['data'])}) — {len(tk['escuro'])} no escuro, {len(tk['claro'])}
redefinidos no claro.</p>
{figura('fig-tokens', f'<div class="amostras">{amostras}</div>',
        f'Os {len(cores)} tokens de cor dos dois temas: metade esquerda do quadrado no escuro, direita no claro.')}
<h3>Qualificação, tela por tela</h3>
{p_txt}
{q_txt}
<h3>Responsiva e fidelidade da conversão de tela</h3>
<p>Cada medição de <code>docs/ui/fidelidade/</code>, com a data gravada no próprio resultado.</p>
{tab_fi}"""))

    # ================================================================ 8 seguranca
    tab_bw = tabela(["arquivo", "chamada", "por que fora do bwrap"],
                    [[f'<code>{e(x["arquivo"])}</code>', f'<code>{e(x["trecho"])}</code>', e(x["motivo"])] for x in bw["excecoes"]])
    sec_lista = "".join(f"<li>{md(x)}</li>" for x in ac["sec"])
    seg_data = N.data_de(RAIZ / "crates/phxclaw-agent/src/segredos.rs")
    secoes.append(("s-seguranca", "Segurança", f"""
<h3>SecretBroker</h3>
<p>Segredo não mora no ambiente da ferramenta: chega por concessão curta. As decisões, como o fonte as
escreve (<code>{e(bk['fonte'])}</code>, {quando(bk['data'])}):</p>
<ul>{''.join(f"<li>{md(x)}</li>" for x in bk['notas'])}</ul>
<h3>Bubblewrap</h3>
<p>Todo processo que uma ferramenta cria roda no <code>bwrap</code>, sem rede e preso em <code>/work</code>. A
exceção é decisão escrita: a guarda <code>todo_processo_desta_crate_passa_pelo_bwrap_ou_esta_declarado</code>
varre o fonte e reprova processo fora da lista, e exceção que não existe mais no fonte. A lista é a do teste
(<code>{e(bw['fonte'])}</code>, {quando(bw['data'])}):</p>
{figura('fig-bwrap', tab_bw, f'As {len(bw["excecoes"])} exceções declaradas ao sandbox, cada uma com o motivo.')}
<h3>Gitleaks no commit</h3>
<p>A varredura de segredos roda dentro do <code>git_write</code>, com o binário oficial do gitleaks travado
pelo SHA-256 que o operador declara. As decisões do módulo (<code>crates/phxclaw-agent/src/segredos.rs</code>,
{quando(seg_data)}):</p>
<ul>{''.join(f"<li>{md(x)}</li>" for x in gl['decisoes'])}</ul>
<h3>Achados da revisão adversária de 01/10</h3>
<p>Do <code>{e(sp['fonte'])}</code>: {md(ac['altos_frase'])}</p>
<p>Os que voltaram à frente dona: <b>{e(', '.join(ac['altos']))}</b>. O fechamento de cada um é lido da saída
guardada da suíte (<code>--suite</code>), pelo teste que o prova — nunca por texto fixo:</p>
{tabela(['achado', 'teste que prova', 'na suíte', 'estado'],
        [[f'<code>{e(c)}</code>', '<br>'.join(f'<code>{e(l["teste"])}</code>' for l in fs[c]['testes']),
          '<br>'.join(e(l['estado']) for l in fs[c]['testes']),
          pilula('feito') if fs[c]['fechado'] else nao_medido('fechamento', 'tools/suite.sh -p phxclaw-agent -- --test segredos --test segredos_shell --test gonogo')]
         for c in ac['altos'] if c in fs])}
<p>Os {len(ac['sec'])} restantes, na conta da SP000013:</p>
<ul>{sec_lista}</ul>"""))
    abertos = [c for c in ac["altos"] if c in fs and not fs[c]["fechado"]]
    if abertos:
        fez_menos.append("achados SEC " + ", ".join(abertos) + ": fechamento NAO MEDIDO (teste ausente ou nao ok na suite)")

    # ================================================================ 9 testes
    if ce:
        lin_ce = []
        for gt in ce["portoes"]:
            med = gt.get("measured")
            if isinstance(med, dict):
                med = ", ".join(f"{k} {v}" for k, v in med.items())
            lin_ce.append([f'<code>{e(gt["gate"])}</code>', "sim" if gt["required"] else "não", pilula(gt["status"]),
                           f'<span class="mot">{e(med or gt.get("reason") or "")}</span>'])
        ce_txt = (f'<p>Certificação <b>{e(ce["veredito"])}</b> da versão {e(ce["versao"])}, alvo {e(ce["alvo"])}: '
                  f'{ce["obrig_ok"]} de {ce["obrig"]} portões obrigatórios, medida em {quando(ce["data"])} '
                  f'(<code>{e(ce["fonte"])}</code>). A árvore andou <b>{ce["commits_depois"]}</b> commits em '
                  f'<code>phxclaw/</code> depois dela: o placar é daquele dia.</p>'
                  + tabela(["portão", "obrigatório", "estado", "medido / motivo"], lin_ce))
    else:
        ce_txt = f'<p>{nao_medido("certificação", "python3 tools/release_certification.py")}</p>'
        fez_menos.append("certificacao sem reports/RELEASE_CERTIFICATION_v*.json")
    if su:
        escopo = ("Suíte inteira" if su["inteira"] else
                  f'Suíte PARCIAL ({su["crates"]} de {su["membros"]} membros do workspace com unitários rodados)')
        # «passam» e a palavra do libtest, e e tudo o que a saida guardada prova: teste que pula
        # tambem sai «ok». Os pulos registrados se dizem; os lugares que ainda pulam calados,
        # tambem -- «provados» so quando nao sobrar nenhum.
        su_txt = (f'<p>{escopo}, de <code>{e(Path(su["fonte"]).name)}</code> ({quando(su["data"])}): '
                  f'<b>{su["passam"]}</b> passam, <b>{su["falham"]}</b> falham, {su["ignorados"]} ignorados. '
                  + (f'Dos que passam, <b>{su["pulados"]}</b> pularam com registro nesta mesma corrida' if su["pulados"] is not None
                     else 'Os pulos registrados desta corrida não foram anexados ao arquivo: '
                          + nao_medido("registro de pulos", "rm -f target/tmp/pulados.jsonl; cargo test --workspace --no-fail-fast 2>&1 | tee ARQ; "
                                       f"{{ echo '{N.MARCA_PULADOS}'; cat target/tmp/pulados.jsonl; }} >> ARQ"))
                  + (', e a guarda de pulo calado não rodou nesta saída: '
                     + nao_medido("guarda de pulo calado", "cargo test -p phxclaw-test-support")
                     + '; o número de provados não se afirma.</p>'
                     if su["calados"] is None else
                     f', e há <b>{len(su["calados"])}</b> lugares no código que ainda pulam sem registrar (só imprimem '
                     f'«pulado»; migração na SP000013), então o número de provados não se afirma.</p>'
                     if su["calados"] else
                     # Zero calados (guarda do phxclaw-test-support em 0) e o bloco anexado: o
                     # pulo e todo visivel, e provado = passou sem pular.
                     (f', e nenhum lugar no código pula sem registrar (guarda em 0): '
                      f'<b>{su["passam"] - su["pulados"]}</b> provados.</p>' if su["pulados"] is not None
                      else ', e nenhum lugar no código pula sem registrar (guarda em 0); sem o bloco do '
                           'registro, o número de provados não se afirma.</p>')))
        if su["pulados"] is None:
            fez_menos.append("pulos registrados: NAO MEDIDOS (o arquivo da suite nao traz o bloco do registro)")
        if su["calados"] is None:
            fez_menos.append("guarda de pulo calado: NAO MEDIDA (o teste dela nao esta na saida da suite)")
        elif su["calados"]:
            fez_menos.append(f'{len(su["calados"])} lugares pulam sem registro: provados NAO AFIRMADOS (SP000013)')
        if not su["inteira"]:
            fez_menos.append(f'suite PARCIAL: {su["crates"]} de {su["membros"]} membros')
    else:
        su_txt = (f'<p>Suíte inteira na árvore de agora: '
                  f'{nao_medido("este gerador não compila (falta disco, e a suíte não é dele)", "cargo test --workspace --no-fail-fast 2>&1 | tee /tmp/suite.txt; python3 tools/dossie/gerar_dossie.py --suite /tmp/suite.txt")}. '
                  f'O placar acima é o da certificação.</p>')
        fez_menos.append("suite cargo test da arvore atual: NAO MEDIDA (use --suite ARQ)")
    lin_me = []
    for m in me:
        if m["resultado"]:
            r = f'<code>{e(m["resultado"]["arquivo"])}</code> · {quando(m["resultado"]["data"])}'
        else:
            r = nao_medido("não grava arquivo de resultado" if not m["grava"] else "sem arquivo de resultado nesta máquina", m["comando"])
            fez_menos.append(f"{m['roteiro']}: NAO MEDIDO -- {m['comando']}")
        lin_me.append([f'<code>{e(m["roteiro"])}</code>', r])
    secoes.append(("s-testes", "Testes e bancadas", f"""
<p>Só entra número com arquivo de resultado; o que não tem aparece como <span class="nm">NÃO MEDIDO</span>,
com o comando para rodar, e nunca some da tabela.</p>
{ce_txt}
{su_txt}
<h3>Roteiros da interface</h3>
<p>A lista sai de <code>tests/desktop/</code>: todo roteiro com linha «Uso:» entra, e o arquivo que ele grava é
lido do próprio <code>writeFileSync</code>. Roteiro que só imprime o placar não deixa número para conferir.</p>
{tabela(["roteiro", "resultado"], lin_me)}"""))

    # ================================================================ 10 falta
    abertas = [i for i in sp["itens"] if i["estado"] != "CONCLUÍDA"]
    bloq_ce = [gt for gt in (ce["portoes"] if ce else []) if gt["status"] == "blocked"]
    secoes.append(("s-falta", "O que falta, e o que está com o dono", f"""
<p>Falta <b>{pct(pct_falta)}</b> das sprints ({falta_sp} de {sp['total']}). Por estado:
{', '.join(f"{sp['contagem'][x]} {PILULA[x][0]}" for x in N.ORDEM_ESTADOS if x != 'CONCLUÍDA')}.</p>
{tabela(['sprint', 'estado', 'foco'], [[f'<code class="id">{e(i["id"])}</code>', pilula(i["estado"]), md(i["foco"])] for i in abertas])}
<h3>Bloqueado com o dono</h3>
<p>O que a SP000014 precisa dele para trocar «contra falso» por «real» ({len(ac['sp14'])} itens):</p>
{tabela(['precisa', 'para'], [[md(x['precisa']), md(x['para'])] for x in ac['sp14']])}
<p>E os portões da certificação que esperam o mundo físico ou uma credencial ({len(bloq_ce)}):</p>
<ul>{''.join(f"<li><code>{e(gt['gate'])}</code> — {e(gt.get('reason') or '')}</li>" for gt in bloq_ce)}</ul>
<h3>Na conta do endurecimento (SP000013)</h3>
<ul>
<li>{len(ac['sp13'])} achados de revisão que nasceram «depois da versão»;</li>
<li>{len(ac['dba'])} defeitos ativos de formato em disco, do parecer do DBA;</li>
<li>{len(ac['qa'])} pétreas sem guarda provada, do inventário do QA;</li>
<li>{len(ac['prova_f'])} itens abertos da prova F;</li>
<li>{len(ac['sec'])} achados médios e baixos de segurança (<a href="#s-seguranca">Segurança</a>).</li>
</ul>
<h3>O que cada fonte ainda tem e o PhxClaw não</h3>
{tabela(['fonte', 'não', 'pela metade'], [[e(f['nome']), f'<span class="mot">{e(", ".join(f["falta"]) or "—")}</span>', f'<span class="mot">{e(", ".join(f["pela_metade"]) or "—")}</span>'] for f in ab['fontes']])}"""))

    # ================================================================ 11 petreas
    secoes.append(("s-petreas", "Pétreas", f"""
<p>As cláusulas pétreas que valem para <b>todo</b> projeto da casa, pelos títulos de
<code>{e(pg['fonte'])}</code> ({quando(pg['data'])}):</p>
<ul>{''.join(f"<li>{md(x)}</li>" for x in pg['titulos'])}</ul>
<p>As que o código do PhxClaw impõe com teste: {len(bw['guardas'])} guardas em <code>{e(bw['fonte'])}</code>:</p>
<ul>{''.join(f"<li><code>{e(x)}</code></li>" for x in bw['guardas'])}</ul>
<p>E as {len(ac['qa'])} que o QA achou <b>sem</b> guarda provada (inventário de 01/10, no
<code>{e(sp['fonte'])}</code>):</p>
<ol>{''.join(f"<li>{md(x)}</li>" for x in ac['qa'])}</ol>"""))

    # ================================================================ 12 commits
    git_li = "".join(f'<li><code>{e(c["hash"])}</code><span class="dt">{e(c["data"])}</span><span>{e(c["assunto"])}</span></li>'
                     for c in g["ultimos"])
    secoes.append(("s-commits", "Últimos commits", f"""
<p>Os {len(g['ultimos'])} commits mais novos que tocam <code>phxclaw/</code>, de {g['commits']} ao todo, no ramo
<code>{e(g['ramo'])}</code> (datas em UTC).</p>
<ul class="git">{git_li}</ul>"""))

    # ================================================================ rodape
    fontes_rod = [
        ("versão, membros do workspace", v["fonte"], v["data"]),
        ("absorção", ab["fonte"], ab["data"]),
        ("sprints, achados, portões de sprint", sp["fonte"], sp["data"]),
        ("esteira de produção", es["fonte"], es["data"]),
        ("ferramentas", fe["fonte"], fe["data"]),
        ("equipe", eq["fonte"], eq["data"]),
        ("papéis de construção", pc["fonte"], pc["data"]),
        ("roteiro do Integrador", ro["fonte"], ro["data"]),
        ("portão único", pu["fonte"], pu["data"]),
        ("exceções do bwrap, guardas", bw["fonte"], bw["data"]),
        ("SecretBroker", bk["fonte"], bk["data"]),
        ("tokens da marca, fontes", tk["fonte"], tk["data"]),
        ("cláusulas pétreas", pg["fonte"], pg["data"]),
        ("commits, ramo", "git log", g["cabeca_data"]),
    ]
    if ce:
        fontes_rod.append(("certificação", ce["fonte"], ce["data"]))
    if qa:
        fontes_rod.append(("qualificação da UI", qa["fonte"], qa["data"]))
    if pa:
        fontes_rod.append(("relatório de partida da UI", pa["fonte"], pa["data"]))
    fontes_rod += [(f"medição {x['arquivo']}", x["fonte"], x["data"]) for x in fi]
    fontes_rod += [(f"roteiro {m['roteiro']}", m["resultado"]["arquivo"], m["resultado"]["data"]) for m in me if m["resultado"]]
    rod = tabela(["número", "fonte", "data", "como a data foi lida"],
                 [[e(a), f"<code>{e(b)}</code>", quando(c), f'<span class="mot">{e(c["como"])}</span>'] for a, b, c in fontes_rod])
    rodape = f"""<footer>
<p><b>Nenhum número desta página foi digitado.</b> Todos saem de <code>{ESTE}</code> e dos leitores de
<code>tools/dossie/numeros.py</code>; linhas de Rust e cognições são contadas na árvore de trabalho na hora da
corrida. A fonte e a data de cada um:</p>
{rod}
<p>Publicado em <a href="{URL}">{URL}</a> — sempre a mesma URL. Regerar:
<code>python3 {ESTE}</code>. Como e o que conferir: <code>docs/dossie/LEIA-ME.md</code>.</p>
</footer>"""

    # ---- montagem final
    numeradas = [(i, *s) for i, s in enumerate(secoes, 1)]
    indice = "".join(f'<li><a href="#{ident}"><span class="n">{i:02d}</span>{e(t)}</a></li>' for i, ident, t, _c in numeradas)
    corpo_secoes = "".join(secao(i, ident, t, c) for i, ident, t, c in numeradas)
    pagina = f"""<!doctype html>
<html lang="pt-BR">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Dossiê PhxClaw {e(v['versao'])}</title>
<style>
{css(tk)}
</style>
</head>
<body>
<div class="barra"><img src="{data_uri(ASSETS / 'marca-96.png')}" alt="" width="32" height="32"><b>PhxClaw</b>
<span class="vers">dossiê · {e(v['versao'])}</span>
<button class="botao" type="button" id="tema">Tema</button></div>
<main>
{capa}
<details class="indice" open><summary>Seções</summary><ol>{indice}</ol></details>
{corpo_secoes}
</main>
{rodape}
<script>
(function(){{var r=document.documentElement,b=document.getElementById('tema'),q=matchMedia('(prefers-color-scheme: dark)');
function escuro(){{var t=r.getAttribute('data-theme');return t?t==='dark':q.matches}}
function rot(){{b.textContent=escuro()?'Tema claro':'Tema escuro';b.setAttribute('aria-pressed',String(escuro()))}}
b.addEventListener('click',function(){{r.setAttribute('data-theme',escuro()?'light':'dark');rot()}});
if(q.addEventListener)q.addEventListener('change',rot);rot()}})();
</script>
</body>
</html>
"""
    pagina, figs = numerar(pagina)
    resumo = {"secoes": len(numeradas), "figuras": len(figs), "falta": pct(pct_falta), "absorcao": pct(ab["pct_agente"]),
              "versao": v["versao"]}
    return pagina, fez_menos, resumo


def main() -> int:
    args = sys.argv[1:]
    suite_arq = Path(args[args.index("--suite") + 1]) if "--suite" in args else None
    if "--novo" in args:
        v = N.versao()["versao"]
        nome = f"dossie-phxclaw-{'.'.join(v.split('.')[:2])}.html"
        existentes = [p for p in candidatos() if p.name != nome]
        if existentes:
            raise SystemExit(f"PARADA: --novo com {', '.join(p.name for p in existentes)} na pasta. Apague o velho "
                             "no mesmo trabalho (so existe um por vez).")
        alvo = PASTA / nome
    else:
        alvo = achar()
    pagina, fez_menos, resumo = montar(suite_arq)
    dados = pagina.encode("utf-8")
    if len(dados) > TETO_BYTES:
        raise SystemExit(f"PARADA: {alvo.name} teria {len(dados)} bytes, acima do teto de {TETO_BYTES} "
                         "(450 KiB: republicar exige reler a pagina publicada inteira)")
    mudou = not alvo.exists() or alvo.read_bytes() != dados
    if mudou:
        alvo.write_bytes(dados)
    print(f"{'gravado' if mudou else 'sem mudanca'}: {alvo.relative_to(RAIZ)} ({len(dados)} bytes, "
          f"{100 * len(dados) / TETO_BYTES:.1f}% do teto de 450 KiB)")
    print(f"versao {resumo['versao']}; {resumo['secoes']} secoes; {resumo['figuras']} figuras; "
          f"absorcao no agente {resumo['absorcao']}; falta {resumo['falta']} das sprints")
    if fez_menos:
        print("\nFEZ MENOS DO QUE O NOME PROMETE (na pagina como NAO MEDIDO):")
        for x in fez_menos:
            print(f"  - {x}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
