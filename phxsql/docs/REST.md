# O webservice REST, e a especificação que sai do código

O pedido foi *«servidor webservice swagger phxsql para Windows, Linux, macOS,
IoT, Android e iOS»*. Este documento responde as três partes: **o que existe**,
**como a especificação se mantém honesta** e **onde roda de verdade** — com a
recusa escrita onde não roda, em vez de silêncio.

```bash
# no config.json, e as duas portas nascem DESLIGADAS
"rest": {
  "ligado": true,
  "bind": "127.0.0.1:6000",
  "nome": "Loja",
  "database": "loja",
  "tabelas": ["clientes", "pedidos"],
  "token": "",
  "swagger_ligado": true,
  "swagger_bind": "127.0.0.1:7000"
}
```

```bash
curl -X POST http://127.0.0.1:6000/v1/ler \
  -H 'Authorization: Bearer SEU-TOKEN' \
  -H 'X-Sessao: a-sessao-do-login' \
  -H 'Content-Type: application/json' \
  -d '{"tabela":"clientes","rowid":1}'
```

---

## 1. A decisão que sustenta tudo: a especificação **sai da tabela de despacho**

<!-- rest:operacoes:inicio (gerado por docs/dossie/numeros-do-projeto.py) -->
São **120 operações**. Uma especificação OpenAPI digitada à mão envelhece na primeira operação nova e **passa a mentir com aparência de documento oficial** — que é pior do que não ter documento nenhum. (Este número também: ele já disse 113 aqui, 108 no `PENDENCIAS.md` e 121 no catálogo lido por uma auditoria externa. Hoje sai da constante `OPERACOES`.)
<!-- rest:operacoes:fim -->

Esta casa já pagou duas vezes pela mesma causa, e as duas estão escritas no
`CLAUDE.md`: o rodapé do dossiê publicou **780 KiB** quando a interface tinha
**1.032**, porque o gerador trazia uma lista de arquivos copiada; e o conferidor
de idiomas media cinco sextos da tela, porque a lista de fontes era digitada.

**A regra: quando um gerador depende de uma lista, a lista tem de sair do
código.**

O `openapi.json` é gerado por `crates/phxsql-server/src/rest.rs` a partir de
`catalogo::OPERACOES`, que por sua vez já é travado contra o `match` do
`despachar` pelo teste `o_catalogo_e_o_despachar_sao_a_mesma_lista`. Nada é
digitado duas vezes: resumo, parâmetros, obrigatoriedade, exemplo, permissão
exigida e «escreve?» saem todos do mesmo lugar que o servidor usa.

### As duas guardas, uma para cada lado do laço

O precedente é o `conferidor.rs`, que já faz exatamente isso para os textos de
tela. Sem as **duas**, a especificação vira chave morta: alguém lê, acredita, e
nada corresponde.

| guarda | o que ela reprova | prova real, com o defeito reposto |
|---|---|---|
| `toda_operacao_do_despachar_esta_na_especificacao` | operação que existe e a spec não documenta | braço novo no `executar` → `o despachar atende estas operacoes e a especificacao nao as documenta: ["operacao_nova_sem_documento"]` |
| `toda_rota_da_especificacao_existe_no_despachar` | rota documentada que não existe | rota escrita à mão no `paths` → `a especificacao documenta a rota "/exportar_para_o_sap" e o despachar nao atende essa operacao` |

As duas leem o **texto do `servidor.rs`** para extrair a lista real de
operações. Ler o fonte é feio e é honesto: Rust não deixa perguntar a um
`match` quais braços ele tem, e a alternativa — a lista escrita à mão num
segundo lugar — é exatamente a duplicação que esta frente existe para não ter.

E há a terceira, no soquete: `bancada/rest/provar.py` percorre **as 113 rotas
que o servidor serviu** e exige que cada uma seja roteada. A especificação e o
servidor vivo, não a especificação e uma constante.

### O que a especificação carrega além do padrão

