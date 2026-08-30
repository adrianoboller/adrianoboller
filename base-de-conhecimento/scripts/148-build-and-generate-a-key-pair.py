# Build and generate a key pair
# 27/08 20:44

p='crates/phxsql-server/src/main.rs'
s=open(p).read()
s=s.replace('''//! phxsqld --senha [senha]          gera a linha senha_hash para o config.json''',
'''//! phxsqld --senha [senha]          gera a linha senha_hash para o config.json
//! phxsqld --gerar-chave             gera um par de chaves Ed25519''')
s=s.replace('''  phxsqld --senha [senha]           gera a linha senha_hash para o config.json''',
'''  phxsqld --senha [senha]           gera a linha senha_hash para o config.json
  phxsqld --gerar-chave             gera um par de chaves Ed25519 (2o fator)''')
open(p,'w').write(s)
