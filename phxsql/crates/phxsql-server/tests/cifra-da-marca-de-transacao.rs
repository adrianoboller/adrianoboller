//! O selo da marca `transacao_<id>.tx` -- pedido 354.
//!
//! A marca do `COMMIT` guardava a linha INTEIRA em claro, e a `linha_antiga`
//! do `atualizar` tambem: essa sai DECIFRADA do `.reg` para entrar aqui. Um
//! `.reg` cifrado, um `.log` cifrado, e no meio do caminho um arquivo em claro
//! com o mesmo conteudo -- aberto para toda a maquina, porque nascia 0644.
//!
//! # Por que um arquivo de `tests/` e nao `mod testes` na biblioteca
//!
//! Porque a cifra e do PROCESSO: `cofre::definir` mexe num global, e cada
//! arquivo de `tests/` e um binario proprio. Ligar o cofre dentro do binario
//! da biblioteca trocaria a cifra de todos os outros testes que correm em
//! paralelo com este. Aqui dentro, o [`UM_DE_CADA_VEZ`] faz o mesmo papel
//! entre os quatro.
//!
//! # O que este arquivo trava
//!
//! 1. o valor da linha **nao aparece em claro** nos bytes do arquivo -- e a
//!    prova e por busca de subcadeia nos bytes crus, nunca pelo proprio
//!    leitor, que devolveria o claro dos dois jeitos;
//! 2. marca cifrada **sem a chave** para e **nao e apagada** pela recuperacao;
//! 3. a senha ERRADA cai no mesmo lugar, e nao no «nao confere» que apaga;
//! 4. marca escrita ANTES da cifra continua sendo lida depois dela.

mod comum;
use comum::DirTemp;

use std::sync::Mutex;

use phxsql_core::value::Value;
use phxsql_server::transacao::{self, gravar_marca, ler_marca, recuperar, Acao, Escrita, Leitura};
use phxsql_store::catalogo::Instancia;
use phxsql_store::cofre;

/// O cofre e um global do processo, e estes quatro o ligam e desligam.
static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

/// O piso de iteracoes do cofre. Um teste nao precisa das 210.000 do padrao
/// para provar que o selo fecha, e pagaria segundos por isso.
const RAPIDO: u32 = cofre::ITERACOES_MINIMAS;

/// A senha com que TODA marca cifrada deste arquivo e escrita.
///
/// Uma so, e de proposito: o material da marca e derivado uma vez por PROCESSO
/// (ver `transacao::material_da_marca`), entao a primeira senha ligada aqui e a
/// que sela tudo o que este binario grava. Usar duas senhas na escrita faria o
/// teste depender da ordem em que o Rust roda os `#[test]`.
const SENHA: &str = "a chave do cofre de teste";
const OUTRA_SENHA: &str = "nao e esta a senha do arquivo";

const SEGREDO: &str = "Adriano Boller da Silva";
const SEGREDO_ANTIGO: &str = "Adriano Boller de Blumenau";
const SEGREDO_MEMO: &str = "parecer confidencial do titular";
const SEGREDO_MOTIVO: &str = "pedido de apagamento do titular 1998";

fn escritas() -> Vec<Escrita> {
    vec![
        Escrita {
            database: "loja".into(),
            tabela: "clientes".into(),
            acao: Acao::Atualizar,
            rowid: 7,
            linha: vec![
                Value::Int(1),
                Value::Str(SEGREDO.into()),
                Value::Memo(SEGREDO_MEMO.into()),
                Value::Bin(b"cartao-4111111111111111".to_vec()),
            ],
            // A que o dono nomeou: ela vem DECIFRADA do `.reg`.
            linha_antiga: vec![Value::Int(1), Value::Str(SEGREDO_ANTIGO.into())],
            motivo: String::new(),
            cascata_na_lista: true,
        },
        Escrita {
            database: "loja".into(),
            tabela: "pedidos".into(),
            acao: Acao::ExcluirSuave,
            rowid: 3,
            linha: Vec::new(),
            linha_antiga: Vec::new(),
            motivo: SEGREDO_MOTIVO.into(),
            cascata_na_lista: false,
        },
    ]
}

fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
    palheiro.windows(agulha.len()).any(|j| j == agulha)
}

/// As agulhas que NAO podem aparecer nos bytes crus.
fn segredos() -> Vec<&'static str> {
    vec![
        SEGREDO,
        SEGREDO_ANTIGO,
        SEGREDO_MEMO,
        SEGREDO_MOTIVO,
        "cartao-4111111111111111",
    ]
}

