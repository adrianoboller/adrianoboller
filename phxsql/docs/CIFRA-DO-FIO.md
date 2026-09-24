# A cifra do fio: aperto de mão estilo Noise

> Este documento foi escrito **antes** do código, e é ele que o código
> obedece. Onde os dois discordarem, um dos dois está errado — e a regra da
> casa é que o número (ou o comportamento) medido ganha do texto.

O dono escolheu, entre três alternativas, o **aperto de mão estilo Noise**:
X25519 para a troca de chaves, HKDF-SHA256 para derivar, e o
ChaCha20-Poly1305 que já existe aqui para cifrar. **Não é TLS**, e navegador
não fala isto. Esse limite é aceito, e a §5 diz onde ele dói.

---

## 0. O que já existia, e o que faltava

Conferido contra vetor oficial, antes desta rodada:

| Peça | Onde | Vetor |
|---|---|---|
| ChaCha20-Poly1305 | `crates/phxsql-core/src/cifra.rs` | RFC 8439 §2.3.2, 2.4.2, 2.5.2, 2.6.2, 2.8.2 |
| XChaCha20 / HChaCha20 | idem | draft-irtf-cfrg-xchacha-03 §2.2.1 e A.3.1 |
| SHA-256 | `crates/phxsql-core/src/hash.rs` | FIPS 180-4 |
| HMAC-SHA256 | idem | RFC 4231 |
| PBKDF2-HMAC-SHA256 | idem | RFC 6070 (adaptado ao SHA-256) |
| Ed25519 | `crates/phxsql-core/src/ed25519.rs` | RFC 8032 |
| Desafio-resposta | `crates/phxsql-core/src/desafio.rs`, `docs/SEGURANCA.md` §2 | — |

Faltava **só a troca de chaves**. Esta rodada acrescenta três peças e nada
mais:

| Peça nova | Onde | Vetor |
|---|---|---|
| X25519 | `crates/phxsql-core/src/x25519.rs` | RFC 7748 §5.2 (dois), §5.2 iterado (1 e 1.000), §6.1 |
| HKDF-SHA256 | `crates/phxsql-core/src/hkdf.rs` | RFC 5869 anexo A.1, A.2, A.3 |
| O aperto e a camada de registro | `crates/phxsql-core/src/fio.rs` | — (composição nossa; ver §9) |

O X25519 **reaproveita a aritmética de corpo do `ed25519.rs`** — os mesmos
`Fe = [u64; 5]` em base 2^51, o mesmo `fe_mul`, o mesmo `fe_inverso`. Escrever
um segundo corpo finito ao lado do primeiro seria dobrar a superfície de erro
justamente na parte que ninguém revisa duas vezes.

---

## 1. Qual padrão Noise

### A decisão

**`Noise_NX_25519_ChaChaPoly_SHA256`**, com o **pino da chave estática do
servidor no cliente, no estilo `known_hosts` do SSH**. O desafio-resposta de
usuário **continua exatamente como está** e passa a correr *dentro* do túnel.

Em notação Noise:

```
NX:
  -> e
  <- e, ee, s, es
```

Duas mensagens, um ida-e-volta. A estática do servidor viaja **cifrada** na
mensagem 2, e a etiqueta final dessa mensagem só fecha se quem respondeu tiver
mesmo a privada correspondente — é isso que autentica o servidor.

### Por que NX, e não NK

`NK` (o cliente já sabe a estática do servidor, e ela não viaja) é o padrão
mais limpo — mas ele **pressupõe** que o cliente já tenha a chave. Quando não
tem, não há mensagem para trocar: o `NK` não tem por onde aprender. O `NX`
resolve isso com um único caminho de código que atende os dois casos:

* **cliente com pino**: recebe a estática, compara com o pino, e **aborta** se
  não bater. O efeito de segurança é o do `NK` — a mesma autenticação do
  servidor, a mesma recusa diante de quem está no meio.
* **cliente sem pino (TOFU)**: aceita a estática da primeira vez e a guarda. É
  o SSH no primeiro `ssh host`, com a mesma virtude e o mesmo defeito.

E o defeito do TOFU tem de ficar escrito, não escondido: **quem estiver no meio
na PRIMEIRA conexão vence para sempre**, porque o pino que o cliente guarda é o
do atacante. O TOFU protege da escuta a partir da segunda conexão; não protege
da primeira. Por isso a recomendação operacional é **pinar de fora**:
`phxsqld --chave-do-fio` imprime a chave pública do servidor, e ela vai para a
configuração do cliente pelo mesmo canal por onde já vai o token.

### Por que a estática do cliente ficou de fora (XX e IK descartados)

`XX` e `IK` dão **autenticação mútua por chave**: o cliente também tem uma
estática, e o servidor a verifica no aperto. O que isso daria, e do que abri
mão:

1. **Recusar o estranho antes do login.** Hoje, e com o NX, qualquer um que
   alcance a porta completa o aperto e só então esbarra no token e no
   desafio-resposta. Com IK/XX, quem não tem chave registrada nem chega ao
   primeiro pedido. **É uma perda real**, e a compensação é que os dois portões
   que já existem continuam onde estavam — o estranho não ganha nada por ter
   completado o aperto.
2. **Amarrar a credencial ao canal (*channel binding*).** Com estática de
   cliente, dá para exigir que a prova do login seja feita sobre a mesma chave
   que fechou o túnel. O hash da transcrição (`fio::Transporte` o expõe) **já é
   consumido** pelo desafio-resposta quando o cliente pede `amarrar_canal` — ver
   §10. O que a estática de cliente acrescentaria por cima é *exigir* a
   amarração antes do login, em vez de aceitá-la como pedido; hoje esse degrau é
   uma decisão de implantação que ainda não virou opção de configuração.

E o que a estática de cliente **custaria**, que é o motivo de não entrar agora:
um ciclo de vida de credencial inteiro e novo — gerar, distribuir, cadastrar,
revogar, girar, uma chave por cliente. O projeto já tem um ciclo desses
funcionando (usuário, PBKDF2, desafio-resposta, e o segundo fator Ed25519 do
`docs/SEGURANCA.md`). Ter **dois** não é o dobro de segurança; é o dobro de
lugares onde a revogação pode ser esquecida.

### Substitui ou convive com o desafio-resposta?

**Convive, e não substitui.** São perguntas diferentes:

* o aperto responde **«com que máquina eu estou falando»**;
* o desafio-resposta responde **«quem é a pessoa do outro lado»**.

O aperto NX não sabe nada sobre usuários e não deve saber. O que ele muda para
o desafio-resposta é o ambiente: a prova, os nonces e o token deixam de andar
em claro. E há um ganho de graça: **hoje o token de serviço viaja em texto puro
em todo pedido** (é o campo `"token"` de cada linha JSON). Dentro do túnel, não
viaja mais.

---

## 2. O rebaixamento — este é O ponto

### O conflito, dito sem enfeite

A regra da casa é **«guarda nova entra pedida, não imposta»**: quem manda o
campo novo ganha a garantia, quem não manda continua como antes. Ela é pétrea
porque proteção que quebra todo cliente antigo não é proteção, é estrago.

Só que **cifra pedida é cifra que o atacante ativo apaga do pedido**. Se o
cliente diz «quero cifrar» e o homem-no-meio corta essa linha e responde «este
servidor não sabe cifrar», o cliente rebaixa para claro e o atacante lê tudo.
Contra quem está no meio, cifra opcional vale **zero**.

Os dois lados da regra são verdadeiros ao mesmo tempo. A saída não é escolher
um: é **separar quem decide**.

### A decisão

Uma opção de configuração no **servidor**:

```json
"cifra_fio": {
  "exigir": false
}
```

* **`exigir: true` — e ele é o PADRÃO desde 18/09/2026** (ordem do dono, pedido
  370: *«A comunicação deve obrigatoriamente ser cifrada.»*). O servidor recusa
  **qualquer** pedido que não venha dentro do túnel. A recusa é uma linha JSON
  em claro, com erro nomeado, e a conexão fecha em seguida — cliente velho
  recebe um erro que sabe exibir, em vez de um silêncio. Provado pelo soquete
  em `o_cliente_velho_sem_o_escape_escrito_e_recusado_com_o_motivo`, que sobe
  de um `config.json` **sem** a seção (teste que escreve o campo não prova o
  padrão dele) e mede o dano, não só o veredito.
* **`exigir: false` — o ESCAPE ESCRITO.** O servidor aceita claro e aceita
  cifrado, exatamente como antes. Quem precisa de transição escreve o campo, e
  aí é escolha escrita em vez de omissão — o mesmo padrão do `"verificar":
  false` da chave que nasce conferida. É o teste que **não mudou de significado
  com a virada**: `o_escape_escrito_deixa_o_cliente_em_claro_entrar`.
