# Fix the figure layout
# 28/08 17:03

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
# A caixa do espelho descia ate a linha do caminho de erro; desce mais 18px
# para o tracejado vermelho passar claramente por cima dela.
a='''          <path d="M563 90 L563 116" stroke="var(--pend)" stroke-width="1.3" stroke-dasharray="4 3" marker-end="url(#setaE)"/>
          <rect x="500" y="120" width="126" height="46" rx="4" fill="none" stroke="var(--pend)" stroke-width="1.4" stroke-dasharray="5 3"/>
          <text x="563" y="139" text-anchor="middle" fill="var(--pend)" font-size="11">espelha o slot</text>
          <text x="563" y="153" text-anchor="middle" fill="var(--pend)" font-size="10.5">.bkp</text>
          <text x="563" y="164" text-anchor="middle" font-size="9" opacity=".55">só quando ligado</text>'''
b='''          <path d="M563 90 L563 132" stroke="var(--pend)" stroke-width="1.3" stroke-dasharray="4 3" marker-end="url(#setaE)"/>
          <rect x="494" y="136" width="138" height="48" rx="4" fill="none" stroke="var(--pend)" stroke-width="1.4" stroke-dasharray="5 3"/>
          <text x="563" y="155" text-anchor="middle" fill="var(--pend)" font-size="11">espelha o slot</text>
          <text x="563" y="169" text-anchor="middle" fill="var(--pend)" font-size="10.5">.bkp</text>
          <text x="563" y="180" text-anchor="middle" font-size="9" opacity=".55">só quando ligado</text>'''
assert a in s; s=s.replace(a,b,1)
# As duas linhas de rodape passavam da largura do desenho.
a='''          <text x="16" y="418" font-size="11" opacity=".55">Alterar segue o mesmo caminho, mas remove a chave antiga do índice só quando ela mudou, e libera os blocos externos antigos no fim.</text>
          <text x="16" y="436" font-size="11" opacity=".55">Excluir tira as chaves, marca os blocos como mortos e marca o slot como livre — sem nunca reaproveitá-lo.</text>'''
b='''          <text x="16" y="416" font-size="10.5" opacity=".55">Alterar segue o mesmo caminho: remove a chave antiga do índice só quando ela mudou, e libera os blocos antigos no fim.</text>
          <text x="16" y="434" font-size="10.5" opacity=".55">Excluir tira as chaves, marca os blocos como mortos e marca o slot como livre — sem nunca reaproveitá-lo.</text>'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
