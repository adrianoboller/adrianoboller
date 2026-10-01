#!/usr/bin/env python3
"""Pagina do comparativo: PhxClaw x quatro agentes, PhxSql x MySQL(R) x PostgreSQL(R).

    python3 docs/comparativo/gerar_comparativo.py

Grava `docs/comparativo/comparativo.html`. Nada de numero digitado:
- a coluna do PhxClaw sai do codigo (ferramentas registradas na montagem do agente,
  provedores do `phxclaw-llm`, o que o agente e a CLI importam) e da certificacao;
- as capacidades do PhxSql saem de `phxsql/bancada/comparativo/resultados.json`;
- o desempenho, de `phxsql/bancada/comparacao/um-milhao-quatro.json`, com a faixa
  min-max em cada barra e o vencedor contornado SO quando as faixas nao se cruzam
  (pedido 155). Sem o arquivo, a secao diz NAO MEDIDO em vez de sumir.

A coluna dos outros quatro agentes e PESQUISA, nao medida: cada celula traz a fonte e a
data da leitura (01/10/2026). Celula que a fonte nao respondeu diz «não encontrado».
"""
from __future__ import annotations

import html
import json
import re
import subprocess
from datetime import datetime, timezone
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
REPO = RAIZ.parent
SAIDA = RAIZ / "docs/comparativo/comparativo.html"
CERT = RAIZ / "reports/RELEASE_CERTIFICATION_v0.70.json"
CAPAC = REPO / "phxsql/bancada/comparativo/resultados.json"
DESEMP = REPO / "phxsql/bancada/comparacao/um-milhao-quatro.json"
LIDO_EM = "01/10/2026"


def e(s) -> str:
    return html.escape(str(s))


def mil(n) -> str:
    return f"{int(n):,}".replace(",", ".")


# ---------------------------------------------------------------- PhxClaw, medido


def phxclaw_medido() -> dict:
    agente = RAIZ / "crates/phxclaw-agent/src"
    ferramentas = len(re.findall(r"impl Tool for \w+",
                                 "".join(p.read_text() for p in agente.glob("*.rs"))))
    llm = (RAIZ / "crates/phxclaw-llm/src/lib.rs").read_text()
    provedores = re.findall(r'^\s*"(\w+)" =>', llm, re.M)
    deps = set()
    for toml in (RAIZ / "crates/phxclaw-agent/Cargo.toml", RAIZ / "apps/phxclaw/Cargo.toml"):
        deps |= set(re.findall(r"^(phxclaw-[\w-]+)\s*=", toml.read_text(), re.M))
    linhas_rs = int(subprocess.run(
        "git ls-files '*.rs' | xargs cat | wc -l", shell=True, cwd=RAIZ,
        capture_output=True, text=True).stdout.strip() or 0)
    crates = len([d for d in (RAIZ / "crates").iterdir() if (d / "Cargo.toml").exists()])
    cert = json.loads(CERT.read_text())
    return {"ferramentas": ferramentas, "provedores": provedores, "deps": deps,
            "linhas_rs": linhas_rs, "crates": crates, "cert": cert}


def ligado(m: dict, crate: str) -> bool:
    """O agente ou a CLI importam o crate? Crate que existe e ninguem importa e
    biblioteca, nao recurso do agente."""
    return crate in m["deps"]


# ------------------------------------------------- agentes: (estado, texto, fonte)
# estado: sim | parcial | nao | nd (não encontrado)

H, CC, GM, CX = "hermes", "claude", "gemini", "codex"
F_H = "https://hermes-agent.nousresearch.com/docs/"
F_HR = "https://github.com/NousResearch/hermes-agent"
F_HS = "https://github.com/NousResearch/hermes-agent/blob/main/SECURITY.md"
F_C = "https://code.claude.com/docs/en/overview"
F_CS = "https://code.claude.com/docs/en/sandboxing"
F_CL = "https://raw.githubusercontent.com/anthropics/claude-code/main/LICENSE.md"
F_G = "https://github.com/google-gemini/gemini-cli"
F_GD = "https://github.com/google-gemini/gemini-cli/discussions/27274"
F_X = "https://learn.chatgpt.com/docs"
F_XS = "https://learn.chatgpt.com/docs/sandboxing"
F_XP = "https://learn.chatgpt.com/docs/pricing"


