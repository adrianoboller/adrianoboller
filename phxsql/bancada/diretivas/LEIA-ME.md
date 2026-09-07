# `bancada/diretivas` — o script de diretivas de usuário, exercitado

**Por que existe.** O pedido M do PDF das 26 perguntas é *«exemplo script de
diretivas de usuário para o gerenciamento do banco de dados e para o
gerenciamento do acesso às tabelas»*. Um exemplo colado à mão envelhece calado:
este script **é** o exemplo, e ele roda. Ele gera os hashes com o próprio
`phxsqld --senha`, escreve o `config.json` com três diretivas — supervisor,
administrador de um banco só, e leitor de uma tabela só —, sobe o servidor e
prova pelo soquete que cada uma vale exatamente o que diz.

**O que mede.** Sete partes, 34 afirmações. As três que não saem de ler o
código: (1) as **três portas dos fundos** — `juntar`, `unir` e `pivotar` não
têm o campo `tabela` que o portão geral lê, e pedir a tabela negada pelos três
caminhos tem de ser recusado; (2) o **controle positivo** delas, porque uma
conferência que recusa tudo protegeria igual e quebraria o motor — as três na
tabela permitida têm de passar; (3) **senha nunca em texto puro**, varrendo a
resposta do `usuarios`, o `acessos.log` e o erro padrão do servidor — e a
varredura só vale porque o script prova antes que ela acha a senha quando ela
está lá.

**Como roda.** `python3 bancada/diretivas/provar.py`, com
`target/release/phxsqld` já compilado
(`flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server`). Sobe UM
servidor na porta 6505 em 127.0.0.1, com tudo em `/tmp/phx-f5d-<pid>`, e
derruba por PID no fim — nunca `pkill`. A porta sai de
`PHX_F5_PORTA_DIRETIVAS`. Os números vão para `resultados.json`.

---

# `sql.py` — o `SHOW … SETTINGS` e o `ALTER … SET`, exercitados

**Por que existe.** O dono mandou um estudo das diretivas do HFSQL
(`HSetServer`, `HSetTransaction`, `HSetLog`, `HSetIntegrity`, `HSetDuplicates`,
`HManageTask`, as propriedades da `Connection`) e pediu para centralizar tudo em
`SHOW`, `ALTER … SET` e valores `TRUE`/`FALSE`. Ler o código diz que
`ALTER SERVER SET` desemboca no `config_gravar`; este script **prova** — pelo
soquete, com o `config.json` aberto em disco depois de cada gravação e o
`diretivas.log` lido linha a linha. O mapa que ele sustenta está em
`docs/DIRETIVAS.md`.

**O que mede.** Oito partes, 61 afirmações. As que não saem de ler o código:
(1) a **diretiva por banco valendo A QUENTE** — `reindexar` passa em `erp`,
o `ALTER DATABASE` entra, e a operação seguinte é recusada **sem reiniciar o
processo**, com o controle positivo de que a mesma operação continua passando em
`loja`; (2) o **`config.json` em disco** depois do `ALTER`, com o comentário
`_nota` sobrevivendo e a entrada global intacta ao lado da nova; (3) o
**diário** com os nove campos, gravado pelas **duas** portas (o `ALTER` e o
`config_gravar` do protocolo), com o varredor de segredo provando primeiro que
acha o que existe; (4) a **compressão que não existe**, medida: uma resposta de
5.000 linhas com **535.870 bytes** vai a **55.284 pelo `deflate` desta casa —
9,69×**, em 4,02 ms; e (5) o **portão único**, com uma operadora de verdade
(hash gerado pelo próprio `phxsqld --senha`) recusada nas três portas e o
controle positivo de que ela continua podendo ler.

**Como roda.** `python3 bancada/diretivas/sql.py`, com `target/release/phxsqld`
já compilado (`cargo build --release -p phxsql-server --bin phxsqld`). Sobe
**dois** servidores — 7300 (o principal) e 7301 (o do portão, com cadastro) —,
ambos em 127.0.0.1, com tudo em `/tmp/phx-f3-sql-<pid>`, e derruba por PID no
fim; nunca `pkill`. As portas saem de `PHX_F3_PORTA_A` e `PHX_F3_PORTA_B`. Os
números vão para `resultados-sql.json`.
