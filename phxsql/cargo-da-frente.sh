#!/usr/bin/env bash
# O cargo de uma FRENTE paralela: duas vagas de compilacao para N frentes.
#
# Por que duas, e nao uma nem N: a maquina tem 4 nucleos e ~14 GB livres.
# Uma vaga so (o `flock /tmp/phx-cargo.lock` de antes) enfileirava seis
# frentes atras de um build de dois minutos cada; N vagas punham seis rustc
# de 2-3 GB a disputar 4 nucleos. Duas vagas com `-j2` cada usam a maquina
# inteira sem thrashing.
#
# `-E 99` e o que faz a vaga 2 existir: sem ele, «vaga ocupada» e «cargo
# falhou» voltam com o mesmo codigo, e a frente cairia na vaga 2 tambem
# quando o proprio build dela quebrou. O cargo nunca sai com 99 (erro e 101).
#
# CARGO_INCREMENTAL=0 porque cada worktree tem target proprio, e o cache
# incremental de um build de testes do servidor custava 4,2 GB no target
# principal -- multiplicado por seis frentes nao cabe no disco.
#
#   ./cargo-da-frente.sh test -p phxsql-server nome_do_teste
#   ./cargo-da-frente.sh build -p phxsql-server --bin phxsqld
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2
flock -n -E 99 /tmp/phx-cargo-vaga1.lock cargo "$@"
rc=$?
if [ "$rc" = 99 ]; then
  exec flock /tmp/phx-cargo-vaga2.lock cargo "$@"
fi
exit "$rc"