def linhas_agentes(m: dict) -> list[tuple[str, dict]]:
    marca = {"openai": "OpenAI", "ollama": "Ollama", "anthropic": "Anthropic", "gemini": "Gemini"}
    prov = ", ".join(marca.get(p, p) for p in m["provedores"])
    lib = lambda c, t: ("sim", t) if ligado(m, c) else (
        "parcial", t + " Existe como biblioteca testada, mas o agente ainda não a usa.")
    return [
        ("Licença", {
            "phx": ("sim", "Apache-2.0, código aberto."),
            H: ("sim", "MIT, código aberto.", F_HR),
            CC: ("nao", "Proprietário: «All rights reserved», uso sob termos comerciais.", F_CL),
            GM: ("parcial", "Gemini CLI é Apache-2.0; o sucessor Antigravity CLI é fechado.", F_GD),
            CX: ("sim", "Apache-2.0, código aberto.", F_X),
        }),
        ("Linguagem", {
            "phx": ("sim", f"Rust: {mil(m['linhas_rs'])} linhas em {m['crates']} crates."),
            H: ("sim", "Python, com interface em Node.js.", F_HR),
            CC: ("sim", "TypeScript/Node.js; instalador nativo binário.", F_C),
            GM: ("sim", "TypeScript (Gemini CLI); Go (Antigravity).", F_GD),
            CX: ("sim", "Rust e TypeScript (pela estrutura do repositório).", "https://github.com/openai/codex"),
        }),
        ("Modelos e modelo local", {
            "phx": ("sim", f"{len(m['provedores'])} provedores no agente: {prov}. Local pelo Ollama, provado na certificação."),
            H: ("sim", "Vários provedores e qualquer endpoint; Ollama local suportado.", F_H),
            CC: ("parcial", "Só modelos Claude (direto, Bedrock, Vertex, Foundry). Modelo local não documentado.", "https://code.claude.com/docs/en/third-party-integrations"),
            GM: ("parcial", "Só Gemini. Modelo local não encontrado.", F_G),
            CX: ("sim", "Modelos da OpenAI; local com --oss (Ollama ou LM Studio).", "https://learn.chatgpt.com/docs/developer-commands?surface=cli"),
        }),
        ("Onde roda", {
            "phx": ("parcial", "CLI `phxclaw`, API HTTP (`phxclaw servir`) e desktop Tauri. Alvo da versão: Linux."),
            H: ("sim", "Terminal, gateway de mensagens (celular) e desktop.", F_HR),
            CC: ("sim", "Terminal, IDEs, desktop, web, celular e Slack.", F_C),
            GM: ("parcial", "Terminal e VS Code; Antigravity tem IDE e desktop.", F_GD),
            CX: ("sim", "CLI, IDEs, desktop, web, celular e nuvem.", F_X),
        }),
        ("Isolamento do shell", {
            "phx": ("sim", "bubblewrap, sem rede por padrão. Sem bubblewrap o agente não recebe shell: falha fechado."),
            H: ("parcial", "Portão de aprovação que o próprio SECURITY.md chama de heurística; isolamento real só envolvendo em Docker.", F_HS),
            CC: ("parcial", "Sandbox opcional (Seatbelt, bubblewrap). Sem as dependências, avisa e roda sem sandbox.", F_CS),
            GM: ("nd", "«Trusted Folders»; mecanismo de isolamento não encontrado.", F_G),
            CX: ("sim", "Seatbelt ou bubblewrap; padrão workspace-write com aprovação para rede.", F_XS),
        }),
        ("MCP", {
            "phx": lib("phxclaw-mcp-lsp-runtime", "Runtime MCP/LSP em crate próprio."),
            H: ("sim", "Cliente e servidor.", F_H),
            CC: ("sim", "Cliente e servidor.", "https://code.claude.com/docs/en/mcp"),
            GM: ("sim", "Cliente.", F_G),
            CX: ("sim", "Cliente.", "https://learn.chatgpt.com/docs/extend/mcp?surface=cli"),
        }),
        ("Memória e skills", {
            "phx": lib("phxclaw-memory-context", "Memória de contexto e runtime de skills em crates próprios."),
            H: ("sim", "Memória curada e skills criadas pelo próprio agente.", F_H),
            CC: ("sim", "CLAUDE.md, memória automática, skills e hooks.", F_C),
            GM: ("parcial", "GEMINI.md por projeto.", F_G),
            CX: ("sim", "AGENTS.md, memórias locais, skills e hooks.", "https://learn.chatgpt.com/docs/customization/overview"),
        }),
        ("Subagentes", {
            "phx": ("sim", "Agentes em paralelo como ferramenta do agente."),
            H: ("sim", "Subagentes isolados com controle ao vivo.", "https://github.com/NousResearch/hermes-agent/releases"),
            CC: ("sim", "Subagentes, agentes em segundo plano e Agent SDK.", F_C),
            GM: ("parcial", "Não encontrado no Gemini CLI; o Antigravity tem.", F_GD),
            CX: ("sim", "Subagentes e tarefas paralelas na nuvem.", F_X),
        }),
        ("Agendamento", {
            "phx": ("sim", "Agenda com cron no agente, provada no fio."),
            H: ("sim", "Cron embutido, entrega em qualquer canal.", F_HR),
            CC: ("sim", "Routines na nuvem e tarefas agendadas.", F_C),
            GM: ("nd", "Não encontrado.", F_G),
            CX: ("sim", "Automações e tarefas agendadas.", F_X),
        }),
        ("Canais de mensagem", {
            "phx": lib("phxclaw-channel-providers", "Telegram, Discord, Slack, WhatsApp e Teams com segredo pelo cofre; o E2E real do Telegram espera o token."),
            H: ("sim", "Mais de 20 plataformas por um gateway.", F_H),
            CC: ("parcial", "Slack, e eventos de Telegram e Discord numa sessão.", F_C),
            GM: ("nao", "Não encontrado.", F_G),
            CX: ("parcial", "Slack, GitLab e Linear.", F_X),
        }),
        ("Navegador", {
            "phx": ("sim", "Chromium real controlado pelo agente, com política de origem."),
            H: ("sim", "Navegador em nuvem (Browser Use).", F_H),
            CC: ("sim", "Integração com o Chrome.", F_C),
            GM: ("parcial", "Chrome no Antigravity.", "https://antigravity.google/docs/getting-started"),
            CX: ("nd", "Não detalhado nas páginas lidas.", F_X),
        }),
        ("Automação de desktop", {
            "phx": lib("phxclaw-system-automation", "Teclado, mouse, captura e shell governado: 4/4 no Xvfb e no Wine; falta a máquina física."),
            H: ("sim", "Computer use em macOS, Windows e Linux.", "https://hermes-agent.nousresearch.com/docs/user-guide/features/computer-use"),
            CC: ("parcial", "Computer use no CLI só em macOS, em prévia, planos Pro/Max.", "https://code.claude.com/docs/en/computer-use"),
            GM: ("nd", "Não encontrado.", F_GD),
            CX: ("nd", "Listado, sem detalhe.", F_X),
        }),
        ("Voz", {
            "phx": lib("phxclaw-media-intelligence", "Fala para texto LOCAL (whisper.cpp), modelo conferido por SHA-256; portão da certificação passou."),
            H: ("sim", "Fala e texto para fala, em tempo real.", F_H),
            CC: ("parcial", "Ditado; o áudio vai para a nuvem da Anthropic.", "https://code.claude.com/docs/en/voice-dictation"),
            GM: ("nd", "Não encontrado.", F_G),
            CX: ("nd", "Listado, sem detalhe.", F_X),
        }),
        ("Plugins assinados", {
            "phx": ("sim", "Ed25519 por raiz própria; a assinatura V2 cobre o manifesto inteiro (permissões, rede, entrypoint)."),
            H: ("nao", "Só varredura estática na instalação.", F_H),
            CC: ("nao", "Commit fixado por SHA no catálogo; assinatura criptográfica não encontrada.", "https://code.claude.com/docs/en/plugins/security"),
            GM: ("nao", "Não encontrada.", F_GD),
            CX: ("nao", "Não encontrada.", "https://learn.chatgpt.com/docs/customization/overview"),
        }),
        ("GitHub", {
            "phx": ("nao", "Não há integração com GitHub no agente."),
            H: ("nd", "Não encontrado.", F_HR),
            CC: ("sim", "@claude em PRs, Actions e revisão automática.", F_C),
            GM: ("sim", "GitHub Action oficial.", F_G),
            CX: ("sim", "Revisão de código e Actions.", F_X),
        }),
        ("Sem nuvem", {
            "phx": ("sim", "Modelo, voz e banco locais (Ollama, whisper.cpp, PostgreSQL)."),
            H: ("parcial", "Possível com modelo local; não afirmado pela fonte.", F_HR),
            CC: ("nao", "Exige a API da Anthropic ou de uma nuvem.", F_C),
            GM: ("nao", "Exige a API Gemini.", F_G),
            CX: ("parcial", "Com --oss e modelo local; o padrão exige conta.", F_XP),
        }),
        ("Preço", {
            "phx": ("sim", "Gratuito; paga-se só o modelo de nuvem, se usado."),
            H: ("sim", "Gratuito; paga-se o modelo.", F_HR),
            CC: ("parcial", "Planos Claude a partir de US$ 17/mês (anual); Free não inclui.", "https://claude.com/pricing"),
            GM: ("parcial", "Descontinuado para consumidor em 18/06/2026; segue no Enterprise.", F_GD),
            CX: ("sim", "Incluído do ChatGPT Free ao Pro, com limites por janela de 5 h.", F_XP),
        }),
    ]


