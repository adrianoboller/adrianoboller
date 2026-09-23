//! A cifra do `.fts` -- a PAGINA selada, provada em disco (pedido 340).
//!
//! # Por que isto e um teste de INTEGRACAO, e num arquivo so dele
//!
//! Pela mesma razao de `cifra-dos-dados.rs`: a chave e do PROCESSO, e
//! `cargo test` roda os testes de um mesmo binario em paralelo. Ligar a cifra
//! dentro da biblioteca faria a tabela de outro teste nascer cifrada no meio
//! da corrida. Aqui o processo e so deste arquivo -- e mesmo assim os testes
//! passam pela trava, porque tambem dividem o processo entre si.
//!
//! # O que se prova, e o que NAO se prova
//!
//! Prova-se que o termo da coluna MARCADA nao aparece nos bytes do `.fts`, e
//! que a busca por palavra continua achando exatamente o que achava. NAO se
//! prova sigilo contra quem tem a senha: o modelo de ameaca esta no
//! `cofre.rs` -- disco levado, backup vazado, copia numa maquina que nao e
//! esta.

mod comum;
use std::path::Path;
use std::sync::Mutex;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, IndiceDeTexto, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;
use phxsql_store::cofre;
use phxsql_store::table::Table;

static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

/// Iteracoes no piso: o que se prova aqui e a amarracao, nao o custo do
/// PBKDF2 -- que ja tem vetor proprio em `phxsql_core::hash`.
const RAPIDO: u32 = cofre::ITERACOES_MINIMAS;
const SENHA: &str = "a chave do cofre de teste";

/// Um segredo bem reconhecivel, e ele vai DOBRADO no indice: minusculas e sem
/// acento. E por isso que a agulha da busca nos bytes e a forma dobrada, e
/// nao a que se digita -- procurar a grafia crua acharia `nao` no `.fts` de
/// um arquivo vazando, e o teste passaria por engano.
const SEGREDO: &str = "fulanodetalzinho";

fn dir(rotulo: &str) -> comum::DirTemp {
    comum::DirTemp::novo(&format!("cifra-fts-{rotulo}"))
}

/// Tabela com indice de texto sobre coluna MARCADA -- o caso do pedido 340.
fn esquema_marcado() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .expect("esquema")
    .com_indices_de_texto(vec![IndiceDeTexto::new("porNome", 1)])
    .expect("indice de texto")
}

/// A MESMA tabela, com a coluna NAO marcada. E o controle do alcance: quem
/// nao declarou dado pessoal nao paga nada e nao muda de formato.
fn esquema_sem_marca() -> Schema {
    Schema::new(
        "publicos",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .expect("esquema")
    .com_indices_de_texto(vec![IndiceDeTexto::new("porNome", 1)])
    .expect("indice de texto")
}

fn linha(i: i64) -> Vec<Value> {
    vec![Value::Int(i), Value::Str(format!("{SEGREDO}{i:04}"))]
}

fn bytes_da(d: &Path, nome: &str, ext: &str) -> Vec<u8> {
    std::fs::read(d.join(format!("{nome}.{ext}"))).expect("arquivo da tabela")
}

fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
    !agulha.is_empty() && palheiro.windows(agulha.len()).any(|j| j == agulha)
}

/// A versao declarada nos bytes 8..10 do cabecalho.
fn versao(d: &Path, nome: &str, ext: &str) -> u16 {
    let b = bytes_da(d, nome, ext);
    u16::from_le_bytes([b[8], b[9]])
}

/// Enche a tabela e devolve o diretorio, ja sincronizado.
fn encher(d: &Path, esquema: Schema, quantas: i64) {
    let mut t = Table::criar(d, esquema).expect("criar");
    for i in 1..=quantas {
        t.inserir(&linha(i)).expect("inserir");
    }
    t.sincronizar().expect("sincronizar");
}

