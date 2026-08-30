# Write security docs and update config example
# 27/08 19:31

import json
p='exemplos/Config_exemplo_01.json'
s=open(p).read()
bloco = '''
  "_seguranca": [
    "comandos_proibidos e bases_proibidas valem para TODO MUNDO, root incluso:",
    "e o que ninguem pede por esta porta. Pedir bloqueia o IP na hora.",
    "",
    "Token errado, senha errada e IP fora da lista sao tentativas LEVES: contam",
    "dentro da janela e bloqueiam ao passar do limite.",
    "",
    "O bloqueio SEMPRE vale dentro do servidor. A regra de firewall e um extra,",
    "desligada por padrao. Quando ligada, o comando vem inteiro daqui como lista",
    "de argumentos e roda SEM SHELL, com o IP validado como endereco antes de",
    "entrar no lugar do {ip}.",
    "",
    "Confira e desfaca com: phxsqld --bloqueios / --desbloquear <ip>"
  ],

  "seguranca": {
    "comandos_proibidos": [],
    "bases_proibidas": [],

    "tentativas_ate_bloquear": 5,
    "janela_minutos": 10,
    "bloqueio_minutos": 60,

    "blacklist": "blacklist.json",

    "firewall": {
      "ligado": false,
      "bloquear": ["/usr/sbin/iptables", "-I", "INPUT", "-s", "{ip}", "-j", "DROP"],
      "desbloquear": ["/usr/sbin/iptables", "-D", "INPUT", "-s", "{ip}", "-j", "DROP"]
    }
  },
'''
s=s.replace('\n  "_usuarios": [', bloco + '\n  "_usuarios": [')
open(p,'w').write(s)
json.load(open(p)); print("exemplo 01: secao de seguranca, JSON valido")
