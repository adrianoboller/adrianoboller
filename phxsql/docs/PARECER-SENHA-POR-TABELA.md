# Parecer do DBA — senha própria por tabela na cifra em repouso

> **A decisão é do dono.** Este documento não decide: ele põe as saídas e o
> custo de cada uma na mesa para que a decisão caiba em cinco minutos. Nada
> aqui foi implementado — **nenhuma linha do motor mudou** nesta rodada, fora
> um contador de derivações que existe só para o medidor não citar número.

**A pergunta, palavra do dono (05/09/2026):** *«Preciso poder ter senha ou não
no `.reg` para tabelas criptografadas, é possível?»*

**A resposta curta: é possível, e não é caro pelo motivo que se esperava.** O
formato já sorteia um sal por arquivo e já grava uma prova de 16 bytes que
responde *«esta senha abre este arquivo?»*. O que falta não é criptografia — é
**onde a senha mora durante a sessão**, e é aí que está todo o custo.

**E há uma coisa a decidir antes desta**, porque ela muda o que a senha por
tabela vale: **o `.ndx` fica em claro**, e sobre uma tabela com índice na
coluna sensível ele entrega **100%** dela sem senha nenhuma (§12.3 do
`SEGURANCA.md`).

As três premissas estão medidas em [`SEGURANCA.md` §12](SEGURANCA.md), e o
gerador é `cargo run --release --example senha-por-tabela -p phxsql-store`.

---

## 1. O que já existe, e que ninguém precisa construir

| peça | onde | o que ela já faz |
|---|---|---|
| chave **por arquivo** | `cofre::Material` | cada arquivo sorteia o próprio sal e tem a própria chave. O que é global é a **senha**, não a chave |
| a pergunta *«esta senha abre este arquivo?»* | prova de 16 bytes no cabeçalho | recusa a senha errada **na abertura**, não na primeira leitura |
| senha que **nunca** é gravada | PBKDF2 + sal em claro | o arquivo carrega sal e prova, nunca a senha |
| caminho de **chegada** de uma senha | desafio-resposta (§2) e cifra do fio (§7) | a senha já sabe atravessar a rede sem trafegar |
| **desligar a cifra por tabela** | a marca `DadoPessoal` no esquema | *já existe, com outro nome*: tabela sem coluna marcada **nasce em claro** mesmo com o cofre ligado (§11.6) |

A última linha muda a conversa: metade da saída (b) **já está pronta e em uso**.

O que **não** existe: um lugar para a senha da tabela morar entre o login e o
`abrir`. O cofre é um `static` **do processo**; a sessão é **por conexão**
(`servidor.rs::Sessao`, `http.rs::Sessao`), e nenhuma das duas guarda segredo.

## 2. As quatro saídas, com o custo medido

### (a) Senha própria por tabela, opcional

Uma tabela pode declarar que tem senha própria. Quem abre precisa apresentá-la.

| item | custo, medido ou contado |
|---|---|
| cache de chaves derivadas | **muda de chave**: de `(sal, iterações)` para `(impressão da senha, sal, iterações)`, e para de ser esvaziado. Sem isso são **20 derivações** onde hoje são 2 — **298 ms por tabela por pedido** (§12.2) |
| lugar da senha na sessão | **novo**. Campo em `Sessao` no fio e na web, com prazo e com esquecimento no logout |
| protocolo | operação nova para apresentar a senha de uma tabela, e o erro «esta tabela pede senha» onde hoje o erro é «este servidor não tem a chave» |
| formato em disco | **1 byte no `PSCH`** para «esta tabela tem senha própria» — sem ele o servidor não sabe qual senha pedir e o erro vira adivinhação. **Muda o formato, e por isso entra cedo ou vira migração** |
| camadas atravessadas | zero, **se** a senha continuar no cofre indexada por tabela. A alternativa (chave como parâmetro) toca **35** chamadas de `Table::criar`/`abrir` em 7 arquivos e 3 crates, e é a que o cabeçalho do `cofre.rs` recusa por escrito |
| replicação | **piora** o que já está quebrado: hoje a coluna externa marcada já não replica nem com a mesma senha (§12.4). Com senha por tabela, some também o caso em que a senha *era* a mesma |
| o que ela protege | o `.reg`, o `.memo` e o `.bin` **daquela tabela**, contra quem tem o arquivo e o `config.json` |
| o que ela **não** protege | o `.ndx` — **100%** da coluna marcada e indexada (§12.3) |

### (b) Só ligar/desligar a cifra por tabela, com a senha do servidor

| item | custo |
|---|---|
| cache de chaves | **nenhum**. Continuam 2 derivações |
| sessão e protocolo | **nenhum**. A senha continua sendo a do `config.json` |
| formato em disco | **1 byte no `PSCH`**, ou zero se a decisão for continuar usando a marca `DadoPessoal` como o interruptor que ela já é |
| o que ganha | *ligar* a cifra numa tabela sem depender de marcar coluna por coluna — hoje só o *desligar* existe (tabela sem coluna marcada nasce em claro) |
| o que **não** ganha | nada contra quem lê o `config.json`: continua uma senha para tudo |

**É a saída barata, e ela é meia saída já construída.**

### (c) As duas — (b) agora, (a) depois

