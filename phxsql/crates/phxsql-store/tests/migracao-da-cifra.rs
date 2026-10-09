//! **Pedido 268: a migracao `Criptografar`/`Descriptografar`**, provada em
//! disco.
//!
//! Operacao PEDIDA (nunca por declaracao) sobre colunas INLINE marcadas, pelo
//! MESMO motor de troca do 632: o `*.novo` de cada volume nasce completo e
//! sincronizado ao lado, e so depois vem o `rename`, volume 1 primeiro. Sem
//! byte novo no formato: o estado e o byte de versao do volume (4 claro, 5
//! cifrado).
//!
//! # Por que e teste de INTEGRACAO
//!
//! O cofre e do PROCESSO (ver `cifra-dos-dados.rs`): ligar a cifra dentro da
//! biblioteca faria a tabela de outro teste nascer cifrada no meio da corrida.
//! Aqui os testes se revezam por uma trava e a queda e de verdade -- panico de
//! teste no ponto exato (`ndx::panico_de_teste`), nao arquivo montado a mao.

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use std::path::Path;
use std::sync::Mutex;

use comum::DirTemp;

use phxsql_core::error::PhxError;
use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, IndiceDeTexto, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;
use phxsql_store::cofre;
use phxsql_store::ndx::panico_de_teste::{armar, desarmar, Ponto};
use phxsql_store::reg::RegFile;
use phxsql_store::table::{Table, Visao};
use phxsql_store::RecusaDaMigracao;

static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

const RAPIDO: u32 = cofre::ITERACOES_MINIMAS;
const SENHA: &str = "a chave do cofre de teste";
const SEGREDO: &str = "Fulano de Tal da Silva";
const POR_VOLUME: u64 = 30;

fn esquema(paginada: bool) -> Schema {
    let e = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("cpf", ColumnType::Str(14)).com_dado_pessoal(DadoPessoal::Sensivel),
            Column::new("cidade", ColumnType::Str(20)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap();
    if paginada {
        e.com_paginacao(Paginacao::nova(POR_VOLUME, 9).unwrap())
            .unwrap()
    } else {
        e
    }
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("{SEGREDO} {i:04}")),
        Value::Str(format!("{i:03}.456.789-00")),
        Value::Str("Blumenau".into()),
    ]
}

/// Uma tabela EM CLARO (v4) com `n` linhas e as colunas marcadas: o cofre
/// esta desligado ao nascer, e ligado depois -- o caso do dono que marcou a
/// coluna antes de ter cofre. Devolve com o cofre LIGADO.
fn tabela_em_claro(rotulo: &str, paginada: bool, n: i64) -> DirTemp {
    cofre::desligar();
    let d = DirTemp::novo(&format!("migra-{rotulo}"));
    let mut t = Table::criar(&d, esquema(paginada)).unwrap();
    for i in 1..=n {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    drop(t);
    assert_eq!(versao(&d), 4, "a tabela de partida nasce em claro");
    cofre::definir(SENHA, RAPIDO).unwrap();
    d
}

fn versao_do(arquivo: &Path) -> u16 {
    let b = std::fs::read(arquivo).unwrap();
    u16::from_le_bytes([b[8], b[9]])
}

/// A versao do volume 1.
fn versao(d: &Path) -> u16 {
    versao_do(&volumes(d)[0])
}

/// Os volumes do `.reg`, em ordem.
fn volumes(d: &Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(d)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let f = p.file_name().unwrap().to_string_lossy();
            f.starts_with("clientes") && f.ends_with(".reg")
        })
        .collect();
    v.sort();
    v
}

fn novos(d: &Path) -> Vec<String> {
    std::fs::read_dir(d)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| f.ends_with(".novo"))
        .collect()
}

fn todo_o_reg(d: &Path) -> Vec<u8> {
    volumes(d)
        .iter()
        .flat_map(|p| std::fs::read(p).unwrap())
        .collect()
}

fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
    palheiro.windows(agulha.len()).any(|j| j == agulha)
}

/// O sal do volume 1 (bytes 136..152 do cabecalho da v5).
fn sal_do(d: &Path) -> [u8; 16] {
    let b = std::fs::read(d.join("clientes.reg")).unwrap();
    b[136..152].try_into().unwrap()
}

/// O payload de cada slot, ABERTO pelo motor: o que a linha guarda, byte a
/// byte, sistema incluso (`rowstamp`/`rowtime`).
fn payloads(d: &Path, n: u64) -> Vec<Option<Vec<u8>>> {
    let mut r = RegFile::abrir(d, "clientes").unwrap();
    (1..=n).map(|i| r.ler(i).unwrap()).collect()
}

fn conferir_linhas(d: &Path, n: i64) {
    let mut t = Table::abrir(d, "clientes").unwrap_or_else(|e| panic!("nao abre: {e}"));
    let linhas = t.varrer_com(Visao::Todas).unwrap();
    assert_eq!(linhas.len() as i64, n, "perdeu ou ganhou linha");
    for (i, (rowid, v)) in linhas.iter().enumerate() {
        assert_eq!(*rowid, i as u64 + 1);
        assert_eq!(
            v[1],
            Value::Str(format!("{SEGREDO} {:04}", i + 1)),
            "o rowid {rowid} nao e mais a mesma linha"
        );
        assert_eq!(v[2], Value::Str(format!("{:03}.456.789-00", i + 1)));
    }
}

/// `Criptografar` ou `Descriptografar` inteiro, as duas fases juntas.
fn migrar(d: &Path, cifrar: bool) -> u64 {
    let mut t = Table::abrir(d, "clientes").unwrap();
    let troca = t.preparar_migracao_da_cifra(cifrar).unwrap();
    t.aplicar_migracao_da_cifra(troca).unwrap()
}

// ---------------------------------------------------------------------------
// (9) round-trip, byte a byte
// ---------------------------------------------------------------------------

/// Criptografar -> Descriptografar devolve o payload byte a byte, com o
/// `rowstamp`/`rowtime` (que moram no payload) intactos -- e no meio o segredo
/// SAIU do disco.
///
/// # Prova real
///
/// Com o `transformar` selando com o material VELHO (em claro), o segredo
/// continua legivel na versao 5 e a segunda asserção reprova.
#[test]
fn round_trip_devolve_o_payload_byte_a_byte() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    for paginada in [false, true] {
        let d = tabela_em_claro("roundtrip", paginada, 75);
        let antes = payloads(&d, 75);
        assert!(contem(&todo_o_reg(&d), SEGREDO.as_bytes()));

        let slots = migrar(&d, true);
        assert_eq!(slots, 75, "o laco passa por TODO slot, vivo ou nao");
        assert_eq!(versao(&d), 5);
        assert!(
            !contem(&todo_o_reg(&d), SEGREDO.as_bytes()),
            "o segredo continua legivel depois de Criptografar"
        );
        assert!(novos(&d).is_empty());
        assert_eq!(payloads(&d, 75), antes, "Criptografar mexeu no payload");
        conferir_linhas(&d, 75);
        for v in volumes(&d) {
            assert_eq!(
                versao_do(&v),
                5,
                "volume {} ficou na versao velha",
                v.display()
            );
        }

        migrar(&d, false);
        assert_eq!(versao(&d), 4);
        assert!(contem(&todo_o_reg(&d), SEGREDO.as_bytes()));
        assert_eq!(payloads(&d, 75), antes, "o round-trip nao e byte a byte");
        conferir_linhas(&d, 75);
        assert!(novos(&d).is_empty());
        cofre::desligar();
    }
}

