# Fix MSRV and add users to config example 1
# 27/08 19:06

import subprocess, json, re
# Gera hashes reais para os exemplos
def h(s):
    return subprocess.run(["./target/release/phxsqld","--senha"],input=s.encode(),
                          capture_output=True).stdout.decode().split('"')[3]
raiz=h("troque-esta-senha-do-root")
adr=h("troque-esta-senha")
oper=h("troque-esta-senha")
cons=h("troque-esta-senha")

bloco = f'''
  "_usuarios": [
    "A senha NUNCA vai em texto puro. Gere o hash com:",
    "    echo -n 'a senha' | phxsqld --senha",
    "e cole a linha aqui. Confira o cadastro com: phxsqld --usuarios",
    "",
    "Poder por base: a chave \\"*\\" vale para as bases nao listadas.",
    "Atividade que nao aparece e FALSE. Base que nao aparece e sem \\"*\\" nega tudo.",
    "Supervisor pode tudo em toda base. O root e sempre supervisor."
  ],

  "root": {{
    "id": 1,
    "nome": "Administrador do sistema",
    "login": "root",
    "senha_hash": "{raiz}",
    "email": "root@empresa.com.br",
    "telefone": ""
  }},

  "usuarios": [
    {{
      "id": 2,
      "nome": "Adriano Boller",
      "login": "adriano",
      "senha_hash": "{adr}",
      "email": "adriano@empresa.com.br",
      "telefone": "+55 47 99999-0000",
      "supervisor": true,
      "ativo": true,
      "bases": {{}}
    }},
    {{
      "id": 3,
      "nome": "Maria Operadora",
      "login": "maria",
      "senha_hash": "{oper}",
      "email": "maria@empresa.com.br",
      "telefone": "+55 47 98888-0000",
      "supervisor": false,
      "ativo": true,
      "bases": {{
        "*": {{ "ler": true }},
        "Z": {{
          "ler": true, "inserir": true, "alterar": true, "excluir": false,
          "criar": false, "reindexar": false, "diario": true,
          "verificar": true, "administrar": false, "replicar": false
        }}
      }}
    }},
    {{
      "id": 4,
      "nome": "Carlos Consulta",
      "login": "carlos",
      "senha_hash": "{cons}",
      "email": "carlos@empresa.com.br",
      "telefone": "+55 47 97777-0000",
      "supervisor": false,
      "ativo": true,
      "bases": {{
        "Z": {{ "ler": true, "verificar": true }}
      }}
    }}
  ],
'''

p='exemplos/Config_exemplo_01.json'
s=open(p).read()
s=s.replace('''  "log_acessos": "acessos.log",''', bloco + '''
  "log_acessos": "acessos.log",''')
open(p,'w').write(s)
print("exemplo 01 com cadastro")
