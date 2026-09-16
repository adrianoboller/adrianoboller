# Visão — os três pilares sobre um motor só

Ordem do dono, 16/09/2026: *«A filosofia do projeto deve ter a finalidade de
ter uma nova base de dados, um novo sistema de e-mails P2P, uma estrutura
blockchain para uso genérico para gerenciar mini-contratos sigilosos.»*

O produto deixou de ser uma base de dados. São **três**, e o que os une é o que
esta casa nunca abriu mão: **zero dependências externas, só a `std`**. Tudo o
que os outros puxam de uma crate — JSON, CRC-32, SHA-256, HMAC, PBKDF2,
Ed25519/SHA-512/X25519/ASN.1 — já é escrito aqui e já está em uso. Os três
pilares reaproveitam essa fundação; nenhum a contradiz.

## Pilar 1 — a base de dados (PhxSql)

O que já existe, e o mais maduro dos três. Motor de arquivos separados no molde
do HFSQL, em Rust, com replicação medida, cluster com eleição, transações
(`BEGIN`/`COMMIT`/`ROLLBACK`/`SAVEPOINT`), integridade referencial imposta na
gravação, cifra em repouso e no fio, SQL que traduz para o protocolo, e a régua
de honestidade da casa (número medido, prova real nos dois sentidos, catraca que
só desce). Estado por recurso em `docs/STATUS.md`; o que falta em
`docs/PENDENCIAS.md`.

**O que os outros dois pilares herdam dele:** o formato em disco e as suas
garantias (ordem de digitação sagrada, nunca reusa slot, chave conferida nasce
com índice dos dois lados), o diário replicável evento a evento, e a criptografia
caseira provada contra vetor oficial.

## Pilar 2 — o sistema de e-mails P2P

Domínio **novo**. O pedido antigo (o #159, «PhxSql como plataforma de correio:
server + client mail») mirava um correio cliente-servidor; a ordem de 16/09 o
reenquadra como **P2P** — sem servidor central de entrega, os pares trocam
mensagem direto. Isso muda o problema difícil de «montar um SMTP» para
**transporte entre pares**: descoberta, travessia de NAT, gossip/anti-entropia
para a caixa que esteve offline, e identidade que não depende de um domínio.

O que já temos a favor: o aperto de mão cifrado do fio (estilo Noise, Ed25519 +
X25519, pedido 61), o diário anti-entropia da replicação (um par que volta
alcança o que perdeu, evento a evento) e a cifra em repouso da caixa. O que é
projeto e risco novo, e por isso vai no escalão forte e no subagente
`pesquisa-rede`: **como os pares se acham e sincronizam sem um servidor** —
medido contra o nosso gargalo antes de virar plano, como manda a casa (Delta
Chat sobre e-mail, o gossip do Cassandra, DHTs — inspiração, nunca cópia).

## Pilar 3 — a blockchain de mini-contratos sigilosos

Domínio **novo**, com fundação já paga. A pesquisa está feita e medida em
`docs/propostas/phxblockchain-melhorias-2026-09.md` (oito técnicas de fonte
primária, cada URL registrada, 16/09/2026). O modo *ledger* encadeado privado já
existe como convenção de esquema (`hash`/`anterior` Uuid256, `altura` Sequence,
índice único `porAltura`), e o `ALTER TABLE` já é recusado numa tabela-cadeia
(pedido 161). A cadeia é **tamper-EVIDENTE** (detecta relendo), não
tamper-RESISTENTE — só a âncora externa fecha o furo do adversário que controla
o nó.

O termo que a ordem acrescenta é **sigiloso**: um mini-contrato guardado no
ledger não pode vazar o conteúdo para quem lê a cadeia. Isso soma um requisito
de **confidencialidade** ao que era só integridade — cifrar o corpo do contrato,
provar a existência e a ordem sem revelar o teor (o caminho é o compromisso
Merkle/hash sobre o texto cifrado, não sobre o claro). É projeto e risco de
criptografia: escalão forte, com `seguranca` (revisor adversário) e
`pesquisa-motor` (a norma e o vetor) como donos, e o `dba` sobre o formato em
disco da cadeia.

## O que a visão decide de imediato

- **Escopo do «projeto e risco»** cresceu: transporte P2P e sigilo de contrato
  entram na lista que o `docs/MODELOS.md` usa para escolher o escalão forte.
- **Nenhum pilar quebra o zero-dependências.** Se algum parecer exigir uma
  crate (TLS de biblioteca, um stack de rede pronto), a pétrea manda **perguntar
  antes** — ou se escreve aqui, como o SHA-256, ou se discute com o dono.
- **A ordem entre os pilares é do dono.** Esta página registra a finalidade; o
  que se faz primeiro sai do `docs/pmo/BACKLOG.md`, redistribuído por escalão
  para não gastar token à toa.
