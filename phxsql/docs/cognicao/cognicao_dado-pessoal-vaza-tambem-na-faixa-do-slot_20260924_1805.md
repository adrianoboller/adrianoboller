# O dado pessoal vaza também na FAIXA do slot, depois da conversão

**Estado:** PENDENTE

*24/09/2026, 18:05 — pedido 464.*

## 1. O que aconteceu

O pedido 464 dizia: «a conversão não recebe a marca `DadoPessoal` da coluna,
e o valor curto (um CPF) sai inteiro na recusa». O conserto óbvio era a
conversão do protocolo (`json_para_valor`) receber a coluna.

## 2. O que eu concluí primeiro, e estava errado

**«A recusa que cita o valor é a da conversão; consertada ela, acabou.»** A
conversão de `"99988877766"` para uma coluna `Int4` **passa**: o texto vira
`Int` de 64 bits sem erro. Quem recusa é o **slot**, depois, no
`escrever_inline` do `phxsql-store`, conferindo a faixa do tipo — e a frase
dele cita o número: `99988877766 nao cabe em inteiro de 32 bits`. Um conserto
só no conversor passaria nos testes de data e texto e deixaria o CPF escrito
em dígitos vazar pela camada de baixo.

E um segundo irmão com a mesma forma: a carga colada não passa pelo
`json_para_valor` — converte célula por célula no `valor_de_texto` do núcleo,
com a própria frase citando a célula.

## 3. O que a medição disse

Antes do conserto, com `nasc Date` e `doc Int4` marcadas:

- `[SP000018] tipo invalido: data invalida: "999.888.777-66" (use AAAA-MM-DD)` — conversão;
- `[SP000018] limite excedido: 99988877766 nao cabe em inteiro de 32 bits` — slot.

Cada um dos três irmãos (upsert, carga colada, faixa do slot), reposto, fez
cair só o teste dele; o do protocolo e o do comportamento velho seguiram.

## 4. A regra

**A recusa que cita um valor se procura em todo lugar que RECUSA o valor, e
não só onde ele se converte.** A pergunta «posso citar?» mora na coluna
(`Column::recusa_de_valor`), e cada camada que recusa em nome de uma coluna
passa a recusa por ela.

## 5. Como está guardado hoje

`Column::recusa_de_valor` (`phxsql-core/src/schema.rs`), chamada pelo
`json_para_valor_da_coluna`, pelo `mesclar` do upsert, pelo `linha_de_texto`
da carga, pelo `montar_payload` e pelas coerções de expressão do
`phxsql-store`. Testes em `servidor::testes_recusa_sem_dado_pessoal`; quatro
guardas no catálogo, provadas à mão.

**Onde o buraco ficou:** uma camada nova que recuse em nome de uma coluna sem
passar pela porta volta a citar — não há catraca que conte quem recusa sem a
coluna; o que existe é o teste por caminho.
