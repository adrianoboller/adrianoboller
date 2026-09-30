# O DateValid do WLanguage é juliano antes de 1582; o Rust gerado era gregoriano

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxclaw-ui-ir/tests/regras.rs` (crate gerado, teste `calendario`):
`29/02/1500` válido e `10/10/1582` inválido. Com o calendário antigo o teste falha na linha
do 1500; com o novo, passa (medido em 30/09).

## O que aconteceu

O gerador emite as mesmas regras em Rust e em WLanguage. Para data, o WLanguage usa
`DateValid`; o Rust usava uma conta gregoriana escrita de memória. A paridade de
mensagens passava, e mesmo assim os dois recusavam datas diferentes.

## O que eu concluí primeiro, e estava errado

Que conferir as mensagens bastava para provar que os dois backends eram o mesmo. Mensagem
igual não prova regra igual: o texto «não é uma data válida» saía nos dois, mas para
entradas diferentes.

## O que a medição disse

O Help do `DateValid` (página 3027003, corpus Help_WL_12k): anos de 0001 a 9999; antes
de 04/10/1582 vale o calendário juliano; depois de 15/10/1582, o gregoriano. O Rust aceitava
o ano 10000, recusava 29/02/1500 e aceitava 10/10/1582, que não existe.

## A regra

Backend que espelha função de outra plataforma espelha a **documentação** dela, não a
intuição sobre o que a função «deveria» fazer. E a prova de paridade cobre o comportamento
nos casos de borda documentados, não só o texto da mensagem.

## Como está guardado hoje

O teste `calendario` do crate gerado e a lista `FUNCOES_CONFERIDAS` do `wlanguage.rs`,
que carrega a página do Help de cada função emitida; o teste `so_emite_funcao_conferida_no_help`
reprova função nova sem conferência.
