# Drop system column from join output
# 28/08 17:34

import io
p='crates/phxsql-server/src/juncao.rs'
s=io.open(p,encoding='utf-8').read()

velho='''    Ok(Resultado {
        colunas: colunas_de(a, b),
        linhas,
        lidas_esquerda,
        lidas_direita,
        chave_nula_esquerda: nula_esq,
        chave_nula_direita: nula_dir,
        truncado,
    })
}'''
novo='''    let mut r = Resultado {
        colunas: colunas_de(a, b),
        linhas,
        lidas_esquerda,
        lidas_direita,
        chave_nula_esquerda: nula_esq,
        chave_nula_direita: nula_dir,
        truncado,
    };
    tirar_a_coluna_de_sistema(&mut r);
    Ok(r)
}

/// Tira a coluna `softdeleted` de cada lado do resultado.
///
/// Uma junção de duas tabelas traria DUAS colunas dela -- `c.softdeleted` e
/// `p.softdeleted` --, e as duas seriam falso em toda linha: a junção só lê
/// linha ativa. Seria ruído em cada resultado, com o agravante de empurrar as
/// colunas úteis para fora da primeira tela da grade.
///
/// Sai aqui, num lugar só, e não no meio da montagem: ali cada linha é
/// montada por posição, e furar a posição no meio é onde nasce campo trocado.
fn tirar_a_coluna_de_sistema(r: &mut Resultado) {
    let sistema = phxsql_core::schema::COLUNA_SOFTDELETED;
    let manter: Vec<bool> = r
        .colunas
        .iter()
        .map(|c| !c.nome.rsplit('.').next().is_some_and(|n| n == sistema))
        .collect();
    if manter.iter().all(|m| *m) {
        return;
    }
    for linha in &mut r.linhas {
        let mut i = 0;
        linha.retain(|_| {
            let fica = manter.get(i).copied().unwrap_or(true);
            i += 1;
            fica
        });
    }
    let mut i = 0;
    r.colunas.retain(|_| {
        let fica = manter[i];
        i += 1;
        fica
    });
}'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
