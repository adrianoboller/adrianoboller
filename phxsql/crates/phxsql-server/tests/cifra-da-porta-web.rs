//! A porta web presa ao laco local, provada CONTRA O SISTEMA OPERACIONAL.
//!
//! # Por que soquete, e nao teste de unidade
//!
//! O `config.rs` ja prova que `web.bind` sem endereco declarado resolve para
//! `127.0.0.1`. Isso e a leitura do campo -- nao e a prova de que ninguem de
//! fora alcanca a porta. Quem decide isso e a pilha de rede do sistema, e a
//! licao do `BULKINSERT` vale igual aqui: o que depende do sistema operacional
//! se prova contra o sistema operacional.
//!
//! # Por que isto e uma guarda de CIFRA, e por isso mora nos `cifra-*.rs`
//!
//! Ordem do dono, 18/09/2026: a comunicacao tem de ser cifrada. Para o
//! navegador e para o REST a saida e o proxy reverso terminando TLS na frente
//! (`docs/SEGURANCA.md` 7.1), porque TLS proprio pediria biblioteca e zero
//! dependencias externas e petrea. E proxy so protege se o motor NAO estiver
//! aberto ao lado dele: com a porta web na rede, o atacante liga direto e pula
//! o TLS inteiro -- o proxy vira teatro. O endereco de fabrica e, portanto,
//! metade da cifra da porta web.

mod comum;
use comum::DirTemp;

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token de servico deste teste";

/// O IP desta maquina na rede, ou `None` quando ela so tem laco local.
///
/// Sai da tabela de rotas, e nao de uma lista de placas: `connect` num soquete
/// UDP nao manda pacote nenhum -- so escolhe a rota de saida e fixa o endereco
/// local. Funciona sem rede e sem DNS, que e a condicao desta bancada. O
/// destino e da faixa de documentacao (RFC 5737), que nao existe: ninguem vai
/// ser incomodado nem que um pacote escapasse.
fn ip_desta_maquina_na_rede() -> Option<IpAddr> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("203.0.113.1:9").ok()?;
    let ip = s.local_addr().ok()?.ip();
    if ip.is_loopback() || ip.is_unspecified() {
        return None;
    }
    Some(ip)
}

/// Um `GET /saude` cru, com prazo. `None` = nem conectou.
fn saude(alvo: SocketAddr) -> Option<String> {
    let mut fluxo = TcpStream::connect_timeout(&alvo, Duration::from_millis(500)).ok()?;
    fluxo
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    write!(
        fluxo,
        "GET /saude HTTP/1.1\r\nHost: {alvo}\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut resposta = String::new();
    let _ = fluxo.read_to_string(&mut resposta);
    Some(resposta)
}

fn esperar_web(alvo: SocketAddr) -> String {
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if let Some(r) = saude(alvo) {
            if r.contains("200") {
                return r;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("a interface web nao subiu em {alvo}");
}

/// Le um `config.json` DE VERDADE que liga a web e NAO declara `web.bind`.
///
/// Montar o `Config` na mao escreveria o campo, e teste que escreve o campo
/// nao prova o padrao dele -- a mesma armadilha ja paga no `cifra-do-fio.rs`,
/// onde a primeira versao da prova se desfazia sozinha.
///
/// O UNICO campo escrito e o `cifra_fio.exigir`, e ele nao tem nada a ver com
/// o que se prova aqui: esta escrito justamente para nao ter -- com ele
/// omitido, o arquivo passaria a ganhar o aviso do ALCANCE no dia em que
/// aquele padrao virar, e uma prova sobre o endereco da porta web ficaria
/// vermelha por um motivo que nao e o dela.
fn config_que_nao_declara_a_web(base: &std::path::Path, dados: u16) -> Config {
    let caminho = base.join("config.json");
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:{dados}",
              "base": "{}",
              "token": "{TOKEN}",
              "log_acessos": "{}",
              "seguranca": {{ "blacklist": "{}" }},
              "dblink": "{}",
              "jobs": "{}",
              "cifra_fio": {{ "exigir": false }},
              "web": {{ "ligado": true }}
            }}"#,
            bar(base.join("base")),
            bar(base.join("acessos.log")),
            bar(base.join("blacklist.json")),
            bar(base.join("dblink.json")),
            bar(base.join("jobs.json")),
        ),
    )
    .unwrap();
    Config::ler(&caminho).unwrap()
}

fn pasta(nome: &str) -> DirTemp {
    let d = DirTemp::novo(&format!("porta-web-{nome}"));
    std::fs::create_dir_all(d.join("base")).unwrap();
    d
}

// ---------------------------------------------------------------------------
// O teste que mais importa
// ---------------------------------------------------------------------------

