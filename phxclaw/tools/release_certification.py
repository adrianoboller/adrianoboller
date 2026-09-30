#!/usr/bin/env python3
"""Certificacao de release do PhxClaw: roda cada portao DE VERDADE e so diz CERTIFICADA
se todos os obrigatorios passarem.

Nada aqui se digita. Os portoes de base (fmt, clippy, testes, audit, PostgreSQL, caos,
desktop, provider) rodam agora; os gates nativos obrigatorios saem do native_gate_catalog
da migracao 0070, lido do banco instalado -- a lista nao e copiada para ca. Gate que nao
pode rodar neste ambiente aparece BLOCKED com o motivo, nunca some e nunca vira PASSED.

Cada resultado vai para phxclaw.native_verification_runs com o SHA-256 da saida, e o
relatorio para reports/RELEASE_CERTIFICATION_v0.70.{json,md}.

Uso: python3 tools/release_certification.py [--sem-caos]
Ambiente: PGHOST/PGPORT (padrao /tmp:55432); PHXCLAW_E2E_OLLAMA_URL para o provider.
"""
from __future__ import annotations

import hashlib, json, os, platform, re, subprocess, sys, time, uuid
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PGHOST = os.environ.get("PGHOST", "/tmp")
PGPORT = os.environ.get("PGPORT", "55432")
DB = os.environ.get("PHXCLAW_E2E_DB", "phxclaw_e2e")
RLS_URL = f"host={PGHOST} port={PGPORT} user=phx_rls dbname={DB}"

# Gates do catalogo que este ambiente NAO alcanca, e por que. O motivo e fato medido
# nesta rodada; o gate continua obrigatorio e continua no relatorio.
BLOQUEIOS = {
    "desktop_os_automation_e2e": "exige desktop FISICO (teclado, mouse, captura reais); aqui so ha Xvfb",
    "device_pairing_wss_keyring_multiplatform_e2e": (
        "exige hardware multiplataforma; e o servidor WSS de dispositivos NAO existe no fonte "
        "(DeviceEnvelope sem consumidor fora do device-transport)"),
    "channel_provider_credentialed_e2e": "exige credenciais reais de Telegram/Discord/Slack/WhatsApp/Teams (decisao do dono)",
    "real_stt_model_e2e": "PHXCLAW_E2E_WHISPER_* nao definidos (whisper.cpp + modelo com SHA-256 de fonte externa)",
}


def roda(cmd: str, env: dict | None = None, timeout: int = 3600) -> tuple[int, str, float]:
    t = time.time()
    p = subprocess.run(cmd, shell=True, cwd=ROOT, capture_output=True, text=True,
                       env={**os.environ, **(env or {})}, timeout=timeout)
    return p.returncode, p.stdout + p.stderr, time.time() - t


