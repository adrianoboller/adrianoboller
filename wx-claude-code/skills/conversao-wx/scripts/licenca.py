#!/usr/bin/env python3
"""Serial de ativacao do WX Claude Code: assinatura RSA-2048 sem dependencias.

Por que assinatura e nao HMAC: o verificador vive dentro do plugin, e qualquer
segredo que ele carregue e lido por quem instala. Com par de chaves, o plugin
so tem a chave PUBLICA (licenca/chave-publica.json) e nao consegue forjar um
serial; a privada fica com quem vende, fora do repositorio.

O que isto protege e o que nao protege esta em licenca/LEIA-ME.md: o hook e
dissuasao para cliente honesto; a protecao real e servir o corpus e os agentes
de um servidor seu. Nao ha nada aqui que impeca alguem de apagar o hook.

Subcomandos:
  chaves gerar --saida DIR         gera chave-privada.json (0600) e chave-publica.json
  gerar --cliente N --validade AAAA-MM-DD [--maquina ID] [--email E] --chave-privada ARQ
  instalar SERIAL                  grava em ~/.wx-claude-code/licenca (ou $WX_LICENCA)
  verificar [--json]               le a licenca instalada; exit 0 valida, 3 invalida
  maquina                          imprime a impressao desta maquina, para prender o serial
  hook                             PreToolUse: nega scripts do plugin e escrita em .wx-migration/ sem licenca
  hook-sessao                      SessionStart: injeta o estado da licenca no contexto
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import platform
import re
import random
import secrets
import sys
from datetime import date
from pathlib import Path

VERSAO_DO_SERIAL = "WX2"
CHAVE_PUBLICA = Path(__file__).resolve().parents[3] / "licenca" / "chave-publica.json"
# DER do AlgorithmIdentifier + OCTET STRING de SHA-256 (RFC 8017, EMSA-PKCS1-v1_5)
PREFIXO_SHA256 = bytes.fromhex("3031300d060960864801650304020105000420")


# ---------------------------------------------------------------- RSA minimo

def _primo_provavel(n: int, rodadas: int = 40) -> bool:
    if n < 4:
        return n in (2, 3)
    if n % 2 == 0:
        return False
    for p in (3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37):
        if n % p == 0:
            return n == p
    d, r = n - 1, 0
    while d % 2 == 0:
        d //= 2
        r += 1
    rng = random.SystemRandom()
    for _ in range(rodadas):
        a = rng.randrange(2, n - 2)
        x = pow(a, d, n)
        if x in (1, n - 1):
            continue
        for _ in range(r - 1):
            x = pow(x, 2, n)
            if x == n - 1:
                break
        else:
            return False
    return True


def _gerar_primo(bits: int) -> int:
    while True:
        cand = secrets.randbits(bits) | (1 << (bits - 1)) | 1
        if _primo_provavel(cand):
            return cand


def gerar_chaves(bits: int = 2048) -> tuple[dict, dict]:
    e = 65537
    while True:
        p, q = _gerar_primo(bits // 2), _gerar_primo(bits // 2)
        if p == q:
            continue
        n = p * q
        phi = (p - 1) * (q - 1)
        if phi % e:
            break
    d = pow(e, -1, phi)
    pub = {"algoritmo": "RSA-2048/SHA-256", "n": hex(n), "e": e}
    priv = {"algoritmo": "RSA-2048/SHA-256", "n": hex(n), "e": e, "d": hex(d)}
    return priv, pub


def _emsa(mensagem: bytes, tamanho: int) -> int:
    t = PREFIXO_SHA256 + hashlib.sha256(mensagem).digest()
    ps = b"\xff" * (tamanho - len(t) - 3)
    return int.from_bytes(b"\x00\x01" + ps + b"\x00" + t, "big")


def assinar(mensagem: bytes, priv: dict) -> bytes:
    n, d = int(priv["n"], 16), int(priv["d"], 16)
    k = (n.bit_length() + 7) // 8
    return pow(_emsa(mensagem, k), d, n).to_bytes(k, "big")


def conferir(mensagem: bytes, assinatura: bytes, pub: dict) -> bool:
    try:
        n, e = int(pub["n"], 16), int(pub["e"])
    except (KeyError, TypeError, ValueError):
        return False
    # Chave fraca ou expoente absurdo (e=1 aceitaria EMSA(payload) como assinatura).
    # dois primos de 1024 bits dao 2047 ou 2048 bits; piso em 2040
    if n.bit_length() < 2040 or e < 3 or e % 2 == 0:
        return False
    k = (n.bit_length() + 7) // 8
    if len(assinatura) != k:
        return False
    s = int.from_bytes(assinatura, "big")
    if s >= n:
        return False
    return pow(s, e, n) == _emsa(mensagem, k)


# ---------------------------------------------------------------- serial

def _b64(b: bytes) -> str:
    return base64.urlsafe_b64encode(b).rstrip(b"=").decode()


def _unb64(s: str) -> bytes:
    return base64.urlsafe_b64decode(s + "=" * (-len(s) % 4))


def impressao_da_maquina() -> str:
    partes = [platform.node(), os.environ.get("USER") or os.environ.get("USERNAME") or ""]
    for arq in ("/etc/machine-id", "/var/lib/dbus/machine-id"):
        try:
            partes.append(Path(arq).read_text(encoding="utf-8").strip())
            break
        except OSError:
            continue
    return hashlib.sha256("|".join(partes).encode()).hexdigest()[:16]


def gerar_serial(cliente: str, validade: str, priv: dict, maquina: str = "", email: str = "") -> str:
    date.fromisoformat(validade)
    corpo = {"id": secrets.token_hex(4).upper(), "cliente": cliente, "email": email, "validade": validade, "maquina": maquina, "emitido_em": date.today().isoformat()}
    payload = json.dumps(corpo, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode()
    return ".".join([VERSAO_DO_SERIAL, _b64(payload), _b64(assinar(payload, priv))])


def caminho_da_licenca() -> Path:
    return Path(os.environ.get("WX_LICENCA") or (Path.home() / ".wx-claude-code" / "licenca"))


def verificar_serial(serial: str, pub: dict | None = None, hoje: date | None = None) -> dict:
    if pub is None:
        try:
            pub = json.loads(CHAVE_PUBLICA.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return {"status": "chave-ausente"}
    hoje = hoje or date.today()
    partes = serial.strip().split(".")
    if len(partes) != 3 or partes[0] != VERSAO_DO_SERIAL:
        return {"status": "formato-invalido"}
    try:
        payload, ass = _unb64(partes[1]), _unb64(partes[2])
        corpo = json.loads(payload)
    except (ValueError, json.JSONDecodeError):
        return {"status": "formato-invalido"}
    if not isinstance(corpo, dict):
        return {"status": "formato-invalido"}
    if not conferir(payload, ass, pub):
        return {"status": "assinatura-invalida"}
    resultado = {"status": "valida", "id": corpo.get("id"), "cliente": corpo.get("cliente"), "validade": corpo.get("validade"), "maquina": corpo.get("maquina", "")}
    try:
        if date.fromisoformat(corpo["validade"]) < hoje:
            resultado["status"] = "vencida"
            return resultado
    except (KeyError, ValueError):
        return {"status": "formato-invalido"}
    if corpo.get("maquina") and corpo["maquina"] != impressao_da_maquina():
        resultado["status"] = "maquina-diferente"
    return resultado


def verificar_instalada() -> dict:
    p = caminho_da_licenca()
    if not p.is_file():
        return {"status": "ausente", "caminho": str(p)}
    r = verificar_serial(p.read_text(encoding="utf-8"))
    r["caminho"] = str(p)
    return r


MENSAGEM = {
    "ausente": "nenhuma licenca instalada; rode licenca.py instalar <serial>",
    "vencida": "licenca vencida; peca um serial novo",
    "maquina-diferente": "serial preso a outra maquina; peca um serial para esta (licenca.py maquina)",
    "assinatura-invalida": "serial nao foi emitido com a chave desta distribuicao",
    "formato-invalido": "serial ilegivel",
    "chave-ausente": "licenca/chave-publica.json nao existe ou nao e JSON; a distribuicao esta incompleta",
}


def _alvo_do_plugin(ferramenta: str, ti: dict) -> bool:
    """Decide se a chamada e do plugin. Normaliza barras, // e caminhos Windows; libera o proprio licenca.py."""
    if ferramenta == "Bash":
        cmd = (ti.get("command") or "").replace("\\", "/")
        while "//" in cmd:
            cmd = cmd.replace("//", "/")
        if re.search(r"licenca\.py\s+(instalar|verificar|maquina)\b", cmd):
            return False
        return bool(re.search(r"conversao-wx/\s*scripts|conversao-wx['\"]?\s*\+\s*['\"]?/?scripts|\.wx-migration", cmd))
    if ferramenta in {"Write", "Edit", "MultiEdit", "NotebookEdit"}:
        caminho = (ti.get("file_path") or ti.get("notebook_path") or "").replace("\\", "/")
        return ".wx-migration" in caminho.split("/")
    return False


