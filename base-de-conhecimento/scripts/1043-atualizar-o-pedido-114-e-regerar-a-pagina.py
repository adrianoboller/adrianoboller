# Atualizar o pedido 114 e regerar a pagina
# 29/08 03:34

import io
p='docs/PENDENCIAS.md'
s=io.open(p,encoding='utf-8').read()
import re
m=[l for l in s.split('\n') if l.startswith('| ☐ | 114 |')]
assert len(m)==1
nova = ('| ☑️ | 114 | **Índice não único fora do caminho crítico** | **a peça que faltava está '
        'feita, e o item em si foi medido e recusado.** `construir_em_lote` monta a B+tree sem '
        'descer nenhuma vez — 7,72 s → **0,31 s** num milhão de chaves (**23× a 25×**), e todo '
        '`reindexar` e todo reparo de índice andam nisso. O enchimento de folha, 80%, é medido e '
        'não herdado. Já o **adiar** em si: o 1,59× vale para tabela vazia, mas `reindexar` refaz '
        'sobre a tabela **inteira** — carregando M numa tabela de N, o ganho é 1,22× quando M=N e '
        'vira **prejuízo abaixo de M≈N/3** (`--example adiar-vale-quando`). E cobraria marcar '
        '**índice suspenso no formato**, cujo defeito é busca respondendo errado em silêncio '
        'depois de uma queda. Fica fora com o número na mesa; o que o faria valer é **fundir** a '
        'série ordenada na árvore existente, e não refazê-la |')
s=s.replace(m[0], nova)
s=s.replace('**111 feitos · 8 parciais · 10 planejados**, de 129 pedidos.',
            '**112 feitos · 8 parciais · 9 planejados**, de 129 pedidos.')
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