Três extensões, porque a OpenAPI não tem onde dizer o que quem integra pergunta
primeiro — e as três saem do **mesmo lugar que o portão lê**:

- `x-phxsql-permissao` — a atividade exigida (`ler`, `alterar`, `administrar`…),
  de `Atividade::da_operacao`;
- `x-phxsql-escreve` — de `OPS_ESCRITA`, a lista que o modo somente-leitura usa;
- `x-phxsql-apelidos` — os outros nomes que a mesma operação atende
  (`systables` e `sistabelas` são a mesma rota).

---

## 2. O Swagger UI: a tensão, e o número que a resolveu

O Swagger UI é um pacote JavaScript de vários MB. O binário do `phxsqld` tem
**7.296.144 bytes** antes desta frente, e isso importa porque ele roda em ARM e
cabe em placa pequena.

**Medido**, baixando o `swagger-ui-dist` 5.17.14 de verdade e compilando com ele
embutido:

| opção | binário | delta | offline? |
|---|---:|---:|---|
| hoje, sem visualizador | 7.296.144 | — | — |
| **embutir** `swagger-ui.css` + `swagger-ui-bundle.js` | 8.900.968 | **+1.604.824 (+22,0%)** | sim |
| embutir + o preset standalone | 9.131.896 | +1.835.752 (+25,2%) | sim |
| **apontar para CDN** | 7.296.144 | +0 | **não — quebra o IoT** |
| **este explorador**, escrito aqui | 7.515.816 | +219.672 (+3,0%)¹ | sim |

¹ os +219.672 são a frente REST **inteira** — o módulo, o gerador da
especificação, a seção do `config.json`, as 40 chaves novas de idioma e o
explorador. O explorador sozinho são os três arquivos de `ui/explorador.*`:
**13.629 bytes**, e a medição mostrou que `include_str!` cresce o binário byte a
byte (embutir os três arquivos do Swagger deu +1.835.752 para 1.835.117 bytes de
arquivo — os 635 restantes são o código que os serviria).

**A escolha: escrever o visualizador aqui.** Ele é **118× menor** que o mínimo
funcional do Swagger UI e não quebra o uso offline, que é justamente o caso da
placa. A CDN custaria zero byte e entregaria uma página em branco no IoT.

### O que este explorador faz, e o que ele não faz

Ele lista as 113 operações com busca, mostra parâmetros com tipo e
obrigatoriedade, marca quem grava, diz a permissão exigida, e monta o `curl`
pronto a partir do exemplo que já vem na especificação.

**Não há «Try it out», e a ausência é decisão.** A porta que documenta e a porta
que executa são separadas de propósito; um console executável aqui exigiria
abrir CORS da porta REST para a origem do explorador — uma folga de segurança
que ninguém pediu, só para não copiar um `curl`.

### Ele não é obrigatório — e por que não virou opção de compilação

A porta dele é **outra** (7000 de fábrica) e tem interruptor próprio: quem sobe
numa placa liga `rest.ligado` e deixa `rest.swagger_ligado` desligado; a 7000
simplesmente não abre.

Foi a medição que dispensou uma *feature* de compilação. A 1,53 MiB ela seria
obrigatória; a 13,3 KiB, um `--no-default-features` a mais para todo mundo
manter não se paga. **Padrão: as duas portas desligadas** — ver a §3.

---

## 3. As duas portas nascem DESLIGADAS

Regra pétrea: guarda nova entra **pedida**, não imposta — e porta nova também.
Um servidor que já roda hoje **não pode passar a expor porta nenhuma** só
porque alguém trocou o binário: isso seria abrir superfície de ataque sem
ninguém pedir.

O teste que mais importa é o do comportamento **velho**, e ele existe em dois
níveis:

- `config_sem_a_secao_rest_nao_escuta` (unitário) — sem a seção, `ligado` e
  `swagger_ligado` são `false`, os endereços de fábrica são do próprio
  computador, nada é estreitado e a seção ausente **não vira aviso de campo
  desconhecido**. Com o defeito reposto (`Rest::default` nascendo ligado) ele
  reprova com *«o REST subiu sem ninguem pedir»*;