# ---------------------------------------------------------------- PhxSql


def tabela_capacidades() -> tuple[str, str]:
    if not CAPAC.exists():
        return "<p class='nm'>NÃO MEDIDO: falta phxsql/bancada/comparativo/resultados.json.</p>", ""
    d = json.loads(CAPAC.read_text())
    motores = [("phxsql", "PhxSql"), ("postgres", "PostgreSQL®"), ("mysql", "MySQL®")]
    simb = {"tem": ("sim", "tem"), "nao": ("nao", "não tem"), "meio": ("parcial", "pela metade"),
            "citado": ("nd", "só na doc")}
    contas = {k: 0 for k, _ in motores}
    corpo = []
    for l in d["linhas"]:
        tds = []
        for k, _ in motores:
            st = (l.get(k) or ["nd", ""])[0]
            cls, rot = simb.get(st, ("nd", st))
            if st == "tem":
                contas[k] += 1
            tds.append(f"<td><span class='pl pl-{cls}'>{e(rot)}</span></td>")
        corpo.append(f"<tr><th scope='row'>{md(l['titulo'])}</th><td class='mot'>{e(l['como'])}</td>{''.join(tds)}</tr>")
    cab = "".join(f"<th>{n}<br><span class='mot'>{e(d['motores_vivos'].get(k, ''))[:24]}</span></th>" for k, n in motores)
    quando = datetime.fromisoformat(d["quando"]).strftime("%d/%m/%Y")
    resumo = " · ".join(f"{n} <b>{contas[k]}</b>/{len(d['linhas'])}" for k, n in motores)
    return (f"<div class='rolo'><table><thead><tr><th>capacidade</th><th>como se decidiu</th>{cab}</tr></thead>"
            f"<tbody>{''.join(corpo)}</tbody></table></div>"), f"{resumo} · medido em {quando}"


