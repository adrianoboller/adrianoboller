//! phxvpn -- redes virtuais no estilo Radmin, sobre OpenVPN.
//!
//! Duas portas para o mesmo motor (`painel.rs`): a tela web servida pelo
//! `phxvpn painel` e a linha de comando abaixo, que fala com o painel pela API.

use phxvpn::comandos::{self, Opcoes};
use phxvpn::{http, painel, pg, supervisor};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;

const AJUDA: &str = "phxvpn -- redes virtuais no estilo Radmin, sobre OpenVPN

  phxvpn painel [--escutar 127.0.0.1:8470] [--dados DIR] [--openvpn]
                [--nome painel.empresa.com.br:443 ...] [--aceito-sem-tls]
      PostgreSQL por PHXVPN_PG=\"host=.. port=.. user=.. password=.. dbname=..\".
      --nome: nome pelo qual o painel e chamado (proxy na frente); o painel
      recusa Host desconhecido (DNS rebinding). Nao instalado, mostra no
      terminal o CODIGO DE INSTALACAO que a tela pede.
      Sobe o painel (tela em http://<escuta>/). Com PHXVPN_SENHA_MESTRE no
      ambiente o cofre ja abre destrancado.
      --openvpn sobe um processo openvpn por rede.

  phxvpn criar-rede --painel http://host:8470 --usuario LOGIN --rede NOME [--finalidade TEXTO]
  phxvpn entrar     --painel http://host:8470 --usuario LOGIN --rede NOME [--saida ARQ.ovpn] [--conectar]
      Senhas por PHXVPN_SENHA e PHXVPN_SENHA_REDE, ou perguntadas no terminal --
      nunca por argumento, que aparece na lista de processos.
      --conectar chama o openvpn com o perfil baixado.

  phxvpn p2p chave [--arquivo p2p.chave]
      Cria (se nao existir) a identidade P2P deste computador e mostra a chave
      publica, que os outros membros usam no --par.

  phxvpn p2p criar --rede NOME [--ip 10.78.0.1/24] [--porta 51820] [--modo auto]
                   [--repasse CHAVE@HOST:PORTA]
      Cria a rede P2P neste computador (arquivo NOME.p2p, sem a senha).
  phxvpn p2p convidar --rede NOME [--endereco MEU_HOST:PORTA] [--validade 24h]
      Gera o codigo do convite, cifrado com a senha da rede, de uso unico.
  phxvpn p2p placa [--interface phxvpn]
      Windows: cria o adaptador TAP do phxvpn pelo tapctl.exe do OpenVPN
      (uma vez, como administrador; exige OpenVPN 2.6+ com TAP-Windows6).
  phxvpn p2p entrar <codigo>
      Aceita o convite (pede a senha da rede) e grava a rede aqui.
      Depois: phxvpn p2p ligar --rede NOME.

  phxvpn p2p ligar --rede NOME --ip 10.78.0.1/24 [--porta 51820] [--chave p2p.chave]
                   [--interface phx0] --par CHAVE@IP[@HOST:PORTA] [--par ...]
                   [--modo direto|repasse|auto] [--repasse CHAVE@HOST:PORTA]
      Modo P2P: liga a placa virtual e fala com os pares (Linux, como root).
      direto  -- so caminho direto, sem servidor nenhum (padrao);
      repasse -- tudo pelo servidor intermediario (CGNAT dos dois lados);
      auto    -- tenta direto e, sem resposta, vai pelo intermediario.
      O intermediario so carrega pacote cifrado de ponta a ponta.
      Senha da rede por PHXVPN_SENHA_REDE ou no terminal.

  phxvpn repasse [--porta 51821] [--chave repasse.chave] [--permitir ARQUIVO]
                 [--contas repasse-contas.txt]
      Servidor intermediario do P2P. Mostra a chave publica para o --repasse.
      --permitir: arquivo com uma chave publica por linha (repasse fechado).
      --contas: so registra quem prova usuario e senha (recomendado).
  phxvpn repasse conta --usuario U [--contas repasse-contas.txt]
      Inclui ou troca a senha de um usuario do repasse (PHXVPN_SENHA_REPASSE
      ou pergunta). O arquivo guarda so a credencial derivada.
      No no: p2p criar/ligar com --repasse-usuario U (senha pedida ao ligar).

  phxvpn usb listar
  phxvpn usb compartilhar <busid> --rede NOME   (Linux, root)
  phxvpn usb parar <busid> --rede NOME
      Oferece um dispositivo USB aos membros da rede (USB/IP, porta 3240,
      so no IP virtual e so para os pares). Sobe junto com o p2p ligar.
  phxvpn usb remotos <IP virtual>
  phxvpn usb usar <IP virtual> <busid>          (Linux: root + vhci-hcd;
  phxvpn usb soltar <porta>                      Windows: usbip-win2)
  phxvpn usb portas
      Usa aqui o dispositivo de um membro, como se estivesse espetado.

  phxvpn servico instalar painel [--openvpn ...]     (PHXVPN_PG no ambiente)
  phxvpn servico instalar repasse [--contas ARQ ...]
  phxvpn servico instalar p2p --rede NOME             (PHXVPN_SENHA_REDE)
        [--mostrar] so imprime a unidade; [--sem-iniciar] liga no arranque
  phxvpn servico remover phxvpn-painel
      Linux/systemd. Os segredos viram credenciais CIFRADAS (systemd-creds),
      nunca texto puro em arquivo. Dados em /var/lib/phxvpn.

  phxvpn cmd  (ou phxvpncmd) [/MODO:painel|p2p|ferramentas] [/PAINEL:http://..]
             [/COMANDO:\"linha\"] [/ENTRADA:script.txt]
      Console no estilo do prompt do MS-DOS, com tres modos. AJUDA dentro dele.

  phxvpn mesa [--pasta DIR] [--porta N] [--sem-janela]
      O programa de mesa (estilo Radmin): abre a janela com as redes P2P,
      os membros e o botao ligar/desligar. Ligar pede root/administrador.
      (No Windows tambem ha o phxvpnw.exe, que abre sem janela de console.)

  phxvpn versao
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = despachar(&args) {
        eprintln!("phxvpn: {e}");
        std::process::exit(1);
    }
}

/// Um comando do phxvpn. Separado do `main` para o servico do Windows rodar
/// o MESMO caminho de dentro do gerenciador de servicos.
fn despachar(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("painel") => cmd_painel(&args[1..]),
        Some("criar-rede") => cmd_rede(&args[1..], true),
        Some("entrar") => cmd_rede(&args[1..], false),
        Some("p2p") => cmd_p2p(&args[1..]),
        Some("repasse") => cmd_repasse(&args[1..]),
        Some("cmd") => phxvpn::console::principal(&args[1..]),
        // Chamado pelo `openvpn` (tls-crypt-v2-verify), nao por gente: codigo
        // de saida 0 aceita, 1 recusa.
        Some("ovpn-v2-verificar") => {
            let dir = PathBuf::from(args.get(1).map(String::as_str).unwrap_or(""));
            let tipo = std::env::var("metadata_type").unwrap_or_default();
            let meta = std::env::var("metadata_file")
                .ok()
                .and_then(|f| std::fs::read(f).ok())
                .unwrap_or_default();
            std::process::exit(if phxvpn::ovpn::verificar_v2(&dir, &tipo, &meta) {
                0
            } else {
                1
            })
        }
        Some("servico") => cmd_servico(&args[1..]),
        // Chamado pelo gerenciador de servicos do Windows, nao por gente.
        #[cfg(windows)]
        Some("servico-rodar") => {
            let unidade = args.get(1).ok_or("falta a unidade")?.clone();
            let resto: Vec<String> = args[2..].to_vec();
            // E por ela que `servico::credencial` acha o selo da unidade.
            std::env::set_var("PHXVPN_UNIDADE", &unidade);
            phxvpn::servico_windows::rodar(&unidade, Box::new(move || despachar(&resto)))
        }
        Some("usb") => comandos::usb(&Opcoes::de_args(&args[1..], &[])).map(|t| print!("{t}")),
        Some("mesa") => {
            let o = Opcoes::de_args(&args[1..], &["sem-janela"]);
            phxvpn::mesa::principal(
                o.um("pasta")
                    .map(PathBuf::from)
                    .unwrap_or_else(phxvpn::mesa::pasta_padrao),
                o.um("porta").and_then(|p| p.parse().ok()).unwrap_or(0),
                !o.tem("sem-janela"),
            )
        }
        Some("versao") | Some("--version") => {
            println!("phxvpn {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => {
            print!("{AJUDA}");
            Ok(())
        }
    }
}

fn opcao(args: &[String], nome: &str) -> Option<String> {
    args.iter()
        .position(|a| a == nome)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn cmd_painel(args: &[String]) -> Result<(), String> {
    let o = Opcoes::de_args(args, &["openvpn", "aceito-sem-tls"]);
    let escuta = o.um("escutar").unwrap_or("127.0.0.1:8470").to_string();
    let local = escuta.starts_with("127.")
        || escuta.starts_with("localhost")
        || escuta.starts_with("[::1]");
    // Fora do loopback so com a escolha escrita: senha e chave privada do
    // membro passam em claro sem TLS (achado A4).
    if !local && !o.tem("aceito-sem-tls") {
        return Err(format!(
            "o painel fala HTTP sem TLS; escutar em {escuta} exporia senha e chave privada. \
             Ponha um proxy com TLS na frente e escute em 127.0.0.1, ou passe \
             --aceito-sem-tls sabendo disso"
        ));
    }
    let pg_texto = match o.um("pg") {
        Some(t) => {
            if t.contains("password=") {
                eprintln!(
                    "phxvpn: AVISO -- a senha do PostgreSQL no --pg aparece no `ps`; prefira PHXVPN_PG"
                );
            }
            t.to_string()
        }
        // Como servico, a credencial cifrada do systemd; em primeiro plano,
        // a variavel de ambiente.
        None => phxvpn::servico::segredo("pg", "PHXVPN_PG")
            .ok_or("informe o PostgreSQL por PHXVPN_PG (ou --pg)")?,
    };
    std::env::remove_var("PHXVPN_PG");
    let dados = PathBuf::from(o.um("dados").unwrap_or("phxvpn-dados"));
    let cfg = pg::Config::de_texto(&pg_texto)?;
    let mut p = painel::Painel::abrir(&cfg, &dados)?;
    if let Some(m) = phxvpn::servico::segredo("mestre", "PHXVPN_SENHA_MESTRE") {
        // Lida, sai do ambiente: nao fica em /proc/<pid>/environ (M4).
        std::env::remove_var("PHXVPN_SENHA_MESTRE");
        p.destrancar(&m)?;
        eprintln!("phxvpn: cofre destrancado pela PHXVPN_SENHA_MESTRE");
    }
    let supervisor = if o.tem("openvpn") {
        Some(supervisor::Supervisor::novo()?)
    } else {
        None
    };
    p.aquecer();
    let (destrancado, instalado) = (p.destrancado(), p.instalado()?);
    let nomes: Vec<String> = o.todos("nome").iter().map(|n| n.to_lowercase()).collect();
    let estado = Arc::new(http::Estado::novo(p, supervisor).com_hosts(&escuta, &nomes));
    if destrancado {
        http::materializar_e_subir(&estado)?;
    }
    if !instalado {
        eprintln!(
            "phxvpn: CODIGO DE INSTALACAO: {} (a tela de instalacao pede este codigo)",
            estado.gerar_codigo_instalacao()
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
    let o = Opcoes::de_args(args, &["conectar"]);
    let painel = o.um("painel").unwrap_or("http://127.0.0.1:8470");
    let login = o.um("usuario").ok_or("informe --usuario")?;
    let rede = o.um("rede").ok_or("informe --rede")?;
    let s_login = senha("PHXVPN_SENHA", "senha do usuario")?;
    let s_rede = senha("PHXVPN_SENHA_REDE", "senha da rede")?;
    let (token, _) = comandos::login(painel, login, &s_login)?;
    let arquivo = comandos::perfil_de_rede(
        painel,
        &token,
        criar,
        rede,
        &s_rede,
        o.um("finalidade").unwrap_or(""),
        o.um("saida"),
    )?;
    println!("perfil gravado em {arquivo} (contem a sua chave privada: guarde-o como senha)");
    if o.tem("conectar") {
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

fn cmd_p2p(args: &[String]) -> Result<(), String> {
    let o = Opcoes::de_args(&args[args.len().min(1)..], &[]);
    let arquivo = o.um("chave").or(o.um("arquivo")).unwrap_or("p2p.chave");
    match args.first().map(String::as_str) {
        Some("chave") => {
            println!(
                "{}",
                comandos::chave_publica_hex(&comandos::identidade(arquivo)?)
            );
            Ok(())
        }
        Some("ligar") => p2p_ligar(&o),
        Some("placa") => p2p_placa(&o),
        Some("criar") => {
            println!("{}", comandos::p2p_criar(&o)?);
            Ok(())
        }
        Some("convidar") => {
            let senha_rede = senha("PHXVPN_SENHA_REDE", "senha da rede")?;
            println!("{}", comandos::p2p_convidar(&o, &senha_rede)?);
            Ok(())
        }
        Some("entrar") => {
            let codigo = o
                .posicionais
                .first()
                .ok_or("phxvpn p2p entrar <codigo do convite>")?;
            let senha_rede = senha("PHXVPN_SENHA_REDE", "senha da rede")?;
            println!("{}", comandos::p2p_entrar(codigo, &senha_rede, &o)?);
            Ok(())
        }
        _ => Err("use: phxvpn p2p chave | phxvpn p2p ligar ... (veja phxvpn ajuda)".into()),
    }
}

#[cfg(any(target_os = "linux", windows))]
fn p2p_ligar(o: &Opcoes) -> Result<(), String> {
    // Como servico: a PSK ja derivada, entregue cifrada pelo systemd.
    if let Some(psk) = phxvpn::servico::credencial("psk") {
        let psk: [u8; 32] = phxsql_core::hash::de_hex(&psk)
            .and_then(|b| b.try_into().ok())
            .ok_or("credencial psk torta")?;
        let (no, tun, resumo) = comandos::p2p_preparar(o, comandos::Segredo::Psk(psk), None)?;
        eprintln!("phxvpn: {resumo}");
        return phxvpn::p2p::rodar(no, tun);
    }
    let senha_rede = senha("PHXVPN_SENHA_REDE", "senha da rede")?;
    // A senha sai do ambiente assim que foi lida: nao fica em /proc/<pid>/environ.
    std::env::remove_var("PHXVPN_SENHA_REDE");
    let senha_repasse = match comandos::usuario_do_repasse(o) {
        Some(u) => Some(senha(
            "PHXVPN_SENHA_REPASSE",
            &format!("senha de {u} no servidor intermediario"),
        )?),
        None => None,
    };
    std::env::remove_var("PHXVPN_SENHA_REPASSE");
    let (no, tun, resumo) = comandos::p2p_preparar(
        o,
        comandos::Segredo::Senha(&senha_rede),
        senha_repasse
            .as_deref()
            .map(comandos::SegredoRepasse::Senha),
    )?;
    eprintln!("phxvpn: {resumo}");
    phxvpn::p2p::rodar(no, tun)
}

#[cfg(not(any(target_os = "linux", windows)))]
fn p2p_ligar(_o: &Opcoes) -> Result<(), String> {
    Err("o modo P2P ainda so roda no Linux (Windows: TAP-Windows6, em escrita)".into())
}

/// `phxvpn servico instalar painel|repasse|p2p [opcoes do servico]
/// [--mostrar] [--sem-iniciar]` e `phxvpn servico remover <unidade>`.
fn cmd_servico(args: &[String]) -> Result<(), String> {
    use phxvpn::servico::{self, Tipo};
    match args.first().map(String::as_str) {
        Some("instalar") => {
            let tipo = Tipo::de_texto(args.get(1).map(String::as_str).unwrap_or(""))?;
            let resto: Vec<String> = args[2..]
                .iter()
                .filter(|a| *a != "--mostrar" && *a != "--sem-iniciar")
                .cloned()
                .collect();
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let plano = servico::planejar(tipo, &resto, &exe)?;
            if args.iter().any(|a| a == "--mostrar") {
                // So a unidade: os segredos nunca saem na tela.
                print!("# {}\n{}", plano.arquivo.display(), plano.texto);
                for (n, _) in &plano.credenciais {
                    println!(
                        "# credencial cifrada: {}/{}-{n}.cred",
                        servico::PASTA_CRED,
                        plano.unidade
                    );
                }
                return Ok(());
            }
            let m = servico::instalar(&plano, !args.iter().any(|a| a == "--sem-iniciar"))?;
            println!("{m}");
            Ok(())
        }
        Some("remover") => {
            let u = args
                .get(1)
                .ok_or("informe a unidade (ex.: phxvpn-painel)")?;
            println!("{}", servico::remover(u)?);
            Ok(())
        }
        _ => Err(
            "use: phxvpn servico instalar painel|repasse|p2p ... | phxvpn servico remover UNIDADE"
                .into(),
        ),
    }
}

fn cmd_repasse(args: &[String]) -> Result<(), String> {
    use phxsql_core::hash::{de_hex, para_hex};
    use phxvpn::repasse::Repasse;
    if args.first().map(String::as_str) == Some("conta") {
        let arquivo = opcao(args, "--contas").unwrap_or_else(|| "repasse-contas.txt".into());
        let usuario = opcao(args, "--usuario").ok_or("informe --usuario")?;
        let s = senha("PHXVPN_SENHA_REPASSE", &format!("senha nova de {usuario}"))?;
        phxvpn::repasse::gravar_conta(&arquivo, &usuario, &s)?;
        println!("conta {usuario} gravada em {arquivo} (so a credencial derivada, nunca a senha)");
        return Ok(());
    }
    let privada =
        comandos::identidade(&opcao(args, "--chave").unwrap_or_else(|| "repasse.chave".into()))?;
    let permitidas = match opcao(args, "--permitir") {
        Some(arq) => {
            let texto = std::fs::read_to_string(&arq).map_err(|e| format!("{arq}: {e}"))?;
            let mut l = std::collections::HashSet::new();
            for (n, linha) in texto.lines().enumerate() {
                let linha = linha.trim();
                if linha.is_empty() || linha.starts_with('#') {
                    continue;
                }
                let k = de_hex(linha)
                    .and_then(|b| <[u8; 32]>::try_from(b).ok())
                    .ok_or_else(|| format!("{arq}:{}: chave invalida", n + 1))?;
                l.insert(k);
            }
            Some(l)
        }
        None => None,
    };
    let porta = opcao(args, "--porta").unwrap_or_else(|| "51821".into());
    let udp = std::net::UdpSocket::bind(format!("0.0.0.0:{porta}"))
        .map_err(|e| format!("porta UDP {porta}: {e}"))?;
    let mut r = Repasse::novo(privada, permitidas);
    match opcao(args, "--contas") {
        Some(arq) => {
            let contas = phxvpn::repasse::ler_contas(&arq)?;
            eprintln!(
                "phxvpn: repasse com CONTAS -- {} usuario(s) de {arq}",
                contas.len()
            );
            r = r.com_contas(contas);
        }
        None => eprintln!(
            "phxvpn: AVISO -- repasse ABERTO: qualquer chave o usa. Para exigir usuario e senha: \
             phxvpn repasse conta --usuario U, e depois --contas repasse-contas.txt"
        ),
    }
    eprintln!(
        "phxvpn: repasse no ar -- UDP {porta}, chave {}",
        para_hex(&r.publica())
    );
    let mut buf = vec![0u8; 65_535];
    loop {
        let (n, de) = udp.recv_from(&mut buf).map_err(|e| e.to_string())?;
        if let Some((alvo, p)) = r.tratar(&buf[..n], de) {
            let _ = udp.send_to(&p, alvo);
        }
    }
}

/// `p2p placa`: no Windows, cria o adaptador TAP do phxvpn pelo `tapctl.exe`
/// do OpenVPN (uma vez, como administrador). No Linux nao ha o que criar: a
/// placa nasce no `ligar`.
fn p2p_placa(o: &Opcoes) -> Result<(), String> {
    #[cfg(windows)]
    {
        println!(
            "{}",
            phxvpn::tun::criar_placa(o.um("interface").unwrap_or("phxvpn"))?
        );
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = o;
        println!("no Linux a placa nasce sozinha no p2p ligar; nada a criar");
        Ok(())
    }
}
