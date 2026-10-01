#!/usr/bin/env python3
"""Gera a pagina das 79 telas do console, na sequencia de uso.

    python3 docs/dossie/pagina-das-telas.py

Le `docs/dossie/telas/roteiro.json` (a ordem, os capitulos e a linha de cada
tela) e `docs/dossie/telas/ativos.json` (a URL de cada captura no deposito de
arquivos da pagina publicada) e grava `docs/dossie/telas-na-sequencia.html`.

Por que as imagens NAO vao embutidas, ao contrario das vinte da oitava pagina:
sao 79 capturas de 1500 px, ~12 MB. Embutidas, a pagina passaria 26x o teto de
republicacao de ~450 KiB (pedido 403) e nunca mais se republicaria. No deposito
de arquivos da propria pagina (capacidade `assets`) elas sobem uma vez, e a
pagina fica com poucos KiB. O preco, escrito: pagina que declara `assets` e
interna da organizacao -- por isso as telas moram numa pagina propria e NAO no
dossie, que e compartilhado (pedido 326).

Tela sem URL em `ativos.json` aparece NOMEADA, com o aviso de que a captura
ainda nao subiu -- nunca some da lista calada.
"""
import base64
import html
import json
import pathlib

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]
ROTEIRO = AQUI / "telas" / "roteiro.json"
ATIVOS = AQUI / "telas" / "ativos.json"
SAIDA = AQUI / "telas-na-sequencia.html"
SIMBOLO = RAIZ / "marca" / "derivados" / "phxsql-icone-64.png"

ESTILO = """
/* Layout: indice dos capitulos fixo a esquerda no desktop; uma coluna de telas,
   cada uma com o passo, o menu de origem e a captura em moldura. */
:root{
  --fundo:#f4f5f8; --painel:#ffffff; --linha:#d9dde6; --texto:#141a2b;
  --mudo:#566079; --ambar:#9a6a00; --acento:#c63c0a;
  --disp:"Exo 2",system-ui,sans-serif; --corpo:"Source Sans 3",system-ui,sans-serif;
  --dado:ui-monospace,"SF Mono",Menlo,monospace;
}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){
  --fundo:#010418; --painel:#0a1122; --linha:#1c2740; --texto:#dde2eb;
  --mudo:#8a93a6; --ambar:#ffc43d; --acento:#ff8a1c; color-scheme:dark}}
:root[data-theme="dark"]{
  --fundo:#010418; --painel:#0a1122; --linha:#1c2740; --texto:#dde2eb;
  --mudo:#8a93a6; --ambar:#ffc43d; --acento:#ff8a1c; color-scheme:dark}
*{box-sizing:border-box}
body{background:var(--fundo);color:var(--texto);font:16px/1.55 var(--corpo);
  padding-inline:16px;padding-block:24px 48px}
.casca{max-width:1240px;margin:0 auto;display:grid;grid-template-columns:240px minmax(0,1fr);gap:40px}
@media (max-width:900px){.casca{grid-template-columns:minmax(0,1fr)}nav.indice{position:static!important;max-height:none!important}}
header.topo{grid-column:1/-1;display:flex;gap:16px;align-items:center;flex-wrap:wrap}
header.topo img{width:48px;height:48px}
h1{font:700 clamp(28px,4vw,40px)/1.1 var(--disp);margin:0;text-wrap:balance}
.sub{color:var(--mudo);margin:4px 0 0;font-size:15px}
nav.indice{position:sticky;top:calc(env(safe-area-inset-top,0px) + 16px);align-self:start;
  max-height:calc(100vh - 32px);overflow:auto;font-size:14px}
nav.indice ol{list-style:none;margin:0;padding:0;display:grid;gap:2px}
nav.indice a{display:flex;gap:10px;padding:6px 8px;border-radius:6px;color:var(--texto);text-decoration:none}
nav.indice a:hover,nav.indice a:focus-visible{background:var(--painel);outline:2px solid var(--linha)}
nav.indice b{font:700 13px var(--dado);color:var(--ambar);min-width:22px;font-variant-numeric:tabular-nums}
main{min-width:0;display:grid;gap:56px}
section.cap>h2{font:700 26px/1.2 var(--disp);margin:0;display:flex;gap:12px;align-items:baseline}
section.cap>h2 span{font:700 18px var(--dado);color:var(--ambar)}
section.cap>p{color:var(--acento);margin:4px 0 20px}
.telas{display:grid;gap:36px}
figure{margin:0;display:grid;gap:10px}
figcaption{display:flex;gap:14px;align-items:baseline;flex-wrap:wrap}
.passo{font:700 15px var(--dado);color:var(--ambar);font-variant-numeric:tabular-nums}
.nome{font:600 19px var(--disp)}
.menu{font-size:13px;color:var(--mudo);text-transform:uppercase;letter-spacing:.06em}
.desc{margin:0;color:var(--texto);max-width:65ch}
.tambem{font-size:13px;color:var(--acento);font-style:italic}
.moldura{background:var(--painel);border:1px solid var(--linha);border-radius:10px;padding:6px;overflow:hidden}
.moldura img{display:block;width:100%;height:auto;border-radius:6px}
.moldura a:focus-visible{outline:2px solid var(--acento);outline-offset:2px}
.falta{padding:40px 16px;text-align:center;color:var(--mudo);font-size:14px}
footer{grid-column:1/-1;color:var(--mudo);font-size:13px;border-top:1px solid var(--linha);padding-top:16px}
"""


