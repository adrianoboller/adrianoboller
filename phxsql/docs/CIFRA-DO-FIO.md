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
   que fechou o túnel. Sem ela, o hash da transcrição existe (`fio::Transporte`
   o expõe) mas **ninguém o consome ainda** — está anotado na §10 como o
   próximo passo, não como coisa feita.

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

* **`exigir: false` (o padrão, e o padrão é o comportamento de hoje).** O
  servidor aceita claro e aceita cifrado. Cliente velho grava e lê igual a
  hoje, sem saber que existe aperto. É o teste que mais importa desta rodada:
  `cliente_sem_cifra_continua_como_antes`.
* **`exigir: true`.** O servidor recusa **qualquer** pedido que não venha
  dentro do túnel. A recusa é uma linha JSON em claro, com erro nomeado, e a
  conexão fecha em seguida — cliente velho recebe um erro que sabe exibir, em
  vez de um silêncio.

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

* **A conexão do driver ODBC** enquanto ele não aprender o aperto — ele fala a
  porta 5000 em claro, e com `exigir: true` ele para. Está dito na §10.
* **O `Remoto`** — a conexão que a interface usa para falar com outro PhxSql —
  e **o cluster**. Os dois estão na §10 com o motivo.
* **Nada disto é TLS.** Não há certificado, não há cadeia, não há autoridade,
  não há revogação. A confiança é o pino, e o pino é responsabilidade de quem
  configura.

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
  "exigir": false,
  "chave_privada_env": "PHXSQL_CHAVE_DO_FIO",
  "chave_privada": "",
  "arquivo": "chave-do-fio.hex"
}
```

| campo | padrão | o que faz |
|---|---|---|
| `ligada` | `true` | o servidor **atende** o `cifrar`. `false` recusa o aperto — e é a única maneira de um servidor dizer «aqui não tem». Não muda nada para quem não pede |
| `exigir` | `false` | recusa qualquer pedido fora do túnel. Ver §2 |
| `chave_privada_env` | vazio | nome da variável de ambiente com a privada em hexadecimal |
| `chave_privada` | vazio | a privada em hexadecimal, no arquivo |
| `arquivo` | `chave-do-fio.hex` | onde a estática é lida/criada, se as duas de cima estiverem vazias |

`ligada: true` por padrão é seguro **porque o aperto só acontece se o cliente
pedir**: um cliente que nunca ouviu falar disto nunca manda `cifrar`, e nada
muda para ele. `exigir: false` por padrão é a regra pétrea da casa.

E na origem da replicação:

```json
"origens": [
  { "nome": "matriz", "host": "10.0.0.1", "porta": 5000,
    "cifra": true,
    "chave_do_fio": "<64 dígitos hexadecimais>" }
]
```

`cifra: false` (padrão) = como sempre foi. `cifra: true` sem `chave_do_fio` =
túnel sem pino, ou seja, **passivo apenas** — e o arranque avisa exatamente
isso, com estas palavras.

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

* **Amarrar o login ao canal.** O `Transporte` já expõe o hash da transcrição;
  falta o desafio-resposta consumi-lo. Entra pedido, como sempre: quem mandar o
  campo novo ganha a garantia.
* **O driver ODBC não fala o aperto.** Com `exigir: true` ele para. Ou ele
  aprende, ou o servidor que exige não é o mesmo que atende ODBC.
* **O `Remoto` (multi-servidor da interface) não liga o túnel.** Não é
  esquecimento: `web.servidores` é uma **lista de textos** `"host:porta"`, e
  não há onde escrever o pino de cada um. Ligar sem pino seria proteção só
  contra escuta passiva vendida como se fosse mais; trocar a lista por objetos
  é mudança de formato de configuração, e ela entra com o pino junto ou não
  entra.
* **O cluster fala em claro.** A replicação do cluster passa pelo mesmo
  `rodada_da_replica`, mas o **pulso** da eleição vai por outro caminho
  (`cluster.rs`). Cifrar só metade do tráfego do cluster é pior que não cifrar
  nenhuma, porque parece protegido.
* **Moldura binária no lugar do Base64**, se os 33% doerem em alguma medição.
  Hoje não doeram porque ninguém mediu com o túnel ligado — e a regra da casa
  diz que isso é palpite até alguém medir.
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
| `config::tests::sem_a_secao_cifra_fio_nada_e_exigido` | **a regra pétrea, no arquivo** |
| `config::tests::a_privada_do_fio_nunca_sai` | nem no `para_json`, nem no `Debug` |
| `config::tests::a_estatica_do_fio_nasce_no_arquivo_e_nao_muda` | e nasce `0600` |
| `config::tests::pino_torto_na_origem_e_erro_e_nao_ausencia` | pino errado nunca vira «sem pino» |
| `fio::testes::mensagem_2_mexida_nao_autentica` | a transcrição cobre tudo |
| `fio::testes::registro_repetido_nao_abre` | contador |
| `fio::testes::registro_fora_de_ordem_nao_abre` | contador |
| `fio::testes::contador_no_teto_recusa_em_vez_de_repetir` | §3 |
| `fio::testes::fim_e_corte_sao_vereditos_diferentes` | §4 |
| `cliente_sem_cifra_continua_como_antes` (soquete) | **a regra pétrea, pelo fio** |
| `exigir_recusa_texto_claro_e_deixa_o_tunel_passar` (soquete) | §2 |
| `registro_repetido_derruba_a_conexao` (soquete) | o laço age sobre a recusa, em vez de engolir |
| `fio_cortado_vira_erro_e_despedida_nao` (soquete) | §4, contado no `acessos.log` |
| `bancada/cifra-do-fio/prova.py` (soquete, cliente Python independente) | o aperto de ponta a ponta, o `exigir`, o corte do fio pelo sistema operacional |

A prova real de cada um está no `bancada/guardas/catalogo.py`, em cinco
entradas novas: o defeito é reposto e o executor confere que o teste **cai**.

### O que a prova real achou — e a leitura não acharia

O executor devolveu **NÃO PEGOU** em duas das cinco entradas, e as duas eram
achados de verdade.

**1. O teste da regra pétrea passava por engano.** Com o padrão trocado para
`exigir: true` — que é o estrago que a entrada `cifra-do-fio-imposta` repõe —
o `cliente_sem_cifra_continua_como_antes` continuava **verde**. O motivo: ele
montava o `Config` na mão e escrevia `cifra_fio.exigir = false`, desfazendo a
troca antes de exercitar coisa nenhuma. Um teste que escreve o campo não pode
provar o padrão dele.

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
