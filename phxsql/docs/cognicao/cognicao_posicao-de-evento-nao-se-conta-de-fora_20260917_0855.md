# A posição de um evento não se conta de fora — quem suprime é o outro lado

**Descoberto em 17/09/2026, 08:55 UTC**, implementando a parte (1) do pedido
292 (parada visível do par + `replicacao_pular`).

## 1. O que aconteceu

O pedido pede uma operação que **pule o evento pela posição**, no espírito do
`ALTER SUBSCRIPTION … SKIP (lsn)` do PostgreSQL. Para isso o servidor que puxa
precisa saber em que posição do diário da origem mora o evento que parou o par.

O lote chega por `replica::puxar_lote` como uma lista de eventos, e o pedido foi
feito a partir de `desde`. A conta óbvia é `desde + índice_na_lista`.

Ela está errada, e o motivo está escrito a quatro linhas de distância, no
próprio `op_replicar` (`crates/phxsql-server/src/servidor.rs`): o source
**suprime** os eventos cuja origem é quem pediu — é o campo `para`, que existe
para não devolver a alguém a escrita que nasceu nele — e a posição `ate` anda
por cima deles. O `LoteRecebido` até documenta isso: *«`ate` pode passar da
conta dos eventos»*. A lista que chega é mais curta que a faixa que ela cobre, e
a diferença é exatamente o número de eventos suprimidos — que num par
bidirecional em uso **não é zero**: é tudo o que este servidor escreveu e o
outro replicou.

## 2. O que eu concluí primeiro, e estava errado

Que dava para contar do lado de cá, e que o campo novo no protocolo era um luxo
evitável — zero dependência externa é pétrea, mas campo novo no fio é barato e
eu preferia não mexer no que já funciona.

E, pior, a saída que eu considerei depois: *«se o source não disser, uso `desde`
como posição — nunca passa do culpado, então é conservador»*. Conservador no
dado, sim; **inútil para quem opera**: se o evento culpado está em `desde + 7`,
pular para `desde + 1` reaplica seis eventos inofensivos, bate no mesmo conflito
e para de novo — o administrador pula sete vezes para andar uma. Adivinhar
posição é sempre ou perder dado alheio ou fazer o operador adivinhar junto.

## 3. O que a medição disse

- **O campo entrou no fio**: `op_replicar` passou a mandar
  `("posicao", desde + i)` por evento, contado **antes** da supressão, que é o
  único lugar onde o número é verdadeiro.
- **A ausência do campo virou recusa, não palpite.**
  `EventoRecebido::posicao` nasce `POSICAO_DESCONHECIDA` (`u64::MAX`) quando o
  source não o manda, e `replicacao_pular` **recusa nomeando** — «a origem é de
  uma versão que ainda não manda o campo». Teste:
  `sem_a_posicao_do_source_o_pulo_recusa_em_vez_de_adivinhar`.
- **E o portão que sai disso paga sozinho.** Com o par parado, contando os
  `replicar` que o parceiro serve em 10 s: **0** com o portão `esta_parada`
  antes do trabalho, **10** sem ele (uma por segundo, o `reconectar_em` do
  cenário). As recusas contadas deste lado subiram de 2 para 12 no mesmo
  intervalo — uma linha de diário por segundo, para sempre.

## 4. A regra

**Número que só um dos lados sabe, o outro lado não calcula — pede.** E quando
ele não vem, a operação que dependia dele **recusa dizendo isso**; ela não
inventa um valor «conservador», porque conservador no dado costuma ser
impraticável para quem opera.

## 5. Como está guardado hoje

- `crates/phxsql-server/src/replica.rs`: `EventoRecebido::posicao` e
  `POSICAO_DESCONHECIDA`, com o porquê no doc-comment do campo — inclusive a
  frase «`desde + indice_na_lista` está ERRADO no bidirecional».
- `crates/phxsql-server/src/servidor.rs`: `op_replicar` manda `posicao` antes da
  supressão; `op_replicacao_pular` recusa quando ela é desconhecida.
- `docs/REPLICACAO.md` §6 (o campo no protocolo) e §22 (o ciclo inteiro).
- Guardas: `par-parado-reapresentado-a-cada-rodada` e
  `dado-pessoal-no-grito-do-conflito`, no `bancada/guardas/catalogo.py`.

**Onde o buraco ficou:** o irmão de mão única (`alcancar_tabela` →
`aplicar_lote_da_replica` → `aplicar_evento`) recebe o campo e **não o usa** —
e não deve usar: lá a aplicação é por rowid, o `.reg` nunca reaproveita slot, e
pular um evento deslocaria todos os rowids seguintes. Está escrito na §22, no
parágrafo «O irmão de mão única NÃO mudou». O que ele ainda não tem é o grito
com conteúdo: um `Duplicado` ali sai como «chave duplicada: índice único
porEmail já tem essa chave», sem nomear tabela, chave nem linha.
