# Fix usage text, format and lint
# 27/08 21:21

p='crates/phxsql-cli/src/main.rs'
s=open(p).read()
s=s.replace('''  phxsql bancos    <base>
  phxsql tabelas   <base> <database>
";''','''  phxsql bancos    <base>
  phxsql tabelas   <base> <database>
  phxsql backup    <base> <destino> [--zip] [--database <n>] [--admin <n>]
  phxsql conferir-backup <destino>
";''')
open(p,'w').write(s)
