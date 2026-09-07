# Y) testes que devem bloquear tentativas de SQL injector, log das tentativas e bloqueio automático do ip no firewall colocando uma regra de blacklist

## Resposta curta

A bateria existe e é nova: `bancada/seguranca/injecao.py`, **8 casos pelo
soquete** — 6 passaram, 0 falharam, **2 são achados**. As doze injeções
clássicas não passam: o tradutor de `crates/phxsql-sql/` **analisa** o texto e
reserializa o pedido, então o segundo comando morre com `sobrou "DROP" depois do
fim do comando; um comando por vez`, e a tabela `clientes` fica com as **mesmas
tabelas e as mesmas linhas** antes e depois. Os dois achados são o que o pedido
supõe e o motor não tem: o `acessos.log` registra a **recusa** mas não o **texto
tentado** — quem guarda o texto é o Profiler, que vem desligado —, e **nada
bloqueia por injeção**: 10.375 tentativas em 2 s, uma conexão nova a cada uma
(311.250 por minuto), e o `blacklist.json` ficou vazio. A regra de firewall
existe, funciona e roda **sem shell**, mas ela só é chamada quando algo já
bloqueou por outro motivo.

## Exemplo exercitado

Corrida de **07/09/2026, 16:37:17 UTC**, commit `a56a165`, portas 6605 (dados) e
6606 (REST). Política de fábrica: `comandos_proibidos: []`, `bases_proibidas: []`.

**1. As doze clássicas pela op `sql`.** Cada uma com o que voltou:

```text
aspa solta / tautologia   SELECT * FROM clientes WHERE nome = '' OR '1'='1'
  -> {"ok":false,…,"erro":"[SP000018] esquema invalido: SQL, coluna 43: o WHERE aceita UMA comparacao. Duas exigiriam interseccao de rowids, e nao ha planejador que decida por qual indic …

segundo comando: DROP     SELECT * FROM clientes; DROP TABLE clientes; --
  -> {"ok":false,…,"erro":"[SP000018] esquema invalido: SQL, coluna 25: sobrou \"DROP\" depois do fim do comando; um comando por vez","codigo":2001, …

segundo comando: DELETE   SELECT * FROM clientes; DELETE FROM clientes
  -> {"ok":false,…,"SQL, coluna 25: sobrou \"DELETE\" depois do fim do comando; um comando por vez", …

UNION SELECT              SELECT * FROM clientes UNION SELECT * FROM clientes
  -> {"ok":false,…,"SQL, coluna 24: sobrou \"UNION\" depois do fim do comando; um comando por vez", …

comentario /* */ no meio  SELECT /* comentario */ * FROM clientes
  -> {"ok":true,"op":"sql","resultado":{"sql":"SELECT /* comentario */ * FROM clientes","op":"varrer", …

comentario como separador SELECT * FROM clientes/**/WHERE/**/nome='Alves'
  -> {"ok":true,…,"op":"buscar","notas":["indice porNome escolhido pelo WHERE … 

'; EXEC                   SELECT * FROM clientes WHERE nome = 'x'; EXEC xp_cmdshell('dir')
  -> {"ok":false,…,"SQL, coluna 42: sobrou \"EXEC\" depois do fim do comando; um comando por vez", …

comentario -- no fim      SELECT * FROM clientes WHERE nome = 'Alves' -- e o resto
  -> {"ok":true,…,"op":"buscar", …

DROP direto               DROP TABLE clientes
  -> {"ok":false,…,"SQL, coluna 6: DROP TABLE e a operacao excluir_tabela do protocolo, que exige repetir o nome no campo \"confirmar\"", …

empilhado com ; solto     SELECT * FROM clientes ;;; DROP TABLE clientes
  -> {"ok":false,…,"SQL, coluna 25: sobrou \";\" depois do fim do comando; um comando por vez", …

byte nulo no texto        SELECT * FROM clientes\x00
  -> {"ok":false,…,"SQL, coluna 23: caractere '\\0' nao faz parte da linguagem", …

aspa escapada como DADO   SELECT * FROM clientes WHERE nome = 'x'' OR ''1''=''1'
  -> {"ok":true,…,"op":"buscar","notas":["indice porNome escolhido pelo WHERE … 
       (a aspa dobrada e DADO: o motor foi BUSCAR pelo texto no indice, nao executar nada)

CONTROLE POSITIVO, SELECT legitimo:
  {"ok":true,"op":"sql","resultado":{"sql":"SELECT nome FROM clientes LIMIT 2","op":"varrer", …

tabelas/linhas ANTES:  (['clientes'], 2)
tabelas/linhas DEPOIS: (['clientes'], 2)
```

