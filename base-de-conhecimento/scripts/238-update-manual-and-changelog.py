# Update manual and changelog
# 27/08 22:43

import re
s = open('MANUAL.txt').read()

# secao 9 (Centro de Controle) ganha o painel e o grid
s = s.replace('''O QUE TEM NA TELA

    A esquerda, a arvore: bancos -> tabelas da raiz -> schemas -> tabelas.''',
'''O QUE TEM NA TELA

    A primeira tela e o PAINEL: o servidor inteiro de uma vez, em graficos.

        Numeros do topo   bancos, registros, usuarios, conexoes agora,
                          acessos, recusados, IPs bloqueados, tabelas em RAM
        Operacoes por hora  as ultimas 24 horas, com as recusadas em vermelho
        Mais pedidas        quais operacoes, e quantas passaram ou nao
        Usuarios por nivel  quem pode o que
        Maiores tabelas     por registro, com o tamanho do .reg
        De onde vem         por IP, separando aceito de recusado
        Bancos              tabelas e registros por banco
        Quem mais usou      por login

    Tudo isso vem de UMA chamada ao servidor -- a operacao "painel", que
    agrega do lado de la. Dez chamadas deixariam a tela dez vezes mais lenta
    so por causa da ida e volta.

    E o painel conta so o que VOCE poderia abrir: base sem permissao de
    leitura nao entra na conta. O numero nunca revela o que a arvore esconde.

    Depois do painel, a arvore: bancos -> tabelas da raiz -> schemas -> tabelas.''')

s = s.replace('''        Conteudo      as linhas, na ordem de digitacao (.reg) ou na ordem de
                      qualquer indice''',
'''        Conteudo      as linhas, no phx-grid: arraste um cabecalho para a
                      faixa de cima e ele AGRUPA, com contagem e totais por
                      grupo. Da para empilhar varios niveis e reordenar
                      arrastando as pastilhas. Tem busca global e paginacao.
                      A ordem sai da digitacao (.reg) ou de qualquer indice''')
open('MANUAL.txt','w').write(s)
