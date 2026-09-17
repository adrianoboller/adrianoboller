# O parecer não sabe o que já foi decidido — o integrador é quem sabe

**Descoberto em 17/09/2026, 13:43 UTC**, integrando o parecer do papel C sobre
adiar o `.ndx` no `BULKINSERT`
(`docs/propostas/parecer-dba-adiar-o-ndx-no-bulkinsert-2026-09-17.md`).

## 1. O que aconteceu

O DBA devolveu um parecer de 612 linhas com seis itens na §8, «o que eu NÃO
consegui provar por leitura». Três deles pareciam defeito novo e mereciam
pedido próprio. Fui abrir os três.

**Um deles não era defeito: era decisão, tomada e escrita 11 horas antes, no
mesmo dia.** A §3.4 do parecer mostra que a unicidade fica fora do
`julga_integridade()` (`table.rs:3158-3166`, medido: todas as outras oito
conferências do arquivo têm a guarda, nas linhas 874, 1844, 2902, 3114, 3414,
3511, 3638 e 3745), que `aplicar_evento` liga `como_replica = true`
(`table.rs:3993`) e que o `?` de `servidor.rs:2941` faz a posição nunca andar —
o par de servidores parado para sempre.

Tudo verdade. E o pedido **292**, fechado às 09:0x do mesmo dia, já dizia, na
letra:

> **E o irmao que ela NAO mexeu, com a decisao escrita**: o unidirecional para
> pelo mesmo desenho e **assim fica**, porque la a aplicacao e por rowid e o
> `.reg` nunca reaproveita slot — pular um evento deslocaria todos os rowids
> seguintes e trocaria um problema que PARA por um que DIVERGE EM SILENCIO.

E o pedaço que ainda falta — o grito pobre do unidirecional — **já é o pedido
319**, aberto.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o parecer tinha achado três defeitos e que o meu trabalho de
integração era **transcrever** os três para o `PENDENCIAS.md`. É a conclusão
natural: o papel C é sênior, leu o fonte, citou arquivo e linha, e nada no
parecer estava errado.

O erro não estava no que o parecer afirmava — estava no que ele **não tinha
como saber**. Um parecer lê o **código**. O registro do que já se pesou e se
decidiu mora no `PENDENCIAS.md`, e esse arquivo tem hoje 323 pedidos, alguns
com 60 linhas de decisão dentro. Nenhum papel convocado para uma pergunta
delimitada lê isso — nem deve.

E a mesma armadilha já tinha me pegado **duas horas antes, pelo outro lado**:
respondi ao dono que adiar o `.ndx` compraria «5 a 6×» sem conferir que o
pedido **114** já recusava o item com número desde 29/08/2026. Ali eu deixei
uma **recusa medida** voltar sem número novo; aqui eu quase deixei uma
**decisão registrada** voltar como pedido. São a mesma falta pelos dois
sentidos: o registro existia e eu não o consultei antes de agir.

## 3. O que a medição disse

Três candidatos a pedido saíram do parecer. Conferidos contra o registro
(`grep -niE 'julga_integridade|barrado_por_carga|Portao 4|reservada para carga'
docs/PENDENCIAS.md`, mais a leitura do 292 inteiro):

| Candidato | Veredito | Onde já estava |
|---|---|---|
| Portão 4 olha um campo só; `juntar`/`unir`/`pivotar`/`diferencas` escondem a tabela | **novo** → pedido 322 | em lugar nenhum; `barrado_por_carga` tem 1 chamador no repositório inteiro |
| Unicidade fora do `julga_integridade` trava a réplica | **decidido** → nenhum pedido | pedido 292 (bidirecional, FEITO) + pedido 319 (o grito, ABERTO) |
| Pedido 255 como pré-requisito (R5) | **já existe** → nenhum pedido | pedido 255, aberto |

**Um de três.** O filtro matou 67% dos pedidos que eu ia abrir, e o custo de
aplicá-lo foi um `grep` e a leitura de um pedido.

E o que ele evitou não é papelada a mais: uma terceira cópia da mesma decisão,
num arquivo onde ela já está escrita duas vezes, é **um lugar onde a lei pode
divergir de si mesma** — e quem lesse o pedido novo, dali a três meses,
reabriria uma discussão fechada com o argumento certo já registrado.

## 4. A regra

**Parecer de papel não se transcreve: confere-se contra o registro antes de
virar pedido.** O que o papel sabe é o código; o que o integrador sabe é o que
já foi pesado. Todo achado passa por três peneiras, nessa ordem — *já é
pedido?*, *já foi decidido?*, *já foi recusado com número?* — e só o que
sobrevive às três vira linha nova.

## 5. Como está guardado hoje

**Guardado em parte, e o buraco fica nomeado.**

- O pedido **322** nasceu desta peneira (o único dos três que passou), e a
  linha dele diz o que está medido por leitura e o que falta medir pelo
  soquete.
- O **114** e o **292** continuam sendo a prova de que o registro funciona
  quando é lido — e de que ele não se lê sozinho.
- **O que NÃO existe é uma guarda.** Não há nada que reprove um pedido novo
  que repita decisão já escrita; a peneira é disciplina de quem integra, e
  disciplina é exatamente o que esta casa não aceita como garantia em nenhum
  outro lugar. Um conferidor plausível — casar o título do pedido novo contra
  os títulos existentes — **não serviria**, e o motivo é o mesmo de sempre: o
  292 fala de «bidirecional» e o achado falava de «unidirecional»; nenhuma
  comparação de texto os aproxima. O que os liga é o **mecanismo**
  (`julga_integridade` + o `?` que não deixa a posição andar), e mecanismo não
  se acha por padrão de frase.
- Fica, então, como o custo declarado: **a peneira é manual, e o registro dela
  é este arquivo.** Papel que não está cumprindo aparece como não cumprindo.
