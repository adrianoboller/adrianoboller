# Testes do contrato novo
# 29/08 06:02

import io
p='crates/phxsql-store/tests/ndx.rs'
s=io.open(p,encoding='utf-8').read()

velho_cab='''/* ------------------------------------------------------ o cache de paginas
As paginas do `.ndx` ficam em RAM para nao pagar `pread` e CRC-32 de pagina
inteira a cada descida da arvore. Ele e de LEITURA: toda gravacao atravessa
para o arquivo na hora.

Os dois testes abaixo travam justamente isso -- que nada fica retido, e que
nenhuma pagina velha e servida no lugar da nova. Um cache que erra em
qualquer um dos dois corrompe indice em silencio. */

/// Nada fica retido em RAM: derrubar o arquivo SEM sincronizar e reabrir tem
/// de encontrar tudo. Se a gravacao passasse a ficar suja no cache, este teste
/// e o primeiro a cair.
#[test]
fn o_cache_nao_segura_gravacao() {'''
novo_cab='''/* ------------------------------------------------------ o cache de paginas
As paginas do `.ndx` ficam em RAM para nao pagar `pread` e CRC-32 de pagina
inteira a cada descida da arvore, e desde a 0.18.0 ele e de ESCRITA tambem: a
pagina fica suja em RAM e o CRC sai no despejo ou no `sincronizar`.

Ate a 0.17.0 o contrato era "nada fica retido", e havia um teste com esse nome
travando-o. Ele nao foi afrouxado -- foi TROCADO pelo contrato novo, que e o
unico que torna a retencao aceitavel: a queda continua sendo possivel, mas
passa a ser DETECTADA. Um `.ndx` que ficou para tras se reconstroi do `.reg`;
um que ficou para tras em silencio nao tem conserto, porque ninguem sabe.

Os testes abaixo travam as tres pernas disso: a queda e detectada, o
`sincronizar` fecha de verdade, e nenhuma pagina velha e servida no lugar da
nova. */

/// Derrubar SEM sincronizar deixa a marca de sujo, e a marca fecha a porta.
///
/// E o teste que substitui o `o_cache_nao_segura_gravacao` da 0.17.0. O que
/// importa nao e mais "achar tudo" -- e **nao responder errado**: toda
/// operacao tem de recusar, com a mensagem que diz o conserto.
#[test]
fn a_queda_sem_sincronizar_e_detectada_e_nao_silenciosa() {'''
assert s.count(velho_cab)==1
s=s.replace(velho_cab,novo_cab)

velho='''    let mut n = NdxFile::abrir(&caminho).unwrap();

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
}'''
novo='''    let mut n = NdxFile::abrir(&caminho).unwrap();
    assert!(
        n.precisa_reconstruir(),
        "a marca de sujo devia ter ficado no cabecalho"
    );

    // Toda operacao recusa, e nenhuma responde meia verdade. A guarda mora no
    // `descritor`, que e por onde todas passam -- espalhada, a que alguem
    // esquecesse viraria a porta dos fundos.
    for (rotulo, e) in [
        ("varrer", n.varrer(0).unwrap_err()),
        ("buscar", n.buscar(0, &chave(1)).unwrap_err()),
        ("existe", n.existe(0, &chave(1)).unwrap_err()),
        ("verificar", n.verificar().unwrap_err()),
        ("inserir", n.inserir(0, &chave(999_999), 1).unwrap_err()),
    ] {
        assert!(
            e.to_string().contains("reparar indice"),
            "{rotulo} devia mandar reconstruir, e disse: {e}"
        );
    }
}

/// Com `sincronizar`, o contrato de sempre continua valendo.
#[test]
fn sincronizar_fecha_o_arquivo_de_verdade() {
    let dir = DirTemp::novo("ndx-sync-fecha");
    let caminho = dir.0.join("t.ndx");
    let quantas = 20_000i64;
    {
        let mut n = NdxFile::criar_com_pagina(&caminho, &esquema(true), PAGINA_PEQUENA).unwrap();
        let mut rng = Rng::nova(7);
        let mut vs: Vec<i64> = (0..quantas).collect();
        rng.embaralhar(&mut vs);
        for v in &vs {
            n.inserir(0, &chave(*v), *v as u64 + 1).unwrap();
        }
        n.sincronizar().unwrap();
    }
    let mut n = NdxFile::abrir(&caminho).unwrap();
    assert!(!n.precisa_reconstruir(), "sincronizar tem de limpar a marca");
    assert_eq!(n.varrer(0).unwrap().len(), quantas as usize);
    for v in [0i64, 1, quantas / 2, quantas - 1] {
        assert_eq!(n.buscar(0, &chave(v)).unwrap(), vec![v as u64 + 1]);
    }
    assert_eq!(n.verificar().unwrap()[0].1, quantas as u64);
    assert_eq!(n.indices()[0].qtd_chaves, quantas as u64);
}

/// A marca so se limpa quando TODAS as paginas ja foram; e por isso que o
/// `sincronizar` grava o cabecalho depois delas, e nao antes.
#[test]
fn a_marca_sai_depois_das_paginas_e_nao_antes() {
    let dir = DirTemp::novo("ndx-ordem");
    let caminho = dir.0.join("t.ndx");
    {
        let mut n = NdxFile::criar_com_pagina(&caminho, &esquema(true), PAGINA_PEQUENA).unwrap();
        for v in 0..5_000i64 {
            n.inserir(0, &chave(v), v as u64 + 1).unwrap();
        }
        // Sem sincronizar: o cabecalho no disco tem de dizer SUJO agora.
        let espiada = NdxFile::abrir(&caminho).unwrap();
        assert!(
            espiada.precisa_reconstruir(),
            "a marca tem de estar no disco ANTES do sincronizar, e nao depois"
        );
        drop(espiada);
        n.sincronizar().unwrap();
    }
    let n = NdxFile::abrir(&caminho).unwrap();
    assert!(!n.precisa_reconstruir());
}'''
assert s.count(velho)==1
s=s.replace(velho,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
