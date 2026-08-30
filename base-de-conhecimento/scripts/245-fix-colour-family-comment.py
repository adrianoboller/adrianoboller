# Fix colour family comment
# 28/08 10:35

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()
v = '''  { ico:"tabelas",  rot:"Tabelas",    cor:"var(--reg)",    faz:gerirTabelasAtual },'''
n = '''  { ico:"tabelas",  rot:"Tabelas",    cor:"var(--bin)",    faz:gerirTabelasAtual },'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''/* A cor agrupa por familia, nao por gosto: quinze ferramentas para oito
   matizes, entao a repeticao e inevitavel e deve significar alguma coisa.
     verde   serviço e salvaguarda ... Start/Stop, Backup
     roxo    consulta e réplica ...... Query, Replicação
     azul    gente e mensagem ........ Usuários, Server Mail
     laranja política ................ Diretivas
     turquesa dados .................. Bancos, Duplicar, Importar
     verde-claro rede ................ Conexões
     vermelho risco .................. Transações
     âmbar   conserto e cadeia ....... Repair, Blockchain'''
n = '''/* A cor agrupa por familia, nao por gosto: dezessete ferramentas para oito
   matizes, entao a repeticao e inevitavel e deve significar alguma coisa.
     verde   serviço e salvaguarda ... Start/Stop, Backup
     roxo    consulta e réplica ...... Query, Replicação
     azul    gente e mensagem ........ Usuários, Server Mail
     laranja política ................ Diretivas
     turquesa dados .................. Bancos, View DB, Tabelas, Duplicar, Importar
     verde-claro rede ................ Conexões
     vermelho risco .................. Transações
     âmbar   conserto e cadeia ....... Repair, Blockchain'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
