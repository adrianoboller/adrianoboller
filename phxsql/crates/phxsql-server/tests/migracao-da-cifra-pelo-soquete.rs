//! Pedido 268, pelo soquete: `criptografar`/`descriptografar` levam ao disco,
//! PEDIDOS, o que `cifra.tabelas` so declara -- e o processo morto no meio
//! (SIGKILL) nunca deixa a tabela pela metade.
//!
//! # Por que um arquivo so para isto
//!
//! O cofre e um global do PROCESSO (`cifra-pelo-config.rs` explica): ligar a
//! cifra dentro do binario da biblioteca faria a tabela de outro teste nascer
//! cifrada no meio da corrida. Aqui os testes se revezam por uma trava.
//!
//! # O que cada teste mede
//!
//! O DANO no disco -- a versao do `.reg`, o texto claro nos bytes, as linhas
//! que voltam --, e so depois o veredito da resposta. Esta casa ja pagou por
//! prova que conferia o veredito depois do estrago.
#![cfg(unix)]

mod comum;
use comum::{porta_do_phxsqld, DirTemp, Filho};

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;
use phxsql_server::{Config, Servidor};
use phxsql_store::catalogo::Instancia;
use phxsql_store::cofre;
use phxsql_store::table::{Table, Visao};

const TOKEN: &str = "migracao-da-cifra";
const SENHA_DO_COFRE: &str = "a chave do cofre de teste";
const SENHA: &str = "segredo-de-teste";
const SEGREDO: &str = "Fulano de Tal da Silva";

/// O cofre e global ao processo.
static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

fn esquema(por_volume: Option<u64>) -> Schema {
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
    match por_volume {
        Some(n) => e.com_paginacao(Paginacao::nova(n, 60).unwrap()).unwrap(),
        None => e,
    }
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("{SEGREDO} {i:05}")),
        Value::Str(format!("{:03}.456.789-00", i % 1000)),
        Value::Str("Blumenau".into()),
    ]
}

/// A base `loja` com a tabela `clientes` EM CLARO (v4) e as colunas marcadas:
/// o cofre esta desligado ao nascer. Quem chama liga o cofre depois.
fn base_em_claro(rotulo: &str, n: i64, por_volume: Option<u64>) -> DirTemp {
    cofre::desligar();
    let d = DirTemp::novo(&format!("268-{rotulo}"));
    let cat = Instancia::nova(&d).unwrap();
    let db = cat.criar_database("loja").unwrap();
    let mut t = db.criar_tabela(None, esquema(por_volume)).unwrap();
    for i in 1..=n {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    d
}

fn volumes(base: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(base.join("loja"))
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

fn versao_do(arquivo: &Path) -> u16 {
    let b = std::fs::read(arquivo).unwrap();
    u16::from_le_bytes([b[8], b[9]])
}

fn versao(base: &Path) -> u16 {
    versao_do(&volumes(base)[0])
}

fn bytes_do_reg(base: &Path) -> Vec<u8> {
    volumes(base)
        .iter()
        .flat_map(|p| std::fs::read(p).unwrap())
        .collect()
}

fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
    palheiro.windows(agulha.len()).any(|j| j == agulha)
}

fn novos(base: &Path) -> Vec<String> {
    std::fs::read_dir(base.join("loja"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| f.ends_with(".novo"))
        .collect()
}

/// Sobe o servidor em processo, com a cifra LIGADA pelo `config.json` (e a
/// lista `cifra.tabelas` declarada, quando pedida) e os `usuarios` dados.
fn subir(base: &Path, usuarios: &str, declarar: bool) -> (Arc<Servidor>, u16) {
    let tabelas = if declarar {
        r#""tabelas": ["loja.clientes"],"#
    } else {
        ""
    };
    let texto = format!(
        r#"{{ "bind": "127.0.0.1:0", "base": {base:?}, "token": "{TOKEN}",
              "log_acessos": {log:?}, "blacklist": {bl:?}, "dblink": {dbl:?},
              {usuarios}
              "cifra": {{ {tabelas} "ligada": true, "senha": "{SENHA_DO_COFRE}",
                         "iteracoes": 10000 }},
              "cifra_fio": {{ "exigir": false }},
              "recursos": {{ "durabilidade": "sistema" }},
              "web": {{ "ligado": false }} }}"#,
        base = base.display().to_string(),
        log = base.join("acessos.log").display().to_string(),
        bl = base.join("blacklist.json").display().to_string(),
        dbl = base.join("dblink.json").display().to_string(),
    );
    let c = Config::de_json(&Json::analisar(&texto).unwrap()).unwrap();
    // `de_json` nao liga o cofre: quem liga e o `Config::ler` do arranque
    // real, pelo mesmo `aplicar` que o teste chama aqui.
    c.cifra.aplicar().unwrap();
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return (s, porta);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("o servidor nao subiu na porta {porta}");
}

struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn nova(porta: u16) -> Ligacao {
        let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let f = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
        f.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
        Ligacao {
            escrita: f.try_clone().unwrap(),
            leitor: BufReader::new(f),
        }
    }

    fn entrar(porta: u16, login: &str) -> Ligacao {
        let mut c = Ligacao::nova(porta);
        let r = c.pedir(&format!(
            r#""op":"login","usuario":"{login}","senha":"{SENHA}""#
        ));
        assert!(
            r.booleano_ou("ok", false),
            "login de {login}: {}",
            r.escrever()
        );
        c
    }

    fn pedir(&mut self, corpo: &str) -> Json {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"))
    }
}

fn ok(j: Json) -> Json {
    assert!(
        j.booleano_ou("ok", false),
        "o pedido falhou: {}",
        j.escrever()
    );
    j.campo("resultado").cloned().unwrap_or(j)
}

fn recusou(j: &Json) -> String {
    assert!(
        !j.booleano_ou("ok", false),
        "devia ter recusado e aceitou: {}",
        j.escrever()
    );
    j.escrever()
}

/// Quantas linhas voltam do `varrer` PELO SERVIDOR e se a do meio e a mesma.
fn varrer_tudo(c: &mut Ligacao, n: usize) {
    let r = ok(c.pedir(&format!(
        r#""op":"varrer","database":"loja","tabela":"clientes","limite":{}"#,
        n + 10
    )));
    let linhas = r.campo("linhas").and_then(Json::lista).expect("sem linhas");
    assert_eq!(linhas.len(), n, "o servidor devolveu outra contagem");
    let texto = r.escrever();
    assert!(texto.contains(&format!("{SEGREDO} 00001")), "linha 1 sumiu");
    assert!(texto.contains(&format!("{SEGREDO} {n:05}")), "ultima sumiu");
}

// ---------------------------------------------------------------------------
// o op, o SQL e o comportamento velho
// ---------------------------------------------------------------------------

