# Corrupt the .reg and verify the second chance
# 27/08 21:48

import socket, json, os
S="/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/espelho/srv"
reg=f"{S}/dados/Comercial/cadastroClientes.reg"
bkp=f"{S}/dados/Comercial/cadastroClientes.bkp"
print("reg e bkp tem o mesmo tamanho:", os.path.getsize(reg)==os.path.getsize(bkp))

def conecta():
    s=socket.create_connection(('127.0.0.1',5500),5); f=s.makefile('rwb')
    def pede(d):
        f.write((json.dumps(d)+'\n').encode()); f.flush(); return json.loads(f.readline().decode())
    pede({"token":"tk","op":"login","usuario":"root","senha":"x"})
    return pede

D={"token":"tk","database":"Comercial","tabela":"cadastroClientes"}
p=conecta()
antes=p({**D,"op":"ler","rowid":3})["resultado"]
print("3. registro 3 antes :", antes["nome"])

# vira 40 bytes do meio do .reg -- so no principal
d=bytearray(open(reg,'rb').read())
for i in range(400, 440): d[i]^=0xff
open(reg,'wb').write(d)
print("4. .reg corrompido de proposito (40 bytes virados)")

p=conecta()
r=p({**D,"op":"ler","rowid":3})
print("5. leitura DEPOIS  :", "OK -> "+r["resultado"]["nome"] if r["ok"] else "FALHOU: "+r["erro"][:70])
print("   veio do espelho? o dado bate:", r["ok"] and r["resultado"]==antes)

rep=p({**D,"op":"reparar"})
print("6. reparar         :", json.dumps(rep.get("resultado", rep)))
d2=open(reg,'rb').read()
print("7. o .reg foi reescrito com a copia boa:", d2[400:440]!=bytes(d[400:440]))
