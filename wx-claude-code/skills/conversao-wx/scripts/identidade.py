#!/usr/bin/env python3
"""Identidade do agente (SPIFFE) e atestado do que a maquina REALMENTE prova.

Duas metades da mesma pergunta: *quem executou isto, e onde?*

IDENTIDADE. Em vez de `agente = desenvolvedor` no prompt -- que e um nome, nao
uma identidade -- cada papel ganha um SPIFFE ID no formato padrao:

    spiffe://<dominio>/projeto/<projeto>/agente/<papel>

e um documento assinado com a MESMA RSA do serial (nada de dependencia nova),
com validade curta. Quem recebe confere com a chave publica. Isso serve para
amarrar operacao a papel no registro e na telemetria, e para o dia em que houver
servico proprio do outro lado.

ATESTADO. Aqui e onde quase todo produto mente. O honesto e este: o script LE o
que o sistema operacional expoe e nao conclui nada alem disso.

    TPM presente        /sys/class/tpm existe? (Linux)
    Secure Boot         a variavel EFI diz o que?
    SEV / TDX / SGX     as flags da CPU aparecem?
    virtualizacao       /sys/hypervisor, DMI

Cada um vira `sim`, `nao` ou **INDISPONIVEL**, e o documento diz, em letras:
*presenca de TPM nao e attestation*. Attestation de verdade exige uma quote
assinada pelo chip e um verificador remoto -- o que este plugin nao faz e nao
finge fazer. Um campo "attested: true" sem quote e a mentira mais cara desta
lista inteira, porque e a que alguem leva para auditoria.

Uso:
  identidade.py emitir --papel desenvolvedor --chave-privada ARQ [--dias 1]
  identidade.py conferir ARQ [--chave-publica ARQ]
  identidade.py atestado [--json]
"""
from __future__ import annotations

import argparse
import json
import os
import platform
import re
import subprocess
import sys
from datetime import date, datetime, timedelta, timezone
from pathlib import Path

INDISP = "INDISPONÍVEL"
DOMINIO_PADRAO = "wx-claude-code"
PAPEIS = ("desenvolvedor", "qa", "validador", "revisor", "documentador",
          "arquiteto", "dba", "pmo", "pesquisador", "zelador")


def _licenca():
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import licenca  # noqa: PLC0415
    return licenca


def spiffe_id(dominio: str, projeto: str, papel: str) -> str:
    limpo = lambda s: re.sub(r"[^A-Za-z0-9._-]+", "-", s).strip("-").lower()  # noqa: E731
    return f"spiffe://{limpo(dominio)}/projeto/{limpo(projeto)}/agente/{limpo(papel)}"


def nome_do_projeto(raiz: Path) -> str:
    man = raiz / ".wx-migration" / "wx-inputs.manifest.json"
    if man.is_file():
        try:
            return json.loads(man.read_text(encoding="utf-8")).get("project", {}).get("name") or raiz.name
        except json.JSONDecodeError:
            pass
    return raiz.name


