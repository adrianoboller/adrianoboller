#!/usr/bin/env bash
# Bateria do ciclo do phxSERVERMAIL -- os 9 passos, re-rodavel de ponta a ponta.
#
#   bash bancada/servermail/rodar.sh [dir_de_trabalho] [n_linhas_replicacao]
#
# Sequencia de proposito: a replicacao (passo 5, portas 5800-5803) e o cluster
# (passo 8, portas 5310-5316) sobem servidores; rodam em ordem e cada um derruba
# os seus pelo caminho antes do proximo. Nao mata processo por nome; nao toca em
# rede externa (o armazenamento na WX e simulado).
#
# Escreve resultados.json com o veredito de cada passo.
set -u

RAIZ="$(cd "$(dirname "$0")/../.." && pwd)"
AQUI="$RAIZ/bancada/servermail"
cd "$RAIZ" || exit 2
WORK="${1:-$(mktemp -d /tmp/phx-servermail-XXXX)}"
NLINHAS="${2:-20000}"
mkdir -p "$WORK"

echo "=================================================================="
echo " BATERIA phxSERVERMAIL  |  trabalho em $WORK  |  $(date +%Y-%m-%d)"
echo "=================================================================="

declare -A V   # veredito por passo

echo ""; echo ">>> compilando (release + examples) para nao medir binario velho"
cargo build --release -p phxsql-server 2>&1 | tail -2
cargo build --release --example servermail-ciclo -p phxsql-store 2>&1 | tail -1
cargo build --release --example correio-e2e -p phxsql-core 2>&1 | tail -1

echo ""; echo "########## PASSO 1 -- instalar + configurar a base ##########"
bash "$AQUI/instalar.sh" "$WORK/instalacao" 5951; V[1]=$?

echo ""; echo "########## PASSOS 2,3,4,7 -- UUID v7, integridade, coligacao, portao WX ##########"
"$RAIZ/target/release/examples/servermail-ciclo" "$WORK/base-correio"; V[2347]=$?
rm -rf "$WORK/base-correio"

echo ""; echo "########## PASSO 6 -- DNS dos dominios ##########"
python3 "$AQUI/dns.py"; V[6]=$?

echo ""; echo "########## PASSO 5 -- replicacao master -> 3 slaves (SHA-256 por linha) ##########"
timeout 400 python3 "$AQUI/replicar.py" "$WORK/replicacao" "$NLINHAS"; V[5]=$?

echo ""; echo "########## PASSO 8 -- cluster: eleicao, promocao, failover ##########"
timeout 400 python3 "$RAIZ/bancada/cluster/provar.py" "$WORK/cluster"; V[8]=$?
rm -rf "$WORK/cluster"

echo ""; echo "########## PASSO 9 -- envio empresa<->empresa (referencia correio-e2e) ##########"
"$RAIZ/target/release/examples/correio-e2e" > "$WORK/correio-e2e.out" 2>&1; V[9]=$?
tail -3 "$WORK/correio-e2e.out"

# ----- veredito -----
echo ""; echo "=================================================================="
echo " VEREDITO"
echo "=================================================================="
diz() { [ "${1}" -eq 0 ] && echo "PASSOU" || echo "FALHOU (exit ${1})"; }
printf "  passo 1 (instalar+base)        : %s\n" "$(diz ${V[1]})"
printf "  passos 2,3,4,7 (v7/FK/colig/WX): %s\n" "$(diz ${V[2347]})"
printf "  passo 5 (replicacao)           : %s\n" "$(diz ${V[5]})"
printf "  passo 6 (DNS)                  : %s (implantacao: dominios do correio ainda nao no DNS)\n" "$(diz ${V[6]})"
printf "  passo 8 (cluster)             : %s\n" "$(diz ${V[8]})"
printf "  passo 9 (correio-e2e)         : %s\n" "$(diz ${V[9]})"

python3 - "$WORK" "${V[1]}" "${V[2347]}" "${V[5]}" "${V[6]}" "${V[8]}" "${V[9]}" <<'PY'
import json, sys, time
work = sys.argv[1]
v = [int(x) for x in sys.argv[2:]]
res = {
    "quando": time.strftime("%Y-%m-%d %H:%M:%S"),
    "passo_1_instalar": v[0] == 0,
    "passos_2347_banco": v[1] == 0,
    "passo_5_replicacao": v[2] == 0,
    "passo_6_dns": v[3] == 0,
    "passo_8_cluster": v[4] == 0,
    "passo_9_correio_e2e": v[5] == 0,
    "todos_verdes": all(x == 0 for x in v),
}
# cwd e a raiz do repo (rodar.sh faz cd no inicio), entao o caminho relativo vale.
open("bancada/servermail/resultados.json", "w").write(json.dumps(res, indent=2, ensure_ascii=False) + "\n")
print("\nresultados.json gravado. todos_verdes =", res["todos_verdes"])
PY

# limpeza: nada de processo, nada de base temporaria criada por nos
pgrep -a phxsqld && echo "ATENCAO: sobrou phxsqld -- verifique" || true
[ -z "${1:-}" ] && rm -rf "$WORK"   # so remove se nos criamos o dir
exit 0
