# Extract the format subsections
# 28/08 13:10

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
linhas = p.read_text().split('\n')
i = next(i for i,l in enumerate(linhas) if '<h3>O campo ganhou identidade</h3>' in l)
j = next(j for j,l in enumerate(linhas) if '<h3>O que este motor ainda não faz</h3>' in l)
k = next(k for k,l in enumerate(linhas) if '<h3>O volume aprendeu a cortar pelo calendário</h3>' in l)
print(f'campo+chave: linhas {i}..{k-1} ({k-i} linhas)')
print(f'volume:      linhas {k}..{j-1} ({j-k} linhas)')
# guarda os dois pedacos
pathlib.Path('/tmp/bloco_campo.html').write_text('\n'.join(linhas[i:k]))
pathlib.Path('/tmp/bloco_volume.html').write_text('\n'.join(linhas[k:j]))
# e tira do lugar errado
p.write_text('\n'.join(linhas[:i] + linhas[j:]))
print('retirados de "Estado e roteiro"')