// ---------------------------------------------------------------------------
// (6) sal novo a cada Criptografar
// ---------------------------------------------------------------------------

/// Cada `Criptografar` sorteia sal NOVO: reaproveitar o da cifragem anterior
/// repetiria chave e nonce entre a tabela decifrada e a que volta a ser
/// cifrada.
///
/// # Prova real
///
/// Guardando o material em vez de sortear (`Material::novo()` trocado por uma
/// copia do velho), o terceiro `assert_ne!` reprova.
#[test]
fn cada_criptografar_sorteia_sal_novo() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = tabela_em_claro("sal", false, 10);
    migrar(&d, true);
    let primeiro = sal_do(&d);
    assert_ne!(primeiro, [0u8; 16], "sal zerado");
    migrar(&d, false);
    migrar(&d, true);
    let segundo = sal_do(&d);
    assert_ne!(primeiro, segundo, "o sal repetiu entre duas cifragens");

    // E a tabela nova com o sal novo abre e le.
    conferir_linhas(&d, 10);
    cofre::desligar();
}

// ---------------------------------------------------------------------------
// (4) slot excluido continua excluido; rowid e ordem de digitacao intactos
// ---------------------------------------------------------------------------

/// O slot excluido (de vez ou suave) continua excluido, o rowid das outras
/// linhas nao anda e NENHUM slot e reaproveitado: a linha nova depois da
/// migracao e a `slots + 1`.
///
/// # Prova real
///
/// Compactando os buracos na passagem (pulando o slot livre no `transformar`),
/// o rowid 6 passa a ser a linha 7 e a asserção de `conferir_linhas` reprova.
#[test]
fn slot_excluido_continua_excluido_e_o_rowid_nao_anda() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    for sentido in [true, false] {
        // `sentido`: parte do claro (Criptografar) ou do cifrado (Descriptografar).
        let d = tabela_em_claro("excluido", true, 70);
        {
            let mut t = Table::abrir(&d, "clientes").unwrap();
            assert!(t.excluir_de_vez(5, "teste").unwrap());
            assert!(t.excluir_suave(40, "teste").unwrap());
            t.sincronizar().unwrap();
        }
        if !sentido {
            migrar(&d, true);
        }
        let antes = payloads(&d, 70);
        assert!(antes[4].is_none(), "o slot 5 devia estar livre");
        let registros_antes = Table::abrir(&d, "clientes").unwrap().registros();
        migrar(&d, sentido);
        assert_eq!(payloads(&d, 70), antes);

        let mut t = Table::abrir(&d, "clientes").unwrap();
        assert!(t.ler(5).unwrap().is_none(), "o excluido de vez voltou");
        assert_eq!(t.slots(), 70);
        assert_eq!(t.registros(), registros_antes, "a contagem de vivos andou");
        let todas = t.varrer_com(Visao::Todas).unwrap();
        let rowids: Vec<u64> = todas.iter().map(|(r, _)| *r).collect();
        assert!(!rowids.contains(&5));
        assert!(rowids.contains(&6) && rowids.contains(&40));
        assert_eq!(
            todas.iter().find(|(r, _)| *r == 6).unwrap().1[1],
            Value::Str(format!("{SEGREDO} 0006")),
            "o rowid 6 deixou de ser a linha 6"
        );
        let excluidas = t.varrer_com(Visao::Excluidas).unwrap();
        assert_eq!(excluidas.len(), 1, "o excluido suave perdeu a marca");
        assert_eq!(excluidas[0].0, 40);
        // Nenhum slot reaproveitado: a linha nova e a 71.
        assert_eq!(t.inserir(&linha(71)).unwrap(), 71);
        cofre::desligar();
    }
}

// ---------------------------------------------------------------------------
// (1) queda depois da FASE A e antes do rename do volume 1
// ---------------------------------------------------------------------------

fn morrer_antes_do_rename(d: &Path, cifrar: bool, ponto: Ponto) {
    let mut t = Table::abrir(d, "clientes").unwrap();
    let troca = t.preparar_migracao_da_cifra(cifrar).unwrap();
    armar(ponto);
    let morreu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = t.aplicar_migracao_da_cifra(troca);
    }));
    desarmar();
    assert!(
        morreu.is_err(),
        "o panico armado em {ponto:?} nao aconteceu"
    );
}

/// A queda DEPOIS da FASE A e ANTES do primeiro `rename`, nos dois sentidos,
/// em tabela de um volume e em paginada de tres: a tabela abre VELHA e
/// INTEIRA, e a sobra e recolhida pela abertura gravavel.
///
/// # Prova real
///
/// Se a FASE A trocasse o volume 1 logo depois de escreve-lo (o defeito do
/// 632), a tabela abriria na versao nova com os volumes 2 e 3 faltando -- e a
/// asserção da versao velha reprova.
#[test]
fn queda_antes_do_rename_do_volume_1_abre_velha_e_inteira() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    for cifrar in [true, false] {
        for paginada in [false, true] {
            let n = if paginada { 75 } else { 40 };
            let d = tabela_em_claro("queda-antes", paginada, n);
            if !cifrar {
                migrar(&d, true);
            }
            let versao_de_partida = versao(&d);
            morrer_antes_do_rename(&d, cifrar, Ponto::FaseBAntesDaPrimeiraTroca);
            assert!(
                !novos(&d).is_empty(),
                "a queda nao deixou os *.novo (cifrar={cifrar}, paginada={paginada})"
            );
            assert_eq!(
                versao(&d),
                versao_de_partida,
                "a tabela mudou de versao antes do compromisso"
            );
            for v in volumes(&d) {
                assert_eq!(versao_do(&v), versao_de_partida);
            }
            conferir_linhas(&d, n); // a abertura gravavel recolhe a sobra
            assert!(
                novos(&d).is_empty(),
                "a sobra da FASE A nao foi recolhida (cifrar={cifrar}, paginada={paginada})"
            );
            // E a migracao refeita depois da queda funciona.
            migrar(&d, cifrar);
            assert_eq!(versao(&d), if cifrar { 5 } else { 4 });
            conferir_linhas(&d, n);
            cofre::desligar();
        }
    }
}

