# Por que este script existe

O PDF das 26 perguntas (`docs/pdf/`) pede cinco respostas que so valem se
saírem exercitadas contra o motor vivo, e nao copiadas de `docs/SQL.md` ou de
`docs/TRIGGERS.md`: a lista de comandos SQL que o phxsql aceita, um exemplo de
stored procedure, um de trigger, um de `CREATE DATABASE`/`TABLE`/coluna com
chave estrangeira e ER, e um de `systables`/`syscolumns`. `exercitar.py` e a
sonda unica que produz as cinco, para as respostas colarem saida real com data
e commit, e para a proxima rodada poder rodar de novo em vez de reescrever a
mesma sonda.

# O que ele mede

Sobe um `phxsqld` proprio (porta 6100, a primeira da faixa desta frente),
manda cada comando das cinco secoes pelo soquete em JSON Lines, e imprime a
resposta CRUA que veio de volta -- ok ou erro, sem redigir nada. A secao A
manda cerca de meia centena de formas de SQL, tanto as que `docs/SQL.md` diz
aceitar quanto as que diz recusar (`AND`, `LIKE`, `JOIN`, `INSERT`/`UPDATE`/
`DELETE`, `CREATE TABLE` pelo texto...), porque comando que o doc lista e o
motor recusa e um achado tao valido quanto o contrario. As secoes E e F
gravam uma stored procedure e um trigger de verdade e chamam/disparam os dois.
A secao G cria um database e uma tabela com oito tipos de coluna, acrescenta
uma coluna numa tabela que ja tem linha, declara uma chave estrangeira nas
tres formas (padrao, `ao_alterar` explicito, `verificar:false`) e mostra a
regra primordial recusando excluir o pai com filha viva. A secao H imprime
`systables`, `syscolumns` e `catalogo`.

# Como roda

```bash
target/release/phxsqld  # ja tem de estar compilado -- este script nao compila
python3 bancada/sql-exemplos/exercitar.py
```

Sobe e derruba o servidor sozinho, num diretorio proprio em
`/tmp/phx-f1-<pid>`, apagado no fim mesmo se o script falhar no meio. Nao
precisa de argumento; a porta pode ser trocada por `PHX_F1_PORTA` se a 6100
estiver ocupada.
