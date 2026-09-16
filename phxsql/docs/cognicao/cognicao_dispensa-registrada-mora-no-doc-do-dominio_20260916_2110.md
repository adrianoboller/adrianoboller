# A dispensa registrada mora no documento do DOMÍNIO, e a frente seguinte não olha lá

## 1. O que aconteceu

Frente 245, itens O2–O6, 16/09/2026. O item O4 dizia «parâmetro `1e21`
recusado como Texto». Puxando o fio, medi que o irmão pelo caminho do **número
cru** não recusava nada: `{"v":1e21}` numa coluna `Int8` gravava
**9223372036854775807**, e `{"v":9007199254740993}` gravava
**9007199254740992** — aceitos, calados, com outro número no `.reg`. A
`Sequence` ao lado recusa os dois desde o bloco 19, e o comentário **dela**
nomeia o irmão: *«o irmão `Int8`/`UInt8` passava por aqui ao lado sem
problema»*.

Escrevi o conserto inteiro: recusar a faixa (o valor que não cabe) **e** a
faixa imprecisa (≥ 2⁵³). Seis testes verdes, quatro vermelhos com o defeito
reposto, portões limpos. Só fui descobrir o problema **duas horas depois**, ao
abrir o `docs/AUTONUMBER.md` para documentar — na §C.4, «O que a G2 recusou
fazer sozinha (é papel C / próximo passo)»:

> **Alargar a recusa de 2⁵³ ao `Int8`/`UInt8`** — muda o comportamento de
> cliente que já manda número grande; decisão de projeto.

Uma frente anterior tinha **medido, decidido e registrado** que aquela metade
exata não era dela. Eu ia revogá-la em silêncio, com teste e tudo.

## 2. O que eu concluí primeiro, e estava errado

Duas vezes, e a segunda é a que ensina.

**Primeiro:** que o comentário da `Sequence` nomeando o irmão era um convite —
«alguém viu e não fez, então está de pé». Não era. Nomear o irmão é o **começo**
da investigação, não a autorização dela: o comentário diz que o irmão existe, e
não o que já se decidiu sobre ele.

**Segundo, e pior:** que «procurar a decisão anterior» é procurar no
`CLAUDE.md`, no `PENDENCIAS.md` e no `git log`. Procurei nos três, e **nos três
não havia nada** — o `PENDENCIAS.md` não tem pedido para isso, o `CLAUDE.md`
não desce a esse nível, e o `git log` conta o que a G2 **fez**, não o que ela
**recusou fazer**. A dispensa estava no lugar certo pela lei da casa (o
documento do domínio, junto do número que a justifica) e no lugar errado para
quem chega pelo código.

O que me salvou não foi método: foi ter ido escrever a documentação **antes** de
fechar a frente, e o parágrafo estar na mesma página que eu ia editar.

## 3. O que a medição disse

- Defeito medido, 16/09/2026, pelo protocolo: `Int8` ← `1e21` grava
  `9223372036854775807`; `UInt8` idem; `Int8` ← `9007199254740993` grava
  `9007199254740992`. A `Sequence` recusa os três.
- Depois de desfazer a metade adiada: **6 testes verdes**; com o defeito
  reposto, **3 vermelhos e 3 controles verdes** — e o controle que importa é
  `a_faixa_imprecisa_continua_passando_por_decisao_registrada`, que **afirma a
  dispensa** e cai no dia em que papel C a tomar.
- A metade que ficou **não** é a adiada, e a diferença é medível: fora da faixa
  do tipo, `1e21`, `1e30` e `1e300` viram **o mesmo** número — o `as i64` do
  Rust satura. `Int1`, `Int2` e `Int4` já recusavam exatamente isso no
  `escrever_inline` («9999 nao cabe em inteiro de 8 bits»); o `Int8` era o
  único sem a recusa porque o carregador **é** o `i64`.
- Terceiro buraco na mesma função, e ninguém o tinha nomeado: a **metade de
  cima do `UInt8`** (de `i64::MAX+1` a `u64::MAX`) era inalcançável pelo
  protocolo — o `parse` era para `i64` numa coluna de 64 bits **sem** sinal.

## 4. A regra

**Antes de alargar uma recusa, procure a dispensa no documento do DOMÍNIO — e
quando a achar, deixe um teste que a AFIRMA em vez de uma linha que a repete.**
Dispensa que só existe em prosa é dispensa que a próxima frente revoga sem
saber; dispensa com guarda é dispensa que aparece no dia em que alguém tenta.

E o corolário do alcance: `CLAUDE.md`, `PENDENCIAS.md` e `git log` **não
guardam o que uma frente recusou fazer**. O primeiro é lei, o segundo é
trabalho pendente, o terceiro é trabalho feito. A recusa medida mora na §
«o que esta frente não fez» do documento da área — e é por isso que escrever a
documentação **antes** de fechar a frente não é burocracia.

## 5. Como está guardado hoje

`valores.rs::testes_teto_de_64_bits::a_faixa_imprecisa_continua_passando_por_decisao_registrada`
prova que a faixa 2⁵³–teto continua passando, diz por quê no comentário e
aponta para `docs/AUTONUMBER.md` §C.4. O §C.4 ganhou, no mesmo passo, a linha
que diz que a 245 **chegou a alargá-la e desfez**, e que a decisão continua
aberta — com o nome do teste que cai quando ela for tomada.

**O buraco que fica, nomeado:** as outras recusas registradas em documento de
domínio **não** têm guarda nenhuma. Não varri quantas são — e essa varredura
(«toda seção “o que esta frente recusou fazer” tem um teste que a afirma?»)
não existe como régua. Enquanto não existir, esta cognição é a única coisa que
diz que o problema é geral e não foi um azar do `Int8`.
