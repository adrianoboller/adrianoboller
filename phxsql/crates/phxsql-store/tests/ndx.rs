//! Testes da B+tree do `.ndx`, com paginas pequenas para forcar divisoes.

mod comum;

use comum::{DirTemp, Rng};

use phxsql_core::keyenc::{escrever_componente, largura_componente};
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::ndx::NdxFile;

/// Paginas de 512 bytes: com chave Int8 cabem 28 entradas por folha, entao
/// alguns milhares de chaves produzem uma arvore de varios niveis.
const PAGINA_PEQUENA: usize = 512;

fn esquema(unico: bool) -> Schema {
    let idx = IndexDef::new("porChave", vec![IndexColumn::asc(0)]);
    let idx = if unico { idx.unico() } else { idx };
    Schema::new("t", vec![Column::new("k", ColumnType::Int8)], vec![idx]).unwrap()
}

fn chave(v: i64) -> Vec<u8> {
    let ty = ColumnType::Int8;
    let mut buf = vec![0u8; largura_componente(&ty).unwrap()];
    escrever_componente(&Value::Int(v), &ty, false, false, &mut buf).unwrap();
    buf
}

#[test]
fn milhares_de_chaves_em_ordem_embaralhada() {
    let dir = DirTemp::novo("ndx-massa");
    let caminho = dir.0.join("t.ndx");
    let mut n = NdxFile::criar_com_pagina(&caminho, &esquema(true), PAGINA_PEQUENA).unwrap();

    const TOTAL: i64 = 5_000;
    let mut valores: Vec<i64> = (1..=TOTAL).collect();
    Rng::nova(0xC0FFEE).embaralhar(&mut valores);

    for (i, v) in valores.iter().enumerate() {
        n.inserir(0, &chave(*v), i as u64 + 1).unwrap();
    }

    assert_eq!(n.indices()[0].qtd_chaves, TOTAL as u64);
    assert!(n.paginas() > 100, "a arvore deveria ter varias paginas");

    // Toda chave inserida e encontrada, com o rowid certo.
    for (i, v) in valores.iter().enumerate() {
        assert_eq!(
            n.buscar(0, &chave(*v)).unwrap(),
            vec![i as u64 + 1],
            "chave {v}"
        );
    }

    // Chave ausente devolve vazio.
    assert!(n.buscar(0, &chave(TOTAL + 1)).unwrap().is_empty());
    assert!(n.buscar(0, &chave(0)).unwrap().is_empty());

    // A varredura sai na ordem do indice, ou seja, ordenada por valor.
    let rowids = n.varrer(0).unwrap();
    assert_eq!(rowids.len(), TOTAL as usize);
    let esperado: Vec<u64> = {
        let mut pares: Vec<(i64, u64)> = valores
            .iter()
            .enumerate()
            .map(|(i, v)| (*v, i as u64 + 1))
            .collect();
        pares.sort();
        pares.into_iter().map(|(_, r)| r).collect()
    };
    assert_eq!(rowids, esperado);

    n.verificar().unwrap();
}

#[test]
fn insercao_sequencial_crescente_e_decrescente() {
    for crescente in [true, false] {
        let dir = DirTemp::novo("ndx-seq");
        let caminho = dir.0.join("t.ndx");
        let mut n = NdxFile::criar_com_pagina(&caminho, &esquema(true), PAGINA_PEQUENA).unwrap();

        const TOTAL: i64 = 2_000;
        for i in 0..TOTAL {
            let v = if crescente { i } else { TOTAL - 1 - i };
            n.inserir(0, &chave(v), (i + 1) as u64).unwrap();
        }
        assert_eq!(n.varrer(0).unwrap().len(), TOTAL as usize);
        for v in 0..TOTAL {
            assert_eq!(n.buscar(0, &chave(v)).unwrap().len(), 1);
        }
        n.verificar().unwrap();
    }
}

