#!/usr/bin/env bash
# A suite com o pulo CONTADO: roda o `cargo test`, guarda a saida num arquivo, anexa os pulos
# registrados pelo `phxclaw_test_support::pulado` e publica «N passam, P pulados, F falham».
#
# Por que existe: o libtest nao tem «ignorado em tempo de execucao», entao o teste que pula
# por falta de bwrap, Chromium ou openssl sai `ok` e entra no placar como PASSOU. Cada pulo
# vai para `<target>/tmp/pulados.jsonl` (crate, teste, recurso, motivo); este portao apaga o
# registro ANTES da suite (pulo de corrida velha nao conta) e o anexa ao proprio arquivo da
# saida, abaixo do marcador que o dossie le (`tools/dossie/numeros.py::suite`, `--suite ARQ`).
#
# O registro e UM por `target`, e outra frente pode rodar `cargo test` no mesmo `target` ao
# mesmo tempo (medido em 02/10/2026: corrida com `-p phxclaw-test-support -p phxclaw-browser`
# publicou 3 pulos de `phxclaw-agent` de um `cargo test` vizinho). Por isso, com `-p`, so
# entram no bloco e no placar os pulos dos crates pedidos; com `--workspace` entra tudo.
#
# Sai com codigo diferente de 0 se algum teste FALHAR (1) ou se houver pulo de recurso que
# ESTA MAQUINA TEM (2): binario no PATH ou em /var/tmp, variavel de ambiente definida,
# caminho existente, bwrap presente. Recurso que o portao nao sabe conferir (ex.:
# `sem-systemd`) conta como pulo, nunca como NoGo -- e a lista sai inteira, com TEM/nao tem,
# para quem le julgar a conferencia em vez de acreditar nela.
#
# Uso:
#   tools/suite.sh                              # cargo test --workspace --no-fail-fast
#   tools/suite.sh -p phxclaw-agent -p phxclaw  # so os crates passados
#   tools/suite.sh -o target/suite.txt ...      # onde guardar a saida (padrao target/suite.txt)
#   tools/suite.sh ... -- --test motor          # o resto vai para o cargo test
# Depois: python3 tools/dossie/gerar_dossie.py --suite target/suite.txt
set -uo pipefail
RAIZ=$(cd "$(dirname "$0")/.." && pwd)
TARGET=${CARGO_TARGET_DIR:-$RAIZ/target}
REGISTRO="$TARGET/tmp/pulados.jsonl"
SAIDA="$TARGET/suite.txt"
CRATES=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o) SAIDA=$2; shift 2 ;;
    -p) CRATES+=(-p "$2"); shift 2 ;;
    --) shift; break ;;
    *) echo "argumento desconhecido: $1 (veja o cabecalho de $0)" >&2; exit 64 ;;
  esac
