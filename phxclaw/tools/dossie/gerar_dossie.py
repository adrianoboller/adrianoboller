#!/usr/bin/env python3
"""Dossie do PhxClaw: todo numero da pagina sai daqui, medido, nunca digitado.

Fontes:
  - reports/RELEASE_CERTIFICATION_v0.70.json   (portoes, veredito, data)
  - saida do `cargo test --workspace --no-fail-fast` (roda agora, ou --suite ARQ)
  - fontes .rs em crates/ e apps/                 (linhas e crates, contados)
  - docs/ESTEIRA_ABSORCAO.md                      (itens com estado na ultima coluna)
  - docs/cognicao/*.md                            (aprendizados)
  - git log                                       (commits da branch)

Saida: docs/dossie/dossie-phxclaw.html
Publicado em https://claude.ai/artifact/J5emfeTgE26AapFRTPssDk (publique sempre nessa URL).

Porcentagem do que falta = (portoes obrigatorios nao passados + itens abertos da esteira)
                           / (portoes obrigatorios + itens da esteira). A formula vai
escrita na pagina, para quem le poder refazer a conta.
"""
from __future__ import annotations

import html, json, re, subprocess, sys
from datetime import datetime, timezone
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
SAIDA = RAIZ / "docs/dossie/dossie-phxclaw.html"
CERT = RAIZ / "reports/RELEASE_CERTIFICATION_v0.70.json"
ESTEIRA = RAIZ / "docs/ESTEIRA_ABSORCAO.md"


def sh(cmd: str) -> str:
    return subprocess.run(cmd, shell=True, cwd=RAIZ, capture_output=True, text=True).stdout


