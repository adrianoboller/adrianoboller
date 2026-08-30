# Update README and commit user system
# 27/08 19:10

p='README.md'
s=open(p).read()
s=s.replace('''O motor de armazenamento está completo e testado: **104 testes**''','''O motor de armazenamento está completo e testado: **166 testes**''')
s=s.replace('''| `config.json` e servidor TCP na porta 5000 | pendente |''','''| `config.json` e servidor TCP na porta 5000 | pronto |
| Log de acessos por IP, com data e hora | pronto |
| Cadastro de usuários, senha em hash, permissão por base | pronto |''')
s=s.replace('''  FORMATO.md       especificação byte a byte dos arquivos
  PLANO.md         leitura do rusqlite e do FraseSQL, e o roteiro do projeto
```''','''  FORMATO.md       especificação byte a byte dos arquivos
  USUARIOS.md      cadastro, senha em hash e as dez permissões
  REPLICACAO.md    o desenho da replicação Source → Réplica
  PLANO.md         leitura do rusqlite e do FraseSQL, e o roteiro do projeto
exemplos/
  Config_exemplo_0N.json   isolado, source e réplica
MANUAL.txt         manual do operador
```''')
s=s.replace('''**Operação recusada não vira evento.** O `.log` registra o que aconteceu, não o
que foi tentado: chave duplicada, tabela cheia ou coluna obrigatória em branco
falham sem sujar o diário.''','''**Operação recusada não vira evento.** O `.log` registra o que aconteceu, não o
que foi tentado: chave duplicada, tabela cheia ou coluna obrigatória em branco
falham sem sujar o diário.

**A senha nunca fica em texto puro.** O `config.json` guarda
PBKDF2-HMAC-SHA256 com 210.000 iterações — SHA-256, HMAC e PBKDF2 escritos aqui
para não quebrar a regra de zero dependências, e conferidos contra os vetores
oficiais (FIPS 180-4, RFC 4231). Gere o hash com
`echo -n 'a senha' | phxsqld --senha`.

**Cadastrar usuários só aperta a segurança.** Sem cadastro, o token dá poder
total, como antes. Com cadastro, o token vira só a chave da porta da rede e o
login passa a ser exigido — nunca o contrário.''')
open(p,'w').write(s)
