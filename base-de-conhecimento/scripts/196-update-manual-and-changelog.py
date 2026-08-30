# Update manual and changelog
# 27/08 21:23

s=open('MANUAL.txt').read()

# secao 11 (BACKUP) ganha o zip e o agendamento
s=s.replace('''COM O SERVIDOR PARADO

    phxsql backup <base> <destino>
    phxsql conferir-backup <destino>''','''EM UM ARQUIVO ZIP

    O nome sai como  BancoNome_Admin_Data_HoraMin.zip:

        Comercial_adriano_2026-08-27_2114.zip

    Quem fez e quando ficam no PROPRIO NOME -- e assim que se acha o arquivo
    certo numa pasta com trezentos backups, sem abrir nenhum. O manifesto vai
    dentro, entao a copia carrega a propria conferencia.

        phxsql backup <base> <destino> --zip
                      [--database <nome>]   so esse banco
                      [--admin <nome>]      o nome que entra no arquivo

        {"op":"backup","destino":"/backup","zip":true,"database":"Comercial"}

    Medido no cadastro de exemplo: 18.311 bytes viram 2.406 -- 87% menor.
    O .reg e slot de tamanho fixo e o .ndx e pagina com enchimento; e disso
    que a compressao vive.

    Abre com qualquer coisa: unzip, o Explorador do Windows(R), o celular.
    Conferido com unzip -t e extraindo byte a byte.

AGENDADO

    No config.json:

        "backup": {
          "agendado": true,
          "hora": "03:00",
          "destino": "/backup/phxsql",
          "zip": true,
          "database": "",
          "admin": "noturno",
          "manter": 14
        }

    hora        "HH:MM", uma vez por dia. Vazia = usa cada_horas.
    cada_horas  intervalo, quando nao ha hora marcada.
    database    qual copiar. Vazio = todos.
    admin       o nome que entra no arquivo, no lugar de um usuario.
    manter      quantos zips guardar. Zero nao apaga nada.

    VEM DESLIGADO. Backup que roda sozinho num destino que ninguem conferiu e
    backup que enche o disco e para.

    A faxina do "manter" so apaga arquivo com a cara dos nossos: .zip com o
    formato Banco_Admin_Data_HoraMin. O que voce guardou na pasta fica.

    Todo backup agendado entra no acessos.log. Senao, a unica prova de que
    ele rodou seria o arquivo existir.

COM O SERVIDOR PARADO

    phxsql backup <base> <destino>
    phxsql conferir-backup <destino>''')

# secao 14 (USUARIOS) ganha o nivel
s=s.replace('''14. USUARIOS E PERMISSOES
--------------------------------------------------------------------------------''',
'''14. USUARIOS E PERMISSOES
--------------------------------------------------------------------------------
NIVEL: UMA PALAVRA NO LUGAR DE DEZ BOOLEANOS

    "nivel": "operador"

    nenhum    nada. E o PADRAO quando nao se diz nivel -- nega por omissao
    leitor    ler, diario, verificar
    operador  o de cima, mais inserir, alterar e excluir
    dono      o de cima, mais criar, reindexar e replicar
    admin     tudo, inclusive acessos, bloqueios, usuarios e backup

    Cada nivel contem o anterior.

    "supervisor": true e a forma antiga de dizer admin em toda base, e
    continua valendo.

    A regra de uma base ESPECIFICA ganha do nivel, inclusive para TIRAR
    poder. E o que permite dar admin a alguem e ainda assim fechar uma base:

        { "login":"ana", "nivel":"admin",
          "bases": { "Financeiro": {} } }      ana nao entra no Financeiro

    Confira sempre com:  phxsqld --config config.json --usuarios
''')
open('MANUAL.txt','w').write(s)
