# O irmão que NÃO deve receber o conserto: a recusa da réplica sem cofre e a restauração

**Estado:** PENDENTE

## O que aconteceu

Pedido 613 (decisão do dono, 01/10/2026): réplica sem cofre recusa a tabela
com coluna externa marcada. A recusa entrou no `Table::aplicar_evento`, que é
o caminho da réplica. Quatro chamadores de produção passam por ele: o laço da
réplica (`servidor.rs`, `aplicar_evento` do lote), a op `aplicar`, a FFI
(`phx_aplicar_evento`) e a **restauração** do diário vivo (PITR), que reaplica
o diário do PRÓPRIO servidor numa cópia local.

## O que eu concluí primeiro, e estava errado

Que bastava pôr a recusa dentro do `aplicar_evento`: «é o ponto único, todos
os irmãos ganham». Os quatro chamam as mesmas funções na mesma ordem
(`imagem_para_o_fio` → `aplicar_evento`), então pela régua da casa são
irmãos. Mas a restauração não é «fora da origem»: o arquivo vivo e o
restaurado moram no mesmo disco, com o mesmo cofre ou a falta dele. Com a
recusa ali, a restauração de um servidor sem cofre pararia em toda tabela com
anexo marcado sem proteger um byte.

## O que a medição disse

Na leitura dos chamadores, antes de rodar a suíte: 4 chamadores de produção,
1 deles (a restauração) fora do alcance da decisão. O teste de store
`imagem_aberta_para_o_fio_replica_com_e_sem_cofre` caiu na primeira corrida
(ele afirmava o preço que o dono recusou), e foi reescrito para a recusa.

## A regra

Antes de pôr um conserto no ponto único, pergunte a cada chamador se a
PREMISSA do conserto vale para ele. A exceção ganha um **nome à parte**
(`reaplicar_evento_do_proprio_diario`), e não um interruptor: o padrão
continua sendo a recusa, e quem replica não consegue pulá-la por engano.

## Como está guardado hoje

Guarda `replica-sem-cofre-grava-externo-marcado-em-claro` (PROVADA) cobre a
recusa. A exceção nasceu sem prova — nenhum teste de restauração usava coluna
marcada — e ganhou uma no mesmo passo:
`servidor::testes_pitr::restaurar_sem_cofre_reaplica_o_anexo_marcado`, com a
guarda `restauracao-recusa-como-replica-sem-cofre` (PROVADA: reposto o
`aplicar_evento` na restauração, a linha de depois da cópia não volta). Fica
PENDENTE porque a regra («pergunte a cada chamador se a premissa vale») ainda
não foi aplicada por outra frente e medida.
