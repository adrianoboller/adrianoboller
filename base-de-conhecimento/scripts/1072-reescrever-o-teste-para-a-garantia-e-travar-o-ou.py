# Reescrever o teste para a garantia e travar o outro sentido
# 29/08 05:19

import io
p='crates/phxsql-store/tests/ndx.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    let mut n = NdxFile::abrir(&caminho).unwrap();
    assert_eq!(n.indices()[0].qtd_chaves, quantas as u64);
    assert_eq!(n.varrer(0).unwrap().len(), quantas as usize);
    for v in [0i64, 1, quantas / 2, quantas - 1] {
        assert_eq!(
            n.buscar(0, &chave(v)).unwrap(),
            vec![v as u64 + 1],
            "chave {v} sumiu"
        );
    }
    n.verificar().unwrap();
}'''
novo='''    let mut n = NdxFile::abrir(&caminho).unwrap();

    // A GARANTIA: a arvore esta inteira no arquivo, sem `sincronizar` nenhum.
    assert_eq!(n.varrer(0).unwrap().len(), quantas as usize);
    for v in [0i64, 1, quantas / 2, quantas - 1] {
        assert_eq!(
            n.buscar(0, &chave(v)).unwrap(),
            vec![v as u64 + 1],
            "chave {v} sumiu"
        );
    }

    // O CONTADOR, nao. Ele nao vai ao disco por chave -- seriam 4 KiB de
    // cabecalho por chave, por indice, so para adiantar um numero que a
    // varredura sabe recalcular. Sem `sincronizar`, ele fica para tras, e
    // `verificar` e quem o reconcilia.
    assert!(
        n.indices()[0].qtd_chaves <= quantas as u64,
        "o contador nunca pode passar do que existe"
    );
    assert_eq!(n.verificar().unwrap()[0].1, quantas as u64);
    assert_eq!(
        n.indices()[0].qtd_chaves,
        quantas as u64,
        "depois de verificar, o contador tem de estar certo"
    );
}

/// Varredura MENOR que o contador continua sendo corrupcao.
///
/// A reconciliacao do teste acima so vale para UM lado. Contador atrasado e o
/// rastro normal de uma queda entre dois `sincronizar`; chave que sumiu da
/// arvore nao e, e tem de continuar parando a conferencia. Confundir os dois
/// faria `verificar` calar justamente no caso que ela existe para pegar.
#[test]
fn contador_maior_que_a_arvore_continua_sendo_corrupcao() {
    let dir = DirTemp::novo("ndx-contador-alto");
    let caminho = dir.0.join("t.ndx");
    let mut n = NdxFile::criar_com_pagina(&caminho, &esquema(true), PAGINA_PEQUENA).unwrap();
    for v in 0..500i64 {
        n.inserir(0, &chave(v), v as u64 + 1).unwrap();
    }
    n.sincronizar().unwrap();
    assert_eq!(n.verificar().unwrap()[0].1, 500);

    // Tira uma chave da arvore por baixo, deixando o contador onde estava.
    assert!(n.remover(0, &chave(250), 251).unwrap());
    n.forjar_contador_para_teste(0, 500);

    let e = n.verificar().unwrap_err();
    assert!(
        e.to_string().contains("achou so"),
        "devia acusar chave faltando, e disse: {e}"
    );
}'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')