/// **Declarar nao cifra; PEDIR cifra** -- pelo `op` e pelo SQL, que chamam o
/// mesmo nucleo.
///
/// Com `cifra.tabelas` declarando a tabela, o arranque e o uso nao tocam o
/// disco (o comportamento velho, ao byte); `criptografar` leva a v5 e o
/// segredo SAI dos bytes; `ALTER TABLE ... DECRYPT` (SQL) devolve ao claro;
/// `ALTER TABLE ... ENCRYPT` cifra de novo. A resposta diz em voz alta o que
/// a migracao nao alcanca.
///
/// # Prova real
///
/// Com o ramo `criptografar` do despacho apontando para a funcao do
/// `marcar_lgpd`, ou com o portao do SQL fora, o disco nao muda de versao e a
/// primeira asserção depois do pedido reprova.
#[test]
fn declarar_nao_cifra_e_pedir_cifra_pelo_op_e_pelo_sql() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = base_em_claro("op-e-sql", 40, None);
    let antes = bytes_do_reg(&d);
    let (_s, porta) = subir(&d, "", true);
    let mut c = Ligacao::nova(porta);

    // O comportamento VELHO: declarada em `cifra.tabelas`, o arranque e uma
    // leitura nao tocam um byte do `.reg`.
    varrer_tudo(&mut c, 40);
    assert_eq!(versao(&d), 4);
    assert_eq!(
        bytes_do_reg(&d),
        antes,
        "declarar cifra.tabelas mexeu no disco"
    );

    // O op PEDIDO cifra.
    let r = ok(c.pedir(r#""op":"criptografar","database":"loja","tabela":"clientes""#));
    assert!(r.booleano_ou("cifrada", false), "{}", r.escrever());
    assert_eq!(r.inteiro_ou("versao", 0), 5);
    assert_eq!(r.inteiro_ou("slots_reescritos", 0), 40);
    let avisos = r.campo("avisos").and_then(Json::lista).expect("sem avisos");
    let aviso = avisos[0].texto().unwrap_or("");
    assert!(
        aviso.contains(".log") && aviso.contains(".ndx") && aviso.contains("replica"),
        "a resposta nao diz em voz alta o que a migracao nao alcanca: {aviso}"
    );
    assert_eq!(versao(&d), 5);
    assert!(
        !contem(&bytes_do_reg(&d), SEGREDO.as_bytes()),
        "o segredo continua legivel no .reg depois de criptografar"
    );
    assert!(novos(&d).is_empty());
    varrer_tudo(&mut c, 40);

    // Cifrar o que ja esta cifrado: recusa com motivo, sem mexer.
    let cifrado = bytes_do_reg(&d);
    let r = recusou(&c.pedir(r#""op":"criptografar","database":"loja","tabela":"clientes""#));
    assert!(r.contains("ja esta cifrada"), "{r}");
    assert_eq!(bytes_do_reg(&d), cifrado);

    // O SQL e a MESMA porta: DECRYPT devolve ao claro...
    let r = ok(c.pedir(r#""op":"sql","database":"loja","texto":"ALTER TABLE clientes DECRYPT""#));
    assert_eq!(r.texto_ou("op", ""), "descriptografar", "{}", r.escrever());
    assert_eq!(versao(&d), 4);
    assert!(contem(&bytes_do_reg(&d), SEGREDO.as_bytes()));
    varrer_tudo(&mut c, 40);
    // ... e ENCRYPT cifra de novo.
    ok(c.pedir(r#""op":"sql","database":"loja","texto":"ALTER TABLE clientes ENCRYPT""#));
    assert_eq!(versao(&d), 5);
    varrer_tudo(&mut c, 40);
}

/// Recusa com motivo ANTES de gravar byte: tabela sem coluna marcada.
#[test]
fn sem_coluna_marcada_a_recusa_diz_o_motivo_e_o_disco_fica() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = DirTemp::novo("268-sem-marca");
    {
        let cat = Instancia::nova(&d).unwrap();
        let db = cat.criar_database("loja").unwrap();
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
        let mut t = db.criar_tabela(None, sem_marca).unwrap();
        t.inserir(&[Value::Int(1), Value::Str("a".into())]).unwrap();
        t.sincronizar().unwrap();
    }
    let antes = bytes_do_reg(&d);
    let (_s, porta) = subir(&d, "", false);
    let mut c = Ligacao::nova(porta);
    let r = recusou(&c.pedir(r#""op":"criptografar","database":"loja","tabela":"clientes""#));
    assert!(
        r.contains("nada a cifrar"),
        "a recusa nao diz o motivo: {r}"
    );
    assert_eq!(bytes_do_reg(&d), antes);
    assert!(novos(&d).is_empty());
}

// ---------------------------------------------------------------------------
// a transacao viva e o portao unico
// ---------------------------------------------------------------------------

/// Transacao viva NA PROPRIA tabela -> `EM_TRANSACAO`, quem cede e a
/// migracao, e o disco nao muda. Solta a transacao, a migracao passa -- o
/// portao nao recusa tudo.
///
/// # O que este teste NAO prova, e quem prova
///
/// Este caso o portao 5 do `despachar` ja cobre (a tabela do pedido), entao
/// ele cai junto com qualquer um dos dois. A pergunta `transacao_na_vizinhanca`
/// do roteiro so se mede na tabela LIGADA pela chave -- o teste ao lado, e a
/// guarda `migracao-da-cifra-sem-pergunta-de-transacao` mediu isso (este
/// passava com a pergunta tirada: 0/1 cairam).
#[test]
fn transacao_viva_barra_a_migracao_e_solta_libera() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = base_em_claro("transacao", 10, None);
    let antes = bytes_do_reg(&d);
    let (_s, porta) = subir(&d, "", false);
    let mut b = Ligacao::nova(porta);
    ok(b.pedir(r#""op":"begin","database":"loja""#));
    ok(b.pedir(
        r#""op":"inserir","database":"loja","tabela":"clientes",
           "linha":{"id":11,"nome":"na transacao","cpf":"1","cidade":"x"}"#,
    ));

    let mut a = Ligacao::nova(porta);
    let r = a.pedir(r#""op":"criptografar","database":"loja","tabela":"clientes""#);
    recusou(&r);
    assert_eq!(r.texto_ou("nome", ""), "EM_TRANSACAO", "{}", r.escrever());
    assert_eq!(bytes_do_reg(&d), antes, "a recusa mexeu no disco");
    assert!(novos(&d).is_empty());

    ok(b.pedir(r#""op":"rollback""#));
    ok(a.pedir(r#""op":"criptografar","database":"loja","tabela":"clientes""#));
    assert_eq!(versao(&d), 5);
}

/// **A VIZINHANCA, e e ela que o `transacao_na_vizinhanca` do roteiro cobre:**
/// a transacao empilha escrita numa FILHA (chave estrangeira para
/// `clientes`), e a migracao de `clientes` cede -- o portao 5 do `despachar`
/// so olha o campo `tabela` do pedido e nao ve a filha. Quem cede e a
/// migracao, nunca a transacao (D5 do 426), e o disco nao muda.
///
/// # Prova real
///
/// Sem a pergunta no roteiro (`op_migrar_cifra`), a migracao congela
/// `clientes` debaixo da transacao da filha e a resposta e `ok`: a asserção
/// de `EM_TRANSACAO` reprova -- e foi isso que a guarda
/// `migracao-da-cifra-sem-pergunta-de-transacao` mediu (o teste da MESMA
/// tabela passava com o defeito reposto, porque o portao 5 o cobre).
#[test]
fn transacao_na_tabela_ligada_pela_chave_barra_a_migracao() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = base_em_claro("vizinhanca", 10, None);
    let antes = bytes_do_reg(&d);
    let (_s, porta) = subir(&d, "", false);
    let mut b = Ligacao::nova(porta);
    ok(b.pedir(
        r#""op":"criar_tabela","database":"loja","tabela":"filha",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                      {"nome":"cliente_id","tipo":"Int8"}],
           "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true},
                      {"nome":"por_cliente","colunas":["cliente_id"]}],
           "chaves_estrangeiras":[{"nome":"fk_cliente","colunas":["cliente_id"],
                                   "tabela_ref":"clientes","colunas_ref":["id"]}]"#,
    ));
    ok(b.pedir(r#""op":"begin","database":"loja""#));
    ok(b.pedir(
        r#""op":"inserir","database":"loja","tabela":"filha",
           "linha":{"id":1,"cliente_id":1}"#,
    ));

    let mut a = Ligacao::nova(porta);
    let r = a.pedir(r#""op":"criptografar","database":"loja","tabela":"clientes""#);
    recusou(&r);
    assert_eq!(r.texto_ou("nome", ""), "EM_TRANSACAO", "{}", r.escrever());
    assert!(
        r.texto_ou("erro", "").contains("filha"),
        "a recusa tinha de nomear a tabela que a transacao segura: {}",
        r.escrever()
    );
    assert_eq!(bytes_do_reg(&d), antes, "a recusa mexeu no disco");
    assert!(novos(&d).is_empty());

    // Solta a transacao, a migracao passa.
    ok(b.pedir(r#""op":"rollback""#));
    ok(a.pedir(r#""op":"criptografar","database":"loja","tabela":"clientes""#));
    assert_eq!(versao(&d), 5);
}

/// O portao e UM, e o SQL nao e porta dos fundos: a op `sql` so exige `ler`,
/// e `ALTER TABLE ... ENCRYPT` precisa de `administrar` na tabela.
///
/// # Prova real
///
/// Com o ramo do SQL chamando `executar` (como as diretivas) em vez do
/// `executar_derivado`, o `so_le` cifra a tabela pelo SQL e a versao do disco
/// vira 5 -- a asserção de `versao == 4` reprova.
#[test]
fn quem_nao_administra_nao_migra_nem_pelo_sql() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = base_em_claro("portao", 8, None);
    let h = phxsql_core::senha::cifrar_com(SENHA, 1);
    let usuarios = format!(
        r#""root": {{ "login": "root", "senha_hash": "{h}" }},
           "usuarios": [
             {{ "id": 2, "login": "so_le", "nome": "So Le", "senha_hash": "{h}",
                "ativo": true, "bases": {{ "loja": {{ "ler": true }} }} }},
             {{ "id": 3, "login": "administra", "nome": "Administra",
                "senha_hash": "{h}", "ativo": true,
                "bases": {{ "loja": {{ "ler": true, "administrar": true }} }} }} ],"#
    );
    let (_s, porta) = subir(&d, &usuarios, false);

    let mut le = Ligacao::entrar(porta, "so_le");
    let r = recusou(&le.pedir(r#""op":"criptografar","database":"loja","tabela":"clientes""#));
    assert!(
        r.contains("administrar"),
        "a recusa nao diz o que falta: {r}"
    );
    recusou(&le.pedir(r#""op":"sql","database":"loja","texto":"ALTER TABLE clientes ENCRYPT""#));
    assert_eq!(versao(&d), 4, "quem so le cifrou a tabela");
    assert!(novos(&d).is_empty());

    // O comportamento velho: quem administra migra.
    let mut adm = Ligacao::entrar(porta, "administra");
    ok(adm.pedir(r#""op":"sql","database":"loja","texto":"ALTER TABLE clientes ENCRYPT""#));
    assert_eq!(versao(&d), 5);
}

// ---------------------------------------------------------------------------
// (10) a prova no SO: SIGKILL em ponto aleatorio, pelo soquete
// ---------------------------------------------------------------------------

fn copiar_pasta(de: &Path, para: &Path) {
    std::fs::create_dir_all(para).unwrap();
    for e in std::fs::read_dir(de).unwrap().flatten() {
        let alvo = para.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copiar_pasta(&e.path(), &alvo);
        } else {
            std::fs::copy(e.path(), alvo).unwrap();
        }
    }
}

/// Sobe o `phxsqld` de VERDADE sobre `base`, com a cifra ligada pelo config.
fn subir_o_phxsqld(base: &Path) -> (Filho, u16) {
    let config = base.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{ "bind": "127.0.0.1:0", "token": "{TOKEN}", "base": {b:?},
                  "cifra": {{ "ligada": true, "senha": "{SENHA_DO_COFRE}",
                              "iteracoes": 10000 }},
                  "cifra_fio": {{ "exigir": false }}, "web": {{ "ligado": false }} }}"#,
            b = base.join("dados").display().to_string(),
        ),
    )
    .unwrap();
    let erro_padrao = base.join("stderr.txt");
    let mut filho = Filho(
        Command::new(env!("CARGO_BIN_EXE_phxsqld"))
            .arg("--config")
            .arg(&config)
            .current_dir(base)
            .stdout(Stdio::null())
            .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
            .spawn()
            .expect("nao consegui iniciar o phxsqld"),
    );
    let porta = porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    (filho, porta)
}

/// O que tem de valer depois de QUALQUER queda: a tabela abre, todas as linhas
/// batem, todo volume esta na MESMA versao e nao sobra `*.novo`. Devolve a
/// versao em que ela ficou.
fn conferir_depois_da_queda(dados: &Path, n: i64) -> u16 {
    let dir = dados.join("loja");
    {
        let mut t = Table::abrir(&dir, "clientes")
            .unwrap_or_else(|e| panic!("a tabela nao abre depois do SIGKILL: {e}"));
        let linhas = t.varrer_com(Visao::Todas).unwrap();
        assert_eq!(linhas.len() as i64, n, "o SIGKILL perdeu linhas");
        for (i, (rowid, v)) in linhas.iter().enumerate() {
            assert_eq!(*rowid, i as u64 + 1);
            assert_eq!(
                v[1],
                Value::Str(format!("{SEGREDO} {:05}", i + 1)),
                "o rowid {rowid} deixou de ser a mesma linha"
            );
        }
    }
    let versoes: std::collections::BTreeSet<u16> =
        volumes(dados).iter().map(|p| versao_do(p)).collect();
    assert_eq!(versoes.len(), 1, "a tabela ficou MISTURADA: {versoes:?}");
    assert!(
        novos(dados).is_empty(),
        "sobrou *.novo depois da abertura gravavel: {:?}",
        novos(dados)
    );
    *versoes.iter().next().unwrap()
}

/// **SIGKILL em ponto aleatorio**, pelo soquete, nos dois sentidos, numa
/// tabela PAGINADA de 40 volumes (a janela da FASE B tem 40 `rename`).
///
/// Cada rodada sobe um `phxsqld` de verdade, manda o pedido e mata o processo
/// depois de um atraso diferente (0 a ~170 ms) -- no PBKDF2, na FASE A, entre
/// dois `rename`, ou depois de tudo. Em TODAS: a tabela abre, 1.600 linhas
/// batem, uma versao so, sem `*.novo`; e repetir a migracao depois da queda
/// termina o trabalho.
///
/// O desfecho de cada rodada (velha ou nova) e impresso: o teste nao exige
/// distribuicao nenhuma, porque ela depende da maquina -- a janela entre dois
/// `rename` tem a prova deterministica em `tests/migracao-da-cifra.rs`
/// (`FaseBDepoisDoVolume1`). Esta e a prova de que o SISTEMA OPERACIONAL, e
/// nao um panico de teste, nao deixa a tabela pela metade.
///
/// # Prova real
///
/// Com a FASE B trocando cada volume logo depois de escreve-lo (o defeito que
/// o 632 consertou), um SIGKILL na janela deixa o volume 1 novo e o 2 velho,
/// sem o `*.novo` que o terminaria: a tabela nao abre.
#[test]
fn sigkill_em_ponto_aleatorio_nunca_deixa_a_tabela_pela_metade() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    const N: i64 = 1_600;
    // O modelo EM CLARO, e o modelo CIFRADO (migrado aqui, em processo).
    let claro = base_em_claro("sigkill-modelo", N, Some(40));
    let cifrado = DirTemp::novo("268-sigkill-cifrado");
    copiar_pasta(&claro, &cifrado);
    cofre::definir(SENHA_DO_COFRE, 10_000).unwrap();
    {
        let mut t = Table::abrir(cifrado.join("loja"), "clientes").unwrap();
        let troca = t.preparar_migracao_da_cifra(true).unwrap();
        t.aplicar_migracao_da_cifra(troca).unwrap();
    }
    assert_eq!(versao_do(&volumes(&cifrado)[0]), 5);

    let mut desfechos = Vec::new();
    for (cifrar, modelo) in [(true, &claro), (false, &cifrado)] {
        let (de, para) = if cifrar { (4, 5) } else { (5, 4) };
        let sentido = if cifrar {
            "criptografar"
        } else {
            "descriptografar"
        };
        for passo in 0..16u64 {
            let rodada = DirTemp::novo(&format!("268-sigkill-{sentido}-{passo}"));
            let dados = rodada.join("dados");
            copiar_pasta(modelo, &dados);
            let (mut filho, porta) = subir_o_phxsqld(&rodada);

            let pedido = format!(
                r#"{{"token":"{TOKEN}","op":"{sentido}","database":"loja","tabela":"clientes"}}"#
            );
            let fio = std::thread::spawn(move || {
                let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
                let Ok(f) = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)) else {
                    return String::new();
                };
                let _ = f.set_read_timeout(Some(Duration::from_secs(20)));
                let mut escrita = f.try_clone().unwrap();
                let _ = writeln!(escrita, "{pedido}");
                let mut r = String::new();
                let _ = BufReader::new(f).read_line(&mut r);
                r
            });
            std::thread::sleep(Duration::from_millis(passo * 11));
            let _ = filho.0.kill();
            let _ = filho.0.wait();
            let resposta = fio.join().unwrap();

            // Antes de abrir: a queda deixou o conjunto MISTURADO (volume 1
            // trocado, outros nao)? E o estado que so a abertura sabe terminar.
            let misturado = volumes(&dados)
                .iter()
                .map(|p| versao_do(p))
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                > 1;
            let ficou = conferir_depois_da_queda(&dados, N);
            assert!(
                ficou == de || ficou == para,
                "versao {ficou} nao e nem a de partida ({de}) nem a de chegada ({para})"
            );
            desfechos.push((
                sentido,
                passo,
                ficou == para,
                resposta.contains("\"ok\":true"),
                misturado,
            ));

            // Repetir a migracao depois da queda termina o trabalho.
            let mut t = Table::abrir(dados.join("loja"), "clientes").unwrap();
            if ficou == de {
                let troca = t.preparar_migracao_da_cifra(cifrar).unwrap();
                t.aplicar_migracao_da_cifra(troca).unwrap();
            }
            drop(t);
            assert_eq!(versao_do(&volumes(&dados)[0]), para);
            assert_eq!(conferir_depois_da_queda(&dados, N), para);
        }
    }
    let chegou = desfechos.iter().filter(|d| d.2).count();
    eprintln!(
        "SIGKILL: {} rodadas, {} terminaram na versao nova, {} voltaram a velha, {} responderam ok antes da queda, {} cairam com o conjunto MISTURADO (a abertura terminou para a frente)",
        desfechos.len(),
        chegou,
        desfechos.len() - chegou,
        desfechos.iter().filter(|d| d.3).count(),
        desfechos.iter().filter(|d| d.4).count()
    );
    cofre::desligar();
}
