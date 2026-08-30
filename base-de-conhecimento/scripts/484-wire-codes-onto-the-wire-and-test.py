# Wire codes onto the wire and test
# 28/08 16:23

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''            Err(e) => vec![
                ("ok", Json::Bool(false)),
                ("op", Json::texto_de(&op)),
                ("erro", Json::texto_de(e.to_string())),
                ("ms", Json::de_u64(ms)),
            ],'''
b='''            // O codigo vem JUNTO com o texto, e nao no lugar dele: o texto e
            // para quem le, o codigo e para quem programa. Trocar um pelo
            // outro obrigaria alguem a perder.
            Err(e) => vec![
                ("ok", Json::Bool(false)),
                ("op", Json::texto_de(&op)),
                ("erro", Json::texto_de(e.to_string())),
                ("codigo", Json::de_u64(e.codigo() as u64)),
                ("nome", Json::texto_de(e.nome())),
                ("classe", Json::texto_de(e.classe())),
                ("repetir", Json::Bool(e.adianta_repetir())),
                ("ms", Json::de_u64(ms)),
            ],'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
