# Fill the real object at the dispatch site
# 28/08 16:25

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''            ok: resultado.is_ok(),
            duracao_ms: ms,
            erro: resultado.as_ref().err().map(|e| e.to_string()),
            database: String::new(),
            tabela: String::new(),
            codigo: 0,
        });'''
b='''            ok: resultado.is_ok(),
            duracao_ms: ms,
            erro: resultado.as_ref().err().map(|e| e.to_string()),
            // O objeto sai do proprio pedido: e o unico ponto que ve os dois
            // -- a operacao e sobre o que ela foi.
            database: pedido.texto_ou("database", "").to_string(),
            tabela: pedido.texto_ou("tabela", "").to_string(),
            codigo: resultado.as_ref().err().map(|e| e.codigo()).unwrap_or(0),
        });'''
assert a in s, "sitio principal nao encontrado"
s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
