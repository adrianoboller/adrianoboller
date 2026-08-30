# Write the 0.15.0 changelog and bump the version
# 28/08 20:02

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
antigo = """---

## 0.14.0 — 2026-08-28"""
novo = """---

## 0.15.0 — 2026-08-28

**Carga em lote** e o **salto para a página 500** — que a versão anterior tinha
deixado escrito como o que faltava.

### Corrigido

- **A bissecção pelo `rownum` estava errada na partição alfanumérica, e errada
  em silêncio.** Ali o `rownum` não cresce com o rowid: a Silva digitada
  primeiro mora no `_S`, com rowid alto, e a Alves digitada depois mora no
  `_A`, com rowid 1 — número de ordem 1 num rowid maior que o do número 2.
  Bissetar uma sequência que não está ordenada devolve a linha errada sem
  reclamar. Nesse modo o motor agora varre, procurando o **menor** número de
  ordem maior ou igual ao alvo. Teste novo em `tests/alfanumerica.rs` prova
  que os rowids saem fora de ordem — e falha se um dia saírem crescentes, para
  não continuar provando outra coisa.

- **`phxsql listar` lia a tabela inteira para mostrar vinte linhas.** Numa
  tabela de 200.000 com memo, 382 ms para uma tela que cabe no terminal.
  Agora o teto entra na leitura, e o comando ganhou `--pular`.

- **Duas sobras da versão anterior**: um comentário duplicado no caminho da
  importação e um doc-comment órfão de função que mudou de arquivo. Zero
  avisos do clippy de novo.

### Adicionado

- **`inserir_lote`: várias linhas num pedido só.** Medido com 20.000 linhas
  pela rede, contra o mesmo trabalho linha a linha: **2.715 → 25.985 linhas/s
  (9,6×)**. O ganho não é do disco — cada linha custa o mesmo lá dentro — e sim
  de tudo que acontecia POR LINHA e passa a acontecer uma vez: abrir os sete
  arquivos, tomar a trava, o `fsync`.

- **Colar em vez de montar.** O mesmo pedido aceita texto em **JSON, CSV, TXT,
  XML e HTML**, e adivinha o formato pelo conteúdo. A primeira linha manda: as
  colunas casam pelo **nome**, não pela posição. `importar_conferir` lê e
  mostra o que entendeu sem gravar nada — é o que a tela de Importar usa, e o
  botão de gravar só acende depois que a conferência passa. Na linha de
  comando, `phxsql importar`.

- **`pular` deixou de andar até a posição.** Quando a posição de uma linha na
  lista *é* o `rownum` dela, o início da página sai de uma bissecção. Medido
  numa tabela de 200.000 linhas, pelo protocolo, pedindo 200 linhas:

  | `pular` | bissecção | passo |
  |---:|---:|---:|
  | 200 | 7 ms | 6 ms |
  | 20.000 | 7 ms | 18 ms |
  | 100.000 | 6 ms | 72 ms |
  | 199.800 | 6 ms | **131 ms** |

  A bissecção é **plana** — e os 6 ms dela são decodificar e serializar as 200
  linhas, não achar o começo. Dentro do motor, sem a rede e sem a serialização:
  **180 µs contra 55 ms** no meio de uma tabela de 200.000, e **164 µs contra
  246 ms** numa de 800.000. Os dois caminhos devolvem a mesma página — o
  exemplo `custo-da-pagina` afirma isso e falha se deixar de ser verdade.

- **`salto` na resposta do `varrer`**: `"bisseccao"` ou `"passo"`. A diferença
  entre os dois é de ordem de grandeza, e quem monta uma tela grande precisa
  saber qual está pagando — e o que fazer com a tabela para pagar o outro.

- **`visiveis` voltou a existir na resposta, e agora é barato.** Sai de dois
  contadores do cabeçalho: `registros − marcadas` são as ativas, `marcadas` são
  as excluídas. Era por essa conta não existir que o `total` tinha saído na
  0.14.0. Com ela, «página 3 de 40» voltou para a grade sem custar varredura.

- **Caixa «ir para a página» na grade**, com o botão `fim ⏭` ao lado. Salto
  para a página 500 de uma tabela de 200.000: **116 ms** medidos no navegador,
  incluindo o desenho da tela. O número da página sobrevive a navegar por
  cursor: `anterior` desconta um, `próxima` soma um.

- **`desde_rownum` no `varrer`**: a página que começa no número de ordem N,
  inclusive. É o cursor de quem guardou o número de ordem em vez do rowid.
  `rownum_inicio` e `rownum_fim` vêm na resposta.

- **`--pular` no `phxsql listar`**, e o rodapé diz por onde a página foi
  achada e qual o `--pular` da próxima.

### Mudado

- **`.reg` v3 → v4**: o contador `marcadas` nos bytes 108..116 do volume 1.
  Arquivo da v3 não abre — e não abrir é o ponto: ele traria zero ali, zero
  quer dizer «nenhuma linha marcada», e o motor concluiria que a posição é o
  `rownum` numa tabela onde não é. A página sairia errada em silêncio.

- **O contador de marcadas vai ao disco na mesma operação que o muda**, e não
  no `sincronizar` — 128 bytes a mais por exclusão suave. Um contador que só é
  gravado depois volta atrás numa queda, e este não é número de vitrine: é ele
  que decide se o salto pode confiar no `rownum`.

- **`verificar` reconta as marcadas varrendo** em vez de acreditar no
  cabeçalho, e corrige de passagem. `Relatorio` ganhou o campo. É o mesmo
  caminho que o reparo chama.

### Sabido

- **Não há transação, e o lote não muda isso.** Se a linha 700 de mil falhar,
  as 699 anteriores ficam gravadas: o `.reg` não reaproveita slot, então
  desfazer deixaria 699 buracos. Por isso o padrão é parar na primeira
  recusada; quem importa dado sujo de propósito passa `parar_no_erro: false` e
  recebe a lista do que ficou de fora, com o número da linha.

- **`1.500` continua ambíguo.** Mil e quinhentos ou um e meio? O motor
  converte `1.500,50` e `1,500.50` — o último separador é o decimal — e deixa
  `1.500` como está, em vez de escolher por conta própria.

- **Com buraco, o salto volta a andar.** Uma única linha excluída — de vez ou
  marcada — derruba a igualdade entre posição e `rownum` na tabela inteira, e
  o `pular` volta aos 131 ms. É correto: a posição realmente mudou. Mas é uma
  degradação em degrau, e não gradual: quem paginava a 6 ms passa a 131 com
  uma exclusão. Um índice de posição resolveria, ao preço de mantê-lo.

- **Por índice o salto continua sendo posição pura.** A ordem da chave não tem
  relação com a ordem de chegada, então não há `rownum` a bissetar ali.

---

## 0.14.0 — 2026-08-28"""
assert antigo in s
s = s.replace(antigo, novo, 1)

# a linha do Sabido da 0.14.0 que deixou de valer
antigo = """- **Não há salto para «a página 500».** O cursor sabe ir e voltar uma página;
  ir direto para a milésima exigiria contar, que é justamente o que foi
  removido. Quem precisa de um ponto específico usa `rownum` com a bissecção."""
novo = """- **Não há salto para «a página 500».** O cursor sabe ir e voltar uma página;
  ir direto para a milésima exigiria contar, que é justamente o que foi
  removido. Quem precisa de um ponto específico usa `rownum` com a bissecção.
  *(Resolvido na 0.15.0: o `pular` passou a bissetar, e a contagem voltou a
  partir do cabeçalho.)*"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
