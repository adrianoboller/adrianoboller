#!/usr/bin/env python3
"""Pareamento de dispositivo no Linux, com os binarios de verdade e o chaveiro NATIVO.

O servidor (`phxclaw dispositivos`) e o no (`phxclaw-device-node`) rodam como processos
separados, com TLS de uma CA propria, dentro de uma sessao DBus com o gnome-keyring: e o
Secret Service que um desktop Linux tem, e e onde o no guarda a chave sem
PHXCLAW_DEVICE_KEYSTORE. O chaveiro "login" nasce com senha NAO vazia, como no login de
um desktop (medido em 01/10: com senha vazia ele nao e criado, e todo store falha com
"no result found").

O que prova, nesta ordem:
1. o no pareia com o token de uso unico e abre sessao (cerca 2);
2. o mesmo token, de novo, e recusado;
3. o no religa SEM token, pela chave do Secret Service, e a cerca sobe (3);
4. sobra exatamente uma chave no Secret Service: a provisoria do pareamento foi apagada.

Uso: python3 tests/devices/pareamento_linux.py     (sai 0 so com 4/4; imprime placar)
Exige: openssl, dbus-run-session, gnome-keyring-daemon, secret-tool.
"""
import os, re, shutil, subprocess, sys, tempfile, uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PORTA = 8792


def sessao(d: Path) -> str:
    """O roteiro que roda DENTRO da sessao DBus: chaveiro, servidor e as tres execucoes."""
    tenant = (d / "tenant").read_text().strip()
    token = (d / "tokens").read_text().split()[1]
    no = uuid.uuid4()
    base = (f"PHXCLAW_DEVICE_WSS_URL=wss://localhost:{PORTA} PHXCLAW_TENANT_UUID={tenant} "
            f"PHXCLAW_NODE_UUID={no} PHXCLAW_DEVICE_CA_PEM={d}/ca.pem")
    roda = (f"env {base} {{}} timeout 8 {ROOT}/target/debug/phxclaw-device-node 2>&1 "
            "| tr '\\n' ' '; echo")
    return "\n".join([
        f"export HOME={d}/home",
        'echo -n "senha-de-teste" | gnome-keyring-daemon --unlock --components=secrets >/dev/null 2>&1',
        f"{ROOT}/target/debug/phxclaw dispositivos --cert {d}/srv.pem --chave {d}/srv.key "
        f"--tokens {d}/tokens --porta {PORTA} >{d}/srv.log 2>&1 & SRV=$!",
        "sleep 2",
        "echo '#1' $(" + roda.format(f"PHXCLAW_ENROLLMENT_TOKEN={token}") + ")",
        "echo '#2' $(" + roda.format(f"PHXCLAW_ENROLLMENT_TOKEN={token}") + ")",
        "echo '#3' $(" + roda.format("") + ")",
        "echo '#4' $(secret-tool search --all service PhxClaw 2>/dev/null | grep -c '^label')",
        "kill $SRV",
    ])


def main() -> int:
    for f in ("openssl", "dbus-run-session", "gnome-keyring-daemon", "secret-tool"):
        if not shutil.which(f):
            print(f"falta {f}: a prova nao roda sem ele")
            return 2
    b = subprocess.run("cargo build -q -p phxclaw -p phxclaw-device-node", shell=True, cwd=ROOT)
    if b.returncode:
        return 2
    d = Path(tempfile.mkdtemp(prefix="phx-par-linux-"))
    (d / "home").mkdir()
    ssl = lambda *a: subprocess.run(["openssl", *a], cwd=d, capture_output=True, check=True)
    ssl("req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256", "-nodes",
        "-days", "1", "-subj", "/CN=CA de teste", "-keyout", "ca.key", "-out", "ca.pem")
    ssl("req", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256", "-nodes",
        "-subj", "/CN=localhost", "-keyout", "srv.key0", "-out", "srv.csr")
    (d / "ext").write_text("basicConstraints=CA:FALSE\nsubjectAltName=DNS:localhost\n")
    ssl("x509", "-req", "-in", "srv.csr", "-CA", "ca.pem", "-CAkey", "ca.key",
        "-CAcreateserial", "-days", "1", "-extfile", "ext", "-out", "srv.pem")
    ssl("pkcs8", "-topk8", "-nocrypt", "-in", "srv.key0", "-out", "srv.key")
    (d / "tenant").write_text(str(uuid.uuid4()))
    (d / "tokens").write_text(f"{(d / 'tenant').read_text()} {os.urandom(16).hex()}\n")
    (d / "sessao.sh").write_text(sessao(d))
    p = subprocess.run(["dbus-run-session", "--", "bash", str(d / "sessao.sh")],
                       capture_output=True, text=True, timeout=180)
    linhas = {l[:2]: l[3:] for l in p.stdout.splitlines() if l.startswith("#")}
    cerca = lambda s: int(m.group(1)) if (m := re.search(r"cerca (\d+)", s or "")) else None
    c1, c3 = cerca(linhas.get("#1")), cerca(linhas.get("#3"))
    checagens = [
        ("pareia e abre sessao", "pareado" in linhas.get("#1", "") and c1 is not None),
        ("token gasto e recusado", "ja usado" in linhas.get("#2", "")),
        ("religa pelo Secret Service, cerca sobe",
         "pareado" not in linhas.get("#3", "") and c1 is not None and c3 is not None and c3 > c1),
        ("uma chave so no Secret Service", linhas.get("#4", "").strip() == "1"),
    ]
    for (nome, ok), k in zip(checagens, ("#1", "#2", "#3", "#4")):
        print(f"{'OK  ' if ok else 'FALHA'} {nome} :: {linhas.get(k, '(sem saida)')[:160]}")
    verdes = sum(ok for _, ok in checagens)
    print(f"placar: {verdes}/{len(checagens)}")
    shutil.rmtree(d, ignore_errors=True)
    return 0 if verdes == len(checagens) else 1


if __name__ == "__main__":
    sys.exit(main())
