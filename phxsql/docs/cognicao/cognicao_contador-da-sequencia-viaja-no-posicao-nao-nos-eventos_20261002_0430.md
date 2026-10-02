# O contador da `Sequence` viaja no `posicao`, porque os eventos chegam depois

**Estado:** PENDENTE

## 1. O que aconteceu

Pedido 229, ponta «contador durável propagado». A promoção de uma réplica
atrasada reemitia números que o master já tinha entregue (bloco 23 do
`docs/AUTONUMBER.md`). O contador mora no cabeçalho do `.reg` e a réplica só o
empurrava com os valores das linhas que **aplicava** — o que o master emitiu e
a réplica não recebeu não estava em lugar nenhum daqui.

## 2. O que eu concluí primeiro, e estava errado

Três hipóteses escritas antes de medir. A que parecia certa era a segunda: «a
promoção já relê o maior valor gravado (`reparar`), então o buraco é só
documentar». Não cobre: o `reconciliar_sequencia` varre o `.reg` **desta**
ponta, onde o 3 a 5 do master não existem. A terceira — «o `.seq` replica e a
promovida o herda» — também morreu: o `.seq` não replica (decisão §C.5.3), e a
promovida recusa pedir o `proximo` de uma sequência que nunca viu.

E um erro de **alcance** que quase passou para a documentação: achar que
propagar o contador fecha o bloco 23. Fecha o atraso de **vazão** (réplica no
ar, eventos ainda não aplicados). O atraso de **rede** — o master emitiu e
morreu antes da réplica perguntar de novo — não fecha com protocolo
assíncrono nenhum; a sonda do bloco 23 (réplica congelada, master morto)
continua dando 5.

## 3. O que a medição disse

Pelo soquete, source e réplica reais, repetidor de linha segurando só o
`replicar` (`crates/phxsql-server/tests/contador-na-promocao.rs`): source com
ids 1–5, réplica com 1–2. **Antes:** a promovida deu o **3** (contador da
réplica: 3). **Depois:** deu o **6**. O campo viaja no `posicao` e não nos
eventos justamente porque o `posicao` é a pergunta que a réplica faz **antes**
de puxar evento — o contador chega mesmo com os eventos presos.

## 4. A regra

Estado que a réplica precisa para assumir viaja pelo canal que chega **antes**
dos dados, não dentro deles; e o conserto entra no ponto que os dois caminhos
(pull e quórum) atravessam — o `abrir_para_replicar` —, não em cada um.

## 5. Como está guardado hoje

Teste do soquete e quatro guardas no catálogo (`contador-do-source-nao-adotado`,
`posicao-sem-o-contador-da-sequencia`, `lote-do-quorum-sem-o-contador-da-sequencia`,
`adocao-do-contador-nao-anda`). **Buraco que fica:** o atraso de rede não tem
guarda porque não tem conserto neste desenho; a `DIVIDA` do
`reconciliar_sequencia` o nomeia. A sonda do bloco 23 não foi rodada de novo.
