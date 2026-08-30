# Document in MANUAL and PENDENCIAS
# 29/08 03:00

import pathlib

# ---------------- MANUAL: a lista de operacoes e a secao
p = pathlib.Path("MANUAL.txt")
s = p.read_text()
s = s.replace('''    inserir_lote    database, tabela, linhas | texto+formato, [parar_no_erro]''',
'''    inserir_lote    database, tabela, linhas | texto+formato, [parar_no_erro]
    bulkinsert      database, tabela, ligado    reserva a tabela para a carga
    cargas                                      quais tabelas estao reservadas''',1)

alvo = '''A JANELA DE CONFLITO DE ESCRITA'''
novo = '''BULKINSERT: A TABELA RESERVADA PARA A CARGA

    Uma carga longa -- importar um arquivo, migrar de outro banco, semear um
    ambiente -- quer duas coisas que o servidor nao dava: ninguem mais mexendo
    naquela tabela enquanto ela entra, e uma sincronizacao so, no fim.

        {"op":"bulkinsert","database":"Z","tabela":"Clientes","ligado":true}
        ... as insercoes, em lote ou uma a uma ...
        {"op":"bulkinsert","database":"Z","tabela":"Clientes","ligado":false}

    Quando a camada SQL existir, isso se le BULKINSERT(true) e BULKINSERT(false).
    Hoje nao ha camada SQL, entao e operacao de protocolo.

    O QUE OS OUTROS RECEBEM. Erro na hora, e nao espera:

        {"ok":false,"nome":"EM_CARGA","codigo":4002,"repetir":true,
         "erro":"tabela em carga: Z.Clientes esta reservada para carga por
                 maria (ligacao 7) desde 2026-08-29 03:10:22, ha 45s;
                 tente de novo quando ela terminar"}

    O recado diz QUEM e DESDE QUANDO -- sem isso, "tabela em carga" manda a
    pessoa procurar sozinha quem esta segurando. E "repetir":true e a
    diferenca que importa para quem integra: EM_CARGA funciona daqui a pouco,
    ACESSO_NEGADO nao funciona nunca. Sao os DOIS unicos erros do protocolo que
    pedem nova tentativa; o outro e o de E/S.

    A LEITURA TAMBEM PARA, e e de proposito: deixar ler durante a carga e o que
    impediria adiar a manutencao do indice mais tarde.

    O QUE ELE COMPRA, medido com 20.000 linhas em lotes de 5.000:

        sem reserva ....... 43.044 e 44.026 linhas/s
        com reserva ....... 65.737 e 67.339 linhas/s     1,53x

    O ganho vem da janela de durabilidade: reservada, ela nao fecha, e a carga
    inteira vira um fsync so no `bulkinsert(false)`.

    SE QUEM RESERVOU SUMIR, ha duas redes de protecao, e nao uma:

        a queda da conexao solta   na hora, por qualquer caminho de saida
        o prazo solta              recursos.carga_prazo_min, padrao 30 min

    A segunda existe porque soquete pendurado vivo com o cliente morto do outro
    lado existe -- e ali a primeira nao pega. O prazo se renova sozinho a cada
    `bulkinsert(true)` repetido, entao uma carga mais longa que o prazo tem como
    se segurar.

    SO PELA PORTA DE DADOS. HTTP nao tem conexao para cair, entao a reserva
    ficaria so no prazo. Pela tela, use "inserir_lote": ele ja e UMA operacao.

    QUEM PODE. Reservar exige o poder de INSERIR na tabela, e nao mais. Ja
    "cargas" -- a lista de quem reservou o que -- exige ADMINISTRAR, porque
    mostra o movimento dos outros. O administrador tambem solta reserva alheia.

A JANELA DE CONFLITO DE ESCRITA'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
s = s.replace('''      "cache_paginas": 2048,''', '''      "cache_paginas": 2048,
      "carga_prazo_min": 30,''',1)
p.write_text(s)
print("manual ok")
