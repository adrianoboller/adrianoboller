# Resolve the ratchet keeping one constant
# 30/08 17:41

p='crates/phxsql-server/src/conferidor.rs'
ls=open(p,encoding='utf-8').read().split('\n'); out=[];i=0
while i<len(ls):
    if ls[i].startswith('<<<<<<<'):
        i+=1;a=[]
        while not ls[i].startswith('======='): a.append(ls[i]);i+=1
        i+=1;z=[]
        while not ls[i].startswith('>>>>>>>'): z.append(ls[i]);i+=1
        i+=1
        # Comentario de historia: os dois lados. A CONSTANTE fica uma so, e o
        # valor sai da medicao depois do merge -- 1.961 e da base 1.996, e a
        # minha ja esta em 1.806; escolher um lado seria regressao silenciosa.
        vistos=set(); junto=[]
        for l in a+z:
            if l.startswith('pub const TETO: usize'):
                if 'TETO' in vistos: continue
                vistos.add('TETO')
            junto.append(l)
        out.extend(junto)
    else: out.append(ls[i]); i+=1
open(p,'w',encoding='utf-8').write('\n'.join(out))
