# Gerador que reconta o que a guarda conta diverge dela

**Estado:** FRUTÍFERO

**Evidência:** `tools/dossie/numeros.py::suite`, commit desta rodada. Sobre a mesma saída da
suíte (897 verdes), a regex antiga do gerador contava **7** pulos calados e a guarda
`o_repositorio_nao_tem_pulo_calado` media **0**. Com o leitor novo: saída verde → `[]`; saída com a
guarda `FAILED` e o achado `ide_web.rs:647` → `['crates/phxclaw-agent/tests/ide_web.rs:647']`;
saída sem o teste da guarda → `None`, publicado como NÃO MEDIDO.

## O que aconteceu

O estado «pulado» do passo do fluxo foi declarado numa constante (`// nao e pulo:`). A guarda
aceitou a declaração; o dossiê continuou dizendo «7 lugares pulam sem registro».

## O que eu concluí primeiro, e estava errado

Que bastava a guarda voltar a 0 para o dossiê acompanhar. O dossiê tinha a SUA cópia da regra
(`"[^"]*\bpulad[oa]`), mais frouxa — casava até `PULADO,` porque achava aspas de outra string
antes. E, no primeiro conserto, ler o veredito com `None` virava «guarda em 0» pelo `if` falso.

## A regra

O alcance da lei «função e comando vêm do mesmo motor» inclui os **geradores de página**: número
que uma guarda já mede se LÊ da saída dela, nunca se reconta. E ausência de veredito é NÃO
MEDIDO, não zero.

## Como está guardado hoje

`numeros.suite` lê `test o_repositorio_nao_tem_pulo_calado ... ok|FAILED`; o gerador trata
`None` como NÃO MEDIDO e diz isso sob «FEZ MENOS DO QUE O NOME PROMETE».
