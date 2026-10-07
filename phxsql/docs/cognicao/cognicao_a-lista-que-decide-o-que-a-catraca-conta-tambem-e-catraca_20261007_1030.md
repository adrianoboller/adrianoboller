# A lista que decide o que a catraca conta também é catraca

**Estado:** PENDENTE

## O que aconteceu

Pedido 654. O `TETO_BOTAO_SEM_PROVA` chegou a zero em 02/10/2026, e parte do
caminho foi o `DISPENSADOS` do `conferidor_botoes.rs` crescer de 22 para 27.
Cada dispensa tem motivo escrito; a lista não tinha teto. O `ISENTOS` do
`conferidor.rs` (81) tinha a mesma forma. Nos dois casos, acrescentar uma
linha tirava um item da conta sem tocar na catraca.

## O que eu concluí primeiro, e estava errado

Que «catraca só desce» já cobria isso, porque o número da catraca não subiu.
Mas o número não subiu justamente **porque** a lista cresceu: a lei olhava o
número e não quem decide o que entra nele. É o alcance da lei, não uma lei
nova.

## O que a medição disse

Medido em 07/10/2026 pelo `bancada/guardas/listas-de-dispensa.py`: 27
dispensas, 81 isenções, zero sem motivo, zero vencidas (botão dispensado que
a bateria já clica). E o analisador de literal Rust contou 27, o mesmo número
da auditoria. Recortar por aspas teria contado as aspas do comentário dentro
da lista como motivos.

## A regra

Toda lista que tira itens da conta de uma catraca (dispensa, isenção,
exceção) recebe o próprio teto, que só desce, e cada linha dela precisa de
um motivo que um conferidor consiga refutar.

## Como está guardado hoje

`TETO_DISPENSADOS_DE_BOTAO`, `TETO_ISENTOS_DE_TRADUCAO`,
`TETO_DISPENSA_SEM_MOTIVO` e `TETO_DISPENSA_VENCIDA`, no `todas.py`. O buraco
que fica: o `ISENTAS` do `conferidor_grades.rs`, o `ISENTOS` do
`conferidor_temporarios.rs` e as marcas `# nao-e-catraca:` do
`docs/qa/medir.py` têm a mesma forma e ainda não têm teto.