* **E as portas HTTP entraram no mesmo interruptor** (pedido 370): com
  `exigir` ligado, `/api`, `/v1`, `/mcp` e o explorador da especificação
  recusam, porque HTTP é texto puro. O escape delas é `"atras_de_proxy": true`
  — o TLS é do proxy reverso, e a receita está em `docs/SEGURANCA.md` §7.0 e
  §7.1.

### Concordo com a solução? Sim, e este é o argumento

O ponto que a torna correta não é técnico, é de **quem sabe o quê**:

* o **cliente** não pode decidir, porque ele não sabe se está falando com o
  servidor ou com o atacante. Quem exige do lado do cliente exige de quem
  responde, e quem responde pode ser o atacante;
* o **servidor** pode decidir, porque ele sabe onde está. Quem sobe um PhxSql
  numa rede em que não confia sabe disso; quem sobe num laboratório também.

Ligar `exigir` é uma **decisão de implantação**, e decisão de implantação é do
administrador, não do protocolo. É o mesmo desenho do `ips_permitidos`, do
`somente_leitura` e do firewall: o servidor é quem conhece a própria rede.

E a mudança de comportamento que `exigir: true` provoca é **visível e
imediata** — o cliente antigo para de funcionar no primeiro pedido, com uma
mensagem que diz o motivo. Isso é o oposto de uma guarda que quebra em
silêncio: ninguém liga isso por engano e descobre daqui a três semanas.

### O que vale com `exigir` DESLIGADO — em palavras claras

> **Com `exigir` desligado, o túnel protege contra escuta PASSIVA e nada
> mais.**
>
> Quem apenas **grava** o tráfego (um espelho de porta, um Wi-Fi aberto, um
> provedor no caminho) não lê nada do que passou pelo túnel.
>
> Quem **modifica** o tráfego — o homem-no-meio de verdade — simplesmente
> impede o aperto de acontecer: apaga o pedido, ou responde que o servidor não
> sabe cifrar. O cliente rebaixa para claro e o atacante lê tudo. **A proteção
> vira zero.**
>
> Contra atacante ativo, só `exigir: true` **mais** o pino da chave do
> servidor no cliente. Um sem o outro não fecha: `exigir` sozinho garante que
> há um túnel, mas não com quem; o pino sozinho é rebaixado junto com o
> aperto.

Isso está repetido no `docs/SEGURANCA.md` §7 e na página de configuração,
porque é a frase que o documento não pode deixar o leitor adivinhar.

---

## 3. Disciplina do nonce

### Contador por direção

Depois do aperto, o `Split()` do Noise deriva **duas** chaves da mesma cadeia:
uma para cada direção. Cada uma carrega o próprio contador de 64 bits,
começando em zero, e o nonce de 96 bits do ChaCha20-Poly1305 é

```
nonce = 00 00 00 00 || n (8 bytes, little-endian)
```

que é a construção do Noise. O contador **sobe de um a cada registro** e nunca
volta. As duas direções têm chaves diferentes, então o mesmo `n` nos dois lados
não é reúso: o par (chave, nonce) é o que precisa ser único, e a chave já
difere.

Não há sorteio de nonce em lugar nenhum aqui — e é de propósito. Com 96 bits,
sortear tem risco de aniversário; com contador, o reúso exige o contador voltar,
que é coisa que a estrutura não faz. É o mesmo argumento que a `Sequencia` do
`cifra.rs` já usa para o `.log`.

### O esgotamento: **fecha**, não rechaveia

Quando o contador chega ao teto (`2^64 - 1`, que o Noise reserva e manda não
usar), a camada de registro **recusa cifrar e devolve erro**. A conexão morre.
Não há rechaveamento.

O argumento tem duas metades:

1. **O teto não é alcançável.** 2^64 registros a um registro por microssegundo
   são cerca de **584 mil anos** numa única conexão. Chegar lá não é uma carga
   de trabalho: é um defeito no contador. Fechar transforma o defeito em erro
   visível, que é o que se quer dele.
2. **Rechavear seria código que nunca roda.** Um `Rekey` precisa acontecer no
   **mesmo registro** dos dois lados; se um rechaveia e o outro não, tudo para
   de autenticar a partir dali — e essa dessincronia só apareceria no dia do
   estouro, ou seja, nunca, ou seja, sem ninguém ter exercitado. Código que só
   roda em condição inatingível é código que se degrada calado. Vale a mesma
   regra do resto do projeto: prefiro a recusa medida à sofisticação não
   exercitada.

Isso é **testado**, não afirmado: o teste do esgotamento força o contador para
o teto e confere que o `selar` recusa e que o `abrir` recusa
(`fio::testes::contador_no_teto_recusa_em_vez_de_repetir`).

---

## 4. Truncamento e repetição

### O hash da transcrição cobre o aperto inteiro

Como no Noise: `h` começa no nome do protocolo (que tem exatamente 32 bytes,
então entra como está, sem hash), recebe o prólogo, e depois **cada pedaço de
cada mensagem, na ordem** — a efêmera do cliente, a efêmera do servidor, a
estática cifrada, a carga cifrada. Cada AEAD do aperto usa o `h` corrente como
dado associado.

Consequência: **a etiqueta final da mensagem 2 só fecha se as duas mensagens
inteiras chegaram byte a byte como saíram.** Um bit mexido na efêmera do
cliente, um pedaço cortado, uma reordenação — qualquer um deles muda `h`, e a
etiqueta não confere. O aperto falha, e falhar aqui é fechar a conexão.

### A camada de registro distingue fim de fio cortado

Este é o ponto em que «não deu erro» não pode virar «deu certo».

Cada registro é `[tipo][conteúdo]` selado, e o tipo está **dentro** do texto
claro (portanto autenticado e invisível de fora):

| tipo | nome | o que é |
|---|---|---|
| `1` | `PEDIDO` | uma linha do protocolo JSON |
| `2` | `FIM` | fim de conversa — o *close_notify* daqui |

E o leitor tem três saídas, não duas:

| o que aconteceu no soquete | veredito |
|---|---|
| registro `FIM` e depois EOF | **fim limpo** |
| EOF **sem** ter visto `FIM` | **erro: fio cortado no meio** |
| linha sem `\n` no fim (EOF no meio de um registro) | **erro: registro truncado** |
| etiqueta não confere | **erro** |

Repetição, reordenação e supressão caem todas no mesmo lugar: o receptor abre
cada registro com **o contador que ele espera**, não com um contador que venha
no fio. Um registro repetido chega com o `n` errado e não autentica; um
registro suprimido desalinha todos os seguintes e nenhum autentica. Não há
janela, não há tolerância, não há reordenação aceita — é uma conexão TCP, e
TCP já entrega em ordem; o que sobra é ataque, e ataque aqui fecha a conexão.

O que **em claro** não dá para distinguir, e é por isso que isto é um ganho e
não um enfeite: hoje, uma conexão cortada no meio de uma resposta e uma
conexão encerrada de propósito chegam ao cliente do mesmo jeito — EOF. Dentro
do túnel, são dois vereditos diferentes.

---

## 5. Onde vale, e onde não vale

### Vale

* **A porta de dados (5000).** É o alvo principal: é por ela que passam o
  token, a prova do login e todos os dados.
* **O transporte da replicação.** A réplica é um cliente da porta de dados do
  source: `replica::Cliente` faz o mesmo aperto, e a origem em `config.json`
  ganha `cifra` e `chave_do_fio` (o pino). É a resposta ao item aberto de
  `docs/REPLICACAO.md` §13.
* **O driver ODBC.** Ele é um cliente comum da porta de dados, e aprendeu o
  mesmo aperto — reusando o `fio` do core, como a réplica. A connection string
  ganha `CIFRA` e `CHAVE_DO_FIO=<hex>` (o pino) — e **desde o pedido 373
  (18/09/2026) a cifra é o PADRÃO da receita**: quem não escreve nada fala
  cifrado, e `CIFRA=0` é o escape escrito. Ver `docs/ODBC.md` §1.1 e a §10 aqui.
* **O `Remoto` (o multi-servidor da interface).** `servidor::Remoto::cifrar`
  (`crates/phxsql-server/src/servidor.rs:556`) reaproveita o mesmo
  `fio::Iniciador` do core que a `replica::Cliente` usa — não é um segundo
  código de aperto. Liga quando o destino em `web.servidores[]` tem
  `"cifra": true`, padrão desde 18/09/2026 (§13, texto solto incluído). Ver
  §10. **Este item saiu de "Também não vale" nesta rodada** — dizia o
  contrário desde antes do §10 registrar o "FEITO", e as duas seções
  discordavam (pedido 382).
* **O DbLink para outro PhxSql.** `dblink/phx.rs:91` reaproveita o MESMO
  `replica::Cliente` da réplica — o terceiro consumidor do mesmo código de
  aperto, não um quarto. Cifra é o padrão desde 22/09/2026 (pedido 378, §13);
  nos outros dois motores do DbLink (MySQL(R), PostgreSQL(R)) isto **não**
  vale — ver "Também não vale", abaixo. Ver `docs/DBLINK.md` §"O fio do
  terceiro motor: o túnel, e ligado de fábrica".

