#!/usr/bin/env python3
"""Gera as duas propostas de simbolo do PhxZip, a partir do simbolo da familia.

    python3 marca/vetor/gerar-phxzip.py

Decisao do dono, 24/09/2026, depois de ver a morsa e o cadeado lado a
lado: «So o cadeado laranja ja ficou bom. Phoenix carregando um cadeado
laranja.» A morsa saiu do repositorio -- um simbolo por vez, como o dossie.

  phxzip-simbolo.svg  a fenix carregando o cadeado laranja
  phxzip-icone.svg    o mesmo, recortado para 16-48 px

As asas, o pescoco, a cabeca e os gradientes vem do `phx-simbolo.svg` por
leitura -- a familia continua sendo UM desenho de fenix, e mudar a asa la
muda aqui na proxima corrida. So o cadeado e desenhado aqui.
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




def simbolo():
    """A fenix carregando o cadeado laranja: asas e cabeca no alto, as garras
    na argola, o cadeado pendurado embaixo."""
    defs, cabeca = pecas()
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" role="img" aria-label="PhxZip">
  <title>PhxZip — a fênix carregando o cadeado laranja</title>
  {defs}  </defs>
  <circle cx="256" cy="250" r="200" fill="url(#brilho)" opacity=".28"/>

  <!-- a fenix: asas abertas, peito em chama, pescoco e cabeca da familia -->
  <g transform="translate(256 164) scale(.74) translate(-256 -250)">
    <use href="#asa-esq"/>
    <use href="#asa-esq" transform="translate(512 0) scale(-1 1)"/>
    <path fill="url(#fogo)" d="M256 336 C214 326 196 292 204 258 C210 234 228 222 248 220 L284 224 C304 234 314 256 310 282 C304 312 286 330 256 336 Z"/>
{cabeca}  </g>

  <!-- a argola, presa nas garras -->
  <g transform="translate(0 14)"><path d="M200 318 V262 C200 230 226 206 256 206 C286 206 312 230 312 262 V318" fill="none" stroke="#DDE2EB" stroke-width="22" stroke-linecap="round"/>
  <path d="M200 318 V262 C200 230 226 206 256 206 C286 206 312 230 312 262 V318" fill="none" stroke="#FFC43D" stroke-width="5" stroke-linecap="round" opacity=".7"/>
  <g fill="none" stroke="#FFC43D" stroke-width="6" stroke-linecap="round">
    <path d="M238 184 L232 208 M238 184 L244 210 M238 184 L224 204"/>
    <path d="M274 184 L268 210 M274 184 L280 208 M274 184 L288 204"/>
  </g>

  <!-- o cadeado laranja, pendurado -->
  <rect x="170" y="304" width="172" height="150" rx="28" fill="url(#fogo)" stroke="#FFC43D" stroke-width="5"/>
  <rect x="186" y="320" width="140" height="11" rx="5.5" fill="#FFC43D" opacity=".55"/>
  <!-- o buraco da chave, e o ziper que desce dele: um arquivo trancado -->
  <circle cx="256" cy="368" r="20" fill="#010418"/>
  <path d="M247 379 H265 L261 414 H251 Z" fill="#010418"/>
  <path d="M256 422 V446" stroke="#010418" stroke-width="4" stroke-dasharray="5 4"/>
  </g>
</svg>
"""


def pequeno(svg, corte):
    """Versao de 16-48 px: sem o brilho de fundo, recortada mais perto."""
    svg = re.sub(r'<circle cx="256" cy="\d+" r="1\d0" fill="url\(#brilho\)"[^>]*/>', "", svg)
    return svg.replace('viewBox="0 0 512 512"', f'viewBox="{corte}"')


def main():
    s = simbolo()
    (AQUI / "phxzip-simbolo.svg").write_text(s, encoding="utf-8")
    (AQUI / "phxzip-icone.svg").write_text(pequeno(s, "56 30 400 440"), encoding="utf-8")
    print("gerados: phxzip-simbolo.svg e phxzip-icone.svg")


if __name__ == "__main__":
    main()