Três coisas que esta lista mostra e que uma resposta redigida esconderia. Os
quatro comandos empilhados morrem **na mesma frase** — «sobrou X depois do fim do
comando; um comando por vez» —, e a mensagem diz a **coluna**, porque quem
analisa sabe onde parou. Os comentários `/* */` e `--` **passam**, e devem
passar: comentário é sintaxe legítima, e recusá-lo seria confundir léxico com
ataque. E o `DROP TABLE` sozinho não é recusado por ser perigoso: é recusado por
não existir nessa camada, e a mensagem diz por onde se faz (`excluir_tabela`,
com `confirmar`).

**1b. A aspa vira dado, e o dado volta inteiro.** O mesmo texto de ataque foi
gravado como **valor** pelo protocolo e depois procurado por um `SELECT` com a
aspa dobrada:

```text
gravado pelo protocolo: "'; DROP TABLE clientes; --"
SQL: SELECT * FROM clientes WHERE nome = '''; DROP TABLE clientes; --'
linhas achadas: 1 -> [{"rowid": 3, "id": 3, "nome": "'; DROP TABLE clientes; --", "cidade": "x", "softdeleted": false, "rownum": 3}]
tabelas: ['clientes']   registros: 3
```

**2. Pelo protocolo JSON**, nos quatro lugares em que um motor que concatena
texto quebraria:

```text
valor:  {"ok":true,"op":"inserir","resultado":{"rowid":4,"registros":4},"ms":0}
coluna: {"ok":false,"op":"inserir","erro":"[SP000018] tipo invalido: coluna \"cidade'); DROP TABLE clientes; --\" nao existe em clientes","codigo":2002, …
tabela: {"ok":false,"op":"varrer","erro":"[SP000018] nao encontrado: nenhum volume de clientes; DROP TABLE clientes.reg em …/dados/loja","codigo":3001, …
base:   {"ok":false,"op":"varrer","erro":"[SP000018] nao encontrado: database loja; DROP DATABASE loja nao existe em …/dados","codigo":3001, …
tabelas/linhas ANTES:  (['clientes'], 3)
tabelas/linhas DEPOIS: (['clientes'], 4)   (a linha nova E a do valor envenenado)
```

O nome envenenado aparece **dentro** da mensagem de erro, como nome de arquivo e
de pasta procurados — e não como comando. É a prova de que ali não há
concatenação: o texto virou chave de busca no disco, não frase para alguém
executar.

**3. Pela porta REST.** Sim, **o REST recebe SQL**: `POST /v1/sql` é uma rota
como qualquer outra, porque a especificação sai da tabela de despacho. E passa
pelo mesmo `despachar`:

```text
CONTROLE POSITIVO POST /v1/sql legitimo:   200 {"ok":true,"op":"sql","resultado":{"sql":"SELECT nome FROM clientes LIMIT 1","op":"varrer", …
POST /v1/sql com injecao:                  400 {"ok":false,"op":"sql","erro":"[SP000018] esquema invalido: SQL, coluna 25: sobrou \"DROP\" depois do fim do comando; um comando por vez", …
POST /v1/sql com token errado:             401 {"ok":false,"op":"sql","erro":"[SP000025] acesso negado: token invalido","codigo":4001, …
POST /v1/ping com op=excluir_tabela no corpo:
                                           400 {"ok":false,"op":"ping","erro":"[SP000018] esquema invalido: o caminho pede a operacao \"ping\" e o corpo traz \"op\":\"excluir_tabela\"; no REST quem manda e o caminho -- tire o campo do co …
tabelas/linhas ANTES:  (['clientes'], 4)
tabelas/linhas DEPOIS: (['clientes'], 4)
```

