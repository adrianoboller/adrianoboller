# Contar a órfã na réplica não custa «um incremento» — custa uma busca na mãe

**Estado:** PENDENTE

## O que aconteceu

Pedido 300 §2.7: a réplica (fiel e bidirecional) passou a CONTAR a filha
gravada sem a mãe (`replicacao_estado.orfas_na_replica`), pelo mesmo motor de
quem julga (`Table::conferir_as_maes` → `conferir_fks_com`), só trocando a
recusa por um número. A triagem de 23/09 (`triagem-das-decisoes-do-dono`,
«O que eu NÃO medi», item 3) estimou o custo como «da ordem de um incremento,
contra os 17.450 eventos/s já medidos».

## O que eu concluí primeiro, e estava errado

Que contar era só somar 1 num contador — o veredito «tem mãe?» parecia já
existir em algum lugar do aplicador. Não existe: a réplica NÃO julga
(`julga_integridade`), então o veredito tem de ser produzido, e produzi-lo é
abrir a mãe e buscar a chave no índice dela, e ainda ler a linha para saber se
está viva (a mãe excluída suave não conta como mãe).

## O que a medição disse

`cargo run --release --example custo-das-orfas -p phxsql-store -- 50000`, três
corridas, 01/10/2026: **6,59–6,76 µs/evento** sem contar contra **8,54–9,26
µs** contando com a mãe presente — **+1,9 a +2,7 µs por evento (+28 a 40%)**,
só em tabela com chave conferida. Com a mãe aberta UMA vez por lote (cache no
handle) e não uma por linha; abrir por linha seria muito pior. Tabela sem chave
conferida: zero (o portão vem antes do trabalho). Contra os ~57 µs/evento da
replicação pela rede (17.450/s), é ~3–5% de ponta a ponta — raciocinado, não
medido pela rede.

E o irmão do teto do bidirecional (330 b), medido no mesmo dia: o mapa de
toques custa **86–118 B/chave** de RSS (o J tinha medido 88–114 num rascunho
fora do repositório); o pico de 118 é a folga da tabela de espalhamento logo
depois de dobrar, não o custo médio.

## A regra

Antes de chamar uma instrumentação de «um incremento», pergunte de onde vem o
VEREDITO que ela conta: se ninguém o produz hoje, o custo é o de produzi-lo.

## Como está guardado hoje

O número está no `docs/REPLICACAO.md` (§13, «Filhas sem mãe na réplica são
CONTADAS») e o medidor é o `--example custo-das-orfas`. Guardas
`replica-grava-filha-sem-mae-calada` e `bidi-grava-filha-sem-mae-calada`
travam a contagem; **nenhuma trava o custo** — se alguém trocar o cache da mãe
por uma abertura por linha, só a bancada acusa.
