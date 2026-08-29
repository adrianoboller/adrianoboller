#!/usr/bin/env python3
"""Monta a bancada de exercicio do painel de bolhas da telemetria.

    python3 bancada/telemetria/monta-bancada.py
    # abre bancada/telemetria/bancada.html no navegador

Por que ela existe: o modulo da telemetria NAO fala com o servidor -- ele
recebe uma funcao `api(op, params)` de quem o chama. Entao da para exercitar o
desenho com retratos inventados (1, 3, 8, 12, 40 atividades, pesos parelhos,
uma dominante, nenhuma) sem subir servidor nenhum e sem carga.

O CSS global sai do proprio `index.html`, e nao de uma copia: copia envelhece
calada, e a graca da bancada e justamente provar o escopo `.tlm` contra as
regras que existem HOJE -- o `input{width:100%}` e o
`label{text-transform:uppercase}` que mordem todo componente novo.
"""
import pathlib

AQUI = pathlib.Path(__file__).resolve().parent
UI = AQUI.parents[1] / "crates" / "phxsql-server" / "ui"
SAIDA = AQUI / "bancada.html"

idx = (UI / "index.html").read_text(encoding="utf-8")
css_global = idx.split("<style>", 1)[1].split("</style>", 1)[0]
css_tlm = (UI / "telemetria.css").read_text(encoding="utf-8")
js_tlm = (UI / "telemetria.js").read_text(encoding="utf-8")

SAIDA.write_text(f"""<!doctype html>
<html lang="pt-BR"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>bancada da telemetria</title>
<style>{css_global}</style>
<style>{css_tlm}</style>
<style>
  body{{overflow:auto}}
  #painel{{padding:14px}}
  .bancada-barra{{display:flex;gap:8px;flex-wrap:wrap;padding:8px 14px;
    border-bottom:1px solid var(--linha);position:sticky;top:0;
    background:var(--fundo);z-index:9}}
  .bancada-barra button{{background:transparent;border:1px solid var(--linha-forte);
    color:var(--texto-2);border-radius:5px;padding:5px 11px;font-size:12px}}
  .bancada-barra button.on{{border-color:var(--laranja);color:var(--texto)}}
</style>
</head><body>
<div class="bancada-barra" id="barra"></div>
<div id="painel"></div>
<script>{js_tlm}</script>
<script src="retrato.js"></script>
</body></html>
""", encoding="utf-8")
print("ok", SAIDA)
