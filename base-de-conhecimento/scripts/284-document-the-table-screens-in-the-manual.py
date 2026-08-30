# Document the table screens in the MANUAL
# 28/08 11:08

import pathlib
p = pathlib.Path('MANUAL.txt')
s = p.read_text()
v = '''    E tres telas de administracao:

        Usuarios      o cadastro e o poder de cada um sobre cada base
        Acessos       o acessos.log: IP, data, hora, operacao, usuario
        Bloqueios     a blacklist.json, com o motivo e ate quando
'''
n = '''    E tres telas de administracao:

        Usuarios      o cadastro e o poder de cada um sobre cada base
        Acessos       o acessos.log: IP, data, hora, operacao, usuario
        Bloqueios     a blacklist.json, com o motivo e ate quando

    GESTAO DE TABELAS

    O botao Tabelas -- ou o menu Tabelas, ou Alt+5 -- lista as tabelas do banco
    corrente. Um clique numa linha abre as oito operacoes sobre ela:

        Estrutura           as colunas, os tipos, os indices e a paginacao
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

    ATENCAO NA EXCLUSAO. Nao ha desfazer e nao ha lixeira: a tela pede o nome
    da tabela DIGITADO, e o servidor exige o mesmo nome no campo "confirmar".
    A permissao e "administrar", nao "excluir" -- poder perder uma linha nao e
    poder perder a tabela.

    A PAGINACAO SE ESCOLHE NA CRIACAO E NAO MUDA DEPOIS. Ela e o divisor que
    transforma o rowid em endereco; trocar mais tarde mudaria o endereco de
    cada registro ja gravado. E nao existe "sem teto": o sufixo tem largura
    fixa, entao com tres digitos cabem 999 volumes. Teto deixado em zero vira
    o maior que couber.
'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
