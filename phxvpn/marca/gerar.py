#!/usr/bin/env python3
"""Gera as pecas da marca do phxvpn a partir de simbolo.svg (a unica fonte do
desenho): logo horizontal, icone de app e a folha de marca. Os PNG saem do
renderizar.mjs (Chromium)."""
import base64, pathlib, re
AQUI = pathlib.Path(__file__).parent
simb = (AQUI / 'simbolo.svg').read_text()
miolo = simb[simb.index('>', simb.index('<svg')) + 1: simb.rindex('</svg>')]
fonte = base64.b64encode((AQUI.parent / 'provas/video/fontes/exo2-700.ttf').read_bytes()).decode()
fonte500 = base64.b64encode((AQUI.parent / 'provas/video/fontes/exo2-500.ttf').read_bytes()).decode()
LEMA = 'BUILT TO CONNECT. ENGINEERED TO PROTECT.'

def aninhado(x, y, lado, prefixo):
    # ids por peca: dois simbolos na mesma pagina nao podem dividir gradientes
    m = re.sub(r'id="([^"]+)"', lambda g: f'id="{prefixo}{g.group(1)}"', miolo)
    m = re.sub(r'url\(#([^)]+)\)', lambda g: f'url(#{prefixo}{g.group(1)})', m)
    m = re.sub(r'href="#([^"]+)"', lambda g: f'href="#{prefixo}{g.group(1)}"', m)
    return f'<svg x="{x}" y="{y}" width="{lado}" height="{lado}" viewBox="0 0 512 512">{m}</svg>'

estilo = f'''<style>@font-face{{font-family:"Exo 2 phx";font-weight:700;src:url(data:font/ttf;base64,{fonte}) format("truetype")}}
@font-face{{font-family:"Exo 2 phx";font-weight:500;src:url(data:font/ttf;base64,{fonte500}) format("truetype")}}
.marca{{font-family:"Exo 2 phx","Exo 2",sans-serif;font-weight:700}}.lema{{font-family:"Exo 2 phx","Exo 2",sans-serif;font-weight:500}}</style>'''
brilho = '''<linearGradient id="cromo" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#FFFFFF"/><stop offset=".55" stop-color="#DDE2EB"/><stop offset="1" stop-color="#9aa3b8"/></linearGradient>
<linearGradient id="xis" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#FFC43D"/><stop offset=".5" stop-color="#FF4D10"/><stop offset="1" stop-color="#D71A1A"/></linearGradient>
<linearGradient id="risco" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#FF4D10" stop-opacity="0"/><stop offset=".5" stop-color="#FFC43D"/><stop offset="1" stop-color="#FF4D10" stop-opacity="0"/></linearGradient>'''

def palavra(x, y, tam):
    return (f'<text class="marca" x="{x}" y="{y}" font-size="{tam}" letter-spacing="1">'
            f'<tspan fill="url(#cromo)">ph</tspan><tspan fill="url(#xis)">x</tspan><tspan fill="url(#cromo)">vpn</tspan></text>')

horizontal = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 360" role="img" aria-label="phxvpn">
<defs>{brilho}</defs>{estilo}
{aninhado(10, 10, 340, "h")}
<line x1="372" y1="70" x2="372" y2="290" stroke="#2c3656" stroke-width="2"/>
{palavra(410, 218, 150)}
<rect x="410" y="244" width="720" height="4" fill="url(#risco)"/>
<text class="lema" x="414" y="296" font-size="24" letter-spacing="5" fill="#b9c0cf">{LEMA}</text>
</svg>'''
(AQUI / 'logo-horizontal.svg').write_text(horizontal)

vertical = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 900" role="img" aria-label="phxvpn">
<defs>{brilho}</defs>{estilo}
{aninhado(100, 0, 600, "v")}
{palavra(400, 730, 150).replace('<text ', '<text text-anchor="middle" ')}
<rect x="130" y="760" width="540" height="4" fill="url(#risco)"/>
<text class="lema" x="400" y="820" font-size="22" letter-spacing="6" fill="#b9c0cf" text-anchor="middle">{LEMA}</text>
</svg>'''
(AQUI / 'logo-vertical.svg').write_text(vertical)