def md(s: str) -> str:
    return re.sub(r"`([^`]+)`", r"<code>\1</code>", e(s))


NOMES = {"phxsql": "PhxSql", "postgresql": "PostgreSQL®", "mysql": "MySQL®", "sqlite": "SQLite®"}
FASES = [("inserir", "Inserir 1.000.000 linhas"), ("buscar", "Buscar 20.000 por chave"),
         ("atualizar", "Atualizar 20.000"), ("excluir", "Excluir 20.000")]


def graficos() -> tuple[str, str]:
    if not DESEMP.exists():
        return ("<p class='nm'>NÃO MEDIDO: rode <code>PHX_SAIDA=um-milhao-quatro.json python3 "
                "bancada/comparacao/medir.py --so phxsql,sqlite,mysql,postgresql</code>.</p>", "")
    d = json.loads(DESEMP.read_text())
    ordem = [m for m in ("phxsql", "postgresql", "mysql", "sqlite") if m in d["motores_medidos"]]
    blocos = []
    for f, titulo in FASES:
        dados = {m: d["fases"][f][m] for m in ordem if d["fases"][f][m]["mediana_s"] is not None}
        teto = max(v["max_s"] for v in dados.values()) * 1.18
        venc = min(dados, key=lambda m: dados[m]["mediana_s"])
        sozinho = all(dados[venc]["max_s"] < dados[o]["min_s"] for o in dados if o != venc)
        alt, y0, h = 30 * len(dados) + 10, 6, 18
        svg = [f"<svg viewBox='0 0 600 {alt}' role='img' aria-label='{e(titulo)}' preserveAspectRatio='none'>"]
        for i, m in enumerate(dados):
            v = dados[m]
            y = y0 + i * 30
            x = lambda s: 130 + 400 * s / teto
            cls = "b-phx" if m == "phxsql" else "b-out"
            borda = " class='venc'" if (m == venc and sozinho) else ""
            svg.append(f"<text x='0' y='{y + 13}' class='rot'>{NOMES[m]}</text>")
            svg.append(f"<rect x='130' y='{y}' width='{x(v['mediana_s']) - 130:.1f}' height='{h}' class='{cls}'/>")
            if borda:
                svg.append(f"<rect x='130' y='{y - 2}' width='{x(v['mediana_s']) - 130:.1f}' height='{h + 4}' class='venc'/>")
            svg.append(f"<line x1='{x(v['min_s']):.1f}' x2='{x(v['max_s']):.1f}' y1='{y + h / 2}' y2='{y + h / 2}' class='bigode'/>")
            svg.append(f"<text x='{x(v['max_s']) + 6:.1f}' y='{y + 13}' class='val'>{v['mediana_s']:.3f} s</text>")
        svg.append("</svg>")
        nota = (f"{NOMES[venc]} vence sem cruzar faixas." if sozinho
                else f"{NOMES[venc]} tem a menor mediana, mas as faixas se cruzam: sem vencedor declarado.")
        blocos.append(f"<figure class='fase'><figcaption>{titulo}</figcaption>{''.join(svg)}"
                      f"<p class='mot'>{e(nota)}</p></figure>")
    ress = "".join(f"<li>{md(r)}</li>" for r in d.get("ressalvas", []))
    dur = "".join(f"<li><b>{e(k)}</b>: {e(v)}</li>" for k, v in d.get("durabilidade", {}).items())
    conf = d["trabalho_conferido"]["marcos_por_motor"]
    um = next(iter(conf.values()))
    rodape = (f"{d['rodadas']} rodadas, medido em {e(d['medido_em'])}. Trabalho conferido: os "
              f"{len(conf)} motores saíram de cada etapa com a mesma contagem e as mesmas somas "
              f"({mil(um[-1][0])} linhas no fim).")
    disco = d.get("disco_bytes", {})
    disco_txt = " · ".join(f"{NOMES[m]} {disco[m] / 1e6:.0f} MB" for m in ordem if m in disco)
    return ("<div class='fases'>" + "".join(blocos) + "</div>"
            f"<p class='mot'>{rodape}</p>"
            f"<p><b>Disco ao fim da carga:</b> {e(disco_txt)}.</p>"
            f"<details><summary>Durabilidade de cada motor, lida do servidor</summary><ul>{dur}</ul></details>"
            f"<details><summary>O que estes números não dizem</summary><ul>{ress}</ul></details>"), d["medido_em"]


