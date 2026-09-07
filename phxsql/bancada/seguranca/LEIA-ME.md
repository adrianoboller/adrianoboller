# Bancada de segurança da porta

Duas baterias contra um `phxsqld` **de pé**, sempre pelo soquete: `porta.py`
mede o acesso pela porta TCP/IP e `injecao.py` mede a injeção de SQL, o log das
tentativas e o bloqueio automático do IP.

```bash
python3 bancada/seguranca/porta.py     # 16 casos: acesso, política, bloqueio
python3 bancada/seguranca/injecao.py   #  8 casos: injeção, log, firewall
```

Variáveis: `PHX_SEG_PORTA` (padrão 6600, e a bateria usa `+1`, `+2` e `+3`) e
`PHX_INJ_PORTA` (padrão 6605, com o REST em `+1`). Os binários saem de
`target/release/`; nenhuma das duas compila nada. Os temporários vivem em
`/tmp/phx-f6-<pid>` e são apagados no fim, e todo servidor é derrubado **pelo
PID guardado** — nunca por `pkill -f`, porque o processo ao lado pode ser de
outro agente.

## Por que elas existem

Pedidos do dono, 07/09/2026: *«bateria de teste de acesso, segurança e
tentativas de acessar o banco pela porta TCP/IP relations se está seguro»* e
*«testes que devem bloquear tentativas de SQL injector, log das tentativas e
bloqueio automático do ip no firewall colocando uma regra de blacklist»*.

O `docs/SEGURANCA.md` já **descreve** as quatro camadas e o tradutor de
`crates/phxsql-sql/` já **analisa** o texto em vez de concatená-lo. Nada disso
é prova: descrição é o que deveria acontecer. As respostas em
`docs/pdf/respostas/W.md` e `Y.md` saem daqui, e cada número delas veio de uma
corrida datada.

## O que cada bateria mede

`porta.py` — token ausente e token errado; a contagem leve até o
`tentativas_ate_bloquear` da política, com o `blacklist.json` e o `acessos.log`
gravados; as três violações **graves** (comando proibido, base proibida,
travessia de diretório) bloqueando na primeira; a whitelist que nunca bloqueia;
JSON malformado, linha acima do teto de 128 MiB, `op` inexistente; as três
operações de replicação com a lista de réplicas vazia e preenchida; o login
obrigatório havendo cadastro e a permissão por base; a queda de conexão soltando
a reserva de carga; o que o `acessos.log` registra de fato; a senha em claro
procurada com `grep` no diretório inteiro; e as portas em LISTEN do processo,
tiradas de `/proc/net/tcp` porque **não há `nmap`, `ss` nem `netstat`** nesta
máquina.

`injecao.py` — doze injeções clássicas pela op `sql` (aspa solta, comando
empilhado, `UNION`, comentário, `'; EXEC`, byte nulo), com a contagem de tabelas
e de linhas **antes e depois**; o mesmo texto gravado como valor pelo protocolo
e achado de volta por um `SELECT`; injeção no nome da coluna, da tabela e da
base; a porta REST (`POST /v1/sql`); o que o `acessos.log` e o Profiler
registram da tentativa; quantas injeções por minuto passam sem que ninguém
bloqueie; e a regra de firewall, com a exportação nos quatro formatos.

## As quatro coisas que a primeira corrida ensinou, e que ficaram nos scripts

1. **A bateria bloqueia a si mesma.** Tudo sai de `127.0.0.1`, e a política não
   abre exceção para o localhost. Provado o bloqueio, o caso seguinte já não
   conecta — daí o `phxsqld --desbloquear` entre um caso e outro. E o caso 1,
   que manda um pedido sem token, **já gastou uma tentativa leve**: contar do
   zero no caso 2 acusaria o motor pelo erro da bateria.
2. **Fechar o soquete não fecha o descritor.** `socket.makefile()` segura o fd;
   fechar só o soquete deixa o servidor sem ver o fim da conexão. É a lição do
   `BULKINSERT`, e é o que faz o caso 5 medir a queda de verdade.
3. **Ausência de resposta é um resultado.** A linha acima do teto derruba a
   conexão sem responder e sem deixar rastro. A bateria olha o `acessos.log`
   *depois* e conta a falta, em vez de registrar «erro de leitura» e seguir.
4. **Firewall de verdade não se testa em contêiner compartilhado.** Um
   `iptables -I INPUT -s 127.0.0.1 -j DROP` aqui derruba a rede de quem está ao
   lado. O comando configurado é um `touch` inofensivo — e ele prova mais do que
   o `iptables` provaria: com `; rm alvo.txt` **dentro** do argumento, o arquivo
   criado tem esse nome inteiro e `alvo.txt` sobrevive. Se houvesse `sh -c`,
   teria sumido.

## O que a corrida de 07/09/2026 mediu (commit `a56a165`)

| bateria | casos | passou | achado |
|---|---|---|---|
| `porta.py` | 16 | 13 | 3 |
| `injecao.py` | 8 | 6 | 2 |

Os cinco achados, com o número, estão nas duas respostas
(`docs/pdf/respostas/W.md` e `Y.md`), na seção **O que NÃO existe**. Em uma
frase cada: a linha acima do teto não deixa rastro nenhum; `replicas_autorizadas`
vazia — o padrão — entrega o diário a quem tem o token; `aplicar` escreve num
servidor em `somente_leitura`; o `acessos.log` guarda a recusa mas não o texto
tentado; e **nada** bloqueia por injeção de SQL — 311.250 tentativas por minuto,
uma conexão nova por tentativa, sem um bloqueio.