**4. O log das tentativas.** A mesma injeção, olhada nos dois lugares:

```text
acessos.log: {"quando": "2026-09-07 16:37:17,407", "quando_ms": 1788799037407, "ip": "127.0.0.1",
              "porta_origem": 55544, "op": "sql", "usuario": "", "autenticado": true, "ok": false,
              "ms": 0, "database": "loja",
              "erro": "[SP000018] esquema invalido: SQL, coluna 25: sobrou \"DROP\" depois do fim do comando; um co …
o texto tentado esta no acessos.log? False

profiler:    {"serial": 1, "quando": "2026-09-07 16:37:17,407", "ip": "127.0.0.1", "usuario": "",
              "op": "sql", "database": "loja", "tabela": "", "bytes": 108,
              "pedido": "{\"op\":\"sql\",\"database\":\"loja\",\"texto\":\"SELECT * FROM clientes; DROP TABLE clientes; --\",\"token\":\"***\"}",
              "ms": 0, "ok": false, "erro": "[SP000018] esquema invalido: SQ …
o texto tentado esta no profiler?    True
o token aparece em claro no profiler? False
```

O `"token":"***"` no meio do pedido é a lei da casa cumprindo: o Profiler
**analisa e reserializa** em vez de recortar, e o campo tapado continua tapado
mesmo com o pedido escrito de qualquer jeito.

**5. Bloqueio automático por injeção — a medição.** Dois caminhos, dois
segundos cada, com o `blacklist.json` olhado antes e depois:

```text
pela MESMA conexao:        25067 tentativas em 2s (752010 por minuto), 25067 recusadas
uma CONEXAO por tentativa: 10375 tentativas em 2s (311250 por minuto), 10375 recusadas
bloqueios antes: []   depois: []
o IP continua entrando: {"ok":true,"op":"ping","resultado":{"phxsql":"0.18.0","papel":"isolado", …
```

**6. A regra de firewall.** O bloqueio veio pelo caminho de fábrica — cinco
credenciais erradas —, e o comando configurado carrega `; rm alvo.txt` **dentro
do argumento** de propósito:

```text
blacklist.json: [{"ip": "127.0.0.1", "desde": "2026-09-07 16:37:21,409", "ate": "2026-09-07 17:37:21,409",
                  "motivo": "token invalido", "comando": "ping", "tentativas": 5, "firewall": true}]
arquivos criados pelo comando: ['marca-127.0.0.1.txt; rm alvo.txt']
alvo.txt sobreviveu? True
```

Um arquivo só, com o nome inteiro, e a vítima de pé: **não há shell**, e o
`{ip}` foi trocado depois de o endereço ser validado. `"firewall": true` na
lista é o servidor dizendo que a regra chegou a ser aplicada.

E a exportação da §5.1, com o IP ativo, nos quatro formatos:

```text
texto     -> '127.0.0.1\n'
iptables  -> 'iptables -I INPUT -s 127.0.0.1 -j DROP\n'
nftables  -> 'add element inet filter phxsql_bloqueados { 127.0.0.1 }\n'
fail2ban  -> 'fail2ban-client set phxsql banip 127.0.0.1\n'
```

**A bateria NÃO aplica a regra**, e a decisão está escrita nela: esta sessão roda
como root (`euid=0`), então o `iptables` funcionaria — e um
`iptables -I INPUT -s 127.0.0.1 -j DROP` num contêiner compartilhado derruba a
rede de quem está ao lado. Quem tem o privilégio revisa o texto acima e aplica;
é exatamente para isso que a exportação existe.

**Placar:** `8 casos -- 6 passou, 0 falhou, 2 achado`.

## O que NÃO existe, e é dispensa registrada

**Nada bloqueia por injeção de SQL, e este é o achado principal.** As duas
gravidades da §3 são comando/base proibidos (grave, bloqueia na primeira) e
credencial errada (leve, conta na janela). Erro de sintaxe não é nenhuma das
duas, e o número mede o tamanho da folga: **10.375 tentativas em 2 s abrindo uma
conexão nova a cada uma — 311.250 por minuto — sem um único bloqueio**, e o
`ping` seguinte passou.

