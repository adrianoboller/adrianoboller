#!/usr/bin/env python3
"""Ressincroniza o FULL_INSTALL do PostgreSQL depois de um reparo nativo.

O pacote v0.70 trouxe o bundle pronto, sem o gerador. Cada reparo feito contra um
PostgreSQL real muda o corpo de um bloco de migracao, e com ele o effective_sha256
gravado em tres lugares: o cabecalho do bloco, o INSERT no phxclaw_schema_migrations
e o manifesto. Digitar esses hashes a mao e o jeito de eles divergirem calados; por
isso saem daqui, do proprio conteudo.

Regra do hash (medida contra os 70 blocos originais antes de escrever este script):
effective_sha256 = sha256(corpo.rstrip("\\n") + "\\n"), onde corpo e o texto entre a
linha de fecho do cabecalho e o INSERT do ledger.

Uso:
    python3 tools/full_install_resync.py          # reescreve bundle, .sha256 e manifesto
    python3 tools/full_install_resync.py --check  # so confere; sai 1 se algo divergir
"""
from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DB = ROOT / "database"
SQL = DB / "PhxClaw_PostgreSQL_FULL_INSTALL_v0.70.sql"
SHA = DB / "PhxClaw_PostgreSQL_FULL_INSTALL_v0.70.sha256"
MANIFEST = DB / "PhxClaw_PostgreSQL_FULL_INSTALL_v0.70_manifest.json"
RULE = "-- ============================================================================\n"

# Reparos feitos contra PostgreSQL 16 real. A nota vai para o repair_notes do ledger,
# para que o banco instalado diga por que o bloco difere do original.
REPAROS: dict[int, str] = {
    23: "native-v070: UNIQUE de knowledge_nodes ganhou epistemic_state; o motor versiona o no trocando so o estado",
    58: "native-v070: array_to_string e STABLE; coluna gerada exige IMMUTABLE -> wrapper phx_immutable_tags_text(text[])",
    70: "native-v070: 21 politicas RLS liam so phxclaw.tenant_id; unificadas em phxclaw.current_tenant_uuid()",
}


def sha(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def blocos(s: str):
    for m in re.finditer(r"^-- MIGRATION (\d{4}): (\S+)\n", s, re.M):
        versao = int(m.group(1))
        fim_cab = s.index(RULE, m.end()) + len(RULE)
        ins = re.compile(
            r"^INSERT INTO phxclaw_schema_migrations\(version,migration_name,source_sha256,"
            r"effective_sha256,source_status,repair_notes\) VALUES \(" + str(versao) + r",.*$",
            re.M,
        ).search(s, fim_cab)
        if ins is None:
            raise SystemExit(f"bloco {versao}: INSERT do ledger nao encontrado")
        yield versao, m.start(), fim_cab, ins


def main() -> int:
    check = "--check" in sys.argv[1:]
    s = SQL.read_text(encoding="utf-8")
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    por_versao = {m["version"]: m for m in manifest["migrations"]}
    divergencias: list[str] = []

    # De tras para a frente, para que os offsets ainda nao lidos continuem valendo.
    for versao, ini, fim_cab, ins in reversed(list(blocos(s))):
        efetivo = sha(s[fim_cab : ins.start()].rstrip("\n") + "\n")
        linha = ins.group(0)
        campos = re.match(
            r"(.*VALUES \(\d+,'[^']*','[0-9a-f]{64}',')([0-9a-f]{64})(','[^']*',)(NULL|'[^']*')(\) ON CONFLICT.*)",
            linha,
        )
        if campos is None:
            raise SystemExit(f"bloco {versao}: INSERT fora do formato esperado")
        nota = REPAROS.get(versao)
        nota_sql = "NULL" if nota is None else "'" + nota.replace("'", "''") + "'"
        if nota is None and campos.group(4) != "NULL":
            nota_sql = campos.group(4)  # nota historica do pacote: preservada
        nova = campos.group(1) + efetivo + campos.group(3) + nota_sql + campos.group(5)
        cab = s[ini:fim_cab]
        cab_novo = re.sub(r"(-- effective_sha256: )[0-9a-f]{64}", r"\g<1>" + efetivo, cab)
        if nova != linha or cab_novo != cab:
            divergencias.append(f"bloco {versao:04d}: effective -> {efetivo[:16]}")
        s = s[:ins.start()] + nova + s[ins.end():]
        s = s[:ini] + cab_novo + s[fim_cab:]
        if versao in por_versao:
            por_versao[versao]["effective_sha256"] = efetivo
            if nota is not None:
                por_versao[versao]["repair_notes"] = nota

    total = sha(s)
    if manifest.get("full_install_sha256") != total:
        divergencias.append(f"full_install_sha256 -> {total[:16]}")
    manifest["full_install_sha256"] = total

    if check:
        for d in divergencias:
            print("DIVERGE", d)
        print(f"{len(divergencias)} divergencia(s)")
        return 1 if divergencias else 0

    SQL.write_text(s, encoding="utf-8")
    SHA.write_text(f"{total}  {SQL.name}\n", encoding="utf-8")
    MANIFEST.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    for d in divergencias:
        print("ATUALIZADO", d)
    print(f"sha256 {total}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
