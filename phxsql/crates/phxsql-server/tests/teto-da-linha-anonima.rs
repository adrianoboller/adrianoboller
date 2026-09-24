//! O teto da linha ANTES do login, na porta de dados -- pedido 434, provado
//! PELO SOQUETE.
//!
//! # O defeito, e por que ele nao era «falta de teto»
//!
//! A leitura da porta 5000 sempre veio do motor (`Canal::ler`), e o motor
//! sempre teve teto -- o `TETO_DO_REGISTRO`, 128 MiB. O que faltava era a
//! pergunta que o teto responde mudar com a sessao: **quanto este lado reserva
//! antes de saber quem esta do outro lado?** Antes do login, 128 MiB e a
//! resposta errada. Com `conexoes_max` nascendo em 64, sessenta e quatro
//! soquetes mandando bytes sem `\n` reservam 8 GiB sem que nenhuma credencial
//! tenha existido.
//!
//! # Os tres casos sao o MESMO estimulo
//!
//! Uma linha de 1 MiB, JSON valido, um `ping` com enchimento. Ela e maior que
//! o teto do anonimo (64 KiB) e muito menor que o do registro (128 MiB), e e
//! isso que faz os tres se distinguirem sem gastar 128 MiB de bateria:
//!
//! | sessao | cadastro | o que tem de acontecer |
//! |---|---|---|
//! | anonima | com usuarios | recusa `LIMITE_EXCEDIDO` |
//! | com login | com usuarios | atendida, como sempre foi |
//! | anonima | SEM usuarios | atendida, como sempre foi |
//!
//! Com o defeito reposto (o teto do anonimo virando o do registro) o primeiro
//! caso responde `"ok":true` e reprova na hora. Os outros dois sao o
//! comportamento VELHO, e sao os que mais importam: guarda nova entra PEDIDA,
//! nao imposta -- a terceira linha e o servidor sem usuario nenhum, que e todo
//! cliente que nunca criou cadastro e que continuaria mandando lote de 128 MiB
//! no dia seguinte.
//!
//! # Por que soquete, e nao teste de unidade
//!
//! Porque o que se prova aqui nao e a conta `lidos > teto` -- essa o
//! `fio::testes` ja prova. E que a recusa CHEGA ao outro lado, com nome e
//! numero, e entra no `acessos.log` com quanto foi lido. E a licao do
//! `BULKINSERT`: o que depende do sistema operacional se prova contra o
//! sistema operacional.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::fio::TETO_DO_APERTO;
use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "teste-do-teto-do-anonimo";
const SENHA: &str = "segredo-de-teste";

/// O enchimento da linha unica desta bateria.
///
/// 1 MiB: dezesseis vezes o teto do anonimo e cento e vinte e oito vezes menos
/// que o do registro. A folga dos dois lados e o que deixa um estimulo so
/// separar os tres casos -- e o que evita mandar 128 MiB por uma bateria que
/// todo mundo roda.
const ENCHIMENTO: usize = 1024 * 1024;

fn pasta(nome: &str) -> DirTemp {
    let d = DirTemp::novo(&format!("teto-anonimo-{nome}"));
    std::fs::create_dir_all(d.join("base")).unwrap();
    d
}

