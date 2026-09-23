//! Phoenix VPN -- redes virtuais no estilo Radmin, sobre OpenVPN.
//!
//! Duas portas para o mesmo motor (`painel.rs`): a tela web servida pelo
//! `phxvpn painel` e a linha de comando abaixo, que fala com o painel pela API.

use phoenix_vpn::{http, painel, pg, supervisor};
use phxsql_core::json::Json;
use std::io::{BufRead, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;

const AJUDA: &str = "Phoenix VPN -- redes virtuais no estilo Radmin, sobre OpenVPN

  phxvpn painel [--escutar 127.0.0.1:8470] [--pg \"host=.. port=.. user=.. password=.. dbname=..\"]
                [--dados DIR] [--openvpn]
      Sobe o painel (tela em http://<escuta>/). Conexao do PostgreSQL tambem por
      PHXVPN_PG. Com PHXVPN_SENHA_MESTRE no ambiente o cofre ja abre destrancado.
      --openvpn sobe um processo openvpn por rede.

  phxvpn criar-rede --painel http://host:8470 --usuario LOGIN --rede NOME [--finalidade TEXTO]
  phxvpn entrar     --painel http://host:8470 --usuario LOGIN --rede NOME [--saida ARQ.ovpn] [--conectar]
      Senhas por PHXVPN_SENHA e PHXVPN_SENHA_REDE, ou perguntadas no terminal --
      nunca por argumento, que aparece na lista de processos.
      --conectar chama o openvpn com o perfil baixado.

  phxvpn versao
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("painel") => cmd_painel(&args[1..]),
        Some("criar-rede") => cmd_rede(&args[1..], true),
        Some("entrar") => cmd_rede(&args[1..], false),
        Some("versao") | Some("--version") => {
            println!("phxvpn {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => {
            print!("{AJUDA}");
            Ok(())
        }
    };
    if let Err(e) = r {
        eprintln!("phxvpn: {e}");
        std::process::exit(1);
    }
}

fn opcao(args: &[String], nome: &str) -> Option<String> {
    args.iter()
        .position(|a| a == nome)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn bandeira(args: &[String], nome: &str) -> bool {
    args.iter().any(|a| a == nome)
}

fn cmd_painel(args: &[String]) -> Result<(), String> {
    let escuta = opcao(args, "--escutar").unwrap_or_else(|| "127.0.0.1:8470".into());
    let pg_texto = opcao(args, "--pg")
        .or_else(|| std::env::var("PHXVPN_PG").ok())
        .ok_or("informe o PostgreSQL por --pg ou PHXVPN_PG")?;
    let dados = PathBuf::from(opcao(args, "--dados").unwrap_or_else(|| "phxvpn-dados".into()));
    let cfg = pg::Config::de_texto(&pg_texto)?;
    let mut p = painel::Painel::abrir(&cfg, &dados)?;
    if let Ok(m) = std::env::var("PHXVPN_SENHA_MESTRE") {
        p.destrancar(&m)?;
        eprintln!("phxvpn: cofre destrancado pela PHXVPN_SENHA_MESTRE");
    }
    let supervisor = if bandeira(args, "--openvpn") {
        Some(supervisor::Supervisor::novo()?)
    } else {
        None
    };
    let destrancado = p.destrancado();
    let estado = Arc::new(http::Estado::novo(p, supervisor));
    if destrancado {
        http::materializar_e_subir(&estado)?;
    }
    if !escuta.starts_with("127.") && !escuta.starts_with("localhost") {
        eprintln!(
            "phxvpn: AVISO -- o painel fala HTTP sem TLS e esta escutando em {escuta}; \
             ponha um proxy com TLS na frente ou use-o pela propria VPN"
        );
    }
    eprintln!("phxvpn: painel em http://{escuta}/");
    http::servir(&escuta, estado)
}

fn senha(var: &str, pergunta: &str) -> Result<String, String> {
    if let Ok(s) = std::env::var(var) {
        return Ok(s);
    }
    eprint!("{pergunta}: ");
    std::io::stderr().flush().ok();
    let mut l = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut l)
        .map_err(|e| e.to_string())?;
    Ok(l.trim_end_matches(['\r', '\n']).to_string())
}

fn cmd_rede(args: &[String], criar: bool) -> Result<(), String> {
    let painel = opcao(args, "--painel").unwrap_or_else(|| "http://127.0.0.1:8470".into());
    let login = opcao(args, "--usuario").ok_or("informe --usuario")?;
    let rede = opcao(args, "--rede").ok_or("informe --rede")?;
    let s_login = senha("PHXVPN_SENHA", "senha do usuario")?;
    let s_rede = senha("PHXVPN_SENHA_REDE", "senha da rede")?;

    let r = chamar(
        &painel,
        "/api/login",
        None,
        &Json::objeto(vec![
            ("usuario", Json::texto_de(&login)),
            ("senha", Json::texto_de(s_login)),
        ]),
    )?;
    let token = r.texto_ou("token", "").to_string();
    let mut pedido = vec![
        ("nome", Json::texto_de(&rede)),
        ("senha", Json::texto_de(s_rede)),
    ];
    if criar {
        pedido.push((
            "finalidade",
            Json::texto_de(opcao(args, "--finalidade").unwrap_or_default()),
        ));
    }
    let rota = if criar {
        "/api/redes"
    } else {
        "/api/redes/entrar"
    };
    let r = chamar(&painel, rota, Some(&token), &Json::objeto(pedido))?;
    let arquivo =
        opcao(args, "--saida").unwrap_or_else(|| r.texto_ou("arquivo", "phoenix.ovpn").to_string());
    std::fs::write(&arquivo, r.texto_ou("perfil", ""))
        .map_err(|e| format!("gravar {arquivo}: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&arquivo, std::fs::Permissions::from_mode(0o600));
    }
    println!("perfil gravado em {arquivo} (contem a sua chave privada: guarde-o como senha)");
    if bandeira(args, "--conectar") {
        let bin = supervisor::achar_no_path("openvpn").ok_or("openvpn nao esta no PATH")?;
        let st = std::process::Command::new(bin)
            .arg("--config")
            .arg(&arquivo)
            .status()
            .map_err(|e| e.to_string())?;
        if !st.success() {
            return Err(format!("openvpn saiu com {st}"));
        }
    }
    Ok(())
}

/// POST JSON ao painel, por HTTP/1.1 cru. So `http://` -- o painel nao fala TLS.
fn chamar(base: &str, rota: &str, token: Option<&str>, corpo: &Json) -> Result<Json, String> {
    let hostporta = base
        .strip_prefix("http://")
        .ok_or("o endereco do painel tem de comecar com http://")?
        .trim_end_matches('/');
    let mut fio = TcpStream::connect(hostporta).map_err(|e| format!("painel {hostporta}: {e}"))?;
    let corpo = corpo.escrever();
    let auth = token
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    write!(
        fio,
        "POST {rota} HTTP/1.1\r\nHost: {hostporta}\r\nContent-Type: application/json\r\n{auth}\
Content-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
        corpo.len()
    )
    .map_err(|e| e.to_string())?;
    let mut resposta = String::new();
    fio.take(4 * 1024 * 1024)
        .read_to_string(&mut resposta)
        .map_err(|e| e.to_string())?;
    let (cab, corpo) = resposta
        .split_once("\r\n\r\n")
        .ok_or("resposta do painel cortada")?;
    let status: u16 = cab
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or("resposta do painel sem status")?;
    let j = Json::analisar(corpo).map_err(|e| format!("resposta do painel: {e}"))?;
    if status != 200 {
        return Err(j.texto_ou("erro", "erro desconhecido").to_string());
    }
    Ok(j)
}
