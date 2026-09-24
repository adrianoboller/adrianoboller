#!/bin/bash
# Prova em netns do rol assinado e da descoberta na LAN (root, Linux).
#
# (a) tres membros A (dono), B, C numa LAN virtual; o dono remove C: os
#     tuneis de C com A e com B caem, o de A com B fica; C religado continua
#     de fora.
# (b) dois membros D, E na mesma bridge, convite SEM endereco e sem repasse:
#     com a descoberta ligada se acham e pingam pelo tunel; com ela desligada,
#     nao. O que passa no fio e gravado: nome da rede em claro, zero.
#
# Uso (de phxvpn/): cargo build --release && sudo provas/rol-descoberta/rodar.sh
# Grava provas/rol-descoberta/resultados.json. Processos mortos por PID.
set -u
AQUI=$(cd "$(dirname "$0")" && pwd)
BIN=${PHXVPN_BIN:-$AQUI/../../target/release/phxvpn}
T=$(mktemp -d /var/tmp/phxvpn-rol-XXXXXX)
SENHA=senha-da-prova
PIDS=()
declare -A R

limpar() {
    for p in "${PIDS[@]}"; do kill "$p" 2>/dev/null; done
    sleep 0.5
    for n in rdA rdB rdC rdD rdE; do ip netns del $n 2>/dev/null; done
    ip link del rdbr0 2>/dev/null
}
trap limpar EXIT

for n in A B C D E; do ip netns del rd$n 2>/dev/null; ip link del rdv$n 2>/dev/null; done
ip link del rdbr0 2>/dev/null
# O netns apagado solta o veth de forma assincrona: espera sumir.
for _ in $(seq 50); do ip link show | grep -q ' rdv[A-E]@' || break; sleep 0.1; done
ip link add rdbr0 type bridge && ip link set rdbr0 up
i=1
for n in A B C D E; do
    ip netns add rd$n
    ip -n rd$n link set lo up
    ip link add rdv$n type veth peer name eth0 netns rd$n
    ip link set rdv$n master rdbr0 up
    ip -n rd$n addr add 192.168.77.$i/24 dev eth0
    ip -n rd$n link set eth0 up
    mkdir -p $T/$n
    i=$((i + 1))
done

# phx NETNS ARGS... : o phxvpn dentro do netns, na pasta do membro.
phx() {
    local n=$1; shift
    ip netns exec rd$n env -C $T/$n PHXVPN_SENHA_REDE=$SENHA "$BIN" "$@"
}
ligar() {
    local n=$1; shift
    ip netns exec rd$n env -C $T/$n PHXVPN_SENHA_REDE=$SENHA "$BIN" p2p ligar "$@" \
        >> $T/$n/ligar.log 2>&1 &
    PIDS+=($!)
    eval "PID_$n=$!"
}
# pingar NETNS IP SEGUNDOS: primeiro ping que volta, em ms desde a chamada; -1 se nenhum.
pingar() {
    local ini=$(date +%s%N) fim=$(($(date +%s) + $3))
    while [ $(date +%s) -lt $fim ]; do
        if ip netns exec rd$1 ping -c1 -W1 $2 >/dev/null 2>&1; then
            echo $((($(date +%s%N) - ini) / 1000000)); return
        fi
    done
    echo -1
}

echo "== (a) rol assinado: remover membro"
phx A p2p criar --rede Matriz --ip 10.78.0.1/24 --apelido matriz > $T/A/criar.log
CB=$(phx A p2p convidar --rede Matriz --endereco 192.168.77.1:51820)
CC=$(phx A p2p convidar --rede Matriz --endereco 192.168.77.1:51820)
phx B p2p entrar "$CB" --apelido filial-b > $T/B/entrar.log
phx C p2p entrar "$CC" --apelido filial-c > $T/C/entrar.log
ligar A --rede Matriz; ligar B --rede Matriz; ligar C --rede Matriz
R[a_ab_ms]=$(pingar A 10.78.0.2 30)
R[a_ac_ms]=$(pingar A 10.78.0.3 30)
R[a_bc_ms]=$(pingar B 10.78.0.3 30)
echo "antes: A->B ${R[a_ab_ms]} ms, A->C ${R[a_ac_ms]} ms, B->C ${R[a_bc_ms]} ms"
R[a_remover]=$(phx A p2p remover --rede Matriz --ip 10.78.0.3)
echo "${R[a_remover]}"
sleep 6
R[a_depois_ca]=$(pingar C 10.78.0.1 4)
R[a_depois_cb]=$(pingar C 10.78.0.2 4)
R[a_depois_ab]=$(pingar A 10.78.0.2 4)
R[a_depois_ba]=$(pingar B 10.78.0.1 4)
echo "depois: C->A ${R[a_depois_ca]}, C->B ${R[a_depois_cb]}, A->B ${R[a_depois_ab]}, B->A ${R[a_depois_ba]}"
# C religado (sessao nova, do zero) continua de fora.
kill $PID_C; sleep 1.5; ligar C --rede Matriz; sleep 3
R[a_religado_cb]=$(pingar C 10.78.0.2 8)
R[a_religado_ca]=$(pingar C 10.78.0.1 4)
echo "C religado: C->B ${R[a_religado_cb]}, C->A ${R[a_religado_ca]}"
kill $PID_A $PID_B $PID_C; sleep 1

