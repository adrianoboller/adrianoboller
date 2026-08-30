# Commit documentation updates
# 27/08 19:56

s=open('README.md').read()
s = s.replace('''  phxsql-cli/      a ferramenta de linha de comando
  phxsql-store/examples/basico.rs   exemplo executável''',
'''  phxsql-cli/      a ferramenta de linha de comando
  phxsql-server/   config, usuários, blacklist, servidor TCP e o HTTP da
                   interface; ui/index.html é o Centro de Controle
  phxsql-store/examples/basico.rs   exemplo executável''')
open('README.md','w').write(s)
