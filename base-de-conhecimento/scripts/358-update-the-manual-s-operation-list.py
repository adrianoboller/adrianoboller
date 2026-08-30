# Update the manual's operation list
# 28/08 13:13

import pathlib
p = pathlib.Path('MANUAL.txt')
s = p.read_text()
v = '''        Estrutura           as colunas, os tipos, os indices e a paginacao
        Editar conteudo     a grade de dados, com ficha por linha
        Particoes           em que volume cada faixa de rowid cai, e qual
                            arquivo do disco guarda cada faixa. A conta e
                            volume = (rowid-1) / por_arquivo + 1
        Duplicar            copia os cinco arquivos byte a byte: a copia nasce
                            com os MESMOS rowids e na MESMA ordem de digitacao
        Reparar tabela      confere cada slot no .reg contra o espelho .bkp
        Reparar indice      joga o .ndx fora e refaz lendo o .reg
        Nova tabela         colunas, indices, schema e paginacao
        Excluir             apaga os cinco arquivos e o espelho, de uma vez
'''
n = '''        Estrutura           as colunas, os tipos, os indices e a particao
        Editar conteudo     a grade de dados, com ficha por linha
        Config. e diretivas a geometria decidida na criacao, as chaves, os
                            volumes no disco, e o que a tabela herda do servidor
        Particoes           em que volume cada faixa de rowid cai, e qual
                            arquivo do disco guarda cada faixa
        Duplicar            copia os cinco arquivos byte a byte, no mesmo banco:
                            a copia nasce com os MESMOS rowids e na MESMA ordem
        Copiar              poe a tabela na area de transferencia
        Colar aqui          traz para este banco o que esta na area
        Reparar tabela      confere cada slot no .reg contra o espelho .bkp
        Reparar indice      joga o .ndx fora e refaz lendo o .reg
        Nova tabela         campos, indices, schema e particao
        Excluir             apaga os cinco arquivos e o espelho, de uma vez

    GESTAO DO BANCO

    O botao Gerir Banco -- ou o menu Banco, ou Alt+6 -- junta o que se faz sobre
    o database inteiro, em quatro grupos:

        Dados e catalogo    Tabelas, SysTables, SysColumns, Copiar tabela
        Config. e acesso    Configuracoes do banco, Diretivas de acesso,
                            Editor de menu
        Operacao            Conexoes, Arquivos bloqueados, Transacoes,
                            Backup e restauracao
        Ainda nao existe    Triggers, Procedures, Jobs, Modo exclusivo

    Os quatro ultimos ficam APAGADOS e, clicados, abrem uma tela que diz o que
    falta e de que depende. Sumir com eles esconderia o roteiro.

    AS TELAS DE CONFIGURACAO LEEM, NAO GRAVAM. Sao tres -- servidor, banco e
    usuarios --, e cada campo aparece com o nome que tem no config.json, o valor
    valendo agora, e para que serve. Gravar pela porta web daria a uma sessao
    roubada o poder de abrir o firewall, esvaziar os comandos proibidos e criar
    supervisor. Para mudar: edite o arquivo e suba o servico.
'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('MANUAL: operacoes e gestao do banco')
