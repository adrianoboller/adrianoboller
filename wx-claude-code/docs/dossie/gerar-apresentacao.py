#!/usr/bin/env python3
"""Gera o roteiro de apresentacao do plugin, para levar a reuniao.

Vinte minutos, sete blocos, com o comando exato de cada um e a frase que o
sustenta. Os numeros -- versao, comandos, testes, cenarios, perguntas, itens que
faltam -- saem do repositorio, porque roteiro com numero digitado a mao e a
maneira mais rapida de dizer uma bobagem na frente do cliente.

O bloco final NAO e enfeite: ele lista o que o plugin nao faz. Vender sem isso
cria a expectativa que o produto nao atende, e o primeiro cliente decepcionado
custa mais caro que os tres que a franqueza espantou.

Uso: python3 docs/dossie/gerar-apresentacao.py [saida.html]
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
MESES = ("janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto",
         "setembro", "outubro", "novembro", "dezembro")


def medir() -> dict:
    n = json.loads((RAIZ / "docs/dossie/numeros.json").read_text(encoding="utf-8"))
    falta = re.findall(r"^- estado: `(\w+)`", (RAIZ / "docs/PENDENCIAS.md").read_text(encoding="utf-8"), re.M)
    cen = len(re.findall(r'^\s+\("\d+ ', (RAIZ / "tests/cenarios.py").read_text(encoding="utf-8"), re.M))
    # a contagem de perguntas nao esta no numeros.json: sai da fonte, como no
    # gerador do fluxo -- inventar a chave daria KeyError, e pior, um numero
    # digitado a mao no roteiro que se le na frente do cliente
    q = subprocess.run([sys.executable, str(RAIZ / "skills/conversao-wx/scripts/listar_perguntas.py"),
                        "--json"], capture_output=True, text=True)
    perguntas = len(json.loads(q.stdout)) if q.returncode == 0 else 0
    if not perguntas:
        raise SystemExit("nao consegui contar as perguntas; roteiro nao sai com numero chutado")
    return {**n, "cenarios": cen, "perguntas": perguntas, "faltam": falta.count("falta"),
            "parciais": falta.count("parcial"), "feitos": falta.count("feito")}


# (minutos, titulo, o que fazer, comandos, a frase, tipo)
def blocos(m: dict) -> list[dict]:
    return [
        {"min": "0", "t": "Antes de entrar", "tipo": "prep",
         "faz": "Sozinho, dez minutos antes. Terminal limpo, fonte grande, e os três PDFs "
                "abertos em abas: workflow, fluxo e o que falta.",
         "cmd": ["python3 skills/conversao-wx/scripts/validate_plugin_bundle.py . --strict",
                 "claude plugin validate ."],
         "frase": ""},
        {"min": "1½", "t": "O problema — sem tela", "tipo": "fala",
         "faz": "Não abra nada. Só a frase. Se você abrir o terminal aqui, perde o único "
                "momento em que a sala está ouvindo em vez de lendo.",
         "cmd": [],
         "frase": "Você tem um ERP em WINDEV com dez anos de regra de negócio. Converter à mão "
                  "é caro; converter com IA solta é pior — ninguém sabe de onde veio cada regra "
                  "nem se o resultado faz a mesma conta. O que falta não é tradutor: é prova."},
        {"min": "2", "t": "O mapa", "tipo": "mostra",
         "faz": "Abra o <b>workflow.pdf</b>. Aponte os dois losangos — «aprova o G0?» e "
                "«homologa?» — e a seta vermelha que volta do QA para quem escreveu.",
         "cmd": [],
         "frase": "Aqui o trabalho para e espera uma pessoa. É de propósito. E quem valida não "
                  "conserta o que detecta — isso é um hook, não regra de etiqueta."},
        {"min": "2", "t": "A instalação", "tipo": "roda",
         "faz": "Ao vivo, do zero. São dois comandos e nenhuma dependência externa.",
         "cmd": ["claude plugin marketplace add adrianoboller/adrianoboller",
                 "claude plugin install wx-claude-code@wx-claude-code"],
         "frase": ""},
        {"min": "4", "t": "O questionário — o momento que vende", "tipo": "roda",
         "faz": f"Mostre <b>uma</b> pergunta sendo respondida das {m['perguntas']}. "
                "Depois faça a demonstração que impressiona: <b>cole uma senha de propósito</b> "
                "(«usuário admin, senha 123456»). Ele não grava e não repete — guarda só o nome "
                "da credencial.",
         "cmd": ["/wx-claude-code:questionario", "/wx-claude-code:progresso retomar"],
         "frase": f"São {m['perguntas']} itens. Ninguém responde numa sessão só — "
                  "ele sabe onde você parou."},
        {"min": "3", "t": "O portão — mostre o «não»", "tipo": "roda",
         "faz": "Com um PDF falso no exemplo, o G0 devolve <b>BLOCKED</b>. Aí peça ao Claude "
                "para escrever código: o hook recusa. É a cena mais forte da demonstração, "
                "porque é a ferramenta trabalhando contra quem a opera.",
         "cmd": ["/wx-claude-code:preflight"],
         "frase": "Enquanto a evidência não passa, nenhum código é escrito. Não é aviso: "
                  "é bloqueio."},
        {"min": "4", "t": "A prova — o coração", "tipo": "roda",
         "faz": "A regra WLanguage do PDF → o Rust com a <b>página de origem citada dentro do "
                "código</b> → o golden master → o grafo.",
         "cmd": ["/wx-claude-code:golden comparar", "/wx-claude-code:grafo conferir"],
         "frase": "O grafo achou, no nosso próprio piloto, um arquivo que requisito nenhum "
                  "pedia — e dentro dele estava arredondamento de dinheiro. É um defeito real, "
                  "achado pela ferramenta contra o próprio autor."},
        {"min": "2", "t": "O fechamento honesto", "tipo": "mostra",
         "faz": "Abra o <b>o-que-falta.pdf</b> e leia em voz alta. Quem faz isso ganha a sala.",
         "cmd": [],
         "frase": f"{m['faltam']} itens ainda faltam. Nenhum projeto WINDEV real passou pelos "
                  "sete gates — o exemplo é sintético. Ele não lê .wdp nem .wda, lê o PDF que o "
                  "IDE exporta. Não há transpilador: a tradução é feita por agente. Por isso é "
                  "300 €, e não 3.000: você compra o método e a prova, não uma fábrica."},
    ]


PERGUNTAS = [
    ("Quanto custa converter meu sistema?",
     "<b>«Não sei ainda — nunca medi num projeto real, e não vou chutar.»</b> Diga assim mesmo. "
     "É a resposta que constrói confiança, e é verdade: o item 18 da lista é exatamente esse."),
    ("Ele converte sozinho?",
     "Não. É copiloto governado: a conversão é feita <b>com</b> ele, e cada regra sai com origem, "
     "teste e prova contra o legado."),
    ("E se eu não usar WINDEV?",
     "O legado é E/OU: PHP, C, C++, Clarion, COBOL. Mostre o cenário 13 da bateria e o vídeo do "
     "legado PHP."),
    ("Meus dados vão para a nuvem?",
     "O corpus e os scripts rodam na máquina. A telemetria fica no disco — sair é explícito, e "
     "argumento de comando não vai junto."),
]


def pagina(m: dict) -> str:
    hoje = date.today()
    bl = []
    for i, b in enumerate(blocos(m)):
        cor = {"prep": "var(--m)", "fala": "var(--a)", "mostra": "var(--az)", "roda": "var(--vd)"}[b["tipo"]]
        rotulo = {"prep": "preparo", "fala": "só fala", "mostra": "mostra a página",
                  "roda": "roda ao vivo"}[b["tipo"]]
        cmds = "".join(f"<div class=\"cmd\">{E(c)}</div>" for c in b["cmd"])
        frase = f'<p class="frase">“{E(b["frase"])}”</p>' if b["frase"] else ""
        bl.append(f'<article><div class="num">{i}</div><div class="corpo">'
                  f'<h3>{E(b["t"])} <span class="tag" style="border-color:{cor};color:{cor}">'
                  f'{rotulo}</span> <span class="min">{E(b["min"])} min</span></h3>'
                  f'<p>{b["faz"]}</p>{cmds}{frase}</div></article>')
    perg = "".join(f"<tr><td><b>{E(a)}</b></td><td>{b}</td></tr>" for a, b in PERGUNTAS)
    return f"""<title>Como apresentar o WX Claude Code</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@500;700;800&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=JetBrains+Mono:wght@400;600&display=swap">
