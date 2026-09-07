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