// ---------------------------------------------------------------------------
// (2) queda depois do rename do volume 1 e antes do 2
// ---------------------------------------------------------------------------

/// O volume 1 e o ponto de compromisso: dele para a frente a migracao esta
/// decidida, e a ABERTURA termina para a frente. Todas as linhas batem e todo
/// volume acaba na versao nova -- nos dois sentidos.
///
/// # Prova real
///
/// Sem a versao na geometria o conjunto "volume 1 novo, 2 e 3 velhos" ainda
/// se distingue pelo `slot_size`; o que este teste pega e a abertura que NAO
/// termina (`terminar_troca_interrompida` sem a troca decidida): a asserção da
/// versao do volume 3 reprova e a tabela nem abre ("pela metade").
#[test]
fn queda_depois_do_rename_do_volume_1_termina_para_a_frente() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    for cifrar in [true, false] {
        let d = tabela_em_claro("queda-depois", true, 75);
        if !cifrar {
            migrar(&d, true);
        }
        let alvo = if cifrar { 5 } else { 4 };
        morrer_antes_do_rename(&d, cifrar, Ponto::FaseBDepoisDoVolume1);
        let vols = volumes(&d);
        assert_eq!(vols.len(), 3);
        assert_eq!(versao_do(&vols[0]), alvo, "o volume 1 devia ter trocado");
        assert_ne!(versao_do(&vols[2]), alvo, "o volume 3 devia estar atrasado");
        assert!(!novos(&d).is_empty());

        conferir_linhas(&d, 75);
        for v in volumes(&d) {
            assert_eq!(versao_do(&v), alvo, "{} nao terminou", v.display());
        }
        assert!(novos(&d).is_empty());
        assert_eq!(
            contem(&todo_o_reg(&d), SEGREDO.as_bytes()),
            !cifrar,
            "o conjunto terminou misturado"
        );
        cofre::desligar();
    }
}

// ---------------------------------------------------------------------------
// (3) escrita no meio
// ---------------------------------------------------------------------------

/// Escrita entre as duas fases (o que o congelamento impede em servico) ->
/// `Conflito`, a tabela fica como estava COM a escrita, e os `*.novo` saem.
///
/// # Prova real
///
/// Sem o `conferir_retrato` na FASE B, o `rename` publica o retrato velho por
/// cima da escrita: a linha 3 volta ao valor de antes e o teste reprova.
#[test]
fn escrita_no_meio_e_conflito_sem_perda() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = tabela_em_claro("escrita", false, 40);
    let mut t = Table::abrir(&d, "clientes").unwrap();
    let troca = t.preparar_migracao_da_cifra(true).unwrap();
    let mut mudada = linha(3);
    mudada[1] = Value::Str("Escrito no meio da migracao".into());
    t.atualizar(3, &mudada).unwrap();
    t.sincronizar().unwrap();

    let e = t.aplicar_migracao_da_cifra(troca).unwrap_err();
    assert!(matches!(e, PhxError::Conflito(_)), "recusou como {e:?}");
    drop(t);
    assert_eq!(versao(&d), 4, "a tabela mudou apesar do conflito");
    let mut t = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(
        t.ler(3).unwrap().unwrap()[1],
        Value::Str("Escrito no meio da migracao".into()),
        "a escrita confirmada foi perdida"
    );
    assert!(novos(&d).is_empty());
    cofre::desligar();
}

// ---------------------------------------------------------------------------
// (5) o comportamento VELHO
// ---------------------------------------------------------------------------

/// Sem coluna marcada ou sem cofre a migracao RECUSA antes de gravar byte, e
/// o disco fica como estava -- o par dos dois sentidos do teste do
/// comportamento velho.
///
/// # Prova real
///
/// Sem a conferencia, `Criptografar` sem coluna marcada reescreveria a tabela
/// para a v5 com rabo zero (64 bytes de cabecalho para nao proteger nada) e a
/// versao 4 da asserção reprova.
#[test]
fn sem_coluna_marcada_ou_sem_cofre_nada_e_tocado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    // (a) sem coluna marcada, com cofre.
    cofre::desligar();
    let d = DirTemp::novo("migra-sem-marca");
    let sem_marca = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap();
    let mut t = Table::criar(&d, sem_marca).unwrap();
    t.inserir(&[Value::Int(1), Value::Str("a".into())]).unwrap();
    t.sincronizar().unwrap();
    drop(t);
    cofre::definir(SENHA, RAPIDO).unwrap();
    let antes = std::fs::read(d.join("clientes.reg")).unwrap();
    let mut t = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(
        t.conferir_migracao_da_cifra(true),
        Err(RecusaDaMigracao::NadaACifrar)
    );
    assert!(t.preparar_migracao_da_cifra(true).is_err());
    drop(t);
    assert_eq!(std::fs::read(d.join("clientes.reg")).unwrap(), antes);
    assert!(novos(&d).is_empty());

    // (b) com coluna marcada, SEM cofre (8): recusa antes de gravar.
    let d2 = tabela_em_claro("sem-cofre", false, 5);
    cofre::desligar();
    let antes = std::fs::read(d2.join("clientes.reg")).unwrap();
    let mut t = Table::abrir(&d2, "clientes").unwrap();
    assert_eq!(
        t.conferir_migracao_da_cifra(true),
        Err(RecusaDaMigracao::CofreDesligado)
    );
    let e = t
        .preparar_migracao_da_cifra(true)
        .err()
        .expect("devia recusar");
    assert!(matches!(e, PhxError::Esquema(_)), "{e:?}");
    drop(t);
    assert_eq!(std::fs::read(d2.join("clientes.reg")).unwrap(), antes);
    assert!(novos(&d2).is_empty());

    // (c) o lado que PODE: com coluna marcada e cofre, migra.
    cofre::definir(SENHA, RAPIDO).unwrap();
    migrar(&d2, true);
    assert_eq!(versao(&d2), 5);
    cofre::desligar();
}

/// Senha ERRADA: a tabela cifrada nem abre, entao `Descriptografar` recusa
/// antes de gravar byte -- o arquivo e o mesmo, ao byte.
#[test]
fn senha_errada_recusa_antes_de_gravar() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = tabela_em_claro("senha", false, 8);
    migrar(&d, true);
    let antes = std::fs::read(d.join("clientes.reg")).unwrap();
    cofre::definir("outra senha qualquer", RAPIDO).unwrap();
    assert!(
        Table::abrir(&d, "clientes").is_err(),
        "a senha errada abriu a tabela cifrada"
    );
    assert_eq!(std::fs::read(d.join("clientes.reg")).unwrap(), antes);
    assert!(novos(&d).is_empty());
    cofre::desligar();
    // Cofre desligado: idem.
    assert!(Table::abrir(&d, "clientes").is_err());
    assert_eq!(std::fs::read(d.join("clientes.reg")).unwrap(), antes);
}

