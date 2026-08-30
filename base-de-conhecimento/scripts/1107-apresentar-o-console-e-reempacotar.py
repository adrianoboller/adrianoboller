# Apresentar o console e reempacotar
# 29/08 11:08

import io
p='empacotar.sh'
s=io.open(p,encoding='utf-8').read()
# ha duas versoes do COMECE-AQUI (linux e windows); ajusta as duas
velho_w='''       phxsqld.exe    o servidor
       phxsql.exe     a linha de comando (10 comandos; rode sem argumentos)'''
novo_w='''       phxsqld.exe    o servidor
       phxsql.exe     a linha de comando (10 comandos; rode sem argumentos)
       phxsqlcmd.exe  o console interativo: conecta no servidor e /help
                      lista todos os comandos, /help <comando> detalha um'''
if s.count(velho_w)==1:
    s=s.replace(velho_w,novo_w)
    print('windows ok')
velho_l='''       phxsqld    o servidor
       phxsql     a linha de comando (10 comandos; rode sem argumentos)'''
novo_l='''       phxsqld    o servidor
       phxsql     a linha de comando (10 comandos; rode sem argumentos)
       phxsqlcmd  o console interativo: conecta no servidor e /help lista
                  todos os comandos, /help <comando> detalha um'''
if s.count(velho_l)==1:
    s=s.replace(velho_l,novo_l)
    print('linux ok')
io.open(p,'w',encoding='utf-8').write(s)
