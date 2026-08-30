# Corrigir os scripts e regenerar os numeros da bancada
# 29/08 03:12

import io
for p in ('docs/dossie/numeros-do-projeto.py','docs/dossie/numeros-da-bancada.py'):
    s=io.open(p,encoding='utf-8').read()
    velho='''            return pathlib.Path(a)'''
    novo='''            # Resolvido: caminho relativo quebrava o `relative_to(RAIZ)`
            # da mensagem final, DEPOIS de ja ter gravado o arquivo.
            return pathlib.Path(a).resolve()'''
    assert s.count(velho)==1, p
    io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
    print('ok',p)
