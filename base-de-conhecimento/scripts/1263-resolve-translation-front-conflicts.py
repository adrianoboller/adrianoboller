# Resolve translation-front conflicts
# 30/08 06:32

for cam in ['crates/phxsql-server/src/idiomas.rs','CHANGELOG.md']:
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
    print(f"{cam}: {c} conflitos, dois lados guardados")
