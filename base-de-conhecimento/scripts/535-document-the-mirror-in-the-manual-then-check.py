# Document the mirror in the manual, then check
# 28/08 17:05

p='MANUAL.txt'
s=open(p).read()
a='''    cadastroClientes.log    diario de inclusoes, alteracoes e exclusoes

    .reg + .ndx + .bin + .memo + .log  =  cadastroClientes
'''
b='''    cadastroClientes.log    diario de inclusoes, alteracoes e exclusoes

    .reg + .ndx + .bin + .memo + .log  =  cadastroClientes

E um SEXTO arquivo, opcional, que so existe com "espelho" ligado no
config.json:

    cadastroClientes.bkp    espelho byte a byte do .reg, volume por volume

O .bkp nao tem formato proprio: ele E o .reg, escrito duas vezes. O mesmo slot
vai para os dois arquivos, no mesmo offset, no MESMO INSTANTE -- nao e uma
copia feita depois.

Ele so e lido quando o slot principal falha (o CRC nao bate, ou o byte de
status nao e nem livre nem ativo). Nesse caso a leitura pega a copia boa do
espelho e devolve. O comando `reparar` percorre todos os slots e conserta nos
dois sentidos: principal quebrado com espelho bom, o principal e reescrito;
principal bom com espelho quebrado, o espelho e reescrito.

Custa uma escrita a mais por gravacao e o dobro do espaco do .reg. Protege
contra o dado ficar RUIM -- bit trocado, escrita cortada, setor com defeito.
NAO protege contra o disco morrer: os dois arquivos moram no mesmo lugar. Para
isso existe o backup, que vai para outro lugar.
'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('MANUAL ok')
