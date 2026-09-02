#!/bin/sh
# A PREMISSA DA SP000016, medida contra o InnoDB de verdade -- e nao contra a
# lembranca dele. O roteiro dizia que o MVCC estava BLOQUEADO pela SP000013
# (RID logico + formato fisico v2). Esta bancada existe para responder uma
# pergunta so: o que ancora uma cadeia de versoes?
#
#   Se for um RID logico, a SP000013 e mesmo pre-requisito.
#   Se for a IDENTIDADE ESTAVEL da linha, o `rowid` daqui ja serve -- ele e
#   estavel por construcao, porque o `.reg` nunca reaproveita slot excluido.
#
# Medido em 2026-09-02 contra MySQL 8.0.46:
#   1. leitura aberta ANTES da escrita alheia continua vendo a versao velha,
#      sem bloquear: 100 -> 100 -> 200 (o 200 so depois do proprio commit);
#   2. versoes velhas ACUMULAM enquanto a leitura esta aberta:
#      History list length 7 -> 207 com 200 escritas, e 0 -> 150 com 150;
#   3. e sao recolhidas quando a leitura fecha: 26 s -> 0.
#
# O passo 3 foi demonstrado UMA vez; uma segunda tentativa ficou inconclusiva
# (o cliente saiu antes de o COMMIT ser consumido), e isso fica dito em vez de
# arredondado. Os passos 1 e 2 sao os que sustentam a conclusao.
#
# CONCLUSAO: em ponto nenhum a cadeia precisou de um identificador diferente do
# que ja identifica a linha. A SP000016 sai do bloqueio; a SP000013 vira
# melhoria de desempenho, e nao pre-requisito.
#
#   sudo mysqld_safe &   # o oraculo precisa estar de pe
#   ./premissa-innodb.sh
set -eu
M="mysql -uroot -N -B"
$M -e "DROP DATABASE IF EXISTS oraculo; CREATE DATABASE oraculo;"
$M oraculo -e "CREATE TABLE t (id INT PRIMARY KEY, v INT) ENGINE=InnoDB;
               INSERT INTO t VALUES (1,100);"

hist() { mysql -uroot -e "SHOW ENGINE INNODB STATUS\G" | grep -o 'History list length [0-9]*' | grep -o '[0-9]*$'; }

# A leitura fica aberta por um FIFO: `mysql -e` fecharia a transacao ao sair,
# e ai nao haveria janela nenhuma para medir.
F=$(mktemp -u); mkfifo "$F"
( $M oraculo < "$F" > /tmp/oraculo-a.txt 2>&1 ) &
exec 3>"$F"
echo "SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ; START TRANSACTION;
      SELECT CONCAT('1) A ve antes:  ', v) FROM t WHERE id=1;" >&3
command sleep 1

$M oraculo -e "UPDATE t SET v=200 WHERE id=1;"
echo "   B escreveu 200 e confirmou"
echo "SELECT CONCAT('2) A ve depois: ', v) FROM t WHERE id=1; COMMIT;
      SELECT CONCAT('3) A pos-commit:', v) FROM t WHERE id=1;" >&3
command sleep 2
exec 3>&-
cat /tmp/oraculo-a.txt; rm -f "$F" /tmp/oraculo-a.txt
echo "   (1 e 2 iguais = a versao velha sobreviveu a escrita alheia)"