/// **A prova do pedido 340.** O valor da coluna marcada NAO aparece em claro
/// dentro do `.fts`.
///
/// # O vermelho
///
/// Antes da selagem da pagina, `strings -n 10 clientes.fts` devolvia
/// `fulanodetalzinho0001`, `...0002`, `...0003` -- com o `.memo` ao lado
/// devolvendo nada. O `fts.rs` tinha ZERO ocorrencias de `cifra`, `cofre`,
/// `Material` e `selar`: nao havia caminho pelo qual ele PUDESSE cifrar.
///
/// O controle positivo esta no mesmo teste e e o que o faz valer: o `.reg`
/// da MESMA tabela e conferido primeiro. Sonda que nao acha o conhecido nao
/// vale para o desconhecido.
#[test]
fn o_fts_de_coluna_marcada_nao_guarda_o_termo_em_claro() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("nao-vaza");
    cofre::definir(SENHA, RAPIDO).unwrap();
    encher(&d, esquema_marcado(), 40);

    // Controle positivo: o `.reg` da mesma tabela esta cifrado. Se ele
    // vazasse, o veredito sobre o `.fts` nao valeria nada.
    assert!(
        !contem(&bytes_da(&d, "clientes", "reg"), SEGREDO.as_bytes()),
        "o controle positivo caiu: o .reg vazou, e ai o veredito do .fts nao prova nada"
    );

    let fts = bytes_da(&d, "clientes", "fts");
    assert!(
        !contem(&fts, SEGREDO.as_bytes()),
        "o termo da coluna marcada apareceu EM CLARO dentro do .fts"
    );
    // E o cabecalho declara a versao com pagina selada.
    assert_eq!(versao(&d, "clientes", "fts"), 2);

    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// Selada, a busca textual continua achando exatamente o que achava.
///
/// E a metade que a saida (d) comprou e que as saidas (a) e (b) do pedido
/// nao compravam: cifrar o ARMAZENAMENTO nao toca na chave dentro da pagina,
/// entao a ordem da B+tree sobrevive inteira.
#[test]
fn selada_a_busca_textual_acha_o_mesmo() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("busca");
    cofre::definir(SENHA, RAPIDO).unwrap();
    encher(&d, esquema_marcado(), 300);

    // Fecha e REABRE: a chave sai do cabecalho do arquivo, nunca da memoria.
    let mut t = Table::abrir(&d, "clientes").unwrap();
    assert!(t.fts_selado(), "a tabela tinha de estar com o .fts selado");
    for i in [1i64, 7, 123, 300] {
        let achado = t
            .procurar_texto("porNome", &format!("{SEGREDO}{i:04}"))
            .unwrap();
        assert_eq!(
            achado.rowids,
            vec![i as u64],
            "a busca por {SEGREDO}{i:04} tinha de achar a linha {i}"
        );
    }
    assert!(
        t.procurar_texto("porNome", "inexistente")
            .unwrap()
            .rowids
            .is_empty(),
        "a busca selada nao pode achar a MAIS"
    );

    // Alterar e excluir continuam acompanhando o indice.
    t.atualizar(7, &[Value::Int(7), Value::Str(format!("{SEGREDO}0777"))])
        .unwrap();
    assert!(
        t.procurar_texto("porNome", &format!("{SEGREDO}0007"))
            .unwrap()
            .rowids
            .is_empty(),
        "o termo velho continuou no indice depois da alteracao"
    );
    assert_eq!(
        t.procurar_texto("porNome", &format!("{SEGREDO}0777"))
            .unwrap()
            .rowids,
        vec![7]
    );
    t.excluir(123).unwrap();
    assert!(
        t.procurar_texto("porNome", &format!("{SEGREDO}0123"))
            .unwrap()
            .rowids
            .is_empty(),
        "o termo da linha excluida continuou no indice"
    );

    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// **O comportamento VELHO, onde ele ainda vale.** Tabela sem coluna marcada
/// nao muda de formato nem paga selo -- mesmo com o cofre ligado.
///
/// E o teste que mais importa numa guarda nova: guarda que muda o formato de
/// quem nao pediu nada e estrago, nao protecao. Aqui o `.fts` continua na
/// versao 1 e continua guardando o termo, que e a verdade sobre uma coluna
/// que ninguem declarou como dado pessoal.
#[test]
fn sem_coluna_marcada_o_fts_nao_muda_nada() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("sem-marca");
    cofre::definir(SENHA, RAPIDO).unwrap();
    encher(&d, esquema_sem_marca(), 40);

    assert_eq!(
        versao(&d, "publicos", "fts"),
        1,
        "tabela sem coluna marcada nao pode mudar de versao de formato"
    );
    assert!(
        contem(&bytes_da(&d, "publicos", "fts"), SEGREDO.as_bytes()),
        "o .fts de coluna NAO marcada continua guardando o termo -- se isso mudou \
         de proposito, apague esta linha do SEGURANCA.md §11.3 junto com este teste"
    );

    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// Ligar a cifra depois nao sela o `.fts` que ja existe -- e `reindexar` sela.
