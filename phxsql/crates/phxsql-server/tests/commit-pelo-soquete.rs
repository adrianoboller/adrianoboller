//! O `COMMIT` pelo soquete: o que ele grava, o que ele promete, e o que ele
//! manda o cliente fazer depois -- pedidos 426 e 262.
//!
//! # Pedido 426 -- o `COMMIT` que esbarra numa reescrita sai PELA METADE
//!
//! A cadeia, sem malicia nenhuma: B abre transacao e empilha escrita em duas
//! tabelas; A pede uma reescrita (`migrar_esquema`, `acrescentar_coluna`) e o
//! portao das travas de transacao confere so a tabela do PEDIDO, e FORA da
//! trava global; a reescrita congela a tabela e solta a trava global para a
//! FASE A; B da `COMMIT` -- a marca vai ao disco (a transacao esta
//! confirmada), a primeira tabela grava, a segunda bate no congelamento e a
//! passada aborta. Tres camadas, e cada uma tem o seu vermelho aqui:
//!
//! - **(a)** uma tabela com a linha e a outra sem;
//! - **(b)** a resposta manda REPETIR uma transacao meio aplicada -- o
//!   cliente que obedece duplica o que ja gravou;
//! - **(c)** a recuperacao do braco de erro nunca roda: o `*.tx` fica, e a
//!   transacao que o cliente viu FALHAR aparece aplicada no proximo arranque.
//!
//! O contrato saiu medido nos quatro motores
//! (`docs/propostas/commit-contra-ddl-4-motores.md` §5): quem cede e a
//! REESCRITA, nunca a transacao; e «repita» so se diz sobre transacao que
//! aplicou ZERO.
//!
//! # Por que a prova mede o DANO, e nao o veredito
//!
//! Esta casa ja pagou por prova que conferia o veredito depois do estrago. Aqui
//! cada teste conta as linhas no disco, procura a marca `*.tx` e roda a
//! recuperacao do arranque -- e so entao julga a resposta.
//!
//! # Pedido 262 -- o `AFTER` que grava no `COMMIT` sumia calado
//!
//! As conferencias C1, C2, C4 e C5 do parecer
//! (`docs/propostas/parecer-j-262-gatilho-after-no-commit-2026-09-23.md` §7.3).
//! A C3 e da ETAPA 2 e nao mora aqui.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_server::{Config, Servidor};
use phxsql_store::catalogo::Instancia;
use phxsql_store::Table;

const TOKEN: &str = "commit-pelo-soquete";

fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Sobe o servidor sem cadastro. `sistema` na durabilidade pelo mesmo motivo
/// da prova da trava do `porta-do-psch-v10.rs`: com `por_lote` cada gravacao
/// paga `fsync`, e a prova mediria o disco em vez da janela.
fn subir(base: &Path) -> (Arc<Servidor>, u16) {
    let porta = porta_livre();
    let texto = format!(
        r#"{{ "bind": "127.0.0.1:{porta}", "base": {base:?}, "token": "{TOKEN}",
              "log_acessos": {log:?}, "blacklist": {bl:?}, "dblink": {dbl:?},
              "cifra_fio": {{ "exigir": false }},
              "recursos": {{ "durabilidade": "sistema" }},
              "web": {{ "ligado": false }} }}"#,
        base = base.display().to_string(),
        log = base.join("acessos.log").display().to_string(),
        bl = base.join("blacklist.json").display().to_string(),
        dbl = base.join("dblink.json").display().to_string(),
    );
    let c = Config::de_json(&Json::analisar(&texto).unwrap()).unwrap();
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
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

    /// O protocolo e JSON por LINHA: o corpo escrito em varias linhas aqui
    /// vira uma so antes de sair.
    fn pedir(&mut self, corpo: &str) -> Json {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"))
    }
}

/// O corpo da resposta, exigindo que o pedido tenha dado certo.
fn ok(j: Json) -> Json {
    assert!(
        j.booleano_ou("ok", false),
        "o pedido falhou: {}",
        j.escrever()
    );
    j.campo("resultado").cloned().unwrap_or(j)
}

