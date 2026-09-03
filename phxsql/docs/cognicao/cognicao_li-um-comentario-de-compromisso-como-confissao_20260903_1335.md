# Li um comentário de compromisso como se fosse confissão — e passei por cima da decisão

**Descoberto em 03/09/2026, 13:35**, consertando o pedido 176.

## 1. O que aconteceu

A conferência de chave estrangeira recusa quando a mãe foi escrita e ainda não
sincronizou, e a recusa está certa. O texto era o problema: ele embrulhava o
erro cru do índice — *«ficou para trás numa queda… reconstrua com `reparar
indice`»* — e mandava junto com a explicação correta. **Primeira metade
mandando reparar um arquivo intacto; segunda metade dizendo a verdade.**

Acima da linha havia este comentário:

> *«O erro cru manda "reconstrua o indice", o que faria o leitor reparar um
> arquivo sao. Este diz o que houve.»*

Eu li isso como **confissão de conserto pela metade** — alguém viu o problema,
escreveu que viu, e deixou o `({e})` lá. Tirei o `({e})`.

## 2. O que eu concluí primeiro, e estava errado

**Não era confissão. Era compromisso, e estava argumentado e testado.**

O teste `a_mae_nao_gravada_recusa_dizendo_por_que` caiu no meu conserto, e o
comentário dele dizia por quê:

> *«A causa crua CONTINUA na mensagem, e de propósito: é ela que diz qual
> arquivo e qual guarda recusou. […] o teste afirma que a explicação vem DEPOIS
> da causa, e não que a causa sumiu — **jogar a causa fora trocaria um recado
> ruim por um recado cego**.»*

Ou seja: a decisão anterior **não** era «não mexi nisso». Era «a causa fica, e a
explicação vai por último para ser a última palavra». Eu apaguei metade de uma
decisão sem ter lido a metade que a defendia — e o que me impediu de fazer isso
não foi cuidado meu, foi o teste que alguém escreveu com o argumento dentro.

E as duas leituras erradas têm a mesma raiz: **eu tratei o comentário como
narração do estado do código, quando ele era a ata de uma decisão.** Comentário
que explica *por que* se parece muito com comentário que confessa *o que falta*,
e a diferença mora no teste ao lado, não no comentário.

## 3. O que a medição disse

O que a decisão anterior não previu, e que é o defeito real: as duas metades
**se contradizem**. Ela otimizou a *ordem* (explicação por último) e não notou
que a primeira metade dá uma **ordem ao operador** — reparar — que a segunda
desmente. Ordenar não desfaz contradição.

A terceira via satisfaz as duas: a causa continua nomeada, mas **montada do
dado** (`diretorio()` + `nome()` + `EXT_NDX`), não recortada do texto do erro —
porque recortar quebra calado no dia em que alguém melhorar a redação, que é a
mesma lei do «texto se resolve por chave, nunca por comparação da frase». Sai
só o imperativo, e entra a afirmação de que o arquivo está são.

Medido, com o defeito reposto (`pendente` forçado a `None`): **2 de 2 testes
caem** — o novo e o antigo, que passou a guardar o comportamento novo também.
Com o conserto, 18/18 no arquivo e **1.545** na suíte. Controles: a mãe já
gravada continua sendo vista, então a troca provou a guarda em vez de quebrar o
arquivo.

## 4. A regra

**Antes de apagar o que um comentário justifica, leia o teste que ele cita** — e
quando o comentário argumenta contra uma alternativa, a alternativa já foi
considerada e recusada. Trocar uma decisão exige derrubar o argumento dela, não
ignorá-lo; e o modo honesto de derrubar é achar o que ela **não previu**, que
aqui foi a contradição entre as duas metades.

## 5. Como está guardado hoje

Guarda `recado-manda-reparar-arquivo-sao` no catálogo, **provada** (2/2 caem,
2 controles seguem). Os dois testes vivem em
`crates/phxsql-store/tests/chave-estrangeira.rs`, e o antigo teve o comentário
**remedido em vez de reescrito**: a intenção dele sobreviveu inteira, só a
agulha da causa deixou de ser a palavra «reconstrua» e passou a ser o caminho
do `.ndx`.

O que **não** está guardado: nada distingue, hoje, um comentário-ata de um
comentário-confissão. A única defesa que funcionou foi um teste com o argumento
escrito dentro dele — e isso é prática, não mecanismo. Fica como buraco.
