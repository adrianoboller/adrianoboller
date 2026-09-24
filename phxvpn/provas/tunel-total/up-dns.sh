#!/bin/bash
# Substituto do /etc/openvpn/update-resolv-conf SO para a prova: o do pacote
# pede o binario `resolvconf`, que este conteiner nao tem (e sai com 0 sem
# fazer nada). Faz o mesmo que ele -- le os `foreign_option_N` que o OpenVPN
# poe no ambiente e escreve o resolv.conf --, direto no arquivo. Dentro de
# `ip netns exec` o /etc/resolv.conf e o /etc/netns/<ns>/resolv.conf montado
# por cima, entao a troca vale so para o netns do membro.
#
# Uso no perfil: up "up-dns.sh PASTA" (a copia do resolv.conf de antes fica
# na PASTA, nunca no /etc do host).
PASTA=$1
case "$script_type" in
  up)
    cat /etc/resolv.conf > "$PASTA/resolv.antes" 2>/dev/null
    novo=""
    for v in $(printf '%s\n' ${!foreign_option_*} | sort -t _ -k 3 -g); do
      set -- ${!v}
      [ "$1" = dhcp-option ] || continue
      case "$2" in
        DNS) novo="${novo}nameserver $3"$'\n' ;;
        DOMAIN) novo="${novo}search $3"$'\n' ;;
      esac
    done
    printf '# escrito pelo up da VPN\n%s' "$novo" > /etc/resolv.conf
    ;;
  down)
    [ -f "$PASTA/resolv.antes" ] && cat "$PASTA/resolv.antes" > /etc/resolv.conf
    ;;
esac
exit 0
