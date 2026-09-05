# A volta não é o espelho da ida: o irmão que ficou de um em um

*Descoberto em 05/09/2026, 03:55, montando a bancada de utilização padrão.*

## 1. O que aconteceu

Abri a frente para dois pedidos novos — a carga de 20.000 registros em tabela
complexa e a paginação alfabética — e o segundo pedia provar **pela porta de
dados** o que `crates/phxsql-store/tests/alfanumerica.rs` já prova por dentro
com dezesseis testes.

A terceira sonda que escrevi criou uma tabela com `particao: "letra"` pelo
protocolo, gravou treze nomes espalhados pelo alfabeto e pediu páginas de três
com `pular`/`max`. As duas primeiras páginas vieram. A terceira não:

```
pular=5 ERRO {"ok": false, "op": "varrer",
  "erro": "[SP000018] nao encontrado: rowid 4000 nao existe em clientes"}
```

`4000` é o **último slot do balde D**, que nunca recebeu linha nenhuma. A
página que eu pedi começava em `Éder`, rowid 4001 — o primeiro slot do balde E.

O mecanismo: `varrer` monta o campo `ha_antes` chamando
`Table::pagina_antes_de(primeiro, 1, visao)`, e essa função andava de um em um
para trás com `self.reg.ler(rowid)?`. Na partição alfanumérica `conferir_faixa`
recusa o slot além do `usados` do balde — ele **não existe** —, e a recusa é
`NaoEncontrado`, não `Ok(None)`. O `?` propaga, e a varredura inteira é
reprovada.

Alcance: **toda página cujo primeira linha seja a primeira do balde dela**. Com
treze linhas em nove baldes, 4 das 5 páginas. E a página 1 também, sempre que o
balde `_A` estiver vazio — aí o primeiro rowid da tabela deixa de ser 1 e o
`ha_antes` passa a perguntar por um slot que não existe.

## 2. O que eu concluí primeiro, e estava errado

Escrevi na primeira anotação: *«o `pular` na alfanumérica anda de um em um pelo
vazio e por isso é lento; falta o salto que o `proximo_ativo` tem»*. Estava
errado em duas metades.

A primeira: **não era o `pular`**. A página vinha certa — a lista de linhas
estava montada e correta quando o erro apareceu. Quem quebrava era o campo
`ha_antes`, calculado **depois** da página, para a tela poder desenhar o botão
de voltar. Eu tinha atribuído o defeito ao caminho que estava no meu pedido, e
não ao que estava no código.

A segunda, e a que importa: **não era lentidão, era recusa**. Eu estava
preparado para achar um caminho caro — «36 milhões de leituras» é o que o
comentário do `proximo_ativo_por_balde` adverte, e foi o que eu esperava
reencontrar. Um caminho caro se mede e se decide se vale consertar. O que
estava lá não se mede: o servidor devolvia `NAO_ENCONTRADO` e a grade não
abria.

O diagnóstico errado sobreviveria bem, porque o conserto certo — dar à volta o
mesmo salto por balde da ida — **conserta os dois**. Se eu não tivesse voltado
ao erro literal, teria escrito «acelerei a página anterior» num documento em
que a verdade é «a página anterior não funcionava».

## 3. O que a medição disse

| | antes do conserto | depois |
|---|---|---|
| `varrer pular=5 max=3` numa tabela por letra | `[SP000018] rowid 4000 nao existe` | as três linhas |
| páginas de 3 que começam no primeiro slot de um balde | 4 de 5 reprovavam | 5 de 5 respondem |
| `pagina_antes_de` com 1.000.000 de slots por balde | erro no primeiro slot vazio | as duas linhas, atravessando 25 milhões de slots |
| testes de `alfanumerica.rs` que caem com o defeito reposto | — | **1 de 16** (`a_pagina_anterior_atravessa_o_vazio_entre_baldes`) |
| afirmações da bancada por soquete | — | **18, zero sem confirmar** |

A prova real fecha nos dois sentidos: com o laço antigo reposto o teste novo
cai e os quinze antigos passam; com o conserto, os dezesseis passam. É a guarda
`pagina-anterior-de-um-em-um` do `bancada/guardas/catalogo.py`.

E a razão de nenhum dos dezesseis testes ter pego isso está na lista de coisas
que o cabeçalho do arquivo dizia proteger: *«a varredura salta os vazios entre
baldes»* — e o teste que a prova chama `varrer()` e `pagina_depois_de()`. A
**ida**. Ninguém tinha escrito a frase no plural.

## 4. A regra

**Quando um caminho ganha um salto, procure o caminho que anda no sentido
contrário — e note que o irmão da ida não é quem tem nome parecido, é quem
percorre a mesma estrutura ao contrário.** E o teste da ida não prova a volta,
por mais que a frase do cabeçalho pareça cobrir as duas.

O corolário, que é o que a porta de dados ensinou: **o erro que um caminho
interno devolve muda de gravidade quando ele sobe pelo protocolo.** `ler` de um
slot que não existe é uma pergunta legítima com resposta legítima para quem
chama por dentro; a mesma resposta, propagada por um `?` dentro do cálculo de
um campo *acessório* da resposta, derruba a operação inteira. Campo acessório
não deve poder reprovar a resposta principal.

## 5. Como está guardado hoje

- `RegFile::anterior_ativo` e `anterior_ativo_por_balde`, em
  `crates/phxsql-store/src/reg.rs` — o espelho do `proximo_ativo`, com o mesmo
  salto por balde e o mesmo corte no `usados`.
- `Table::pagina_antes_de` passou a andar por ele, em
  `crates/phxsql-store/src/table.rs`. Fora da alfanumérica ele percorre
  exatamente os mesmos slots de antes.
- `crates/phxsql-store/tests/alfanumerica.rs`:
  `a_pagina_anterior_atravessa_o_vazio_entre_baldes`, com os dois casos — o
  balde do meio e o `_A` vazio.
- `bancada/guardas/catalogo.py`: `pagina-anterior-de-um-em-um`.
- `bancada/utilizacao-padrao/paginacao-alfabetica.py`: as afirmações
  `a_pagina_anterior_atravessa_os_mesmos_vazios`, `a_pagina_2_atravessa_balde`
  e `ha_antes_responde_em_toda_pagina`, cada uma com o controle da mesma
  corrida.

**Onde o buraco ficou:** o conferidor genérico para isto está **recusado, e sem
número** — não medi quantos pares ida/volta existem no `phxsql-store`. O que
dá para dizer é que a família vizinha estava certa: `rowid_do_rownum` **já**
sabia que não pode bissetar na alfanumérica e varre, com o comentário
explicando por quê. Um casador de padrão que procurasse «laço com `-= 1`»
acharia esse também, e o reprovaria sem defeito nenhum. O que distingue o
defeito é o `ler` cru dentro do laço, e isso se acha procurando o irmão.
