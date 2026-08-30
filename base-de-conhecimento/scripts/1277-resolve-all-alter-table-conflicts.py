# Resolve all ALTER TABLE conflicts
# 30/08 06:42

for cam in ['CHANGELOG.md','crates/phxsql-server/src/conferidor.rs','docs/DESEMPENHO.md','docs/PENDENCIAS.md','provar.py']:
    ls=open(cam,encoding='utf-8').read().split('\n'); out=[];i=0;c=0
    while i<len(ls):
        if ls[i].startswith('<<<<<<<'):
            i+=1;a=[]
            while not ls[i].startswith('======='): a.append(ls[i]);i+=1
            i+=1;z=[]
            while not ls[i].startswith('>>>>>>>'): z.append(ls[i]);i+=1
            i+=1;out.extend(a);out.extend(z);c+=1
        else: out.append(ls[i]);i+=1
    open(cam,'w',encoding='utf-8').write('\n'.join(out))
    print(f"{cam}: {c} conflitos, dois lados")
