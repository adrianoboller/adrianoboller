# Board da rodada 0.19 — aberto em 23/09/2026 03:0x

Arquivo de apoio e controle de contexto (pétrea do `CLAUDE.md`). **Não é
entregável de produto.** Só o integrador comita, por caminho explícito.

## O que motivou a rodada

Pergunta do dono, 23/09/2026: *«Essa versão do motor não sai do 18. Tem muitos
gaps a serem feitos. Algo que eu possa te ajudar?»*

Medido na hora, e a primeira metade é **defeito, não estado**:

- `version = "0.18.0"` foi selada em **29/08/2026** (`baff46e`) e há **887
  commits** desde então. O número é **digitado à mão** no `Cargo.toml` e
  **nenhuma regra, portão ou gerador decide quando ele sobe**. O selo da capa
  do dossiê sai de gerador — mas o gerador **lê a linha digitada**. É a lei da
  casa quebrada um nível acima: o gerador está certo e a **fonte** dele
  envelhece calada.
- Agravante: o `empacotar.sh` trava «versão igual em `Cargo.toml`/`Cargo.lock`/
  `MANUAL`/`CHANGELOG`». Confere que as quatro **concordam**; nenhuma confere
  se o número ainda **descreve** o que existe. Quatro cópias do mesmo número
  velho passam na trava.
- Gaps: **101 abertos** — 87 planejados + 14 parciais. Deles **15 paravam no
  dono**, 21 são de segurança (4 BLOQUEIO/ALTO).

## O que o dono decidiu, em duas rodadas de pergunta

| # | decisão | quando |
|---|---|---|
| versão | portão da versão **e** selar 0.19.0 agora | 23/09 |
| 393 | o braço do `unir` vira **pedido**, não nome de tabela | 23/09 |
| 324 | **só o regime (b)**: adiar o `.ndx` sob a reserva, **sem thread** | 23/09 |
| 314 | ledger com cadeia **fica em v9**, e o motor **diz** o motivo | 23/09 |
| 342 | **exigir a cifra do fio** quando a tabela tem coluna marcada | 23/09 |
| 340 | saída **(d)**: selar a página do `.fts` — entra por aceite automático | 23/09 |
| 289 | nanos com **avanço forçado**, 8 bytes, `u64` | 17/09 07:10 |
| 290 | **passo** no esquema, **início** na identidade do nó | 17/09 07:10 |
| 355 | recusar na **declaração**: ledger + coluna marcada não nasce | 18/09 |

**Quatro dos que ele marcou já estavam decididos e só esperavam engenharia.**
Isso é achado de processo: pedido que carrega a decisão do dono no corpo
continua marcado «Planejado» e some da vista como se estivesse travado nele.

## Frentes desta onda

| frente | pedidos | nível do modelo | por quê |
|---|---|---|---|
| **P** | 289 + 290 + 314 | forte | formato em disco, migração que não se desfaz |
| **S** | 340 + 342 | forte | criptografia e formato juntos |
| **U** | 393 | forte | a trava única morre; é concorrência |
| **V** | versão + 0.19.0 | leve | regra mecânica e verificável |

**Dispensados com motivo** (dispensa registrada é decisão; silenciosa é
esquecimento): **E** designer — nenhuma frente toca tela; **J** pesquisador —
as quatro já vêm medidas; **D** zelador — roda de hora em hora e está
silencioso.

## A triagem do papel J, e o placar que justifica a lei nova

Feita em 23/09/2026 contra o **help e o fonte** de PostgreSQL, MariaDB, MySQL e
SQLite. Documento: `docs/propostas/triagem-das-decisoes-do-dono-2026-09-23.md`.

| classe | quantos | quais |
|---|---:|---|
| **A** — convergência, entra sem pergunta | **4** | 251, 255, 300, 309 |
| **B** — voto ponderado | 1 meia | «reparar sozinho» do 255 (**6 × 4**) |
| **C** — é do dono | **4** | 325, 333, 337, 368 |
| **fora da fila** | **3** | 274, 293, 294 |

**A fila do dono caiu de 11 para 4 na primeira aplicação da lei.**

