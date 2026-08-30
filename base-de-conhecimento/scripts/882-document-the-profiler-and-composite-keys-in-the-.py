# Document the profiler and composite keys in the manual
# 28/08 23:08

import pathlib
p = pathlib.Path("MANUAL.txt")
s = p.read_text()

antigo = """REPLICACAO: MASTER E ESPELHOS
"""
novo = """PROFILER: O QUE ESTA CHEGANDO, ANTES DE VIRAR DADO

    O equivalente do Profiler do SQL Server(R). Liga-se, escolhe-se o que
    observar, e ve-se o trafego passar -- com o TEXTO do pedido, do jeito que
    veio pelo soquete.

    O ponto de captura e uma linha depois do read_line e uma antes do
    despacho: NADA FOI GRAVADO AINDA. Por isso o pedido que TRAVA aparece na
    lista como "em curso" -- que e justamente o que se quer achar.

        {"op":"profiler_ligar",
         "database":"Comercial",        vazio = todos
         "usuario":"adm",               vazio = todos
         "operacao":"inserir",          vazio = todas
         "so_escrita":false,
         "arquivo":"/var/log/phx.txt",  vazio = so em memoria
         "guardar":500}                 tamanho do anel

        {"op":"profiler","max":200,"desde_serial":0}
        {"op":"profiler_desligar"}
        {"op":"profiler_limpar"}

    Na tela: botao Profiler na barra de ferramentas. Ela atualiza sozinha e
    pede so o que ainda nao viu.

    A SENHA NAO PASSA POR AQUI. O pedido e mostrado como chegou MENOS os
    campos sensiveis: senha, senha_b64, senha_hash, nova_senha, prova, token,
    chave, chave_privada e assinatura viram "***" ANTES de encostar na
    memoria ou no arquivo. Pedido que nao e JSON valido nao vira texto
    nenhum: vira o tamanho dele, porque nao ha como tapar um campo numa
    estrutura que nao se le.

    SO ADMINISTRADOR, e a razao esta no que ele mostra: o texto dos pedidos
    de todo mundo, com os dados que estao sendo gravados dentro. Quem pode ler
    uma tabela nao ganha por isso o direito de ver o que os outros escrevem
    nela -- nem de mandar o servidor escrever um arquivo no disco.

    O ANEL TEM TETO. Profiler esquecido ligado num servidor movimentado nao
    pode comer a memoria da maquina: o mais antigo sai, e a resposta diz
    quantos sairam. O ARQUIVO, esse, cresce -- quem o pediu escolheu o
    caminho e sabe onde ele esta. Ele abre em modo APPEND: religar no mesmo
    arquivo continua o registro em vez de apagar o que estava la.

    ELE OBSERVA AS DUAS PORTAS -- a de dados e a da interface web. Deixar a
    web de fora faria o profiler mentir por omissao justamente para quem esta
    olhando por ela. E NAO observa a si mesmo: a tela pergunta uma vez por
    segundo, e sem isso o anel viraria so ele.

    O ARQUIVO fica assim:

      2026-08-28 23:01:33 127.0.0.1  adm  inserir  loja.clientes  ok  3ms
        194B  {"op":"inserir",...,"token":"***"}
      2026-08-28 23:01:33 127.0.0.1  adm  inserir  loja.clientes  ERRO 1ms
        151B  {"op":"inserir",...}  <- chave duplicada: indice unico porId

CHAVE COMPOSTA

    Um indice sobre varias colunas. Ele e LIVRE ou UNICO, e a diferenca nao e
    de grau:

        livre  -- aceita a combinacao repetida. E um indice de busca.
        unico  -- recusa a repetida, e recusa ANTES DE GRAVAR.

    Recusar antes importa por causa do formato: o .reg nao reaproveita slot.
    Gravar primeiro e descobrir depois deixaria um buraco permanente, e uma
    carga com muita chave repetida iria inchando o arquivo sem nunca crescer
    a tabela.

        "indices":[{"nome":"porDocumento",
                    "colunas":["empresa","filial","documento"],
                    "unico":true}]

    Mudar QUALQUER uma das colunas ja e outra chave. A alteracao respeita a
    regra: levar uma linha para a chave de outra e recusado; reescrever a
    linha com a chave DELA mesma, nao -- senao salvar uma ficha sem mexer na
    chave seria impossivel.

    O esquema DECLARA que a chave e composta; a tela le dali em vez de contar
    colunas por conta propria.

VARIAS INSTANCIAS, DOCKER E CLUSTER

    VARIAS INSTANCIAS: sim. Cada phxsqld le o config.json do diretorio em que
    foi iniciado -- porta, base, usuarios e papel sao daquela instancia. Nao
    ha registro global nem porta fixa.

        cd /srv/erp        && phxsqld    # 5000 / 5001
        cd /srv/telemetria && phxsqld    # 5100 / 5101

    DOCKER: sim, e a imagem final e `scratch` -- sem shell, sem gerenciador
    de pacotes, so o binario. So e possivel porque nao ha dependencia externa
    nenhuma. Dockerfile e docker-compose.yml na raiz.

    CLUSTER: NAO. Ha replicacao (um master, N replicas, medida), e com ela a
    escala de LEITURA. Nao ha endereco unico, nem eleicao de primario, nem
    promocao automatica -- o failover e manual. docs/CLUSTER.md diz item por
    item o que falta.

REPLICACAO: MASTER E ESPELHOS
"""
assert antigo in s
s = s.replace(antigo, novo, 1)

antigo = """    posicao         database, [com_esquema]  quantos eventos cada tabela tem"""
novo = """    profiler_ligar  [database], [usuario], [operacao], [so_escrita],
                    [arquivo], [guardar]     comeca a observar a porta
    profiler        [max], [desde_serial]    o que foi observado
    profiler_desligar                        para
    profiler_limpar                          esvazia o anel
    posicao         database, [com_esquema]  quantos eventos cada tabela tem"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