# ---------------------------------------------------------------- hooks

def hook_pre_tool() -> int:
    try:
        entrada = json.load(sys.stdin)
    except json.JSONDecodeError:
        return 0
    ferramenta = entrada.get("tool_name", "")
    ti = entrada.get("tool_input", {}) or {}
    if not _alvo_do_plugin(ferramenta, ti):
        return 0
    r = verificar_instalada()
    if r["status"] == "valida":
        return 0
    print(json.dumps({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny",
                      "permissionDecisionReason": f"WX Claude Code sem licenca valida ({r['status']}): {MENSAGEM.get(r['status'], '')}. Os comandos do plugin ficam travados ate instalar um serial."}}, ensure_ascii=False))
    return 0


def hook_sessao() -> int:
    r = verificar_instalada()
    if r["status"] == "valida":
        ctx = f"WX Claude Code licenciado para {r['cliente']} (serial {r['id']}, valido ate {r['validade']})."
    else:
        ctx = (f"WX Claude Code SEM LICENCA VALIDA ({r['status']}: {MENSAGEM.get(r['status'], '')}). "
               "Recuse os comandos /wx-claude-code:* e explique como instalar o serial; nao tente contornar o hook.")
    print(json.dumps({"hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": ctx}}, ensure_ascii=False))
    return 0