/// **Sem endereco declarado, a porta web atende `127.0.0.1` e NAO atende de
/// fora.**
///
/// Os dois sentidos numa corrida so, e de proposito: provar apenas a recusa
/// externa passaria tambem se o servidor nao tivesse subido de jeito nenhum --
/// e a prova certa e "ele esta no ar E mesmo assim nao responde la fora".
///
/// # A unica coisa que este teste escreve, e por que
///
/// So a PORTA. O endereco de fabrica e `127.0.0.1:5001`, e uma porta fixa
/// colidiria com os servidores das outras frentes na mesma arvore -- a prova
/// viraria vermelha por motivo alheio. O HOST, que e o que se prova aqui, sai
/// do arquivo sem ninguem o declarar e vai para o servidor como veio.
///
/// # Quando a metade de fora nao e medida, ela DIZ que nao foi
///
/// Numa maquina so com laco local nao ha IP externo para tentar, e um teste
/// que se declara verde nesse caso esconde a metade que nao rodou. Bancada sem
/// medida aparece como NAO MEDIDA.
#[test]
fn a_porta_web_sem_endereco_declarado_nao_atende_de_fora() {
    let base = pasta("padrao");
    // Nada conecta na porta de DADOS deste teste -- o que se prova e a web --
    // entao ela pede 0 e nunca precisa de leitura de volta.
    let mut c = config_que_nao_declara_a_web(&base, 0);

    let host = c.web.endereco().unwrap().ip();
    assert!(
        host.is_loopback(),
        "a porta web nasceu em {host}, e nao no laco local: o proxy TLS da \
         frente vira teatro, porque da para ligar direto no motor ao lado dele"
    );
    // Um `config.json` que nao fala de proxy nem abre porta nenhuma para fora
    // nao pode ganhar aviso: aviso falso gasta a confianca do verdadeiro.
    assert!(c.avisos.is_empty(), "{:?}", c.avisos);

    c.web.bind = format!("{host}:0");
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    // A REAL, lida do proprio servidor -- pedido 401: escolher um numero por
    // fora e solta-lo antes do `bind` de verdade deixava uma janela para
    // outro teste em paralelo tomar o mesmo numero.
    let web = comum::porta_real(|| s.porta_web());

    let de_dentro = SocketAddr::new(host, web);
    let r = esperar_web(de_dentro);
    assert!(
        r.contains("200"),
        "o /saude do laco local nao respondeu: {r}"
    );

    match ip_desta_maquina_na_rede() {
        Some(ip) => {
            let de_fora = SocketAddr::new(ip, web);
            // O servidor esta comprovadamente no ar (a linha de cima), entao
            // uma conexao que FECHA aqui e a guarda funcionando, e nao um
            // servidor que nunca subiu.
            assert!(
                saude(de_fora).is_none(),
                "a porta web atendeu em {de_fora}: quem estiver na rede fala \
                 com a interface em HTTP claro, sem passar pelo proxy TLS"
            );
        }
        None => println!(
            "NAO MEDIDA a metade de fora: esta maquina so tem laco local, \
             entao nao ha endereco externo para tentar. A metade de dentro \
             (o /saude em {de_dentro}) foi medida e passou."
        ),
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// **O outro sentido: quem declara o endereco de fora e atendido, e avisado.**
///
/// Sem esta metade, a guarda de cima passaria tambem num servidor que
/// simplesmente nao sabe abrir porta -- e a ordem do dono foi endereco de
/// fabrica fechado, nao porta impossivel de abrir. Abrir continua sendo
/// escolha escrita de quem implanta.
///
/// Escuta em `0.0.0.0` e alcanca pelo IP da placa: a prova e que a porta
/// ABERTA e mesmo alcancavel de fora, que e o que faz a de cima significar
/// alguma coisa.
#[test]
fn endereco_declarado_para_fora_e_atendido_e_o_arranque_avisa() {
    let Some(ip) = ip_desta_maquina_na_rede() else {
        println!(
            "NAO MEDIDA: esta maquina so tem laco local, entao nao da para \
             provar que a porta aberta e alcancavel de fora."
        );
        return;
    };
    let base = pasta("aberta");
    let mut c = config_que_nao_declara_a_web(&base, 0);
    c.web.bind = "0.0.0.0:0".into();

    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let web = comum::porta_real(|| s.porta_web());

    let de_fora = SocketAddr::new(ip, web);
    let r = esperar_web(de_fora);
    assert!(r.contains("200"), "a porta aberta nao atendeu de fora: {r}");
    let _ = std::fs::remove_dir_all(&base);
}

/// E o aviso do arranque, lido do arquivo -- porque e do ARQUIVO que ele sai.
///
/// Fica neste arquivo, e nao so no `config.rs`, porque aqui ele e lido pelo
/// mesmo caminho do arranque de verdade (`Config::ler`), e nao pelo
/// `de_json` avulso: um aviso que nascesse depois da leitura do arquivo
/// passaria no teste de unidade e ficaria calado no servidor.
#[test]
fn abrir_a_porta_web_no_arquivo_avisa_a_falta_do_proxy() {
    let base = pasta("aviso");
    let caminho = base.join("config.json");
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:0",
              "base": "{}",
              "token": "{TOKEN}",
              "log_acessos": "{}",
              "seguranca": {{ "blacklist": "{}" }},
              "dblink": "{}",
              "jobs": "{}",
              "web": {{ "ligado": true, "bind": "0.0.0.0:0" }}
            }}"#,
            bar(base.join("base")),
            bar(base.join("acessos.log")),
            bar(base.join("blacklist.json")),
            bar(base.join("dblink.json")),
            bar(base.join("jobs.json")),
        ),
    )
    .unwrap();
    let c = Config::ler(&caminho).unwrap();
    let aviso = c
        .avisos
        .iter()
        .find(|a| a.starts_with("web.bind"))
        .unwrap_or_else(|| panic!("o arranque ficou calado: {:?}", c.avisos));
    assert!(aviso.contains("proxy"), "{aviso}");
    assert!(aviso.contains("atras_de_proxy"), "{aviso}");
    // E o campo declarado cala -- no mesmo caminho, lido do mesmo arquivo.
    let texto = std::fs::read_to_string(&caminho).unwrap().replace(
        r#""ligado": true"#,
        r#""ligado": true, "atras_de_proxy": true"#,
    );
    std::fs::write(&caminho, texto).unwrap();
    let c = Config::ler(&caminho).unwrap();
    assert!(
        !c.avisos.iter().any(|a| a.starts_with("web.bind")),
        "{:?}",
        c.avisos
    );
    assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    let _ = std::fs::remove_dir_all(&base);
}