- o passo 1 de `bancada/rest/provar.py` — um `phxsqld` real, com um
  `config.json` em que a palavra `rest` **não aparece**, e a prova de que as
  portas de fábrica **6000 e 7000 estão fechadas** enquanto a porta de dados
  dele responde. Teste unitário não prova que uma porta não abriu; o sistema
  operacional prova.

As duas portas entram na mesma conta de colisão de endereço que `bind` e
`web.bind` já tinham — e a porta **desligada** não colide com nada, senão quem
deixou o campo apontando para a porta de dados sem ligar o REST ficaria sem
servidor.

---

## 4. Autenticação: nada foi inventado

São os dois mecanismos que já existiam, e a especificação os descreve como
`securitySchemes`:

| esquema | o que é | onde |
|---|---|---|
| `token` (http bearer) | a chave da **porta**, não a identidade | `Authorization: Bearer …` |
| `sessao` (apiKey) | a **identidade**, do `login` | `X-Sessao: …` |

O caminho completo, e a senha não viaja:

```
POST /v1/desafio  {"usuario":"adm"}      → sal, iterações, nonce, e a sessão
POST /v1/login    {"usuario":"adm","prova":"…","nonce_cliente":"…"}   (X-Sessao)
POST /v1/ler      {"tabela":"clientes","rowid":1}                     (X-Sessao)
```

A sessão é **a mesma máquina da interface web** — `web.sessao_minutos` manda nas
duas —, e não uma segunda. Quando o REST entrou, as duas metades da sessão
(carregar do cabeçalho e acertar depois do despacho) saíram de dentro do `/api`
e viraram `sessao_do_cabecalho` e `acertar_sessao`, chamadas pelos dois
caminhos: duas cópias seriam duas ideias do que é estar logado, e a que alguém
esquecesse de atualizar viraria a porta dos fundos.

### `rest.token`: dois segredos, e qual é a diferença

- **vazio (o padrão)** — o `Bearer` é o **mesmo `token` do protocolo**. Um
  segredo só, zero confusão.
- **preenchido** — o `Bearer` tem de ser esse, e **o token do protocolo deixa de
  abrir o REST**. Serve para entregar uma credencial de webservice sem entregar
  o token mestre, e para revogá-la sem tocar em nenhum cliente da porta 5000.

O que ele **não** é: não é identidade e não é poder. É a chave da porta da rede,
igual à outra; quem entra continua sendo quem faz `login`, e o direito continua
saindo do cadastro. E ele **não abre a porta 5000** — a separação vale nos dois
sentidos, senão trocar o segredo do REST não fecharia nada.

O token **não se edita pela tela**, pelo mesmo motivo que já mantinha `token`
fora de `CAMPOS_EDITAVEIS`: campo que carrega credencial se edita no arquivo,
para que uma sessão tomada não troque a fechadura. A tela diz **se existe um**;
nunca qual é. E a resposta de `config` não o carrega — `senha nunca em texto
puro` vale para segredo de porta também.

### O limite, escrito e não escondido

> **`Bearer` sobre HTTP em claro entrega o token a quem escuta o fio**, em todo
> pedido. Não há TLS aqui. As saídas honestas são as mesmas da §6 do
> `SEGURANCA.md`: um proxy que termine TLS à frente, ou um túnel (WireGuard,
> IPSec).

Isso está em três lugares de propósito: neste documento, na `description` do
esquema `token` dentro do `openapi.json` (quem gera cliente pode nunca abrir
este arquivo), e no rodapé da seção na tela de configuração. Há teste travando
que a frase continua na especificação.

E a §7 do `SEGURANCA.md` explica por que a cifra do fio **não** cobre isto: o
navegador — e o cliente HTTP — fala TLS ou fala claro, e um aperto de mão em
JavaScript seria teatro.

---

