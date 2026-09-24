# O motor único muda de arquivo a linha que a guarda aponta — e pode nascer com um segundo lugar que nenhuma guarda cobre

**Descoberto em 24/09/2026, ~06:00**, na etapa 2 do pedido 450 (o
`config.json` gravado como `config.phz`), quando o integrador rodou
`python3 bancada/catracas/todas.py` na árvore viva no meio da frente.

## 1. O que aconteceu

A pétrea de 23/09 manda que função e comando venham do mesmo motor. A frente a
cumpriu: os seis leitores do arquivo de configuração e o único escritor
(`gravar_a_arvore`, no `config.rs`) passaram a chamar
`config_phz::ler_texto` e `config_phz::gravar_texto`, que decidem a forma
(claro ou `.phz`) num lugar só.

A proteção de permissão continuou intacta — `gravar_texto` chama o mesmo
`config::gravar_privado`, `0600` desde o primeiro byte. Os portões passaram.
E a `trecho-vivo` reprovou: `TETO_TRECHO_MORTO` foi de **0 a 1**. A guarda
`config-json-escreve-aberto-e-herda` (achado A4 da SEC, 17/09) apontava para o
TEXTO da chamada dentro do `gravar_a_arvore`, e o texto tinha ido para outro
arquivo.

## 2. O que eu concluí primeiro, e estava errado

Que a proteção estar intacta bastava. Eu tinha conferido o que importava ao
produto — a função chamada era a mesma, e a permissão, medida pelo `stat`,
continuava `600`. Não procurei a linha no `catalogo.py` porque não MUDEI a
proteção: só a mudei de lugar.

Mas a guarda não aponta para a função; aponta para a linha. E «mudar de lugar»
é exatamente o que a lei do motor único produz, toda vez: não duplica a linha
(o caso da cognição de 05/09, `porta-nova-quebra-a-guarda-da-porta-velha`),
**muda-a de arquivo**. A guarda morre sem reprovar teste nenhum.

E a segunda metade só apareceu ao re-apontar: o motor novo tinha nascido com um
**segundo** lugar onde o arquivo de configuração é gravado — a troca de forma
do `--empacotar-config`/`--desempacotar-config` (`trocar`), que antes não
existia. Re-apontar a guarda para `gravar_texto` e parar ali deixaria esse
segundo nascimento sem guarda nenhuma, e ele é justamente o de uma instalação
em `0644` migrando: o `.phz` herdaria o `0644`, e o parecer SEC de 24/09 diz
que a permissão é a ÚNICA autenticidade que o `.phz` tem.

## 3. O que a medição disse

- `todas.py` na árvore viva: `TETO_TRECHO_MORTO` 1 (teto 0).
- Quem mais grava o arquivo de configuração, procurado pela pergunta «quem
  chama `gravar_privado` com o caminho do config?»: **2** lugares no motor
  novo (`gravar_texto` e `trocar`), contra **1** antes.
- Pelo `stat`, com o binário e `umask 022`: `644` → `--empacotar-config` →
  `.phz` em `600`; `.phz` aberto à mão para `644` → `config_gravar` → `600`.
- Catálogo: a guarda velha re-apontada (agora com 2 testes que caem, o do claro
  e o do `.phz`) e a irmã nova da troca; mais cinco guardas do que a frente
  decidiu (tetos de leitura, os dois presentes, a forma nos dois sentidos).
  Todas provadas pelo `provar-guardas.py` — ver `SEGURANCA.md` §23.

## 4. A regra

**Antes de levar uma linha para um motor único, procure-a no `catalogo.py` —
e depois de levar, pergunte quantos lugares o motor novo tem de fazer o que a
linha fazia.** O primeiro achado re-aponta a guarda; o segundo é o que cria a
irmã.

## 5. Como está guardado hoje

Pela `trecho-vivo` (`TETO_TRECHO_MORTO`), que o `todas.py` roda — é ela que
acusou, e acusou na integração, não no commit, porque só roda quando alguém a
chama. O segundo lugar **não** tem régua: nenhuma catraca sabe que uma função
nova grava o mesmo arquivo que uma guarda protege. Achou-se pela pergunta da
seção 4, e é esse o buraco que fica.