/// Sobe um servidor com ou sem cadastro.
///
/// `com_cadastro` falso e o servidor sem usuario nenhum -- o caso VELHO, que
/// nao pode mudar de comportamento por causa de uma regra nova.
fn subir(base: &std::path::Path, com_cadastro: bool) -> (Arc<Servidor>, u16) {
    // Uma iteracao so: a senha real nao interessa a esta bateria, e 210.000
    // iteracoes por login fariam a corrida levar segundos por nada.
    let h = phxsql_core::senha::cifrar_com(SENHA, 1);
    let usuarios = if com_cadastro {
        format!(r#""root": {{ "login": "root", "senha_hash": "{h}" }},"#)
    } else {
        String::new()
    };
    // O ESCAPE ESCRITO: desde 18/09/2026 a cifra do fio nasce exigida (pedido
    // 370), e esta bateria conecta em claro porque o que ela mede e o teto da
    // linha. Sem esta linha a recusa lida aqui seria a da cifra, e a prova
    // passaria a medir o portao errado.
    //
    // E o `timeout_s` alto e proposital: com o defeito reposto o servidor fica
    // esperando a quebra de linha, e um prazo curto o faria desistir sozinho
    // -- a prova passaria por um motivo que nao e o dela.
    let texto = format!(
        r#"{{ "bind": "127.0.0.1:0", "base": {base:?}, "token": "{TOKEN}",
              "log_acessos": {log:?}, "blacklist": {bl:?}, "dblink": {dbl:?},
              "jobs": {jobs:?}, {usuarios}
              "timeout_s": 60,
              "cifra_fio": {{ "exigir": false }},
              "web": {{ "ligado": false }} }}"#,
        base = base.display().to_string(),
        log = base.join("acessos.log").display().to_string(),
        bl = base.join("blacklist.json").display().to_string(),
        dbl = base.join("dblink.json").display().to_string(),
        jobs = base.join("jobs.json").display().to_string(),
    );
    let c = Config::de_json(&Json::analisar(&texto).unwrap()).unwrap();
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
        f.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        f.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
        Ligacao {
            escrita: f.try_clone().unwrap(),
            leitor: BufReader::new(f),
        }
    }

    fn entrar(porta: u16) -> Ligacao {
        let mut c = Ligacao::nova(porta);
        let r = c.mandar(&format!(
            r#"{{"token":"{TOKEN}","op":"login","usuario":"root","senha":"{SENHA}"}}"#
        ));
        assert!(
            r.booleano_ou("ok", false),
            "login de root: {}",
            r.escrever()
        );
        c
    }

    /// Manda uma linha ja pronta e devolve a resposta.
    fn mandar(&mut self, linha: &str) -> Json {
        self.escrita.write_all(linha.as_bytes()).unwrap();
        self.escrita.write_all(b"\n").unwrap();
        self.escrita.flush().unwrap();
        let mut r = String::new();
        match self.leitor.read_line(&mut r) {
            Ok(0) => panic!(
                "a conexao fechou SEM responder a linha de {} bytes",
                linha.len()
            ),
            Ok(_) => {}
            Err(e) => panic!("linha de {} bytes ficou sem resposta: {e}", linha.len()),
        }
        Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"))
    }
}

/// A linha de 1 MiB: JSON valido, `ping` com enchimento.
///
/// Valida de proposito, e ao contrario da prova do teto do REGISTRO: la o
/// lixo servia porque ninguem chegava a analisa-lo nos dois estados. Aqui,
/// com o defeito reposto, a linha PASSA pela leitura -- e so uma linha valida
/// faz a diferenca aparecer como `"ok":true` em vez de como um erro de JSON,
/// que seria erro do jeito errado.
fn linha_de_um_mib() -> String {
    format!(
        r#"{{"token":"{TOKEN}","op":"ping","enchimento":"{}"}}"#,
        "A".repeat(ENCHIMENTO)
    )
}