// ---------------------------------------------------------------------------
// As recusas de escopo
// ---------------------------------------------------------------------------

/// Coluna EXTERNA marcada e indice de texto sobre coluna marcada: recusa com
/// motivo, ANTES de gravar byte.
///
/// # Prova real
///
/// Sem a recusa, `Criptografar` numa tabela com `Memo` marcado cifraria o
/// `.reg` e deixaria o conteudo legivel no `.memo` -- o teste reprova na
/// primeira asserção.
#[test]
fn coluna_externa_e_indice_de_texto_sao_recusados_antes_de_gravar() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = DirTemp::novo("migra-externa");
    let com_memo = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("obs", ColumnType::Memo).com_dado_pessoal(DadoPessoal::Sensivel),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    let mut t = Table::criar(&d, com_memo).unwrap();
    t.inserir(&[
        Value::Int(1),
        Value::Str("a".into()),
        Value::Memo("anotacao".into()),
    ])
    .unwrap();
    t.sincronizar().unwrap();
    drop(t);
    cofre::definir(SENHA, RAPIDO).unwrap();
    let antes = std::fs::read(d.join("clientes.reg")).unwrap();
    let mut t = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(
        t.conferir_migracao_da_cifra(true),
        Err(RecusaDaMigracao::ColunaExterna("obs".into()))
    );
    assert!(t.preparar_migracao_da_cifra(true).is_err());
    drop(t);
    assert_eq!(std::fs::read(d.join("clientes.reg")).unwrap(), antes);
    assert!(novos(&d).is_empty());

    // Indice de texto sobre a coluna marcada `nome`.
    cofre::desligar();
    let d = DirTemp::novo("migra-fts");
    let com_fts = esquema(false)
        .com_indices_de_texto(vec![IndiceDeTexto::new("buscaNome", 1)])
        .unwrap();
    let mut t = Table::criar(&d, com_fts).unwrap();
    t.inserir(&linha(1)).unwrap();
    t.sincronizar().unwrap();
    drop(t);
    cofre::definir(SENHA, RAPIDO).unwrap();
    let antes = std::fs::read(d.join("clientes.reg")).unwrap();
    let t = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(
        t.conferir_migracao_da_cifra(true),
        Err(RecusaDaMigracao::IndiceDeTexto(
            "buscaNome".into(),
            "nome".into()
        ))
    );
    drop(t);
    assert_eq!(std::fs::read(d.join("clientes.reg")).unwrap(), antes);
    cofre::desligar();
}

/// Cifrar o que ja esta cifrado e decifrar o que ja esta em claro: recusa com
/// motivo proprio, sem `*.novo`.
#[test]
fn migrar_para_onde_a_tabela_ja_esta_e_recusado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = tabela_em_claro("ja-esta", false, 6);
    let t = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(
        t.conferir_migracao_da_cifra(false),
        Err(RecusaDaMigracao::JaEmClaro)
    );
    drop(t);
    migrar(&d, true);
    let t = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(
        t.conferir_migracao_da_cifra(true),
        Err(RecusaDaMigracao::JaCifrada)
    );
    cofre::desligar();
}

// ---------------------------------------------------------------------------
// (7) a guarda de geometria: a VERSAO entra na tupla
// ---------------------------------------------------------------------------

/// Em tabela so de colunas EXTERNAS o rabo e zero: `slot_size`, `data_offset`
/// e CRC do esquema podem ser IGUAIS nos dois lados da migracao, e o `*.novo`
/// ficaria indistinguivel do volume velho. O volume 2 aqui tem o cabecalho de
/// 128 bytes (versao 4) com a MESMA tripla do 192 do volume 1 (versao 5), e o
/// `*.novo` ao lado e o v5 completo: a troca esta decidida, e so a versao a
/// distingue de uma sobra.
///
/// # Prova real
///
/// Com a tupla de tres campos (`geometria_do_volume` sem a versao), o volume 2
/// forjado "bate" com o esperado, a troca vira sobra, o `*.novo` e apagado e
/// o volume 2 fica na versao 4 sob um volume 1 na 5 -- calado, porque a
/// uniformidade tambem compara a tripla. A asserção da versao do volume 2
/// reprova.
#[test]
fn a_versao_entra_na_geometria_quando_o_resto_empata() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::definir(SENHA, RAPIDO).unwrap();
    let d = DirTemp::novo("migra-geometria");
    let so_externa = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("obs", ColumnType::Memo).com_dado_pessoal(DadoPessoal::Sensivel),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
    .com_paginacao(Paginacao::nova(POR_VOLUME, 9).unwrap())
    .unwrap();
    let mut t = Table::criar(&d, so_externa).unwrap();
    for i in 1..=75 {
        t.inserir(&[Value::Int(i), Value::Memo(format!("anotacao {i}"))])
            .unwrap();
    }
    t.sincronizar().unwrap();
    drop(t);
    let vols = volumes(&d);
    assert_eq!(vols.len(), 3);
    assert_eq!(
        versao_do(&vols[1]),
        5,
        "a tabela so de externas nasce cifrada"
    );

    // O `*.novo` do volume 2 e o v5 completo; o volume 2 "velho" tem o
    // cabecalho de 128 bytes da v4 com a MESMA tripla.
    let novo = vols[1].with_file_name(format!(
        "{}.novo",
        vols[1].file_name().unwrap().to_string_lossy()
    ));
    std::fs::copy(&vols[1], &novo).unwrap();
    let mut b = std::fs::read(&vols[1]).unwrap();
    b[8..10].copy_from_slice(&4u16.to_le_bytes());
    b[10..12].copy_from_slice(&128u16.to_le_bytes());
    let crc = phxsql_core::crc::crc32(&b[..124]);
    b[124..128].copy_from_slice(&crc.to_le_bytes());
    std::fs::write(&vols[1], &b).unwrap();

    // A abertura gravavel termina a troca decidida.
    let mut t = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(t.varrer_com(Visao::Todas).unwrap().len(), 75);
    drop(t);
    assert_eq!(
        versao_do(&vols[1]),
        5,
        "a troca decidida foi tomada por sobra: a versao nao esta na geometria"
    );
    assert!(novos(&d).is_empty());
    cofre::desligar();
}

// ---------------------------------------------------------------------------
// (661) o `*.novo` e sempre nosso e novo, e a FASE B o confere
// ---------------------------------------------------------------------------