## 5. O filtro de tabelas: **ele só estreita**

O dono pediu uma tela com *nome do serviço, qual banco, quais tabelas e qual
token*. A lista de tabelas é a peça perigosa, e há um jeito certo e um errado.

> **A lista de tabelas é um filtro que só ESTREITA, aplicado ANTES do portão e
> nunca no lugar dele.** Tabela fora da lista não existe para o REST — responde
> como inexistente, sem vazar que existe. Tabela dentro da lista continua
> passando pelo `despachar` e pelo direito do usuário **exatamente como hoje**.
> **Nunca alarga:** se o usuário não tem direito, estar na lista não dá.

O jeito errado seria a lista virar um segundo sistema de permissão. Aí passam a
existir duas verdades sobre direito de acesso, e no primeiro desacordo entre
elas alguém ganha acesso que ninguém concedeu — que é exatamente a família dos
quatro furos contados no `docs/USUARIOS.md`.

São **dois** testes, porque são dois erros opostos:

| teste | erro que ele impede |
|---|---|
| `tabela_fora_da_lista_nao_aparece` | não estreita |
| `tabela_na_lista_ainda_pede_direito_do_usuario` | **alarga** |

O segundo é o que mais importa, e é o mesmo padrão do `sem_regra_de_tabela_nada_muda`.
No unitário ele é provado pela **forma**: `estreitar` não recebe usuário, não
recebe sessão, e o pedido que sai dele é byte a byte o que entrou. A outra
metade é provada pelo soquete, com um usuário que **não pode ler `salarios`**
pedindo `salarios`, que **está** na lista: sai `403 ACESSO_NEGADO`.

E há o `sem_lista_de_tabelas_nada_muda`: lista vazia — o padrão — não estreita
nada.

### Qual campo o filtro olha, e por que não é «o campo `tabela`»

O furo clássico desta casa: o portão passou a olhar um campo novo e havia
operação sem esse campo. `juntar` guarda as tabelas em `a.tabela`/`b.tabela` e
`unir` guarda numa **lista** — bastaria pedir a tabela escondida como o lado B.

Então a varredura é **estrutural**: desce a árvore inteira do pedido e recolhe
todo valor sob uma chave chamada `tabela`, `tabelas` ou `tabela_ref`. Ela pega os
três casos sem saber que eles existem, e pega o próximo sem ninguém alterar
nada. A guarda `rest-filtro-so-o-campo-tabela` repõe o defeito (olhar só o
primeiro nível) e o teste cai.

`destino` fica de fora **de propósito**: a mesma chave é o nome de uma tabela no
`duplicar_tabela` e uma **pasta** no `backup`, e barrar uma pasta por não estar
na lista de tabelas seria recusar por um motivo falso.

### `rest.database`

Vazio não estreita nada. Preenchido, faz duas coisas e nenhuma delas alarga:
preenche o `database` do pedido que veio sem ele (conveniência de quem publica
UM banco) e recusa, como inexistente, o pedido que nomeia outro. O preenchimento
só acontece em operação que **tem** esse parâmetro, e quem diz isso é o
catálogo — não uma lista escrita ao lado.

---

## 6. Como uma operação vira uma rota

`POST /v1/<operação>`, corpo JSON com o resto do pedido. Corpo vazio vale como
`{}`.

**O caminho manda.** Um corpo que traga um `"op"` diferente do caminho é
**recusado**, e não ignorado: o caminho é o que o operador vê no log do proxy e
nas regras do firewall, e deixar o corpo mandar faria um `POST /v1/ping` ser um
`excluir` no servidor e continuar um `ping` em tudo o que observa de fora.

O `despachar` é o mesmo — **não há um segundo caminho de dados**. O que o REST
faz antes dele é: resolver a operação pelo caminho, pôr o token no pedido,
aplicar o estreitamento. Tudo o que decide **acesso** continua acontecendo num
lugar só, e o REST anota no **mesmo log de acessos** que a porta 5000.

