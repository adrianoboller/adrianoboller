//! O que a linha de comando (`phxvpn ...`) e o console (`phxvpncmd`) fazem
//! -- escrito UMA vez. As duas portas so diferem em como leem as opcoes:
//! `--nome valor` numa, `/nome:valor` na outra. Depois disso, as duas chamam
//! as mesmas funcoes daqui; uma regra corrigida de um lado so nao existe.

#[cfg(target_os = "linux")]
use crate::p2p;
use phxsql_core::hash::{de_hex, para_hex};
use phxsql_core::json::Json;
use std::io::{Read, Write};
use std::net::TcpStream;

pub type R<T> = Result<T, String>;

/// Opcoes nomeadas (podem repetir, como `--par`), bandeiras e posicionais.
#[derive(Default, Debug)]
pub struct Opcoes {
    nomeadas: Vec<(String, String)>,
    bandeiras: Vec<String>,
    pub posicionais: Vec<String>,
}

impl Opcoes {
    /// Estilo Unix: `--nome valor` e `--bandeira`.
    pub fn de_args(args: &[String], bandeiras: &[&str]) -> Opcoes {
        let mut o = Opcoes::default();
        let mut i = 0;
        while i < args.len() {
            let a = &args[i];
            if let Some(nome) = a.strip_prefix("--") {
                if bandeiras.contains(&nome) || i + 1 >= args.len() {
                    o.bandeiras.push(nome.to_lowercase());
                } else {
                    o.nomeadas.push((nome.to_lowercase(), args[i + 1].clone()));
                    i += 1;
                }
            } else {
                o.posicionais.push(a.clone());
            }
            i += 1;
        }
        o
    }

    /// Estilo MS-DOS: `/nome:valor` e `/bandeira`. Nomes sem caixa.
    pub fn de_dos(palavras: &[String]) -> Opcoes {
        let mut o = Opcoes::default();
        for p in palavras {
            match p.strip_prefix('/') {
                Some(resto) => match resto.split_once(':') {
                    Some((k, v)) => o.nomeadas.push((k.to_lowercase(), v.to_string())),
                    None => o.bandeiras.push(resto.to_lowercase()),
                },
                None => o.posicionais.push(p.clone()),
            }
        }
        o
    }

    pub fn um(&self, nome: &str) -> Option<&str> {
        self.nomeadas
            .iter()
            .rev()
            .find(|(k, _)| k == nome)
            .map(|(_, v)| v.as_str())
    }

