# Cognição: o «resto» de um split de custo é medida, não sobra

**Descoberta:** 08/09/2026, ~12:40 UTC, ao medir o split do `.reg` heap
(pedido do dono, opção «só medição»).

## 1. O que aconteceu

O `.reg` heap era a maior parcela do inserto depois que o `.ndx` ganhou cache
e o `.log` moveu o contador para o `sincronizar`. O dono pediu o split «CRC ×
slot × syscall». Montei `fn medir_reg_split` no `onde-doi`: insere no `RegFile`
cru (que não toca o `.log`), isola CRC, montagem e os dois writes com
`black_box`, e reconcilia contra a verdade de campo (`.reg inserir` direto,
4,45 µs/linha, 200k linhas).

A primeira corrida atribuiu 43% e deixou **57% em «resto»**.

## 2. O que eu concluí primeiro, e estava errado

Que o custo do `.reg` estava nas três operações VISÍVEIS do `escrever_slot`:
o CRC, a montagem do slot, e os dois `write`. Repliquei essas três, e chamei
os 57% que sobraram de «aritmética do rowid, marcar_escrito, arredondamento» —
um balde pequeno, de nanossegundos. Estava tratando o resto como sobra: o que
não coube nas parcelas que eu já esperava encontrar.

Errado por dois motivos. Primeiro, 57% de 4,45 µs são 2,5 µs — não é
arredondamento, é a maioria do custo. Segundo, `marcar_escrito` e as buscas de
`HashMap` custam ~15 ns cada; cinco delas dão 0,08 µs, 30× menos que o resto.
O balde «pequeno» estava escondendo alguma coisa grande.

## 3. O que a medição disse

O resto não estava nas operações que eu repliquei — estava no que as
**embrulha**, e só apareceu lendo o `inserir_no_periodo` inteiro, linha a
linha, em vez das três funções óbvias:

- `garantir(volume)` chama `existe()` a CADA inserto, e `existe()` faz
  `caminho()` (um `format!` que aloca) + `Path::exists()` (um **stat** de
  filesystem). Medido: **1,28 µs, 28,8%** — só para confirmar que o volume
  existe, quando depois da 1ª linha a resposta é sempre «sim» e o descritor já
  está no cache `abertos`.
- `gravar_contadores` chama `montar_cabecalho` por linha, que aloca o buf,
  chama `agora()` (um `clock_gettime`) e faz um 2º CRC. Medido: 0,15 µs.
- e o próprio 2º write, o do cabeçalho de contadores por linha: 0,86 µs, 19,3%.

Medir essas peças — cada uma tirada do caminho real, não chutada — encolheu o
resto de **57% → 24%**. E a parcela que eu suspeitava (o CRC do corpo do slot)
deu **0,9%**: nunca foi o gargalo.

## 4. A regra

**Num split de custo, o «resto» é uma medida, não uma sobra. Resto grande é o
sinal de que só se mediu a operação visível e se perdeu o que a embrulha —
feche-o lendo o caminho de chamada INTEIRO e medindo cada peça tirada do
código, nunca chutando um balde de «arredondamento».** É o irmão de «número
citado não é número que se mede»: balde não-medido é número citado com outro
nome.

## 5. Como está guardado hoje

No `onde-doi` (`fn medir_reg_split`) e no `DESEMPENHO.md` §2.2.1, com a tabela
que se recompõe a cada corrida. O split é só medição: as duas idas ao núcleo
que se repetem por linha (o `stat` do `garantir` e o cabeçalho de contadores)
são o alvo certo, mas movê-las para «por lote» toca o caminho de escrita
sagrado e é decisão do DBA sênior — uma frente própria, não escrita ainda.