### Os códigos HTTP

Derivados da faixa do código de erro do PhxSql, e não de uma lista por variante
— erro novo cai na faixa certa sozinho:

| erro | faixa | HTTP |
|---|---|---|
| `ESQUEMA_INVALIDO`, `TIPO_INVALIDO` | 2000 | 400 |
| token que esta porta não aceita | — | **401** |
| `ACESSO_NEGADO` | 4001 | 403 |
| `NAO_ENCONTRADO` (inclusive a tabela que o REST não expõe) | 3001 | 404 |
| `DUPLICADO`, `CONFLITO` | 3002/3004 | 409 |
| `REDIRECIONA` (escreveu na réplica) | 4003 | **421** |
| `CANCELADO` | 6001 | 499 |
| `EM_CARGA`, `SPARE_EM_ESPERA` | 4002/4004 | 503 |
| resto | 1000/5000 | 500 |

401 e 403 querem dizer coisas diferentes e o cliente trata cada uma de um jeito:
401 é *«a porta não abriu, mande credencial»*; 403 é *«você entrou e não pode
isso»*. O `despachar` usa o mesmo tipo de erro para as duas, então o REST
escolhe o número olhando se o token conferia — **quem decide continua sendo o
`despachar`**, isto só escolhe o número.

O envelope da resposta é o mesmo do `/api`: `ok`, `op`, `resultado`/`erro`,
`codigo`, `nome`, `classe`, `repetir`, `ms`, e `sessao` quando há uma.

---

## 7. Onde roda — por plataforma, com honestidade

| plataforma | estado | como |
|---|---|---|
| **Linux** x86-64 | funciona | é o binário de sempre |
| **Windows** | funciona | mesmo binário, mesma seção do `config.json` |
| **IoT ARM64 / ARMv7** | **funciona, e está medido** | ver abaixo |
| **macOS** | **não há alvo** | ver abaixo |
| **Android / iOS** | o REST **não é a forma certa** | ver abaixo |

### ARM: «compilou» não basta, então foi executado

`bancada/rest/arm.sh` sobe o binário `aarch64-unknown-linux-musl` sob
`qemu-aarch64-static` — o mesmo caminho que `bancada/arm/provar.sh` já usava — e
exercita o REST de verdade. Rodada desta frente:

```
phxsqld ARM64 no ar sob emulacao, RSS 12836 kB
  OK     o REST responde em ARM64
  OK     a especificacao e gerada na placa  -- 113 rotas, 418965 bytes
  OK     o explorador desenha em ARM64
  OK     e o portao continua fechando  -- 401
```

O binário ARM64 desta rodada tem **7.298.600 bytes**. Em **ESP32 de 4 MB de
flash não cabe**, e não é o REST que não cabe — o binário já não cabia. Placas
com sistema de arquivos (Raspberry Pi, roteador com OpenWrt e disco, qualquer
ARM com Linux) rodam, e o RSS de 12,8 MB sob emulação é o teto de cima: emulação
custa mais do que o alvo real.

### macOS: não há alvo, e não dá para compilar aqui

Não há alvo `*-apple-darwin` instalado, e não daria para provar mesmo que
houvesse: o SDK da Apple só existe em macOS, e a licença dele não permite
redistribuir para compilação cruzada. **O código não tem nada de específico de
plataforma no caminho do REST** — é `std::net::TcpListener` e nada mais —, então
a expectativa é que compile e rode; mas *expectativa* não é *medição*, e esta
casa não escreve «entregue» para o que ninguém executou.

O que destrava: alguém com um Mac rodar `cargo build --release` e
`bancada/rest/provar.py`.

### Android e iOS: o REST não é a forma certa, e isso não é limitação nossa

- **iOS proíbe** um app manter socket escutando para outros apps consumirem. Um
  servidor HTTP dentro do app só serve a ele mesmo, e o sistema o suspende ao
  sair do primeiro plano.