/// Um `clientes.reg.novo` plantado como LINK FISICO de uma isca antes da
/// FASE A: o `Descriptografar` escreve o texto claro num arquivo NOVO, e a
/// isca fica como estava.
///
/// A isca e plantada com a tabela ja aberta: a abertura gravavel recolhe os
/// `*.novo` orfaos (pedido 625), e plantar antes dela faria o teste passar
/// pela limpeza, e nao pelo conserto.
///
/// # Prova real
///
/// Com o `.novo` voltando ao `recriar_do_banco` (modo `Banco`, que trunca e
/// reusa o inode do nome), a isca recebe o payload em claro -- o
/// `contem(SEGREDO)` da isca reprova.
#[test]
fn o_novo_plantado_como_link_fisico_nao_recebe_o_texto_claro() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = tabela_em_claro("isca-novo", false, 20);
    migrar(&d, true);
    assert_eq!(versao(&d), 5);

    let isca = d.join("isca.txt");
    std::fs::write(&isca, b"conteudo da isca").unwrap();

    let mut t = Table::abrir(&d, "clientes").unwrap();
    std::fs::hard_link(&isca, d.join("clientes.reg.novo")).unwrap();
    let troca = t.preparar_migracao_da_cifra(false).unwrap();
    t.aplicar_migracao_da_cifra(troca)
        .expect("o descriptografar com o nome plantado tinha de passar por um arquivo novo");
    drop(t);

    let na_isca = std::fs::read(&isca).unwrap();
    assert!(
        !contem(&na_isca, SEGREDO.as_bytes()),
        "o texto claro da tabela caiu na isca pelo link fisico do `.novo`"
    );
    assert_eq!(na_isca, b"conteudo da isca", "a isca foi reescrita");
    assert_eq!(versao(&d), 4);
    conferir_linhas(&d, 20);
    assert!(novos(&d).is_empty());
    cofre::desligar();
}

/// O `.novo` plantado difere do escrito SO no inode: mesmo tamanho, mesma
/// data, um nome so.
#[cfg(unix)]
fn so_o_inode_mudou(novo: &Path, antes: &std::fs::Metadata) {
    use std::os::unix::fs::MetadataExt;
    let depois = std::fs::metadata(novo).unwrap();
    assert_ne!(depois.ino(), antes.ino(), "o inode voltou");
    assert_eq!(depois.modified().unwrap(), antes.modified().unwrap());
    assert_eq!((depois.len(), depois.nlink()), (antes.len(), 1));
}

#[cfg(not(unix))]
fn so_o_inode_mudou(_novo: &Path, _antes: &std::fs::Metadata) {}

/// O `clientes.reg.novo` trocado na janela entre as fases (trava solta) por
/// um arquivo de mesmo conteudo e OUTRO inode: a FASE B recusa com
/// `Conflito`, nada e trocado, e o nome plantado sai.
///
/// Mesmo conteudo de proposito: a conferencia e de IDENTIDADE (o arquivo que
/// a FASE A escreveu), nao de forma -- um `*.novo` montado por fora com
/// cabecalho valido passaria por uma conferencia de forma.
///
/// # Mesmo tamanho, mesma data, e o inode velho SEGURO -- pedido 672
///
/// A versao de antes apagava o `.novo` e escrevia outro, e o sistema de
/// arquivos DEVOLVIA o mesmo numero de inode ao arquivo novo: medido em
/// 07/10/2026, sem a comparacao das DATAS este teste caia, e sem a do inode
/// nao caia -- ele provava a data dizendo provar a identidade. Agora o
/// velho fica vivo com outro nome (o inode nao volta a fila) e a data do
/// novo e reposta: so o inode separa.
///
/// # Prova real
///
/// Sem o `conferir_novos` no `alargar_fase_b`, o `rename` publica o arquivo
/// plantado e o `unwrap_err` reprova; sem o `mesmo_arquivo` no
/// `ainda_o_mesmo_temporario`, tambem.
#[test]
fn o_novo_trocado_entre_as_fases_e_recusado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = tabela_em_claro("troca-novo", false, 20);
    migrar(&d, true);
    let mut t = Table::abrir(&d, "clientes").unwrap();
    let troca = t.preparar_migracao_da_cifra(false).unwrap();

    let novo = d.join("clientes.reg.novo");
    let antes = std::fs::metadata(&novo).unwrap();
    let bytes = std::fs::read(&novo).unwrap();
    std::fs::rename(&novo, d.join("o-velho-seguro")).unwrap();
    std::fs::write(&novo, &bytes).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&novo)
        .unwrap()
        .set_modified(antes.modified().unwrap())
        .unwrap();
    so_o_inode_mudou(&novo, &antes);

    let e = t.aplicar_migracao_da_cifra(troca).unwrap_err();
    assert!(matches!(e, PhxError::Conflito(_)), "recusou como {e:?}");
    assert!(
        !format!("{e}").contains(d.to_string_lossy().as_ref()),
        "a recusa vazou o caminho da raiz: {e}"
    );
    drop(t);
    assert_eq!(versao(&d), 5, "o `.novo` trocado foi publicado");
    assert!(
        novos(&d).is_empty(),
        "o nome plantado ficou: {:?}",
        novos(&d)
    );
    conferir_linhas(&d, 20);
    cofre::desligar();
}

// ---------------------------------------------------------------------------
// Pedido 672: as outras condicoes da conferencia da FASE B
// ---------------------------------------------------------------------------
//
// O `util::ainda_o_mesmo_temporario` confere cinco coisas (regular, mesmo
// inode, um nome so, mesmo tamanho, mesma data) e a prova de cima so trocava
// o inode. Cada teste abaixo deixa as outras iguais e quebra UMA, com a
// tabela EM CLARO saindo no `Descriptografar` -- o caso em que o `.novo`
// publicado e o texto claro da tabela inteira.

/// Uma tabela cifrada (v5) aberta, com o `Descriptografar` preparado: o
/// `clientes.reg.novo` em claro esta no disco e a FASE B ainda nao rodou.
fn entre_as_fases(rotulo: &str) -> (DirTemp, Table, phxsql_store::TrocaDaCifra) {
    let d = tabela_em_claro(rotulo, false, 20);
    migrar(&d, true);
    assert_eq!(versao(&d), 5);
    let mut t = Table::abrir(&d, "clientes").unwrap();
    let troca = t.preparar_migracao_da_cifra(false).unwrap();
    (d, t, troca)
}

/// A FASE B recusou com `Conflito`, nada foi trocado e nenhum `.novo` ficou.
fn recusou_sem_trocar(d: &Path, r: phxsql_core::error::Result<u64>, o_que: &str) {
    let e = r.expect_err(o_que);
    assert!(matches!(e, PhxError::Conflito(_)), "recusou como {e:?}");
    assert_eq!(versao(d), 5, "{o_que}: o `.novo` foi publicado");
    assert!(novos(d).is_empty(), "o nome ficou: {:?}", novos(d));
    conferir_linhas(d, 20);
}

