# O `id()` de um modelo envolvido diz quem foi pedido, não quem atendeu

**Estado:** PENDENTE

**Evidência (o que existe, e por que ainda não promove):** `tests/roteamento.rs`
(`o_motor_grava_passo_e_evidencia_de_quem_atendeu`) prova que o passo `modelo` e a linha
`modelo.roteamento` da evidência dizem `atendeu x:b` quando o `id()` do modelo é `rota:x:a`; com
o registro do motor reposto como defeito, o teste cai. Falta a prova do lado do preço:
o `motor.rs` da frente R2/R3 cobra por `self.llm.id()`, e nenhum teste ainda roda `rota` com
tabela de preços.

## O que aconteceu

R4 pôs um `Llm` que envolve vários (`LlmRoteado`). O `id()` dele tem de ser `rota:<spec>`, porque
a retomada remonta o agente pelo `task.model`. No mesmo dia a frente R2/R3 passou a precificar
cada chamada pelo `self.llm.id()`. Com rota, o preço procuraria `rota:ollama:x` na tabela: o
custo sai «não medido» (e o orçamento em dinheiro recusa antes de gastar). Falha segura, mas o
número que devia existir não existe. Quem atendeu está no diário (`roteamento::com_diario`), por
chamada.

## O que eu concluí primeiro, e estava errado

Que bastava pôr na `LlmReply` um campo com o roteamento. Medido antes: 19 construtores literais de
`LlmReply` no repositório, e a outra frente mexendo nos tipos vizinhos (`Usage`, `Task`) no mesmo
dia. O campo novo quebraria 19 lugares e colidiria com a outra frente sem nada aparecer no
conflito de texto. O diário por tarefa do tokio (`task_local!`) leva o dado sem mudar tipo de fio.

## A regra

Envoltório de `Llm` (gravador, medidor, rota) muda o significado do `id()`: quem precifica,
compara ou mede por modelo usa o modelo que ATENDEU (o diário), não o `id()` do envoltório.

## Como está guardado hoje

`roteamento::Atendimento::atendeu`. O ajuste do preço (usar o `atendeu` quando houver diário) é
ponto de integração com R2, não feito aqui para não reescrever código de outra frente em curso.