///
/// E a mesma lei do §11.6 (`Ligar a cifra nao cifra o que ja existe`), com a
/// diferenca que so o `.fts` tem: ele e DERIVADO, entao a saida existe e e
/// barata. Quem esperasse a selagem sozinha na abertura ganharia uma
/// varredura de tabela inteira que nao pediu.
#[test]
fn o_fts_nascido_em_claro_so_sela_no_reindexar() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("nasceu-claro");
    // Cofre DESLIGADO: nasce tudo em claro, inclusive o `.reg`.
    encher(&d, esquema_marcado(), 40);
    assert_eq!(versao(&d, "clientes", "fts"), 1);

    // Liga a cifra e reabre: o `.fts` velho continua abrindo e continua em
    // claro. "Continua mesmo" e a resposta certa, e nao um conserto.
    cofre::definir(SENHA, RAPIDO).unwrap();
    {
        let mut t = Table::abrir(&d, "clientes").unwrap();
        assert!(
            !t.fts_selado(),
            "a cifra ligada depois nao sela o que ja existe"
        );
        assert_eq!(
            t.procurar_texto("porNome", &format!("{SEGREDO}0001"))
                .unwrap()
                .rowids,
            vec![1]
        );
        // E agora o conserto escrito: refazer o indice.
        let feitas = t.reconstruir_fts().unwrap();
        assert_eq!(feitas, 40);
        t.sincronizar().unwrap();
        assert!(t.fts_selado(), "o reindexar tinha de fazer nascer selado");
    }
    assert_eq!(versao(&d, "clientes", "fts"), 2);
    assert!(
        !contem(&bytes_da(&d, "clientes", "fts"), SEGREDO.as_bytes()),
        "depois do reindexar o termo nao pode continuar no disco"
    );
    // O `.reg` continua em claro: ele nasceu antes, e nao ha recifragem.
    assert!(contem(&bytes_da(&d, "clientes", "reg"), SEGREDO.as_bytes()));

    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// Sem a chave, o `.fts` selado RECUSA -- e nao cai na vala do "refaz".
///
/// # O defeito que esta guarda impede
///
/// A abertura da tabela trata `.fts` que nao abre refazendo-o do `.reg`, e
/// isso e certo para arquivo corrompido ou divergente. Com a selagem, uma
/// senha errada no `config.json` passaria por aquela mesma vala e entregaria
/// um `.fts` NOVO e em claro, com os termos da coluna marcada de volta ao
/// disco -- desfazendo a protecao calado, por causa de um erro de digitacao.
#[test]
fn sem_a_chave_o_fts_selado_recusa_em_vez_de_refazer_em_claro() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("sem-chave");
    cofre::definir(SENHA, RAPIDO).unwrap();
    encher(&d, esquema_marcado(), 20);

    // O `.reg` desta tabela e cifrado tambem, entao para chegar ao `.fts` sem
    // chave e preciso o caso real: `.reg` em claro e `.fts` selado. E o que
    // acontece com quem ligou a cifra depois e rodou `reindexar`.
    let em_claro = dir("sem-chave-misto");
    cofre::desligar();
    encher(&em_claro, esquema_marcado(), 20);
    cofre::definir(SENHA, RAPIDO).unwrap();
    {
        let mut t = Table::abrir(&em_claro, "clientes").unwrap();
        t.reconstruir_fts().unwrap();
        t.sincronizar().unwrap();
    }
    assert_eq!(versao(&em_claro, "clientes", "fts"), 2);

    // Agora sem a chave: tem de RECUSAR nomeando, e o arquivo tem de
    // continuar selado no disco.
    cofre::desligar();
    let e = match Table::abrir(&em_claro, "clientes") {
        Err(e) => e.to_string(),
        Ok(_) => panic!("abrir sem a chave tinha de recusar"),
    };
    assert!(
        e.contains("cifra"),
        "a recusa tem de nomear a cifra, e nao mandar procurar arquivo: {e}"
    );
    assert_eq!(
        versao(&em_claro, "clientes", "fts"),
        2,
        "a tentativa sem chave REFEZ o .fts em claro -- e exatamente o que nao pode"
    );

    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&em_claro);
}

