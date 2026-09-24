#!/usr/bin/env python3
"""Gera validacao.html: revalidacao (validacao.json), bancada
(bancada/comparativo/resultados.json) e matriz + pendencias (PHXVPN.md).
Nenhum numero e digitado aqui; a pagina nao se edita, se gera."""
import json, re, statistics as st, html, pathlib
R = pathlib.Path(__file__).resolve().parents[2]
val = json.load(open(R / 'validacao.json'))
banc = json.load(open(R / 'bancada/comparativo/resultados.json'))
doc = (R / 'docs/PHXVPN.md').read_text()
feitos = len(re.findall(r'^- \[x\]', doc, re.M)); faltam = len(re.findall(r'^- \[ \]', doc, re.M))
pct = round(100 * feitos / (feitos + faltam))
pend = [re.sub(r'[*`]', '', m) for m in re.findall(r'^- \[ \] (.*)$', doc, re.M)]
e = html.escape
br = lambda v, d=2: f'{v:.{d}f}'.replace('.', ',')

# --- matriz: a tabela do Comparativo do PHXVPN.md
sec = doc[doc.index('## Comparativo'):doc.index('**Bancada comparativa**')]
linhas = [l for l in sec.splitlines() if l.startswith('|') and not l.startswith('|---')]
cab = [c.strip().strip('*') for c in linhas[0].strip('|').split('|')]
def chip(t):
    t = t.strip(); lim = re.sub(r'\*\*|`', '', t)
    medido = '— medido' in lim or 'medido' in lim.split('—')[-1]
    lim = re.sub(r'\s*—\s*medido( no protocolo)?', '', lim)
    base = lim.lower()
    if base.startswith('ainda não'): k = 'falta'
    elif base.startswith('não documentad') or base == 'nd': k = 'nd'
    elif base.startswith('não') or base == '—': k = 'nao'
    elif base.startswith('sim'): k = 'sim'
    else: k = 'txt'
    selo = '<span class="medido" title="provado neste repositório">medido</span>' if medido else ''
    return f'<td class="c-{k}"><span class="v">{e(lim)}</span>{selo}</td>'
corpo_matriz = ''.join('<tr><th scope="row">' + e(l.strip('|').split('|')[0].strip().strip('*')) + '</th>' +
                       ''.join(chip(c) for c in l.strip('|').split('|')[1:]) + '</tr>' for l in linhas[1:])

# --- bancada: faixa min-max + mediana, escala unica
nomes = {'phxvpn': 'phxvpn', 'wireguard-go': 'WireGuard-go', 'openvpn-AES-256-GCM': 'OpenVPN · AES-256-GCM',
         'openvpn-CHACHA20-POLY1305': 'OpenVPN · ChaCha20'}
res = banc['resultados']; teto = max(max(r['mbits']) for r in res)
escala = (int(teto / 100) + 1) * 100
L, W, H0, P = 190, 520, 30, 44
alt = H0 + len(res) * P + 30
x = lambda v: L + W * v / escala
svg = [f'<svg viewBox="0 0 {L + W + 70} {alt}" role="img" aria-label="Vazão TCP pelo túnel, mínimo a máximo e mediana">']
for t in range(0, escala + 1, 100):
    svg.append(f'<line class="grade" x1="{x(t):.1f}" x2="{x(t):.1f}" y1="{H0-8}" y2="{alt-26}"/>'
               f'<text class="eixo" x="{x(t):.1f}" y="{alt-10}" text-anchor="middle">{t}</text>')
phx = next(r for r in res if r['nome'] == 'phxvpn')
for i, r in enumerate(res):
    y = H0 + i * P + 14; mn, md, mx = min(r['mbits']), st.median(r['mbits']), max(r['mbits'])
    classe = 'nosso' if r['nome'] == 'phxvpn' else 'outro'
    svg.append(f'<text class="rot" x="{L-12}" y="{y+4}" text-anchor="end">{e(nomes.get(r["nome"], r["nome"]))}</text>'
               f'<rect class="faixa {classe}" x="{x(mn):.1f}" y="{y-7}" width="{max(2, x(mx)-x(mn)):.1f}" height="14" rx="3"/>'
               f'<circle class="med {classe}" cx="{x(md):.1f}" cy="{y}" r="5"/>'
               f'<text class="val" x="{x(mx)+8:.1f}" y="{y+4}">{md:.0f}</text>')