<style>
:root{{--g:#FBFAF7;--p:#FFFFFF;--i:#14161F;--m:#6B6F82;--l:#E4E2DB;--a:#C63C0A;--az:#1F5FBF;--vd:#1F7A4D}}
*{{box-sizing:border-box}}body{{margin:0;background:var(--g);color:var(--i);font-family:"Source Serif 4",Georgia,serif;font-size:15px;line-height:1.55}}
.wrap{{max-width:920px;margin:0 auto;padding:36px 26px 60px}}
h1,h2,h3{{font-family:"Exo 2","Segoe UI",sans-serif;margin:0}}
h1{{font-size:36px;font-weight:800;letter-spacing:-.5px;line-height:1.1;margin-top:6px}}
h2{{font-size:20px;font-weight:700;margin:36px 0 12px}}
h3{{font-size:17px;font-weight:700}}
.eyebrow{{font-family:"Exo 2",sans-serif;font-size:12px;letter-spacing:.14em;text-transform:uppercase;color:var(--a);font-weight:700}}
.lead{{max-width:70ch;color:var(--m);margin:12px 0 0;font-size:16.5px}}
header{{border-bottom:2px solid var(--i);padding-bottom:18px}}
.kpis{{display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin-top:20px}}
.kpis div{{background:var(--p);border:1px solid var(--l);padding:11px 13px;font-family:"Exo 2",sans-serif}}
.kpis b{{display:block;font-size:24px;font-weight:800;font-variant-numeric:tabular-nums}}
.kpis small{{color:var(--m);font-size:11px;letter-spacing:.06em;text-transform:uppercase}}
article{{display:flex;gap:16px;background:var(--p);border:1px solid var(--l);padding:16px 18px;margin-top:12px;page-break-inside:avoid}}
.num{{font-family:"Exo 2",sans-serif;font-size:30px;font-weight:800;color:var(--l);line-height:1;min-width:34px}}
.corpo{{flex:1}}.corpo p{{margin:8px 0 0;max-width:66ch}}
.tag{{font-family:"Exo 2",sans-serif;font-size:10px;letter-spacing:.08em;text-transform:uppercase;border:1px solid;padding:2px 7px;vertical-align:middle;font-weight:700}}
.min{{font-family:"Exo 2",sans-serif;font-size:12px;color:var(--m);font-weight:600}}
.cmd{{font-family:"JetBrains Mono",monospace;font-size:12.5px;background:#14161F;color:#E6E8F2;padding:7px 11px;margin-top:8px;overflow-x:auto}}
.frase{{border-left:3px solid var(--a);padding-left:12px;font-size:15.5px;color:var(--i);margin-top:12px!important}}
table{{border-collapse:collapse;width:100%;font-size:14px;background:var(--p);border:1px solid var(--l)}}
th{{font-family:"Exo 2",sans-serif;font-size:11px;letter-spacing:.08em;text-transform:uppercase;text-align:left;color:var(--m);padding:9px 10px;border-bottom:2px solid var(--l)}}
td{{padding:9px 10px;border-bottom:1px solid var(--l);vertical-align:top}}
.nota{{color:var(--m);font-size:14px;max-width:74ch;margin-top:14px}}
code{{font-family:"JetBrains Mono",monospace;font-size:12.5px;background:#F2F0EA;padding:1px 4px}}
@media print{{.wrap{{padding:0}}article{{border-color:#ccc}}}}
</style>
<div class="wrap">
<header>
 <div class="eyebrow">WX Claude Code {E(m['versao'])} · roteiro de apresentação · {hoje.day} de {MESES[hoje.month - 1]} de {hoje.year}</div>
 <h1>Vinte minutos, sete blocos</h1>
 <p class="lead">Regra que vale para tudo: <b>não mostre slide onde couber a coisa rodando</b>.
 Você tem terminal, exemplo real e vídeo. E abra dizendo o que o plugin <b>não</b> faz — quem
 faz isso ganha a sala. Página gerada; os números saem do repositório.</p>
 <div class="kpis">
  <div><b>{m['comandos']}</b><small>comandos</small></div>
  <div><b>{m['testes']} · {m['cenarios']}</b><small>testes · cenários</small></div>
  <div><b>{m['corpus_paginas_validas']:,}</b><small>páginas do Help</small></div>
  <div><b>{m['faltam']}</b><small>itens que ainda faltam</small></div>
 </div>
</header>
<h2>O roteiro</h2>
{"".join(bl)}
<h2>Se a apresentação for gravada ou assíncrona</h2>
<p>Use os vídeos em vez de repetir a demonstração: <code>docs/video/wx-claude-code-video-de-uso.mp4</code>
({E(m['video_duracao'])}, {m['video_cenas']} cenas) e <code>…-video-php.mp4</code>
({E(m['video_php_duracao'])}, {m['video_php_cenas']} cenas — legado que nunca foi WINDEV).
Toda cena é saída real de sessão, não montagem.</p>
<h2>As quatro perguntas que vão fazer</h2>
<table><thead><tr><th>pergunta</th><th>resposta curta</th></tr></thead><tbody>{perg}</tbody></table>
<p class="nota">O bloco 7 não é modéstia: é proteção. Vender sem ele cria a expectativa que o
produto não atende, e o primeiro cliente decepcionado custa mais caro que os três que a franqueza
espantou. Leve o <code>o-que-falta.pdf</code> impresso — ele lista {m['faltam']} itens que faltam,
{m['parciais']} começados e {m['feitos']} feitos, contados por gerador.</p>
</div>
"""


def main() -> int:
    saida = Path(sys.argv[1]) if len(sys.argv) > 1 else RAIZ / "docs/apresentacao.html"
    m = medir()
    saida.write_text(pagina(m), encoding="utf-8")
    print(f"ok {saida} ({len(blocos(m))} blocos, {m['faltam']} itens que faltam)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
