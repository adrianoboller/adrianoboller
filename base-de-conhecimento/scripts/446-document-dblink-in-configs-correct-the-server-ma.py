# Document dblink in configs; correct the Server Mail note
# 28/08 15:01

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
a='''  { ico:"correio",  rot:"Server Mail",cor:"var(--reg)",    faz:null,
    falta:"O armazenamento serve bem — uma tabela de mensagens com pasta "
        + "indexada, corpo no .memo e anexo no .bin. Falta o serviço. E sem "
        + "SMTP ele não troca correio com o mundo: seria um sistema de "
        + "mensagens fechado." },'''
b='''  { ico:"correio",  rot:"Server Mail",cor:"var(--reg)",    faz:null,
    falta:"O armazenamento serve bem — uma tabela de mensagens com pasta "
        + "indexada, corpo no .memo e anexo no .bin. Falta o serviço. O SMTP "
        + "de ENVIO já existe (é o que manda o alerta de disco), mas ele só "
        + "entrega para um relé interno e não sabe RECEBER — sem isso o "
        + "correio é de mão única." },'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