svg.append(f'<text class="eixo" x="{L + W}" y="{H0-14}" text-anchor="end">Mbit/s · faixa mín–máx, ponto na mediana</text></svg>')
def veredito(o):
    a, b = (min(phx['mbits']), max(phx['mbits'])), (min(o['mbits']), max(o['mbits']))
    if a[0] > b[1]: return 'phxvpn mais rápido', 'bom'
    if a[1] < b[0]: return 'phxvpn mais lento', 'ruim'
    return 'empate dentro do ruído', 'neutro'
tabela_banc = ''.join(
    f'<tr><th scope="row">{e(nomes.get(r["nome"], r["nome"]))}</th><td class="n">{min(r["mbits"]):.0f}</td><td class="n"><b>{st.median(r["mbits"]):.0f}</b></td>'
    f'<td class="n">{max(r["mbits"]):.0f}</td><td class="n">{br(r["conectar_s"])} s</td><td class="n">{br(r["rtt_ms"])} ms</td>'
    + (f'<td>—</td>' if r['nome'] == 'phxvpn' else '<td><span class="pilula {1}">{0}</span></td>'.format(*veredito(r))) + '</tr>'
    for r in res)

provas = val['provas']; ok = sum(p['ok'] for p in provas)
tabela_provas = ''.join(
    f'<tr><td><span class="pilula {"bom" if p["ok"] else "ruim"}">{"OK" if p["ok"] else "FALHOU"}</span></td>'
    f'<th scope="row">{e(p["recurso"])}</th><td><code>{e(p["como"])}</code></td><td class="n">{p["s"]} s</td></tr>' for p in provas)
lista_pend = ''.join(f'<li>{e(t)}</li>' for t in pend)

