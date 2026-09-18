# Virar um padrão booleano muda dois lugares que ninguém olha: o irmão que não passa pelo analisador, e o valor que ninguém reconhece

**Descoberta:** 18/09/2026, 13:45. **Pedido:** 373 (`CIFRA=1` vira o padrão da
receita do ODBC, com escape `CIFRA=0` escrito — decisão do dono).

## 1. O que aconteceu

O driver ODBC lia a cifra do fio da connection string em
`crates/phxsql-odbc/src/conexao.rs`: `CIFRA=1` ligava, a omissão falava claro.
O dono virou o padrão. A ordem parecia de uma linha — trocar o `false` por
`true` — e o contrato até nomeava o teste que cairia
(`sem_as_chaves_a_receita_e_em_claro`).

Caíram **quatro**, não um. E o conserto não coube na linha que a ordem
nomeava.

## 2. O que eu concluí primeiro, e estava errado

Concluí duas coisas, as duas plausíveis e as duas erradas:

1. **«O padrão mora no `analisar_receita`, é lá que se troca.»** O analisador é
   quem lê `CIFRA=`, então é onde o padrão parece morar. Mas o `SQLConnect`
   com `host:porta/database` (`src/lib.rs`) **não passa por ele**: monta a
   `Receita` com `..Receita::default()`. Um padrão posto só no analisador
   deixaria esse caminho falando claro, calado — e nenhum teste do analisador
   acusaria.
2. **«Só cai o teste que a ordem nomeou.»** Caíram três outros, todos em
   `src/lib.rs`, e por um motivo que não está escrito em lugar nenhum do
   assunto «cifra»: eles sobem um `servidor_de_eco` em processo que fala JSON
   **em claro**, e o driver passou a pedir aperto de mão antes do primeiro
   pedido. O assunto deles é parâmetro de instrução preparada. A virada os
   alcançou pelo transporte.

## 3. O que a medição disse

Suíte inteira, `cargo test --workspace --no-fail-fast`, com a virada crua
(só o `Default`) e nada mais:

| | passaram | falharam |
|---|---|---|
| antes (árvore com três frentes vivas) | 2552 | 22 |
| com a virada crua | 2562 | 12 |

As somas não se comparam direto — outra frente mexeu na árvore entre as duas
corridas. O que se compara é a **lista**: quatro falhas novas, e nenhuma das
22 antigas era minha.

```
conexao::testes::sem_as_chaves_a_receita_e_em_claro
testes::a_contagem_e_a_descricao_dos_parametros_pela_abi
testes::ligacao_recusa_na_hora_o_que_o_driver_nao_sabe_mandar
testes::parametros_viajam_no_pedido_e_a_falta_deles_nao_sai_do_driver
```

Um era a garantia revogada. **Três eram o servidor de mentira falando claro** —
75% do estrago fora do arquivo do assunto.

E um segundo achado, que não aparece em teste nenhum porque não havia teste:
com o padrão ligado, o `verdadeiro()` do analisador vira um **rebaixador**.
Ele respondia sim/não em dois estados, e o «não reconhecido» caía no não. Isso
era inofensivo enquanto o padrão era claro (não reconhecido = o padrão de
qualquer jeito); depois da virada, `CIFRA=zero` — dedo errado, não escolha —
**desligaria a cifra**. Virou `interruptor()` com três estados: sim, não, e
`None` que não mexe no padrão.

## 4. A regra

**Ao virar um padrão booleano, procure primeiro quem constrói a struct sem
passar pelo analisador, e depois o que o analisador faz com o valor que ele
não reconhece.** O irmão de um padrão não é quem chama as mesmas funções: é
quem escreve `..Default::default()`. E parser tolerante, sob padrão ligado,
deixa de ser tolerante e passa a ser porta de rebaixamento.

E o corolário do erro que ensina a saída: **saída só se ensina onde ela vale.**
A frase «escreva `CIFRA=0`» não sai quando há pino escrito — ali ela seria
conselho falso (o pino vence o interruptor) e, pior, mandaria baixar a guarda
exatamente no caso em que a chave apresentada não confere.

## 5. Como está guardado hoje

* O padrão mora em `impl Default for Receita` (`conexao.rs`), com o comentário
  dizendo por que não mora no analisador.
  `o_caminho_do_sqlconnect_tambem_nasce_cifrado` trava o irmão do `SQLConnect`.
* `valor_torto_na_cifra_nao_rebaixa_para_claro` trava os três estados.
* `o_escape_escrito_e_o_cifra_zero` é o teste antigo, com o assunto trocado:
  provava a garantia revogada, e hoje prova o escape. Teste que some leva a
  garantia junto.
* `o_aperto_recusado_ensina_a_saida` e
  `com_pino_o_diagnostico_nao_manda_baixar_a_guarda` sobem um servidor que
  recusa o aperto e conferem os dois lados da frase.
* Os três testes de ABI passaram a escrever `CIFRA=0` por um ajudante só
  (`receita_do_eco`), e não copiado em cada um — receita de teste espalhada é
  onde o próximo interruptor de fio envelhece um lugar e esquece os outros.
* **Onde o buraco ficou:** o catálogo de guardas (`bancada/guardas/catalogo.py`,
  papel G) não tem entrada para esta virada, e a entrada
  `receita-odbc-devolve-a-senha` diz «1 dos **59** do `phxsql-odbc --lib`»
  quando o próprio corredor de guardas mediu **66** nesta data. Os dois são de
  QA, e ficaram nomeados no relatório em vez de mexidos por esta frente.