/// **Um LINK FISICO pendurado no `.novo` entre as fases.** Mesmo inode,
/// mesmo tamanho, mesma data: so o `um_nome_so` separa. Publicado, a isca
/// vira um segundo nome da tabela EM CLARO que continua valendo depois do
/// `rename` -- toda escrita seguinte na tabela aparece nela.
///
/// # Prova real
///
/// Sem o `um_nome_so` no `ainda_o_mesmo_temporario`, medido em 07/10/2026:
/// a FASE B publica e este cai; o `o_novo_trocado_entre_as_fases_e_recusado`
/// segue verde.
#[test]
fn o_novo_com_link_fisico_pendurado_entre_as_fases_e_recusado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let (d, mut t, troca) = entre_as_fases("link-entre-fases");
    std::fs::hard_link(d.join("clientes.reg.novo"), d.join("isca")).unwrap();
    let r = t.aplicar_migracao_da_cifra(troca);
    drop(t);
    recusou_sem_trocar(&d, r, "o `.novo` com um segundo nome passou");
    cofre::desligar();
}

/// **Uma escrita pelo NOME, no mesmo tamanho, entre as fases.** Mesmo inode,
/// um nome so, mesmo tamanho: so a data separa.
///
/// O relogio dos carimbos do sistema de arquivos pode ser grosso (um tique
/// do kernel), e a escrita na mesma fatia nao moveria a data: se nao moveu,
/// a prova a empurra um segundo, que e o que a escrita moveria num relogio
/// fino. O que se prova e a CONDICAO, nao a resolucao do relogio.
///
/// # Prova real
///
/// Sem a comparacao das datas, medido em 07/10/2026: este cai.
#[test]
fn o_novo_escrito_pelo_nome_no_mesmo_tamanho_e_recusado() {
    use std::io::{Seek, SeekFrom, Write};
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let (d, mut t, troca) = entre_as_fases("escrita-entre-fases");
    let novo = d.join("clientes.reg.novo");
    let antes = std::fs::metadata(&novo).unwrap();
    let ultimo = antes.len() - 1;
    let byte = std::fs::read(&novo).unwrap()[ultimo as usize];
    let mut f = std::fs::OpenOptions::new().write(true).open(&novo).unwrap();
    f.seek(SeekFrom::Start(ultimo)).unwrap();
    f.write_all(&[byte ^ 0xFF]).unwrap();
    if f.metadata().unwrap().modified().unwrap() == antes.modified().unwrap() {
        f.set_modified(antes.modified().unwrap() + std::time::Duration::from_secs(1))
            .unwrap();
    }
    drop(f);
    assert_eq!(std::fs::metadata(&novo).unwrap().len(), antes.len());
    let r = t.aplicar_migracao_da_cifra(troca);
    drop(t);
    recusou_sem_trocar(&d, r, "o `.novo` escrito por fora passou");
    cofre::desligar();
}

/// **O `.novo` que CRESCEU entre as fases, com a data reposta.** Mesmo
/// inode, um nome so, mesma data: so o tamanho separa. Repor a data e uma
/// chamada (`utimensat`) ao alcance de quem escreve no arquivo.
///
/// # Prova real
///
/// Sem a comparacao dos tamanhos, medido em 07/10/2026: este cai.
#[test]
fn o_novo_que_cresceu_com_a_data_reposta_e_recusado() {
    use std::io::Write;
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let (d, mut t, troca) = entre_as_fases("cresceu-entre-fases");
    let novo = d.join("clientes.reg.novo");
    let antes = std::fs::metadata(&novo).unwrap();
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&novo)
        .unwrap();
    f.write_all(&[0u8; 64]).unwrap();
    f.set_modified(antes.modified().unwrap()).unwrap();
    drop(f);
    let depois = std::fs::metadata(&novo).unwrap();
    assert_eq!(depois.modified().unwrap(), antes.modified().unwrap());
    assert_ne!(depois.len(), antes.len());
    let r = t.aplicar_migracao_da_cifra(troca);
    drop(t);
    recusou_sem_trocar(&d, r, "o `.novo` que cresceu passou");
    cofre::desligar();
}

/// **O `.novo` do ESPELHO que sumiu entre as fases.** O `conferir_novos`
/// seguia em frente (`continue`) com qualquer erro do `lstat`; o volume
/// principal tem a pergunta «sumiu?» logo antes, o espelho nao. Sem a
/// recusa, o `rename` do `.reg` acontecia, o do `.bkp` falhava, e a tabela
/// ficava com o principal EM CLARO e o espelho cifrado: a segunda chance do
/// `.reg` passava a ser outro arquivo.
///
/// # Prova real
///
/// Com o `continue` de volta no erro do `lstat`, medido em 07/10/2026: este
/// cai.
#[test]
fn o_novo_do_espelho_que_sumiu_entre_as_fases_e_recusado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = DirTemp::novo("migra-espelho-sumiu");
    let mut t = Table::criar_espelhada(&d, esquema(false)).unwrap();
    for i in 1..=20 {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    drop(t);
    cofre::definir(SENHA, RAPIDO).unwrap();
    let mut t = Table::abrir_espelhada(&d, "clientes").unwrap();
    let troca = t.preparar_migracao_da_cifra(true).unwrap();
    t.aplicar_migracao_da_cifra(troca).unwrap();
    drop(t);
    let bkp = d.join("clientes.bkp");
    assert_eq!((versao(&d), versao_do(&bkp)), (5, 5));

    let mut t = Table::abrir_espelhada(&d, "clientes").unwrap();
    let troca = t.preparar_migracao_da_cifra(false).unwrap();
    let do_espelho = d.join("clientes.bkp.novo");
    assert!(
        do_espelho.is_file(),
        "o espelho nao ganhou `.novo`: {:?}",
        novos(&d)
    );
    std::fs::remove_file(&do_espelho).unwrap();
    let r = t.aplicar_migracao_da_cifra(troca);
    drop(t);
    let e = r.expect_err("o `.novo` do espelho sumiu e a FASE B seguiu");
    assert!(matches!(e, PhxError::Conflito(_)), "recusou como {e:?}");
    assert_eq!(
        (versao(&d), versao_do(&bkp)),
        (5, 5),
        "o principal e o espelho ficaram em versoes diferentes"
    );
    assert!(novos(&d).is_empty(), "o nome ficou: {:?}", novos(&d));
    conferir_linhas(&d, 20);
    cofre::desligar();
}

// ---------------------------------------------------------------------------
// Pedido 647: o inode velho morre FORA da trava
// ---------------------------------------------------------------------------

/// Quantos descritores DESTE processo apontam para um arquivo apagado dentro
/// de `d` -- o volume velho que a FASE B trocou e que alguem ainda segura. E o
/// sistema operacional que responde, pelo `/proc/self/fd`, e nao um contador
/// do motor: o que se prova e onde o nucleo solta o inode.
#[cfg(target_os = "linux")]
fn velhos_seguros(d: &Path) -> usize {
    let d = std::fs::canonicalize(d).unwrap();
    let prefixo = format!("{}/", d.display());
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .flatten()
        .filter_map(|e| std::fs::read_link(e.path()).ok())
        .map(|alvo| alvo.to_string_lossy().into_owned())
        .filter(|alvo| alvo.starts_with(&prefixo) && alvo.ends_with(" (deleted)"))
        .count()
}