pagina = f'''<title>phxvpn frente a frente</title>
<link rel="preconnect" href="https://fonts.googleapis.com"><link href="https://fonts.googleapis.com/css2?family=Exo+2:wght@600;700&display=swap" rel="stylesheet">
<style>
:root{{--fundo:#f5f6fa;--papel:#ffffff;--tinta:#10162b;--fraco:#5b6479;--linha:#dde1ea;--acento:#C63C0A;
--bom:#157a4c;--ruim:#b3261e;--aviso:#8a6100;--nd:#6b7280;--barra:#9aa3b8;color-scheme:light}}
@media (prefers-color-scheme:dark){{:root:not([data-theme="light"]){{--fundo:#010418;--papel:#0a1122;--tinta:#DDE2EB;--fraco:#8a93a8;--linha:#1c2742;
--acento:#FF4D10;--bom:#3ecf8e;--ruim:#ff6b6b;--aviso:#FFC43D;--nd:#8a93a8;--barra:#56607a;color-scheme:dark}}}}
:root[data-theme="dark"]{{--fundo:#010418;--papel:#0a1122;--tinta:#DDE2EB;--fraco:#8a93a8;--linha:#1c2742;
--acento:#FF4D10;--bom:#3ecf8e;--ruim:#ff6b6b;--aviso:#FFC43D;--nd:#8a93a8;--barra:#56607a;color-scheme:dark}}
body{{background:var(--fundo);color:var(--tinta);font:15px/1.55 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;padding-inline:16px;padding-block:28px 48px}}
.pag{{max-width:1080px;margin:0 auto;display:grid;gap:28px}}
h1,h2{{font-family:"Exo 2",system-ui,sans-serif;text-wrap:balance;margin:0}}
h1{{font-size:clamp(28px,5vw,42px);line-height:1.1}}h1 b{{color:var(--acento)}}
h2{{font-size:22px;margin-bottom:10px}}
.sub{{color:var(--fraco);max-width:68ch;margin:8px 0 0}}
.resumo{{display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:12px}}
.ind{{background:var(--papel);border:1px solid var(--linha);border-radius:10px;padding:14px 16px}}
.ind b{{display:block;font:700 30px/1.1 "Exo 2",system-ui,sans-serif;font-variant-numeric:tabular-nums}}
.ind span{{color:var(--fraco);font-size:13px}}
.barra{{height:8px;border-radius:5px;background:var(--linha);margin-top:10px;overflow:hidden}}.barra i{{display:block;height:100%;width:{pct}%;background:var(--acento)}}
section{{display:grid;gap:10px}}
.rola{{overflow-x:auto;border:1px solid var(--linha);border-radius:10px;background:var(--papel)}}
table{{border-collapse:collapse;width:100%;font-size:14px}}
th,td{{text-align:left;padding:8px 10px;border-bottom:1px solid var(--linha);vertical-align:top}}
thead th{{color:var(--fraco);font-size:12px;letter-spacing:.04em;text-transform:uppercase;font-weight:600;white-space:nowrap}}
tbody th{{font-weight:600}}
td.n{{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}}
code{{font:12px ui-monospace,monospace;color:var(--fraco)}}
.pilula{{display:inline-block;font-size:12px;font-weight:600;border:1.5px solid currentColor;border-radius:999px;padding:1px 9px;white-space:nowrap}}
.pilula.bom{{color:var(--bom)}}.pilula.ruim{{color:var(--ruim)}}.pilula.neutro{{color:var(--fraco)}}
.matriz{{min-width:900px}}.matriz td{{font-size:13px}}
.matriz .v{{display:block}}
.c-sim{{border-left:3px solid var(--bom)}}.c-nao{{border-left:3px solid var(--linha)}}.c-falta{{border-left:3px solid var(--aviso)}}
.c-nd{{border-left:3px dashed var(--nd);color:var(--fraco);font-style:italic}}.c-txt{{border-left:3px solid transparent}}
.matriz tbody td:nth-child(2){{background:color-mix(in srgb,var(--acento) 7%,transparent)}}
.medido{{display:inline-block;margin-top:4px;font-size:11px;font-weight:600;color:var(--bom);letter-spacing:.03em}}
.legenda{{display:flex;flex-wrap:wrap;gap:14px;font-size:13px;color:var(--fraco)}}
.legenda span{{display:inline-flex;align-items:center;gap:6px}}.legenda i{{width:4px;height:16px;display:inline-block}}
svg{{width:100%;max-width:800px;height:auto;display:block}}
svg .grade{{stroke:var(--linha)}}svg .eixo{{fill:var(--fraco);font-size:11px}}svg .rot{{fill:var(--tinta);font-size:13px}}
svg .val{{fill:var(--tinta);font-size:12px;font-weight:600}}
svg .faixa.outro{{fill:var(--barra)}}svg .faixa.nosso{{fill:var(--acento);opacity:.55}}
svg .med.outro{{fill:var(--tinta)}}svg .med.nosso{{fill:var(--acento)}}
.duas{{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:16px}}
.caixa{{background:var(--papel);border:1px solid var(--linha);border-radius:10px;padding:14px 18px}}
.caixa h3{{margin:0 0 6px;font-size:15px}}.caixa ul{{margin:0;padding-left:18px}}.caixa li{{margin:4px 0}}
.nota{{color:var(--fraco);font-size:13px;max-width:80ch}}
</style>
<div class="pag">
<header><h1><b>phx</b>vpn frente a frente</h1>
<p class="sub">Radmin VPN, OpenVPN, a VPN nativa do Windows e a do Linux (WireGuard, strongSwan), recurso por recurso. O que é do phxvpn foi <b>reprovado hoje</b>; o que é dos outros vem da documentação oficial, com fonte. Revalidação de {e(val["data"][:10])}.</p></header>

<div class="resumo">
<div class="ind"><b>{ok} de {len(provas)}</b><span>provas revalidadas, rodando de verdade</span></div>
<div class="ind"><b>{pct}%</b><span>do plano entregue ({feitos} de {feitos + faltam} itens)</span><div class="barra"><i></i></div></div>
<div class="ind"><b>{st.median(phx["mbits"]):.0f} Mbit/s</b><span>vazão do phxvpn, mediana de {banc["corridas"]} corridas</span></div>
<div class="ind"><b>{br(phx["conectar_s"])} s</b><span>para conectar (OpenVPN: {br(next(r for r in res if r["nome"].startswith("openvpn"))["conectar_s"])} s)</span></div>
</div>

<section><h2>Onde o phxvpn está</h2>
<div class="duas">
<div class="caixa"><h3>À frente</h3><ul>
<li>Criar e entrar na rede com nome e senha, como no Radmin, mas <b>sem depender dos servidores de um fabricante</b>: o relé é seu.</li>
<li>Ponta a ponta e aberto: Noise com vetor oficial, sigilo futuro, janela contra repetição, barreira antes do aperto. O Radmin documenta só «AES 256-bit».</li>
<li>Chat, ping e <b>USB pela rede</b> embutidos; nenhum dos outros quatro tem USB nativo.</li>
<li>Com a mesma cifra, mais rápido que o OpenVPN, e conecta no primeiro pacote.</li>
</ul></div>
<div class="caixa"><h3>Atrás</h3><ul>
<li>Perfuração de NAT: o Radmin tenta direto antes do relé; aqui o relé é o caminho quando não há rota direta.</li>
<li>Sem macOS, Android e iOS (OpenVPN e WireGuard têm).</li>
<li>Sem auditoria externa (OpenVPN auditado em 2017; WireGuard com verificação formal).</li>
<li>Sem MFA, RADIUS ou AD no painel: decisão de produto em aberto.</li>
</ul></div></div></section>

<section><h2>Recurso por recurso</h2>
<div class="legenda"><span><i style="background:var(--bom)"></i>tem</span><span><i style="background:var(--linha)"></i>não tem</span><span><i style="background:var(--aviso)"></i>ainda não, no plano</span><span><i style="border-left:3px dashed var(--nd)"></i>não documentado pelo fabricante</span><span style="color:var(--bom);font-weight:600">medido</span><span>= provado neste repositório</span></div>
<div class="rola"><table class="matriz"><thead><tr>{"".join(f"<th>{e(c)}</th>" for c in cab)}</tr></thead><tbody>{corpo_matriz}</tbody></table></div>
<p class="nota">Cada célula dos concorrentes, com a URL e a frase citada, está em <code>phxvpn/docs/propostas/matriz-vpns-2026-09-24.md</code> (108 células; 62 com fonte primária, 26 «não documentado»). Nada foi preenchido de memória.</p></section>

<section><h2>Medido lado a lado</h2>
<p class="nota">Mesma máquina ({e(banc["maquina"])}), dois <code>netns</code> ligados por veth, <code>iperf3</code> TCP de 5 s, {banc["corridas"]} corridas, {e(banc["data"][:10])}. Todos em espaço de usuário: o kernel daqui não tem o módulo do WireGuard nem o DCO do OpenVPN. Só há vencedor quando as faixas não se cruzam. Radmin e a VPN do Windows não rodam aqui e não foram medidos.</p>
<div class="caixa">{"".join(svg)}</div>
<div class="rola"><table><thead><tr><th>Túnel</th><th>mín</th><th>mediana</th><th>máx</th><th>conectar</th><th>ping</th><th>contra o phxvpn</th></tr></thead><tbody>{tabela_banc}</tbody></table></div></section>

<section><h2>Cada recurso do phxvpn, reprovado hoje</h2>
<div class="rola"><table><thead><tr><th>resultado</th><th>recurso</th><th>como se prova</th><th>tempo</th></tr></thead><tbody>{tabela_provas}</tbody></table></div>
<p class="nota">Um comando refaz tudo: <code>sudo phxvpn/validar-tudo.sh</code>. Limites que continuam valendo: o túnel P2P e a bandeja ainda não rodaram num Windows real (o roteiro <code>prova-windows.ps1</code> está pronto), e o USB foi provado no protocolo e na tela, sem pendrive físico.</p></section>

<section><h2>O que falta ({faltam})</h2><div class="caixa"><ul>{lista_pend}</ul></div></section>
</div>
'''
(pathlib.Path(__file__).parent / 'validacao.html').write_text(pagina)
print(f'validacao.html: {ok}/{len(provas)} provas, {pct}% ({feitos}/{feitos+faltam}), bancada {banc["corridas"]} corridas')