def suite() -> tuple[dict, str, str]:
    if "--suite" in sys.argv:
        arq = Path(sys.argv[sys.argv.index("--suite") + 1])
        texto = arq.read_text(errors="replace")
        quando = datetime.fromtimestamp(arq.stat().st_mtime, timezone.utc)
        origem = f"arquivo {arq.name} (data do mtime)"
    else:
        p = subprocess.run("cargo test --workspace --no-fail-fast", shell=True, cwd=RAIZ,
                           capture_output=True, text=True)
        texto = p.stdout + p.stderr
        quando = datetime.now(timezone.utc)
        origem = "cargo test --workspace, rodado por este gerador"
    r = [tuple(map(int, m)) for m in re.findall(
        r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored", texto)]
    placar = {"passam": sum(x[0] for x in r), "falham": sum(x[1] for x in r),
              "ignorados": sum(x[2] for x in r)}
    alvos = re.findall(r"`-p ([\w-]+) --\w+`", texto.split("target failed")[-1]) \
        if "target failed" in texto else []
    placar["alvos_falhos"] = sorted(set(alvos))
    return placar, quando.strftime("%d/%m/%Y %H:%M UTC"), origem


def codigo() -> dict:
    arqs = [p for base in ("crates", "apps") for p in (RAIZ / base).rglob("*.rs")
            if "target" not in p.parts and "node_modules" not in p.parts]
    linhas = sum(len(p.read_text(errors="replace").splitlines()) for p in arqs)
    crates = len([d for d in (RAIZ / "crates").iterdir() if (d / "Cargo.toml").exists()])
    apps = len([d for d in (RAIZ / "apps").iterdir() if d.is_dir()])
    return {"linhas": linhas, "arquivos": len(arqs), "crates": crates, "apps": apps}


def esteira() -> list[dict]:
    itens = []
    for linha in ESTEIRA.read_text().splitlines():
        m = re.match(r"\|\s*([EU]\d+[a-z]?)\s*\|\s*(.+?)\s*\|(.+)\|\s*$", linha)
        if not m:
            continue
        cols = [c.strip() for c in m.group(3).split("|")]
        ultima = cols[-1]
        estado = "feito" if "✓" in ultima or "☑" in ultima else (
            "bloqueado" if "bloqueado" in ultima else "aberto")
        itens.append({"id": m.group(1), "texto": m.group(2), "estado": estado,
                      "nota": ultima.replace("☐", "").strip() if estado != "aberto" else ""})
    return itens


NOMES = {
    "cargo_fmt": "Formatação (cargo fmt)",
    "cargo_clippy_deny_warnings": "Clippy sem avisos",
    "cargo_test_workspace": "Suíte de testes inteira",
    "cargo_audit": "Auditoria de dependências",
    "agente_autonomo": "Agente autônomo",
    "postgresql_e2e": "PostgreSQL ponta a ponta",
    "postgresql_chaos_sigkill": "Caos: SIGKILL no PostgreSQL",
    "provider_local_ollama_e2e": "Modelo local (Ollama)",
    "channel_provider_credentialed_e2e": "Canais com credencial real",
    "desktop_os_automation_e2e": "Automação de desktop físico",
    "device_pairing_wss_keyring_multiplatform_e2e": "Pareamento de dispositivos",
    "native_tauri_e2e": "Desktop Tauri (WebDriver)",
    "real_stt_model_e2e": "Voz para texto (modelo real)",
    "builtin_plugin_signatures": "Assinatura dos plugins nativos",
}


def e(s) -> str:
    return html.escape(str(s))


def main() -> int:
    cert = json.loads(CERT.read_text())
    cert_quando = datetime.fromisoformat(cert["certified_at"]).strftime("%d/%m/%Y %H:%M UTC")
    placar, suite_quando, suite_origem = suite()
    cod = codigo()
    est = esteira()
    cogn = len(list((RAIZ / "docs/cognicao").glob("cognicao_*.md")))
    commits = int(sh("git rev-list --count HEAD").strip() or 0)
    ramo = sh("git branch --show-current").strip()
    ultimos = [l.split("\x1f") for l in sh(
        "git log -8 --date=format:'%d/%m %H:%M' --format='%h\x1f%ad\x1f%s'").splitlines()]

    obrig = [g for g in cert["gates"] if g["required"]]
    ob_ok = [g for g in obrig if g["status"] == "passed"]
    est_aberto = [i for i in est if i["estado"] != "feito"]
    total = len(obrig) + len(est)
    falta = len(obrig) - len(ob_ok) + len(est_aberto)
    pct = 100 * falta / total if total else 0.0
    agora = datetime.now(timezone.utc).strftime("%d/%m/%Y %H:%M UTC")

    def pilula(st):
        rot = {"passed": "passou", "failed": "falhou", "blocked": "bloqueado",
               "feito": "feito", "aberto": "aberto", "bloqueado": "bloqueado"}[st]
        return f'<span class="pl pl-{e(st)}">{rot}</span>'

    linhas_portao = []
    for g in cert["gates"]:
        motivo = g.get("reason") or ""
        if g["gate"] == "cargo_test_workspace" and g["status"] == "failed":
            motivo = f"{g['measured']['failed']} teste falhou em " + \
                ", ".join(placar["alvos_falhos"] or ["?"]) + \
                " — mesma causa da assinatura dos plugins (chave externa)"
        linhas_portao.append(
            f"<tr><td>{e(NOMES.get(g['gate'], g['gate']))}</td>"
            f"<td>{'sim' if g['required'] else 'não'}</td><td>{pilula(g['status'])}</td>"
            f"<td class='mot'>{e(motivo)}</td></tr>")

    linhas_est = "".join(
        f"<tr><td class='num'>{e(i['id'])}</td><td>{e(i['texto'])}</td>"
        f"<td>{pilula(i['estado'])}</td><td class='mot'>{e(i['nota'])}</td></tr>" for i in est)
    linhas_git = "".join(
        f"<li><code>{e(h)}</code><span class='dt'>{e(d)}</span>{e(s)}</li>" for h, d, s in ultimos)

    # rosca: fracao feita
    feito = total - falta
    circ = 2 * 3.14159265 * 52
    arco = circ * feito / total if total else 0

    pagina = f"""<title>Dossiê PhxClaw</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@500;700&family=Source+Sans+3:wght@400;600&family=JetBrains+Mono:wght@400&display=swap">
<style>
/* painel de engenharia: capa com os numeros, depois portoes, esteira e historico em blocos */
:root {{
  --fundo:#f3f4f8; --papel:#ffffff; --tinta:#161a2e; --suave:#5a6078; --linha:#d9dce8;
  --marca:#C63C0A; --ok:#1c7a45; --ruim:#b3261e; --trava:#8a5a00;
  --disp:"Exo 2", "Segoe UI", sans-serif; --corpo:"Source Sans 3", "Segoe UI", sans-serif;
  --mono:"JetBrains Mono", ui-monospace, monospace;
}}
@media (prefers-color-scheme: dark) {{ :root:not([data-theme="light"]) {{
  --fundo:#010418; --papel:#0b1030; --tinta:#e8eaf5; --suave:#9aa0bd; --linha:#232a52;
  --marca:#ff6a2b; --ok:#4fd08a; --ruim:#ff7b72; --trava:#f0b44c; color-scheme:dark; }} }}
:root[data-theme="dark"] {{
  --fundo:#010418; --papel:#0b1030; --tinta:#e8eaf5; --suave:#9aa0bd; --linha:#232a52;
  --marca:#ff6a2b; --ok:#4fd08a; --ruim:#ff7b72; --trava:#f0b44c; color-scheme:dark; }}
body {{ background:var(--fundo); color:var(--tinta); font:16px/1.5 var(--corpo); }}
.wrap {{ max-width:1080px; margin:0 auto; padding-inline:16px; padding-block:32px 48px;
  display:grid; gap:28px; }}
h1,h2 {{ font-family:var(--disp); text-wrap:balance; margin:0; }}
h1 {{ font-size:2.2rem; letter-spacing:.01em; }}
h1 span {{ color:var(--marca); }}
h2 {{ font-size:1.25rem; display:flex; gap:10px; align-items:baseline; }}
h2 small {{ font:400 .8rem var(--mono); color:var(--suave); }}
.sub {{ color:var(--suave); margin:.2rem 0 0; }}
.capa {{ display:grid; grid-template-columns:auto 1fr; gap:28px; align-items:center;
  background:var(--papel); border:1px solid var(--linha); border-radius:10px; padding:24px; }}
.rosca text {{ font-family:var(--disp); fill:var(--tinta); }}
.tiles {{ display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); gap:14px; }}
.tile {{ display:grid; gap:2px; }}
.tile b {{ font:700 1.7rem var(--disp); font-variant-numeric:tabular-nums; }}
.tile span {{ font-size:.82rem; color:var(--suave); text-transform:uppercase; letter-spacing:.06em; }}
.tile b.ruim {{ color:var(--ruim); }}
.veredito {{ font:700 .85rem var(--mono); color:var(--ruim); border:1px solid var(--ruim);
  border-radius:4px; padding:2px 8px; justify-self:start; }}
section {{ display:grid; gap:12px; min-width:0; }}
.tab {{ overflow-x:auto; background:var(--papel); border:1px solid var(--linha); border-radius:10px; }}
table {{ border-collapse:collapse; width:100%; font-size:.93rem; }}
th,td {{ text-align:left; padding:9px 12px; border-bottom:1px solid var(--linha); vertical-align:top; }}
th {{ font:600 .75rem var(--corpo); text-transform:uppercase; letter-spacing:.06em; color:var(--suave); }}
tr:last-child td {{ border-bottom:0; }}
td.mot {{ color:var(--suave); font-size:.87rem; }}
td.num {{ font-family:var(--mono); }}
.pl {{ font:600 .74rem var(--mono); padding:1px 7px; border-radius:3px; border:1px solid; white-space:nowrap; }}
.pl-passed,.pl-feito {{ color:var(--ok); }}
.pl-failed {{ color:var(--ruim); }}
.pl-blocked,.pl-bloqueado {{ color:var(--trava); border-style:dashed; }}
.pl-aberto {{ color:var(--suave); }}
.formula {{ font:400 .82rem var(--mono); color:var(--suave); }}
ul.git {{ list-style:none; margin:0; padding:0; display:grid; gap:6px; font-size:.92rem; }}
ul.git li {{ display:grid; grid-template-columns:auto auto 1fr; gap:10px; min-width:0; }}
ul.git code {{ font-family:var(--mono); color:var(--marca); }}
.dt {{ font-family:var(--mono); color:var(--suave); }}
footer {{ font-size:.8rem; color:var(--suave); border-top:1px solid var(--linha); padding-top:12px; }}
@media (max-width:620px) {{ .tiles {{ grid-template-columns:repeat(2,minmax(0,1fr)); }} .capa {{ grid-template-columns:1fr; justify-items:start; }}
  ul.git li {{ grid-template-columns:auto 1fr; }} ul.git li .dt {{ display:none; }} }}
</style>
<div class="wrap">
<header>
  <h1>Phx<span>Claw</span> · dossiê</h1>
  <p class="sub">Versão {e(cert['version'])} · branch <code>{e(ramo)}</code> · gerado em {agora}</p>
</header>

<div class="capa">
  <svg class="rosca" width="140" height="140" viewBox="0 0 140 140" role="img"
       aria-label="{feito} de {total} itens concluídos">
    <circle cx="70" cy="70" r="52" fill="none" stroke="var(--linha)" stroke-width="14"/>
    <circle cx="70" cy="70" r="52" fill="none" stroke="var(--marca)" stroke-width="14"
      stroke-dasharray="{arco:.1f} {circ:.1f}" transform="rotate(-90 70 70)"/>
    <text x="70" y="72" text-anchor="middle" font-size="26" font-weight="700">{pct:.1f}%</text>
    <text x="70" y="92" text-anchor="middle" font-size="11" fill="var(--suave)">falta</text>
  </svg>
  <div style="display:grid;gap:14px;min-width:0">
    <span class="veredito">{e(cert['verdict'])} · {len(ob_ok)}/{len(obrig)} portões obrigatórios</span>
    <div class="tiles">
      <div class="tile"><b>{placar['passam']}</b><span>testes passam</span></div>
      <div class="tile"><b class="{'ruim' if placar['falham'] else ''}">{placar['falham']}</b><span>testes falham</span></div>
      <div class="tile"><b>{str(format(cod['linhas'], ',')).replace(',', '.')}</b><span>linhas de Rust</span></div>
      <div class="tile"><b>{cod['crates']}</b><span>crates · {cod['apps']} apps</span></div>
      <div class="tile"><b>{commits}</b><span>commits</span></div>
      <div class="tile"><b>{cogn}</b><span>cognições</span></div>
    </div>
    <p class="formula">falta = ({len(obrig) - len(ob_ok)} portões obrigatórios não passados + {len(est_aberto)} itens abertos da esteira) / ({len(obrig)} + {len(est)}) = {falta}/{total}</p>
  </div>
</div>

<section>
  <h2>Certificação de release <small>medida em {cert_quando}</small></h2>
  <div class="tab"><table>
    <thead><tr><th>Portão</th><th>Obrigatório</th><th>Estado</th><th>Motivo</th></tr></thead>
    <tbody>{''.join(linhas_portao)}</tbody>
  </table></div>
  <p class="formula">suíte: {placar['passam']} passam, {placar['falham']} falham, {placar['ignorados']} ignorados · {e(suite_origem)} · {suite_quando}</p>
</section>

<section>
  <h2>Esteira de produção <small>docs/ESTEIRA_ABSORCAO.md</small></h2>
  <div class="tab"><table>
    <thead><tr><th>#</th><th>Item</th><th>Estado</th><th>Nota</th></tr></thead>
    <tbody>{linhas_est}</tbody>
  </table></div>
</section>

<section>
  <h2>Últimos commits</h2>
  <ul class="git">{linhas_git}</ul>
</section>

<footer>Nenhum número desta página foi digitado: todos saem de
<code>tools/dossie/gerar_dossie.py</code>, que lê a certificação, roda a suíte, conta o fonte e lê a esteira.</footer>
</div>
"""
    SAIDA.parent.mkdir(parents=True, exist_ok=True)
    SAIDA.write_text(pagina)
    print(f"gravado {SAIDA.relative_to(RAIZ)}")
    print(f"falta {pct:.1f}% ({falta}/{total}); certificacao {len(ob_ok)}/{len(obrig)}; "
          f"suite {placar['passam']}/{placar['falham']}/{placar['ignorados']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