/// Duas gravacoes da mesma pagina nao repetem o par (chave, nonce).
///
/// # Por que isto e teste e nao comentario
///
/// Porque e a unica falha que quebra uma cifra de fluxo sem quebrar a
/// matematica dela: com o mesmo nonce e a mesma chave, quem tem as duas
/// copias do arquivo tem o XOR dos dois conteudos. O endereco da pagina
/// sozinho se repetiria em toda gravacao -- sao os 8 bytes sorteados que
/// seguram isto, e este teste e o que impede alguem "simplificar" e tira-los.
#[test]
fn regravar_a_mesma_pagina_muda_os_bytes_cifrados() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("nonce");
    cofre::definir(SENHA, RAPIDO).unwrap();
    encher(&d, esquema_marcado(), 5);
    // O arquivo tem de estar selado para este teste querer dizer alguma
    // coisa: em claro os bytes tambem mudariam -- pela reorganizacao da
    // folha --, e o teste passaria por engano, provando nada sobre o nonce.
    assert_eq!(versao(&d, "clientes", "fts"), 2);
    let antes = bytes_da(&d, "clientes", "fts");

    // Uma alteracao que devolve o indice ao conteudo EXATO de antes: o termo
    // sai e volta. A pagina em claro e a mesma; os bytes no disco nao podem
    // ser.
    {
        let mut t = Table::abrir(&d, "clientes").unwrap();
        t.atualizar(3, &[Value::Int(3), Value::Str("outracoisa".into())])
            .unwrap();
        t.atualizar(3, &linha(3)).unwrap();
        t.sincronizar().unwrap();
    }
    let depois = bytes_da(&d, "clientes", "fts");
    assert_eq!(
        antes.len(),
        depois.len(),
        "o arquivo nao devia mudar de tamanho"
    );
    assert_ne!(
        antes, depois,
        "regravar repetiu os bytes cifrados: o tempero sorteado sumiu do caminho"
    );

    // E, mesmo assim, o indice continua respondendo o mesmo.
    let mut t = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(
        t.procurar_texto("porNome", &format!("{SEGREDO}0003"))
            .unwrap()
            .rowids,
        vec![3]
    );

    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}

/// Um bit trocado no corpo cifrado da pagina e RECUSADO, e nao lido.
///
/// O CRC-32 sozinho nao daria esta garantia: ele acha bit trocado, e nao
/// impede troca deliberada. A etiqueta Poly1305 impede as duas.
#[test]
fn bit_trocado_na_pagina_selada_e_recusado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("bit");
    cofre::definir(SENHA, RAPIDO).unwrap();
    encher(&d, esquema_marcado(), 40);

    // Byte no meio da pagina 1 -- a raiz da arvore, que toda busca desce.
    let caminho = d.join("clientes.fts");
    let mut bytes = std::fs::read(&caminho).unwrap();
    let alvo = 4096 + 512;
    bytes[alvo] ^= 0x01;
    std::fs::write(&caminho, &bytes).unwrap();

    let mut t = Table::abrir(&d, "clientes").unwrap();
    let e = match t.procurar_texto("porNome", &format!("{SEGREDO}0001")) {
        Err(e) => e.to_string(),
        Ok(a) => panic!("a busca devolveu {a:?} sobre pagina adulterada"),
    };
    // "etiqueta", e nao "ou CRC": num `.fts` em claro o CRC-32 tambem pegaria
    // este bit, e aceitar as duas mensagens faria o teste passar sem selo
    // nenhum -- provando o que ja existia em vez do que entrou.
    assert!(
        e.contains("etiqueta"),
        "a recusa tem de vir da etiqueta da pagina selada: {e}"
    );

    cofre::desligar();
    let _ = std::fs::remove_dir_all(&d);
}