#[test]
fn indice_duplicado_guarda_varios_rowids_em_ordem() {
    let dir = DirTemp::novo("ndx-dup");
    let caminho = dir.0.join("t.ndx");
    let mut n = NdxFile::criar_com_pagina(&caminho, &esquema(false), PAGINA_PEQUENA).unwrap();

    // 100 chaves distintas, 20 rowids cada.
    let mut rowid = 0u64;
    let mut esperado: Vec<(i64, Vec<u64>)> = (0..100).map(|k| (k, Vec::new())).collect();
    let mut ordem: Vec<(i64, u64)> = Vec::new();
    for _ in 0..20 {
        for k in 0..100i64 {
            rowid += 1;
            ordem.push((k, rowid));
        }
    }
    Rng::nova(7).embaralhar(&mut ordem);
    for (k, r) in &ordem {
        n.inserir(0, &chave(*k), *r).unwrap();
        esperado[*k as usize].1.push(*r);
    }

    for (k, mut rowids) in esperado {
        rowids.sort_unstable();
        // O rowid entra no fim da chave em big-endian, entao o resultado ja
        // vem na ordem de digitacao.
        assert_eq!(n.buscar(0, &chave(k)).unwrap(), rowids, "chave {k}");
    }
    assert_eq!(n.indices()[0].qtd_chaves, 2_000);
    n.verificar().unwrap();
}

#[test]
fn indice_unico_recusa_repetida() {
    let dir = DirTemp::novo("ndx-unico");
    let caminho = dir.0.join("t.ndx");
    let mut n = NdxFile::criar_com_pagina(&caminho, &esquema(true), PAGINA_PEQUENA).unwrap();

    n.inserir(0, &chave(42), 1).unwrap();
    let e = n.inserir(0, &chave(42), 2).unwrap_err();
    assert!(matches!(e, phxsql_core::error::PhxError::Duplicado(_)));
    assert_eq!(n.indices()[0].qtd_chaves, 1);
}

#[test]
fn remocao_em_massa() {
    let dir = DirTemp::novo("ndx-rem");
    let caminho = dir.0.join("t.ndx");
    let mut n = NdxFile::criar_com_pagina(&caminho, &esquema(true), PAGINA_PEQUENA).unwrap();

    const TOTAL: i64 = 3_000;
    let mut valores: Vec<i64> = (0..TOTAL).collect();
    Rng::nova(99).embaralhar(&mut valores);
    for v in &valores {
        n.inserir(0, &chave(*v), (*v as u64) + 1).unwrap();
    }

    // Remove os pares.
    for v in (0..TOTAL).step_by(2) {
        assert!(n.remover(0, &chave(v), v as u64 + 1).unwrap());
    }
    // Remover de novo nao acha nada.
    assert!(!n.remover(0, &chave(0), 1).unwrap());

    assert_eq!(n.indices()[0].qtd_chaves, (TOTAL / 2) as u64);
    for v in 0..TOTAL {
        let achado = n.buscar(0, &chave(v)).unwrap();
        if v % 2 == 0 {
            assert!(achado.is_empty(), "chave {v} deveria ter sumido");
        } else {
            assert_eq!(achado, vec![v as u64 + 1]);
        }
    }
    n.verificar().unwrap();

    // Reinserir uma chave removida volta a funcionar.
    n.inserir(0, &chave(0), 1).unwrap();
    assert_eq!(n.buscar(0, &chave(0)).unwrap(), vec![1]);
    n.verificar().unwrap();
}

#[test]
fn intervalo_respeita_os_limites() {
    let dir = DirTemp::novo("ndx-faixa");
    let caminho = dir.0.join("t.ndx");
    let mut n = NdxFile::criar_com_pagina(&caminho, &esquema(true), PAGINA_PEQUENA).unwrap();

    for v in 0..1_000i64 {
        n.inserir(0, &chave(v), v as u64 + 1).unwrap();
    }

    let faixa = n
        .intervalo(0, Some(&chave(100)), Some(&chave(199)))
        .unwrap();
    assert_eq!(faixa.len(), 100);
    assert_eq!(faixa.first(), Some(&101));
    assert_eq!(faixa.last(), Some(&200));

    let so_inicio = n.intervalo(0, Some(&chave(990)), None).unwrap();
    assert_eq!(so_inicio.len(), 10);

    let so_fim = n.intervalo(0, None, Some(&chave(9))).unwrap();
    assert_eq!(so_fim.len(), 10);

    let tudo = n.intervalo(0, None, None).unwrap();
    assert_eq!(tudo.len(), 1_000);

    // Faixa vazia.
    assert!(n
        .intervalo(0, Some(&chave(2_000)), Some(&chave(3_000)))
        .unwrap()
        .is_empty());
}