echo "== (b) descoberta na LAN, sem endereco nem repasse"
tcpdump -i rdbr0 -U -w $T/fio.pcap udp > /dev/null 2>&1 &
TCPD=$!; PIDS+=($TCPD); sleep 1
phx D p2p criar --rede FilialNorte --ip 10.79.0.1/24 > $T/D/criar.log
CE=$(phx D p2p convidar --rede FilialNorte)
phx E p2p entrar "$CE" > $T/E/entrar.log
ligar D --rede FilialNorte; ligar E --rede FilialNorte
R[b_ligada_ms]=$(pingar E 10.79.0.1 30)
echo "descoberta ligada: E->D ${R[b_ligada_ms]} ms"
kill $PID_D $PID_E; sleep 1; kill $TCPD; sleep 0.5
mv $T/D $T/D1; mv $T/E $T/E1; mkdir -p $T/D $T/E
phx D p2p criar --rede FilialNorte --ip 10.79.0.1/24 --sem-descoberta > $T/D/criar.log
CE=$(phx D p2p convidar --rede FilialNorte)
phx E p2p entrar "$CE" --sem-descoberta > $T/E/entrar.log
ligar D --rede FilialNorte; ligar E --rede FilialNorte
R[b_desligada_ms]=$(pingar E 10.79.0.1 20)
echo "descoberta desligada: E->D ${R[b_desligada_ms]}"
kill $PID_D $PID_E; sleep 1

# O fio da corrida ligada: anuncios (UDP de 200 bytes) e nome da rede em claro.
R[b_anuncios]=$(tcpdump -r $T/fio.pcap 'udp and udp[4:2] = 208' 2>/dev/null | wc -l)
R[b_nome_em_claro]=$(grep -c -a FilialNorte $T/fio.pcap)
echo "fio: ${R[b_anuncios]} anuncios, nome da rede em claro: ${R[b_nome_em_claro]}"

python3 - "$AQUI/resultados.json" <<EOF
import json, sys, datetime
r = {k: v for k, v in [l.split("=", 1) for l in """$(for k in "${!R[@]}"; do echo "$k=${R[$k]}"; done)""".splitlines() if "=" in l]}
n = lambda k: int(r[k])
ok = {
  "a_antes_os_tres_se_pingam": n("a_ab_ms") >= 0 and n("a_ac_ms") >= 0 and n("a_bc_ms") >= 0,
  "a_removido_perde_A_e_B": n("a_depois_ca") < 0 and n("a_depois_cb") < 0,
  "a_A_e_B_continuam": n("a_depois_ab") >= 0 and n("a_depois_ba") >= 0,
  "a_removido_religado_continua_fora": n("a_religado_cb") < 0 and n("a_religado_ca") < 0,
  "b_ligada_se_acham": n("b_ligada_ms") >= 0,
  "b_desligada_nao": n("b_desligada_ms") < 0,
  "b_nome_da_rede_fora_do_fio": n("b_nome_em_claro") == 0 and n("b_anuncios") > 0,
}
json.dump({"data": datetime.datetime.now().isoformat(timespec="seconds"),
           "roteiro": "provas/rol-descoberta/rodar.sh",
           "medidas": r, "veredito": ok, "tudo_ok": all(ok.values())},
          open(sys.argv[1], "w"), indent=2, ensure_ascii=False)
print(json.dumps(ok, indent=2))
sys.exit(0 if all(ok.values()) else 1)
EOF
