# Nudge the last two and recheck
# 28/08 18:16

import io
p='docs/dossie/dossie-phxsql.html'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''          <text x="560" y="140" font-size="11">offset = data_offset + (234 − 1) × slot_size</text>''',
            '''          <text x="520" y="140" font-size="11">offset = data_offset + (234 − 1) × slot_size</text>''',1)
# a segunda linha do Ed25519 nao cabe na altura: sobe as duas e cresce o viewBox
s=s.replace('''          <text x="16" y="286" font-size="11" opacity=".6">Ed25519 escrito neste projeto, conferido contra os quatro vetores da RFC 8032</text>
          <text x="16" y="304" font-size="11" opacity=".6">e contra a implementação de referência da própria RFC.</text>''',
            '''          <text x="16" y="280" font-size="11" opacity=".6">Ed25519 escrito neste projeto, conferido contra os quatro vetores da RFC 8032</text>
          <text x="16" y="296" font-size="11" opacity=".6">e contra a implementação de referência da própria RFC.</text>''',1)
io.open(p,'w',encoding='utf-8').write(s)
