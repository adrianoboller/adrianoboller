# Fix overlapping boxes in figure 12
# 27/08 21:03

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
velho='''          <rect x="16" y="34" width="108" height="112" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="70" y="82" text-anchor="middle" font-size="11">consulta</text>
          <text x="70" y="100" text-anchor="middle" font-size="10" opacity=".6">um pedido</text>

          <path d="M124 66 L166 66" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaM)"/>
          <path d="M124 114 L166 114" stroke="var(--acento)" stroke-width="1.5" marker-end="url(#setaMa)"/>

          <rect x="170" y="38" width="150" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="245" y="60" text-anchor="middle" font-size="11">varrer o .reg</text>
          <text x="245" y="78" text-anchor="middle" font-size="10" opacity=".6">seek + read por linha</text>

          <rect x="170" y="86" width="150" height="56" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.7"/>
          <text x="245" y="108" text-anchor="middle" fill="var(--acento)" font-size="11">SelectMemory</text>
          <text x="245" y="126" text-anchor="middle" font-size="10" opacity=".6">vetor + mapa em RAM</text>

          <path d="M320 66 L362 66" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaM)"/>
          <path d="M320 114 L362 114" stroke="var(--acento)" stroke-width="1.5" marker-end="url(#setaMa)"/>

          <rect x="366" y="38" width="164" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="448" y="60" text-anchor="middle" font-size="11">50.000 linhas lidas</text>
          <text x="448" y="78" text-anchor="middle" font-size="11" font-weight="600">55.878 us</text>

          <rect x="366" y="86" width="164" height="56" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.7"/>
          <text x="448" y="108" text-anchor="middle" font-size="11">8.333 examinadas</text>
          <text x="448" y="126" text-anchor="middle" fill="var(--acento)" font-size="11" font-weight="600">641 us</text>

          <text x="556" y="80" font-size="30" font-weight="700" fill="var(--acento)">87x</text>
          <text x="556" y="102" font-size="10" opacity=".6">mesma resposta,</text>
          <text x="556" y="116" font-size="10" opacity=".6">conferida linha a linha</text>

          <line x1="16" y1="172" x2="824" y2="172" stroke="currentColor" stroke-width="1" opacity=".25"/>'''
velho = velho.replace('55.878 us','55.878 &#181;s').replace('641 us','641 &#181;s').replace('87x','87&#215;')
novo='''          <rect x="16" y="30" width="108" height="124" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="70" y="86" text-anchor="middle" font-size="11">consulta</text>
          <text x="70" y="104" text-anchor="middle" font-size="10" opacity=".6">um pedido</text>

          <path d="M124 60 L166 60" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaM)"/>
          <path d="M124 124 L166 124" stroke="var(--acento)" stroke-width="1.5" marker-end="url(#setaMa)"/>

          <rect x="170" y="30" width="150" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="245" y="52" text-anchor="middle" font-size="11">varrer o .reg</text>
          <text x="245" y="70" text-anchor="middle" font-size="10" opacity=".6">seek + read por linha</text>

          <rect x="170" y="98" width="150" height="56" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.7"/>
          <text x="245" y="120" text-anchor="middle" fill="var(--acento)" font-size="11">SelectMemory</text>
          <text x="245" y="138" text-anchor="middle" font-size="10" opacity=".6">vetor + mapa em RAM</text>

          <path d="M320 60 L362 60" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaM)"/>
          <path d="M320 124 L362 124" stroke="var(--acento)" stroke-width="1.5" marker-end="url(#setaMa)"/>

          <rect x="366" y="30" width="164" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="448" y="52" text-anchor="middle" font-size="11">50.000 linhas lidas</text>
          <text x="448" y="70" text-anchor="middle" font-size="11" font-weight="600">55.878 &#181;s</text>

          <rect x="366" y="98" width="164" height="56" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.7"/>
          <text x="448" y="120" text-anchor="middle" font-size="11">8.333 examinadas</text>
          <text x="448" y="138" text-anchor="middle" fill="var(--acento)" font-size="11" font-weight="600">641 &#181;s</text>

          <text x="556" y="86" font-size="30" font-weight="700" fill="var(--acento)">87&#215;</text>
          <text x="556" y="110" font-size="10" opacity=".6">mesma resposta,</text>
          <text x="556" y="124" font-size="10" opacity=".6">conferida linha a linha</text>

          <line x1="16" y1="180" x2="824" y2="180" stroke="currentColor" stroke-width="1" opacity=".25"/>'''
assert s.count(velho)==1, "bloco da figura 12 nao casou"
s=s.replace(velho,novo)
s=s.replace('<text x="16" y="194" font-size="10" opacity=".55" letter-spacing=".08em">E QUANDO ALGU&#201;M ESCREVE</text>',
            '<text x="16" y="202" font-size="10" opacity=".55" letter-spacing=".08em">E QUANDO ALGU&#201;M ESCREVE</text>')
open(p,'w').write(s)
print('ajustado')
