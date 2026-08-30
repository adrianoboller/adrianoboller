# Update README with the new features
# 27/08 21:00

s=open('README.md').read()
s=s.replace('''O motor de armazenamento está completo e testado: **214 testes**,''',
            '''O motor de armazenamento está completo e testado: **254 testes**,''')
s=s.replace('''| Centro de Controle — interface web embutida no `phxsqld` | pronto |''',
'''| Centro de Controle — interface web embutida no `phxsqld` | pronto |
| Tabela em memória e `SelectMemory` — 87× mais rápido, medido | pronto |
| Chave assimétrica Ed25519 (RFC 8032) como segundo fator | pronto |
| Backup com manifesto SHA-256 e comando que confere | pronto |
| Tema claro e escuro, console para mais de um servidor | pronto |''')
s=s.replace('''| Replicação — `.log` v2 com imagem da linha | desenhada |''',
            '''| Replicação — `.log` v2 com imagem da linha | desenhada |''')
s=s.replace('''  docs/
  FORMATO.md       especificação byte a byte dos arquivos''',
'''  docs/
  FORMATO.md       especificação byte a byte dos arquivos''')
s=s.replace('''MANUAL.txt         manual do operador
```''','''MANUAL.txt         manual do operador
CHANGELOG.md       o que mudou em cada versão, defeitos primeiro
```''')
open('README.md','w').write(s)