### Não vale: a interface web

**A interface web NÃO ganha isto, e não é por falta de vontade.** O navegador
fala TLS ou fala claro; ele não tem como executar um aperto Noise antes do
`GET`. Dá para imaginar um aperto em JavaScript por cima do HTTP — e seria
teatro: o JavaScript que faria o aperto chega **pelo mesmo canal em claro** que
se quer proteger, então quem está no meio troca o script e pronto. Cifra cujo
código o atacante entrega não é cifra.

Para a porta web, as saídas honestas são duas, e nenhuma é esta:

1. **TLS de verdade**, terminado por um proxy à frente (nginx, Caddy) — é o que
   o `docs/SEGURANCA.md` §6 já recomenda;
2. **túnel** (WireGuard, IPSec, SSH), que é o que a §7 já dizia.

Escrever «o PhxSql agora cifra o tráfego» sem esta ressalva seria vender o que
não se entrega.

### Também não vale

* **O DbLink para MySQL(R) e PostgreSQL(R).** É protocolo alheio — não há
  aperto Noise do lado deles —, então `Motor::cifra_o_fio()` devolve `false`
  para os dois, e `cifra` ou `chave_do_fio` na declaração de uma ligação com
  esses motores é **erro na declaração**
  (`conferir_cifra_do_motor`, `crates/phxsql-server/src/dblink/mod.rs`), não
  um interruptor que fica sem efeito. Os sítios de rede desses dois motores —
  `crates/phxsql-server/src/pg/mod.rs:604` e
  `crates/phxsql-server/src/dblink/mysql.rs:452` — não implementam aperto
  nenhum. Ver `docs/DBLINK.md` §"O limite honesto: não há TLS".
* **Nada disto é TLS.** Não há certificado, não há cadeia, não há autoridade,
  não há revogação. A confiança é o pino, e o pino é responsabilidade de quem
  configura.

**O `Remoto`, o cluster e o DbLink para outro PhxSql deixaram de estar aqui.**
Os três passaram a cifrar quando se pede — estão na seção **Vale**, acima, e
em §10, §12 e §13. Esta seção dizia o contrário do `Remoto` até esta rodada,
enquanto a §10 já registrava "FEITO": duas seções do mesmo arquivo em
desacordo, e a errada era a que um leitor apressado encontrava primeiro
(achado do papel G, pedido 382, 22/09/2026). Resolvido pelo código: `Remoto`
tem `fn cifrar` (`servidor.rs:556`) usando `fio::Iniciador::comecar`, exatamente
como a `replica::Cliente` — o suporte já existia, só a prosa estava velha.

---

## 6. O formato no fio

### A moldura

O protocolo da porta 5000 é **JSON por linha**. Um registro cifrado ocupa
**uma linha**, em Base64:

```
<base64( cifrado || etiqueta )>\n
```

Base64, e não binário com prefixo de tamanho, porque assim **a moldura do
protocolo não muda**: o laço que lê o soquete continua sendo um `read_line`, e
todo o resto do servidor continua recebendo uma `String` com uma linha JSON
dentro. Uma mudança de moldura tocaria o laço de conexão, o cliente da
replicação, o `Remoto` e o ODBC de uma vez.

### O preço, medido — e ele **não** é «+33%»

Escrevi «+33%» aqui antes de medir, e estava errado para o caso que mais
acontece. Os 33% são a expansão do Base64 no **limite**; o que se paga de
verdade é a expansão *mais* 17 bytes fixos por registro (1 de tipo, 16 de
etiqueta), e num pedido curto os 17 bytes fixos pesam mais que a expansão.

Medido sobre registros selados de verdade, e o número **sai de um script**,
não daqui: `python3 bancada/cifra-do-fio/prova.py`, passo 9.

| o que passa | em claro | no fio | a mais |
|---|---:|---:|---:|
| um `ping` com token | 52 B | 93 B | **+78,8%** |
| uma inserção de uma linha | 168 B | 249 B | **+48,2%** |
| um lote de ~5 KiB | 5.001 B | 6.693 B | **+33,8%** |
| uma resposta de ~200 KiB | 200.001 B | 266.693 B | **+33,3%** |

Ou seja: **o pedido pequeno é o que paga caro**, e é justamente ele que o
protocolo mais faz. Quem precisar desses bytes de volta tem o caminho aberto —
trocar a moldura por tamanho binário mexe em `fio::Canal` e em mais nada —, mas
não é esta rodada, e agora o número que sustenta a decisão está aqui, medido,
em vez de arredondado de cabeça.

### O aperto, dentro do protocolo que já existe

A mensagem 1 vai como um pedido comum, para não precisar de moldura nova:

```json
{"op":"cifrar","e":"<base64 de 32 bytes>"}
```

e a resposta é uma resposta comum:

```json
{"ok":true,"op":"cifrar","resultado":{"m2":"<base64 de 96 bytes>"}}
```

Da linha seguinte em diante, **os dois lados falam registros**.

`cifrar` é atendido **antes do portão do token**, e isso é deliberado: o token
é justamente uma das coisas que o túnel existe para esconder; exigi-lo em claro
para abrir o túnel esvaziaria metade do ganho. O aperto não concede nada — quem
o completa continua tendo de passar por token, login e permissão, todos agora
por dentro. O `cifrar` fica **depois** dos portões que já valem para a conexão
(lista de bloqueio e `ips_permitidos`), e ele **é registrado no
`acessos.log`**, como qualquer operação.

Tamanhos: mensagem 1 = 32 bytes; mensagem 2 = 32 (efêmera) + 48 (estática
cifrada) + 16 (etiqueta da carga vazia) = **96 bytes**.

### A chave estática do servidor

Nasce **na primeira vez que alguém pede o aperto** — não no arranque. Um
servidor com quem ninguém faz aperto não escreve arquivo nenhum, e é assim que
uma implantação antiga continua idêntica a si mesma.

Ordem de procura, e a primeira que responder ganha:

1. `cifra_fio.chave_privada_env` — o nome de uma variável de ambiente com a
   privada em hexadecimal. **É o caminho recomendado**, pelo mesmo motivo da
   senha do cofre: `config.json` costuma ir para o controle de versão, e
   variável de ambiente não;
2. `cifra_fio.chave_privada` — a privada em hexadecimal, no próprio arquivo;
3. `cifra_fio.arquivo` (padrão: `chave-do-fio.hex`, ao lado do `config.json`) —
   lido se existir; criado com permissão `0600` se não existir.

Se o arquivo não puder ser escrito, o servidor **usa a chave em memória e
avisa** — e o aviso importa, porque uma chave que muda a cada arranque quebra
todo pino de cliente. Recusar-se a subir por causa disso seria pior: derrubaria
um servidor que estava funcionando.

`phxsqld --chave-do-fio` imprime a pública (criando a estática se ainda não
houver) — é por ela que o pino do cliente é configurado.

---

## 7. O que o atacante ganha e o que não ganha

| Atacante | Sem túnel (hoje) | Túnel, `exigir` desligado | Túnel, `exigir` ligado + pino |
|---|---|---|---|
| Grava o tráfego (passivo) | lê tudo: token, dados, prova | **não lê nada** | **não lê nada** |
| Repete uma resposta gravada | o desafio-resposta já barra o login; o resto passa | não passa (contador) | não passa |
| Corta o fio no meio de uma resposta | o cliente vê EOF e não sabe distinguir | **erro nomeado** | **erro nomeado** |
| Está no meio e modifica | manda no diálogo inteiro | **apaga o aperto e manda igual** | não fecha o aperto: conexão morre |
| Rouba a estática do servidor depois | — | não lê as sessões passadas (efêmeras) | idem |
| Lê o `config.json` da máquina | já ganhou (token, senhas, dados) | já ganhou | já ganhou |

A linha do sigilo futuro (*forward secrecy*) merece a frase: as chaves de
sessão saem de `ee` **e** de `es`. O `ee` é efêmero dos dois lados e morre com a
conexão, então quem roubar a estática do servidor amanhã não decifra o que
gravou ontem.

---

## 8. Configuração, inteira

```json
"cifra_fio": {
  "ligada": true,
  "exigir": true,
  "exigir_amarra": false,
  "chave_privada_env": "PHXSQL_CHAVE_DO_FIO",
  "chave_privada": "",
  "arquivo": "chave-do-fio.hex"
}
```