#[test]
fn reabre_e_continua_inserindo() {
    let dir = DirTemp::novo("ndx-reabre");
    let caminho = dir.0.join("t.ndx");
    {
        let mut n = NdxFile::criar_com_pagina(&caminho, &esquema(true), PAGINA_PEQUENA).unwrap();
        for v in 0..500i64 {
            n.inserir(0, &chave(v), v as u64 + 1).unwrap();
        }
        n.sincronizar().unwrap();
    }
    let mut n = NdxFile::abrir(&caminho).unwrap();
    assert_eq!(n.indices()[0].qtd_chaves, 500);
    assert_eq!(n.page_size(), PAGINA_PEQUENA);
    for v in 500..1_000i64 {
        n.inserir(0, &chave(v), v as u64 + 1).unwrap();
    }
    assert_eq!(n.varrer(0).unwrap().len(), 1_000);
    n.verificar().unwrap();
}

#[test]
fn chave_grande_demais_para_a_pagina_e_recusada() {
    let dir = DirTemp::novo("ndx-chavao");
    let caminho = dir.0.join("t.ndx");
    let esq = Schema::new(
        "t",
        vec![Column::new("s", ColumnType::Str(400))],
        vec![IndexDef::new("i", vec![IndexColumn::asc(0)])],
    )
    .unwrap();
    // Chave de 401+8 bytes nao cabe 4x numa pagina de 512.
    assert!(NdxFile::criar_com_pagina(&caminho, &esq, 512).is_err());
    // Mas cabe numa pagina de 4096.
    assert!(NdxFile::criar_com_pagina(&caminho, &esq, 4096).is_ok());
}

#[test]
fn existe_concorda_com_buscar_em_toda_chave() {
    // `existe` desce uma vez e responde sim/nao sem juntar rowid nenhum. O
    // oraculo e o `buscar`, que ja era testado: se os dois divergirem em uma
    // chave que seja, quem esta errado e o novo.
    //
    // Paginas de 512 bytes forcam a arvore a ter varios niveis, e as chaves
    // pares deixam buracos -- entao a busca cai tanto no meio de uma folha
    // quanto exatamente no fim dela, que e o caso de borda que interessa: a
    // primeira entrada com o prefixo pode estar na folha SEGUINTE.
    let dir = DirTemp::novo("existe-vs-buscar");
    let mut n =
        NdxFile::criar_com_pagina(dir.0.join("t.ndx"), &esquema(false), PAGINA_PEQUENA).unwrap();

    for i in 0..2_000i64 {
        n.inserir(0, &chave(i * 2), (i + 1) as u64).unwrap();
    }
    n.sincronizar().unwrap();

    // Chaves que existem, chaves que nao existem, e as pontas.
    for v in (-4..4_010i64).step_by(1) {
        let pelo_buscar = !n.buscar(0, &chave(v)).unwrap().is_empty();
        let pelo_existe = n.existe(0, &chave(v)).unwrap();
        assert_eq!(
            pelo_existe, pelo_buscar,
            "divergiram na chave {v}: existe={pelo_existe}, buscar={pelo_buscar}"
        );
    }
}

#[test]
fn existe_com_chaves_repetidas() {
    // Num indice comum a mesma chave se repete. `existe` tem de dizer sim uma
    // vez, sem varrer as mil entradas que o `buscar` juntaria.
    let dir = DirTemp::novo("existe-repetida");
    let mut n =
        NdxFile::criar_com_pagina(dir.0.join("t.ndx"), &esquema(false), PAGINA_PEQUENA).unwrap();
    for rowid in 1..=1_000u64 {
        n.inserir(0, &chave(42), rowid).unwrap();
    }
    n.sincronizar().unwrap();

    assert!(n.existe(0, &chave(42)).unwrap());
    assert_eq!(n.buscar(0, &chave(42)).unwrap().len(), 1_000);
    assert!(!n.existe(0, &chave(41)).unwrap());
    assert!(!n.existe(0, &chave(43)).unwrap());
}

#[test]
fn existe_em_indice_vazio() {
    let dir = DirTemp::novo("existe-vazio");
    let mut n =
        NdxFile::criar_com_pagina(dir.0.join("t.ndx"), &esquema(true), PAGINA_PEQUENA).unwrap();
    assert!(!n.existe(0, &chave(1)).unwrap());
}
