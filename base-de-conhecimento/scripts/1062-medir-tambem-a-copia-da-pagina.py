# Medir tambem a copia da pagina
# 29/08 04:12

import io,re
p='crates/phxsql-store/examples/onde-doi.rs'
s=io.open(p,encoding='utf-8').read()
# a linha final que explica o acerto de cache: passa a citar o numero medido
velho = '''  So a leitura do arquivo e a gravacao passam pelo CRC -- {:.2} paginas
  por linha, ou {:.1} us de CRC, de {dois:.1} us medidos ({:.0}%). O acerto de
  cache custa a copia da pagina, e nao o CRC dela: e dai que veio o ganho.'''
print(velho in s)