| campo | padrão | o que faz |
|---|---|---|
| `ligada` | `true` | o servidor **atende** o `cifrar`. `false` recusa o aperto — e é a única maneira de um servidor dizer «aqui não tem». Não muda nada para quem não pede |
| `exigir` | `true` | recusa qualquer pedido fora do túnel — **e nasce ligado desde 18/09/2026** (pedido 370). `"exigir": false` é o escape escrito. Ver §2 |
| `exigir_amarra` | `false` | havendo túnel, recusa o `login` que não amarra a credencial ao canal (`erro.amarra_exigida`). Sem túnel não se aplica — não há transcrição a que amarrar. Ver §10 |
| `chave_privada_env` | vazio | nome da variável de ambiente com a privada em hexadecimal |
| `chave_privada` | vazio | a privada em hexadecimal, no arquivo |
| `arquivo` | `chave-do-fio.hex` | onde a estática é lida/criada, se as duas de cima estiverem vazias |

`ligada: true` por padrão é seguro **porque o aperto só acontece se o cliente
pedir**: um cliente que nunca ouviu falar disto nunca manda `cifrar`, e nada
muda para ele. `exigir: true` é a ordem do dono de 18/09/2026, e o que ela
custa está na §13.

E na origem da replicação:

```json
"origens": [
  { "nome": "matriz", "host": "10.0.0.1", "porta": 5000,
    "cifra": false,
    "chave_do_fio": "<64 dígitos hexadecimais>" }
]
```

`cifra: true` é o **padrão** desde 18/09/2026 (§13) — o exemplo traz o `false`
escrito porque é ele que se escreve: o escape. `cifra` ligada sem
`chave_do_fio` = túnel sem pino, ou seja, **passivo apenas** — e o arranque
avisa exatamente isso, com estas palavras.

E na interface web, para o `Remoto` (o multi-servidor). Aqui a mudança foi de
**formato**: `web.servidores` era uma lista de textos `"host:porta"`, e não
havia onde escrever o pino de cada destino. Agora ela aceita as **duas** formas
na mesma lista — texto solto (em claro, como sempre foi) e objeto (que carrega
o pino):

```json
"web": {
  "servidores": [
    "curitiba:5000",
    { "host": "10.0.0.9", "porta": 5000,
      "cifra": true,
      "chave_do_fio": "<64 dígitos hexadecimais>" }
  ]
}
```

A regra é a mesma da origem, palavra por palavra — **inclusive o padrão**:
desde 18/09/2026 o texto solto também pede o aperto (§13), e quem quer claro
troca aquele item pelo objeto com `"cifra": false`. `cifra` ligada sem
`chave_do_fio` = túnel sem pino, **passivo apenas**, e o arranque avisa. O
pino **nunca** sai numa
resposta de protocolo — o `/saude` diz por servidor apenas `cifra` e
`tem_pino`, para a tela avisar sem carregar material de chave.

---

## 9. O que isto NÃO é

* **Não é TLS.** Ver §5.
* **Não interopera com outras implementações de Noise.** Os tijolos são de
  norma e conferidos contra vetor oficial — X25519 (RFC 7748), HKDF (RFC 5869),
  ChaCha20-Poly1305 (RFC 8439), SHA-256 (FIPS 180-4). A **composição** segue o
  padrão NX da especificação Noise, mas **não** foi rodada contra os vetores de
  interoperabilidade do Noise (os do *cacophony*), então não afirmo que um
  `snow` ou um `noise-c` do outro lado fecharia o aperto. O que está provado é
  que os dois lados **daqui** fecham, e que uma implementação independente em
  Python (a da bancada) fecha com o servidor — o que é evidência boa, e não é a
  mesma coisa que interoperabilidade certificada.
* **Não autentica o cliente por chave.** Ver §1.
* **Não protege contra quem lê o `config.json`.** Nunca protegeu: é lá que
  estão o token e as senhas.
* **Não substitui o `ips_permitidos` nem o firewall.** Cifra não é controle de
  acesso.

---

## 10. O que fica para depois, escrito para não se perder

* **Amarrar o login ao canal — FEITO (07/09/2026), com o limite escrito.** O
  desafio-resposta passou a consumir o hash da transcrição: quem manda
  `"amarrar_canal": true` no `login` prende a prova à transcrição *deste* túnel,
  e o servidor a confere contra a *sua*. As duas só coincidem se não há ninguém
  no meio que tenha terminado o túnel — então um homem-no-meio que fechou o
  túnel do cliente (TOFU na primeira conexão, ou `exigir` desligado sem pino) já
  não reencaminha a prova: ela vale para o túnel dele, não para o do servidor.
  Entra **pedido**, como sempre: sem o campo, a prova é byte a byte a de antes,
  e a porta web (HTTP, sem túnel) e o cliente velho não mudam. A `replica::Cliente`
  amarra sozinha quando fala por dentro do túnel; pedir `amarrar_canal` numa
  conexão em claro é recusa nomeada (`erro.amarra_sem_tunel`), não uma amarração
  a coisa nenhuma.

  **O limite, dito sem enfeite:** a amarração é *pedida*, e um atacante ativo que
  já terminou o túnel do cliente pode cortar o campo `amarrar_canal` antes de
  reencaminhar — a mesma aritmética do rebaixamento da §2. Por isso ela **não
  substitui o pino**: para o cliente que pinou a chave do servidor (o que não
  tem o túnel terminado por ninguém) a amarração é a garantia inteira; para o
  cliente sem pino é defesa em profundidade, e o que fecha o buraco continua
  sendo `exigir: true` **mais** o pino. O passo seguinte — o servidor **exigir**
  a amarração quando há túnel — está **FEITO (07/09/2026)**: `cifra_fio.exigir_amarra`
  (padrão `false`) faz o `login` recusar, com `erro.amarra_exigida`, quem não
  amarra quando há transcrição na sessão. É a mesma decisão de implantação do
  `exigir`, do lado que sabe onde está — e nasce desligada, porque ligá-la para
  todo mundo quebraria o cliente que só pede o túnel (guarda nova entra pedida,
  não imposta). **Só morde quando há túnel:** em claro não há transcrição a que
  amarrar, e a conexão em claro não muda. A prova real está em
  `desafio::tests::prova_amarrada_a_um_canal_nao_serve_em_outro` (a cripto),
  `login_amarrado_ao_canal_confere_contra_a_transcricao_da_sessao` (a amarração
  pedida) e `login_exige_amarra_quando_ha_tunel` (a exigência, nos quatro
  sentidos: exige+túnel+não-amarra recusa nomeada, amarra entra, sem túnel não
  muda, e desligado entra como sempre). As guardas `amarra-ao-canal-ignorada` e
  `amarra-exigida-ignorada` repõem os dois defeitos.
* **O driver ODBC fala o aperto — FEITO (08/09/2026).** Ele era o cliente da
  porta 5000 que ainda falava só claro, e com `exigir: true` parava. Agora a
  connection string liga o túnel: `CIFRA=1` faz o driver mandar
  `{"op":"cifrar",...}` antes de qualquer pedido, fechar o aperto com
  `Iniciador::terminar` e falar registros pelo `Canal` — reusando o `fio` do
  core, exatamente como a `replica::Cliente`, sem uma segunda cópia de cripto.
  `CHAVE_DO_FIO=<hex>` é o pino, e escrevê-lo já liga a cifra (esquecer
  `CIFRA=1` não rebaixa para claro em silêncio). O login e o token passam a
  viajar por dentro do túnel. A prova real é nos dois sentidos:
  `bancada/odbc/prova-cifra.py` sobe um `phxsqld` com `exigir: true`, monta os
  dados por dentro do túnel e confere pelo `ctypes` **e** pelo `isql -k` de
  verdade que a conexão cifrada trabalha e que a **em claro é recusada** com o
  erro nomeado; em Rust,
  `conexao::testes::aperto_pelo_canal_fecha_e_fala_por_dentro` prova o aperto
  sem gerenciador de driver, e o defeito reposto (driver ignorando a cifra)
  derruba a prova. Ver `docs/ODBC.md` §1.1 e §7.

  **E ela deixou de ser `opt-in` em 18/09/2026 (pedido 373), por decisão do
  dono:** a receita **nasce** cifrada e `CIFRA=0` é o escape escrito — o mesmo
  raciocínio do `exigir` que nasceu ligado do lado do servidor, *o esquecimento
  não pode ser o padrão quando o assunto é senha no fio*. Quem a escreve contra
  um servidor com `cifra_fio.ligada: false` recebe o motivo do servidor **mais
  a saída** (`CIFRA=0`) no diagnóstico; com pino escrito, a saída não é
  ensinada, porque ali a falha é a chave não conferir.

  **O limite caducou em 23/09/2026 (pedido 275).** Era este: *o login do driver
  é a senha em claro dentro do túnel, então ele não amarra a credencial ao
  canal — um servidor com `exigir_amarra: true` o recusaria.* Hoje o driver faz
  o desafio-resposta pelo mesmo `phxsql_core::desafio` da `replica::Cliente`, e
  manda `amarrar_canal: true` **quando há túnel** — em claro (`CIFRA=0`) não há
  transcrição a que amarrar, e a mensagem provada continua byte a byte a de
  sempre. Servidor com `exigir_amarra: true` passa a aceitar o ODBC. Ver
  `docs/ODBC.md` §5.1.
