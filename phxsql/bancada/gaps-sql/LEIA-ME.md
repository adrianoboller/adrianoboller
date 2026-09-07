# `bancada/gaps-sql` — a sonda das listas de gap de SQL

**Por que existe.** As respostas B, C, D, O, P e Q do PDF das 26 perguntas
afirmam que tal comando do PostgreSQL(R), do MariaDB(R), do MySQL(R), do
SQLite(R) ou do Cassandra(R) **não existe** no PhxSql. Uma afirmação dessas lida
em `docs/SQL.md` é palpite: aquele documento é o desenho, e desenho envelhece —
foi assim que ele mesmo passou a dizer «não há transação» depois de a transação
entrar. A lei que esta pasta cumpre é curta: **lista de gap sem a recusa colada
é palpite**. Cada linha de cada lista saiu de um comando mandado ao motor vivo
nesta corrida, e a mensagem publicada é a que voltou.

**O que ela mede.** Uma coisa só: *o motor aceita ou recusa este texto?* Em
cinco fases. A primeira é o **controle positivo** — nove comandos que têm de
passar (`SELECT`, `COUNT(*)`, `ORDER BY`, `WHERE` por chave `Int8`, `WHERE` por
coluna de texto, `BEGIN`, `COMMIT`, `SHOW TRIGGERS`, `SHOW PROCEDURES`); se um
falhar a sonda **para**, porque zero sem controle é a sonda medindo a própria
conexão quebrada. A segunda manda os **136 candidatos** dos cinco manuais. A
terceira são os **achados** que a primeira corrida topou sem procurar (a chave
`Sequence` que recusa onde a `Int8` passa; o `FROM schema.tabela` inexistente
vazando `No such file or directory` com `repetir: true`; o `.fts` dobrando
acento e não fazendo prefixo). A quarta são as **equivalências**: para cada SQL
recusado, a operação do protocolo que faz a mesma coisa, rodada de verdade —
sem ela a lista mentiria por omissão, porque «o PhxSql não tem `INSERT`» é
verdade sobre a linguagem e falso sobre o motor. A quinta é a **conferência dos
sprints antigos** — o `config`, os `jobs`, o `replicacao_estado` e as sete formas
de corpo de rotina que as respostas O, P e Q citam para dizer que tal sprint
fechou ou não; ela roda **por último** porque o `CALL` dela grava uma linha, e
rodando antes mudaria as contagens das equivalências. Ela **não** julga se o gap
importa (isso é das respostas) e **não** mede desempenho.

Um aviso de vizinhança: o `phxsqld` lê o `jobs.json` do diretório de onde é
chamado, então a fase da conferência mostra os jobs que estiverem na raiz do
repositório — inclusive os de outra frente. Eles não são criados nem alterados
por esta sonda; aparecem porque o arquivo é do diretório, não da base.

**Como roda.** Precisa do binário já compilado — a sonda não compila nada, e
avisa com o comando certo se ele faltar. Sobe um `phxsqld` próprio, cria a base
em `/tmp/phx-f2-<pid>`, e derruba servidor e diretório no fim, aconteça o que
acontecer.

```bash
python3 bancada/gaps-sql/sondar.py              # tudo
python3 bancada/gaps-sql/sondar.py postgresql   # um motor só
PHX_GAPS_PORTA=6110 python3 bancada/gaps-sql/sondar.py
```

Grava `resultados.json` ao lado, com as quatro fases. **Acrescentar um comando à
lista é acrescentar uma linha em `CANDIDATOS`** — a tupla `(motor, rótulo, sql)`
—, e nada mais precisa mudar.