def emitir(args, raiz: Path) -> int:
    lic = _licenca()
    chave = Path(args.chave_privada)
    if not chave.is_file():
        print(f"erro: chave privada não encontrada: {chave}", file=sys.stderr)
        return 2
    priv = json.loads(chave.read_text(encoding="utf-8"))
    agora = datetime.now(timezone.utc)
    corpo = {
        "spiffe_id": spiffe_id(args.dominio, nome_do_projeto(raiz), args.papel),
        "papel": args.papel,
        "projeto": nome_do_projeto(raiz),
        "emitido_em": agora.isoformat(timespec="seconds"),
        "expira_em": (agora + timedelta(days=args.dias)).isoformat(timespec="seconds"),
        "maquina": lic.impressao_da_maquina(),
        # o atestado vai junto, com os limites dele: identidade sem contexto de
        # execucao vale pouco, e contexto exagerado vale menos ainda
        "atestado": medir_atestado(),
    }
    bruto = json.dumps(corpo, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    doc = {"documento": corpo,
           "assinatura": {"algoritmo": "RSA-2048 SHA-256 (licenca.py)",
                          "valor": lic._b64(lic.assinar(bruto, priv))}}
    alvo = Path(args.saida) if args.saida else raiz / ".wx-migration" / "identidade" / f"{args.papel}.json"
    alvo.parent.mkdir(parents=True, exist_ok=True)
    alvo.write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if args.json:
        print(json.dumps({"escrito": str(alvo), "spiffe_id": corpo["spiffe_id"]}, ensure_ascii=False))
    else:
        print(f"{corpo['spiffe_id']}")
        print(f"  válido até {corpo['expira_em']} · escrito em {alvo}")
    return 0


def conferir(args, raiz: Path) -> int:
    lic = _licenca()
    doc = json.loads(Path(args.arquivo).read_text(encoding="utf-8"))
    pub_path = Path(args.chave_publica) if args.chave_publica else \
        Path(__file__).resolve().parents[3] / "licenca" / "chave-publica.json"
    if not pub_path.is_file():
        print(f"erro: chave pública não encontrada: {pub_path}", file=sys.stderr)
        return 2
    pub = json.loads(pub_path.read_text(encoding="utf-8"))
    corpo = doc["documento"]
    bruto = json.dumps(corpo, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    valida = lic.conferir(bruto, lic._unb64(doc["assinatura"]["valor"]), pub)
    expirada = date.fromisoformat(corpo["expira_em"][:10]) < date.today()
    mesma_maquina = corpo.get("maquina") == lic.impressao_da_maquina()
    resultado = {"assinatura_confere": valida, "expirada": expirada,
                 "mesma_maquina": mesma_maquina, "spiffe_id": corpo["spiffe_id"]}
    if args.json:
        print(json.dumps(resultado, ensure_ascii=False))
    else:
        print(f"{corpo['spiffe_id']}")
        print(f"  assinatura: {'confere' if valida else 'NÃO CONFERE'}")
        print(f"  validade:   {'vencida' if expirada else 'em dia'} (até {corpo['expira_em'][:10]})")
        print(f"  máquina:    {'a mesma que emitiu' if mesma_maquina else 'OUTRA máquina'}")
    return 0 if (valida and not expirada) else 1


def _ler(caminho: str) -> str:
    try:
        return Path(caminho).read_text(encoding="utf-8", errors="replace").strip()
    except OSError:
        return ""


def medir_atestado() -> dict:
    """Le o que o sistema expoe. Nao conclui nada alem do que leu."""
    a: dict = {
        "sistema": f"{platform.system()} {platform.release()}",
        "arquitetura": platform.machine(),
        "tpm_presente": INDISP,
        "secure_boot": INDISP,
        "cpu_confidencial": INDISP,
        "virtualizado": INDISP,
        "container": INDISP,
    }
    if platform.system() == "Linux":
        tpm = Path("/sys/class/tpm")
        a["tpm_presente"] = "sim" if tpm.is_dir() and any(tpm.iterdir()) else "não"
        # SecureBoot: a variavel EFI comeca com 4 bytes de atributo
        efi = list(Path("/sys/firmware/efi/efivars").glob("SecureBoot-*")) \
            if Path("/sys/firmware/efi/efivars").is_dir() else []
        if efi:
            try:
                dados = efi[0].read_bytes()
                a["secure_boot"] = "ligado" if len(dados) > 4 and dados[4] == 1 else "desligado"
            except OSError:
                a["secure_boot"] = INDISP
        elif Path("/sys/firmware/efi").is_dir():
            a["secure_boot"] = INDISP
        else:
            a["secure_boot"] = "sem EFI (BIOS legado)"
        flags = _ler("/proc/cpuinfo")
        achadas = [f for f in ("sev", "sev_es", "sev_snp", "tdx_guest", "sgx") if f in flags]
        a["cpu_confidencial"] = ", ".join(achadas) if achadas else "nenhuma flag encontrada"
        a["virtualizado"] = "sim" if Path("/sys/hypervisor/type").is_file() or \
            "hypervisor" in flags else "não detectado"
        a["container"] = "sim" if Path("/.dockerenv").exists() or \
            "docker" in _ler("/proc/1/cgroup") or "kubepods" in _ler("/proc/1/cgroup") else "não detectado"
    elif platform.system() == "Darwin":
        try:
            r = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"],
                               capture_output=True, text=True, timeout=10)
            a["cpu_confidencial"] = "não se aplica (Apple Silicon usa Secure Enclave, não lido aqui)" \
                if r.returncode == 0 and "Apple" in r.stdout else INDISP
        except (OSError, subprocess.TimeoutExpired):
            pass
    a["_limites"] = {
        "isto_e": "leitura do que o sistema operacional expõe, na máquina que rodou o comando",
        "isto_nao_e": "attestation",
        "por_que": ("attestation exige uma quote assinada pelo próprio chip (TPM/SEV/TDX) e um "
                    "verificador remoto que confira essa assinatura contra a raiz do fabricante. "
                    "Nada disso acontece aqui. Presença de TPM não prova integridade da máquina, "
                    "e um campo 'attested: true' sem quote é mentira cara, porque alguém a leva "
                    "para auditoria."),
        "para_ter_attestation_de_verdade": [
            "coletar a quote no próprio hardware (tpm2_quote, SEV-SNP report, TDX quote)",
            "verificar contra a cadeia do fabricante, fora desta máquina",
            "amarrar a quote ao artefato entregue (hash no relatório da quote)",
        ],
    }
    return a


def atestado(args, raiz: Path) -> int:
    a = medir_atestado()
    if args.json:
        print(json.dumps(a, ensure_ascii=False))
        return 0
    print(f"máquina: {a['sistema']} · {a['arquitetura']}")
    for k in ("tpm_presente", "secure_boot", "cpu_confidencial", "virtualizado", "container"):
        print(f"  {k.replace('_', ' '):<20} {a[k]}")
    print(f"\nisto NÃO é attestation: {a['_limites']['por_que'][:150]}…")
    print("  (o documento completo, com o que faltaria, sai em --json)")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--project-root", default=".")
    p.add_argument("--json", action="store_true")
    sub = p.add_subparsers(dest="cmd", required=True)
    e = sub.add_parser("emitir", help="emite a identidade assinada de um papel")
    e.add_argument("--papel", required=True, choices=PAPEIS)
    e.add_argument("--chave-privada", required=True, dest="chave_privada")
    e.add_argument("--dominio", default=DOMINIO_PADRAO)
    e.add_argument("--dias", type=int, default=1)
    e.add_argument("--saida")
    c = sub.add_parser("conferir", help="confere assinatura, validade e máquina")
    c.add_argument("arquivo")
    c.add_argument("--chave-publica", dest="chave_publica")
    sub.add_parser("atestado", help="o que a máquina realmente expõe")
    args = p.parse_args()
    raiz = Path(args.project_root).resolve()
    return {"emitir": emitir, "conferir": conferir, "atestado": atestado}[args.cmd](args, raiz)


try:
    import registro
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