// ------------------------------------------------------------------ 1. o claro

/// O valor da linha nao aparece em claro no arquivo -- e isso se prova nos
/// BYTES, nunca pelo leitor.
///
/// Prova real: tirar o `material.selar(...)` do `gravar_marca` e gravar o
/// `payload` cru faz as cinco agulhas reaparecerem.
///
/// A segunda metade do teste e o que o impede de passar por engano: uma marca
/// que cifrasse para lixo tambem esconderia as agulhas. Ela tem de **voltar
/// igual** pelo `ler_marca`.
#[test]
fn o_valor_da_linha_nao_aparece_em_claro_na_marca() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = DirTemp::novo("marca-cifrada");
    cofre::definir(SENHA, RAPIDO).unwrap();

    let caminho = gravar_marca(&d, 101, 1_700_000_000_000, &escritas()).unwrap();
    let bruto = std::fs::read(&caminho).unwrap();

    assert_eq!(
        u32::from_le_bytes([bruto[8], bruto[9], bruto[10], bruto[11]]),
        transacao::VERSAO,
        "com o cofre ligado a marca tem de nascer na versao com material de cifra"
    );
    for agulha in segredos() {
        assert!(
            !contem(&bruto, agulha.as_bytes()),
            "{agulha:?} apareceu em claro nos {} bytes da marca",
            bruto.len()
        );
    }
    // A tabela continua em claro, e isso e decisao: a recuperacao precisa
    // saber ONDE reaplicar antes de abrir O QUE reaplicar. O que ela ganhou
    // foi o dado associado, que a amarra ao selo.
    assert!(contem(&bruto, b"clientes"));

    // E a volta fecha -- sem isto, cifrar para lixo passaria no teste de cima.
    let m = ler_marca(&caminho)
        .unwrap()
        .marca()
        .expect("a marca cifrada tem de abrir com a chave que a gravou");
    assert_eq!(m.id, 101);
    assert_eq!(m.operacoes.len(), 2);
    assert_eq!(m.operacoes[0].linha[1], Value::Str(SEGREDO.into()));
    assert_eq!(m.operacoes[0].linha[2], Value::Memo(SEGREDO_MEMO.into()));
    assert_eq!(
        m.operacoes[0].linha_antiga[1],
        Value::Str(SEGREDO_ANTIGO.into())
    );
    assert!(
        m.operacoes[0].cascata_na_lista,
        "o byte da v3 tem de voltar"
    );
    assert_eq!(m.operacoes[1].motivo, SEGREDO_MOTIVO);

    cofre::desligar();
}

/// E ela continua nascendo 0600 com a cifra ligada -- os dois pedacos do
/// pedido andam juntos, e um nao dispensa o outro.
#[cfg(unix)]
#[test]
fn a_marca_cifrada_tambem_nasce_so_para_o_dono() {
    use std::os::unix::fs::PermissionsExt as _;
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = DirTemp::novo("marca-cifrada-modo");
    cofre::definir(SENHA, RAPIDO).unwrap();
    let caminho = gravar_marca(&d, 102, 0, &escritas()).unwrap();
    let modo = std::fs::metadata(&caminho).unwrap().permissions().mode() & 0o777;
    cofre::desligar();
    assert_eq!(modo, 0o600, "a marca cifrada nasceu {modo:o}");
}

// ------------------------------------------------- 2 e 3. a terceira resposta

/// Monta uma base com um database vazio e deixa la uma marca CIFRADA.
///
/// A base e o database nascem com o cofre DESLIGADO de proposito: o que este
/// teste prova e o da marca, e cifrar o catalogo junto misturaria duas coisas
/// numa falha so.
fn base_com_marca_cifrada(rotulo: &str) -> (DirTemp, Instancia, std::path::PathBuf) {
    let d = DirTemp::novo(rotulo);
    cofre::desligar();
    let inst = Instancia::nova(&d).unwrap();
    let db = inst.criar_database("loja").unwrap();
    let dir = db.caminho().to_path_buf();
    drop(db);

    cofre::definir(SENHA, RAPIDO).unwrap();
    let caminho = gravar_marca(&dir, 103, 0, &escritas()).unwrap();
    (d, inst, caminho)
}

