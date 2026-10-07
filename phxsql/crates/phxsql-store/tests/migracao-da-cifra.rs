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

/// O `clientes.reg.novo` trocado na janela entre as fases (trava solta) por
/// um arquivo de mesmo conteudo e OUTRO inode: a FASE B recusa com
/// `Conflito`, nada e trocado, e o nome plantado sai.
///
/// Mesmo conteudo de proposito: a conferencia e de IDENTIDADE (o arquivo que
/// a FASE A escreveu), nao de forma -- um `*.novo` montado por fora com
/// cabecalho valido passaria por uma conferencia de forma.
///
/// # Prova real
///
/// Sem o `conferir_novos` no `alargar_fase_b`, o `rename` publica o arquivo
/// plantado e o `unwrap_err` reprova.
#[test]
fn o_novo_trocado_entre_as_fases_e_recusado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = tabela_em_claro("troca-novo", false, 20);
    migrar(&d, true);
    let mut t = Table::abrir(&d, "clientes").unwrap();
    let troca = t.preparar_migracao_da_cifra(false).unwrap();

    let novo = d.join("clientes.reg.novo");
    let bytes = std::fs::read(&novo).unwrap();
    std::fs::remove_file(&novo).unwrap();
    std::fs::write(&novo, &bytes).unwrap();

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