def placar_cargo(saida: str) -> dict:
    r = [tuple(map(int, m)) for m in re.findall(
        r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored", saida)]
    return {"passed": sum(x[0] for x in r), "failed": sum(x[1] for x in r),
            "ignored": sum(x[2] for x in r)}


def main() -> int:
    com_caos = "--sem-caos" not in sys.argv[1:]
    portoes: list[dict] = []

    def portao(codigo, obrigatorio, cmd=None, env=None, bloqueio=None, detalhe=None, ok=None):
        g = {"gate": codigo, "required": obrigatorio}
        if bloqueio:
            g.update(status="blocked", reason=bloqueio, evidence_sha256=None)
        else:
            rc, out, dt = roda(cmd, env)
            passou = rc == 0 if ok is None else ok(rc, out)
            g.update(status="passed" if passou else "failed", command=cmd,
                     seconds=round(dt, 1), evidence_sha256=hashlib.sha256(out.encode()).hexdigest(),
                     tail=out.strip().splitlines()[-3:])
            if detalhe:
                g["measured"] = detalhe(out)
        portoes.append(g)
        print(f"{g['status'].upper():8} {codigo}" + (f" :: {g.get('reason','')}" if bloqueio else ""))
        return g

    portao("cargo_fmt", True, "cargo fmt --all --check")
    portao("cargo_clippy_deny_warnings", True, "cargo clippy --workspace --all-targets -q -- -D warnings")
    portao("cargo_test_workspace", True, "cargo test --workspace --no-fail-fast 2>&1",
           detalhe=placar_cargo)
    portao("cargo_audit", True, "cargo audit 2>&1",
           detalhe=lambda o: {"vulnerabilities": 0 if "vulnerabilities found" not in o else
                              int(re.search(r"(\d+) vulnerabilit", o).group(1))})
    portao("agente_autonomo", True,
           "cargo test -q -p phxclaw-agent -p phxclaw-llm -p phxclaw-browser -p phxclaw-web-search -p phxclaw-office 2>&1",
           detalhe=placar_cargo)
    portao("postgresql_e2e", True, "tests/postgres/run_e2e.sh 2>&1",
           env={"PGHOST": PGHOST, "PGPORT": PGPORT},
           detalhe=lambda o: re.search(r"placar: (.*)", o).group(1) if "placar:" in o else None)
    if com_caos:
        portao("postgresql_chaos_sigkill", True,
               "(tests/chaos/pg_kill_loop.sh 30 >/dev/null 2>&1 &) ; "
               "cargo test -q -p phxclaw-hypothesis --test pg_chaos -- --ignored --nocapture 2>&1",
               env={"PHXCLAW_E2E_RLS_URL": RLS_URL + " connect_timeout=2", "PHXCLAW_CHAOS_SECONDS": "30"},
               detalhe=lambda o: (re.search(r"caos: .*", o) or [None])[0])
        time.sleep(3)
    else:
        portao("postgresql_chaos_sigkill", True, bloqueio="pulado por --sem-caos nesta corrida")
    ollama = os.environ.get("PHXCLAW_E2E_OLLAMA_URL")
    portao("provider_local_ollama_e2e", False,
           "cargo test -q -p phxclaw-ollama-adapter --test ollama_e2e -- --ignored 2>&1",
           bloqueio=None if ollama else "PHXCLAW_E2E_OLLAMA_URL nao definido")

    # Gates nativos obrigatorios: a lista e a do catalogo instalado.
    rc, cat, _ = roda(f"psql -h {PGHOST} -p {PGPORT} -U postgres -d {DB} -qtAc "
                      "\"select gate_code from phxclaw.native_gate_catalog where required order by 1\"")
    catalogo = [l.strip() for l in cat.splitlines() if l.strip()] if rc == 0 else []
    if not catalogo:
        portao("native_gate_catalog", True, bloqueio="catalogo ilegivel: rode tests/postgres/run_e2e.sh antes")
    for codigo in catalogo:
        if codigo == "real_stt_model_e2e" and os.environ.get("PHXCLAW_E2E_WHISPER_BIN"):
            portao(codigo, True, "cargo test -q -p phxclaw-media-intelligence --test stt_e2e -- --ignored 2>&1",
                   detalhe=lambda o: (re.search(r"test result: .*?;.*?;", o) or [None])[0])
        elif codigo == "native_tauri_e2e":
            portao(codigo, True, "cargo build -q -p phxclaw-desktop && python3 tests/desktop/desktop_e2e.py 2>&1",
                   detalhe=lambda o: (re.search(r"placar: .*", o) or [None])[0])
        else:
            portao(codigo, True, bloqueio=BLOQUEIOS.get(codigo, "sem executor neste ambiente"))
    portao("builtin_plugin_signatures", True, bloqueio=(
        "6 manifestos builtin com digest/assinatura quebrados desde o rename da v0.41; "
        "reassinar exige a chave privada phxclaw-dev-root-2026-v05r2 (externa)"))

    obrig = [g for g in portoes if g["required"]]
    certificada = all(g["status"] == "passed" for g in obrig)
    agora = datetime.now(timezone.utc).isoformat(timespec="seconds")
    ambiente = f"{platform.system()} {platform.release()} {platform.machine()}"
    rel = {
        "product": "PhxClaw", "version": "0.70.0", "certified_at": agora, "environment": ambiente,
        "verdict": "CERTIFIED" if certificada else "NOT_CERTIFIED",
        "required_passed": sum(g["status"] == "passed" for g in obrig), "required_total": len(obrig),
        "gates": portoes,
    }

    # Evidencia no proprio banco, na tabela que a v0.70 criou para isso.
    linhas = []
    for g in portoes:
        st = g["status"]
        linhas.append("('{}','{}','{}','{}','{}',{},'{}'::jsonb,now(),now())".format(
            uuid.uuid4(), g["gate"], "linux", ambiente.replace("'", ""), st,
            f"'{g['evidence_sha256']}'" if g.get("evidence_sha256") else "NULL",
            json.dumps({k: v for k, v in g.items() if k in ("reason", "measured", "seconds")}).replace("'", "''")))
    sql = ("INSERT INTO phxclaw.native_verification_runs(run_uuid,gate_code,platform,"
           "environment_fingerprint,status,evidence_sha256,details,started_at,completed_at) VALUES "
           + ",".join(linhas) + ";")
    # Por stdin: o JSON dos detalhes tem aspas, e passado por -c ele quebrava o comando e a
    # evidencia sumia calada (medido: 0 linhas na primeira corrida).
    grav = subprocess.run(["psql", "-h", PGHOST, "-p", PGPORT, "-U", "postgres", "-d", DB,
                           "-v", "ON_ERROR_STOP=1", "-q"], input=sql, text=True, capture_output=True)
    if grav.returncode != 0:
        print("FALHA ao gravar evidencia em native_verification_runs:", grav.stderr.strip()[:300])
        return 2
    print(f"evidencia: {len(linhas)} linhas em phxclaw.native_verification_runs")

    out = ROOT / "reports"
    out.mkdir(exist_ok=True)
    (out / "RELEASE_CERTIFICATION_v0.70.json").write_text(json.dumps(rel, indent=2, ensure_ascii=False) + "\n")
    md = [f"# Certificacao de release PhxClaw 0.70.0 -- {rel['verdict']}", "",
          f"Gerado por `tools/release_certification.py` em {agora} ({ambiente}). Nao se edita.", "",
          f"Obrigatorios: **{rel['required_passed']}/{rel['required_total']}** passaram.", "",
          "| Gate | Obrigatorio | Estado | Medido / motivo |", "|---|---|---|---|"]
    for g in portoes:
        info = g.get("reason") or json.dumps(g.get("measured"), ensure_ascii=False) if g.get("reason") or g.get("measured") else ""
        md.append(f"| `{g['gate']}` | {'sim' if g['required'] else 'nao'} | {g['status'].upper()} | {info} |")
    (out / "RELEASE_CERTIFICATION_v0.70.md").write_text("\n".join(md) + "\n")
    print(f"\nveredito: {rel['verdict']} ({rel['required_passed']}/{rel['required_total']} obrigatorios)")
    return 0 if certificada else 1


if __name__ == "__main__":
    raise SystemExit(main())