# ---------------------------------------------------------------- cli

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("chaves"); c.add_argument("acao", choices=["gerar"]); c.add_argument("--saida", type=Path, required=True)
    g = sub.add_parser("gerar"); g.add_argument("--cliente", required=True); g.add_argument("--validade", required=True); g.add_argument("--maquina", default=""); g.add_argument("--email", default=""); g.add_argument("--chave-privada", type=Path, required=True)
    i = sub.add_parser("instalar"); i.add_argument("serial")
    v = sub.add_parser("verificar"); v.add_argument("--json", action="store_true")
    sub.add_parser("maquina"); sub.add_parser("hook"); sub.add_parser("hook-sessao")
    a = ap.parse_args()

    if a.cmd == "chaves":
        a.saida.mkdir(parents=True, exist_ok=True)
        priv, pub = gerar_chaves()
        fp = a.saida / "chave-privada.json"
        fd = os.open(fp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(priv, f, indent=2)
        (a.saida / "chave-publica.json").write_text(json.dumps(pub, indent=2) + "\n", encoding="utf-8")
        print(f"CREATED {fp} (0600; nunca entra no repositorio)\nCREATED {a.saida / 'chave-publica.json'} (copie para licenca/chave-publica.json do plugin)")
        return 0
    if a.cmd == "gerar":
        priv = json.loads(a.chave_privada.read_text(encoding="utf-8"))
        print(gerar_serial(a.cliente, a.validade, priv, a.maquina, a.email))
        return 0
    if a.cmd == "instalar":
        r = verificar_serial(a.serial)
        if r["status"] not in {"valida", "maquina-diferente"}:
            print(f"recusado: {r['status']} ({MENSAGEM.get(r['status'], '')})", file=sys.stderr)
            return 3
        p = caminho_da_licenca(); p.parent.mkdir(parents=True, exist_ok=True)
        fd = os.open(p, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            f.write(a.serial.strip() + "\n")
        print(f"instalada em {p}: {r['status']}, cliente {r['cliente']}, ate {r['validade']}")
        return 0 if r["status"] == "valida" else 3
    if a.cmd == "verificar":
        r = verificar_instalada()
        print(json.dumps(r, ensure_ascii=False) if a.json else f"{r['status']}" + (f": {r['cliente']} ate {r['validade']} (serial {r['id']})" if r["status"] == "valida" else f": {MENSAGEM.get(r['status'], '')}"))
        return 0 if r["status"] == "valida" else 3
    if a.cmd == "maquina":
        print(impressao_da_maquina()); return 0
    if a.cmd == "hook":
        return hook_pre_tool()
    if a.cmd == "hook-sessao":
        return hook_sessao()
    return 2


# Registro das operacoes do plugin (.wx-migration/logs/): sem projeto por
# perto, nao grava nada; falha de registro nunca derruba a operacao.
try:
    import registro
except ImportError:  # rodando de outro diretorio
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
