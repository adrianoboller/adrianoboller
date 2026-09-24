#!/usr/bin/env python3
"""Gera as duas propostas de simbolo do PhxZip, a partir do simbolo da familia.

    python3 marca/vetor/gerar-phxzip.py

Pedido do dono, 24/09/2026: «O logo deve ter uma morsa de aperto e a Phoenix
pousada em cima. Pode ser um cadeado laranja com asas.» Saem as duas, para
ele escolher:

  phxzip-simbolo-morsa.svg    a fenix pousada na morsa que aperta o arquivo
  phxzip-simbolo-cadeado.svg  o cadeado laranja com asas

As asas, o pescoco, a cabeca e os gradientes vem do `phx-simbolo.svg` por
leitura -- a familia continua sendo UM desenho de fenix, e mudar a asa la
muda aqui na proxima corrida. So a morsa e o cadeado sao desenhados aqui.
Sem dependencia nenhuma: e texto sobre texto.
"""
import re
from pathlib import Path

AQUI = Path(__file__).resolve().parent


def pecas():
    base = (AQUI / "phx-simbolo.svg").read_text(encoding="utf-8")
    defs = base[base.index("<defs>") : base.index("</defs>")]
    cabeca = base[base.index("  <!-- a fenix: pescoco") : base.index("</svg>")]
    return defs, cabeca


ACO = """    <linearGradient id="aco" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#3a4668"/><stop offset=".5" stop-color="#1a2440"/><stop offset="1" stop-color="#0a1122"/>
    </linearGradient>
"""


def morsa():
    defs, cabeca = pecas()
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" role="img" aria-label="PhxZip">
  <title>PhxZip — a fênix pousada na morsa que aperta o arquivo</title>
  {defs}{ACO}  </defs>
  <circle cx="256" cy="250" r="190" fill="url(#brilho)" opacity=".28"/>

  <!-- a fenix, pousada: as asas, o pescoco e a cabeca do simbolo da familia -->
  <g transform="translate(256 200) scale(.8) translate(-256 -250)">
    <use href="#asa-esq"/>
    <use href="#asa-esq" transform="translate(512 0) scale(-1 1)"/>
    <!-- o peito em chama, no lugar do cilindro do PhxSql -->
    <path fill="url(#fogo)" d="M256 330 C214 322 196 290 204 256 C210 232 228 222 248 220 L284 224 C304 234 314 254 310 280 C304 308 286 324 256 330 Z"/>
{cabeca}  </g>
  <!-- as garras seguram a boca da morsa -->
  <g fill="none" stroke="#FFC43D" stroke-width="6" stroke-linecap="round">
    <path d="M236 262 L228 280 M236 262 L238 282 M236 262 L248 280"/>
    <path d="M276 262 L266 280 M276 262 L276 282 M276 262 L286 280"/>
  </g>

  <!-- a morsa: base, corpo, as duas bocas, o fuso e a manivela -->
  <rect x="92" y="410" width="328" height="26" rx="8" fill="url(#aco)" stroke="#FF8A1C" stroke-width="4"/>
  <path d="M130 410 L150 346 H362 L382 410 Z" fill="url(#aco)" stroke="#FF8A1C" stroke-width="4" stroke-linejoin="round"/>
  <rect x="130" y="280" width="104" height="70" rx="6" fill="url(#aco)" stroke="#FF8A1C" stroke-width="4"/>
  <rect x="278" y="280" width="104" height="70" rx="6" fill="url(#aco)" stroke="#FF8A1C" stroke-width="4"/>
  <g stroke="#FFC43D" stroke-width="3" opacity=".8">
    <path d="M226 288 V342 M218 288 V342"/><path d="M286 288 V342 M294 288 V342"/>
  </g>
  <!-- o arquivo apertado entre as bocas: a compressao em si, com o ziper -->
  <path d="M234 292 Q245 285 256 292 T278 292 V338 Q267 345 256 338 T234 338 Z" fill="url(#fogo)"/>
  <path d="M256 294 V336" stroke="#010418" stroke-width="3" stroke-dasharray="4 4"/>
  <rect x="382" y="354" width="66" height="14" rx="7" fill="#DDE2EB" opacity=".85"/>
  <rect x="444" y="318" width="14" height="86" rx="7" fill="#FF8A1C"/>
  <circle cx="451" cy="314" r="11" fill="#FFC43D"/><circle cx="451" cy="408" r="11" fill="#FFC43D"/>
</svg>
"""


def cadeado():
    defs, _ = pecas()
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" role="img" aria-label="PhxZip">
  <title>PhxZip — o cadeado laranja com asas</title>
  {defs}  </defs>
  <circle cx="256" cy="290" r="180" fill="url(#brilho)" opacity=".3"/>
  <g transform="translate(0 30)">
    <use href="#asa-esq"/>
    <use href="#asa-esq" transform="translate(512 0) scale(-1 1)"/>
  </g>
  <!-- a argola -->
  <path d="M190 262 V200 C190 160 220 132 256 132 C292 132 322 160 322 200 V262" fill="none" stroke="#DDE2EB" stroke-width="26" stroke-linecap="round"/>
  <path d="M190 262 V200 C190 160 220 132 256 132 C292 132 322 160 322 200 V262" fill="none" stroke="#FFC43D" stroke-width="6" stroke-linecap="round" opacity=".7"/>
  <!-- o corpo laranja -->
  <rect x="158" y="248" width="196" height="170" rx="30" fill="url(#fogo)" stroke="#FFC43D" stroke-width="5"/>
  <rect x="176" y="266" width="160" height="12" rx="6" fill="#FFC43D" opacity=".55"/>
  <!-- o buraco da chave, e o ziper que desce dele: um arquivo trancado -->
  <circle cx="256" cy="320" r="22" fill="#010418"/>
  <path d="M246 332 H266 L262 372 H250 Z" fill="#010418"/>
  <path d="M256 380 V408" stroke="#010418" stroke-width="4" stroke-dasharray="5 4"/>
</svg>
"""


def pequeno(svg, corte):
    """Versao de 16-48 px: sem o brilho de fundo, recortada mais perto."""
    svg = re.sub(r'<circle cx="256" cy="\d+" r="1\d0" fill="url\(#brilho\)"[^>]*/>', "", svg)
    return svg.replace('viewBox="0 0 512 512"', f'viewBox="{corte}"')


def main():
    m, c = morsa(), cadeado()
    (AQUI / "phxzip-simbolo-morsa.svg").write_text(m, encoding="utf-8")
    (AQUI / "phxzip-simbolo-cadeado.svg").write_text(c, encoding="utf-8")
    (AQUI / "phxzip-icone-morsa.svg").write_text(pequeno(m, "60 40 412 412"), encoding="utf-8")
    (AQUI / "phxzip-icone-cadeado.svg").write_text(pequeno(c, "60 70 392 392"), encoding="utf-8")
    print("gerados: phxzip-simbolo-{morsa,cadeado}.svg e phxzip-icone-{morsa,cadeado}.svg")


if __name__ == "__main__":
    main()
