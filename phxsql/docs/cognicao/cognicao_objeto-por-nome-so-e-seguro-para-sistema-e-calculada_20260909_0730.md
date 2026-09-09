# Cognição: trocar `valores` de Array para Object por nome só é seguro para as colunas que o MOTOR já protege sozinho

## 1. O que aconteceu

O achado 1 da revisão de tela (G5-TELA, `docs/cognicao/`, relatório em
`RELATORIO.md`) pedia que a ficha (`abrirFicha`, `crates/phxsql-server/ui/index.html`)
parasse de mandar a coluna negada por direito de coluna (nem como `null`) no
pedido `atualizar`/`inserir`. A forma antiga mandava `valores` como um
`Array` posicional com uma entrada por coluna visível — a linha INTEIRA,
sempre, porque `json_para_linha` (`crates/phxsql-server/src/valores.rs`) só
sabe montar a linha completa a partir de um array: posição ausente vira
`NULL` incondicionalmente.

A troca óbvia era mandar um `Object` por NOME em vez do array, omitindo só a
coluna negada. E aqui apareceu a pergunta que decidiu a arquitetura do
conserto inteiro: **o que acontece com as colunas que a ficha OMITE do
objeto — a de sistema, a calculada — se elas não têm uma regra de coluna que
as proteja?**

## 2. O que eu concluí primeiro, e estava errado

Concluí, antes de ler o motor, que "objeto por nome com ausente = não
mexeu" era uma propriedade GERAL do protocolo — que bastava não mandar uma
coluna para ela ficar como estava. Sob essa leitura, excluir `rownum` e
`softdeleted` (colunas de sistema, já fora do array de sempre) do objeto
seria trivialmente seguro, e a única coisa a decidir seria SE a coluna
calculada e a coluna sem-alterar também deviam sair.

Isso é falso como regra geral. `json_para_linha` (Objeto) faz o MESMO que
`json_para_linha` (Lista): monta a linha inteira, e chave ausente vira
`Value::Null` — **sem exceção**, para qualquer coluna que não tenha uma
regra de coluna do usuário cobrindo-a. Se a ficha omitisse, por exemplo, uma
coluna comum sem regra nenhuma (`cidade`, `nome`), o `atualizar` gravaria
`NULL` nela — perda de dado silenciosa, o mesmo estrago que o achado 1
queria consertar, só que por um caminho novo.

## 3. O que a medição (leitura do fonte, com teste depois) disse

A pergunta certa não é "o protocolo protege ausência?" — é "quem protege
CADA coluna que a ficha vai omitir, e como?". Três respostas, três
mecanismos diferentes, nenhum deles genérico:

- **Coluna com regra de coluna negada** (`colunas_sem_alteracao`,
  `colunas_sem_leitura`): protegida em `servidor.rs`,
  `escrita_sob_direito_por_coluna` — quando a coluna está AUSENTE do
  pedido e o usuário não pode alterá-la, a função repõe o valor GRAVADO
  (lido do disco) antes de `json_para_linha` rodar. Ausência aqui é
  "ninguém mexeu", e o servidor completa por conta.
- **Coluna calculada**: protegida em `phxsql-store/src/table.rs`,
  `aplicar_regras` — recalcula a coluna a cada `atualizar`/`inserir`
  incondicionalmente, sobrescrevendo QUALQUER valor que tenha chegado
  (`null` incluído). Não importa o que a ficha mande ou deixe de mandar.
- **Coluna de sistema `rownum`**: protegida em `table.rs`,
  `numerar_linha` — no `atualizar` (quando há linha anterior), sempre
  copia o `rownum` da linha antiga para a nova, e ignora o que chegou no
  pedido. `softdeleted` é protegida numa terceira função, específica dela,
  em `servidor.rs::op_atualizar` (lê a linha atual de novo e repõe o
  índice, quando a chave não veio no objeto).

Nenhuma coluna COMUM (sem regra de coluna, não calculada, não de sistema)
tem proteção nenhuma contra ausência — e é por isso que a ficha continua
mandando TODAS elas no objeto, com o valor lido (ou editado) do formulário.
Confirmado depois por teste de ponta a ponta
(`testes-web/casos/27-direito-por-coluna.mjs`): salvar como `vendedor`
mexendo só em `cidade` grava `cidade` certa e preserva `limite_credito` e
`resumo`, nos dois sentidos (campo presente vs. ausente).

## 4. A regra

**Omitir uma coluna do `valores`/`linha` por nome só é seguro quando existe
um mecanismo NOMEADO — regra de coluna, coluna calculada, ou coluna de
sistema com a sua própria função — que reponha o valor por conta própria.
Para qualquer outra coluna, ausência vira `NULL` sem aviso, e "usar objeto
por nome" não é, sozinho, uma proteção contra perda de dado.**

## 5. Como está guardado hoje

O comentário em `abrirFicha` (`valores()`, `crates/phxsql-server/ui/index.html`)
nomeia os dois motivos de exclusão (`semAlterar`, `calculada`) e por que
cada um é seguro; `docs/SEGURANCA.md` §15.13 registra o desenho do lado do
front. O que NÃO está escrito em lugar nenhum, e é o alcance que este
arquivo existe para fechar: se algum dia a ficha precisar omitir uma coluna
por um motivo NOVO (por exemplo, um campo grande que não se quer reenviar a
cada salvar), quem escrever esse código precisa achar — ou criar — o
mecanismo de reposição do lado do motor primeiro. Sem isso, "não mandei
porque não mudou" vira "não mandei e virou nulo" na primeira coluna comum
que alguém tentar poupar.
