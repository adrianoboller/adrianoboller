# Document the sixth file in the spec and readme
# 28/08 17:05

p='docs/FORMATO.md'
s=open(p).read()
a='''| `.log` | Diário de inclusões, alterações e exclusões | `PHXLOG\\0\\0` | sim |

Uma tabela grande se parte em volumes numerados'''
b='''| `.log` | Diário de inclusões, alterações e exclusões | `PHXLOG\\0\\0` | sim |

E um **sexto arquivo opcional**, que só existe quando `espelho` está ligado no
`config.json`:

| Arquivo | Papel | Assinatura | Pagina? |
|---|---|---|---|
| `.bkp` | Espelho byte a byte do `.reg`, volume por volume | igual à do `.reg` | sim, junto |

O `.bkp` **não tem formato próprio**: ele é o `.reg`, escrito duas vezes. O
mesmo slot vai para os dois arquivos, no mesmo *offset*, no mesmo instante — e
por isso todo volume do `.reg` tem um volume irmão do `.bkp`.

Ele é lido **só quando o slot principal falha**: o CRC não bate, ou o byte de
status não é nem `0` (livre) nem `1` (ativo). Nesse caso a leitura busca o
mesmo *offset* no espelho, confere o CRC dele, e devolve a cópia boa. O
`reparar` faz a varredura completa nos dois sentidos: onde o principal quebrou
e o espelho está bom, o principal é reescrito; onde o principal está bom e o
espelho quebrou, o espelho é reescrito.

Custa uma escrita a mais por gravação e o dobro do espaço do `.reg`. Protege
contra o dado ficar **ruim** — bit trocado, escrita cortada, setor com
defeito. **Não** protege contra o disco morrer: os dois arquivos moram no mesmo
lugar.

Uma tabela grande se parte em volumes numerados'''
assert a in s; s=s.replace(a,b,1)
a='''Uma tabela de dados do PhxSql é composta por cinco arquivos físicos que
compartilham o mesmo nome-base:

```
cadastroClientes.reg  +  .ndx  +  .bin  +  .memo  +  .log  =  cadastroClientes
```'''
b='''Uma tabela de dados do PhxSql é composta por cinco arquivos físicos que
compartilham o mesmo nome-base — mais um sexto, opcional:

```
cadastroClientes.reg  +  .ndx  +  .bin  +  .memo  +  .log  =  cadastroClientes
                     ( +  .bkp, o espelho, quando ligado )
```'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('FORMATO.md ok')
