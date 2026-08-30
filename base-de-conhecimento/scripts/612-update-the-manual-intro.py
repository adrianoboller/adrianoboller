# Update the manual intro
# 28/08 18:04

import io
p='MANUAL.txt'
s=io.open(p,encoding='utf-8').read()
velho='''O PhxSql guarda cada tabela logica em cinco arquivos fisicos:

    cadastroClientes.reg    registros, na ordem de digitacao
    cadastroClientes.ndx    indices (B+tree)
    cadastroClientes.bin    binarios (fotos, anexos)
    cadastroClientes.memo   textos longos
    cadastroClientes.log    diario de inclusoes, alteracoes e exclusoes

    .reg + .ndx + .bin + .memo + .log  =  cadastroClientes

E um SEXTO arquivo, opcional, que so existe com "espelho" ligado no
config.json:'''
novo='''O PhxSql guarda cada tabela logica em sete arquivos fisicos:

    cadastroClientes.reg    registros, na ordem de digitacao
    cadastroClientes.ndx    indices (B+tree)
    cadastroClientes.bin    binarios (fotos, anexos)
    cadastroClientes.memo   textos longos
    cadastroClientes.log    diario de inclusoes, alteracoes e exclusoes
    cadastroClientes.trash  as linhas que sairam do .reg, inteiras
    cadastroClientes.reason por que cada linha foi excluida, e por quem

    .reg + .ndx + .bin + .memo + .log + .trash + .reason = cadastroClientes

OS TRES ULTIMOS SAO OS ARQUIVOS DO ADMINISTRADOR. A razao esta no que cada um
guarda:

    .trash    guarda o dado que alguem mandou apagar. Quem so tem "ler" perdeu
              o direito aquela linha no instante em que ela foi excluida, e a
              lixeira devolveria o direito por outra porta.
    .reason   costuma ser MAIS revelador que o registro que foi excluido:
              "fraude", "pedido de remocao do titular", "duplicidade com o
              contrato X".
    .log      tem permissao propria ("diario"), que so um administrador
              concede.

E um OITAVO arquivo, opcional, que so existe com "espelho" ligado no
config.json:'''
assert velho in s
s=s.replace(velho,novo,1)
s=s.replace('''        Estrutura     colunas, tipos, em qual dos cinco arquivos cada coluna''',
            '''        Estrutura     colunas, tipos, em qual dos arquivos cada coluna''',1)
s=s.replace('''        Duplicar            copia os cinco arquivos byte a byte, no mesmo banco:''',
            '''        Duplicar            copia os arquivos byte a byte, no mesmo banco:''',1)
s=s.replace('''        Excluir             apaga os cinco arquivos e o espelho, de uma vez''',
            '''        Excluir             apaga todos os arquivos e o espelho, de uma vez''',1)
s=s.replace('''    Tudo debaixo da raiz de dados: os cinco arquivos de cada tabela, os''',
            '''    Tudo debaixo da raiz de dados: os arquivos de cada tabela, os''',1)
s=s.replace('''PhxSql - cinco arquivos, uma tabela.''','''PhxSql - sete arquivos, uma tabela.''',1)
io.open(p,'w',encoding='utf-8').write(s)
print('manual ok')