done
if [[ ${#CRATES[@]} -eq 0 ]]; then CRATES=(--workspace); fi

mkdir -p "$(dirname "$SAIDA")" "$TARGET/tmp"
rm -f "$REGISTRO"
cd "$RAIZ"
# `tee` para a tela e para o arquivo; o codigo e o do cargo, nao o do tee.
cargo test "${CRATES[@]}" --no-fail-fast "$@" 2>&1 | tee "$SAIDA"
RC_CARGO=${PIPESTATUS[0]}

# Os nomes dos crates pedidos (sem o `-p`), para o filtro do registro; vazio = todos.
PEDIDOS=()
for ((i = 0; i < ${#CRATES[@]}; i++)); do
  [[ ${CRATES[$i]} == -p ]] && PEDIDOS+=("${CRATES[$((i + 1))]}")
done
{
  echo
  echo "=== PULADOS (target/tmp/pulados.jsonl)"
  if [[ -f "$REGISTRO" ]]; then
    if [[ ${#PEDIDOS[@]} -eq 0 ]]; then
      cat "$REGISTRO"
    else
      python3 - "$REGISTRO" "${PEDIDOS[@]}" <<'PY'
import json, sys
from pathlib import Path
registro, pedidos = Path(sys.argv[1]), set(sys.argv[2:])
for lin in registro.read_text(errors="replace").splitlines():
    try:
        d = json.loads(lin)
    except ValueError:
        continue
    if d.get("crate") in pedidos:
        print(lin)
PY
    fi
  fi
} >> "$SAIDA"

# O placar e a conferencia dos recursos. O marcador e o mesmo do numeros.py.
python3 - "$SAIDA" "$RAIZ" "$RC_CARGO" <<'PY'
import json, os, re, shutil, sys
from pathlib import Path
saida, raiz, rc_cargo = Path(sys.argv[1]), Path(sys.argv[2]), int(sys.argv[3])
texto = saida.read_text(errors="replace")
corpo, _, bloco = texto.partition("=== PULADOS (target/tmp/pulados.jsonl)")
r = [tuple(map(int, m)) for m in re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored", corpo)]
passam, falham = sum(x[0] for x in r), sum(x[1] for x in r)
pulos = {}
for lin in bloco.splitlines():
    try:
        d = json.loads(lin)
    except ValueError:
        continue
    pulos[(d.get("crate"), d.get("teste"))] = d

def servidor_responde(host: str, porta: str) -> bool:
    """`nome:host:porta`: servidor e TEM so se o socket aceita conexao. Host que comeca com
    `/` e diretorio de socket Unix (o PostgreSQL poe `.s.PGSQL.<porta>` ali); senao, TCP."""
    import socket
    try:
        if host.startswith("/"):
            s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            s.settimeout(1)
            s.connect(str(Path(host) / f".s.PGSQL.{porta}"))
        else:
            s = socket.create_connection((host, int(porta)), timeout=1)
        s.close()
        return True
    except (OSError, ValueError):
        return False

def tem(recurso: str) -> bool:
    """A maquina TEM o recurso? Binario no PATH ou em /var/tmp, variavel definida, caminho,
    ou servidor (`nome:host:porta`) que aceita conexao -- binario presente com servidor
    parado nao e TEM (medido: `psql` no PATH e o PostgreSQL de /tmp:55432 parado dava NoGo)."""
    if not recurso:
        return False
    if recurso.count(":") == 2:
        _, host, porta = recurso.split(":")
        return servidor_responde(host, porta)
    if shutil.which(recurso):
        return True
    if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", recurso) and os.environ.get(recurso):
        return True
    if (raiz / recurso).exists() or Path(recurso).is_absolute() and Path(recurso).exists():
        return True
    base = Path("/var/tmp")
    if base.is_dir():
        for d in [base] + [p for p in base.iterdir() if p.is_dir()]:
            try:
                for sub in [d, d / "bin", d / "build" / "bin", d / "usr" / "bin"]:
                    if (sub / recurso).is_file():
                        return True
            except PermissionError:
                pass
    return False

nogo = []
for (crate, teste), d in sorted(pulos.items()):
    rec = d.get("recurso") or ""
    tem_ = tem(rec)
    print(f"  pulado {'TEM     ' if tem_ else 'nao tem '} {rec:28} {crate}::{teste} -- {d.get('motivo','')}")
    if tem_:
        nogo.append((crate, teste, rec))
print(f"{passam} passam, {len(pulos)} pulados, {falham} falham  (saida em {saida})")
if rc_cargo != 0 or falham:
    print(f"FALHOU: cargo test saiu {rc_cargo}, {falham} teste(s) falharam", file=sys.stderr)
    sys.exit(1)
if nogo:
    print(f"NOGO: {len(nogo)} pulo(s) de recurso que esta maquina TEM: "
          + ", ".join(f"{c}::{t} ({r})" for c, t, r in nogo), file=sys.stderr)
    sys.exit(2)
PY
