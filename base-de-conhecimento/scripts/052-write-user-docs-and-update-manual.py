# Write user docs and update manual
# 27/08 19:09

import re
p='MANUAL.txt'
s=open(p).read()
# Renumera 10..13 para 11..14, de tras para frente
for velho, novo in [(13,14),(12,13),(11,12),(10,11)]:
    s=re.sub(rf'^{velho}\. ', f'{novo}. ', s, flags=re.M)
novo_bloco = '''10. USUARIOS E PERMISSOES
--------------------------------------------------------------------------------
O cadastro fica no config.json, com nome completo, login, senha, email,
telefone, supervisor e o poder sobre cada base. A SENHA E GUARDADA COMO HASH.

10.1 Gerar o hash

    echo -n 'a senha de verdade' | phxsqld --senha
    "senha_hash": "pbkdf2-sha256$210000$7570c880...$becbc17c..."

    Use o cano, e nao o argumento: assim a senha nao fica no historico do
    shell nem aparece no ps. Senha em texto puro no campo "senha" funciona,
    mas o servidor avisa alto no arranque.

10.2 O cadastro

    "root": { "id":1, "nome":"...", "login":"root", "senha_hash":"...",
              "email":"...", "telefone":"" },

    "usuarios": [{
      "id": 3, "nome": "Maria Operadora", "login": "maria",
      "senha_hash": "pbkdf2-sha256$...",
      "email": "maria@empresa.com.br", "telefone": "+55 47 98888-0000",
      "supervisor": false, "ativo": true,
      "bases": {
        "*": { "ler": true },
        "Z": { "ler":true, "inserir":true, "alterar":true, "excluir":false,
               "criar":false, "reindexar":false, "diario":true,
               "verificar":true, "administrar":false, "replicar":false }
      }
    }]

    id          vai para o .log da tabela como autor da operacao
    supervisor  pode tudo, em toda base
    ativo       false bloqueia o login sem apagar o cadastro
    bases       o poder base por base; "*" vale para as nao listadas

    O root e SEMPRE supervisor e sempre ativo.

10.3 As dez atividades

    ler          bancos, tabelas, esquema, ler, varrer, buscar
    inserir      inserir
    alterar      atualizar
    excluir      excluir
    criar        criar_database, criar_schema
    reindexar    reindexar
    diario       diario
    verificar    verificar
    administrar  acessos, ips, config, usuarios
    replicar     posicao, replicar

    NEGA POR OMISSAO: atividade que nao aparece vale false. Base listada
    manda (o "*" nao completa o que faltou). Sem a base e sem "*", nega tudo.

10.4 Os tres portoes

    pedido -> token -> login -> permissao -> executa
              (rede)  (identidade) (poder)

    O token continua sendo exigido em todo pedido: ele e a chave da porta da
    rede, nao a identidade. Havendo cadastro, e preciso fazer login antes de
    qualquer operacao:

        {"token":"...","op":"login","usuario":"maria","senha":"..."}

    SEM CADASTRO NENHUM, o token continua dando poder total -- o
    comportamento anterior. Cadastrar usuarios so aperta, nunca afrouxa.

    A autenticacao acontece UMA VEZ POR CONEXAO. PBKDF2 custa ~100 ms de
    proposito: irrelevante uma vez, inviavel a cada pedido.

10.5 Conferir

    phxsqld --usuarios

    login    nome                      supervisor ativo  poder por base
    root     Administrador do sistema  sim        sim    (supervisor: tudo)
    maria    Maria Operadora           nao        sim    *=ler  Z=ler+inserir+...

    Pelo protocolo: {"op":"usuarios"} -- nunca devolve senha nem hash.

    O login aparece nos dois registros:
        acessos.log  "op":"inserir","usuario":"carlos","ok":false
        Tabela.log   ... inclusao  rowid 6  versao 1  usuario 3

    Detalhes em docs/USUARIOS.md.


'''
s=s.replace('11. SEGURANCA\n', novo_bloco + '11. SEGURANCA\n')
s=s.replace('''  phxsqld --exemplo <1|2|3>         imprime um config.json de exemplo''','''  phxsqld --exemplo <1|2|3>         imprime um config.json de exemplo''')
open(p,'w').write(s)
print("MANUAL.txt: secao 10 inserida e as demais renumeradas")
