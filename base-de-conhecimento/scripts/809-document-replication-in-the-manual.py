# Document replication in the manual
# 28/08 20:35

import pathlib
p = pathlib.Path("MANUAL.txt")
s = p.read_text()

antigo = """CARGA EM LOTE
"""
novo = """REPLICACAO: MASTER E ESPELHOS

    A replica PROCURA o master; o master nao empurra nada. E o desenho do
    MySQL(R), e existe por causa do firewall: o master abre UMA porta de
    entrada e nao precisa alcancar ninguem de volta.

        Master 5000 --+--> Slave01
                      +--> Slave02
                      +--> Slave03

    NO MASTER. Papel "source", e a imagem da linha ligada -- ela vem ligada
    sozinha nesse papel, porque sem ela o diario grava QUE a linha mudou e nao
    grava PARA QUE, e as replicas nao tem o que aplicar:

        "replicacao": { "papel": "source", "imagem_da_linha": true }

    NA REPLICA. Papel "replica", somente leitura, e de onde puxar:

        "somente_leitura": true,
        "replicacao": {
          "papel": "replica",
          "origens": [
            {"nome":"curitiba", "host":"10.1.1.102", "porta":5000,
             "token":"...", "usuario":"replicador",
             "senha_hash":"pbkdf2-sha256$210000$...",
             "databases":["Z"], "reconectar_em":10}
          ]
        }

    A SENHA NAO FICA EM CLARO. No config.json da replica mora o senha_hash --
    o mesmo texto do cadastro de usuarios --, e dele sai a chave do
    desafio-resposta. Gere com: phxsqld --senha

    SOMENTE_LEITURA NA REPLICA NAO E OPCIONAL na pratica. Uma replica escrita
    pela aplicacao quebra a numeracao dos rowids, e a proxima inclusao vinda do
    master para a replicacao inteira. O arranque avisa se estiver desligado.

    A TABELA NASCE SOZINHA na replica, a partir do bloco de esquema do master
    -- os mesmos bytes, e nao uma remontagem coluna a coluna.

    O QUE A REPLICA CONFERE. O .reg nunca reaproveita slot e o rowid e sempre
    o proximo. Entao, se a replica aplicar TODOS os eventos NA ORDEM e mais
    ninguem escrever nela, os rowids saem identicos aos do master -- sem
    transmitir nem negociar nada. Se o rowid que ela gerou nao bate com o do
    evento, ela JA divergiu, e a replicacao PARA ali em vez de espalhar.

    MEDIDO com quatro servidores (bancada/replicacao):

        master, com a imagem no diario ........ 18.773 linhas/s
        aplicacao, por replica ................  4.273 eventos/s
        atraso de uma escrita ate as tres .....  1,3 s a 2,1 s
        replica derrubada: voltar a atender ...    343 ms
        replica derrubada: alcancar 4.000 .....    1,0 s
        retrato SHA-256 das quatro tabelas ....  identicos

    O atraso e o intervalo do laco, e nao o trabalho: e quanto a replica dorme
    entre uma pergunta e outra. E a replica APLICA MAIS DEVAGAR do que o
    master escreve -- sob carga sustentada ela fica atras.

    CASCATA. Uma replica pode ser origem de outra, desde que tambem esteja com
    imagem_da_linha ligada. Master -> Slave01 -> Slave03 mediu 1.827 ms contra
    1.679 ms do primeiro salto.

    AS OPERACOES, para quem quiser dirigir de fora:

        posicao    database, [com_esquema]   quantos eventos cada tabela tem
        replicar   database, tabela, desde, [max]   os eventos com a imagem
        aplicar    database, tabela, eventos        grava aqui o que veio

    posicao e replicar exigem a permissao "replicar", que e propria: da para
    concede-la a uma replica sem conceder mais nada. aplicar exige
    "administrar", porque grava com o rowid escolhido e o payload cru.

    Detalhes e o desenho inteiro em docs/REPLICACAO.md.

CARGA EM LOTE
"""
assert antigo in s
s = s.replace(antigo, novo, 1)

antigo = """    inserir_lote    database, tabela, linhas | texto+formato, [parar_no_erro]
    importar_conferir  texto, [formato]      le sem gravar, para conferir
"""
novo = """    inserir_lote    database, tabela, linhas | texto+formato, [parar_no_erro]
    importar_conferir  texto, [formato]      le sem gravar, para conferir
    posicao         database, [com_esquema]  quantos eventos cada tabela tem
    replicar        database, tabela, desde, [max]   os eventos com a imagem
    aplicar         database, tabela, eventos        grava aqui o que veio
"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