A ordem importa e é a favor: (b) não atrapalha (a), e (a) precisa do mesmo byte
de `PSCH` que (b) precisa. Fazer (b) primeiro paga a mudança de formato
**enquanto ela é barata** e deixa (a) como decisão de sessão e protocolo, sem
tocar em disco de novo.

### (d) Nada

| o que fica como está | |
|---|---|
| uma senha do servidor, no `config.json` | quem lê o arquivo tem a chave — e esse arquivo já tem o token de serviço |
| a cifra protege o **arquivo copiado** | disco levado, *backup* vazado, cópia noutra máquina |
| não há rotação de senha | trocar `cifra.senha` faz os arquivos gravados com a antiga pararem de abrir (§11.5) |
| o `.ndx` em claro | continua |

Custo zero, e é a única saída em que nada em disco muda.

## 3. As três coisas que decidem, e que não são o preço

### 3.1 Senha esquecida = dado perdido, sem recuperação

É consequência de **produto**, não de engenharia, e precisa estar dita antes da
decisão e não depois do primeiro chamado.

**Hoje** a senha mora no `config.json` e o administrador tem o arquivo: quem
esqueceu abre o arquivo e lê. **Com senha por tabela, quem esqueceu perdeu** —
a senha não é gravada em lugar nenhum, por desenho, e a prova de 16 bytes no
cabeçalho só sabe dizer *«não é esta»*. Não há recuperação, não há «esqueci
minha senha», e não pode haver: qualquer caminho de recuperação seria a chave
guardada em outro lugar, que é exatamente o que a cifra existe para não fazer.

Isso pede, na hora de implementar: um aviso na tela que declara a senha, e uma
decisão explícita do dono sobre haver ou não uma **segunda senha de
recuperação** por tabela (que é, na prática, uma segunda cópia da chave
envelopada — o desenho do envelope da §11.5).

### 3.2 O `.ndx` decide o que a senha vale

Sobre tabela **com** índice na coluna sensível, a senha por tabela protege
23.440 bytes de `.reg` e deixa 28.672 bytes de `.ndx` abertos ao lado, com
**200 de 200** pares `(valor, rowid)` recuperáveis. Sobre tabela **sem** esse
índice, protege tudo.

Então a pergunta que vem junto e que **também é do dono**: o motor deve
**avisar** — ou **recusar** — quando alguém indexa uma coluna marcada numa
tabela com senha própria? Hoje ele **mostra** (`nos_indices` na tela de
estrutura), e o comentário do `servidor.rs` já diz por quê: *«índice é o
caminho por onde o dado sai sem ninguém ler a linha»*.

### 3.3 A replicação já está quebrada, e a senha por tabela não é a causa

Medido em §12.4, e vale registrar como **defeito achado por esta frente**, não
como custo da senha por tabela:

- coluna externa (`Memo`/`Bin`) marcada **não replica nem com a mesma senha nos
  dois lados** — os sais são sorteados por arquivo, e a §11.8 pedia sal igual,
  o que nunca acontece;
- a recusa diz *«arquivo corrompido»* quando não há corrupção;
- réplica **sem** cifra grava o texto cifrado como conteúdo, **em silêncio**,
  numa coluna `Bin` (a `Memo` para por acidente, no `from_utf8`).

Consertar isso é o **envelope** da §11.5 para os dois primeiros, e uma
conferência que falta para o terceiro. Está no `PENDENCIAS.md`.

## 4. O que este parecer recusa, e com que número

| proposta | por que não | número |
|---|---|---|
| **implementar senha por tabela agora** | a ordem do dono foi medir primeiro | — |
| **levar a chave como parâmetro** em vez de mantê-la no cofre | as quatro camadas passariam a carregar segredo, e o cabeçalho do `cofre.rs` já recusa isso por escrito | **35** chamadas de `Table::criar`/`abrir` em 7 arquivos e 3 crates; **32** usos do cofre em `src/` |
| **manter o cache como está e ligar senha por tabela** | o cache só é correto porque é esvaziado a cada troca de senha | **20** derivações onde hoje são 2 — **6,0 s** de PBKDF2 em 10 pedidos, contra 0,6 s |
| **cifrar o `.ndx` para fechar o buraco da premissa 2** | destrói a ordem da B+tree; e a saída medida já existe e é outra: tirar o índice da coluna sensível | a §11.1 mediu a alternativa: cifrar **página** do `.ndx` custa 0,23 µs/linha e **não** esconde a chave, só a página em repouso |
| **«a senha por tabela resolve a replicação»** | não resolve: hoje a coluna externa já não replica, e a inline viaja em claro na imagem | **3 de 3** casos medidos falham ou corrompem |

## 5. O que ficou por medir, nomeado

- **o custo no protocolo**: quantos pedidos a mais um cliente faria para abrir
  uma tabela com senha própria, e o que acontece com o `dblink` e a réplica,
  que abrem tabela sem gente na frente;
- **a rotação**: não existe nem hoje (§11.5). Com senha por tabela, passa a ser
  N rotações em vez de uma, e sem o envelope cada uma reescreve a tabela
  inteira;
- **Argon2id** (RFC 9106) no lugar do PBKDF2: com senha por tabela, a
  derivação deixa de ser uma vez na vida do processo e passa a acontecer no
  caminho de um pedido — e aí o custo de KDF vira decisão de desempenho, e não
  só de segurança.