/// O corpo, se deu certo; a resposta inteira, se nao deu. Para quem precisa
/// ler os dois lados sem entrar em panico no meio da medicao.
fn corpo(j: &Json) -> Json {
    if j.booleano_ou("ok", false) {
        j.campo("resultado").cloned().unwrap_or_else(|| j.clone())
    } else {
        j.clone()
    }
}

/// Quantas linhas quem consulta VE. Pelo `varrer`, que toma a ficha
/// COMPARTILHADA e por isso le ate tabela congelada.
fn contar(c: &mut Ligacao, tabela: &str) -> u64 {
    let r = ok(c.pedir(&format!(
        r#""op":"varrer","database":"loja","tabela":"{tabela}","max":1000"#
    )));
    r.campo("linhas")
        .and_then(Json::lista)
        .map(|l| l.len() as u64)
        .unwrap_or(0)
}

/// As marcas `.tx` que sobraram no diretorio do database.
fn marcas(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|it| {
            it.filter_map(|e| e.ok())
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n.starts_with("transacao_") && n.ends_with(".tx")
                })
                .count()
        })
        .unwrap_or(0)
}

/// O esquema de uma tabela ANTERIOR ao `PSCH` v10 -- a que o `migrar_esquema`
/// tem o que reescrever.
fn esquema_v9(nome: &str) -> Schema {
    Schema::do_disco(
        nome.to_string(),
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
            Column::new(phxsql_core::schema::COLUNA_SOFTDELETED, ColumnType::Bool).obrigatoria(),
            Column::new(phxsql_core::schema::COLUNA_ROWNUM, ColumnType::UInt8).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn criar_v9(base: &Path, tabela: &str, linhas: i64) {
    let mut t = Table::criar(base.join("loja"), esquema_v9(tabela)).unwrap();
    for i in 1..=linhas {
        t.inserir(&[Value::Int(i), Value::Str(format!("linha {i}"))])
            .unwrap();
    }
    t.sincronizar().unwrap();
}

/// `mae` (v9, `linhas` linhas), `filha` apontando para ela por chave
/// conferida, `outra` sem chave nenhuma, e `solta` -- sem chave nenhuma e sem
/// relacao com as tres, para provar que a reescrita nao cede a quem nao a
/// alcanca.
fn preparar(c: &mut Ligacao, base: &Path, linhas: i64) {
    ok(c.pedir(r#""op":"criar_database","database":"loja""#));
    criar_v9(base, "mae", linhas);
    ok(c.pedir(
        r#""op":"criar_tabela","database":"loja","tabela":"filha",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                      {"nome":"mae_id","tipo":"Int8"}],
           "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true},
                      {"nome":"por_mae","colunas":["mae_id"]}],
           "chaves_estrangeiras":[{"nome":"fk_mae","colunas":["mae_id"],
                                   "tabela_ref":"mae","colunas_ref":["id"]}]"#,
    ));
    for tabela in ["outra", "solta"] {
        ok(c.pedir(&format!(
            r#""op":"criar_tabela","database":"loja","tabela":"{tabela}",
               "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                          {{"nome":"nome","tipo":"Str(20)"}}],
               "indices":[{{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}}]"#
        )));
    }
    // A primeira abertura de uma tabela recem-criada pode pedir a ficha
    // exclusiva (curar o `.log`, criar o `.fts`) -- sem esta volta a prova
    // mediria isso em vez do commit.
    for tabela in ["mae", "filha", "outra", "solta"] {
        contar(c, tabela);
    }
}

/// O retrato do que a transacao deixou, tirado DEPOIS de a reescrita acabar.
struct Retrato {
    /// Linhas da transacao que estao no disco agora (de `alvos` possiveis).
    aplicadas: u64,
    alvos: u64,
    /// A resposta do COMMIT mandou repetir?
    repetir: bool,
    /// A resposta do COMMIT disse `COMMITTED`?
    confirmado: bool,
    /// Marcas `.tx` sobrando depois da resposta.
    marcas: usize,
    /// Linhas da transacao depois da RECUPERACAO do arranque.
    depois_do_arranque: u64,
}

impl Retrato {
    /// As tres camadas do 426, cada uma dita no proprio nome -- e juntas, para
    /// o vermelho mostrar TODO o estrago de uma vez, e nao so o primeiro.
    fn defeitos(&self) -> Vec<String> {
        let mut d = Vec::new();
        if self.aplicadas != 0 && self.aplicadas != self.alvos {
            d.push(format!(
                "(a) ATOMICIDADE: {} de {} linhas da transacao no disco",
                self.aplicadas, self.alvos
            ));
        }
        if self.repetir && self.aplicadas > 0 {
            d.push(format!(
                "(b) REPETIR depois de aplicar: a resposta manda repetir e ja ha {} \
                 linha(s) da transacao gravada(s) -- quem obedece duplica",
                self.aplicadas
            ));
        }
        // Marca depois de um COMMIT CONFIRMADO e a janela de durabilidade
        // aberta (`marcas_pendentes`): quem a apaga e o `fsync`, e ela e
        // legitima. Marca depois de uma RECUSA e trabalho que o arranque vai
        // aplicar pelas costas de quem ouviu «falhou».
        if self.marcas > 0 && !self.confirmado {
            d.push(format!(
                "(c) RECUPERACAO QUE NAO RODOU: {} marca(s) .tx sobrando depois de \
                 uma resposta que NAO confirmou",
                self.marcas
            ));
        }
        if self.depois_do_arranque != self.aplicadas {
            d.push(format!(
                "(c) O ARRANQUE MUDA O PASSADO: {} linha(s) antes, {} depois da \
                 recuperacao -- a transacao que o cliente viu falhar aparece aplicada",
                self.aplicadas, self.depois_do_arranque
            ));
        }
        d
    }
}

/// Conta as linhas da transacao (em `outra` e na tabela `segunda`), as marcas,
/// e roda a recuperacao do arranque -- a mesma funcao que `Servidor::novo`
/// chama -- para ver se o passado muda.
///
/// Por uma conexao PROPRIA, sem transacao: a de B enxerga as proprias escritas
/// pendentes (read-your-own-writes), e contar por ela mediria a lista em RAM
/// em vez do disco.
fn retratar(porta: u16, base: &Path, segunda: &str, commit: &Json) -> Retrato {
    let mut fora = Ligacao::nova(porta);
    let c = &mut fora;
    let alvo_segunda = |c: &mut Ligacao| -> u64 {
        match segunda {
            // Na `mae` a transacao acrescentou UMA linha as que ja existiam.
            "mae" => {
                let r = ok(c.pedir(
                    r#""op":"buscar","database":"loja","tabela":"mae","indice":"porId","chave":[900000]"#,
                ));
                r.campo("linhas")
                    .and_then(Json::lista)
                    .map(|l| l.len() as u64)
                    .unwrap_or(0)
            }
            outra => contar(c, outra),
        }
    };
    let aplicadas = contar(c, "outra") + alvo_segunda(c);
    let marcas = marcas(&base.join("loja"));
    let inst = Instancia::nova(base).unwrap();
    let _ = phxsql_server::transacao::recuperar(&inst);
    let depois_do_arranque = contar(c, "outra") + alvo_segunda(c);
    Retrato {
        aplicadas,
        alvos: 2,
        repetir: commit.booleano_ou("repetir", false),
        confirmado: corpo(commit).texto_ou("transaction_state", "") == "COMMITTED",
        marcas,
        depois_do_arranque,
    }
}

// ------------------------------------------------------------------ pedido 426

/// **426 pela cadeia do pedido, sem corrida nenhuma: A congela UMA das duas
/// tabelas da transacao de B.**
///
/// O congelamento aqui e o do armazem, posto por fora -- o mesmo registro que
/// a reescrita poe, na mesma chave. E o jeito de o estado do defeito
/// (transacao com escrita numa tabela congelada, na hora do `COMMIT`) nascer
/// deterministico, em vez de depender de uma janela de microssegundos entre o
/// portao e a trava.
///
/// Depois do conserto a recusa vem ANTES da marca: nada gravado, a transacao
/// continua ativa, e o segundo `COMMIT` -- depois de a reescrita acabar --
/// grava as duas.
#[test]
fn o_commit_contra_a_tabela_congelada_nao_sai_pela_metade() {
    let base = DirTemp::novo("426-congelada");
    let (_s, porta) = subir(&base);
    let mut b = Ligacao::nova(porta);
    preparar(&mut b, &base, 3);

    ok(b.pedir(r#""op":"begin","database":"loja""#));
    ok(b.pedir(r#""op":"inserir","database":"loja","tabela":"outra","linha":{"id":1,"nome":"b"}"#));
    ok(b.pedir(
        r#""op":"inserir","database":"loja","tabela":"mae","linha":{"id":900000,"nome":"b"}"#,
    ));

    let posse =
        phxsql_store::congelamento::congelar(&base.join("loja"), "mae", "prova do pedido 426")
            .unwrap();
    let commit = b.pedir(r#""op":"commit""#);
    drop(posse);

    let r = retratar(porta, &base, "mae", &commit);
    let defeitos = r.defeitos();
    assert!(
        defeitos.is_empty(),
        "o COMMIT contra a tabela congelada:\n  {}\nresposta do COMMIT: {}",
        defeitos.join("\n  "),
        commit.escrever()
    );

    // O caminho feliz depois do conserto: a recusa foi ANTES da marca, a
    // transacao continua ativa, e o segundo COMMIT grava as duas.
    assert_eq!(
        commit.texto_ou("nome", ""),
        "EM_MIGRACAO",
        "a recusa tinha de nomear a reescrita: {}",
        commit.escrever()
    );
    let segundo = corpo(&b.pedir(r#""op":"commit""#));
    assert_eq!(
        segundo.texto_ou("transaction_state", ""),
        "COMMITTED",
        "a transacao tinha de continuar ativa depois da recusa: {}",
        segundo.escrever()
    );
    assert_eq!(
        segundo.inteiro_ou("gravadas", -1),
        2,
        "{}",
        segundo.escrever()
    );
    let r2 = retratar(porta, &base, "mae", &Json::Nulo);
    assert_eq!(r2.aplicadas, 2, "o segundo COMMIT nao gravou as duas");
    assert_eq!(
        r2.depois_do_arranque, 2,
        "o arranque mudou o segundo COMMIT"
    );
}

/// **426, D3 -- «repita» se diz na INSTRUCAO.** Com a `mae` congelada, a
/// escrita na `filha` -- que o COMMIT abriria junto, na conferencia da chave --
/// recusa AQUI, com zero aplicado e a transacao viva, e nao no COMMIT depois
/// da marca. E o lugar do 1412 `ER_TABLE_DEF_CHANGED` do MySQL.
#[test]
fn a_escrita_na_vizinha_da_congelada_recusa_na_instrucao() {
    let base = DirTemp::novo("426-instrucao");
    let (_s, porta) = subir(&base);
    let mut b = Ligacao::nova(porta);
    preparar(&mut b, &base, 3);

    let posse =
        phxsql_store::congelamento::congelar(&base.join("loja"), "mae", "prova da instrucao")
            .unwrap();
    ok(b.pedir(r#""op":"begin","database":"loja""#));
    let r =
        b.pedir(r#""op":"inserir","database":"loja","tabela":"filha","linha":{"id":1,"mae_id":1}"#);
    drop(posse);
    assert_eq!(
        r.texto_ou("nome", ""),
        "EM_MIGRACAO",
        "a escrita que o COMMIT nao conseguiria aplicar entrou na lista: {}",
        r.escrever()
    );
    assert!(r.booleano_ou("repetir", false), "{}", r.escrever());
    // A transacao continua viva -- erro de instrucao (D4) --, e sem a escrita.
    let c = corpo(&b.pedir(r#""op":"commit""#));
    assert_eq!(
        c.texto_ou("transaction_state", ""),
        "COMMITTED",
        "{}",
        c.escrever()
    );
    assert_eq!(c.inteiro_ou("gravadas", -1), 0, "{}", c.escrever());
    assert_eq!(contar(&mut b, "filha"), 0);
}

/// Corre uma reescrita de verdade contra a transacao de B, que escreveu em
/// `outra` e na `filha` -- a tabela LIGADA a `mae` por chave estrangeira. O
/// portao velho so olhava a tabela do pedido, e a `filha` nao e ela: e o
/// caminho sem corrida nenhuma para o COMMIT abrir a `mae` congelada, na
/// conferencia da chave.
///
/// Devolve (resposta do COMMIT, resposta da reescrita, se a `mae` chegou a
/// congelar com B aberta).
fn corrida_contra_reescrita(pedido: &'static str, rotulo: &str) -> (Retrato, Json, Json, bool) {
    // Grande o bastante para a FASE A durar muitas idas e voltas de soquete:
    // o `COMMIT` de B tem de cair DENTRO dela. Mesmo numero da prova da trava
    // do `porta-do-psch-v10.rs`, medido la.
    const LINHAS: i64 = 100_000;
    let base = DirTemp::novo(rotulo);
    let (_s, porta) = subir(&base);
    let mut b = Ligacao::nova(porta);
    preparar(&mut b, &base, LINHAS);

    ok(b.pedir(r#""op":"begin","database":"loja""#));
    ok(b.pedir(r#""op":"inserir","database":"loja","tabela":"outra","linha":{"id":1,"nome":"b"}"#));
    ok(b.pedir(r#""op":"inserir","database":"loja","tabela":"filha","linha":{"id":1,"mae_id":1}"#));

    let a = std::thread::spawn(move || {
        let mut a = Ligacao::nova(porta);
        a.pedir(pedido)
    });
    // Espera a `mae` congelar -- pela CHAVE dela, e nao pelo contador global:
    // outro teste deste binario pode estar congelando outra tabela agora.
    let mut congelou = false;
    let ate = Instant::now() + Duration::from_secs(60);
    while Instant::now() < ate {
        if phxsql_store::congelamento::conferir(&base.join("loja"), "mae").is_err() {
            congelou = true;
            break;
        }
        if a.is_finished() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let commit = b.pedir(r#""op":"commit""#);
    let reescrita = a.join().unwrap();
    let r = retratar(porta, &base, "filha", &commit);
    (r, corpo(&commit), reescrita, congelou)
}

/// **426 pela reescrita de VERDADE (`migrar_esquema --confirmar`)**, com a
/// transacao escrevendo na tabela que a CHAVE liga a que vai ser migrada.
#[test]
fn a_migracao_cede_a_transacao_que_a_alcanca_pela_chave() {
    let (r, commit, reescrita, congelou) = corrida_contra_reescrita(
        r#""op":"migrar_esquema","database":"loja","tabela":"mae","confirmar":"mae""#,
        "426-migrar",
    );
    let defeitos = r.defeitos();
    assert!(
        defeitos.is_empty(),
        "a migracao contra a transacao aberta (congelou a mae: {congelou}):\n  {}\n\
         COMMIT: {}\nmigracao: {}",
        defeitos.join("\n  "),
        commit.escrever(),
        reescrita.escrever()
    );
    // Quem cede e a REESCRITA (D5 do parecer): ela nao congela, e diz por que.
    assert!(
        !congelou,
        "a mae congelou com a transacao de B viva na filha: o portao nao olha \
         a tabela ligada pela chave. migracao: {}",
        reescrita.escrever()
    );
    assert_eq!(
        reescrita.texto_ou("nome", ""),
        "EM_TRANSACAO",
        "a migracao tinha de ceder dizendo quem segura: {}",
        reescrita.escrever()
    );
    assert!(
        reescrita.texto_ou("erro", "").contains("filha"),
        "a recusa tinha de nomear a tabela que a transacao segura: {}",
        reescrita.escrever()
    );
    assert_eq!(
        commit.texto_ou("transaction_state", ""),
        "COMMITTED",
        "{}",
        commit.escrever()
    );
    assert_eq!(r.aplicadas, 2);
}

/// O IRMAO: `acrescentar_coluna` chama as mesmas funcoes na mesma ordem
/// (`travar_dados` -> `abrir_travada` -> `congelar` -> solta a trava), e um
/// conserto que entrasse so no `migrar_esquema` o deixaria com o defeito.
#[test]
fn o_acrescentar_coluna_tambem_cede_a_transacao() {
    let (r, commit, reescrita, congelou) = corrida_contra_reescrita(
        r#""op":"acrescentar_coluna","database":"loja","tabela":"mae",
           "coluna":{"nome":"extra","tipo":"Int4"}"#,
        "426-acrescentar",
    );
    let defeitos = r.defeitos();
    assert!(
        defeitos.is_empty(),
        "o acrescentar_coluna contra a transacao aberta (congelou a mae: {congelou}):\n  {}\n\
         COMMIT: {}\nreescrita: {}",
        defeitos.join("\n  "),
        commit.escrever(),
        reescrita.escrever()
    );
    assert!(!congelou, "a mae congelou: {}", reescrita.escrever());
    assert_eq!(
        reescrita.texto_ou("nome", ""),
        "EM_TRANSACAO",
        "{}",
        reescrita.escrever()
    );
    assert_eq!(
        commit.texto_ou("transaction_state", ""),
        "COMMITTED",
        "{}",
        commit.escrever()
    );
}

/// **O comportamento VELHO**, e o teste que impede um portao que recusaria
/// tudo: transacao aberta numa tabela SEM ligacao nenhuma com a que vai ser
/// reescrita nao segura a reescrita.
#[test]
fn transacao_em_tabela_sem_ligacao_nao_segura_a_reescrita() {
    let base = DirTemp::novo("426-solta");
    let (_s, porta) = subir(&base);
    let mut b = Ligacao::nova(porta);
    preparar(&mut b, &base, 3);

    ok(b.pedir(r#""op":"begin","database":"loja""#));
    ok(b.pedir(r#""op":"inserir","database":"loja","tabela":"solta","linha":{"id":1,"nome":"b"}"#));

    let mut a = Ligacao::nova(porta);
    let v =
        ok(a.pedir(r#""op":"migrar_esquema","database":"loja","tabela":"mae","confirmar":"mae""#));
    assert!(
        v.booleano_ou("migrado", false),
        "a migracao cedeu a uma transacao que nao a alcanca: {}",
        v.escrever()
    );
    let c = corpo(&b.pedir(r#""op":"commit""#));
    assert_eq!(
        c.texto_ou("transaction_state", ""),
        "COMMITTED",
        "{}",
        c.escrever()
    );
    assert_eq!(contar(&mut b, "solta"), 1);
}

// ------------------------------------------------------------------ pedido 262

/// `clientes` com um `AFTER INSERT` que grava na `auditoria`.
fn com_auditoria(c: &mut Ligacao, com_gatilho: bool) {
    ok(c.pedir(r#""op":"criar_database","database":"loja""#));
    ok(c.pedir(
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                      {"nome":"nome","tipo":"Str(20)"}],
           "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#,
    ));
    ok(c.pedir(
        r#""op":"criar_tabela","database":"loja","tabela":"auditoria",
           "colunas":[{"nome":"id","tipo":"Int8"},{"nome":"ev","tipo":"Str(20)"}]"#,
    ));
    if com_gatilho {
        ok(c.pedir(
            r#""op":"sql","database":"loja","texto":"CREATE TRIGGER audita AFTER INSERT ON
               clientes FOR EACH ROW INSERT INTO auditoria (id, ev) VALUES (NEW.id, 'entrou')""#,
        ));
    }
    contar(c, "clientes");
    contar(c, "auditoria");
}

fn avisos(j: &Json) -> Vec<String> {
    j.campo("gatilhos_avisos")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .map(|a| {
            a.texto()
                .map(str::to_string)
                .unwrap_or_else(|| a.escrever())
        })
        .collect()
}

/// **C1** -- o `AFTER` que grava, disparado no `COMMIT`: ou a auditoria tem a
/// linha, ou a resposta diz, nomeando, por que nao tem. Calado nunca.
#[test]
fn o_after_que_grava_no_commit_nao_some_calado() {
    let base = DirTemp::novo("262-c1");
    let (_s, porta) = subir(&base);
    let mut c = Ligacao::nova(porta);
    com_auditoria(&mut c, true);

    ok(c.pedir(r#""op":"begin","database":"loja""#));
    ok(c.pedir(
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    ));
    let commit = ok(c.pedir(r#""op":"commit""#));
    let na_auditoria = contar(&mut c, "auditoria");
    let avisos = avisos(&commit);
    let nomeado = avisos
        .iter()
        .any(|a| a.contains("audita") && a.contains("COMMIT") && a.contains("NAO foi gravada"));
    assert!(
        na_auditoria == 1 || nomeado,
        "o AFTER gravou no COMMIT e a gravacao sumiu calada: auditoria com \
         {na_auditoria} linha(s), gatilhos_avisos = {avisos:?}; COMMIT: {}",
        commit.escrever()
    );
    // As duas decisoes que o parecer tomou para o papel B nao inventar: o
    // COMMIT continua COMMITTED (falha de AFTER e aviso), e `gravadas` nao
    // conta a linha do gatilho.
    assert_eq!(commit.texto_ou("transaction_state", ""), "COMMITTED");
    assert_eq!(
        commit.inteiro_ou("gravadas", -1),
        1,
        "{}",
        commit.escrever()
    );
    assert_eq!(contar(&mut c, "clientes"), 1);
}

/// **C2 -- o comportamento VELHO, e a mais importante das cinco.** `COMMIT`
/// sem gatilho nenhum: `gravadas: 1`, sem `gatilhos_avisos`. E o irmao que o
/// portao comum alcanca: fora de transacao, o MESMO gatilho continua gravando
/// a auditoria de verdade.
#[test]
fn o_commit_sem_gatilho_nao_muda_nada() {
    let base = DirTemp::novo("262-c2");
    let (_s, porta) = subir(&base);
    let mut c = Ligacao::nova(porta);
    com_auditoria(&mut c, false);

    ok(c.pedir(r#""op":"begin","database":"loja""#));
    ok(c.pedir(
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    ));
    let commit = ok(c.pedir(r#""op":"commit""#));
    assert_eq!(
        commit.inteiro_ou("gravadas", -1),
        1,
        "{}",
        commit.escrever()
    );
    assert!(
        commit.campo("gatilhos_avisos").is_none(),
        "COMMIT sem gatilho ganhou aviso: {}",
        commit.escrever()
    );
    assert_eq!(contar(&mut c, "clientes"), 1);

    // O caminho SEM transacao, com o gatilho: a linha de base R1 do parecer.
    ok(c.pedir(
        r#""op":"sql","database":"loja","texto":"CREATE TRIGGER audita AFTER INSERT ON
           clientes FOR EACH ROW INSERT INTO auditoria (id, ev) VALUES (NEW.id, 'entrou')""#,
    ));
    let r = ok(c.pedir(
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":2,"nome":"Bia"}"#,
    ));
    assert!(r.campo("gatilhos_avisos").is_none(), "{}", r.escrever());
    assert_eq!(
        contar(&mut c, "auditoria"),
        1,
        "fora de transacao o AFTER tem de continuar gravando de verdade"
    );
}

/// **C4** -- o irmao que fica: `ROLLBACK` nao roda o `AFTER`, e a auditoria
/// fica intacta.
#[test]
fn o_after_do_commit_nao_roda_no_rollback() {
    let base = DirTemp::novo("262-c4");
    let (_s, porta) = subir(&base);
    let mut c = Ligacao::nova(porta);
    com_auditoria(&mut c, true);

    ok(c.pedir(r#""op":"begin","database":"loja""#));
    ok(c.pedir(
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    ));
    let r = ok(c.pedir(r#""op":"rollback""#));
    assert_eq!(r.inteiro_ou("descartadas", -1), 1, "{}", r.escrever());
    assert_eq!(contar(&mut c, "clientes"), 0);
    assert_eq!(contar(&mut c, "auditoria"), 0);
}

/// **C5** -- a gravacao do gatilho recusada nao deixa buraco: o `inserir`
/// seguinte na auditoria sai com o primeiro rowid.
#[test]
fn o_rowid_da_auditoria_nao_ganha_buraco() {
    let base = DirTemp::novo("262-c5");
    let (_s, porta) = subir(&base);
    let mut c = Ligacao::nova(porta);
    com_auditoria(&mut c, true);

    ok(c.pedir(r#""op":"begin","database":"loja""#));
    ok(c.pedir(
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}"#,
    ));
    ok(c.pedir(r#""op":"commit""#));
    let na_auditoria = contar(&mut c, "auditoria");
    let r = ok(c.pedir(
        r#""op":"inserir","database":"loja","tabela":"auditoria","linha":{"id":99,"ev":"manual"}"#,
    ));
    assert_eq!(
        r.inteiro_ou("rowid", -1),
        na_auditoria as i64 + 1,
        "o rowid da auditoria ganhou buraco: {}",
        r.escrever()
    );
}
