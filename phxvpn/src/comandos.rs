//! O que a linha de comando (`phxvpn ...`) e o console (`phxvpncmd`) fazem
//! -- escrito UMA vez. As duas portas so diferem em como leem as opcoes:
//! `--nome valor` numa, `/nome:valor` na outra. Depois disso, as duas chamam
//! as mesmas funcoes daqui; uma regra corrigida de um lado so nao existe.

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
            // Quem cadastrou o autenticador manda o codigo pelo ambiente.
            (
                "codigo",
                Json::texto_de(std::env::var("PHXVPN_CODIGO").unwrap_or_default()),
            ),
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
    o: &Opcoes,
) -> R<String> {
    let (finalidade, saida) = (o.um("finalidade").unwrap_or(""), o.um("saida"));
    let mut pedido = vec![
        ("nome", Json::texto_de(rede)),
        ("senha", Json::texto_de(senha_rede)),
    ];
    if criar {
        pedido.push(("finalidade", Json::texto_de(finalidade)));
        // Rede OpenVPN por TCP (e porta, so o administrador): a rede que so
        // deixa sair TCP/443.
        if let Some(p) = nao_vazia(o.um("protocolo")) {
            pedido.push(("protocolo", Json::texto_de(p)));
        }
        if let Some(p) = nao_vazia(o.um("porta")) {
            let n: u16 = p.parse().map_err(|_| format!("porta invalida: {p}"))?;
            pedido.push(("porta", Json::de_i64(n as i64)));
        }
    }
    if let Some(p) = nao_vazia(o.um("http-proxy")) {
        pedido.push(("http_proxy", Json::texto_de(p)));
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
    // No Windows, so o dono -- ANTES de o segredo entrar no arquivo.
    crate::acl::so_do_dono(std::path::Path::new(caminho))?;
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

/// O arquivo da rede: `/arquivo:` (ou `--arquivo`), ou `<rede>.p2p`.
pub fn arquivo_da_rede(o: &Opcoes) -> R<String> {
    match (
        o.um("arquivo"),
        o.um("rede").or(o.posicionais.first().map(String::as_str)),
    ) {
        (Some(a), _) => Ok(a.to_string()),
        (None, Some(r)) => Ok(crate::rede_p2p::Rede::caminho(r)),
        _ => Err("informe a rede".into()),
    }
}

/// `p2p criar`: a rede nasce neste computador, sem pares ainda.
pub fn p2p_criar(o: &Opcoes) -> R<String> {
    use crate::rede_p2p::Rede;
    let nome = o.um("rede").ok_or("informe a rede")?;
    let (ip, prefixo) = o
        .um("ip")
        .unwrap_or("10.78.0.1/24")
        .split_once('/')
        .ok_or("o ip precisa do prefixo, ex.: 10.78.0.1/24")?;
    let mut r = Rede::nova(
        nome,
        ip.parse().map_err(|_| "IP virtual invalido")?,
        prefixo
            .parse()
            .ok()
            .filter(|p: &u8| (8..=30).contains(p))
            .ok_or("prefixo de 8 a 30")?,
        o.um("porta")
            .unwrap_or("51820")
            .parse()
            .map_err(|_| "porta invalida")?,
    );
    r.modo = o.um("modo").unwrap_or("direto").to_string();
    p2p::Modo::de_texto(&r.modo)?;
    r.repasse = o.um("repasse").map(str::to_string);
    if let Some(u) = o.um("repasse-usuario") {
        crate::repasse::validar_usuario(u)?;
        r.repasse_usuario = Some(u.to_string());
    }
    // O fio ate o repasse fica no arquivo (a senha do proxy, nunca).
    r.fio = escolha_do_fio(o, None)?.map(|e| e.texto().to_string());
    r.repasse_tcp = porta_tcp_do_repasse(o, None)?;
    r.proxy = nao_vazia(o.um("proxy")).map(str::to_string);
    r.proxy_usuario = nao_vazia(o.um("proxy-usuario")).map(str::to_string);
    if let Some(p) = &r.proxy {
        crate::fio::Proxy::novo(p, None)?;
    }
    let caminho = arquivo_da_rede(o)?;
    if std::path::Path::new(&caminho).exists() {
        return Err(format!(
            "{caminho} ja existe: a rede ja foi criada ou recebida aqui"
        ));
    }
    let privada = identidade(o.um("chave").unwrap_or("p2p.chave"))?;
    r.apelido = apelido(o)?;
    r.descoberta = !o.tem("sem-descoberta");
    // Rede nova nasce com rol assinado: quem cria e o dono, e o primeiro
    // membro do rol e ele mesmo.
    r.dono = Some(crate::rol::publica_do_dono(&privada, nome));
    r.rol = Some(crate::rol::Rol::primeiro(
        nome,
        crate::rol::Membro {
            chave: phxsql_core::x25519::chave_publica(&privada),
            ip: r.ip,
            nome: r.apelido.clone(),
            farol: None,
        },
        &privada,
    )?);
    r.gravar(&caminho)?;
    Ok(format!(
        "rede {nome} criada em {caminho} -- {}/{}; convide com p2p convidar",
        r.ip, r.prefixo
    ))
}

/// `p2p convidar`: gera o codigo e deixa a ficha aberta no arquivo.
pub fn p2p_convidar(o: &Opcoes, senha_rede: &str) -> R<String> {
    use crate::rede_p2p::{convidar, Rede};
    let caminho = arquivo_da_rede(o)?;
    let mut r = Rede::ler(&caminho)?;
    let privada = identidade(o.um("chave").unwrap_or("p2p.chave"))?;
    so_o_dono(&r, &privada, "convida")?;
    let horas: u64 = o
        .um("validade")
        .unwrap_or("24")
        .trim_end_matches('h')
        .parse()
        .map_err(|_| "validade em horas")?;
    let psk = p2p::psk_da_rede(&r.nome, senha_rede, p2p::ITERACOES_PSK);
    let codigo = convidar(
        &mut r,
        &psk,
        phxsql_core::x25519::chave_publica(&privada),
        o.um("endereco").map(str::to_string),
        horas.clamp(1, 24 * 30) * 3600,
    )?;
    r.gravar(&caminho)?;
    Ok(codigo)
}

/// `p2p entrar <codigo>`: abre o convite com a senha e grava a rede aqui.
pub fn p2p_entrar(codigo: &str, senha_rede: &str, o: &Opcoes) -> R<String> {
    use crate::rede_p2p::{abrir_convite, rede_do_convidado, rede_do_convite, Rede};
    let nome = rede_do_convite(codigo)?;
    let psk = p2p::psk_da_rede(&nome, senha_rede, p2p::ITERACOES_PSK);
    let c = abrir_convite(codigo, &psk)?;
    let caminho = o
        .um("arquivo")
        .map(str::to_string)
        .unwrap_or_else(|| Rede::caminho(&nome));
    if std::path::Path::new(&caminho).exists() {
        return Err(format!(
            "{caminho} ja existe: esta rede ja esta neste computador"
        ));
    }
    identidade(o.um("chave").unwrap_or("p2p.chave"))?;
    let mut r = rede_do_convidado(
        &c,
        o.um("porta")
            .unwrap_or("51820")
            .parse()
            .map_err(|_| "porta invalida")?,
    );
    r.apelido = apelido(o)?;
    r.descoberta = !o.tem("sem-descoberta");
    r.gravar(&caminho)?;
    Ok(format!(
        "convite aceito: rede {nome}, seu IP {}/{}, anfitriao {} -- ligue com p2p ligar /rede:{nome}",
        r.ip, r.prefixo, c.anfitriao.ip
    ))
}

/// `--apelido`: o nome deste computador no rol (opcional, ate 32 bytes).
fn apelido(o: &Opcoes) -> R<Option<String>> {
    match o.um("apelido") {
        Some(a) if a.len() > crate::rol::TETO_NOME => {
            Err(format!("apelido acima de {} bytes", crate::rol::TETO_NOME))
        }
        Some(a) if !a.is_empty() => Ok(Some(a.to_string())),
        _ => Ok(None),
    }
}

/// Em rede de rol assinado, so o dono (quem criou) muda quem e membro.
fn so_o_dono(r: &crate::rede_p2p::Rede, privada: &[u8; 32], acao: &str) -> R<()> {
    match r.dono {
        Some(d) if crate::rol::publica_do_dono(privada, &r.nome) != d => Err(format!(
            "nesta rede so quem a criou {acao}: o rol de membros e assinado pela chave dele"
        )),
        _ => Ok(()),
    }
}

/// `p2p remover --rede NOME --ip IP` (ou a chave publica): rol novo sem o
/// membro, gravado no arquivo da rede. Com a rede ligada, o no o le do disco
/// em ate 2 s e o espalha pela malha; quem o recebe derruba o removido.
pub fn p2p_remover(o: &Opcoes) -> R<String> {
    use crate::rede_p2p::Rede;
    let caminho = arquivo_da_rede(o)?;
    let mut r = Rede::ler(&caminho)?;
    let privada = identidade(o.um("chave").unwrap_or("p2p.chave"))?;
    if r.dono.is_none() {
        return Err(
            "rede sem rol assinado (criada antes dele): nao ha como remover membro -- \
             crie a rede de novo para ter o rol"
                .into(),
        );
    }
    so_o_dono(&r, &privada, "remove membro")?;
    let rol = r.rol.clone().ok_or("rede sem rol no arquivo")?;
    let alvo = o.um("ip").or(o.posicionais.first().map(String::as_str));
    let membro = rol
        .membros
        .iter()
        .find(|m| {
            alvo.is_some_and(|a| m.ip.to_string() == a || para_hex(&m.chave) == a.to_lowercase())
        })
        .ok_or("informe --ip IP_VIRTUAL (ou a chave publica) de um membro do rol")?
        .clone();
    if membro.chave == phxsql_core::x25519::chave_publica(&privada) {
        return Err("o dono nao sai da propria rede".into());
    }
    let novo = rol.sem(&membro.chave, &privada)?;
    r.pares.retain(|p| p.chave != membro.chave);
    let versao = novo.versao;
    r.rol = Some(novo);
    r.gravar(&caminho)?;
    Ok(format!(
        "{} removido da rede {} -- rol versao {versao}, {} membros",
        membro.ip,
        r.nome,
        r.rol.as_ref().map_or(0, |x| x.membros.len())
    ))
}

/// `p2p farol --rede NOME --ip IP --endereco IP:PORTA [--tirar]`.
///
/// No DONO, assina um rol novo com o farol daquele membro marcado (ou
/// desmarcado com `--tirar`) -- a autoridade. No proprio membro (dono ou
/// nao), grava `"farol": true` no arquivo dele -- o consentimento. As duas
/// metades sao o mesmo comando para ninguem precisar lembrar de duas.
pub fn p2p_farol(o: &Opcoes) -> R<String> {
    use crate::rede_p2p::Rede;
    let caminho = arquivo_da_rede(o)?;
    let mut r = Rede::ler(&caminho)?;
    let privada = identidade(o.um("chave").unwrap_or("p2p.chave"))?;
    let minha = phxsql_core::x25519::chave_publica(&privada);
    if r.dono.is_none() {
        return Err(
            "rede sem rol assinado (criada antes dele): o farol mora no rol -- \
             crie a rede de novo"
                .into(),
        );
    }
    let tirar = o.tem("tirar");
    let rol = r.rol.clone().ok_or("rede sem rol no arquivo")?;
    let alvo = o.um("ip").or(o.posicionais.first().map(String::as_str));
    let membro = match alvo {
        Some(a) => rol
            .membros
            .iter()
            .find(|m| m.ip.to_string() == a || para_hex(&m.chave) == a.to_lowercase())
            .ok_or("informe --ip IP_VIRTUAL (ou a chave publica) de um membro do rol")?
            .clone(),
        None => rol
            .membro(&minha)
            .ok_or("este computador nao esta no rol; informe --ip")?
            .clone(),
    };
    let dono = crate::rol::publica_do_dono(&privada, &r.nome) == r.dono.unwrap_or_default();
    let eu = membro.chave == minha;
    if !dono && !eu {
        return Err("so o dono marca o farol de outro membro (o rol e assinado por ele)".into());
    }
    let mut feito = Vec::new();
    if dono {
        let endereco = if tirar {
            None
        } else {
            let e = o
                .um("endereco")
                .ok_or("informe --endereco IP_PUBLICO:PORTA (o que os outros alcancam)")?;
            // IP literal: o endereco vai assinado no rol, e um nome
            // resolvido depois poderia apontar para outro lugar.
            Some(
                e.parse::<std::net::SocketAddr>()
                    .map_err(|_| "--endereco no formato IP:PORTA (sem nome de host)")?,
            )
        };
        let novo = rol.com_farol(&membro.chave, endereco, &privada)?;
        feito.push(format!("rol versao {}", novo.versao));
        if endereco.is_some() {
            // Rol com farol sai no formato v2, que no anterior a esta versao
            // nao le: ele fica com o rol velho -- e deixa de receber as
            // mudancas seguintes, inclusive as remocoes.
            feito.push(
                "ATENCAO: nos com phxvpn anterior ao farol deixam de receber o rol \
                 (inclusive remocoes) -- atualize todos os membros"
                    .into(),
            );
        }
        r.rol = Some(novo);
    }
    if eu {
        r.farol = !tirar;
        feito.push(if tirar {
            "este computador nao serve mais".into()
        } else {
            "este computador aceita servir".to_string()
        });
    }
    r.gravar(&caminho)?;
    let aviso = if dono && !eu && !tirar {
        " -- no computador dele: p2p farol --rede NOME (ou ligar com --farol)"
    } else if !dono && !tirar {
        " -- falta o dono marcar no rol (p2p farol no computador dele)"
    } else {
        ""
    };
    Ok(format!(
        "farol de {} {}: {}{aviso}",
        membro.ip,
        if tirar { "tirado" } else { "marcado" },
        feito.join("; ")
    ))
}

/// Monta o no P2P. Com o arquivo da rede (`p2p criar` / `p2p entrar`), tudo
/// sai dele; sem arquivo, das opcoes (`ip`, `par` repetido, `porta`, `modo`,
/// `repasse`). Devolve o no, a placa ja ligada e um resumo.
///
/// `senha_proxy`: ver o comentario de `fio_do_no` -- por parametro, nunca
/// pelo ambiente do processo.
#[cfg(any(target_os = "linux", windows))]
pub fn p2p_preparar(
    o: &Opcoes,
    rede: Segredo,
    repasse: Option<SegredoRepasse>,
    senha_proxy: Option<String>,
) -> R<(std::sync::Arc<p2p::No>, crate::tun::Tun, String)> {
    let (no, ip, prefixo, porta, modo) = p2p_montar(o, rede, repasse, senha_proxy)?;
    // No Windows e o nome do adaptador TAP criado pelo `p2p placa`.
    let padrao = if cfg!(windows) { "phxvpn" } else { "phx0" };
    let interface = o.um("interface").unwrap_or(padrao);
    let tun = crate::tun::Tun::abrir(interface, ip, prefixo, p2p::MTU)?;
    let resumo = format!(
        "P2P no ar -- {interface} {ip}/{prefixo}, UDP {porta}, modo {modo:?}{}, chave {}",
        no.fio()
            .map(|f| format!(", fio do repasse {}", f.escolha().texto()))
            .unwrap_or_default(),
        para_hex(&no.publica())
    );
    let no = std::sync::Arc::new(no);
    // O USB da rede sobe junto, preso a esta placa: compartilhar depois, com
    // a rede ja ligada, vale na hora (o servidor rele o arquivo). Porta 3240
    // ocupada (um usbipd rodando) nao impede a rede de ligar.
    #[cfg(target_os = "linux")]
    if let Ok(caminho) = arquivo_da_rede(o) {
        let n = std::sync::Arc::clone(&no);
        if let Err(e) = crate::usb::servir_rede(
            caminho,
            Some(interface.to_string()),
            move || n.desligado(),
            |m| eprintln!("phxvpn usb: {m}"),
        ) {
            eprintln!("phxvpn usb: sem compartilhamento de USB nesta rede -- {e}");
        }
    }
    Ok((no, tun, resumo))
}

type Montado = (p2p::No, std::net::Ipv4Addr, u8, u16, p2p::Modo);

/// A parte do preparo que nao depende de placa (e roda em qualquer sistema).
/// O segredo da rede: a senha (deriva a PSK aqui) ou a PSK ja derivada
/// (lembrada pelo programa de mesa).
#[derive(Clone, Copy)]
pub enum Segredo<'a> {
    Senha(&'a str),
    Psk([u8; 32]),
}

/// O segredo da conta no servidor intermediario: a senha, ou a credencial
/// ja derivada.
#[derive(Clone, Copy)]
pub enum SegredoRepasse<'a> {
    Senha(&'a str),
    Credencial([u8; 32]),
}

fn nao_vazia(t: Option<&str>) -> Option<&str> {
    t.filter(|t| !t.trim().is_empty())
}

/// `--tcp` (bandeira) ou `--fio auto|udp|tcp`, senao o do arquivo.
fn escolha_do_fio(o: &Opcoes, arquivo: Option<&str>) -> R<Option<crate::fio::Escolha>> {
    if o.tem("tcp") {
        return Ok(Some(crate::fio::Escolha::Tcp));
    }
    nao_vazia(o.um("fio"))
        .or(arquivo)
        .map(crate::fio::Escolha::de_texto)
        .transpose()
}

fn porta_tcp_do_repasse(o: &Opcoes, arquivo: Option<u16>) -> R<Option<u16>> {
    match nao_vazia(o.um("repasse-tcp")) {
        Some(p) => p
            .parse::<u16>()
            .ok()
            .filter(|p| *p > 0)
            .map(Some)
            .ok_or_else(|| format!("porta TCP do repasse invalida: {p}")),
        None => Ok(arquivo),
    }
}

/// O usuario do proxy HTTP desta rede (opcao ou arquivo), para quem chama
/// saber se precisa pedir a senha dele (PHXVPN_SENHA_PROXY).
pub fn usuario_do_proxy(o: &Opcoes) -> Option<String> {
    nao_vazia(o.um("proxy-usuario"))
        .map(str::to_string)
        .or_else(|| {
            arquivo_da_rede(o)
                .ok()
                .and_then(|c| crate::rede_p2p::Rede::ler(&c).ok())
                .and_then(|r| r.proxy_usuario)
        })
}

/// O fio ate o repasse, das opcoes e do arquivo. Padrao `auto`: UDP e, sem
/// confirmacao, TCP na 443. Proxy implica TCP (UDP nao passa por `CONNECT`).
///
/// A senha do proxy chega por PARAMETRO, nunca pelo ambiente do processo:
/// `set_var`/`remove_var` concorrentes com `getenv` de outra thread sao
/// comportamento indefinido na `glibc` (por isso viraram `unsafe` na edicao
/// 2024), e a mesa (`mesa.rs`) e um processo longo, de varias threads, que
/// pode ligar duas redes com proxies diferentes ao mesmo tempo -- variavel
/// de ambiente e um estado GLOBAL do processo, e duas ligacoes trocariam a
/// senha uma da outra. Quem le do ambiente ou do terminal (a CLI, uma vez
/// so, no comeco) e quem passa por parametro daqui pra baixo.
fn fio_do_no(
    o: &Opcoes,
    rede: Option<&crate::rede_p2p::Rede>,
    senha_proxy: Option<String>,
) -> R<crate::fio::CfgFio> {
    use crate::fio::{CfgFio, Escolha, Proxy};
    let escolha = escolha_do_fio(o, rede.and_then(|r| r.fio.as_deref()))?;
    let porta_tcp = porta_tcp_do_repasse(o, rede.and_then(|r| r.repasse_tcp))?.unwrap_or(443);
    let proxy_end = nao_vazia(o.um("proxy"))
        .map(str::to_string)
        .or_else(|| rede.and_then(|r| r.proxy.clone()));
    let proxy = match proxy_end {
        Some(end) => {
            let cred = match (usuario_do_proxy(o), senha_proxy) {
                (Some(u), Some(s)) => Some((u, s)),
                (Some(u), None) => return Err(format!("falta a senha do usuario {u} no proxy")),
                (None, _) => None,
            };
            Some(Proxy::novo(&end, cred)?)
        }
        None => None,
    };
    let escolha = match (escolha, &proxy) {
        (Some(Escolha::Udp), Some(_)) => {
            return Err("proxy HTTP so leva TCP: tire --fio udp ou o --proxy".into())
        }
        (_, Some(_)) => Escolha::Tcp,
        (Some(e), None) => e,
        (None, None) => Escolha::Auto,
    };
    Ok(CfgFio {
        escolha,
        porta_tcp,
        proxy,
        ..CfgFio::default()
    })
}

/// O usuario do servidor intermediario desta rede (opcao ou arquivo), para
/// quem chama saber se precisa pedir a senha dele.
pub fn usuario_do_repasse(o: &Opcoes) -> Option<String> {
    o.um("repasse-usuario").map(str::to_string).or_else(|| {
        arquivo_da_rede(o)
            .ok()
            .and_then(|c| crate::rede_p2p::Rede::ler(&c).ok())
            .and_then(|r| r.repasse_usuario)
    })
}

pub fn p2p_montar(
    o: &Opcoes,
    segredo: Segredo,
    segredo_repasse: Option<SegredoRepasse>,
    senha_proxy: Option<String>,
) -> R<Montado> {
    use crate::rede_p2p::Rede;
    let privada = identidade(o.um("chave").unwrap_or("p2p.chave"))?;
    let caminho = arquivo_da_rede(o)
        .ok()
        .filter(|c| std::path::Path::new(c).exists());
    let rede = caminho.as_deref().map(Rede::ler).transpose()?;
    let (nome, ip, prefixo, porta, modo_t, repasse_t, pares) = match &rede {
        Some(r) => (
            r.nome.clone(),
            r.ip,
            r.prefixo,
            o.um("porta")
                .map(str::to_string)
                .unwrap_or(r.porta.to_string()),
            o.um("modo").unwrap_or(&r.modo).to_string(),
            o.um("repasse").map(str::to_string).or(r.repasse.clone()),
            r.pares
                .iter()
                .map(|p| {
                    Ok(p2p::ParConfig {
                        publica: p.chave,
                        ip: p.ip,
                        endereco: match &p.endereco {
                            Some(e) => std::net::ToSocketAddrs::to_socket_addrs(e.as_str())
                                .ok()
                                .and_then(|mut i| i.next()),
                            None => None,
                        },
                    })
                })
                .collect::<R<Vec<_>>>()?,
        ),
        None => {
            let nome = o.um("rede").ok_or("informe a rede")?.to_string();
            let (ip, prefixo) = o
                .um("ip")
                .ok_or("sem arquivo da rede: informe o ip (ex.: 10.78.0.1/24) ou use p2p criar / p2p entrar")?
                .split_once('/')
                .ok_or("o ip precisa do prefixo, ex.: 10.78.0.1/24")?;
            let pares = o
                .todos("par")
                .iter()
                .map(|t| p2p::ler_par(t))
                .collect::<R<Vec<_>>>()?;
            if pares.is_empty() {
                return Err("informe ao menos um par (CHAVE@IP[@HOST:PORTA])".into());
            }
            (
                nome,
                ip.parse().map_err(|_| "IP virtual invalido")?,
                prefixo
                    .parse()
                    .ok()
                    .filter(|p: &u8| *p <= 32)
                    .ok_or("prefixo invalido")?,
                o.um("porta").unwrap_or("51820").to_string(),
                o.um("modo").unwrap_or("direto").to_string(),
                o.um("repasse").map(str::to_string),
                pares,
            )
        }
    };
    let porta: u16 = porta.parse().map_err(|_| "porta invalida")?;
    let modo = p2p::Modo::de_texto(&modo_t)?;
    let repasse = match repasse_t {
        Some(t) => {
            let (chave, end) = t
                .split_once('@')
                .ok_or("repasse no formato CHAVE@HOST:PORTA")?;
            let par = p2p::ler_par(&format!("{chave}@0.0.0.0@{end}"))?;
            let conta = match (usuario_do_repasse(o), segredo_repasse) {
                (Some(u), Some(s)) => Some(crate::repasse::Conta {
                    credencial: match s {
                        SegredoRepasse::Senha(s) => {
                            crate::repasse::credencial(&u, s, crate::repasse::ITERACOES_CONTA)
                        }
                        SegredoRepasse::Credencial(c) => c,
                    },
                    usuario: u,
                }),
                (Some(u), None) => {
                    return Err(format!(
                        "falta a senha do usuario {u} no servidor intermediario"
                    ))
                }
                (None, _) => None,
            };
            Some(p2p::RepasseCfg {
                endereco: par.endereco.ok_or("repasse sem endereco")?,
                publica: par.publica,
                conta,
            })
        }
        None => None,
    };
    let psk = match segredo {
        Segredo::Senha(s) => p2p::psk_da_rede(&nome, s, p2p::ITERACOES_PSK),
        Segredo::Psk(k) => k,
    };
    let udp = std::net::UdpSocket::bind(format!("0.0.0.0:{porta}"))
        .map_err(|e| format!("porta UDP {porta}: {e}"))?;
    let tem_repasse = repasse.is_some();
    let descoberta = !o.tem("sem-descoberta") && rede.as_ref().map_or(true, |r| r.descoberta);
    // Rede de rol assinado pode ter farol no rol (ou vir a ter): o `auto` e
    // o `repasse` sem `--repasse` valem nela -- o intermediario e o farol.
    // O convidado ainda nao tem o rol na primeira vez que liga, entao a
    // conferencia e «rede assinada», nao «ha farol agora».
    let so_farol = modo != p2p::Modo::Direto
        && repasse.is_none()
        && rede.as_ref().is_some_and(|r| r.dono.is_some());
    let servir = o.tem("farol") || rede.as_ref().is_some_and(|r| r.farol);
    let mbit = o
        .um("farol-mbit")
        .map(|t| t.parse::<u32>().ok().filter(|m| *m > 0))
        .map(|m| m.ok_or("--farol-mbit: inteiro positivo (Mbit/s)"))
        .transpose()?;
    let mut no = p2p::No::novo(privada, psk, ip, udp, pares);
    no = if so_farol {
        no.com_repasse(p2p::Modo::Direto, None)?
            .com_modo_so_farol(modo)
    } else {
        no.com_repasse(modo, repasse)?
    }
    .com_descoberta(descoberta)
    .com_farol(servir, mbit);
    if o.tem("sem-perfuracao") {
        no = no.sem_perfuracao();
    }
    if tem_repasse {
        no = no.com_fio(fio_do_no(o, rede.as_ref(), senha_proxy)?)?;
    } else if o.tem("tcp") || nao_vazia(o.um("proxy")).is_some() {
        return Err("TCP e proxy sao o fio ate o repasse: informe --repasse".into());
    }
    if let (Some(r), Some(c)) = (rede, caminho) {
        no = no.com_rede(r, c);
    }
    Ok((no, ip, prefixo, porta, modo))
}

/// `usb ...` -- a mesma funcao para `phxvpn usb` e para o `USB` do console.
/// `o.posicionais[0]` e a acao; `--sysfs` so existe para as provas.
pub fn usb(o: &Opcoes) -> R<String> {
    use crate::rede_p2p::Rede;
    use crate::usb::{self, Sysfs};
    let sysfs = match o.um("sysfs") {
        Some(r) => Sysfs::em(std::path::Path::new(r)),
        None => Sysfs::do_ambiente(),
    };
    let arg = |i: usize, oque: &str| -> R<&str> {
        o.posicionais
            .get(i)
            .map(String::as_str)
            .ok_or_else(|| format!("falta {oque}"))
    };
    let ip = |t: &str| -> R<std::net::Ipv4Addr> {
        t.parse().map_err(|_| format!("IP virtual invalido: {t}"))
    };
    match o.posicionais.first().map(|a| a.to_lowercase()).as_deref() {
        Some("listar") | None => {
            let rede = arquivo_da_rede_sem_posicional(o)
                .ok()
                .and_then(|c| Rede::ler(&c).ok());
            let v = usb::locais_da_rede(&sysfs, rede.as_ref())?;
            if v.is_empty() {
                return Ok("nenhum dispositivo USB neste computador\n".into());
            }
            Ok(v.iter()
                .map(|l| {
                    let marca = match (l.preso, l.nesta_rede) {
                        (true, true) => "  [compartilhado nesta rede]",
                        (true, false) => "  [compartilhado]",
                        _ => "",
                    };
                    format!("{}{marca}\n", l.dispositivo.resumo())
                })
                .collect())
        }
        Some("compartilhar") => {
            let busid = arg(1, "o busid (veja: usb listar)")?;
            let caminho = arquivo_da_rede_sem_posicional(o)?;
            Ok(format!("{}\n", usb::compartilhar_na_rede(&sysfs, &caminho, busid)?))
        }
        Some("parar") => {
            let busid = arg(1, "o busid")?;
            let caminho = arquivo_da_rede_sem_posicional(o)?;
            Ok(format!("{}\n", usb::parar_na_rede(&sysfs, &caminho, busid)?))
        }
        Some("remotos") => {
            let de = ip(arg(1, "o IP virtual do membro")?)?;
            let v = usb::remotos(de)?;
            if v.is_empty() {
                return Ok(format!("{de} não compartilha nada nesta rede\n"));
            }
            Ok(v.iter().map(|d| format!("{}\n", d.resumo())).collect())
        }
        Some("usar") => {
            let de = ip(arg(1, "o IP virtual do membro")?)?;
            let busid = arg(2, "o busid (veja: usb remotos IP)")?;
            Ok(format!("{}\n", usb::usar(&sysfs, de, busid)?))
        }
        Some("soltar") => {
            let porta = arg(1, "a porta (veja: usb portas)")?
                .parse()
                .map_err(|_| "porta invalida")?;
            Ok(format!("{}\n", usb::soltar(&sysfs, porta)?))
        }
        Some("portas") => {
            let v = usb::em_uso(&sysfs)?;
            if v.is_empty() {
                return Ok("nenhum USB remoto em uso aqui\n".into());
            }
            Ok(v.iter()
                .map(|u| format!("porta {:<3} {}\n", u.porta, u.texto))
                .collect())
        }
        Some("servir") => usb_servir(o, sysfs),
        Some(outro) => Err(format!(
            "usb {outro}? use listar | compartilhar | parar | remotos | usar | soltar | portas | servir"
        )),
    }
}

/// `usb servir`: o servidor em primeiro plano. Com `--rede`, igual ao que
/// sobe junto do `p2p ligar`. Sem ela (modo servidor, OpenVPN), a rede e
/// dita a mao: `--ip` virtual, `--interface`, `--permitir CIDR` e `--busid`
/// (repetiveis) -- e a interface e OBRIGATORIA, pela mesma razao da trava.
fn usb_servir(o: &Opcoes, sysfs: crate::usb::Sysfs) -> R<String> {
    use std::sync::Arc;
    let log = |m: &str| eprintln!("phxvpn usb: {m}");
    if o.um("rede").is_some() || o.um("arquivo").is_some() {
        let h = crate::usb::servir_rede(
            arquivo_da_rede_sem_posicional(o)?,
            o.um("interface").map(str::to_string),
            || false,
            log,
        )?;
        let _ = h.join();
        return Ok(String::new());
    }
    let ip: std::net::Ipv4Addr = o
        .um("ip")
        .ok_or("informe --rede, ou --ip --interface --permitir --busid")?
        .parse()
        .map_err(|_| "--ip invalido")?;
    let interface = o
        .um("interface")
        .ok_or("--interface e obrigatoria: sem ela a LAN alcancaria o USB")?;
    let redes = o
        .todos("permitir")
        .iter()
        .map(|c| cidr(c))
        .collect::<R<Vec<_>>>()?;
    if redes.is_empty() {
        return Err("--permitir CIDR (ex.: 10.8.0.0/24) e obrigatorio".into());
    }
    let busids: Vec<String> = o.todos("busid").iter().map(|b| b.to_string()).collect();
    for b in &busids {
        crate::usb::validar_busid(b)?;
    }
    let ouvinte = crate::usb::escutar(ip, Some(interface))?;
    let srv = crate::usb::Servidor {
        sysfs,
        permitido: Arc::new(move |de| {
            de != ip && redes.iter().any(|(r, m)| u32::from(de) & m == *r)
        }),
        compartilhado: Arc::new(move |b| busids.iter().any(|x| x == b)),
    };
    log(&format!(
        "escutando {ip}:{} em {interface}",
        crate::usb::PORTA
    ));
    srv.servir(ouvinte, || false, log);
    Ok(String::new())
}

/// `10.8.0.0/24` -> (rede, mascara).
fn cidr(t: &str) -> R<(u32, u32)> {
    let (ip, p) = t
        .split_once('/')
        .ok_or_else(|| format!("CIDR invalido: {t}"))?;
    let ip: std::net::Ipv4Addr = ip.parse().map_err(|_| format!("CIDR invalido: {t}"))?;
    let p: u32 = p
        .parse()
        .ok()
        .filter(|p| *p <= 32)
        .ok_or_else(|| format!("CIDR invalido: {t}"))?;
    let m = if p == 0 { 0 } else { u32::MAX << (32 - p) };
    Ok((u32::from(ip) & m, m))
}

/// Como `arquivo_da_rede`, mas a rede NAO vem do posicional (que no `usb`
/// e a acao e o busid).
fn arquivo_da_rede_sem_posicional(o: &Opcoes) -> R<String> {
    match (o.um("arquivo"), o.um("rede")) {
        (Some(a), _) => Ok(a.to_string()),
        (None, Some(r)) => Ok(crate::rede_p2p::Rede::caminho(r)),
        _ => Err("informe a rede (--rede NOME)".into()),
    }
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
