# Resolve the code conflict and recount raw data locks
# 30/08 06:28

p='crates/phxsql-server/src/servidor.rs'
linhas=open(p,encoding='utf-8').read().split('\n')
saida=[];i=0;n=0
while i<len(linhas):
    if linhas[i].startswith('<<<<<<<'):
        i+=1; meu=[]
        while not linhas[i].startswith('======='): meu.append(linhas[i]); i+=1
        i+=1; dele=[]
        while not linhas[i].startswith('>>>>>>>'): dele.append(linhas[i]); i+=1
        i+=1
        # O lado de HEAD aqui e a cauda da assinatura ANTIGA, que o git alinhou
        # contra a funcao nova. A antiga esta inteira mais abaixo, com o
        # `hash_dele` e o `Result<u64>` no lugar dela.
        saida.extend(dele); n+=1
    else:
        saida.append(linhas[i]); i+=1
open(p,'w',encoding='utf-8').write('\n'.join(saida))
print(f"{n} conflito resolvido pelo lado da frente da trava")