def main():
    rot = json.loads(ROTEIRO.read_text(encoding="utf-8"))
    ativos = json.loads(ATIVOS.read_text(encoding="utf-8")) if ATIVOS.exists() else {}
    simbolo = base64.b64encode(SIMBOLO.read_bytes()).decode()
    e = html.escape
    total = sum(len(c["telas"]) for c in rot["capitulos"])
    indice, corpo, faltam, passo = [], [], [], 1
    for k, c in enumerate(rot["capitulos"], 1):
        indice.append(f'<li><a href="#cap{k}"><b>{k:02d}</b>{e(c["nome"])}</a></li>')
        figs = []
        for t in c["telas"]:
            menu, _, resto = t["nome"].partition(" › ")
            nome = resto or menu
            menu = "tela inicial" if t["id"] == 0 else menu
            url = ativos.get(str(t["id"]))
            if url:
                img = (f'<a href="{e(url)}" target="_blank" rel="noopener">'
                       f'<img src="{e(url)}" alt="Tela {e(t["nome"])} do console do PhxSql" '
                       f'loading="lazy" width="1500" height="900"></a>')
            else:
                faltam.append(t["nome"])
                img = '<div class="falta">A captura desta tela ainda não subiu para a página.</div>'
            tambem = f'<span class="tambem">{e(t["tambem"])}</span>' if t.get("tambem") else ""
            figs.append(
                f'<figure id="t{t["id"]}"><figcaption><span class="passo">{passo}/{total}</span>'
                f'<span class="nome">{e(nome)}</span><span class="menu">{e(menu)}</span>{tambem}</figcaption>'
                f'<p class="desc">{e(t["desc"])}</p><div class="moldura">{img}</div></figure>')
            passo += 1
        corpo.append(
            f'<section class="cap" id="cap{k}"><h2><span>{k:02d}</span>{e(c["nome"])}</h2>'
            f'<p>{e(c["sub"])}</p><div class="telas">{"".join(figs)}</div></section>')
    pagina = f"""<title>Telas do PhxSql</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@600;700&family=Source+Sans+3:wght@400;600&display=swap">
<style>{ESTILO}</style>
<div class="casca">
<header class="topo"><img src="data:image/png;base64,{simbolo}" alt="">
<div><h1>As telas do console, na sequência de uso</h1>
<p class="sub">{total} telas em {len(rot["capitulos"])} capítulos · capturadas do servidor real, tema escuro, commit <code>{e(rot["commit"])}</code></p></div></header>
<nav class="indice" aria-label="Capítulos"><ol>{"".join(indice)}</ol></nav>
<main>{"".join(corpo)}</main>
<footer>Gerada por <code>docs/dossie/pagina-das-telas.py</code> a partir de <code>docs/dossie/telas/roteiro.json</code>. Seis itens do menu abrem diálogo do navegador, dois estão desligados no código e dez são ações, não telas: por isso não aparecem aqui. Clique numa captura para abri-la no tamanho original.</footer>
</div>
"""
    SAIDA.write_text(pagina, encoding="utf-8")
    print(f"pagina gravada: {SAIDA} ({len(pagina.encode()):,} bytes), {total} telas")
    if faltam:
        print(f"ATENCAO: {len(faltam)} tela(s) sem captura no deposito -- a pagina as nomeia:")
        for n in faltam:
            print("   ", n)


if __name__ == "__main__":
    main()