icone = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" role="img" aria-label="phxvpn">
<defs><linearGradient id="fundo" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#0d1630"/><stop offset="1" stop-color="#010418"/></linearGradient></defs>
<rect x="8" y="8" width="496" height="496" rx="110" fill="url(#fundo)" stroke="#FF4D10" stroke-opacity=".55" stroke-width="4"/>
{aninhado(46, 40, 420, "i")}
</svg>'''
(AQUI / 'icone-app.svg').write_text(icone)
print('ok: logo-horizontal.svg, logo-vertical.svg, icone-app.svg')

# --- Folha de marca (pagina unica, SVG embutidos como data URI)
def uri(nome):
    return 'data:image/svg+xml;base64,' + base64.b64encode((AQUI / nome).read_bytes()).decode()
cores = [('#FFC43D', 'Âmbar'), ('#FF8A1C', 'Laranja'), ('#FF4D10', 'Vermelhão'), ('#D71A1A', 'Brasa'), ('#8B0D0D', 'Carvão'), ('#DDE2EB', 'Cromo'), ('#010418', 'Noite')]
recursos = [('Cifra de ponta a ponta', 'Noise IKpsk2 + ChaCha20-Poly1305, conferido contra o vetor oficial'),
            ('Sem servidor, ou o seu', 'P2P direto, ou relé e painel próprios; nada passa pelo fabricante'),
            ('USB pela rede', 'o pendrive de um membro vira local no outro, só dentro da rede'),
            ('Um binário', 'Linux e Windows, sem bibliotecas de fora; .deb, .msi e serviço')]
folha = f'''<meta charset="utf-8">
<title>Marca phxvpn</title>
<link href="https://fonts.googleapis.com/css2?family=Exo+2:wght@500;600;700&display=swap" rel="stylesheet">
<style>
:root{{color-scheme:dark}}
body{{background:#010418;color:#DDE2EB;font:15px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif;padding-inline:16px;padding-block:24px 40px}}
.f{{max-width:1180px;margin:0 auto;display:grid;gap:18px}}
.rot{{font:600 11px/1 "Exo 2",system-ui,sans-serif;letter-spacing:.16em;text-transform:uppercase;color:#8a93a8;margin:0 0 12px}}
.topo{{display:grid;grid-template-columns:minmax(0,1.4fr) minmax(0,1fr);gap:18px}}
.pl{{background:#050a1c;border:1px solid #1c2742;border-radius:14px;padding:20px}}
.pl img{{display:block;margin:0 auto;max-width:100%;height:auto}}
.lado{{display:grid;gap:18px}}
.cores{{display:grid;grid-template-columns:repeat(auto-fit,minmax(100px,1fr));gap:12px}}
.cor i{{display:block;aspect-ratio:1.4;border-radius:10px;border:1px solid #1c2742}}
.cor b{{display:block;font:600 13px "Exo 2",system-ui,sans-serif;margin-top:6px}}.cor span{{font:12px ui-monospace,monospace;color:#8a93a8}}
.tipo{{display:grid;grid-template-columns:auto 1fr;gap:24px;align-items:center}}
.aa{{font:700 96px/1 "Exo 2",system-ui,sans-serif}}
.alf{{font:500 17px/1.8 "Exo 2",system-ui,sans-serif;letter-spacing:.14em;color:#b9c0cf;overflow-wrap:anywhere}}
.rec{{display:grid;grid-template-columns:repeat(auto-fit,minmax(220px,1fr));gap:14px}}
.rec div{{border-left:2px solid #FF4D10;padding-left:12px}}
.rec b{{display:block;font:600 13px "Exo 2",system-ui,sans-serif;letter-spacing:.08em;text-transform:uppercase}}.rec span{{color:#8a93a8;font-size:14px}}
.nota{{color:#8a93a8;font-size:13px;max-width:78ch}}
@media (max-width:760px){{.topo{{grid-template-columns:1fr}}.tipo{{grid-template-columns:1fr}}.aa{{font-size:72px}}}}
</style>
<div class="f">
<div class="topo">
 <div class="pl"><p class="rot">Logo principal</p><img src="{uri('logo-vertical.svg')}" width="560" alt="phxvpn: fênix com cadeado no peito e o cano do túnel nos pés"></div>
 <div class="lado">
  <div class="pl"><p class="rot">Símbolo</p><img src="{uri('simbolo.svg')}" width="230" alt=""></div>
  <div class="pl"><p class="rot">Logo horizontal</p><img src="{uri('logo-horizontal.svg')}" width="460" alt=""></div>
  <div class="pl"><p class="rot">Ícone de aplicativo</p><img src="{uri('icone-app.svg')}" width="140" alt=""></div>
 </div>
</div>
<div class="pl"><p class="rot">O que o desenho diz</p><p class="nota">A fênix da família Phoenix, com duas trocas em relação ao PhxSql: o <b>cadeado no peito</b> no lugar do cilindro de dados, porque o produto guarda a conexão e não o banco; e o <b>cano nos pés</b>, que é o túnel: as setas mostram o tráfego nos dois sentidos, e as trilhas de circuito ligam os dois computadores. O «x» laranja é a assinatura da família, como em Phx<span style="color:#FF4D10">x</span>Sql.</p></div>
<div class="pl"><p class="rot">Paleta</p><div class="cores">{"".join(f'<div class="cor"><i style="background:{h}"></i><b>{n}</b><span>{h}</span></div>' for h, n in cores)}</div></div>
<div class="pl tipo"><div class="aa">Aa</div><div><p class="rot" style="margin-bottom:4px">Exo 2 · SemiBold / Medium</p><div class="alf">ABCDEFGHIJKLMNOPQRSTUVWXYZ<br>abcdefghijklmnopqrstuvwxyz<br>0123456789 !@#$%&amp;*()</div></div></div>
<div class="pl"><p class="rot">O que a marca promete (e o que está provado)</p><div class="rec">{"".join(f'<div><b>{t}</b><span>{d}</span></div>' for t, d in recursos)}</div>
<p class="nota" style="margin-top:14px">Lema proposto, para o dono decidir: «Built to connect. Engineered to protect.» Os arquivos estão em <code>phxvpn/marca/</code> (SVG e PNG), gerados por <code>gerar.py</code> e <code>renderizar.mjs</code> a partir de um único desenho, <code>simbolo.svg</code>.</p></div>
</div>'''
(AQUI / 'folha.html').write_text(folha)
print('ok: folha.html')
