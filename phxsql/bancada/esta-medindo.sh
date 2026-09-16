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
#
# QUINTA vez que a armadilha do crivo de texto aparece, achada em 16/09/2026
# (pedido 260) em DOIS encontros no mesmo dia: o item 0b da bateria via a
# propria casca `bash -c ... prova-bateria.py` e dava ERRO em "maquina
# limpa"; e as bancadas da colmeia e da tomada, cada uma esperando o portao
# por dentro, se esperavam ETERNAMENTE porque cada uma enxergava a casca da
# outra. O crivo 2 (caminho do script no `cmdline`) casava qualquer
# INVOLUCRO que so MENCIONASSE o caminho -- um `bash -c "cd X && python3
# bancada/.../foo.py"` NAO exec-substitui o bash quando o comando tem `&&`
# (so substitui um comando simples), entao o processo do `bash -c` continua
# de pe com o texto inteiro no proprio `cmdline`, e o crivo por SUBSTRING
# casava a casca em vez do processo real.
#
# O conserto: os dois crivos de "bancada" (python e exemplo) passam a exigir
# que o PROPRIO EXECUTAVEL do processo seja o interprete ou o binario do
# exemplo -- nunca so o texto do cmdline. Um `bash -c` continua sendo `bash`
# no `/proc/<pid>/exe`, nao importa o que esteja escrito dentro do `-c`;
# so um processo que de fato EXECUTOU `python3 ...script.py` ou o binario de
# `target/release/examples/` tem o exe correspondente. E o inverso tambem
# vale: quem faz `exec -a "python3 foo.py" sleep 20` (o truque que o item 0b
# usa para simular uma bancada sem subir uma de verdade) passa a exec-trocar
# o proprio binario para `sleep`, e o `exe` deixa de mentir junto com o
# `cmdline` -- por isso o item 0b passou a lancar um `python3` de verdade
# (com o caminho da bancada so como argumento extra, nunca executado).

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

	exe=$(readlink "$d/exe" 2>/dev/null)
	exe_nome=$(basename "$exe" 2>/dev/null)

	motivo=''
	case "$exe_nome" in
	cargo) motivo='compilacao (cargo)' ;;
	rustc) motivo='compilacao (rustc)' ;;
	esac
	if [ -z "$motivo" ]; then
		# so conta como "bancada em python" quem de fato EXECUTOU o
		# interprete -- um `bash -c` que so tem o caminho escrito dentro
		# do proprio comando continua sendo `bash` aqui, nunca `python3`
		case "$exe_nome" in
		python3*|python)
			case "$linha" in
			*bancada/*.py*) motivo='bancada em python' ;;
			esac
			;;
		esac
	fi
	if [ -z "$motivo" ]; then
		# mesma logica para o exemplo: o crivo e o binario que RODOU,
		# nunca o texto que uma casca de shell carrega sobre ele
		case "$exe" in
		*/target/release/examples/*) motivo='exemplo de medicao' ;;
		esac
	fi
	[ -n "$motivo" ] || continue

	achou=1
	printf '%s\t%s\t%s\n' "$pid" "$motivo" "$(printf '%s' "$linha" | cut -c1-90)"
done

[ "$achou" = 1 ] && exit 0
exit 1