- **Android mata processo em segundo plano** por política de bateria. Um
  webservice que o sistema derruba a cada poucos minutos não é um webservice.

O caminho nos dois é a **biblioteca embutida**, que outra frente está
construindo (`phxsql-ffi`): o app chama o motor por FFI, no mesmo processo, sem
porta nenhuma. As duas peças se encaixam assim:

```
   celular                        servidor / placa
  ┌──────────────────┐           ┌──────────────────────┐
  │ app              │           │ phxsqld              │
  │  └ phxsql-ffi ───┼── rede ──▶│  ├ porta 5000 (JSON) │
  │     (motor local)│           │  ├ porta 6000 (REST) │
  └──────────────────┘           │  └ porta 7000 (docs) │
                                 └──────────────────────┘
```

O app guarda o dado local pela FFI e conversa com o servidor central pelo REST
quando há rede — que é o desenho de sincronia que o DbLink já faz entre
servidores. **Nada do `phxsql-ffi` foi duplicado aqui.**

---

## 8. A tela

A seção entra na tela de Configurações que já existe, com os sete campos
(`ligado`, `bind`, `nome`, `database`, `tabelas`, `swagger_ligado`,
`swagger_bind`) e um rodapé que diz o estado do token e os dois limites.

**Todo texto passa pela fábrica de idiomas.** Os rótulos e as explicações dos
campos novos saem de chaves `tela.cfg_rest_*` da `FABRICA_TELA`, nos seis
idiomas — e não dos dicionários portugueses cravados do `index.html`, que são
herança de quando a tela era só em português. O caminho novo é o mapa
`AJUSTE_NA_FABRICA`, e campo que entrar daqui para a frente entra por ele. A
catraca do `conferidor.rs` **não subiu**: continua em 1.996.

E foi exercitado no navegador, porque ler o código não acha o que o CSS global
faz com componente novo (`testes-web/prova-rest.mjs`, 6 passos):

```
  OK    a secao aparece com os sete campos  -- 7 campos
  OK    o textarea nao foi esmagado nem esticado pelo CSS global  -- 260x66px
  OK    salvar grava no config.json
  OK    recarregar mostra o gravado, e sem gritar o dado  -- clientes|pedidos|itens
  OK    trocar o idioma traduz a secao na hora
  OK    o token nunca aparece -- a tela so diz que nao ha um proprio
```

A lista de tabelas é um `<textarea>`, **uma por linha e não separadas por
vírgula**: vírgula obrigaria a decidir o que fazer com o espaço em volta e com o
nome que tem vírgula dentro. Ela é o primeiro campo do tipo `lista` da tela, e o
tipo recusa o que não for lista de textos — sem isso, gravar
`"rest.tabelas": "clientes"` passaria, o leitor cairia em vazio calado, e vazio
aqui quer dizer **expõe tudo**.

---

## 9. O que a bancada achou, e que ler o código não acharia

**A recusa da lista negra nunca chegava em quem foi barrado.**

Cinco tokens errados bloqueiam o IP — o comportamento certo. O pedido seguinte
devia trazer o `403` com o motivo e o prazo. Recebia `Connection reset by peer`.

A causa não é do PhxSql, é do TCP: fechar um soquete que ainda tem bytes por ler
no buffer de recepção faz o sistema mandar um **RST**, e o RST **descarta a
resposta em voo**. As recusas por lista negra e por IP fora dos permitidos
respondem **antes** de ler o pedido, de propósito — e é justamente por isso que
o corpo fica sem ler.

Vale para as **três** portas HTTP, e valia **desde que a interface web existe**.
A recusa cuidadosamente redigida e traduzida nunca alcançava quem mais precisava
dela.

O conserto é `http::escoar`: lê e descarta o que o cliente ainda estava mandando
antes de fechar, com prazo curto e teto igual ao de um pedido legítimo — escoar
não dá a quem foi barrado o direito de fazer o servidor ler mais do que ele já
leria de qualquer um. O primeiro teto (16 KiB) não bastava: um corpo de 20 KB
ainda deixava resto, e resto é RST.

