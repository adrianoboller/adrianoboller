# Insert web section into manual
# 27/08 19:55

import re
s = open('MANUAL.txt').read()

# 1. renumera as secoes 9..14 -> 10..15 (de tras para frente, para nao colidir)
for n in range(14, 8, -1):
    velho = f"\n{n}. "
    novo  = f"\n{n+1}. "
    # so os cabecalhos de secao: linha comecando com "N. TITULO" seguida da regua
    s = re.sub(rf"\n{n}\. ([A-Z][^\n]*)\n-{{80}}", lambda m, n=n: f"\n{n+1}. {m.group(1)}\n" + "-"*80, s)

secao = '''
9. CENTRO DE CONTROLE (INTERFACE WEB)
--------------------------------------------------------------------------------
A mesma coisa que a porta 5000 faz, pelo navegador. A pagina esta embutida no
proprio phxsqld -- nao ha servidor web para instalar, nem arquivo para copiar.

LIGAR

    No config.json:

        "web": {
          "ligado": true,
          "bind": "127.0.0.1:5001",
          "sessao_minutos": 60
        }

    Depois: phxsqld --config config.json

    No terminal aparece:

        interface web em http://127.0.0.1:5001 | sessao de 60 min

    Abra esse endereco. Entre com o token do config.json e o seu usuario.

    VEM DESLIGADA. Ligar abre uma porta a mais, e isso e decisao de quem
    administra. Quando ligada, escuta so no proprio computador ate voce
    mudar o bind.

    A porta e outra, separada da 5000 de proposito: quem fala HTTP nao e quem
    fala JSON Lines, e separar deixa o firewall tratar cada uma do seu jeito.
    O servidor recusa subir com as duas no mesmo endereco.

O QUE TEM NA TELA

    A esquerda, a arvore: bancos -> tabelas da raiz -> schemas -> tabelas.

    Escolhida uma tabela, cinco abas:

        Estrutura     colunas, tipos, em qual dos cinco arquivos cada coluna
                      mora, indices, chaves estrangeiras e a paginacao
        Conteudo      as linhas, na ordem de digitacao (.reg) ou na ordem de
                      qualquer indice
        Indices       o que ha no .ndx, e por que ele e o unico que nao pagina
        Diario        o .log: quem alterou o que, quando, e em que versao
        Integridade   roda o "verificar": CRC de cada registro, de cada pagina
                      de indice, de cada bloco externo e de cada evento

    E tres telas de administracao:

        Usuarios      o cadastro e o poder de cada um sobre cada base
        Acessos       o acessos.log: IP, data, hora, operacao, usuario
        Bloqueios     a blacklist.json, com o motivo e ate quando

    O que voce ve e o que o SEU usuario pode ver. A interface nao contorna o
    portao de permissao: quem nao pode inserir recebe a recusa do servidor,
    igualzinho a quem pede pela porta 5000.

A SENHA NAO TRAFEGA

    Em http://127.0.0.1 e em https:// o navegador oferece cifra, e a pagina
    usa desafio-resposta: pede um desafio, deriva a prova com PBKDF2 ali
    mesmo e manda so a prova. A senha nao sai da sua maquina, e gravar o
    dialogo para repetir depois nao autentica ninguem -- o desafio vale uma
    vez so.

    Em http:// para outra maquina o navegador NAO oferece a cifra. A pagina
    cai em Base64 e avisa na tela, com todas as letras. Base64 esconde a
    senha de quem olha por cima do ombro, nao de quem captura o pacote.
    Para expor na rede: tunel (IPSec, WireGuard) ou um proxy com TLS.

SESSAO

    A porta 5000 autentica uma vez por CONEXAO. HTTP nao tem conexao que
    dure, entao o login devolve um identificador de sessao que o navegador
    repete a cada clique no cabecalho X-Sessao.

    O prazo conta a partir do ULTIMO clique, nao do login: cada uso renova.
    "Sair" derruba a sessao na hora, no servidor -- nao so na tela.

    E o que faz a conta cara valer a pena uma vez so: o PBKDF2 de 210.000
    iteracoes custa cerca de 300 ms no login e 0 ms em cada clique seguinte.

O QUE ESTA PORTA NAO FAZ

    Nao serve arquivo do disco. Nao lista diretorio. Nao interpreta caminho.
    Ha tres rotas, e nenhuma toca o sistema de arquivos:

        GET  /        a pagina (embutida no binario)
        GET  /saude   sinal de vida, sem token, para a pagina saber se ha
                      servidor nesta origem
        POST /api     o mesmo protocolo da secao 8, um pedido por vez

    Qualquer outro caminho e 404. Nao ha ".." para explorar porque nao ha
    diretorio para escapar.

BLOQUEIO E LOG VALEM NAS DUAS PORTAS

    A lista de bloqueio e do SERVIDOR, nao da porta. Cinco tokens errados
    pelo navegador bloqueiam tambem a porta 5000, e o phxsqld --desbloquear
    solta as duas. Todo pedido pela web entra no acessos.log com IP, data,
    hora, operacao e usuario, do mesmo jeito que os da porta 5000.

SEM SERVIDOR: MODO DEMONSTRACAO

    Aberta sem servidor na origem (o arquivo direto do disco, por exemplo),
    a pagina percebe e abre com dados embutidos, para ser avaliada antes de
    instalar qualquer coisa. Um selo "modo demonstracao" fica visivel no topo
    o tempo todo. Nenhum dado seu passa por ali -- nao ha servidor para ter
    dado seu.

'''
ancora = "\n10. LOG DE ACESSOS\n"
assert ancora in s, "ancora nao encontrada"
s = s.replace(ancora, secao + ancora.lstrip('\n').join(['\n','']) if False else secao + ancora[1:], 1)

# 2. pacote: cita a interface
s = s.replace('''    phxsqld       servidor TCP (porta 5000)''',
              '''    phxsqld       servidor TCP (porta 5000) e Centro de Controle web
                  (porta 5001, desligado por padrao)''')

open('MANUAL.txt','w').write(s)
