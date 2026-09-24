# A conferência que relê pelo resolvedor confere o arquivo irmão

## 1. O que aconteceu

A revisão completa da frente do PhxZip achou isto no `converter_configs` de
volta (`--deszipar-config`). O `.json` era gravado, a releitura de conferência
chamava `phz::ler_texto(json)`, e o `.phz` era apagado. Só que `ler_texto`
resolve o nome: com os dois no disco, **vale o `.phz`**. A conferência comparava
o `.phz` com ele mesmo, passava sempre, e apagava a única cópia boa.

## 2. O que eu concluí primeiro, e estava errado

Que «reler antes de apagar» bastava, porque a regra estava escrita e
cumprida. Estava cumprida pela letra: relia. Mas relia pelo caminho que
escolhe o arquivo, e o arquivo escolhido era justamente o que se queria
comparar com o outro.

## 3. O que a medição disse

Ler o código mostrou o laço fechado: `resolver` dá o `.phz` quando os dois
existem, e essa regra foi escrita no mesmo pedido para proteger o arranque.
A mesma regra que protege o arranque cega a conferência da volta. Não há
teste que reproduza escrita ruim sem injetar falha no disco, então o conserto
entrou sem teste vermelho, e isso fica dito.

## 4. A regra

A conferência de uma cópia lê a cópia pelo caminho LITERAL, nunca pelo
resolvedor que escolhe entre ela e o original. Quem confere não pode usar a
função que decide qual dos dois vale.

## 5. Como está guardado hoje

`config.rs`, `converter_configs`: na volta, a conferência usa
`std::fs::read_to_string(json)`. Não há guarda automática; a regra está aqui e
no `PHXZIP.md` §5b.
