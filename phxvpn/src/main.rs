//! phxvpn -- redes virtuais no estilo Radmin, sobre OpenVPN.
//!
//! Duas portas para o mesmo motor (`painel.rs`): a tela web servida pelo
//! `phxvpn painel` e a linha de comando abaixo, que fala com o painel pela API.

use phxsql_core::json::Json;
use phxvpn::{http, painel, pg, supervisor};
use std::io::{BufRead, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;

const AJUDA: &str = "phxvpn -- redes virtuais no estilo Radmin, sobre OpenVPN

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

  phxvpn p2p chave [--arquivo p2p.chave]
      Cria (se nao existir) a identidade P2P deste computador e mostra a chave
      publica, que os outros membros usam no --par.

  phxvpn p2p ligar --rede NOME --ip 10.78.0.1/24 [--porta 51820] [--chave p2p.chave]
                   [--interface phx0] --par CHAVE@IP[@HOST:PORTA] [--par ...]
      Modo P2P, sem servidor: liga a placa virtual e fala direto com os pares
      (Linux, como root). Senha da rede por PHXVPN_SENHA_REDE ou no terminal.

  phxvpn versao
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("painel") => cmd_painel(&args[1..]),
        Some("criar-rede") => cmd_rede(&args[1..], true),
        Some("entrar") => cmd_rede(&args[1..], false),
        Some("p2p") => cmd_p2p(&args[1..]),
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
        opcao(args, "--saida").unwrap_or_else(|| r.texto_ou("arquivo", "phxvpn.ovpn").to_string());
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

fn opcoes(args: &[String], nome: &str) -> Vec<String> {
    args.windows(2)
        .filter(|w| w[0] == nome)
        .map(|w| w[1].clone())
        .collect()
}

/// Le a identidade P2P do arquivo, ou cria uma nova (0600, criada ja com a
/// permissao -- sem a janela de um `chmod` depois).
fn identidade(caminho: &str) -> Result<[u8; 32], String> {
    use phxsql_core::hash::{de_hex, para_hex};
    if let Ok(t) = std::fs::read_to_string(caminho) {
        return de_hex(t.trim())
            .and_then(|b| <[u8; 32]>::try_from(b).ok())
            .ok_or_else(|| format!("{caminho}: chave P2P torta"));
    }
    let k = phxsql_core::x25519::gerar_privada();
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let mut f = o
        .open(caminho)
        .map_err(|e| format!("criar {caminho}: {e}"))?;
    writeln!(f, "{}", para_hex(&k)).map_err(|e| e.to_string())?;
    Ok(k)
}

fn cmd_p2p(args: &[String]) -> Result<(), String> {
    use phxsql_core::hash::para_hex;
    let arquivo = opcao(args, "--chave")
        .or_else(|| opcao(args, "--arquivo"))
        .unwrap_or_else(|| "p2p.chave".into());
    match args.first().map(String::as_str) {
        Some("chave") => {
            let k = identidade(&arquivo)?;
            println!("{}", para_hex(&phxsql_core::x25519::chave_publica(&k)));
            Ok(())
        }
        Some("ligar") => p2p_ligar(args, &arquivo),
        _ => Err("use: phxvpn p2p chave | phxvpn p2p ligar ... (veja phxvpn ajuda)".into()),
    }
}

#[cfg(target_os = "linux")]
fn p2p_ligar(args: &[String], arquivo: &str) -> Result<(), String> {
    use phxsql_core::hash::para_hex;
    use phxvpn::p2p;
    let privada = identidade(arquivo)?;
    let rede = opcao(args, "--rede").ok_or("informe --rede")?;
    let (ip, prefixo) = opcao(args, "--ip")
        .ok_or("informe --ip (ex.: 10.78.0.1/24)")?
        .split_once('/')
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .ok_or("--ip precisa do prefixo, ex.: 10.78.0.1/24")?;
    let ip: std::net::Ipv4Addr = ip.parse().map_err(|_| "IP virtual invalido")?;
    let prefixo: u8 = prefixo
        .parse()
        .ok()
        .filter(|p| *p <= 32)
        .ok_or("prefixo invalido")?;
    let porta = opcao(args, "--porta").unwrap_or_else(|| "51820".into());
    let pares = opcoes(args, "--par")
        .iter()
        .map(|t| p2p::ler_par(t))
        .collect::<Result<Vec<_>, _>>()?;
    if pares.is_empty() {
        return Err("informe ao menos um --par".into());
    }
    let senha_rede = senha("PHXVPN_SENHA_REDE", "senha da rede")?;
    // A senha sai do ambiente assim que foi lida: nao fica em /proc/<pid>/environ.
    std::env::remove_var("PHXVPN_SENHA_REDE");
    let psk = p2p::psk_da_rede(&rede, &senha_rede, p2p::ITERACOES_PSK);
    let udp = std::net::UdpSocket::bind(format!("0.0.0.0:{porta}"))
        .map_err(|e| format!("porta UDP {porta}: {e}"))?;
    let interface = opcao(args, "--interface").unwrap_or_else(|| "phx0".into());
    let tun = phxvpn::tun::Tun::abrir(&interface, ip, prefixo, p2p::MTU)?;
    let no = Arc::new(p2p::No::novo(privada, psk, ip, udp, pares));
    eprintln!(
        "phxvpn: P2P no ar -- {interface} {ip}/{prefixo}, UDP {porta}, chave {}",
        para_hex(&no.publica())
    );
    p2p::rodar(no, tun)
}

#[cfg(not(target_os = "linux"))]
fn p2p_ligar(_args: &[String], _arquivo: &str) -> Result<(), String> {
    Err("o modo P2P ainda so roda no Linux (Windows: driver do OpenVPN, em pesquisa)".into())
}
