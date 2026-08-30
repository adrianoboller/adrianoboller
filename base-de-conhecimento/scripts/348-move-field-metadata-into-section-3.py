# Move field metadata into section 3
# 28/08 13:10

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()
bloco = pathlib.Path('/tmp/bloco_campo.html').read_text().rstrip() + '\n'

# o campo e a chave primaria pertencem a "A tabela, peca a peca"
v = '''    <figcaption><b>Figura 2.</b> O ponteiro de 16 bytes é o que mantém o slot pequeno.
    O CRC aparece nos dois lados de propósito — assim um ponteiro que aponta para o bloco
    errado é detectado, não só um bloco corrompido.</figcaption>
  </figure>
</section>'''
n = '''    <figcaption><b>Figura 2.</b> O ponteiro de 16 bytes é o que mantém o slot pequeno.
    O CRC aparece nos dois lados de propósito — assim um ponteiro que aponta para o bloco
    errado é detectado, não só um bloco corrompido.</figcaption>
  </figure>

''' + bloco + '''</section>'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('campo e chave primaria foram para a secao 3')
