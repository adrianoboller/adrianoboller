//! O id de sessao HTTP em claro de fora do loopback (pedido 674) e a troca
//! do id a cada login (pedido 719) -- provados PELO SOQUETE, de um IP que
//! nao e o 127.0.0.1.
//!
//! O `desafio` e o `login` pela web devolvem um id que vale a identidade
//! inteira enquanto durar; quem escuta a rede da loja e o leva entra no lugar
//! de quem logou, sem senha nenhuma. O que cada prova segura:
//!
//! 1. pela web em claro (`exigir: false`) e do IP da rede, o `desafio` e o
//!    `login` por prova se RECUSAM nomeando o escape, e a resposta nao traz
//!    o campo `sessao` -- pelo `/api`, pelo `/v1/` do REST e pelo login que
//!    vai para OUTRO servidor, que nao passa pelo `op_desafio` daqui;
//! 2. do loopback entram, sem nada escrito (o comportamento velho);
//! 3. com o escape do 667 (`senha_em_claro_pela_rede`) entram de fora -- um
//!    escape so, o mesmo da senha;
//! 4. o id que o `desafio` devolve NAO e o que o `login` devolve, e o do
//!    desafio morre (fixacao de sessao, 719) -- no local e no remoto.

mod comum;
use comum::DirTemp;

use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream, UdpSocket};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token de servico deste teste";
const SENHA: &str = "segredo123";