**Prova real, com o defeito reposto** (`bancada/rest/provar.py`, passo 13):

```
com escoar:  OK     a recusa por bloqueio chega  -- 403 {"ok":false,"erro":"bloqueado desde … ate …"}
sem escoar:  FALHA  ConnectionResetError: [Errno 104] Connection reset by peer
```

A guarda `rest-fecha-sem-escoar` está no catálogo marcada **REDUNDANTE** com o
motivo escrito: nenhum teste de unidade sente isto e nenhum poderia — o RST é do
sistema operacional. É a mesma lição do `BULKINSERT`: **teste unitário não prova
queda de conexão, soquete prova.**

### E uma armadilha da própria bancada, que ela pagou duas vezes

A varredura das 113 rotas foi escrita três vezes:

1. com o token bom **e sessão** — `servico_parar` e `esvaziar_lixeira`
   executaram, e a bancada derrubou o servidor que estava medindo;
2. com o token **errado** — a quinta sonda bloqueou o IP, e da sexta em diante a
   bancada media o bloqueio, não o roteamento;
3. com o token bom e **sem sessão** — num servidor com cadastro, toda operação
   que pede direito para em «faça login» antes de tocar em nada, e as seis que
   não pedem (`ping`, `login`, `desafio`, `quem_sou`, `sair`, `catalogo`) são
   justamente as que não fazem estrago.

E o passo da lista negra ficou por **último** de propósito: a lista vale para o
servidor inteiro, então depois dele nem a porta de dados atende esta máquina —
nem para pedir `desbloquear`.

---

## 10. O que ficou de fora, e o motivo

- **TLS.** Não entra, e não é esquecimento: é a mesma decisão da §7 do
  `SEGURANCA.md`. TLS sem dependência externa significaria escrever X.509, ASN.1
  e uma pilha de handshake aqui dentro — e criptografia de transporte escrita em
  casa é pior que nenhuma. O caminho é o proxy à frente.
- **CORS.** Nenhum cabeçalho `Access-Control-Allow-*` é emitido. Um navegador em
  outra origem não chama esta porta, e isso é o padrão certo: abrir CORS é
  decisão de quem implanta, e ela precisaria de uma lista de origens no
  `config.json` que ninguém pediu ainda.
- **`GET` para leitura.** Toda operação é `POST`, inclusive `ler` e `varrer`.
  Um `GET /v1/ler?tabela=…` poria os parâmetros na URL, e URL vai para o log do
  proxy e para o histórico do navegador. É o mesmo motivo pelo qual o token não
  entra em query.
- **Paginação por cabeçalho (`Link`, `Range`).** As operações já têm `pular` e
  `limite` no corpo, e uma segunda forma de dizer a mesma coisa é uma segunda
  verdade.
- **Métricas em formato Prometheus.** A telemetria já responde pelo protocolo; a
  tradução para outro formato é uma frente própria.
- **macOS**, pelo motivo da §7.
- **Try it out no explorador**, pelo motivo da §2.

---

## 11. Onde está cada coisa

| arquivo | o que é |
|---|---|
| `crates/phxsql-server/src/rest.rs` | o gerador da especificação, o roteador, o estreitamento, o explorador e 20 testes |
| `crates/phxsql-server/src/config.rs` | a seção `Rest`, e os testes do comportamento velho |
| `crates/phxsql-server/src/servidor.rs` | `subir_rest`, `subir_swagger`, `atender_rest`, `api_rest` |
| `crates/phxsql-server/ui/explorador.{html,css,js}` | o visualizador |
| `bancada/rest/provar.py` | 57 passos por soquete, com cliente HTTP próprio |
| `bancada/rest/arm.sh` | o mesmo REST sob `qemu-aarch64-static` |
| `testes-web/prova-rest.mjs` | a seção da tela, exercitada no navegador |
| `bancada/guardas/catalogo.py` | seis defeitos repostos desta frente |