/// **A terceira resposta.** Cifrada e sem chave: para, e a recuperacao NAO
/// apaga.
///
/// Sem ela o selo trocaria confidencialidade por durabilidade -- o arranque
/// leria «nao confere», contaria a marca em `descartadas` e apagaria um
/// `COMMIT` confirmado porque faltou uma senha no `config.json`.
///
/// Prova real: fazer `ler_marca` devolver `Leitura::NaoConfere` no lugar de
/// `SemChave` faz o arquivo sumir e `descartadas` virar 1.
#[test]
fn marca_cifrada_sem_chave_para_e_a_recuperacao_nao_a_apaga() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let (_d, inst, caminho) = base_com_marca_cifrada("sem-chave");

    // O servidor sobe SEM a chave -- e o `config.json` que esqueceu a cifra.
    cofre::desligar();

    let resposta = ler_marca(&caminho).unwrap();
    let motivo = match resposta {
        Leitura::SemChave(m) => m,
        outra => panic!("cifrada e sem chave tem de parar, e nao {outra:?}"),
    };
    assert!(
        motivo.contains("nao tem a chave"),
        "a parada tem de dizer que falta a chave: {motivo}"
    );
    // E nao vaza a senha nem o conteudo no texto do erro.
    for agulha in segredos() {
        assert!(!motivo.contains(agulha), "o erro vazou {agulha:?}");
    }

    let r = recuperar(&inst);
    assert_eq!(r.achadas, 1);
    assert_eq!(
        r.descartadas, 0,
        "marca ilegivel por falta de chave nao e descarte"
    );
    assert_eq!(r.completadas, 0);
    assert_eq!(r.paradas.len(), 1, "a parada tem de aparecer no relatorio");
    assert!(
        caminho.exists(),
        "a marca foi APAGADA: a transacao confirmada sumiu por falta de uma senha"
    );
    let texto = r.texto(std::path::Path::new("/base"));
    assert!(
        texto.contains("marcas PARADAS sem a chave") && texto.contains("NAO foram apagadas"),
        "o relatorio do arranque tem de gritar: {texto}"
    );
}

/// A senha ERRADA cai no mesmo lugar da senha ausente.
///
/// Os dois erros errados nao custam o mesmo: parar numa marca que nao abre
/// enche o relatorio; apaga-la apaga uma transacao confirmada. Quem digitou a
/// senha errada no `config.json` merece o primeiro.
#[test]
fn a_senha_errada_tambem_para_em_vez_de_apagar() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let (_d, inst, caminho) = base_com_marca_cifrada("senha-errada");

    cofre::desligar();
    cofre::definir(OUTRA_SENHA, RAPIDO).unwrap();

    match ler_marca(&caminho).unwrap() {
        Leitura::SemChave(m) => assert!(
            m.contains("nao e a que gravou"),
            "a parada tem de dizer que a senha e outra: {m}"
        ),
        outra => panic!("senha errada tem de parar, e nao {outra:?}"),
    }
    let r = recuperar(&inst);
    assert_eq!(r.descartadas, 0);
    assert_eq!(r.paradas.len(), 1);
    assert!(caminho.exists(), "a marca foi apagada pela senha errada");
    cofre::desligar();
}

// ------------------------------------------------------- 4. a marca de antes

/// Marca escrita ANTES da cifra continua sendo lida depois dela.
///
/// E o caso real de quem liga a cifra num banco que ja roda: a marca que
/// estava no disco e um `COMMIT` que ja aconteceu. Proteger o que chega
/// quebrando o que ja esta la nao e protecao, e estrago.
#[test]
fn marca_em_claro_continua_sendo_lida_com_o_cofre_ligado() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = DirTemp::novo("marca-de-antes");

    cofre::desligar();
    let caminho = gravar_marca(&d, 104, 0, &escritas()).unwrap();
    let bruto = std::fs::read(&caminho).unwrap();
    assert_eq!(
        u32::from_le_bytes([bruto[8], bruto[9], bruto[10], bruto[11]]),
        transacao::VERSAO_CASCATA_EM_CLARO,
        "sem cofre a marca nasce na versao anterior"
    );

    // Agora o dono liga a cifra e o servidor sobe.
    cofre::definir(SENHA, RAPIDO).unwrap();
    let m = ler_marca(&caminho)
        .unwrap()
        .marca()
        .expect("a marca de antes da cifra tem de continuar abrindo");
    assert_eq!(m.id, 104);
    assert_eq!(m.operacoes[0].linha[1], Value::Str(SEGREDO.into()));
    assert_eq!(
        m.operacoes[0].linha_antiga[1],
        Value::Str(SEGREDO_ANTIGO.into())
    );
    assert!(m.operacoes[0].cascata_na_lista);
    assert_eq!(m.operacoes[1].motivo, SEGREDO_MOTIVO);
    cofre::desligar();
}
