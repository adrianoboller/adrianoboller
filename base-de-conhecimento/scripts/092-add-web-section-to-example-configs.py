# Add web section to example configs
# 27/08 19:53

import re

web01 = '''
  "_web": [
    "Centro de Controle: a mesma coisa que a porta 5000 faz, so que pelo",
    "navegador. Sobe DESLIGADO e, quando ligado, escuta so no proprio",
    "computador -- abrir para a rede e decisao de quem administra.",
    "",
    "Porta separada de proposito: quem fala HTTP nao e quem fala JSON Lines,",
    "e separar deixa o firewall tratar cada uma do seu jeito.",
    "",
    "A pagina esta embutida no binario. O servidor nao le arquivo do disco",
    "para servi-la, nao lista diretorio e nao interpreta caminho: das tres",
    "rotas que existem (/, /saude, /api), nenhuma toca o sistema de arquivos.",
    "",
    "sessao_minutos conta a partir do ULTIMO clique, nao do login.",
    "",
    "Em 127.0.0.1 e em https o login usa desafio-resposta e a senha nao sai",
    "da maquina de quem entra. Fora disso o navegador nao oferece a cifra e a",
    "pagina cai em Base64, avisando na tela. Para expor na rede: tunel."
  ],

  "web": {
    "ligado": false,
    "bind": "127.0.0.1:5001",
    "sessao_minutos": 60
  },
'''

web_curto = '''
  "web": {
    "_": "Centro de Controle pelo navegador. Ver Config_exemplo_01.json.",
    "ligado": false,
    "bind": "127.0.0.1:5001",
    "sessao_minutos": 60
  },
'''

def insere(caminho, texto, ancora):
    s = open(caminho).read()
    i = s.index(ancora)
    s = s[:i] + texto.lstrip('\n') + '\n' + s[i:]
    open(caminho,'w').write(s)

insere('exemplos/Config_exemplo_01.json', web01, '  "_seguranca": [')
insere('exemplos/Config_exemplo_02.json', web_curto, '  "replicacao": {')
insere('exemplos/Config_exemplo_03.json', web_curto, '  "replicacao": {')

import json
for n in (1,2,3):
    p=f'exemplos/Config_exemplo_0{n}.json'
    json.load(open(p))
    print(p, 'valido')