* **O `Remoto` (multi-servidor da interface) liga o túnel — FEITO, com a
  mudança de formato que faltava.** O diagnóstico estava certo: `web.servidores`
  era uma **lista de textos** `"host:porta"`, e sem lugar para o pino ligar a
  cifra seria proteção só contra escuta passiva vendida como se fosse mais. A
  saída foi trocar a lista por **objetos** (§8), retrocompatível — texto solto
  continua valendo, como sempre foi. Agora o `abrir_remoto` pede o aperto
  (`Remoto::cifrar`, reusando o `fio::Iniciador` da `replica::Cliente`) **antes**
  do login, quando o destino tem `cifra: true`, com o pino de `chave_do_fio`.
  Sem pino, protege só da passiva, e o arranque avisa com estas palavras. A
  prova é por soquete, contra um servidor de verdade: `o_remoto_liga_o_tunel_e_
  carrega_um_pedido_real`, `o_pino_certo_entra_o_errado_derruba` e
  `abrir_remoto_liga_o_tunel_quando_a_config_pede_cifra` (este último com o
  destino em `exigir: true`, para que "esqueci de cifrar" caia nomeado em vez de
  vazar em claro). A guarda `remoto-em-claro-para-quem-exige` repõe o defeito.
  Falta ainda **exercitar a tela** — o `/saude` já diz `cifra`/`tem_pino` por
  servidor e o login mostra o aviso, mas isso é papel do designer (E), porque
  interface só se prova exercitando.
* **O cluster fala em claro — FEITO (08/09/2026), e o INTEIRO, não a metade.**
  A ressalva que este item trazia — «cifrar só metade do tráfego do cluster é
  pior que não cifrar nenhuma, porque parece protegido» — foi o que guiou o
  conserto: o pulso da eleição **e** a replicação entre os nós passam os dois a
  cifrar juntos, sob um único interruptor. Ver a **§12**, escrita para não se
  perder.
* **O DbLink para outro PhxSql fala o aperto — FEITO (22/09/2026, pedido
  378).** Faltava o campo: `dblink/phx.rs` já abria pelo mesmo
  `replica::Cliente`, mas nunca chamava `cifrar`, e o `dblink.json` não tinha
  onde escrever a decisão. O parecer do DBA (papel C) mediu por que a forma
  dos três precedentes (`origens[]`, `cluster.nos[]`, ODBC) não bastava aqui —
  o `dblink.json` é o único cadastro que o servidor reescreve INTEIRO a cada
  salvar e cujo dono é polimórfico em três motores — e fechou com três
  garantias que nenhum dos dois pedidos anteriores tinha: (1) herança no
  quarto lugar (`op_dblink_salvar`), para salvar pela tela não apagar o pino
  em silêncio; (2) `cifra: Option<bool>` privado (`Default` em `None`, não
  `false`), o efetivo saindo de um método só; (3) `cifrar` antes de
  `autenticar`, porque o token do DbLink viaja no primeiro pedido. Cifra
  nasce **ligada** (`CIFRA_DE_SAIDA_PADRAO`), e é **recusada na declaração**
  para motor diferente de `phxsql` (`Motor::cifra_o_fio()`,
  `conferir_cifra_do_motor`) — nunca um interruptor mudo. Prova real:
  `bancada/dblink/prova-do-tunel.py` (três `phxsqld`, um deles SURDO, 15
  conferências verdes) e 17 testes novos. Ver `docs/DBLINK.md` §"O fio do
  terceiro motor: o túnel, e ligado de fábrica", e §5 aqui.
* **Moldura binária no lugar do Base64**, se os 33% doerem em alguma medição.
  Hoje não doeram porque ninguém mediu com o túnel ligado — e a regra da casa
  diz que isso é palpite até alguém medir.
* **Compressão DENTRO do túnel — decisão de segurança adiada de propósito,
  medida antes.** O pedido 226 mediu a premissa (uma resposta de `varrer` com
  5.000 linhas cai de 535.870 para 55.284 bytes com o `zlib` do Python,
  **9,69×**) e faltava só a negociação. A frente que fechou o pedido
  implementou a compressão **apenas no caminho em claro** —
  `Servidor::talvez_comprimir` recusa comprimir sempre que `canal.cifrado()`
  é verdadeiro, ainda que o pedido mande `"aceita_compressao":true` — e mediu
  de novo com o DEFLATE **desta casa** (Huffman fixo, não o dinâmico do
  zlib): **5,66×** nas mesmas 5.000 linhas (535.871 → 94.694 bytes,
  `compressao-do-fio.rs::medir_o_ganho_do_deflate_desta_casa`). A diferença
  entre 9,69× e 5,66× não é «uns por cento», como o comentário do
  `phxsql_core::zip` supunha — é o preço concreto de não montar a árvore
  dinâmica, e fica registrado aqui para não virar número citado de memória na
  próxima medição.

  **Por que não comprimir e depois cifrar.** Comprimir o texto claro antes de
  selar o registro (compress-then-encrypt) faz o TAMANHO do registro cifrado
  depender do quanto o conteúdo comprimiu — e quando parte do conteúdo é
  influenciável por quem ataca (um campo ecoado na resposta, por exemplo) e
  outra parte é secreta, o tamanho observado vaza se as duas partes têm um
  trecho em comum. É o mesmo mecanismo do CRIME/BREACH contra TLS: o atacante
  não lê o segredo, mas *infere* um byte dele por vez, observando quando a
  compressão encolhe mais um pouco. Este servidor tem campos ecoados de volta
  ao cliente (mensagens de erro com o texto do pedido, por exemplo) na mesma
  resposta que pode carregar dado sensível — a superfície existe, mesmo que
  ninguém a tenha explorado ainda.

  **O que decidiria isso, e por que não é papel de quem fechou o 226.**
  Mitigar exigiria escolha de projeto de segurança — por exemplo, comprimir
  só campos que nunca ecoam entrada do cliente, ou preencher (padding) o
  registro para esconder o tamanho real, ou aceitar o risco documentando quais
  campos podem ser ecoados e proibindo compressão quando um deles aparece.
  Qualquer uma dessas é decisão de arquitetura de segurança, não implementação
  mecânica — por isso continua **fora do escopo** de quem só tinha a
  negociação e o enquadramento para fechar. Enquanto ninguém tomar essa
  decisão, a regra que vale é a mais simples e a mais segura: **dentro do
  túnel não se comprime, ponto**, provado pelo soquete em
  `compressao-do-fio.rs::dentro_do_tunel_o_pedido_de_compressao_e_ignorado`
  (inclusive com o defeito reposto — comentar a conferência de `cifrado()`
  faz esse teste cair, mostrando o envelope `{"cz":...}` vazando para dentro
  do túnel decifrado).
* **Estática de cliente (IK)** para recusar o estranho antes do login, se um
  dia o ciclo de vida da chave de cliente valer o próprio custo.

---

## 11. Os testes, e o que cada um prova

