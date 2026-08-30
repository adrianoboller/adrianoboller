# Disambiguate the two Juncao names
# 28/08 15:28

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
# `Juncao` ja e o nome do lookup do pivot: o tipo da juncao entra como TipoJuncao
s=s.replace('use crate::juncao::{Lado, Tipo as Juncao, Uniao};',
            'use crate::juncao::{Lado, Tipo as TipoJuncao, Uniao};',1)
s=s.replace('let tipo = Juncao::de_texto(p.texto_ou("tipo", "interna"))?;',
            'let tipo = TipoJuncao::de_texto(p.texto_ou("tipo", "interna"))?;',1)
open(p,'w').write(s)