/// O IP desta maquina na rede, sem resolver nome: a rota que o sistema
/// escolheria para fora. `None` quando a maquina nao tem rede -- e a prova
/// diz que nao rodou em vez de passar calada.
fn ip_da_rede() -> Option<IpAddr> {
    let u = UdpSocket::bind("0.0.0.0:0").ok()?;
    u.connect("192.0.2.1:9").ok()?;
    let ip = u.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

fn loopback() -> IpAddr {
    "127.0.0.1".parse().unwrap()
}

/// UM hash so por processo: o sal e sorteado a cada `cifrar_com`, e a prova
/// do cliente tem de sair do MESMO hash que o servidor guardou.
fn hash_da_ana() -> String {
    static H: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    H.get_or_init(|| phxsql_core::senha::cifrar_com(SENHA, 1))
        .clone()
}

/// Sobe um servidor com a web (e o REST) em `0.0.0.0`, em claro, com a
/// exigencia de cifra desligada. `extra_web` entra dentro da secao `web`.
fn subir(base: &Path, escape: bool, extra_web: &str) -> Arc<Servidor> {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let h = hash_da_ana();
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
              "cifra_fio": {{ "exigir": false, "senha_em_claro_pela_rede": {escape} }},
              "usuarios": [
                {{ "id": 2, "login": "ana", "nome": "Ana", "senha_hash": "{h}",
                   "ativo": true, "supervisor": true }} ],
              "web": {{ "ligado": true, "bind": "0.0.0.0:0"{extra_web} }},
              "rest": {{ "ligado": true, "bind": "0.0.0.0:0" }}
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
    assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    s
}

/// Um pedido HTTP cru a `ip:porta`; devolve o corpo da resposta.
fn http(ip: IpAddr, porta: u16, rota: &str, cabecalhos: &str, corpo: &str) -> String {
    let mut c = TcpStream::connect((ip, porta)).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    write!(
        c,
        "POST {rota} HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\n{cabecalhos}\
         Content-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
        corpo.len()
    )
    .unwrap();
    let mut v = Vec::new();
    let _ = c.read_to_end(&mut v);
    let r = String::from_utf8_lossy(&v).into_owned();
    match r.split_once("\r\n\r\n") {
        Some((_, corpo)) => corpo.to_string(),
        None => r,
    }
}

fn api(ip: IpAddr, porta: u16, sessao: &str, corpo: &str) -> Json {
    let cab = if sessao.is_empty() {
        String::new()
    } else {
        format!("X-Sessao: {sessao}\r\n")
    };
    let r = http(ip, porta, "/api", &cab, corpo);
    Json::analisar(&r).unwrap_or_else(|e| panic!("resposta que nao e JSON ({e}): {r}"))
}

fn rest(ip: IpAddr, porta: u16, op: &str, sessao: &str, corpo: &str) -> Json {
    let mut cab = format!("Authorization: Bearer {TOKEN}\r\n");
    if !sessao.is_empty() {
        cab.push_str(&format!("X-Sessao: {sessao}\r\n"));
    }
    let r = http(ip, porta, &format!("/v1/{op}"), &cab, corpo);
    Json::analisar(&r).unwrap_or_else(|e| panic!("resposta que nao e JSON ({e}): {r}"))
}

fn desafio(extra: &str) -> String {
    format!(r#"{{"token":"{TOKEN}","op":"desafio","usuario":"ana"{extra}}}"#)
}

/// O login por prova, a partir da resposta de um `desafio`. A senha NAO
/// viaja: e a forma (1) do `op_login`, a que o 667 deixa passar sempre.
fn login_por_prova(resposta_do_desafio: &Json, extra: &str) -> String {
    let r = resposta_do_desafio
        .campo("resultado")
        .unwrap_or(resposta_do_desafio);
    let nonce = r.texto_ou("nonce", "");
    assert!(
        !nonce.is_empty(),
        "o desafio nao devolveu nonce: {}",
        resposta_do_desafio.escrever()
    );
    let dk = phxsql_core::senha::derivado_do_hash(&hash_da_ana()).unwrap();
    let nc = phxsql_core::desafio::nonce();
    let prova = phxsql_core::desafio::calcular_prova(&dk, nonce, &nc, "ana", None);
    format!(
        r#"{{"token":"{TOKEN}","op":"login","usuario":"ana","prova":"{prova}","nonce_cliente":"{nc}"{extra}}}"#
    )
}

fn sessao_de(r: &Json) -> String {
    r.texto_ou("sessao", "").to_string()
}

/// A recusa do 674: nomeada, com o escape, sem sucesso e SEM o id.
fn recusa_sem_sessao(r: &Json, onde: &str) {
    let txt = r.escrever();
    assert!(!r.booleano_ou("ok", false), "{onde}: entrou -- {txt}");
    assert!(
        r.texto_ou("erro", "").contains("senha_em_claro_pela_rede"),
        "{onde}: a recusa nao nomeia o escape -- {txt}"
    );
    assert!(
        r.campo("sessao").is_none(),
        "{onde}: o id de sessao saiu pelo fio em claro -- {txt}"
    );
}

#[test]
fn id_de_sessao_em_claro_de_fora_do_loopback_se_recusa_pelos_tres_caminhos() {
    let Some(rede) = ip_da_rede() else {
        eprintln!("NAO RODOU: esta maquina nao tem IP de rede fora do loopback");
        return;
    };
    // Um OUTRO servidor de verdade, alcancavel e listado: sem o portao no
    // caminho remoto, o desafio la daria certo e o id nasceria aqui.
    let dr = DirTemp::novo("token-674-remoto");
    let (_r, porta_remota) = subir_remoto(&dr.0);
    let d = DirTemp::novo("token-674");
    let extra = format!(
        r#", "servidores": [{{ "host": "127.0.0.1", "porta": {porta_remota}, "cifra": false }}]"#
    );
    let s = subir(&d.0, false, &extra);
    let web = comum::porta_real(|| s.porta_web());
    let porta_rest = comum::porta_real(|| s.porta_rest());

    // (1) o `/api`, pelo IP da rede: o desafio nao emite id.
    let r = api(rede, web, "", &desafio(""));
    recusa_sem_sessao(&r, "desafio pelo /api");

    // O login por prova tambem nao: o desafio vem do loopback (unico jeito
    // de ter um nonce valido) e o login sai pela rede, sem senha nenhuma.
    let d_local = api(loopback(), web, "", &desafio(""));
    let id_local = sessao_de(&d_local);
    assert!(!id_local.is_empty(), "{}", d_local.escrever());
    let r = api(rede, web, &id_local, &login_por_prova(&d_local, ""));
    assert!(!r.booleano_ou("ok", false), "{}", r.escrever());
    assert!(
        r.texto_ou("erro", "").contains("senha_em_claro_pela_rede"),
        "login por prova pelo /api: {}",
        r.escrever()
    );
    // Nenhum id NOVO saiu: no maximo o eco do que o cliente ja tinha.
    let eco = sessao_de(&r);
    assert!(eco.is_empty() || eco == id_local, "{}", r.escrever());

    // O REST, pelo `/v1/desafio` e `/v1/login`.
    let r = rest(rede, porta_rest, "desafio", "", r#"{"usuario":"ana"}"#);
    recusa_sem_sessao(&r, "desafio pelo REST");
    let r = rest(
        rede,
        porta_rest,
        "login",
        "",
        r#"{"usuario":"ana","prova":"00","nonce_cliente":"00"}"#,
    );
    recusa_sem_sessao(&r, "login pelo REST");

    // O irmao: o desafio que vai para OUTRO servidor, e nao passa pelo
    // `op_desafio` daqui.
    let r = api(
        rede,
        web,
        "",
        &desafio(&format!(r#","servidor":"127.0.0.1:{porta_remota}""#)),
    );
    recusa_sem_sessao(&r, "desafio remoto");
}

/// O comportamento VELHO: do loopback tudo entra, sem nada escrito.
#[test]
fn do_loopback_o_desafio_e_o_login_entram_e_o_id_gira() {
    let d = DirTemp::novo("token-674-local");
    let s = subir(&d.0, false, "");
    let web = comum::porta_real(|| s.porta_web());
    let porta_rest = comum::porta_real(|| s.porta_rest());

    let d1 = api(loopback(), web, "", &desafio(""));
    assert!(d1.booleano_ou("ok", false), "{}", d1.escrever());
    let id_do_desafio = sessao_de(&d1);
    assert!(!id_do_desafio.is_empty(), "{}", d1.escrever());

    let l = api(loopback(), web, &id_do_desafio, &login_por_prova(&d1, ""));
    assert!(l.booleano_ou("ok", false), "{}", l.escrever());
    let id_do_login = sessao_de(&l);
    assert!(!id_do_login.is_empty(), "{}", l.escrever());

    // (4) o giro: o login devolve um id NOVO...
    assert_ne!(
        id_do_desafio, id_do_login,
        "o login promoveu o id do desafio (fixacao de sessao)"
    );
    // ...e o do desafio morreu: quem o viu passar nao entra com ele.
    let ping = format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#);
    let r = api(loopback(), web, &id_do_desafio, &ping);
    assert_ne!(
        sessao_de(&r),
        id_do_desafio,
        "o id do desafio continua vivo: {}",
        r.escrever()
    );
    let r = api(loopback(), web, &id_do_login, &ping);
    assert_eq!(sessao_de(&r), id_do_login, "{}", r.escrever());

    // O REST pelo loopback tambem entra.
    let r = rest(
        loopback(),
        porta_rest,
        "desafio",
        "",
        r#"{"usuario":"ana"}"#,
    );
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());
    assert!(!sessao_de(&r).is_empty(), "{}", r.escrever());
}

/// O id de sessao e a credencial do `X-Sessao`, e a chave da atividade web
/// sai no `telemetria` e no `aquario_retrato` (que quem so monitora le): ela
/// e o RESUMO da sessao, nunca o id. Achado exercitando a F9 do pedido 495 --
/// a ocorrencia gravava `"tarefa":"web:<id>"`.
///
/// Vermelho medido: com `format!("web:{id_sessao}")` de volta no
/// `servico_web_01.rs`, o id aparece nas duas respostas.
#[test]
fn o_id_da_sessao_nao_sai_como_chave_da_atividade() {
    let d = DirTemp::novo("token-chave-da-atividade");
    let s = subir(&d.0, false, "");
    let web = comum::porta_real(|| s.porta_web());
    let d1 = api(loopback(), web, "", &desafio(""));
    let l = api(loopback(), web, &sessao_de(&d1), &login_por_prova(&d1, ""));
    let id = sessao_de(&l);
    assert!(!id.is_empty(), "{}", l.escrever());
    for op in ["telemetria", "aquario_retrato"] {
        let r = api(
            loopback(),
            web,
            &id,
            &format!(r#"{{"token":"{TOKEN}","op":"{op}"}}"#),
        );
        assert!(r.booleano_ou("ok", false), "{op}: {}", r.escrever());
        // So o `resultado`: o envelope da resposta HTTP devolve o id ao
        // PROPRIO dono no campo `sessao`, e isso e o protocolo da tela.
        let texto = r.campo("resultado").map(Json::escrever).unwrap_or_default();
        assert!(
            texto.contains("web:"),
            "{op}: a atividade web sumiu: {texto}"
        );
        assert!(!texto.contains(&id), "{op}: o id da sessao saiu: {texto}");
    }
}

/// O escape do 667 abre a emissao do id tambem: um escape so.
#[test]
fn com_o_escape_escrito_o_id_sai_de_fora() {
    let Some(rede) = ip_da_rede() else {
        eprintln!("NAO RODOU: esta maquina nao tem IP de rede fora do loopback");
        return;
    };
    let d = DirTemp::novo("token-674-escape");
    let s = subir(&d.0, true, "");
    let web = comum::porta_real(|| s.porta_web());

    let d1 = api(rede, web, "", &desafio(""));
    assert!(d1.booleano_ou("ok", false), "{}", d1.escrever());
    let id_do_desafio = sessao_de(&d1);
    let l = api(rede, web, &id_do_desafio, &login_por_prova(&d1, ""));
    assert!(l.booleano_ou("ok", false), "{}", l.escrever());
    let id_do_login = sessao_de(&l);
    assert!(!id_do_login.is_empty(), "{}", l.escrever());
    // O giro vale com o escape -- a fixacao nao depende de escutar o fio.
    assert_ne!(id_do_desafio, id_do_login);
}

/// Um `phxsql` so com a porta de dados, para ser o OUTRO servidor do login
/// remoto. A web o alcanca pelo fio em claro, escrito (`"cifra": false`).
fn subir_remoto(base: &Path) -> (Arc<Servidor>, u16) {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let h = hash_da_ana();
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
              "cifra_fio": {{ "exigir": false }},
              "usuarios": [
                {{ "id": 2, "login": "ana", "nome": "Ana", "senha_hash": "{h}",
                   "ativo": true, "supervisor": true }} ]
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
    assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    (s, porta)
}

/// O giro no login que vai para OUTRO servidor: o caminho irmao, que nao
/// passa pelo `acertar_sessao`.
#[test]
fn no_login_remoto_o_id_tambem_gira() {
    let dr = DirTemp::novo("token-719-remoto");
    let (_r, porta_remota) = subir_remoto(&dr.0);
    let destino = format!("127.0.0.1:{porta_remota}");

    let d = DirTemp::novo("token-719-web");
    let extra = format!(
        r#", "servidores": [{{ "host": "127.0.0.1", "porta": {porta_remota}, "cifra": false }}]"#
    );
    let s = subir(&d.0, false, &extra);
    let web = comum::porta_real(|| s.porta_web());

    let campo = format!(r#","servidor":"{destino}""#);
    let d1 = api(loopback(), web, "", &desafio(&campo));
    assert!(d1.booleano_ou("ok", false), "{}", d1.escrever());
    let id_do_desafio = sessao_de(&d1);
    assert!(!id_do_desafio.is_empty(), "{}", d1.escrever());

    let l = api(
        loopback(),
        web,
        &id_do_desafio,
        &login_por_prova(&d1, &campo),
    );
    assert!(l.booleano_ou("ok", false), "{}", l.escrever());
    let id_do_login = sessao_de(&l);
    assert!(!id_do_login.is_empty(), "{}", l.escrever());
    assert_ne!(
        id_do_desafio, id_do_login,
        "o login remoto promoveu o id do desafio (fixacao de sessao)"
    );

    // O id novo continua amarrado ao remoto; o velho nao leva a nada.
    let ping = format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#);
    let r = api(loopback(), web, &id_do_login, &ping);
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());
    assert_eq!(sessao_de(&r), id_do_login, "{}", r.escrever());
    let r = api(loopback(), web, &id_do_desafio, &ping);
    assert_ne!(sessao_de(&r), id_do_desafio, "{}", r.escrever());
}