/// A FASE B deixa cada volume velho vivo num descritor, sem nome, e o
/// `soltar_volumes_velhos` -- que o servidor chama depois de soltar a trava
/// global -- e quem o fecha. Medido em 08/10/2026: a 10 M de linhas o
/// `rename` por cima custava 1,2 s porque derrubava o ultimo nome do inode e
/// o nucleo liberava as extensoes ali dentro, com a trava na mao.
///
/// Tambem prova o esquecido: o `Drop` da tabela solta, e no fim nenhum volume
/// velho -- o EM CLARO do `Criptografar` -- fica preso no processo.
///
/// # Prova real
///
/// Sem o `segurar_o_velho` no `alargar_fase_b`, nenhum descritor aponta para
/// arquivo apagado depois da FASE B e a segunda asserção reprova (0 contra o
/// numero de volumes).
#[cfg(target_os = "linux")]
#[test]
fn a_fase_b_segura_o_volume_velho_ate_soltar() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = tabela_em_claro("segura-o-velho", true, 70);
    let n = volumes(&d).len();
    assert!(n >= 3, "a prova pede mais de um volume: {n}");

    let mut t = Table::abrir(&d, "clientes").unwrap();
    let troca = t.preparar_migracao_da_cifra(true).unwrap();
    assert_eq!(velhos_seguros(&d), 0, "a FASE A nao troca nada");
    t.aplicar_migracao_da_cifra(troca).unwrap();
    assert_eq!(
        velhos_seguros(&d),
        n,
        "a FASE B nao segurou os volumes velhos: o `rename` os liberou sob a trava"
    );
    assert_eq!(t.soltar_volumes_velhos(), n);
    assert_eq!(velhos_seguros(&d), 0, "soltar nao fechou os velhos");

    // O esquecido: ninguem chama o `soltar`, e o `Drop` fecha.
    let troca = t.preparar_migracao_da_cifra(false).unwrap();
    t.aplicar_migracao_da_cifra(troca).unwrap();
    assert_eq!(velhos_seguros(&d), n);
    drop(t);
    assert_eq!(velhos_seguros(&d), 0, "o `Drop` deixou volume velho preso");

    conferir_linhas(&d, 70);
    cofre::desligar();
}

// ---------------------------------------------------------------------------
// O `.ndx` vai junto (pedido 339, achado 2)
// ---------------------------------------------------------------------------

/// `Criptografar` numa tabela cuja ARVORE guarda a coluna marcada leva o
/// `.ndx` junto, selado: cifrar o `.reg` e deixar o indice ao lado em claro
/// diria «cifrada» com o valor legivel no arquivo vizinho.
///
/// # Prova real
///
/// Sem o `refazer_ndx` no `aplicar_migracao_da_cifra`, o `.reg` sai v5 sem o
/// segredo e o `.ndx` continua v1 COM ele -- a terceira assercao reprova.
/// A busca pelo indice depois da migracao e o controle: um `.ndx` vazio
/// tambem nao conteria o segredo.
#[test]
fn criptografar_sela_o_ndx_sobre_a_coluna_marcada() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = DirTemp::novo("migra-ndx-selado");
    let com_arvore_marcada = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("cpf", ColumnType::Str(14)).com_dado_pessoal(DadoPessoal::Sensivel),
            Column::new("cidade", ColumnType::Str(20)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)])
                .unico()
                .primaria(),
            IndexDef::new("porNome", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap();
    {
        let mut t = Table::criar(&d, com_arvore_marcada).unwrap();
        for i in 1..=40 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    let ndx = d.join("clientes.ndx");
    assert!(
        contem(&std::fs::read(&ndx).unwrap(), SEGREDO.as_bytes()),
        "controle: em claro, o .ndx TEM de guardar o nome"
    );
    cofre::definir(SENHA, RAPIDO).unwrap();

    migrar(&d, true);
    assert_eq!(versao(&d), 5);
    assert!(
        !contem(&std::fs::read(&ndx).unwrap(), SEGREDO.as_bytes()),
        "Criptografar deixou o nome em claro no .ndx"
    );
    assert_eq!(versao_do(&ndx), 2, "o .ndx nao declara a pagina selada");

    let mut t = Table::abrir(&d, "clientes").unwrap();
    assert!(t.ndx_selado());
    assert_eq!(
        t.buscar("porNome", &[Value::Str(format!("{SEGREDO} 0013"))])
            .unwrap(),
        vec![13],
        "a busca pela arvore refeita nao achou"
    );
    drop(t);
    cofre::desligar();
}

/// A FASE A monta o `.ndx.novo` selado FORA da trava, e a queda entre as
/// fases o deixa sem dono: a abertura gravavel seguinte o recolhe, e a
/// tabela volta ao estado de antes -- o `.reg` em claro servido pela arvore
/// em claro.
///
/// # Prova real
///
/// Sem o `recolher_ndx_ao_lado` na abertura, o `.ndx.novo` sobra e a
/// primeira assercao depois da reabertura reprova.
#[test]
fn a_queda_entre_as_fases_recolhe_o_ndx_ao_lado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = DirTemp::novo("migra-ndx-queda");
    let com_arvore_marcada = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("cpf", ColumnType::Str(14)).com_dado_pessoal(DadoPessoal::Sensivel),
            Column::new("cidade", ColumnType::Str(20)),
        ],
        vec![IndexDef::new("porNome", vec![IndexColumn::asc(1)])],
    )
    .unwrap();
    {
        let mut t = Table::criar(&d, com_arvore_marcada).unwrap();
        for i in 1..=15 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    cofre::definir(SENHA, RAPIDO).unwrap();
    {
        let mut t = Table::abrir(&d, "clientes").unwrap();
        let troca = t.preparar_migracao_da_cifra(true).unwrap();
        let ao_lado = d.join("clientes.ndx.novo");
        assert!(ao_lado.exists(), "a FASE A nao montou o .ndx ao lado");
        assert_eq!(versao_do(&ao_lado), 2, "o .ndx ao lado nasceu em claro");
        assert!(
            !contem(&std::fs::read(&ao_lado).unwrap(), SEGREDO.as_bytes()),
            "o .ndx ao lado guarda o nome em claro"
        );
        // A queda: nem FASE B nem descarte.
        std::mem::forget(troca);
    }
    let mut t = Table::abrir(&d, "clientes").unwrap();
    // So o `.ndx.novo`: o `clientes.reg.novo` continua com dono NESTE
    // processo (o `forget` nao solta o registro do pedido 625), e quem o
    // recolhe numa queda de verdade e o processo novo.
    assert!(
        !d.join("clientes.ndx.novo").exists(),
        "o .ndx.novo sem dono sobrou: {:?}",
        novos(&d)
    );
    assert_eq!(versao(&d), 4);
    assert!(!t.ndx_selado());
    assert_eq!(
        t.buscar("porNome", &[Value::Str(format!("{SEGREDO} 0009"))])
            .unwrap(),
        vec![9]
    );
    drop(t);
    cofre::desligar();
}

