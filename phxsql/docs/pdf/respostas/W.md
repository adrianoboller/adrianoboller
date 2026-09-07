# W) bateria de teste de acesso, segurança e tentativas de acessar o banco pela porta TCP/IP relations se está seguro

## Resposta curta

A bateria existe e é nova: `bancada/seguranca/porta.py`, **16 casos pelo
soquete** contra um `phxsqld` de pé — 13 passaram, 0 falharam, **3 são
achados**. O que está provado: token ausente e token errado recusam com código;
cinco tentativas leves bloqueiam o IP e gravam `blacklist.json` e `acessos.log`;
comando proibido, base proibida e travessia de diretório bloqueiam na
**primeira**; a whitelist nunca bloqueia; JSON torto não derruba nada; a queda
da conexão solta a reserva de carga; a senha não aparece em lugar nenhum
(38 arquivos varridos, zero ocorrências); e cada servidor escuta **uma** porta,
só em `127.0.0.1`. O que **não** está seguro tem nome e número: sem TLS a cifra
do fio protege contra escuta passiva e nada mais; `replicacao.replicas_autorizadas`
nasce **vazia**, e com ela vazia quem tem o token leva o diário — e, num Source
com `imagem_da_linha` ligada, leva a linha inteira e ainda **escreve** de volta
com o servidor em `somente_leitura`; e uma linha acima do teto de 128 MiB
derruba a conexão **sem uma linha de log**.

## Exemplo exercitado

Corrida de **07/09/2026, 16:33:48 UTC**, commit `a56a165`, portas 6600–6603.

**1. O token, e a contagem que a política manda.** A política em vigor é a de
fábrica: `tentativas_ate_bloquear: 5`.

```text
[1] pedido SEM token
    {"ok":false,"op":"ping","erro":"[SP000025] acesso negado: token invalido","codigo":4001,"nome":"ACESSO_NEGADO","classe":"acesso","sprint":"SP000025","repetir":false,"ms":0}

[2] token errado ate o limite da politica (5) -> bloqueio do IP
    tentativa leve 2..5: {"ok":false,"op":"ping","erro":"[SP000025] acesso negado: token invalido","codigo":4001,"nome":"ACESSO_NEGADO", …
    tentativa leve 6:    {"ok":false,"op":"conexao","erro":"[SP000025] acesso negado: bloqueado desde 2026-09-07 16:33:49,265 ate 2026-09-07 17:33:49,265 por token invalido (p …
    blacklist.json: [{"ip": "127.0.0.1", "desde": "2026-09-07 16:33:49,265", "desde_ms": 1788798829265,
                      "ate": "2026-09-07 17:33:49,265", "ate_ms": 1788802429265, "motivo": "token invalido",
                      "comando": "ping", "tentativas": 5, "firewall": false}]

[2b] o bloqueio entra no acessos.log
    {"quando": "2026-09-07 16:33:49,266", "quando_ms": 1788798829266, "ip": "127.0.0.1", "porta_origem": 54808,
     "op": "conexao", "usuario": "", "autenticado": false, "ok": false, "ms": 0,
     "erro": "bloqueado desde 2026-09-07 16:33:49,265 ate 2026-09-07 17:33:49,265 …
```

Da sexta em diante a recusa muda de `"op":"ping"` para `"op":"conexao"`: o IP na
lista é barrado **antes** do token, como a §4 do `SEGURANCA.md` promete. A
tentativa 1 é o caso 1 — o pedido sem token já é uma violação leve, e a bateria
conta a partir dali em vez de acusar o motor pelo erro dela.

**2c. As três graves bloqueiam na primeira**, e a resposta diz que bloqueou:

```text
comando proibido: {"ok":false,"op":"reindexar","erro":"[SP000025] acesso negado: operacao reindexar esta proibida neste servidor; o IP foi bloqueado", …
    -> motivo 'comando proibido pela politica', tentativas 1
base proibida:    {"ok":false,"op":"varrer","erro":"[SP000025] acesso negado: a base financeiro esta proibida neste servidor; o IP foi bloqueado", …
    -> motivo 'base proibida pela politica', tentativas 1
travessia:        {"ok":false,"op":"varrer","erro":"[SP000025] acesso negado: database \"../../etc\" nao e um nome; o IP foi bloqueado", …
    -> motivo 'tentativa de travessia de diretorio', tentativas 1
```

**3. A whitelist vence**, e o controle positivo prova que a sonda enxerga:

```text
8 tokens errados -> blacklist.json bloqueios = []
CONTROLE POSITIVO, pedido legitimo depois: {"ok":true,"op":"ping","resultado":{"phxsql":"0.18.0","papel":"isolado", …
```

**4. Pedido torcido.** Quatro formas de mandar lixo, quatro erros nomeados, e a
conexão continua viva para o pedido seguinte:

```text
JSON malformado:        {"ok":false,"op":"?","erro":"[SP000018] esquema invalido: JSON invalido na posicao 25: fim inesperado","codigo":2001, …
nao e JSON:             {"ok":false,"op":"?","erro":"[SP000018] esquema invalido: JSON invalido na posicao 0: caractere inesperado 'i'", …
lista em vez de objeto: {"ok":false,"op":"ping","erro":"[SP000025] acesso negado: token invalido","codigo":4001, …
byte nulo no meio:      {"ok":false,"op":"?","erro":"[SP000018] esquema invalido: JSON invalido na posicao 10: caractere de controle cru no texto", …
[4c] op inexistente:    {"ok":false,"op":"xyzzy","erro":"[SP000018] nao encontrado: operacao desconhecida: xyzzy","codigo":3001, …
                        bloqueios: []
```

**4b. A linha gigante** — o teto do fio é `TETO_DO_REGISTRO = 128 MiB`, em
`crates/phxsql-core/src/fio.rs`:

```text
1 MiB:          {"ok":true,"op":"ping","resultado":{"phxsql":"0.18.0", …
134218794 bytes: <A CONEXAO FECHOU SEM RESPONDER>   (0.43s)
RSS do servidor: 6080 kB antes, 8204 kB depois -- o processo continua de pe
linhas novas no acessos.log por causa dela: 0
bloqueios apos a tentativa: []
```

**4d. As três operações de replicação.** Com a lista de réplicas **vazia** — o
padrão de fábrica — o token sozinho leva o diário:

```text
[4d-i] servidor de fabrica: eventos devolvidos: 1
    [{"operacao": "inclusao", "rowid": 1, "versao": 1, "carimbo_ms": 1788798830083, "usuario": 0, "origem": 0, "imagem": ""}]

[4d-ii] Source com imagem_da_linha ligada
    imagem do evento 1: 76000000000100000000000000416c7665730000000000000000000000000000… (248 caracteres hex)
    config_gravar somente_leitura=true: {"ok":true,"op":"config_gravar","resultado":{"gravado":true, …
    CONTROLE POSITIVO, `inserir` com o servidor trancado:
        {"ok":false,"op":"inserir","erro":"[SP000025] acesso negado: servidor em modo somente leitura","codigo":4001, …
    `aplicar` com a mesma imagem:
        {"ok":true,"op":"aplicar","resultado":{"recebidos":1,"aplicados":1,"posicao":2,"erro":null},"ms":0}
    linhas na tabela depois: 2 -> [{"rowid":1,"id":1,"nome":"Alves","cidade":"Blumenau","softdeleted":false,"rownum":1},
                                   {"rowid":2,"id":1,"nome":"Alves","cidade":"Blumenau","softdeleted":false,"rownum":2}]
```

Com a lista **preenchida** com outro IP, o portão 2a-bis fecha as três — e
fecha para o supervisor também, porque não é permissão, é origem:

```text
login root: {"ok":true,"op":"login","resultado":{"id":1,"nome":"root","login":"root", …,"nivel":"adm …
replicar:   {"ok":false,"op":"replicar","erro":"[SP000025] acesso negado: este ip nao esta em replicacao.replicas_autorizadas", …
posicao:    {"ok":false,"op":"posicao","erro":"[SP000025] acesso negado: este ip nao esta em replicacao.replicas_autorizadas", …
aplicar:    {"ok":false,"op":"aplicar","erro":"[SP000025] acesso negado: este ip nao esta em replicacao.replicas_autorizadas", …
CONTROLE POSITIVO ping: {"ok":true,"op":"ping", …
```

**4e. Havendo cadastro, o token não é identidade.** `ping` passa sem login
porque não toca dado; `varrer` não passa:

```text
ping sem login:   {"ok":true,"op":"ping", …
varrer sem login: {"ok":false,"op":"varrer","erro":"[SP000025] acesso negado: faca login antes: {\"op\":\"login\",\"usuario\":...,\"senha\":...}", …
senha errada:     {"ok":false,"op":"login","erro":"[SP000025] acesso negado: usuario ou senha invalidos","codigo":4001, …
senha certa:      {"ok":true,"op":"login","resultado":{"id":4,"nome":"Carlos Consulta","login":"carlos", …,"nivel":"nenhum", …
CONTROLE POSITIVO varrer: {"ok":true,"op":"varrer","resultado":{"registros":1, …
inserir:          {"ok":false,"op":"inserir","erro":"[SP000025] acesso negado: carlos nao tem permissao de inserir em loja.clientes", …
```

**5. Conexão aberta e abandonada.** O soquete e o `makefile` são fechados os
dois — fechar só o soquete deixaria o fd aberto e o servidor nunca veria o fim,
que é o defeito que a prova do `BULKINSERT` já pagou:

```text
reserva:          {"ok":true,"op":"bulkinsert","resultado":{"bulkinsert":true,"database":"loja","tabela":"clientes","reservada":true,"expira_em_s":1800,"prazo_min":30},"ms":0}
CONTROLE POSITIVO antes: {"ok":false,"op":"inserir","erro":"[SP000012] tabela em carga: loja.clientes esta reservada para carga pela ligacao 19 …
depois da queda:  {"ok":true,"op":"inserir","resultado":{"rowid":2,"registros":2},"ms":2}
```

**6. O que o `acessos.log` registra de fato.** A frase da casa é «o log de IP
registra o acesso, não o sucesso» — medido, ele registra **os dois**:

```text
linhas: 24   ok:true 7   ok:false 17
campos vistos: ['autenticado','codigo','database','erro','ip','ms','ok','op','porta_origem','quando','quando_ms','tabela','usuario']
uma de SUCESSO: {"quando":"2026-09-07 16:33:51,650","quando_ms":1788798831650,"ip":"127.0.0.1","porta_origem":54932,"op":"inserir","usuario":"","autenticado":true,"ok":true,"ms":2,"database":"loja","tabela":"cliente …
uma de FALHA:   {"quando":"2026-09-07 16:33:51,150", …,"op":"inserir","usuario":"","autenticado":true,"ok":false,"ms":0,"database":"loja","tabela":"clientes","erro":"[SP000 …
```

**7. A senha**, procurada com `grep` no diretório inteiro depois da bateria:

```text
arquivos varridos: 38
arquivos com a senha em claro: NENHUM
respostas do protocolo com a senha: 0
o config.json guarda: pbkdf2-sha256$210000$c2dce7de5269fc9b35ce4ec0f…
```

**8. Que portas estão abertas.** **Não há `nmap`, `ss` nem `netstat` nesta
máquina** — a lista sai de `/proc/net/tcp` cruzado com `/proc/<pid>/fd`, que é o
mesmo dado sem a ferramenta:

```text
ferramentas ausentes nesta maquina: ['nmap', 'ss', 'netstat']
servidor padrao    (pid 18945): ['127.0.0.1:6600']
servidor whitelist (pid 18949): ['127.0.0.1:6601']
servidor fonte     (pid 18953): ['127.0.0.1:6603']
servidor replica   (pid 18959): ['127.0.0.1:6602']
/proc/net/tcp6 existe? False (sem IPv6 neste conteiner)
porta 6609, que nenhum config abriu: recusada: ConnectionRefusedError: [Errno 111] Connection refused
```

**Placar:** `16 casos -- 13 passou, 0 falhou, 3 achado`.

## O que NÃO existe, e é dispensa registrada

**Não há TLS, e a cifra do fio protege contra escuta PASSIVA e nada mais.** É a
§7 do `SEGURANCA.md`, e a frase é dela: com `cifra_fio.exigir` desligada — o
padrão — «cifra pedida é cifra que o atacante ativo apaga do pedido». Contra
quem está no meio só vale `exigir: true` **mais** o pino da chave
(`phxsqld --chave-do-fio`) no cliente; um sem o outro não fecha. Esta bateria
**não exercitou o túnel** — quem o exercita é `bancada/cifra-do-fio/` —, e por
isso a frase acima é a decisão documentada e não uma medição desta rodada.

**`replicacao.replicas_autorizadas` nasce vazia, e vazia libera todos.** Medido:
com a lista vazia, `replicar` devolveu o diário a quem só tinha o token. Num
Source de verdade — `imagem_da_linha: true`, que toda replicação tem — a mesma
chamada devolveu **a linha inteira em hexadecimal**, 248 caracteres, e o
`aplicar` a gravou de volta. A recusa existe e funciona (caso 4d-iii); o que não
existe é ela vir ligada. É «guarda nova entra pedida, não imposta», e o preço
está agora medido em vez de suposto.

**`aplicar` grava num servidor em `somente_leitura`.** Não é defeito escondido:
está escrito no `catalogo.rs` (`o_aplicar_nao_e_ferramenta_mcp`) que ele fica
fora de `OPS_ESCRITA` de propósito, porque uma réplica somente-leitura precisa
aplicar. A consequência é que, num servidor **isolado**, `somente_leitura` não é
uma tranca contra quem tem o token: `inserir` recusou com `servidor em modo
somente leitura` e `aplicar` gravou a linha na mesma sessão. Quem tranca essa
porta é `replicas_autorizadas`, e ela está vazia por padrão.

**A linha acima do teto não deixa rastro.** 134.218.794 bytes derrubaram a
conexão em 0,43 s, sem resposta, **sem uma linha no `acessos.log`** e sem contar
como violação. O teto protege a memória — o RSS foi de 6.080 kB para 8.204 kB e
o processo ficou de pé —, mas quem opera não vê o evento: o único ramo que anota
o erro de leitura é o do canal **cifrado** (`servidor.rs`, no `Err(e)` do laço
de conexão). Quem quiser barrar isso não tem contador para pendurar.

**Não há bloqueio por FAIXA.** É a §7: o bloqueio é IP a IP, e banir um `/24`
exige o firewall de fora (a exportação da §5.1 ajuda). A *whitelist* aceita
CIDR; a blacklist, não.

**As tentativas vivem em memória.** Reiniciar o servidor zera os contadores das
leves e das graves; os bloqueios já gravados sobrevivem, porque estão no
`blacklist.json`.

**`127.0.0.1` não tem exceção implícita**, e isso é decisão. Foi o que fez esta
bateria bloquear a si mesma e precisar do `phxsqld --desbloquear` entre um caso
e outro — o mesmo caminho do operador local, que nunca fica trancado de verdade.

## Como se refaz

```bash
python3 bancada/seguranca/porta.py
```

Usa `target/release/phxsqld` (não compila), sobe quatro servidores nas portas
6600–6603 (`PHX_SEG_PORTA` muda a faixa), cria e apaga `/tmp/phx-f6-<pid>` e
derruba cada servidor pelo PID guardado. O que cada caso mede está em
`bancada/seguranca/LEIA-ME.md`.