| teste | prova |
|---|---|
| `x25519::testes::vetor_1_da_secao_5_2` … `vetor_2_…` | RFC 7748 §5.2 |
| `x25519::testes::iteracoes_da_secao_5_2` | RFC 7748 §5.2, 1 e 1.000 vezes (o de 1.000.000 fica atrás de `#[ignore]`) |
| `x25519::testes::diffie_hellman_da_secao_6_1` | RFC 7748 §6.1 |
| `x25519::testes::ponto_de_ordem_pequena_e_recusado` | o segredo todo-zeros é **erro**, não segredo |
| `hkdf::testes::caso_1/2/3_do_anexo_a` | RFC 5869 A.1, A.2, A.3 |
| `fio::testes::aperto_fecha_e_os_dois_lados_derivam_o_mesmo` | o aperto |
| `fio::testes::pino_certo_passa_e_pino_errado_derruba` | a autenticação do servidor |
| `fio::testes::quem_apresenta_estatica_alheia_nao_fecha` | a etiqueta final depende da PRIVADA |
| `fio::testes::efemera_de_ordem_pequena_derruba_o_aperto` | o servidor também recusa |
| `fio::testes::o_texto_claro_nao_aparece_no_fio` | o que um `tcpdump` veria |
| `config::tests::sem_a_secao_cifra_fio_a_cifra_ja_e_exigida` | **o padrão, no arquivo** — sem a seção, a cifra É exigida (pedido 370) |
| `config::tests::a_privada_do_fio_nunca_sai` | nem no `para_json`, nem no `Debug` |
| `config::tests::a_estatica_do_fio_nasce_no_arquivo_e_nao_muda` | e nasce `0600` |
| `config::tests::pino_torto_na_origem_e_erro_e_nao_ausencia` | pino errado nunca vira «sem pino» |
| `config::tests::a_origem_nasce_cifrada_e_o_escape_escrito_a_deixa_em_claro` | **o padrão da SAÍDA, no arquivo** (§13) — e o escape, no mesmo teste |
| `config::tests::cifra_do_cluster_nasce_ligada` / `o_escape_escrito_deixa_o_cluster_em_claro` | o mesmo par, no cluster |
| `config::tests::web_servidores_texto_solto_passa_a_pedir_o_aperto` / `o_escape_escrito_deixa_o_destino_da_tela_em_claro` | o mesmo par, na tela — **inclusive o texto solto** |
| `config::tests::web_servidor_em_objeto_sem_o_campo_cifra_nasce_cifrado` | as duas formas da mesma lista dão o mesmo destino |
| `config::tests::valor_torto_no_cifra_de_saida_nao_rebaixa_e_avisa` | os TRÊS estados: valor torto nunca desliga a cifra (§13) |
| `config::tests::saida_no_padrao_de_fabrica_avisa_que_vai_pedir_o_aperto` / `as_duas_decisoes_escritas_calam_o_aviso_da_saida` | o aviso do arranque, nos dois sentidos |
| `servidor::testes_config_gravar::a_sonda_de_replicacao_tem_o_mesmo_padrao_de_cifra_do_arquivo` | o IRMÃO fora do `config.rs` — compara os dois caminhos, não o valor |
| `fio::testes::mensagem_2_mexida_nao_autentica` | a transcrição cobre tudo |
| `fio::testes::registro_repetido_nao_abre` | contador |
| `fio::testes::registro_fora_de_ordem_nao_abre` | contador |
| `fio::testes::contador_no_teto_recusa_em_vez_de_repetir` | §3 |
| `fio::testes::fim_e_corte_sao_vereditos_diferentes` | §4 |
| `desafio::tests::sem_canal_a_prova_e_identica_a_de_sempre` | amarração `None` = a prova de sempre, byte a byte (a regra pétrea, no cálculo) |
| `desafio::tests::prova_amarrada_a_um_canal_nao_serve_em_outro` | a amarração ao canal: mesma transcrição passa, outra cai (§10) |
| `login_amarrado_ao_canal_confere_contra_a_transcricao_da_sessao` | a fiação no servidor: transcrição na sessão, recusa sem túnel, e o velho intacto |
| `login_exige_amarra_quando_ha_tunel` | a EXIGÊNCIA (`exigir_amarra`): com túnel, quem não amarra é recusado; sem túnel não muda; desligado entra como sempre (§10) |
| `o_cliente_velho_sem_o_escape_escrito_e_recusado_com_o_motivo` (soquete) | **o padrão, pelo fio** — e a recusa diz o que fazer |
| `o_escape_escrito_deixa_o_cliente_em_claro_entrar` (soquete) | o ESCAPE escrito — o que ficou igual dos dois lados da virada |
| `dentro_do_tunel_a_diretiva_confirma_a_cifra_desta_conexao` (soquete) | `encryption_exigida` diz a verdade DESTA conexão (pedido 370) |
| `exigir_recusa_texto_claro_e_deixa_o_tunel_passar` (soquete) | §2 |
| `registro_repetido_derruba_a_conexao` (soquete) | o laço age sobre a recusa, em vez de engolir |
| `fio_cortado_vira_erro_e_despedida_nao` (soquete) | §4, contado no `acessos.log` |
| `bancada/cifra-do-fio/prova.py` (soquete, cliente Python independente) | o aperto de ponta a ponta, o `exigir`, o corte do fio pelo sistema operacional |

A prova real de cada um está no `bancada/guardas/catalogo.py`, em cinco
entradas novas: o defeito é reposto e o executor confere que o teste **cai**.
A exigência da amarração (07/09/2026) somou a sexta, `amarra-exigida-ignorada`:
repõe o `op_login` que ignora `exigir_amarra` e confere que o
`login_exige_amarra_quando_ha_tunel` cai no caso da recusa nomeada.

### O que a prova real achou — e a leitura não acharia

O executor devolveu **NÃO PEGOU** em duas das cinco entradas, e as duas eram
achados de verdade.

**1. O teste da regra pétrea passava por engano.** Com o padrão trocado para
`exigir: true` — que é o estrago que a entrada `cifra-do-fio-imposta` repunha —
o `cliente_sem_cifra_continua_como_antes` continuava **verde**. O motivo: ele
montava o `Config` na mão e escrevia `cifra_fio.exigir = false`, desfazendo a
troca antes de exercitar coisa nenhuma. Um teste que escreve o campo não pode
provar o padrão dele.

> **Os dois nomes acima não existem mais, e o que aconteceu com eles ensina**
> (18/09/2026, pedido 370): o padrão virou de verdade, por ordem do dono. O
> teste mudou de lado e virou
> `o_cliente_velho_sem_o_escape_escrito_e_recusado_com_o_motivo` — a mesma
> pergunta («o que acontece com quem só trocou o binário?») com a resposta
> nova. A guarda `cifra-do-fio-imposta` foi **aposentada** (a lista
> `APOSENTADAS` do `trecho-vivo.py` diz a data e o motivo), porque o defeito
> que ela repunha virou o produto; no lugar dela nasceu a
> `cifra-do-fio-rebaixada`, que repõe o defeito **contrário**. Guarda cujo
> defeito deixou de existir não se remenda para o número fechar.

Consertado: ele agora sobe de um `config.json` **sem a seção `cifra_fio`** —
literalmente o arquivo de quem atualizou o binário e não mexeu em nada — e
confere o padrão lido de volta. Com o defeito reposto, cai.

É a lição do `BULKINSERT` por outro caminho: **teste que passa por engano é
pior que teste que falta**, e quem o encontrou foi a mutação, não a leitura.

**2. O `canal_leva_e_traz` não sente o contador parado, e isso é medido.** A
primeira versão da entrada `contador-do-fio-parado` listava quatro testes que
deveriam cair; caíram três. Investigado: com o contador congelado, os dois
lados usam nonce zero em **todo** registro, então uma conversa que vai e volta
uma vez continua fechando — ela não repete registro nenhum, que é o único jeito
de sentir a falta do contador. O teste não está errado; errada estava a conta
de quatro. Está escrito no catálogo, ao lado da entrada, para ninguém a
«consertar» de volta.

---

## 12. O cluster cifrado — o INTEIRO, não a metade

Este é o item que a §10 prometeu e a §5 deixou de recusar. A lei que o guiou é
a que já estava escrita no item aberto: **cifrar só metade do tráfego do
cluster é pior que não cifrar nenhuma, porque parece protegido.** Por isso o
cluster ganha um interruptor só, que liga os dois caminhos de uma vez.

### O que já ajudava, medido antes de escrever código

O pulso da eleição **e** a replicação entre os nós passam os dois pela mesma
`replica::Cliente` — a mesma que já sabia apertar a mão (`cifrar`) para a porta
de dados. O pulso conecta por `conectar_com_prazo` e manda `cluster_pulso`; a
replicação monta uma `Origem` e cai em `replica::ligar`, que já cifrava quando
`origem.cifra`. Ou seja: **os dois transportes eram compatíveis e o servidor do
outro lado já atendia o aperto** — não havia decisão de formato em disco a
tomar, só fiação a fazer. Isso é o que permitiu fechar o item INTEIRO em vez de
parar numa proposta.

### O interruptor, e o pino por nó

```json
"cluster": {
  "cifra": true,
  "nos": [
    { "id": "no1", "endereco": "10.0.0.1", "porta": 5000,
      "chave_do_fio": "<64 dígitos hexadecimais>" },
    { "id": "no2", "endereco": "10.0.0.2", "porta": 5000,
      "chave_do_fio": "<64 dígitos hexadecimais>" },
    { "id": "no3", "endereco": "10.0.0.3", "porta": 5000,
      "chave_do_fio": "<64 dígitos hexadecimais>" }
  ]
}
```

| campo | padrão | o que faz |
|---|---|---|
| `cluster.cifra` | `false` | cifra **todo** o tráfego do cluster — pulso e replicação. `false` é o cluster de sempre, em claro |
| `nos[].chave_do_fio` | vazio | o **pino** daquele nó: a chave pública que se espera do servidor daquele nó, no estilo `known_hosts`. Vazio com a cifra ligada = túnel **sem pino** (só escuta passiva) |

O pino é **por nó** e mora dentro de cada `no`, ao lado de `endereco` e `porta`,
pelo mesmo motivo do `chave_do_fio` da origem: é a chave **daquele** servidor, e
cada nó confere a de quem alcança — quando `no1` pulsa `no2`, `no1` pina a chave
de `no2`. Como a lista de nós é a mesma em todos, cada `config.json` acaba
carregando o pino de todos, que é exatamente o `known_hosts` do cluster. A chave
pública de um nó sai de `phxsqld --chave-do-fio` **naquele** nó.