**A proposta, com o número, e sem implementar nada.** Contar o erro de sintaxe
SQL **com padrão de injeção** como violação **leve**: N por janela, no mesmo
contador de `tentativas_ate_bloquear` que já existe, e desligada de fábrica
(guarda nova entra pedida, não imposta). Padrão de injeção quer dizer o que o
motor já sabe distinguir sem casar texto: a recusa `sobrou "X" depois do fim do
comando` — que é o comando empilhado e **só** ele —, e não «erro de sintaxe» em
geral.

E por que **não** pode ser grave: **um erro de digitação do operador não pode
derrubar o operador.** É a lição do pedido 203, e ela já custou uma gravação
inteira: uma réplica mal configurada tentou login em laço, o master a tratou
como ataque e bloqueou `127.0.0.1` por uma hora — derrubando junto a sessão da
tela, porque a réplica e o navegador do operador saíam do mesmo IP. Grave
bloqueia na primeira; com o operador legítimo digitando SQL na tela — e um
`SELECT * FROM clientes;` com o ponto-e-vírgula sobrando é o erro mais comum que
existe —, isso trancaria o dono do banco antes de trancar qualquer atacante, e
trancaria o IP inteiro por causa de um processo. Leve, com N por janela, cobra
reincidência em vez de cobrar desatenção.

O contra-argumento que precisa ser medido antes: quem manda SQL por esta porta é
sobretudo **driver**, e driver não erra sintaxe por desatenção — erra por
versão. Um cliente antigo mandando uma cláusula que esta camada ainda não tem
geraria recusa repetida e legítima, e bloquearia um sistema inteiro. É o mesmo
motivo pelo qual a contagem tem de olhar `sobrou … um comando por vez` e não o
código `2001` genérico.

**O `acessos.log` não guarda o texto tentado.** Medido: `False`. Ele guarda
`ip`, `quando`, `op`, `usuario`, `ok`, `codigo` e a **mensagem da recusa** — que
já basta para um filtro de `fail2ban` (`failregex = ^.*"ip":"<HOST>".*"ok":false.*$`,
§5.1), mas não diz **o que** a pessoa tentou. Quem quiser o texto liga o
Profiler, e ele **vem desligado**, exige `administrar` e guarda em memória com
teto. Não há hoje um log de segurança que grave a tentativa em disco por padrão.

**A regra de firewall não é o bloqueio.** É a §5, e vale repetir: um IP na lista
é recusado **dentro** do servidor, sem firewall, sem root e sem poder falhar. O
comando é um extra, desligado de fábrica; se falhar, o bloqueio continua valendo
e a falha vira aviso. Ligar o `iptables` embutido exige `CAP_NET_ADMIN` ou root
para o `phxsqld`, o que é aumento de privilégio real — a §5.1 existe justamente
para não pedir isso.

**Não há bloqueio por faixa.** Banir um `/24` inteiro exige o firewall de fora; a
exportação acima ajuda. A *whitelist* aceita CIDR, a blacklist não.

**Um erro cru que escapa, achado de passagem.** `SELECT * FROM
information_schema.tables` devolve `[SP000010] erro de E/S: No such file or
directory (os error 2)` — sem caminho, então não vaza estrutura, mas manda
procurar arquivo em vez de dizer que o catálogo não é uma tabela desta camada. É
a mesma família do erro que a lei da casa já mandou envolver em vez de vazar;
fica aqui nomeado, não consertado.

## Como se refaz

```bash
python3 bancada/seguranca/injecao.py
```

Usa `target/release/phxsqld` (não compila), sobe um servidor na 6605 com o REST
na 6606 (`PHX_INJ_PORTA` muda a faixa), cria e apaga `/tmp/phx-f6-<pid>` e
derruba o servidor pelo PID guardado. O firewall configurado é um `touch`
inofensivo dentro do diretório temporário, e a bateria **não** aplica regra de
`iptables` nenhuma. O que cada caso mede está em `bancada/seguranca/LEIA-ME.md`.
