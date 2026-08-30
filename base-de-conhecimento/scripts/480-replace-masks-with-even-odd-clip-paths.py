# Replace masks with even-odd clip paths
# 28/08 15:48

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
a='''          <clipPath id="jcA"><circle cx="34" cy="30" r="23"/></clipPath>
          <mask id="jmSemB"><rect x="0" y="0" width="96" height="60" fill="#fff"/><circle cx="58" cy="30" r="23" fill="#000"/></mask>
          <mask id="jmSemA"><rect x="0" y="0" width="96" height="60" fill="#fff"/><circle cx="34" cy="30" r="23" fill="#000"/></mask>'''
b='''          <clipPath id="jcA"><circle cx="34" cy="30" r="23"/></clipPath>
          <!-- Os dois circulos como UM caminho de dois subcaminhos, com
               `clip-rule="evenodd"`: a regiao que sobra e a que esta dentro de
               exatamente um deles, ou seja, os dois crescentes sem o meio.
               Uma mascara faria o mesmo, mas exigiria branco e preto
               literais, e a regra da pagina e nao ter cor literal nenhuma. -->
          <clipPath id="jcSoUm">
            <path clip-rule="evenodd"
                  d="M11,30 a23,23 0 1,0 46,0 a23,23 0 1,0 -46,0 Z M35,30 a23,23 0 1,0 46,0 a23,23 0 1,0 -46,0 Z"/>
          </clipPath>'''
assert a in s; s=s.replace(a,b,1)
s=s.replace('<g mask="url(#jmSemB)"><circle cx="34" cy="30" r="23" fill="var(--reg)"/></g>',
            '<g clip-path="url(#jcSoUm)"><circle cx="34" cy="30" r="23" fill="var(--reg)"/></g>')
s=s.replace('<g mask="url(#jmSemA)"><circle cx="58" cy="30" r="23" fill="var(--reg)"/></g>',
            '<g clip-path="url(#jcSoUm)"><circle cx="58" cy="30" r="23" fill="var(--reg)"/></g>')
open(p,'w').write(s)
print('dossie ok')

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
a='''  const defs = `<defs>
    <clipPath id="cA-${tipo}">${c(A)}</clipPath>
    <mask id="mSemB-${tipo}"><rect x="0" y="0" width="96" height="58" fill="#fff"/>${c(B, 'fill="#000"')}</mask>
    <mask id="mSemA-${tipo}"><rect x="0" y="0" width="96" height="58" fill="#fff"/>${c(A, 'fill="#000"')}</mask>
  </defs>`;'''
b='''  // Os dois círculos como UM caminho de dois subcaminhos, com
  // `clip-rule="evenodd"`: sobra o que está dentro de exatamente um deles — os
  // dois crescentes, sem o meio. Uma máscara faria o mesmo e exigiria branco e
  // preto literais, que não seguem o tema; o recorte não precisa de cor
  // nenhuma.
  const soUm = `M12,29 a24,24 0 1,0 48,0 a24,24 0 1,0 -48,0 Z`
             + ` M36,29 a24,24 0 1,0 48,0 a24,24 0 1,0 -48,0 Z`;
  const defs = `<defs>
    <clipPath id="cA-${tipo}">${c(A)}</clipPath>
    <clipPath id="cUm-${tipo}"><path clip-rule="evenodd" d="${soUm}"/></clipPath>
  </defs>`;'''
assert a in s; s=s.replace(a,b,1)
s=s.replace('''    so_esquerda:  `<g mask="url(#mSemB-${tipo})">${c(A, F)}</g>`,
    so_direita:   `<g mask="url(#mSemA-${tipo})">${c(B, F)}</g>`,
    so_dos_lados: `<g mask="url(#mSemB-${tipo})">${c(A, F)}</g>`
                + `<g mask="url(#mSemA-${tipo})">${c(B, F)}</g>`,''',
'''    so_esquerda:  `<g clip-path="url(#cUm-${tipo})">${c(A, F)}</g>`,
    so_direita:   `<g clip-path="url(#cUm-${tipo})">${c(B, F)}</g>`,
    so_dos_lados: `<g clip-path="url(#cUm-${tipo})">${c(A, F)}${c(B, F)}</g>`,''',1)
open(p,'w').write(s)
print('ui ok')