### Padrão LIGADO desde 18/09/2026 — e por que ele mudou

Este bloco dizia o contrário: `cifra: false` por padrão, porque *guarda nova
entra pedida, não imposta*. A ordem do dono de 18/09/2026 («a comunicação deve
obrigatoriamente ser cifrada») alcança a **saída** também, e o padrão virou —
`cluster.cifra` nasce `true`, com `"cifra": false` como escape escrito. O que
a pétrea protege continua protegido, e é o dado que já está em disco; o que
mudou é o que **nasce** daqui para a frente, que é exatamente o alcance dela.

O que não mudou: ligar exige que **todo** nó atenda o aperto (a
`cifra_fio.ligada` já nasce ligada), então continua sendo uma decisão do
cluster inteiro. Um cluster com um nó anterior ao aperto escreve o escape até
atualizar esse nó. Detalhe e número na §13.

### As duas metades, e a guarda de cada uma

O medo escrito na lei — proteger só metade — virou **duas** guardas, uma por
metade, para que esconder uma delas não passe:

- `pulso-do-cluster-em-claro` repõe o pulso saindo em claro (tira o `cifrar` do
  `pulsar`). A prova é pelo **soquete**: dois nós com `cifra_fio.exigir: true` só
  se enxergam vivos no `cluster_estado` se o pulso atravessa o túnel — com o
  defeito reposto, cada pulso bate no `exigir` do outro e cai, e nenhum aparece.
  **Os dois nós exigem de propósito:** se só um exigisse, o pulso em claro do
  outro sentido ainda registraria o par, e o defeito passaria despercebido.
- `replicacao-do-cluster-em-claro` repõe a replicação saindo em claro (a linha
  `cifra: c.cifra` da `origem_do_master`, que é pura justamente para o teste
  pegar o que a leitura do laço vivo não pega).

### O que o arranque diz, sem enfeite

Com a cifra ligada, o arranque diz se **todos** os nós têm pino («tráfego
CIFRADO, com pino em todos os nós») ou **nomeia** os que não têm — porque
cifrado sem pino protege só da escuta passiva, e esconder isso seria vender
proteção que não existe contra quem está no meio. É a mesma franqueza do aviso
da origem sem pino. E um pino **torto** é recusado na **declaração**, com o nó
nomeado — nunca vira um túnel sem pino descoberto três semanas depois no
primeiro pulso.

### O nó acrescentado a quente também leva pino

O escalonamento a quente (`cluster_no_acrescentar`, pedido 217) aceita
`chave_do_fio` e o propaga aos outros nós — senão um nó entrado a quente num
cluster cifrado seria pulsado e replicado **sem** âncora enquanto os do arquivo
têm pino: de novo a metade que engana. E o pino sobrevive à reescrita da lista:
gravar `cluster.nos` a quente sem ele apagaria o pino de **todos** os nós de uma
vez, deixando a cifra ligada e rebaixada a escuta passiva em silêncio.

### O que o pino cifra, e o que ele NÃO prova — parecer sobre o A1 pleno (17/09/2026)

A revisão SEC de 17/09/2026 (achado A1, pedido 278) pediu, como conserto
pleno, amarrar a **identidade de quem manda o pulso** ao pino — para um pulso
forjado não poder se passar por outro nó da lista. O pedido é razoável e
esbarra numa decisão já tomada aqui: o aperto do cluster é **Noise NX**
(§1), e no NX **só o respondedor apresenta chave estática** — quem inicia a
conexão (e é o iniciador quem manda o pulso, `replica::ligar`) é **anônimo por
desenho**, e essa foi a escolha registrada em «Por que a estática do cliente
ficou de fora (XX e IK descartados)» (§1). A sessão que sai do aperto guarda a
**transcrição**, não uma identidade do lado que chamou — não há, hoje, nada
para comparar contra `chave_do_fio` do lado de quem pulsa.

**O que fecharia o A1 por completo, e por que nenhum dos dois entrou nesta
rodada**: (1) uma prova **dentro do próprio pulso**, por Diffie-Hellman contra
as chaves estáticas que já existem — `chave_do_fio` de cada `NoCluster` já é,
de fato, um `known_hosts`, então o material está todo aqui; falta o protocolo
que o usa para autenticar o `id` declarado no corpo do pulso, não só cifrar o
transporte. (2) trocar o padrão do aperto do cluster para **XX** ou **IK**,
em que o iniciador também apresenta estática — o que reabriria a decisão da
§1 e teria o mesmo custo que fez XX/IK serem descartados lá (mais uma
ida-e-volta no aperto, ou uma chave pré-compartilhada por nó). As duas são
desenho de protocolo cifrado, e ficam com o dono — o teto de época
(`FOLGA_DE_EPOCA`, `docs/CLUSTER.md` §2.2) que entrou em `49a3af7` cobre o
sintoma (envenenar `maior_epoca_vista` para sempre), não a causa (o pulso não
prova quem o mandou).

**Decidido pelo dono em 17/09/2026, e ENTREGUE: o caminho (1).** A prova mora
dentro do proprio pulso, e o aperto **nao mudou** -- continua NX, com o
iniciador anonimo, e a decisao da §1 fica intacta. Cada no assina o pulso com
`HMAC-SHA256` sob a chave derivada do Diffie-Hellman entre a estatica dele e o
`chave_do_fio` do destinatario, e o destinatario refaz a conta do outro lado:
X25519 e simetrico, entao o material ja esta todo aqui e nao ha chave nova a
distribuir. A §2.2 do `docs/CLUSTER.md` conta o desenho inteiro; o codigo e
`crates/phxsql-server/src/pulso.rs`.

O que isto empresta desta secao, e vale dizer: **a transcricao do tunel entra
na mensagem assinada** quando ha tunel -- a mesma amarracao ao canal da §10, e
pelo mesmo motivo. Sem ela, um pulso gravado numa conexao valeria em outra. A
prova de que os dois lados calculam a MESMA transcricao nao e de unidade: sao
dois nos de pe, cifrados, com `exigir_prova_do_pulso` ligado nos dois
(`tests/identidade-do-pulso.rs`, `dois_nos_cifrados_com_exigencia_ligada_continuam_se_enxergando`).
Se as transcricoes divergissem, nenhuma prova fecharia e o cluster cifrado --
que e o **padrao** desde 18/09 -- pararia inteiro.

### O aperto de mao le com teto -- pedido 312

O aperto acontece **antes** de o outro lado se identificar, e por muito tempo
os dois clientes desta casa liam a resposta dele com `read_line` cru: a
`replica::Cliente::cifrar` (a replica falando com o source, e o no pulsando o
outro) e a `servidor::Remoto::cifrar` (a interface falando com outro PhxSql).
O `TETO_DO_REGISTRO` de 128 MiB mora no `Canal`, e nenhuma das duas passava
por ele ali -- o canal so nascia **depois** do aperto.

**Medido**: um source falso que aceita a conexao e nunca manda o fim de linha
fez a replica guardar **192 MiB numa linha so, em 294 ms** -- 1,5x o teto do
registro, o que e a prova de que aquele teto nao alcancava esta leitura. Quem
escolhia quanta memoria este lado reservava era o outro lado, que ali ainda
nao provou ser ninguem.

Hoje as duas leituras passam pelo mesmo `Canal` (ainda `Claro` naquele ponto),
com um teto **proprio e curto**: `TETO_DO_APERTO`, **64 KiB**. Nao sao os 200
bytes do caso feliz de proposito -- a resposta de **erro** do aperto carrega
texto traduzido e os campos da classificacao, e um teto colado no caso feliz
viraria recusa de mensagem legitima no dia em que alguem alongar uma frase.
Ainda assim e **2.048x menor** que o teto do registro.

---

## 13. A virada da SAÍDA (18/09/2026) — o outro lado da ordem

A §2 e a §8 contam a virada da **entrada**: `cifra_fio.exigir` nasce `true`, as
três portas HTTP recusam o claro, e `"exigir": false` é o escape escrito. Ela
deixou de fora, medido e escrito no próprio pedido que a fez, o que este
servidor **conecta**:

> com `cifra_fio.exigir` nascendo `true` e `replicacao.origens[].cifra`,
> `cluster.cifra` e `web.servidores[].cifra` nascendo **desligados**, um source
> de fábrica **recusa uma réplica de fábrica** — e a suíte fica verde do mesmo
> jeito, porque as bancadas escrevem o escape dos dois lados.

A ordem do dono alcança as duas direções («a *comunicação* deve
obrigatoriamente ser cifrada»), e os **três padrões viraram** — mais o quarto,
que em 18/09 não existia porque **faltava o campo** (pedido 378, 22/09/2026):