/// Uma base `loja` com `clientes`, arvore sobre a coluna marcada, criada com
/// o cofre DESLIGADO e devolvida com ele LIGADO.
fn base_com_arvore_marcada(rotulo: &str) -> (DirTemp, phxsql_store::catalogo::Instancia) {
    cofre::desligar();
    let base = DirTemp::novo(rotulo);
    let inst = phxsql_store::catalogo::Instancia::nova(&base).unwrap();
    let db = inst.criar_database("loja").unwrap();
    let esq = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("cpf", ColumnType::Str(14)).com_dado_pessoal(DadoPessoal::Sensivel),
            Column::new("cidade", ColumnType::Str(20)),
        ],
        vec![IndexDef::new("porNome", vec![IndexColumn::asc(1)])],
    )
    .unwrap();
    let mut t = db.criar_tabela(None, esq).unwrap();
    for i in 1..=12 {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    drop(t);
    cofre::definir(SENHA, RAPIDO).unwrap();
    (base, inst)
}

/// **A queda ENTRE os dois `rename` da FASE B** (condicao A do papel C): o
/// `.reg` ja cifrado, o `.ndx.novo` selado ao lado. A abertura recolhe o
/// `.ndx.novo` -- ele nao tem dono --, e a tabela fica cifrada com a arvore
/// em claro. Nao se converte a forca (guarda nova entra pedida): o arranque
/// AVISA, nomeando a tabela e o `reindexar`, e o `reindexar` sela.
///
/// # Prova real
///
/// Com o `indices_em_claro_sobre_coluna_marcada` devolvendo lista vazia, o
/// arranque cala e a assercao do aviso reprova. O `a_queda_entre_as_fases_...`
/// nao pegava isto: ele cai ANTES da FASE B, quando o `.reg` ainda e claro.
#[test]
fn a_queda_entre_os_dois_renames_da_fase_b_e_avisada_no_arranque() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let (base, inst) = base_com_arvore_marcada("migra-entre-renames");
    let db = inst.abrir_database("loja").unwrap();
    let loja = base.join("loja");
    {
        let mut t = db.abrir_tabela(None, "clientes").unwrap();
        let troca = t.preparar_migracao_da_cifra(true).unwrap();
        armar(Ponto::CifraEntreOsDoisRenames);
        let caiu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = t.aplicar_migracao_da_cifra(troca);
        }));
        desarmar();
        assert!(caiu.is_err(), "o ponto entre os dois rename nao disparou");
    }
    assert_eq!(versao(&loja), 5, "o .reg nao chegou a virar");
    assert!(
        loja.join("clientes.ndx.novo").exists(),
        "a FASE A nao deixou o .ndx ao lado"
    );

    // O arranque AVISA -- ele so le cabecalhos, e quem recolhe o `.ndx.novo`
    // e a primeira abertura gravavel, logo abaixo.
    let r = db.recuperar_marcas();
    assert_eq!(versao_do(&loja.join("clientes.ndx")), 1);
    assert!(contem(
        &std::fs::read(loja.join("clientes.ndx")).unwrap(),
        SEGREDO.as_bytes()
    ));
    assert_eq!(
        r.indices_em_claro.len(),
        1,
        "o arranque calou: {:?}",
        r.indices_em_claro
    );
    let aviso = &r.indices_em_claro[0];
    assert!(
        aviso.contains("loja/clientes") && aviso.contains("reindexar"),
        "{aviso}"
    );
    assert!(aviso.contains("com o cofre ligado -- sele com"), "{aviso}");
    assert!(
        r.texto(&base).contains("EM CLARO sob o cofre"),
        "{}",
        r.texto(&base)
    );

    // Nao converte sozinho; o `reindexar` pedido sela, e o aviso some.
    let mut t = db.abrir_tabela(None, "clientes").unwrap();
    assert!(
        !loja.join("clientes.ndx.novo").exists(),
        "o .ndx.novo sem dono sobrou"
    );
    assert!(t.ndx_em_claro_sobre_coluna_marcada());
    t.reindexar().unwrap();
    t.sincronizar().unwrap();
    assert!(!t.ndx_em_claro_sobre_coluna_marcada());
    drop(t);
    assert!(db.recuperar_marcas().indices_em_claro.is_empty());
    assert!(!contem(
        &std::fs::read(loja.join("clientes.ndx")).unwrap(),
        SEGREDO.as_bytes()
    ));
    cofre::desligar();
}

/// **O irmao: marcar coluna JA indexada** (item 3b do papel C). A marca nao
/// refaz a arvore -- decisao do DBA, guarda nova entra pedida --, e a mesma
/// deteccao avisa no arranque.
///
/// # Prova real
///
/// Com a deteccao calada, o aviso nao aparece e a assercao reprova.
#[test]
fn marcar_coluna_ja_indexada_e_avisado_e_nao_refaz_a_arvore() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let base = DirTemp::novo("migra-marcar-indexada");
    let inst = phxsql_store::catalogo::Instancia::nova(&base).unwrap();
    let db = inst.criar_database("loja").unwrap();
    cofre::definir(SENHA, RAPIDO).unwrap();
    let esq = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new("cpf", ColumnType::Str(14)),
            Column::new("cidade", ColumnType::Str(20)),
        ],
        vec![IndexDef::new("porNome", vec![IndexColumn::asc(1)])],
    )
    .unwrap();
    let mut t = db.criar_tabela(None, esq).unwrap();
    for i in 1..=5 {
        t.inserir(&linha(i)).unwrap();
    }
    assert!(
        !t.ndx_em_claro_sobre_coluna_marcada(),
        "sem marca nao ha o que avisar"
    );
    t.marcar_dado_pessoal(&[("nome".to_string(), DadoPessoal::Pessoal)])
        .unwrap();
    t.sincronizar().unwrap();
    assert!(!t.ndx_selado(), "marcar nao refaz a arvore");
    assert!(t.ndx_em_claro_sobre_coluna_marcada());
    drop(t);
    let r = db.recuperar_marcas();
    assert_eq!(
        r.indices_em_claro.len(),
        1,
        "o arranque calou: {:?}",
        r.indices_em_claro
    );
    assert!(
        r.indices_em_claro[0].contains("com o cofre ligado -- sele com"),
        "{:?}",
        r.indices_em_claro
    );
    cofre::desligar();
}