E os três «fora da fila» são o defeito do **340** outras três vezes: o **274**
já está entregue (o **378** o fechou em 22/09 e diz isso no corpo — conferido
no fonte: a frase «a `std` não traz TLS» não existe mais), e o **293** e o
**294** trazem **«DECIDIDO PELO DONO, 17/09/2026 07:10 UTC»** por extenso.
Estavam parados esperando o dono decidir o que ele já tinha decidido.

**O mais grave dos A é o 300**, e é divergência nossa com a restrição nomeada:
a posição da réplica é **contagem local** (`servidor.rs:3807-3825`) quando nos
três maduros ela é a coordenada da **ORIGEM** (LSN, `gtid_slave_pos`,
`gtid_executed`). Eles têm diário único; nós temos `.log` por tabela, modelo
HFSQL — então a receita entra como «a posição é a do evento que veio, por
tabela».

**Brinde medido:** das 10 referências `arquivo:linha` citadas nos onze pedidos,
**7 estavam deslocadas** — conteúdo certo, coordenada movida. J **não** propôs
gerador, e fez bem: seria catraca sem defeito medido que a motive.

## Decisões do dono de 23/09, segunda leva

| # | decisão |
|---|---|
| **333a** | o chat do PhxMail é **ponta-a-ponta** — servidor não busca no texto, robô não lê |
| **333b** | **escrever TLS aqui**, como foi o SHA-256. Nada de crate |
| **337** | **trocar a razão escrita** da pétrea de zero dependências: de «compila offline» para **instalação simples**. A pétrea fica intacta |
| **368** | o expurgo do `.lgpd` **nasce desligado** (padrão do MariaDB); existe e quem quiser liga |

**325 continua sem resposta** — abrir ou não a frente das N pontas (cada caixa
como `source`, central como `replica` multi-origem). Não foi perguntada porque
não há capacidade nesta onda.

**O 333b é o maior pedaço de criptografia que esta casa já encarou**, e entra
pelo método que deu o SHA-256: norma lida, entendida, reescrita, provada contra
vetor oficial. **Não começa por código** — começa por J medindo o escopo
(X.509, ASN.1, validação de cadeia) contra o nosso gargalo. «Cripto de
transporte malfeita é pior que nenhuma» é a frase que governa a frente.

## Onda 2, e o motivo de não ser paralela

**324** (adiar o `.ndx` sob a reserva) mexe na reserva dentro do `servidor.rs`;
**393** mexe na trava do mesmo arquivo. Esta casa já pagou **três defeitos que
só apareceram no encontro das frentes**. Os dois entram em ordem, não juntos.

## Cerca de cada frente (para o defeito não nascer no merge)

- **P**: não toca `op_unir`, reserva do BULKINSERT, `.fts`, `Cargo.toml`.
- **S**: não toca `PSCH`/`schema.rs`, `op_unir`, reserva, `Cargo.toml`.
- **U**: não toca `PSCH`/`schema.rs`, `.fts`/cofre, reserva, `Cargo.toml`.
- **V**: **único** autorizado em `Cargo.toml`/`Cargo.lock`; não toca motor.

Quem precisar de algo fora da cerca **nomeia no relatório** em vez de mexer.

## Estado da árvore ao abrir

Ponta `e1edcc3`, local e no `origin` conferidos por `git ls-remote`. Árvore
limpa. Portão dos geradores VERDE (27 ok, zero velhos). Backup provado
(`phxsql-20260923-0117.bundle`, 1019 commits, restaurado e comparado). Suíte
**2.647 passando / 0 falhando**, `clippy` zero avisos. As sete páginas
republicadas.

## O que fica para o dono, ainda

- **326** — o dossiê compartilhado mostra uma versão **FIXADA**: quem abre o
  link não vê o que a gente publica. A troca é pelo menu Share, que o agente
  não alcança.
- As outras decisões travadas nele: 251, 255, 274, 293, 294, 300, 309, 325,
  333, 337, 368.

## Conferência de integração: o que TEM de estar fechado no `PENDENCIAS.md`