/// **Anonimo, num servidor com cadastro: a linha de 1 MiB e recusada pelo
/// teto do anonimo.**
///
/// O defeito que este teste trava: sem a decisao por sessao, quem ainda nao
/// provou ser ninguem escolhe quanta memoria este lado reserva -- ate 128 MiB
/// por conexao, e ha 64 delas de fabrica.
#[test]
fn a_linha_grande_antes_do_login_e_recusada_pelo_teto_do_anonimo() {
    let d = pasta("anonimo");
    let (_s, porta) = subir(&d, true);
    let mut c = Ligacao::nova(porta);

    let linha = linha_de_um_mib();
    let mandados = linha.len() as u64 + 1;
    let r = c.mandar(&linha);

    assert!(!r.booleano_ou("ok", true), "resposta: {}", r.escrever());
    assert_eq!(
        r.texto_ou("nome", ""),
        "LIMITE_EXCEDIDO",
        "{}",
        r.escrever()
    );
    assert_eq!(r.inteiro_ou("codigo", 0), 3003, "{}", r.escrever());

    let erro = r.texto_ou("erro", "").to_string();
    // O TETO EM BYTES, porque e por ele que quem opera compara com o que mediu.
    assert!(
        erro.contains(&TETO_DO_APERTO.to_string()),
        "a recusa nao traz o teto em bytes: {erro}"
    );
    // E a MEDIDA legivel, que antes de hoje dizia «0 MiB» aqui -- achado B2 da
    // revisao de seguranca de 23/09/2026.
    assert!(
        erro.contains("64 KiB"),
        "a recusa anuncia a medida errada: {erro}"
    );
    assert!(
        !erro.contains("0 MiB"),
        "a recusa voltou a dizer zero: {erro}"
    );
    // E a ORDEM que nao cabe fica de fora: quem mandou uma linha grande num
    // aperto de mao nao tem lote para baixar nem tabela para partir.
    assert!(
        !erro.contains("parta a tabela"),
        "a recusa manda partir uma tabela dentro de uma sessao anonima: {erro}"
    );

    // QUANTO foi lido, pela contabilidade do proprio servidor: o teto mais um
    // que ele guardou, mais o que drenou ate a quebra de linha. O `teto` do
    // log e o do ANONIMO, e nao o do registro -- um log que dissesse 128 MiB
    // aqui mandaria quem investiga procurar um lote que nunca existiu.
    let esperado = format!("lidos {mandados} bytes desta linha (teto {TETO_DO_APERTO})");
    let log = esperar_no_log(&d.join("acessos.log"), &esperado);
    assert!(
        log.contains(r#""op":"fio""#),
        "a linha do log nao e a do fio: {log}"
    );
}

/// **O COMPORTAMENTO VELHO, com login: a mesma linha de 1 MiB atravessa.**
///
/// Este e o teste que mais importa: quem provou quem e continua com o teto do
/// registro, e o lote grande de todo dia nao encolheu por causa de uma guarda
/// que nao e sobre ele.
#[test]
fn depois_do_login_a_linha_grande_continua_passando() {
    let d = pasta("logado");
    let (_s, porta) = subir(&d, true);
    let mut c = Ligacao::entrar(porta);
    let r = c.mandar(&linha_de_um_mib());
    assert!(r.booleano_ou("ok", false), "resposta: {}", r.escrever());
}

/// **O COMPORTAMENTO VELHO, sem cadastro: a mesma linha de 1 MiB atravessa.**
///
/// Guarda nova entra PEDIDA, nao imposta. Servidor sem usuario nenhum nao tem
/// credencial a esperar -- chamar de anonima a sessao ali seria apertar o
/// `inserir` em lote de todo cliente que nunca criou cadastro, que e o estrago
/// do pedido 203 com outra roupa.
#[test]
fn sem_cadastro_a_linha_grande_continua_passando() {
    let d = pasta("sem-cadastro");
    let (_s, porta) = subir(&d, false);
    let mut c = Ligacao::nova(porta);
    let r = c.mandar(&linha_de_um_mib());
    assert!(r.booleano_ou("ok", false), "resposta: {}", r.escrever());
}

/// **A linha PEQUENA de quem ainda nao entrou continua atravessando.**
///
/// O `login` e a primeira coisa que qualquer cliente manda, e ele e anonimo
/// por definicao. Teto que recusasse o proprio login trancaria todo mundo para
/// fora -- e a bateria inteira falharia, mas por um motivo que este teste
/// nomeia em uma linha.
#[test]
fn o_login_anonimo_de_sempre_continua_passando() {
    let d = pasta("login");
    let (_s, porta) = subir(&d, true);
    let mut c = Ligacao::nova(porta);
    let r = c.mandar(&format!(
        r#"{{"token":"{TOKEN}","op":"login","usuario":"root","senha":"{SENHA}"}}"#
    ));
    assert!(r.booleano_ou("ok", false), "resposta: {}", r.escrever());
}

/// Espera a linha aparecer no `acessos.log` -- por CONDICAO, e nao por tempo
/// fixo: o servidor responde ANTES de anotar, entao ler o arquivo na hora e
/// uma corrida perdida em maquina carregada.
fn esperar_no_log(caminho: &std::path::Path, pedaco: &str) -> String {
    let ate = Instant::now() + Duration::from_secs(10);
    let mut ultimo = String::new();
    while Instant::now() < ate {
        ultimo = std::fs::read_to_string(caminho).unwrap_or_default();
        if let Some(l) = ultimo.lines().find(|l| l.contains(pedaco)) {
            return l.to_string();
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("o `acessos.log` nunca trouxe «{pedaco}»; o que ele tem:\n{ultimo}");
}

// ---------------------------------------------------------------------------
// Pedido 442 -- o teto escolhido com a ficha VELHA
// ---------------------------------------------------------------------------
//
// O 434 decidia o teto ANTES de a leitura bloquear, com a sessao do jeito que
// ela estava naquele instante. A conexao passa a vida parada dentro dessa
// leitura -- e enquanto ela esta parada o cadastro muda. Medido pela revisao
// de seguranca (achado M1): a conexao de um usuario EXCLUIDO ainda mandou
// 1 MiB com `ok:true`, e a aberta antes do primeiro `usuario_criar` tambem.

const SENHA_DA_ANA: &str = "segredo-da-ana";

/// Um servidor que sobe de um ARQUIVO -- o `usuario_criar` e o
/// `usuario_excluir` gravam o cadastro no `config.json`, e sem arquivo eles
/// recusam antes de a prova comecar.
fn subir_de_arquivo(base: &std::path::Path, usuarios: &str) -> (Arc<Servidor>, u16) {
    let caminho = base.join("config.json");
    let texto = format!(
        r#"{{ "bind": "127.0.0.1:0", "base": {b:?}, "token": "{TOKEN}",
              "log_acessos": {log:?}, "blacklist": {bl:?}, "dblink": {dbl:?},
              "jobs": {jobs:?}, "usuarios": [{usuarios}],
              "timeout_s": 60,
              "cifra_fio": {{ "exigir": false }},
              "web": {{ "ligado": false }} }}"#,
        b = base.join("base").display().to_string(),
        log = base.join("acessos.log").display().to_string(),
        bl = base.join("blacklist.json").display().to_string(),
        dbl = base.join("dblink.json").display().to_string(),
        jobs = base.join("jobs.json").display().to_string(),
    );
    std::fs::write(&caminho, texto).unwrap();
    let s = Servidor::novo(Config::ler(&caminho).unwrap()).unwrap();
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

fn ficha(login: &str, senha: &str, supervisor: bool) -> String {
    format!(
        r#"{{ "login": "{login}", "senha_hash": "{}", "supervisor": {supervisor} }}"#,
        phxsql_core::senha::cifrar_com(senha, 1)
    )
}

/// Confere que a linha foi RECUSADA pelo teto do anonimo -- e o `acessos.log`
/// diz quanto este lado aceitou guardar: o teto dele, e nao o do registro.
fn recusada_pelo_teto_do_anonimo(d: &std::path::Path, r: &Json, quem: &str) {
    assert!(
        !r.booleano_ou("ok", true),
        "{quem}: o servidor guardou a linha INTEIRA de 1 MiB (ok:true) -- a \
         leitura usou o teto de 128 MiB de quem ja nao e ninguem"
    );
    assert_eq!(
        r.texto_ou("nome", ""),
        "LIMITE_EXCEDIDO",
        "{quem}: {}",
        r.escrever()
    );
    esperar_no_log(
        &d.join("acessos.log"),
        &format!("desta linha (teto {TETO_DO_APERTO})"),
    );
}

/// **O usuario EXCLUIDO com a conexao aberta perde o teto de identificado na
/// linha seguinte.**
///
/// A ordem e a do achado M1: ana entra e a conexao dela fica PARADA na
/// leitura; root a exclui por outra conexao; ana manda 1 MiB. Com o defeito,
/// a leitura ja estava armada com 128 MiB desde antes da exclusao, e a linha
/// inteira entrava -- `ok:true` para quem nao existe mais.
#[test]
fn o_usuario_excluido_com_a_conexao_aberta_perde_o_teto_na_linha_seguinte() {
    let d = pasta("excluido");
    let (_s, porta) = subir_de_arquivo(
        &d,
        &format!(
            "{}, {}",
            ficha("root", SENHA, true),
            ficha("ana", SENHA_DA_ANA, false)
        ),
    );
    let mut ana = Ligacao::nova(porta);
    let r = ana.mandar(&format!(
        r#"{{"token":"{TOKEN}","op":"login","usuario":"ana","senha":"{SENHA_DA_ANA}"}}"#
    ));
    assert!(r.booleano_ou("ok", false), "login da ana: {}", r.escrever());
    // A conexao da ana ja respondeu e voltou para a leitura: e ali que ela
    // fica enquanto o cadastro muda.
    let mut root = Ligacao::entrar(porta);
    let r = root.mandar(&format!(
        r#"{{"token":"{TOKEN}","op":"usuario_excluir","login":"ana"}}"#
    ));
    assert!(
        r.booleano_ou("ok", false),
        "excluir a ana: {}",
        r.escrever()
    );

    let r = ana.mandar(&linha_de_um_mib());
    recusada_pelo_teto_do_anonimo(&d, &r, "usuario excluido");
}

/// **O irmao: a conexao aberta ANTES do primeiro cadastro.**
///
/// Servidor sem usuario nenhum da o teto do registro a todos -- e o
/// comportamento velho, travado acima. Mas quando o primeiro supervisor nasce,
/// a conexao anonima que ja estava aberta deixa de estar num servidor sem
/// cadastro, e a linha seguinte dela e a de um anonimo como outro qualquer.
#[test]
fn a_conexao_aberta_antes_do_primeiro_cadastro_perde_o_teto_quando_ele_nasce() {
    let d = pasta("primeiro-cadastro");
    let (_s, porta) = subir_de_arquivo(&d, "");
    let mut antiga = Ligacao::nova(porta);
    let r = antiga.mandar(&format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#));
    assert!(r.booleano_ou("ok", false), "ping: {}", r.escrever());

    let mut outra = Ligacao::nova(porta);
    let r = outra.mandar(&format!(
        r#"{{"token":"{TOKEN}","op":"usuario_criar","login":"chefe","senha":"{SENHA}","supervisor":true}}"#
    ));
    assert!(
        r.booleano_ou("ok", false),
        "primeiro cadastro: {}",
        r.escrever()
    );

    let r = antiga.mandar(&linha_de_um_mib());
    recusada_pelo_teto_do_anonimo(&d, &r, "conexao aberta antes do cadastro");
}

/// **O COMPORTAMENTO VELHO: quem continua no cadastro continua com o teto do
/// registro** -- inclusive quando o cadastro muda por causa de OUTRO usuario.
/// Um conserto que refrescasse a ficha errado derrubaria o lote de todo mundo
/// a cada `usuario_criar`.
#[test]
fn quem_continua_no_cadastro_continua_com_o_teto_do_registro() {
    let d = pasta("continua");
    let (_s, porta) = subir_de_arquivo(
        &d,
        &format!(
            "{}, {}",
            ficha("root", SENHA, true),
            ficha("ana", SENHA_DA_ANA, false)
        ),
    );
    let mut ana = Ligacao::nova(porta);
    let r = ana.mandar(&format!(
        r#"{{"token":"{TOKEN}","op":"login","usuario":"ana","senha":"{SENHA_DA_ANA}"}}"#
    ));
    assert!(r.booleano_ou("ok", false), "login da ana: {}", r.escrever());
    let mut root = Ligacao::entrar(porta);
    let r = root.mandar(&format!(
        r#"{{"token":"{TOKEN}","op":"usuario_criar","login":"bia","senha":"{SENHA}"}}"#
    ));
    assert!(r.booleano_ou("ok", false), "criar a bia: {}", r.escrever());

    let r = ana.mandar(&linha_de_um_mib());
    assert!(r.booleano_ou("ok", false), "resposta: {}", r.escrever());
}
