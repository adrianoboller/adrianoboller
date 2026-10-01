# Operação anônima que toma trava pelo campo que o portão não lê

**Estado:** PENDENTE

## O que aconteceu

Pedido 607 (revisão SEC independente, S1). O `begin` está entre as dezesseis
operações anônimas (`Atividade::da_operacao` devolve `None`). Com `scope`, ele
toma trava de tabela **na abertura** (`declarar_escopo`), e o `scope` nomeia a
tabela num campo que o portão 3 não lê. Medido pelo soquete contra o binário de
antes: só com o token, `scope:["salarios"]` + `EXCLUSIVE` + `timeout_ms:10^12`
devolveu `expira_em_s=1000000000`, e o supervisor recebeu `SP000006` no
`inserir`. O leitor de outra base passou pelo mesmo caminho, e a recusa «está no
SCOPE e não existe» enumerou o catálogo sem login.

## O que eu concluí primeiro, e estava errado

Que o inventário das anônimas (pedido 445) já cobria o caso, porque contou as
dezesseis uma a uma. Ele respondia a uma pergunta só: «cabe nos 64 KiB do
`TETO_DO_APERTO`?». A pergunta «alguma delas toma trava ou nomeia tabela?»
nunca foi feita. Um inventário completo para uma pergunta é cego para a outra.

## O que a medição disse

Antes → depois, no mesmo roteiro (`scratchpad`, `prova.py`): anônimo `ok` →
`faca login`, com a mesma frase para a tabela que existe e para a que não
existe; leitor de `Z` `SP000006` → `leitor nao tem permissao de alterar em rh.salarios`,
e a mesma frase para `rh.fantasma`; prazo de 10^12 ms → `expira_em_s=300`.

## A regra

Quando uma operação passa a **tomar trava ou nomear tabela** por um campo
novo, ela deixa de ser «anônima» por natureza: pergunte quem a pode chamar sem
login e por qual campo ela nomeia a tabela. A conferência própria vem **antes**
da conferência de existência, senão a recusa vira oráculo do catálogo.

## Como está guardado hoje

`pede_identidade` e `direitos_da_trava` (`servidor.rs`); os três testes de
`servidor::testes_escopo_do_begin_607`; as guardas `escopo-do-begin-sem-login`,
`escopo-do-begin-sem-direito` e `prazo-da-transacao-sem-teto`. O buraco que
fica: o teste `as_operacoes_anonimas_sao_estas_dezesseis` ainda conta só os
**nomes**; uma anônima nova que tome trava por um campo do pedido passaria por
ele sem acusar.
