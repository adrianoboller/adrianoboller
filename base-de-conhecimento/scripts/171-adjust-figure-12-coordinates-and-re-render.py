# Adjust figure 12 coordinates and re-render
# 27/08 21:04

p='docs/dossie/dossie-phxsql.html'
linhas=open(p).read().split('\n')
# ajustes de y so no bloco da figura 12 (linhas 1374..1410, indice 1373..)
trocas = {
 '          <rect x="16" y="34" width="108" height="112" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>':
 '          <rect x="16" y="30" width="108" height="124" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>',
 '          <text x="70" y="82" text-anchor="middle" font-size="11">consulta</text>':
 '          <text x="70" y="86" text-anchor="middle" font-size="11">consulta</text>',
 '          <text x="70" y="100" text-anchor="middle" font-size="10" opacity=".6">um pedido</text>':
 '          <text x="70" y="104" text-anchor="middle" font-size="10" opacity=".6">um pedido</text>',
 '          <path d="M124 66 L166 66" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaM)"/>':
 '          <path d="M124 58 L166 58" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaM)"/>',
 '          <path d="M124 114 L166 114" stroke="var(--acento)" stroke-width="1.5" marker-end="url(#setaMa)"/>':
 '          <path d="M124 126 L166 126" stroke="var(--acento)" stroke-width="1.5" marker-end="url(#setaMa)"/>',
 '          <rect x="170" y="38" width="150" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>':
 '          <rect x="170" y="30" width="150" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>',
 '          <text x="245" y="60" text-anchor="middle" font-size="11">varrer o .reg</text>':
 '          <text x="245" y="52" text-anchor="middle" font-size="11">varrer o .reg</text>',
 '          <text x="245" y="78" text-anchor="middle" font-size="10" opacity=".6">seek + read por linha</text>':
 '          <text x="245" y="70" text-anchor="middle" font-size="10" opacity=".6">seek + read por linha</text>',
 '          <rect x="170" y="86" width="150" height="56" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.7"/>':
 '          <rect x="170" y="98" width="150" height="56" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.7"/>',
 '          <text x="245" y="108" text-anchor="middle" fill="var(--acento)" font-size="11">SelectMemory</text>':
 '          <text x="245" y="120" text-anchor="middle" fill="var(--acento)" font-size="11">SelectMemory</text>',
 '          <text x="245" y="126" text-anchor="middle" font-size="10" opacity=".6">vetor + mapa em RAM</text>':
 '          <text x="245" y="138" text-anchor="middle" font-size="10" opacity=".6">vetor + mapa em RAM</text>',
 '          <path d="M320 66 L362 66" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaM)"/>':
 '          <path d="M320 58 L362 58" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaM)"/>',
 '          <path d="M320 114 L362 114" stroke="var(--acento)" stroke-width="1.5" marker-end="url(#setaMa)"/>':
 '          <path d="M320 126 L362 126" stroke="var(--acento)" stroke-width="1.5" marker-end="url(#setaMa)"/>',
 '          <rect x="366" y="38" width="164" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>':
 '          <rect x="366" y="30" width="164" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>',
 '          <text x="448" y="60" text-anchor="middle" font-size="11">50.000 linhas lidas</text>':
 '          <text x="448" y="52" text-anchor="middle" font-size="11">50.000 linhas lidas</text>',
 '          <rect x="366" y="86" width="164" height="56" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.7"/>':
 '          <rect x="366" y="98" width="164" height="56" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.7"/>',
 '          <text x="448" y="108" text-anchor="middle" font-size="11">8.333 examinadas</text>':
 '          <text x="448" y="120" text-anchor="middle" font-size="11">8.333 examinadas</text>',
 '          <line x1="16" y1="172" x2="824" y2="172" stroke="currentColor" stroke-width="1" opacity=".25"/>':
 '          <line x1="16" y1="182" x2="824" y2="182" stroke="currentColor" stroke-width="1" opacity=".25"/>',
}
n=0
for i,l in enumerate(linhas):
    if l in trocas:
        linhas[i]=trocas[l]; n+=1
# as linhas com caractere especial, por conteudo
for i,l in enumerate(linhas):
    if '55.878' in l and 'y="78"' in l:
        linhas[i]=l.replace('y="78"','y="70"'); n+=1
    elif '641' in l and 'y="126"' in l and 'font-weight' in l:
        linhas[i]=l.replace('y="126"','y="138"'); n+=1
    elif '87' in l and 'font-size="30"' in l:
        linhas[i]=l.replace('y="80"','y="86"'); n+=1
    elif 'mesma resposta,' in l:
        linhas[i]=l.replace('y="102"','y="112"'); n+=1
    elif 'conferida linha a linha' in l:
        linhas[i]=l.replace('y="116"','y="126"'); n+=1
    elif 'E QUANDO ALGU' in l:
        linhas[i]=l.replace('y="194"','y="204"'); n+=1
open(p,'w').write('\n'.join(linhas))
print(f'{n} coordenadas ajustadas')
