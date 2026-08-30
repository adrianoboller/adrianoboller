# Add backup and level sections to example configs
# 27/08 21:20

import json, re, pathlib
p = pathlib.Path('exemplos/Config_exemplo_01.json')
s = p.read_text()

backup = '''
  "_backup": [
    "Backup agendado. Vem DESLIGADO: backup que roda sozinho num destino que",
    "ninguem conferiu e backup que enche o disco e para.",
    "",
    "hora: \\"HH:MM\\" roda uma vez por dia nesse horario.",
    "cada_horas: usado so quando nao ha hora marcada.",
    "zip: um arquivo Banco_Admin_Data_HoraMin.zip, com o manifesto dentro.",
    "database: qual copiar. Vazio = todos.",
    "admin: o nome que entra no arquivo, no lugar de um usuario.",
    "manter: quantos zips guardar. Zero nao apaga nada. So apaga arquivo com",
    "        a cara dos nossos -- o que voce guardou na pasta fica.",
    "",
    "Conferir sempre:  phxsql conferir-backup <destino>",
    "O comando sai com erro quando nao bate, entao cabe numa linha de cron."
  ],

  "backup": {
    "agendado": false,
    "hora": "03:00",
    "cada_horas": 24,
    "destino": "backups",
    "zip": true,
    "database": "",
    "admin": "agendado",
    "manter": 14
  },
'''
s = s.replace('  "_web": [', backup.lstrip('\n') + '\n  "_web": [')

nivel = '''
  "_niveis": [
    "nivel resolve o caso comum com uma palavra, em vez de dez booleanos:",
    "",
    "  nenhum    nada. E o padrao quando nao se diz nivel -- nega por omissao",
    "  leitor    ler, diario, verificar",
    "  operador  o de cima, mais inserir, alterar e excluir",
    "  dono      o de cima, mais criar, reindexar e replicar",
    "  admin     tudo, inclusive acessos, bloqueios, usuarios e backup",
    "",
    "Cada nivel contem o anterior. supervisor:true e a forma antiga de dizer",
    "admin em toda base, e continua valendo.",
    "",
    "A regra de uma base especifica GANHA do nivel -- inclusive para TIRAR",
    "poder. E o que permite dar admin a alguem e ainda assim fechar uma base."
  ],
'''
s = s.replace('  "root": {', nivel.lstrip('\n') + '\n  "root": {')
p.write_text(s)
json.load(open(p))
print('exemplo 01 ok')

for n in (2, 3):
    q = pathlib.Path(f'exemplos/Config_exemplo_0{n}.json')
    t = q.read_text()
    t = t.replace('''  "replicacao": {''', '''  "backup": {
    "_": "Backup agendado. Ver Config_exemplo_01.json.",
    "agendado": false,
    "hora": "03:00",
    "destino": "backups",
    "zip": true,
    "admin": "agendado",
    "manter": 14
  },

  "replicacao": {''', 1)
    q.write_text(t)
    json.load(open(q))
    print(f'exemplo 0{n} ok')
