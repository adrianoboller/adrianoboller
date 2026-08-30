# Finish the rename and restart
# 27/08 20:50

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
s=s.replace('                // destinos que ela pode alcancar e se ha chave a informar.',
            '                // servidores que ela pode alcancar e se ha chave a informar.')
s=s.replace('                            "destinos",','                            "servidores",')
s=s.replace('                                    .destinos','                                    .servidores')
open(p,'w').write(s)
