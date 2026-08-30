# Inspect benchmark results structure
# 28/08 17:16

import json,io
d=json.load(io.open('resultados.json',encoding='utf-8'))
def anda(o,pre=''):
    if isinstance(o,dict):
        for k,v in o.items(): anda(v,pre+'/'+k)
    elif isinstance(o,list):
        print(pre,'[lista de',len(o),']')
        if o and isinstance(o[0],dict): print('   chaves:',list(o[0].keys()))
    else:
        print(pre,'=',o)
anda(d)
