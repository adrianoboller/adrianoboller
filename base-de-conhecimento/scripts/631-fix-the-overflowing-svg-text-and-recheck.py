# Fix the overflowing SVG text and recheck
# 28/08 18:15

import io
p='docs/dossie/dossie-phxsql.html'
s=io.open(p,encoding='utf-8').read()
trocas = [
 # fig 4: a formula comeca em x=606 e nao cabe. Encolhe e desloca para a esquerda.
 ('''          <text x="606" y="140" font-size="12.5">offset = data_offset + (234 − 1) × slot_size</text>''',
  '''          <text x="560" y="140" font-size="11">offset = data_offset + (234 − 1) × slot_size</text>'''),
 # fig 9: a frase e longa demais para uma linha so. Quebra em duas.
 ('''          <text x="16" y="288" font-size="11" opacity=".6">A trava única existe porque o motor ainda não tem travas de arquivo nem de registro. Conexões são aceitas em paralelo; as operações se enfileiram.</text>''',
  '''          <text x="16" y="288" font-size="11" opacity=".6">A trava única existe porque o motor ainda não tem travas de arquivo nem de registro.</text>
          <text x="16" y="306" font-size="11" opacity=".6">Conexões são aceitas em paralelo; as operações se enfileiram.</text>'''),
 # fig 15: passa 3px. Encolhe um ponto.
 ('''          <text x="16" y="326" font-size="11" opacity=".6">Nada entra em memória sozinho: um cache que decide sozinho o que guardar é um cache que um dia decide errado no pior momento.</text>''',
  '''          <text x="16" y="326" font-size="10.5" opacity=".6">Nada entra em memória sozinho: um cache que decide sozinho o que guardar é um cache que um dia decide errado no pior momento.</text>'''),
 # fig 16: quebra em duas.
 ('''          <text x="16" y="292" font-size="11" opacity=".6">Ed25519 escrito neste projeto, conferido contra os quatro vetores da RFC 8032 e contra a implementação de referência da própria RFC.</text>''',
  '''          <text x="16" y="286" font-size="11" opacity=".6">Ed25519 escrito neste projeto, conferido contra os quatro vetores da RFC 8032</text>
          <text x="16" y="304" font-size="11" opacity=".6">e contra a implementação de referência da própria RFC.</text>'''),
]
for velho, novo in trocas:
    assert velho in s, velho[:60]
    s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