    pub fn todos(&self, nome: &str) -> Vec<&str> {
        self.nomeadas
            .iter()
            .filter(|(k, _)| k == nome)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    pub fn tem(&self, nome: &str) -> bool {
        self.bandeiras.iter().any(|b| b == nome)
    }
}

/// Divide uma linha em palavras; aspas seguram espaco.
pub fn palavras(linha: &str) -> Vec<String> {
    let mut saida = Vec::new();
    let mut atual = String::new();
    let mut aspas = false;
    let mut tem = false;
    for c in linha.chars() {
        match c {
            '"' => {
                aspas = !aspas;
                tem = true;
            }
            c if c.is_whitespace() && !aspas => {
                if tem {
                    saida.push(std::mem::take(&mut atual));
                    tem = false;
                }
            }
            c => {
                atual.push(c);
                tem = true;
            }
        }
    }
    if tem {
        saida.push(atual);
    }
    saida
}

// ------------------------------------------------------------ painel ----

/// Um pedido ao painel por HTTP/1.1 cru. So `http://` -- o painel nao fala
/// TLS ainda, e isso e dito em vez de escondido.
pub fn pedir(
    base: &str,
    metodo: &str,
    rota: &str,
    token: Option<&str>,
    corpo: Option<&Json>,
) -> R<Json> {
    let hostporta = base
        .strip_prefix("http://")
        .ok_or("o endereco do painel tem de comecar com http://")?
        .trim_end_matches('/');
    // Fora do loopback, senha, chave privada e tls-crypt passariam em claro
    // (achado A4). So com a escolha escrita no ambiente.
    let host = hostporta.rsplit_once(':').map_or(hostporta, |(h, _)| h);
    let local = matches!(host, "127.0.0.1" | "localhost" | "[::1]") || host.starts_with("127.");
    if !local && std::env::var("PHXVPN_ACEITO_SEM_TLS").as_deref() != Ok("1") {
        return Err(format!(
            "o painel {hostporta} nao e local e fala HTTP sem TLS: senha e perfil iriam em \
             claro. Use por um tunel/proxy com TLS, ou defina PHXVPN_ACEITO_SEM_TLS=1 \
             sabendo disso"
        ));
    }
    let mut fio = TcpStream::connect(hostporta).map_err(|e| format!("painel {hostporta}: {e}"))?;
    let corpo = corpo.map(Json::escrever).unwrap_or_default();
    let auth = token
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    write!(
        fio,
        "{metodo} {rota} HTTP/1.1\r\nHost: {hostporta}\r\nContent-Type: application/json\r\n{auth}\
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

pub fn login(painel: &str, usuario: &str, senha: &str) -> R<(String, bool)> {
    let r = pedir(
        painel,
        "POST",
        "/api/login",
        None,
        Some(&Json::objeto(vec![
            ("usuario", Json::texto_de(usuario)),
            ("senha", Json::texto_de(senha)),
        ])),
    )?;
    Ok((
        r.texto_ou("token", "").to_string(),
        r.booleano_ou("admin", false),
    ))
}

/// Cria ou entra numa rede do painel e grava o perfil `.ovpn`. Devolve o
/// caminho gravado.
pub fn perfil_de_rede(
    painel: &str,
    token: &str,
    criar: bool,
    rede: &str,
    senha_rede: &str,
    finalidade: &str,
    saida: Option<&str>,
) -> R<String> {
    let mut pedido = vec![
        ("nome", Json::texto_de(rede)),
        ("senha", Json::texto_de(senha_rede)),
    ];
    if criar {
        pedido.push(("finalidade", Json::texto_de(finalidade)));
    }
    let rota = if criar {
        "/api/redes"
    } else {
        "/api/redes/entrar"
    };
    let r = pedir(
        painel,
        "POST",
        rota,
        Some(token),
        Some(&Json::objeto(pedido)),
    )?;
    let arquivo = saida
        .map(str::to_string)
        .unwrap_or_else(|| r.texto_ou("arquivo", "phxvpn.ovpn").to_string());
    gravar_secreto(&arquivo, r.texto_ou("perfil", "").as_bytes(), false)?;
    Ok(arquivo)
}

/// Grava arquivo que carrega segredo ja com 0600 na criacao -- sem a janela
/// de um `chmod` depois (achado M2 da revisao de seguranca).
pub fn gravar_secreto(caminho: &str, dados: &[u8], so_se_novo: bool) -> R<()> {
    let mut o = std::fs::OpenOptions::new();
    o.write(true);
    if so_se_novo {
        o.create_new(true);
    } else {
        o.create(true).truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let mut f = o
        .open(caminho)
        .map_err(|e| format!("gravar {caminho}: {e}"))?;
    f.write_all(dados)
        .map_err(|e| format!("gravar {caminho}: {e}"))
}

// --------------------------------------------------------------- p2p ----

/// Le a identidade P2P do arquivo, ou cria uma nova.
pub fn identidade(caminho: &str) -> R<[u8; 32]> {
    if let Ok(t) = std::fs::read_to_string(caminho) {
        return de_hex(t.trim())
            .and_then(|b| <[u8; 32]>::try_from(b).ok())
            .ok_or_else(|| format!("{caminho}: chave P2P torta"));
    }
    let k = phxsql_core::x25519::gerar_privada();
    gravar_secreto(caminho, format!("{}\n", para_hex(&k)).as_bytes(), true)?;
    Ok(k)
}

pub fn chave_publica_hex(privada: &[u8; 32]) -> String {
    para_hex(&phxsql_core::x25519::chave_publica(privada))
}

/// Monta o no P2P a partir das opcoes (as mesmas nas duas portas):
/// `rede`, `ip` (com prefixo), `par` (repete), `porta`, `modo`, `repasse`,
/// `chave`, `interface`. Devolve o no, a placa ja ligada e um resumo.
#[cfg(target_os = "linux")]
pub fn p2p_preparar(
    o: &Opcoes,
    senha_rede: &str,
) -> R<(std::sync::Arc<p2p::No>, crate::tun::Tun, String)> {
    let privada = identidade(o.um("chave").unwrap_or("p2p.chave"))?;
    let rede = o.um("rede").ok_or("informe a rede")?;
    let (ip, prefixo) = o
        .um("ip")
        .ok_or("informe o ip (ex.: 10.78.0.1/24)")?
        .split_once('/')
        .ok_or("o ip precisa do prefixo, ex.: 10.78.0.1/24")?;
    let ip: std::net::Ipv4Addr = ip.parse().map_err(|_| "IP virtual invalido")?;
    let prefixo: u8 = prefixo
        .parse()
        .ok()
        .filter(|p| *p <= 32)
        .ok_or("prefixo invalido")?;
    let porta = o.um("porta").unwrap_or("51820");
    let pares = o
        .todos("par")
        .iter()
        .map(|t| p2p::ler_par(t))
        .collect::<R<Vec<_>>>()?;
    if pares.is_empty() {
        return Err("informe ao menos um par (CHAVE@IP[@HOST:PORTA])".into());
    }
    let modo = p2p::Modo::de_texto(o.um("modo").unwrap_or("direto"))?;
    let repasse = match o.um("repasse") {
        Some(t) => {
            let (chave, end) = t
                .split_once('@')
                .ok_or("repasse no formato CHAVE@HOST:PORTA")?;
            let par = p2p::ler_par(&format!("{chave}@0.0.0.0@{end}"))?;
            Some(p2p::RepasseCfg {
                endereco: par.endereco.ok_or("repasse sem endereco")?,
                publica: par.publica,
            })
        }
        None => None,
    };
    let psk = p2p::psk_da_rede(rede, senha_rede, p2p::ITERACOES_PSK);
    let udp = std::net::UdpSocket::bind(format!("0.0.0.0:{porta}"))
        .map_err(|e| format!("porta UDP {porta}: {e}"))?;
    let interface = o.um("interface").unwrap_or("phx0");
    let no = p2p::No::novo(privada, psk, ip, udp, pares).com_repasse(modo, repasse)?;
    let tun = crate::tun::Tun::abrir(interface, ip, prefixo, p2p::MTU)?;
    let resumo = format!(
        "P2P no ar -- {interface} {ip}/{prefixo}, UDP {porta}, modo {modo:?}, chave {}",
        para_hex(&no.publica())
    );
    Ok((std::sync::Arc::new(no), tun, resumo))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn as_duas_portas_leem_as_mesmas_opcoes() {
        let unix = Opcoes::de_args(
            &["--rede", "Matriz", "--par", "a", "--par", "b", "--conectar"].map(String::from),
            &["conectar"],
        );
        let dos = Opcoes::de_dos(&palavras("/REDE:Matriz /par:a /par:b /conectar"));
        for o in [&unix, &dos] {
            assert_eq!(o.um("rede"), Some("Matriz"));
            assert_eq!(o.todos("par"), vec!["a", "b"]);
            assert!(o.tem("conectar"));
        }
    }

    #[test]
    fn aspas_seguram_espaco() {
        assert_eq!(
            palavras(r#"criarrede "Filial Sul" /finalidade:"ERP e NF""#),
            vec!["criarrede", "Filial Sul", "/finalidade:ERP e NF"]
        );
    }

    #[test]
    fn segredo_nasce_0600() {
        let c = std::env::temp_dir().join(format!("phxvpn-seg-{}", std::process::id()));
        let c = c.to_str().unwrap();
        gravar_secreto(c, b"x", true).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let m = std::fs::metadata(c).unwrap().permissions().mode() & 0o777;
            assert_eq!(m, 0o600);
        }
        assert!(
            gravar_secreto(c, b"y", true).is_err(),
            "so_se_novo nao sobrescreve"
        );
        let _ = std::fs::remove_file(c);
    }
}