# ---------------------------------------------------------------- página


def celula(v) -> str:
    st, txt = v[0], v[1]
    fonte = f" <a href='{e(v[2])}' class='fonte'>fonte</a>" if len(v) > 2 else ""
    rot = {"sim": "sim", "parcial": "parcial", "nao": "não", "nd": "não encontrado"}[st]
    return f"<td><span class='pl pl-{st}'>{rot}</span> {e(txt)}{fonte}</td>"


def main() -> int:
    m = phxclaw_medido()
    cert = m["cert"]
    agentes = [("phx", "PhxClaw"), (H, "Hermes Agent"), (CC, "Claude Code"),
               (GM, "Gemini CLI / Antigravity"), (CX, "Codex")]
    linhas = linhas_agentes(m)
    corpo = "".join(
        f"<tr><th scope='row'>{e(dim)}</th>{''.join(celula(c[k]) for k, _ in agentes)}</tr>"
        for dim, c in linhas)
    placar = {k: sum(1 for _, c in linhas if c[k][0] == "sim") for k, _ in agentes}
    soltos = [dim for dim, c in linhas if c["phx"][0] == "parcial"
              and "biblioteca testada" in c["phx"][1]]
    cab = "".join(f"<th>{n}<br><span class='mot'>{placar[k]} de {len(linhas)} «sim»</span></th>"
                  for k, n in agentes)
    tab_cap, resumo_cap = tabela_capacidades()
    graf, quando_desemp = graficos()
    agora = datetime.now(timezone.utc).strftime("%d/%m/%Y %H:%M UTC")
    pagina = f"""<title>Comparativo Phoenix</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@500;700&family=Source+Sans+3:wght@400;600&family=JetBrains+Mono:wght@400&display=swap">
<style>
/* Duas bancas lado a lado: a tabela larga rola dentro do proprio quadro; o resto e coluna de leitura. */
:root {{
  --fundo:#f3f4f8; --papel:#ffffff; --tinta:#161a2e; --suave:#5a6078; --linha:#d9dce8;
  --marca:#C63C0A; --ok:#1c7a45; --ruim:#b3261e; --trava:#8a5a00; --outro:#7c84a6;
  --disp:"Exo 2", "Segoe UI", sans-serif; --corpo:"Source Sans 3", "Segoe UI", sans-serif;
  --mono:"JetBrains Mono", ui-monospace, monospace;
}}
@media (prefers-color-scheme: dark) {{ :root:not([data-theme="light"]) {{
  --fundo:#010418; --papel:#0b1030; --tinta:#e8eaf5; --suave:#9aa0bd; --linha:#232a52;
  --marca:#ff6a2b; --ok:#4fd08a; --ruim:#ff7b72; --trava:#f0b44c; --outro:#5d6690; color-scheme:dark; }} }}
:root[data-theme="dark"] {{
  --fundo:#010418; --papel:#0b1030; --tinta:#e8eaf5; --suave:#9aa0bd; --linha:#232a52;
  --marca:#ff6a2b; --ok:#4fd08a; --ruim:#ff7b72; --trava:#f0b44c; --outro:#5d6690; color-scheme:dark; }}
body {{ background:var(--fundo); color:var(--tinta); font:400 1rem/1.55 var(--corpo); }}
main {{ max-width:1180px; margin:0 auto; padding-inline:16px; padding-block:32px 64px; display:grid; gap:40px; }}
h1,h2 {{ font-family:var(--disp); font-weight:700; text-wrap:balance; margin:0; }}
h1 {{ font-size:clamp(1.8rem,4vw,2.6rem); }}
h1 span {{ color:var(--marca); }}
h2 {{ font-size:1.45rem; }}
p {{ margin:0; max-width:72ch; }}
section {{ display:grid; gap:16px; min-width:0; }}
.olho {{ font:600 .74rem var(--mono); letter-spacing:.08em; text-transform:uppercase; color:var(--suave); }}
.rolo {{ overflow-x:auto; border:1px solid var(--linha); border-radius:8px; background:var(--papel); }}
table {{ border-collapse:collapse; width:100%; font-size:.9rem; }}
th,td {{ text-align:left; vertical-align:top; padding:10px 12px; border-bottom:1px solid var(--linha); }}
thead th {{ font:600 .78rem var(--disp); letter-spacing:.03em; position:sticky; top:0; background:var(--papel); }}
tbody th {{ font:600 .9rem var(--corpo); white-space:nowrap; }}
.ag td {{ min-width:190px; }}
.ag td:nth-child(2), .ag thead th:nth-child(2) {{ background:color-mix(in srgb, var(--marca) 7%, var(--papel)); }}
tr:last-child td, tr:last-child th {{ border-bottom:0; }}
.mot {{ color:var(--suave); font-size:.84rem; }}
.pl {{ font:600 .7rem var(--mono); padding:1px 6px; border-radius:3px; border:1px solid; white-space:nowrap; }}
.pl-sim {{ color:var(--ok); }} .pl-nao {{ color:var(--ruim); }}
.pl-parcial {{ color:var(--trava); border-style:dashed; }} .pl-nd {{ color:var(--suave); border-style:dotted; }}
a {{ color:var(--marca); }} a.fonte {{ font-size:.76rem; }}
code {{ font-family:var(--mono); font-size:.86em; }}
.aviso {{ border-left:3px solid var(--marca); padding:10px 14px; background:var(--papel); border-radius:0 6px 6px 0; max-width:none; }}
.fases {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(min(100%,440px),1fr)); gap:16px; }}
.fase {{ margin:0; background:var(--papel); border:1px solid var(--linha); border-radius:8px; padding:14px; display:grid; gap:8px; min-width:0; }}
.fase figcaption {{ font:700 .95rem var(--disp); }}
.fase svg {{ width:100%; height:auto; }}
.rot {{ font:600 13px var(--corpo); fill:var(--tinta); }}
.val {{ font:12px var(--mono); fill:var(--suave); }}
.b-phx {{ fill:var(--marca); }} .b-out {{ fill:var(--outro); }}
.venc {{ fill:none; stroke:var(--tinta); stroke-width:1.5; }}
.bigode {{ stroke:var(--tinta); stroke-width:1.5; }}
.nm {{ color:var(--trava); font-weight:600; }}
details {{ max-width:90ch; }} summary {{ cursor:pointer; font-weight:600; }}
footer {{ color:var(--suave); font-size:.82rem; }}
</style>
<main>
<header style="display:grid;gap:10px">
  <p class="olho">Phoenix · comparativo de {e(agora)}</p>
  <h1>PhxClaw e PhxSql <span>frente a quem já está no mercado</span></h1>
  <p>Duas comparações. O PhxClaw é um agente e se compara com agentes. O PhxSql é um banco e se compara com bancos.
  O PhxClaw usa o PostgreSQL como banco, então não concorre com ele.</p>
</header>

<section id="agentes">
  <p class="olho">Parte 1 · agentes</p>
  <h2>PhxClaw × Hermes Agent × Claude Code × Gemini CLI × Codex</h2>
  <p>A coluna do PhxClaw sai do código e da certificação (alvo {e(cert.get('target', 'todos'))},
  {cert['required_passed']}/{cert['required_total']} portões obrigatórios, {e(cert['certified_at'][:10])}).
  As outras quatro saem da documentação oficial de cada produto, lida em {LIDO_EM}, com o link em cada célula.</p>
  <p class="aviso"><b>O ponto fraco do PhxClaw:</b> {e(', '.join(soltos))} existem como bibliotecas testadas,
  mas o agente e a CLI ainda não as usam. Hermes, Claude Code e Codex trazem esses recursos dentro do agente.
  <b>Os pontos fortes:</b> é o único com plugins de assinatura criptográfica que cobre o manifesto inteiro, o único
  cujo shell falha fechado sem isolamento, e funciona sem nuvem (modelo, voz e banco locais).</p>
  <div class="rolo ag"><table><thead><tr><th>dimensão</th>{cab}</tr></thead><tbody>{corpo}</tbody></table></div>
  <p class="mot">«sim» conta só o que existe e está ligado ao produto; «parcial» inclui o que existe pela metade
  ou fora do agente; «não encontrado» é o que a fonte lida não respondeu, e não prova ausência.</p>
</section>

<section id="bancos">
  <p class="olho">Parte 2 · bancos</p>
  <h2>PhxSql × PostgreSQL® × MySQL®</h2>
  <p>Capacidades perguntadas ao motor vivo de cada um, com um gêmeo que tem de ser recusado para a resposta valer.
  {resumo_cap}.</p>
  {tab_cap}
  <h2>Desempenho, um milhão de linhas</h2>
  <p>O mesmo trabalho nos quatro motores, intercalados na mesma rodada e na mesma máquina. A bancada recusa publicar
  se algum terminar uma etapa com contagem ou soma diferente. Cada barra é a mediana; o traço vai do mínimo ao máximo.</p>
  {graf}
</section>

<footer>Gerado por <code>docs/comparativo/gerar_comparativo.py</code> em {e(agora)}. Esta página não se edita:
os números saem do código, da certificação e dos <code>resultados.json</code> das bancadas.
PostgreSQL®, MySQL® e SQLite® são marcas de seus donos.</footer>
</main>
"""
    SAIDA.parent.mkdir(parents=True, exist_ok=True)
    SAIDA.write_text(pagina)
    print(f"gravado {SAIDA.relative_to(RAIZ)}; PhxClaw {placar['phx']} «sim», "
          f"{len(soltos)} recurso(s) fora do agente; desempenho: {quando_desemp or 'NAO MEDIDO'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
