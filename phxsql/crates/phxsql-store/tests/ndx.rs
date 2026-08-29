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

/* ------------------------------------------------------ o cache de paginas
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
fn o_cache_nao_segura_gravacao() {
    let dir = DirTemp::novo("ndx-cache-atravessa");
    let caminho = dir.0.join("t.ndx");
    // Chaves de sobra para encher varias vezes o teto do cache e forcar
    // despejo no meio da carga.
    let quantas = 20_000i64;
    {
        let mut n = NdxFile::criar_com_pagina(&caminho, &esquema(true), PAGINA_PEQUENA).unwrap();
        let mut rng = Rng::nova(7);
        let mut vs: Vec<i64> = (0..quantas).collect();
        rng.embaralhar(&mut vs);
        for v in &vs {
            n.inserir(0, &chave(*v), *v as u64 + 1).unwrap();
        }
        // De proposito SEM `sincronizar`: o que se quer provar e que o `write`
        // ja aconteceu, e nao que o `fsync` salvou o dia.
    }
    let mut n = NdxFile::abrir(&caminho).unwrap();
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
}

/// Pagina velha nunca no lugar da nova: depois de remover e inserir de novo,
/// a busca tem de ver o estado de agora -- e o mesmo estado que um arquivo
/// reaberto do zero enxerga.
#[test]
fn o_cache_nao_serve_pagina_velha() {
    let dir = DirTemp::novo("ndx-cache-velha");
    let caminho = dir.0.join("t.ndx");
    let quantas = 6_000i64;

    let mut n = NdxFile::criar_com_pagina(&caminho, &esquema(true), PAGINA_PEQUENA).unwrap();
    for v in 0..quantas {
        n.inserir(0, &chave(v), v as u64 + 1).unwrap();
    }
    // Tira as pares e devolve as multiplas de 4, com rowid novo. As folhas
    // envolvidas sao reescritas varias vezes, que e onde um cache errado
    // devolveria o conteudo de antes.
    for v in (0..quantas).step_by(2) {
        n.remover(0, &chave(v), v as u64 + 1).unwrap();
    }
    for v in (0..quantas).step_by(4) {
        n.inserir(0, &chave(v), v as u64 + 10_000).unwrap();
    }
    n.sincronizar().unwrap();

    let esperado = |v: i64| -> Vec<u64> {
        if v % 4 == 0 {
            vec![v as u64 + 10_000]
        } else if v % 2 == 0 {
            vec![]
        } else {
            vec![v as u64 + 1]
        }
    };
    for v in 0..quantas {
        assert_eq!(n.buscar(0, &chave(v)).unwrap(), esperado(v), "chave {v}");
    }
    n.verificar().unwrap();

    // E o mesmo que um arquivo sem cache nenhum enxerga.
    drop(n);
    let mut fresco = NdxFile::abrir(&caminho).unwrap();
    for v in 0..quantas {
        assert_eq!(
            fresco.buscar(0, &chave(v)).unwrap(),
            esperado(v),
            "chave {v} depois de reabrir"
        );
    }
}

// ------------------------------------------------------- construcao em lote

/// Junta as chaves de um lote no buffer plano que `construir_em_lote` espera.
fn lote(pares: &[(i64, u64)]) -> Vec<u8> {
    let mut b = Vec::new();
    for (v, rowid) in pares {
        b.extend_from_slice(&NdxFile::chave_completa(&chave(*v), *rowid));
    }
    b
}

#[test]
fn lote_monta_a_mesma_arvore_que_inserir_uma_a_uma() {
    let dir = DirTemp::novo("ndx-lote-igual");

    const TOTAL: i64 = 5_000;
    let mut valores: Vec<i64> = (1..=TOTAL).collect();
    Rng::nova(0xB0A7).embaralhar(&mut valores);
    let pares: Vec<(i64, u64)> = valores
        .iter()
        .enumerate()
        .map(|(i, v)| (*v, i as u64 + 1))
        .collect();

    // Uma a uma, o caminho de sempre.
    let mut uma =
        NdxFile::criar_com_pagina(dir.0.join("uma.ndx"), &esquema(true), PAGINA_PEQUENA).unwrap();
    for (v, rowid) in &pares {
        uma.inserir(0, &chave(*v), *rowid).unwrap();
    }

    // De uma vez.
    let mut em_lote =
        NdxFile::criar_com_pagina(dir.0.join("lote.ndx"), &esquema(true), PAGINA_PEQUENA).unwrap();
    em_lote.construir_em_lote(0, lote(&pares)).unwrap();

    // O que importa nao e a arvore ser byte a byte igual -- ela nao e, e nem
    // deveria -- e sim as duas responderem a mesma coisa.
    assert_eq!(uma.verificar().unwrap(), em_lote.verificar().unwrap());
    assert_eq!(uma.varrer(0).unwrap(), em_lote.varrer(0).unwrap());
    for (v, rowid) in &pares {
        assert_eq!(em_lote.buscar(0, &chave(*v)).unwrap(), vec![*rowid]);
    }
    // Quantas paginas cada uma gasta e assunto do `ENCHIMENTO_PADRAO`, e esta
    // medido em `--example indice-em-lote` -- nao se afirma aqui, porque
    // insercao aleatoria ja assenta perto de 69% de ocupacao sozinha e a
    // comparacao depende do numero escolhido.
}

#[test]
fn depois_do_lote_a_insercao_continua_funcionando() {
    let dir = DirTemp::novo("ndx-lote-depois");
    let mut n =
        NdxFile::criar_com_pagina(dir.0.join("t.ndx"), &esquema(true), PAGINA_PEQUENA).unwrap();

    // Numeros pares, para os impares caberem no meio e forcarem divisao.
    let pares: Vec<(i64, u64)> = (1..=2_000).map(|i| (i * 2, i as u64)).collect();
    n.construir_em_lote(0, lote(&pares)).unwrap();

    for i in 1..=2_000i64 {
        n.inserir(0, &chave(i * 2 - 1), 2_000 + i as u64).unwrap();
    }
    assert_eq!(n.verificar().unwrap()[0].1, 4_000);
    assert_eq!(n.buscar(0, &chave(1)).unwrap(), vec![2_001]);
    assert_eq!(n.buscar(0, &chave(4_000)).unwrap(), vec![2_000]);
}

#[test]
fn lote_encadeia_as_folhas_nos_dois_sentidos() {
    let dir = DirTemp::novo("ndx-lote-cadeia");
    let mut n =
        NdxFile::criar_com_pagina(dir.0.join("t.ndx"), &esquema(false), PAGINA_PEQUENA).unwrap();
    let pares: Vec<(i64, u64)> = (1..=3_000).map(|i| (i, i as u64)).collect();
    n.construir_em_lote(0, lote(&pares)).unwrap();

    // `verificar` anda a cadeia de folhas pelo `prox` e confere a ordem; se o
    // encadeamento tivesse buraco, ela acharia menos chaves do que o diretorio
    // diz e recusaria.
    assert_eq!(n.verificar().unwrap()[0].1, 3_000);
    assert_eq!(n.varrer(0).unwrap().len(), 3_000);
    // O intervalo desce e depois anda de folha em folha: pega os dois sentidos.
    assert_eq!(
        n.intervalo(0, Some(&chave(10)), Some(&chave(20)))
            .unwrap()
            .len(),
        11
    );
}

#[test]
fn lote_recusa_indice_que_ja_tem_chave() {
    let dir = DirTemp::novo("ndx-lote-povoado");
    let mut n =
        NdxFile::criar_com_pagina(dir.0.join("t.ndx"), &esquema(true), PAGINA_PEQUENA).unwrap();
    n.inserir(0, &chave(1), 1).unwrap();

    let e = n.construir_em_lote(0, lote(&[(2, 2)])).unwrap_err();
    assert!(
        e.to_string().contains("exige indice vazio"),
        "recusa errada: {e}"
    );
    // E a chave que estava la continua la: a recusa nao mexeu em nada.
    assert_eq!(n.buscar(0, &chave(1)).unwrap(), vec![1]);
}

#[test]
fn lote_recusa_a_mesma_chave_completa_duas_vezes() {
    let dir = DirTemp::novo("ndx-lote-cc");
    let mut n =
        NdxFile::criar_com_pagina(dir.0.join("t.ndx"), &esquema(false), PAGINA_PEQUENA).unwrap();
    let e = n
        .construir_em_lote(0, lote(&[(7, 1), (9, 2), (7, 1)]))
        .unwrap_err();
    assert!(e.to_string().contains("duas vezes"), "recusa errada: {e}");
}

#[test]
fn lote_em_indice_unico_recusa_chave_repetida() {
    let dir = DirTemp::novo("ndx-lote-unico");
    let mut n =
        NdxFile::criar_com_pagina(dir.0.join("t.ndx"), &esquema(true), PAGINA_PEQUENA).unwrap();
    // Rowids diferentes: a chave COMPLETA nao repete, so a do usuario.
    let e = n.construir_em_lote(0, lote(&[(7, 1), (7, 2)])).unwrap_err();
    assert!(e.to_string().contains("repetida"), "recusa errada: {e}");
}

#[test]
fn lote_em_indice_nao_unico_aceita_chave_repetida() {
    let dir = DirTemp::novo("ndx-lote-repete");
    let mut n =
        NdxFile::criar_com_pagina(dir.0.join("t.ndx"), &esquema(false), PAGINA_PEQUENA).unwrap();
    let pares: Vec<(i64, u64)> = (1..=500u64).map(|r| (7, r)).collect();
    n.construir_em_lote(0, lote(&pares)).unwrap();
    // O rowid no fim da chave e o que torna a ordem total, e ele sai em ordem.
    assert_eq!(
        n.buscar(0, &chave(7)).unwrap(),
        (1..=500u64).collect::<Vec<_>>()
    );
}

#[test]
fn lote_vazio_deixa_a_arvore_vazia_e_utilizavel() {
    let dir = DirTemp::novo("ndx-lote-zero");
    let mut n =
        NdxFile::criar_com_pagina(dir.0.join("t.ndx"), &esquema(true), PAGINA_PEQUENA).unwrap();
    n.construir_em_lote(0, Vec::new()).unwrap();
    assert_eq!(n.verificar().unwrap()[0].1, 0);
    n.inserir(0, &chave(1), 1).unwrap();
    assert_eq!(n.buscar(0, &chave(1)).unwrap(), vec![1]);
}

#[test]
fn lote_recusa_buffer_que_nao_fecha_na_chave() {
    let dir = DirTemp::novo("ndx-lote-torto");
    let mut n =
        NdxFile::criar_com_pagina(dir.0.join("t.ndx"), &esquema(true), PAGINA_PEQUENA).unwrap();
    let mut b = lote(&[(1, 1)]);
    b.pop(); // um byte a menos: o buffer deixa de ser multiplo da chave
    let e = n.construir_em_lote(0, b).unwrap_err();
    assert!(e.to_string().contains("multiplo"), "recusa errada: {e}");
}

#[test]
fn lote_sobrevive_a_reabrir_o_arquivo() {
    let dir = DirTemp::novo("ndx-lote-reabre");
    let caminho = dir.0.join("t.ndx");
    let pares: Vec<(i64, u64)> = (1..=3_000).map(|i| (i, i as u64)).collect();
    {
        let mut n = NdxFile::criar_com_pagina(&caminho, &esquema(true), PAGINA_PEQUENA).unwrap();
        n.construir_em_lote(0, lote(&pares)).unwrap();
        n.sincronizar().unwrap();
    }
    // A raiz e a contagem tem de ter ido para o cabecalho, e nao so para a RAM.
    let mut n = NdxFile::abrir(&caminho).unwrap();
    assert_eq!(n.verificar().unwrap()[0].1, 3_000);
    assert_eq!(n.buscar(0, &chave(1_500)).unwrap(), vec![1_500]);
}
