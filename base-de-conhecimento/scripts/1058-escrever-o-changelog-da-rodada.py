# Escrever o CHANGELOG da rodada
# 29/08 03:56

import io
p='CHANGELOG.md'
s=io.open(p,encoding='utf-8').read()
anc = '''- **`docs/SQL.md`: o que a camada SQL precisa saber, antes de existir.**'''
assert s.count(anc)==1
novo = '''- **A réplica passou a acompanhar o master** (pedido 111): **4.273 → 17.450
  eventos/s por réplica (4,08×)**, e as três juntas aplicam ~52.000/s contra os
  34.048 que o master escreve. O alcance de 100.000 eventos caiu de 18,7 s para
  **5,7 s**, e a latência de uma exclusão física até as três, de 1.952 para
  **140 ms**.

  **A causa registrada estava errada, e a medição a derrubou.** Estava escrito
  em dois documentos que «aplicar decodifica a imagem para `Value` e reencoda o
  payload, em vez de gravar os bytes que vieram». Medido
  (`--example onde-doi-na-replica`): `aplicar_evento` custa **16,15 µs** e uma
  inserção local pura custa **15,88 µs** — a acusação vale **0,27 µs**. E os
  4.273/s eram **229 µs por evento**, enquanto o caminho de CPU inteiro dos dois
  lados custa 20,5.

  Os 208 µs que faltavam estavam **no source**, e não na réplica:

  - **O diário era varrido desde o começo a cada lote.** Desde que o evento
    deixou de ter largura fixa, chegar ao evento N é caminhar pelos N−1
    anteriores lendo o cabeçalho de cada um. Servir «500 a partir de P» custava
    1,11 µs por evento com P=0 e **72,65 µs** com P=90.000; alcançar 100.000 em
    lotes de 500 gastava **4,07 s só ali** (`--example custo-do-desde`). Com uma
    **marca de posição**, **0,09 s — 45×**.

    A marca é uma **dica**, e não uma verdade: uma errada faz a leitura começar
    no lugar errado e o CRC do evento recusar, ou cair depois do fim e devolver
    vazio. Nenhum dos dois entrega evento errado, e é isso que a torna segura.
    Ela mora no servidor, e não na tabela, porque a tabela é aberta e fechada a
    cada pedido — e são pedidos seguidos que ela serve. **São várias por
    tabela**: um source atende réplicas em posições diferentes, e uma marca só
    seria empurrada para frente pela mais adiantada e nunca serviria às outras.

  - **O laço dormia depois de toda rodada, inclusive das produtivas.** O
    `reconectar_em` é o intervalo entre perguntas **em vão**; uma rodada que
    aplicou eventos volta na hora, porque o source continuou escrevendo enquanto
    ela aplicava. Erro continua dormindo, de propósito.

  E um terceiro, menor: **`bytes_para_hex` fazia um `format!` — e uma alocação
  de `String` — por byte** da imagem. Tabela de dígitos no lugar: 3,48 → 0,24 µs
  por evento, **14,5×**.

  3 testes novos, e o que mais importa é `a_marca_da_exatamente_os_mesmos_eventos`:
  a marca é otimização num caminho onde errar não dá erro, dá **evento errado
  aplicado como se fosse o certo**.

- **Construção em lote da B+tree** (pedido 114): `NdxFile::construir_em_lote`
  monta a árvore sem descer nenhuma vez — ordena as chaves, enche as folhas em
  sequência e monta os níveis de cima por cima. Um milhão de chaves: **7,72 s →
  0,31 s, 23× a 25×**. Todo `reindexar` e todo *reparar índice* andam nisso.

  O **enchimento das folhas — 80% — é medido, e não herdado**. 70% é a folga
  clássica e não compra nada, porque inserção aleatória já assenta perto de 69%
  de ocupação sozinha; de 90% para cima a folha fica sem folga, e crescer aloca
  milhares de páginas e fica **mais lento** do que na árvore mais frouxa.

  A construção **exige índice vazio** e recusa em vez de aproveitar árvore
  existente: aproveitar pediria devolver as páginas velhas à lista de livres uma
  a uma, e vazar página em silêncio é pior que recusar.

  **O adiamento que ela deveria destravar foi medido e ficou de fora.** O 1,59×
  vale para tabela vazia; `reindexar` refaz sobre a tabela **inteira**, então
  carregar M numa tabela de N ganha 1,22× quando M=N e **vira prejuízo abaixo de
  M≈N/3**. E cobraria marcar índice suspenso no formato, cujo defeito é busca
  respondendo errado em silêncio depois de uma queda. O que o faria valer é
  **fundir** a série ordenada na árvore existente, e não refazê-la.

- **`docs/SQL.md`: o que a camada SQL precisa saber, antes de existir.**'''
io.open(p,'w',encoding='utf-8').write(s.replace(anc,novo))
print('ok')