| saída | interruptor | padrão | escape escrito |
|---|---|---|---|
| réplica → source | `replicacao.origens[].cifra` | `true` | `"cifra": false` naquela origem |
| nó → nó do cluster | `cluster.cifra` | `true` | `"cifra": false` no bloco `cluster` |
| interface web → outro servidor | `web.servidores[].cifra` | `true` | o item vira objeto com `"cifra": false` |
| **DbLink → outro PhxSql** | `dblink[].cifra` | `true` **só no motor `phxsql`** | `"cifra": false` naquela ligação |

A quarta é a única **polimórfica**, e é por isso que ela lê o motor antes do
interruptor: no MySQL(R) e no PostgreSQL(R) o fio é protocolo alheio, o aperto
de mão não existe lá, e `cifra` ou `chave_do_fio` ali são **recusados na
declaração** — aceitar seria um interruptor que não faz nada. Os detalhes do
desenho (os três estados do campo, o que vai para o disco e a herança no
salvar) estão em `docs/DBLINK.md`.

### O censo dos sítios, e o comando que o refaz

Achado do papel G em 22/09/2026 (pedido 382): esta tabela listava três saídas
quando a busca no código já achava quatro, e o DbLink não aparecia em lugar
nenhum do documento. Medido de novo agora, em vez de repetir o número de
memória.

**Sete** sítios de `TcpStream::connect*` de produção, medidos em 24/09/2026
por `grep -rn "TcpStream::connect" crates/*/src`, com os dois sítios que são
só comentário (`replica.rs:72`, `email.rs:124`) e os seis que vivem sob
`#[cfg(test)]` (`replica.rs:789`, dentro de `mod testes_do_prazo_de_conexao`, e
cinco em `servidor.rs`, dentro dos módulos de teste do fim do arquivo)
excluídos à mão:

| sítio | quem chama | aperto de mão |
|---|---|---|
| `crates/phxsql-odbc/src/conexao.rs:332` | driver ODBC → phxsqld | próprio, inline (`Iniciador::comecar`, `conexao.rs:460`) |
| `crates/phxsql-server/src/replica.rs:112` | réplica → source; cluster (pulso e replicação, mesmo `replica::ligar`); DbLink → outro PhxSql (`dblink/phx.rs:91` reusa este cliente) | `replica::Cliente::cifrar` |
| `crates/phxsql-server/src/servidor.rs:533` | `Remoto` (interface → outro PhxSql) | `servidor::Remoto::cifrar` (`servidor.rs:556`) |
| `crates/phxsql-server/src/pg/mod.rs:604` | DbLink → PostgreSQL(R) | nenhum — protocolo alheio, recusado na declaração |
| `crates/phxsql-server/src/dblink/mysql.rs:452` | DbLink → MySQL(R) | nenhum — protocolo alheio, idem |
| `crates/phxsql-server/src/email.rs:180` | alerta por e-mail (SMTP) | nenhum — protocolo alheio, fora do escopo deste documento |
| `crates/phxsql-server/src/servidor.rs:6584` | `acordar_o_accept` — auto-conexão de loopback para destravar o `accept` | nenhum — fecha antes de trocar um byte, não fala protocolo algum |

**Três** implementações do aperto (o inline do ODBC, `replica::Cliente::cifrar`
e `servidor::Remoto::cifrar`) cobrem as **quatro** saídas do nosso protocolo da
tabela acima — `replica::Cliente` é reaproveitado duas vezes por cima do uso
original (réplica): pelo cluster e pelo DbLink → PhxSql. Os outros quatro
sítios não são saída do nosso protocolo: dois falam o fio de outro banco
(recusados na declaração, ver §5 "Também não vale"), um fala SMTP, e um não
fala protocolo nenhum. Este é o número que bate com o parecer do papel G: 7
sítios, 3 implementações de aperto, 4 saídas do protocolo.

**Para refazer o censo:** `grep -rn "TcpStream::connect" crates/*/src`,
descartando à mão as linhas de comentário e as que vivem sob `#[cfg(test)]`.

O padrão mora num lugar só — `CIFRA_DE_SAIDA_PADRAO`, em `config.rs` —, e os
**quatro** leitores o citam pelo nome. **Mais o irmão que mora fora do arquivo**: a
sonda `replicacao_testar` monta uma `Origem` com o que veio no pedido e cai no
**mesmo** `replica::ligar` do laço. Padrão que morasse só no analisador do
`config.json` deixaria essa sonda falando claro, calada — a armadilha que o
pedido 373 pagou no ODBC, onde o `SQLConnect` monta a receita sem passar pelo
analisador. Hoje o irmão tem prova própria, e ela compara os **dois caminhos**
em vez do valor:
`a_sonda_de_replicacao_tem_o_mesmo_padrao_de_cifra_do_arquivo`.

### O que custa, dito sem enfeite

**Uma réplica desta versão deixa de falar com um source anterior ao aperto de
mão.** Aquele servidor não atende o `cifrar`, e a conexão para. O mesmo vale
para um cluster com um nó atrasado e para a interface que alcança um PhxSql
antigo. Quem precisa da transição **escreve o escape** naquela saída — e o
custo foi aceito pelo dono junto com a ordem.

### O texto solto de `web.servidores` virou junto — e por quê

`"host:porta"` não tem onde escrever decisão nenhuma. Deixá-lo em claro faria a
virada não alcançar a forma **mais escrita** das duas, e quem lista um endereço
não está escolhendo o claro: está escrevendo o endereço. Ele nasce pedindo o
aperto, e o escape é trocar aquele item pelo objeto com `"cifra": false`.

### Os três estados do interruptor, e por que agora são três

`booleano_ou` fundia **ausente** e **torto** no mesmo destino. Isso era
inofensivo com o padrão desligado — «valor que não entendi vira `false`» dava no
mesmo que a ausência. Com o padrão ligado, o mesmo caminho seria um
**rebaixamento silencioso** da cifra por causa de um erro de digitação: é a
armadilha que o pedido 373 pagou no ODBC, onde `CIFRA=sim` cairia em claro.

Hoje o leitor distingue os três: **ausente** → padrão, e entra no aviso do
arranque; **escrito** (`true` ou `false`) → obedece e cala; **torto** → padrão
(cifrado) e um aviso que diz o que corrigir. Travado por
`valor_torto_no_cifra_de_saida_nao_rebaixa_e_avisa`.

### O aviso do arranque mudou de assunto — de novo

Ele dizia «estas saídas estão em claro, e um PhxSql desta versão vai
RECUSAR». Fazia sentido enquanto claro era o **esquecimento**. Com o padrão
ligado, saída em claro só existe **escrita** — e avisar quem escreveu o escape
é avisar contra a decisão dele, todo arranque, em toda instalação em transição.

O que sobrou de consequência é o que ele diz agora: *estas saídas vão **pedir**
o aperto, e um PhxSql anterior a 18/09/2026 não o atende*. Ele fala com quem
**não escreveu decisão nenhuma**, que é exatamente a população que a virada
pegou de surpresa, e **cala para as duas decisões escritas** — `true` («eu sei,
o outro lado fala») e `false` (o escape) —, porque o que se cobra é a decisão
registrada, não um dos valores. Servidor isolado não ouve nada: sem saída
configurada não há conexão que possa parar.

Os dois sentidos travados:
`saida_no_padrao_de_fabrica_avisa_que_vai_pedir_o_aperto` e
`as_duas_decisoes_escritas_calam_o_aviso_da_saida`.

### O que a virada derrubou, medido

Numa árvore verde (**2.585** passando, nenhum falhando), trocar os três padrões
derruba **5** testes — e **1 dos 5 estava fora do arquivo do assunto**
(`servidor.rs`, a outra metade da cifra do cluster). Os cinco:

| teste | onde | o que ele dizia |
|---|---|---|
| `cifra_do_cluster_nasce_desligada` | `config.rs` | o padrão velho do cluster — virou `cifra_do_cluster_nasce_ligada` |
| `exigir_com_saida_em_claro_avisa_que_o_outro_lado_vai_recusar` | `config.rs` | o aviso velho — virou `saida_no_padrao_de_fabrica_avisa_que_vai_pedir_o_aperto` |
| `web_servidores_texto_solto_continua_em_claro` | `config.rs` | o texto solto em claro — virou `web_servidores_texto_solto_passa_a_pedir_o_aperto` |
| `web_servidores_mistura_texto_e_objeto` | `config.rs` | as duas formas na mesma lista — continua, com o item de texto agora cifrado |
| `origem_do_cluster_carrega_a_cifra_e_o_pino` | `servidor.rs` | a metade «cluster em claro» passou a ser escrita (`"cifra": false`) em vez de omitida |

Nenhum teste foi apagado, e os que mudaram de veredito mudaram de **nome**
junto, com o nome antigo escrito no comentário: teste que muda de significado e
fica com o nome de ontem mente para quem lê a lista. E os que **não** mudaram
de significado são os do escape escrito — eles ficam iguais dos dois lados da
virada, que é o que faz a virada ter um lado de fora.
