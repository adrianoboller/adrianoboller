# Document dblink in configs; correct the Server Mail note
# 28/08 15:01

import json
bloco = '''
  "_dblink": [
    "Arquivo com as ligacoes para bancos de fora (DbLink). Padrao dblink.json.",
    "",
    "Separado deste arquivo de proposito: o cadastro muda pela tela, e",
    "reescrever o config.json inteiro a cada ligacao nova arriscaria os",
    "comentarios e o resto da configuracao.",
    "",
    "Toda operacao de DbLink exige administrar, e uma ligacao nasce",
    "somente-leitura. Nao ha TLS: a conversa com o outro banco e em texto",
    "claro (a SENHA nao -- ela vai embaralhada com um sal por conexao).",
    "",
    "Detalhes em docs/DBLINK.md."
  ],
  "dblink": "dblink.json",
'''
for n in ("01","02","03"):
    p=f'exemplos/Config_exemplo_{n}.json'
    s=open(p).read()
    if '"_dblink"' in s: continue
    marca='\n  "_alertas": ['
    assert marca in s, p
    s=s.replace(marca, bloco+marca,1)
    open(p,'w').write(s)
    json.loads(s)
    print(p,'ok')
