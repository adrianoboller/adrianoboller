# Update README with the new features
# 27/08 21:00

s=open('MANUAL.txt').read()
# a secao 9 aponta para a secao errada agora? confere referencias
s=s.replace('POST /api     o mesmo protocolo da secao 8, um pedido por vez',
            'POST /api     o mesmo protocolo da secao 8, um pedido por vez')
# secao 6: comandos do phxsql ganha backup
s=s.replace('''phxsql tabelas   <base> <database>       lista as tabelas, com schema''',
'''phxsql tabelas   <base> <database>       lista as tabelas, com schema
phxsql backup    <base> <destino>        copia com manifesto SHA-256
phxsql conferir-backup <destino>         le a copia de volta e confere''')
# secao 7: campos novos no config
s=s.replace('''    web               Centro de Controle pelo navegador. Desligado por
                      padrao; ver a secao 9''',
'''    web               Centro de Controle pelo navegador. Desligado por
                      padrao; ver a secao 9
    web.servidores    outros PhxSql que a interface pode alcancar. VAZIO =
                      so este servidor, e esse e o padrao certo
    replicacao.escuta socket onde o source serve os eventos. Ver a secao 16''')
open('MANUAL.txt','w').write(s)
