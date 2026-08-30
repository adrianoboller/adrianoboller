# Document the pivot and grouping in the manual
# 28/08 13:36

import pathlib
# ------------------------------------------------------------------ MANUAL
p = pathlib.Path('MANUAL.txt'); s = p.read_text()
v = '''    sistabelas      database                 o catalogo de tabelas
    siscolunas      database, [tabela]       o dicionario de dados'''
n = '''    sistabelas      database                 o catalogo de tabelas
    siscolunas      database, [tabela]       o dicionario de dados
    pivotar         database, tabela, linhas, [colunas], [valor],
                    [agregador], [juntar], [max]   a tabulacao cruzada'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''    ler          bancos, tabelas, esquema, ler, varrer, buscar,
                 sistabelas, siscolunas'''
n = '''    ler          bancos, tabelas, esquema, ler, varrer, buscar,
                 sistabelas, siscolunas, pivotar'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''    CATALOGO DO SISTEMA'''
n = '''    TABELA DINAMICA (PIVOT)

    O botao Pivot -- ou Alt+7 -- abre um assistente de tres passos: quais
    tabelas entram, que campo vai em cada eixo, e o resultado.

    Passo 1, TABELAS. Escolha a tabela dos FATOS (a que tem as linhas a somar).
    Se ela declarar chave estrangeira, as tabelas de consulta aparecem
    propostas; senao da para juntar a mao, dizendo a tabela e a coluna de
    ligacao. A tabela de consulta e lida INTEIRA para a memoria, uma vez -- e o
    hash join, e o teto e 500.000 linhas por tabela.

    Passo 2, CAMPOS. Arraste da lista da esquerda para Linhas, Colunas ou
    Valores. Campo de data ganha um seletor de granularidade: cada valor, por
    dia, mes, trimestre ou ano -- cruzar venda por DIA daria uma coluna por dia
    do ano, que nao serve. Os campos das tabelas juntadas aparecem com o
    prefixo (cli.cidade).

    Como resumir: soma, media, contagem, minimo, maximo ou valores distintos.
    CONTAGEM e o unico que dispensa campo de valor -- ele conta linhas.

    Passo 3, RESULTADO. A grade, com total por linha, por coluna e geral.
    "Copiar como CSV" leva para a area de transferencia; "Ver o pedido" mostra
    o JSON que faz o mesmo cruzamento pela porta 5000.

    A AGREGACAO ACONTECE NO SERVIDOR. Um pivot resume: cem mil linhas viram uma
    grade de vinte por doze. Trazer as cem mil para o navegador somar seria
    pagar o transporte do que vai ser jogado fora.

    CELULA VAZIA NAO E ZERO. Vazio quer dizer que nenhuma linha caiu ali; zero
    seria "somou e deu nada".

    DINHEIRO NAO PERDE CENTAVO. Um campo Decimal e somado no dominio inteiro
    escalado e so vira texto na saida. A media divide uma vez, no fim -- nao a
    cada parcela.

    CATALOGO DO SISTEMA'''
assert s.count(v) == 1
s = s.replace(v, n)

# a faixa de agrupamento da aba Conteudo
v = '''        Conteudo      as linhas, no phx-grid: arraste um cabecalho para a
                      faixa de cima e ele AGRUPA, com contagem e totais por
                      grupo. Da para empilhar varios niveis e reordenar
                      arrastando as pastilhas. Tem busca global e paginacao.
                      A ordem sai da digitacao (.reg) ou de qualquer indice'''
n = '''        Conteudo      as linhas, no phx-grid: arraste um cabecalho para a
                      faixa de cima e ele AGRUPA, com contagem e totais por
                      grupo. Da para empilhar varios niveis e reordenar
                      arrastando as pastilhas; a seta na pastilha inverte a
                      ordem daquele nivel. Cada grupo ganha um rodape com o
                      total alinhado NA COLUNA, e a grade um total geral sobre
                      o conjunto filtrado -- que nao muda ao virar de pagina.
                      "expandir tudo" e "recolher tudo" abrem e fecham de uma
                      vez. O agregador de cada coluna cicla no clique da
                      pastilha SUM do cabecalho: soma, media, contagem, minimo,
                      maximo. Tem busca global e paginacao. A ordem sai da
                      digitacao (.reg) ou de qualquer indice'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('MANUAL')

# ---------------------------------------------------------------- USUARIOS
p = pathlib.Path('docs/USUARIOS.md'); s = p.read_text()
v = '`buscar`, `sistabelas`, `siscolunas` |'
n = '`buscar`, `sistabelas`, `siscolunas`, `pivotar` |'
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('USUARIOS.md')
