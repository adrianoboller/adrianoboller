# Drop system column from union too
# 28/08 17:34

import io
p='crates/phxsql-server/src/juncao.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    Ok(ResultadoUniao {
        colunas,
        linhas,
        por_parte,
        repetidas,
        truncado,
    })
}'''
novo='''    let mut r = ResultadoUniao {
        colunas,
        linhas,
        por_parte,
        repetidas,
        truncado,
    };
    // Mesma razao da juncao: a coluna de sistema seria falso em toda linha.
    if r.colunas.last().is_some_and(|c| c.nome == COLUNA_SOFTDELETED) {
        r.colunas.pop();
        for linha in &mut r.linhas {
            linha.pop();
        }
    }
    Ok(r)
}'''
assert velho in s
s=s.replace(velho,novo,1)
# import
s=s.replace("use phxsql_core::schema::Schema;","use phxsql_core::schema::{Schema, COLUNA_SOFTDELETED};",1)
io.open(p,'w',encoding='utf-8').write(s)
