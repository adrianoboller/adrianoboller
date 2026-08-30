# Verify demo mode as published
# 27/08 19:57

frag=open('artefato/centro-de-controle-phxsql.html').read()
open('demo/index.html','w').write('<!doctype html>\n<html lang="pt-BR">\n<head>\n<meta charset="utf-8">\n</head>\n<body>\n'+frag+'\n</body>\n</html>\n')
