# Document bulk load in the manual
# 28/08 20:01

import pathlib
p = pathlib.Path("MANUAL.txt")
s = p.read_text()
antigo = """    varrer          database, tabela, [indice], [max], [visao],
                    [depois], [antes], [pular]
    buscar          database, tabela, indice, chave, [max]
    inserir         database, tabela, valores
"""
novo = """    varrer          database, tabela, [indice], [max], [visao],
                    [depois], [antes], [pular], [desde_rownum]
    buscar          database, tabela, indice, chave, [max]
    inserir         database, tabela, valores
    inserir_lote    database, tabela, linhas | texto+formato, [parar_no_erro]
    importar_conferir  texto, [formato]      le sem gravar, para conferir
"""
assert antigo in s
s = s.replace(antigo, novo)

# Secao nova sobre a carga em lote, logo antes de AS DUAS EXCLUSOES.
antigo = """AS DUAS EXCLUSOES
"""
novo = """CARGA EM LOTE

    Gravar mil linhas com mil pedidos custa mil aberturas de tabela, mil
    travas e mil fsync. "inserir_lote" faz tudo uma vez so:

        {"op":"inserir_lote","database":"loja","tabela":"clientes",
         "linhas":[{"id":1,"nome":"Adriano"},{"id":2,"nome":"Maria"}]}

    Medido com 20.000 linhas pela rede, contra o mesmo trabalho linha a linha:

        uma por pedido ...........  2.715 linhas/s
        em lote .................. 25.985 linhas/s   9,6x

    DE ONDE VEM O GANHO. Nao e do disco -- cada linha custa o mesmo la
    dentro: montar o payload, conferir a unicidade, gravar o slot, manter
    cada indice. O ganho e de tudo que ACONTECIA POR LINHA e passa a
    acontecer uma vez.

    NAO HA TRANSACAO, e isso muda o que se pode prometer. Se a linha 700 de
    mil falhar, as 699 anteriores FICAM GRAVADAS: o .reg nao reaproveita
    slot, entao desfazer deixaria 699 buracos. Por isso o padrao e parar:

        "parar_no_erro": true    (padrao) para na primeira recusada
        "parar_no_erro": false   grava o que der e devolve a lista do que
                                 ficou de fora, com o NUMERO da linha

    COLAR EM VEZ DE MONTAR. O mesmo pedido aceita texto colado em cinco
    formatos, e o motor adivinha qual e:

        {"op":"inserir_lote","database":"loja","tabela":"clientes",
         "texto":"id;nome;cidade\\n1;Adriano;Blumenau","formato":"csv"}

        json   lista de objetos, ou objeto com "linhas"
        csv    virgula ou ponto-e-virgula, com aspas e quebra dentro do campo
        txt    separado por TAB
        xml    <linha><id>1</id></linha>
        html   <table> com <tr> e <td>

    A PRIMEIRA LINHA MANDA. O cabecalho diz quais colunas vem, e elas casam
    pelo NOME e nao pela posicao: coluna que a tabela nao tem e recusada com
    o nome dela, coluna que falta fica nula.

    NUMERO NO FORMATO DAQUI. "1.500,50" vira 1500.50 e "1,500.50" tambem --
    o ultimo separador e o decimal. "1.500" e ambiguo (mil e quinhentos ou
    um e meio?) e fica como esta.

    CONFERIR ANTES DE GRAVAR. "importar_conferir" le o texto e devolve o que
    entendeu -- quantas linhas, quais colunas, uma amostra -- sem gravar
    nada. E o que a tela de Importar usa: o botao de gravar so acende depois
    que a conferencia passa.

    PELA LINHA DE COMANDO:

        phxsql importar <dir> <tabela> --arquivo dados.csv [--formato csv]
                        [--conferir] [--seguir]

AS DUAS EXCLUSOES
"""
assert antigo in s
s = s.replace(antigo, novo, 1)
p.write_text(s)
print("ok")