Escrito em 23/09/2026 04:25, com as quatro frentes ainda vivas, e por um motivo
medido: a **S já escreveu** no `PENDENCIAS.md` (fechou 340 e 342, abriu o 395)
quando a cerca mandava nomear no relatório. Se uma segunda frente reescrever o
arquivo inteiro em vez de editar a linha, o trabalho da primeira some **sem
conflito nenhum aparecer** — é o defeito que esta casa já pagou três vezes no
encontro das frentes, e o merge escolhe o lado de quem não tinha a seção.

Não interrompi as quatro para avisar: interromper quatro frentes no fim do
trabalho custa mais que conferir depois, e **conferir depois é medição, não
esperança.** A lista abaixo é a régua.

| dono | pedido | estado esperado ao fim |
|---|---|---|
| **P** | 289 | ☑️ nanos com avanço forçado |
| **P** | 290 | ☑️ passo no esquema, início na identidade do nó |
| **P** | 314 | ☑️ fica em v9 **e o motor diz o motivo** |
| **S** | 340 | ☑️ — **já escrito** |
| **S** | 342 | ☑️ — **já escrito** |
| **S** | 395 | ☐ nasce aberto (o `.ndx` pelo mecanismo do `.fts`) — **já escrito** |
| **U** | 393 | ☑️ o braço do `unir` vira pedido |
| **V** | versão | ☑️ portão da versão, 0.19.0 selada |
| **A** (integrador) | 274 | ☑️ — entregue pelo **378** em 22/09, nunca fechado |
| **A** | 293 | corrigir: traz **«DECIDIDO PELO DONO, 17/09 07:10»** e segue ☐ |
| **A** | 294 | idem |

**Como se confere, e não é por leitura:** `git diff -U0` no arquivo, extraindo
só as linhas de pedido alteradas. Falta de uma linha esperada é frente
clobberada, não esquecimento — e aí o conserto é repor a linha, não repetir a
frente.

Contagem ao abrir a conferência: **395 pedidos** no arquivo.

## Achados do integrador com as frentes ainda vivas (23/09, 04:25–04:35)

### O `frente-p` que o zelador recusava: 2,0 GB, e o motivo dele estava errado

O zelador achava «cópia sem `git` fora do repositório, 2.052 MiB» e recusava
apagar. Estava certo em recusar e **errado nos dois motivos**:

- Não é «fora do repositório»: estava no **scratchpad da própria sessão**, que
  o varredor dele não alcança — é por isso que ele conta «16 diretórios soltos,
  0 MiB» enquanto o `/tmp` carrega 5,5 GB. **Defeito de medidor, não de disco.**
- Não é «sem `git`»: é **worktree registrado** (`git worktree list`), em
  `832a888`, HEAD solto. O `.git` de um worktree é **arquivo**, não diretório —
  quem testa `-d .git` chama worktree de cópia solta.

A prova antes de remover, na ordem que a lei do zelador manda:

| prova | resultado |
|---|---|
| processo vivo com `cwd` lá dentro | **nenhum** (varredura de `/proc/*/cwd`) |
| arquivos idênticos à árvore principal | **18 de 21**, byte a byte (`cmp`) |
| os 3 que diferem | a **principal está à frente** — um é o conserto do `clippy` que a P2 aplicou às 04:25 |

As 1.334 inserções ficaram guardadas como remendo em
`scratchpad/frente-p-remendo/` (156 KB: `rastreados.patch` + os 3 novos +
`base.txt`) **antes** da remoção, para a decisão ser reversível. Disco tinha
caído a **3,6 GB** durante a medição e voltou a **5,6 GB**.

### As três conferências do encontro das frentes

Feitas na árvore viva, porque é ali que o defeito do encontro existe:

| conferência | resultado |
|---|---|
| alguma catraca subiu? | **nenhuma tocada** — só prosa citando `TETO_JUNCAO` |
| algum teste calado? | **nenhum** `#[ignore]`, nenhum `#[test]` comentado |
| coluna de sistema nova quebra a tela? | **não** — e por estrutura, não por sorte |

A terceira era a de risco real: a P não acrescentou **uma** coluna de sistema,
acrescentou **duas** — `rowstamp` e `rowtime` —, e **nenhum arquivo de
interface está entre os 42 sujos**. Foi exatamente assim que o `rownum` quebrou
todo salvar e todo incluir pela tela.

Desta vez a corrente segura, e o mérito é de quem transformou a lição em
estrutura:

- `phxsql_core::schema::e_coluna_de_sistema` é **uma função só**, e o comentário
  dela registra que a mesma pergunta esteve repetida em **quatro** lugares (a
  sincronia do DbLink, a chave única do bidirecional, o `completar` do `Table` e
  a carga por texto) — os quatro passam por ali agora.
- `servidor.rs:17248` monta a marca `"sistema"` do JSON **a partir dessa
  função**, não de uma lista.
- A tela filtra **genericamente** nas seis chamadas (`filter(c => c.sistema)`),
  e a cicatriz está escrita no `index.html:5068`: *«era `find(c => c.sistema)`,
  só o `softdeleted` saía e o `rownum` continuava na ficha»*.

**A lição virou estrutura, e por isso duas colunas novas custaram zero.** É o
contraexemplo da lei «guarda nova entra pedida»: aqui a guarda entrou no lugar
certo — a função única —, e não em quarenta pontos de chamada onde o esquecido
vira a porta dos fundos.

## V2 voltou: o portão da versão presta, e o selo passa a ser o ÚLTIMO passo

Conferência feita em 23/09/2026 04:47 (a frente V2 mediu; o integrador
reconferiu cinco afirmações uma a uma, porque relatório de agente não vale
pelo valor de face).

| o que V2 afirmou | reconferido pelo integrador |
|---|---|
| o `Cargo.toml` voltou byte a byte do teste do vermelho | **sim** — `version = "0.19.0"` |
| o portão roda verde, saída 0 | **sim** |
| régua medida, não digitada (versão, commit da selagem, distância) | **sim**, e o teto 51 foi recalculado à parte, batendo com a tabela |
| o CHANGELOG diz **890** commits, o real é 891 | **é 892** — o `b189098` do integrador empurrou mais um |
| `CAPABILITIES.json` parado em `0.18.0` | **sim** |
| tag Docker `phxsql:0.18.0` digitada à mão | **sim**, `bancada/docker/montar-dois.py:25` |

**O 890 → 891 → 892 em quarenta minutos é o achado, não o erro.** A frase do
próprio CHANGELOG — «quem selar por último confere de novo, porque a árvore é
compartilhada e o número anda» — previu isso, e a prova veio sozinha **duas
vezes**. Número medido numa árvore com quatro frentes vivas tem validade de
minutos, e é por isso que ele se remede no instante do commit final.

### A dependência que reordena a integração

O `CHANGELOG.md` consolida só o que **já estava commitado** antes desta rodada.
Ele não tem seção para 289, 290, 314, 324, 340, 342, 393 e 395 — os pedidos que
P, S e U estão fechando **agora**. Não é defeito da V: não se descreve commit
que ainda não existe. Mas a consequência é de ordem:

> **O selo da 0.19.0 é o ÚLTIMO passo da rodada, não o primeiro.** Primeiro
> entram P, S e U; depois se escreve a seção deles no CHANGELOG; só então se
> remede a distância e se sela.

### Os três que V2 nomeou para o `PENDENCIAS` (ela não escreveu lá, como a cerca mandava)

1. **`CAPABILITIES.json` desatualizado** — regenerar com `numeros-do-projeto.py`
   antes da selagem. Enquanto não rodar, `status.html`, `testes.html` e
   `graficos.html` publicam **0.18.0**, porque os três leem daquele arquivo.
2. **Tag Docker digitada** em `bancada/docker/montar-dois.py:25` e
   `dblink-mariadb.py:76` — vão apontar para imagem velha depois do bump. O
   irmão `bancada/replicacao/docker/provar.py:72` já resolve certo, com
   `phxsql-bancada:local`, sem número nenhum.
3. **Seção nova no CHANGELOG** para os pedidos desta rodada, e a distância
   remedida no commit final.

### Portões no instante em que V2 mediu

- `clippy --workspace --all-targets`: **zero avisos, saída 0** — o aviso do
  `table.rs:2552` sumiu (P o consertou ao vivo).
- `fmt --check`: **1 divergência**, em `valores.rs` — na linha **1331**, não
  1316: o arquivo está sendo editado enquanto se mede, e a coordenada anda
  junto. É de P, e fica com P.
