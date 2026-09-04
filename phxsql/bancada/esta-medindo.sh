#!/bin/sh
# Responde a UMA pergunta: ha medicao em curso nesta maquina?
#
# Sai 0 e LISTA quando achou; sai 1 e cala quando nao ha nada. E o portao que
# o aviso e o zelador consultam antes de rodar, porque rodar dentro de uma
# janela de medicao ja reprovou tres baterias num dia so -- e o vizinho que as
# reprovou era eu.
#
# POR QUE ELE EXISTE, e nao um `pgrep -f` na hora: o `pgrep -f PADRAO` SE
# ACHA. O padrao viaja na linha de comando do proprio pgrep, entao ele casa a
# si mesmo e responde «esta medindo» para sempre. Instrumento que sempre
# responde a mesma coisa nao mede nada.
#
# E a QUARTA vez que essa armadilha aparece nesta base: ja pegou um
# `pgrep -f cacar2`, um `pgrep -f video-demonstracao` e a contagem do proprio
# `comunicacao.sh` -- que traz a lei escrita no comentario dele: «o crivo e o
# NOME DO EXECUTAVEL, nunca a linha de comando». A lei estava certa, e cobria
# o script onde foi escrita. Nao cobria a PERGUNTA, que ninguem tinha
# transformado em script -- e pergunta improvisada se improvisa errado do
# mesmo jeito toda vez. Lei so vale onde alguem a pode chamar.
#
# E ela nao se aplica aqui ao pe da letra, e a divergencia e nossa: o
# executavel de uma bancada e `python3`, que nao distingue bancada de coisa
# nenhuma. A identidade dela mora no `argv[1]`. Dai os DOIS crivos:
#
#   1. por NOME DO EXECUTAVEL, onde ele diz algo (cargo, rustc);
#   2. por CAMINHO DO SCRIPT, onde o executavel e so o interpretador.
#
# E a exclusao do proprio observador nunca e por texto -- e por LINHAGEM.
# Nenhum ancestral deste processo conta, e e exatamente por isso que o shell
# que me chamou (que carrega o meu nome na linha de comando dele) nao me faz
# achar a mim mesmo. Descendente, ao contrario, CONTA: uma bancada que eu
# tivesse acabado de lancar e medicao em curso.
#
# Um `phxsqld` sozinho NAO conta como medicao, de proposito: servidor de pe e
# processo vivo (assunto do zelador, que ja o conta a parte), nao janela de
# medicao. Quem mede e a bancada, e ela sobe o servidor dela.

set -u

# -- a linhagem, medida UMA vez
proprios=" "
p=$$
while [ "${p:-0}" -gt 1 ]; do
	proprios="$proprios$p "
	[ -r "/proc/$p/stat" ] || break
	# o campo `comm` vem entre parenteses e pode ter espaco dentro; cortar
	# ate o ultimo `)` e o unico jeito estavel de chegar no ppid
	p=$(sed 's/.*) //' "/proc/$p/stat" 2>/dev/null | cut -d' ' -f2)
done

achou=0
for d in /proc/[0-9]*; do
	pid=${d#/proc/}
	case "$proprios" in *" $pid "*) continue ;; esac
	[ -r "$d/cmdline" ] || continue
	linha=$(tr '\0' ' ' <"$d/cmdline" 2>/dev/null) || continue
	[ -n "$linha" ] || continue

	motivo=''
	case "$(basename "$(readlink "$d/exe" 2>/dev/null)" 2>/dev/null)" in
	cargo) motivo='compilacao (cargo)' ;;
	rustc) motivo='compilacao (rustc)' ;;
	esac
	if [ -z "$motivo" ]; then
		case "$linha" in
		*bancada/*.py*) motivo='bancada em python' ;;
		*target/release/examples/*) motivo='exemplo de medicao' ;;
		esac
	fi
	[ -n "$motivo" ] || continue

	achou=1
	printf '%s\t%s\t%s\n' "$pid" "$motivo" "$(printf '%s' "$linha" | cut -c1-90)"
done

[ "$achou" = 1 ] && exit 0
exit 1
