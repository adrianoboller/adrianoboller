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
                    [--protocolo udp|tcp] [--porta 443] [--http-proxy HOST:PORTA]
  phxvpn entrar     --painel http://host:8470 --usuario LOGIN --rede NOME [--saida ARQ.ovpn] [--conectar]
                    [--http-proxy HOST:PORTA]
      --protocolo tcp: a rede OpenVPN escuta em TCP (proto tcp-server) -- para
      membros atras de rede que so deixa TCP/443; --porta so o administrador.
      --http-proxy: o perfil sai com http-proxy (so em rede TCP).
      Senhas por PHXVPN_SENHA e PHXVPN_SENHA_REDE, ou perguntadas no terminal --
      nunca por argumento, que aparece na lista de processos. Quem cadastrou o
      autenticador passa o codigo em PHXVPN_CODIGO.
      --conectar chama o openvpn com o perfil baixado.

  phxvpn p2p chave [--arquivo p2p.chave]
      Cria (se nao existir) a identidade P2P deste computador e mostra a chave
      publica, que os outros membros usam no --par.

  phxvpn p2p criar --rede NOME [--ip 10.78.0.1/24] [--porta 51820] [--modo auto]
                   [--repasse CHAVE@HOST:PORTA]
      Cria a rede P2P neste computador (arquivo NOME.p2p, sem a senha), com
      rol de membros assinado por este computador. [--apelido NOME]
      [--sem-descoberta]: nao anunciar nem procurar membros na LAN.
  phxvpn p2p convidar --rede NOME [--endereco MEU_HOST:PORTA] [--validade 24h]
      Gera o codigo do convite, cifrado com a senha da rede, de uso unico.
      Numa rede criada com rol assinado, so quem a criou convida.
  phxvpn p2p placa [--interface phxvpn]
      Windows: cria o adaptador TAP do phxvpn pelo tapctl.exe do OpenVPN
      (uma vez, como administrador; exige OpenVPN 2.6+ com TAP-Windows6).
  phxvpn p2p entrar <codigo> [--apelido NOME]
      Aceita o convite (pede a senha da rede) e grava a rede aqui.
      Depois: phxvpn p2p ligar --rede NOME.
  phxvpn p2p remover --rede NOME --ip IP_VIRTUAL
      So quem criou a rede: assina um rol de membros novo, sem esse membro.
      Com a rede ligada, os outros o recebem pela malha e param de aceita-lo.

  phxvpn p2p ligar --rede NOME --ip 10.78.0.1/24 [--porta 51820] [--chave p2p.chave]
                   [--interface phx0] --par CHAVE@IP[@HOST:PORTA] [--par ...]
                   [--modo direto|repasse|auto] [--repasse CHAVE@HOST:PORTA]
                   [--fio auto|udp|tcp] [--tcp] [--repasse-tcp 443]
                   [--proxy HOST:PORTA [--proxy-usuario U]]
      Modo P2P: liga a placa virtual e fala com os pares (Linux, como root).
      direto  -- so caminho direto, sem servidor nenhum (padrao);
      repasse -- tudo pelo servidor intermediario (CGNAT dos dois lados);
      auto    -- tenta direto e, sem resposta, vai pelo intermediario; ja pelo
                 intermediario, pede a ele o endereco publico do par e perfura
                 o NAT para migrar ao caminho direto (--sem-perfuracao desliga).
      O intermediario so carrega pacote cifrado de ponta a ponta.
      Fio ate o intermediario: auto (padrao: UDP e, sem confirmacao em 10 s,
      TCP na porta --repasse-tcp), udp, ou tcp (--tcp). --proxy passa pelo
      CONNECT de um proxy HTTP (implica tcp); a senha do proxy vem de
      PHXVPN_SENHA_PROXY ou do terminal. HTTPS_PROXY NAO e lido.
      Senha da rede por PHXVPN_SENHA_REDE ou no terminal.

  phxvpn repasse [--porta 51821] [--tcp 443] [--chave repasse.chave] [--permitir ARQUIVO]
                 [--contas repasse-contas.txt]
      Servidor intermediario do P2P. Mostra a chave publica para o --repasse.
      --tcp: escuta tambem em TCP (redes que so deixam sair TCP/443 ou proxy).
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
  phxvpn servico instalar cliente --perfil rede.ovpn
      Modo cliente (entrar na rede de outro servidor) antes do login --
      o gap do connect-before-logon do OpenVPN Connect: baixe o perfil
      uma vez com `phxvpn entrar --saida rede.ovpn`, depois instale o
      servico; ele sobe o `openvpn --config rede.ovpn` com a maquina,
      sem ninguem logado (LocalSystem no Windows, root no Linux).
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

  phxvpn atualizar [--manifesto http://host/manifesto.json] [--verificar]
      PHXVPN_ATUALIZAR_MANIFESTO vale como --manifesto. --verificar so diz se
      ha versao mais nova, sem baixar nem trocar nada. Sem --verificar, baixa,
      confere assinatura Ed25519 e SHA-256, recusa versao igual ou menor, e
      troca o binario EM USO (atomico no Linux; no Windows, o antigo vira
      .old e e limpo no proximo arranque).
      Checagem periodica opcional: com PHXVPN_ATUALIZAR_MANIFESTO no
      ambiente, `phxvpn painel` e `phxvpn mesa` avisam sozinhos quando ha
      versao nova (nunca aplicam sozinhos). PHXVPN_ATUALIZAR_INTERVALO_S
      muda o intervalo (padrao 21600 = 6h).
  phxvpn atualizar-assinar --versao X.Y.Z --saida manifesto.json
      --chave-privada ARQUIVO (ou PHXVPN_CHAVE_PRIVADA_ATUALIZACAO)
      --alvo PLATAFORMA=URL,SHA256 [--alvo ...]
      Gera e assina o manifesto (uso do publicar-atualizacao.sh).
  phxvpn atualizar-gerar-chave
      Gera um par Ed25519 novo para publicar atualizacoes (uso unico, por
      quem for lancar de verdade -- a privada nunca vai para o repositorio).

  phxvpn versao
";

fn main() {
    // Sobra de uma troca de binario anterior (so existe passo intermediario
    // no Windows -- o Linux troca atomico, sem sobra).
    phxvpn::atualizar::limpar_binario_antigo();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = despachar(&args) {
        eprintln!("phxvpn: {e}");
        std::process::exit(1);
    }
}

/// Checagem periodica de atualizacao: so nasce se `PHXVPN_ATUALIZAR_MANIFESTO`
/// estiver no ambiente -- guarda pedida, nao imposta, e sem custo nenhum em
/// quem nunca a liga.
fn talvez_checar_atualizacao() {
    let Ok(url) = std::env::var("PHXVPN_ATUALIZAR_MANIFESTO") else {
        return;
    };
    let Some(chave) = phxsql_core::ed25519::chave_de_hex(phxvpn::atualizar::CHAVE_PUBLICA_PADRAO)
    else {
        return;
    };
    let intervalo = std::env::var("PHXVPN_ATUALIZAR_INTERVALO_S")
        .ok()
        .and_then(|s| s.parse().ok())
        .map(std::time::Duration::from_secs)
        .unwrap_or(std::time::Duration::from_secs(6 * 3600));
    phxvpn::atualizar::iniciar_verificacao_periodica(
        url,
        chave,
        env!("CARGO_PKG_VERSION").to_string(),
        intervalo,
    );
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
        // Tambem do `openvpn` (auth-user-pass-verify): 0 aceita, 1 recusa, 2 adiado.
        Some("ovpn-mfa-verificar") => std::process::exit(phxvpn::verificar::principal(&args[1..])),
        Some("ovpn-mfa-adiado") => std::process::exit(phxvpn::verificar::adiado(&args[1..])),
        Some("servico") => cmd_servico(&args[1..]),
        // Chamado pelo servico "cliente", nao por gente.
        Some("cliente-rodar") => cmd_cliente_rodar(&args[1..]),
        Some("atualizar") => cmd_atualizar(&args[1..]),
        Some("atualizar-assinar") => cmd_atualizar_assinar(&args[1..]),
        Some("atualizar-gerar-chave") => cmd_atualizar_gerar_chave(),
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
            talvez_checar_atualizacao();
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
    talvez_checar_atualizacao();
    let (destrancado, instalado) = (p.destrancado(), p.instalado()?);
    let nomes: Vec<String> = o.todos("nome").iter().map(|n| n.to_lowercase()).collect();
    let estado = Arc::new(http::Estado::novo(p, supervisor).com_hosts(&escuta, &nomes));
    // Antes de subir o OpenVPN: o usuario proprio dele (em primeiro plano,
    // como root; como servico ele nasceu no `servico instalar`) e o soquete
    // onde rede que exige o autenticador pergunta. Falhar em criar o usuario
    // nao para o painel: o `servir` avisa que caiu para nobody.
    #[cfg(unix)]
    {
        if let Err(e) = phxvpn::ovpn::garantir_usuario_dedicado() {
            eprintln!("phxvpn: AVISO {e}");
        }
        phxvpn::verificar::servir(estado.clone())?;
    }
    // Reconcilia banco, ccd/ e conexoes a cada `credencial::VIGIA`.
    phxvpn::credencial::vigiar(estado.clone());
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
    let arquivo = comandos::perfil_de_rede(painel, &token, criar, rede, &s_rede, &o)?;
    println!("perfil gravado em {arquivo} (contem a sua chave privada: guarde-o como senha)");
    if o.tem("conectar") {
        rodar_openvpn_cliente(&arquivo)?;
    }
    Ok(())
}

/// Sobe o `openvpn` cliente com um perfil `.ovpn` ja emitido e espera ele
/// terminar. UM motor so: `entrar --conectar` (em primeiro plano) e
/// `cliente-rodar` (dentro do servico "connect before logon") chamam esta
/// mesma funcao -- nao duas copias que um conserto no processo filho
/// esqueceria de repetir na outra.
fn rodar_openvpn_cliente(perfil: &str) -> Result<(), String> {
    let bin = supervisor::achar_no_path("openvpn").ok_or("openvpn nao esta no PATH")?;
    let st = std::process::Command::new(bin)
        .arg("--config")
        .arg(perfil)
        .status()
        .map_err(|e| e.to_string())?;
    if !st.success() {
        return Err(format!("openvpn saiu com {st}"));
    }
    Ok(())
}

/// `phxvpn cliente-rodar --perfil ARQ`: o que o servico "cliente" executa.
/// Nao e chamado por gente -- e o `ExecStart` de
/// `phxvpn servico instalar cliente --perfil ...`.
fn cmd_cliente_rodar(args: &[String]) -> Result<(), String> {
    let o = Opcoes::de_args(args, &[]);
    let perfil = o.um("perfil").ok_or("informe --perfil ARQUIVO.ovpn")?;
    rodar_openvpn_cliente(perfil)
}

fn cmd_p2p(args: &[String]) -> Result<(), String> {
    let o = Opcoes::de_args(
        &args[args.len().min(1)..],
        &["sem-perfuracao", "sem-descoberta", "tcp"],
    );
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
        Some("remover") => {
            println!("{}", comandos::p2p_remover(&o)?);
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
        let (no, tun, resumo) = comandos::p2p_preparar(o, comandos::Segredo::Psk(psk), None, None)?;
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
    // Le uma vez so, aqui no comeco -- dali pra baixo (fio_do_no) a senha
    // viaja por PARAMETRO, nunca pelo ambiente do processo: a mesa e um
    // processo longo, de varias threads, e `set_var`/`getenv` concorrentes
    // sao indefinidos na glibc (por isso `unsafe` na edicao 2024).
    let senha_proxy = match comandos::usuario_do_proxy(o) {
        Some(u) => Some(senha(
            "PHXVPN_SENHA_PROXY",
            &format!("senha de {u} no proxy"),
        )?),
        None => None,
    };
    std::env::remove_var("PHXVPN_SENHA_PROXY");
    let (no, tun, resumo) = comandos::p2p_preparar(
        o,
        comandos::Segredo::Senha(&senha_rede),
        senha_repasse
            .as_deref()
            .map(comandos::SegredoRepasse::Senha),
        senha_proxy,
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
    // TCP so quando pedido: e mais uma porta exposta, e 443 pede root.
    let tcp = match opcao(args, "--tcp") {
        Some(p) => Some((
            std::net::TcpListener::bind(format!("0.0.0.0:{p}"))
                .map_err(|e| format!("porta TCP {p}: {e}"))?,
            p,
        )),
        None => None,
    };
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
        "phxvpn: repasse no ar -- UDP {porta}{}, chave {}",
        tcp.as_ref()
            .map(|(_, p)| format!(" e TCP {p}"))
            .unwrap_or_default(),
        para_hex(&r.publica())
    );
    let central = phxvpn::repasse_tcp::Central::nova(r, udp);
    if let Some((ouvinte, _)) = tcp {
        let c = std::sync::Arc::clone(&central);
        std::thread::spawn(move || c.servir_tcp(ouvinte));
    }
    central.servir_udp()
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

fn chave_publica_de_atualizacao() -> [u8; 32] {
    phxsql_core::ed25519::chave_de_hex(phxvpn::atualizar::CHAVE_PUBLICA_PADRAO)
        .expect("CHAVE_PUBLICA_PADRAO e uma constante do proprio binario, sempre valida")
}

fn cmd_atualizar(args: &[String]) -> Result<(), String> {
    let o = Opcoes::de_args(args, &["verificar"]);
    let url = o
        .um("manifesto")
        .map(str::to_string)
        .or_else(|| std::env::var("PHXVPN_ATUALIZAR_MANIFESTO").ok())
        .ok_or("informe --manifesto http://host/manifesto.json ou PHXVPN_ATUALIZAR_MANIFESTO")?;
    let chave = chave_publica_de_atualizacao();
    let versao_atual = env!("CARGO_PKG_VERSION");
    if o.tem("verificar") {
        let d = phxvpn::atualizar::verificar(&url, &chave, versao_atual)?;
        if d.mais_novo {
            println!(
                "atualizacao disponivel: {} (atual: {versao_atual})",
                d.versao
            );
        } else {
            println!("ja esta na versao mais nova ({versao_atual})");
        }
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let versao = phxvpn::atualizar::aplicar(&url, &chave, versao_atual, &exe)?;
    println!("atualizado para {versao} -- reinicie o phxvpn");
    Ok(())
}

/// `phxvpn atualizar-assinar`: ferramenta de quem PUBLICA, chamada pelo
/// `publicar-atualizacao.sh`. Nao roda em quem so consome a atualizacao.
fn cmd_atualizar_assinar(args: &[String]) -> Result<(), String> {
    use phxvpn::atualizar::{assinar_manifesto, Alvo, Manifesto};
    let o = Opcoes::de_args(args, &[]);
    let versao = o.um("versao").ok_or("informe --versao MAJOR.MINOR.PATCH")?;
    let saida = o.um("saida").ok_or("informe --saida ARQUIVO")?;
    let privada_txt = match o.um("chave-privada") {
        Some(caminho) => std::fs::read_to_string(caminho).map_err(|e| format!("{caminho}: {e}"))?,
        None => std::env::var("PHXVPN_CHAVE_PRIVADA_ATUALIZACAO").map_err(|_| {
            "informe --chave-privada ARQUIVO ou PHXVPN_CHAVE_PRIVADA_ATUALIZACAO \
             (a chave privada nunca vai para o repositorio)"
        })?,
    };
    let privada = phxsql_core::ed25519::chave_de_hex(privada_txt.trim())
        .ok_or("a chave privada nao e hex valido de 32 bytes")?;
    let mut alvos = Vec::new();
    for a in o.todos("alvo") {
        let (plataforma, resto) = a
            .split_once('=')
            .ok_or_else(|| format!("--alvo {a:?}: use PLATAFORMA=URL,SHA256"))?;
        let (url, sha256) = resto
            .rsplit_once(',')
            .ok_or_else(|| format!("--alvo {a:?}: use PLATAFORMA=URL,SHA256"))?;
        alvos.push((
            plataforma.to_string(),
            Alvo {
                url: url.to_string(),
                sha256: sha256.trim().to_lowercase(),
            },
        ));
    }
    if alvos.is_empty() {
        return Err("informe ao menos um --alvo PLATAFORMA=URL,SHA256".into());
    }
    let m = Manifesto {
        versao: versao.to_string(),
        alvos,
    };
    let texto = assinar_manifesto(&m, &privada);
    std::fs::write(saida, texto).map_err(|e| format!("{saida}: {e}"))?;
    println!("manifesto assinado gravado em {saida}");
    Ok(())
}

fn cmd_atualizar_gerar_chave() -> Result<(), String> {
    use phxsql_core::hash::para_hex;
    let privada = phxsql_core::ed25519::gerar_privada();
    let publica = phxsql_core::ed25519::chave_publica(&privada);
    println!(
        "privada: {} (NUNCA grave isto no repositorio; guarde por fora, ou em \
         PHXVPN_CHAVE_PRIVADA_ATUALIZACAO na hora de publicar)",
        para_hex(&privada)
    );
    println!(
        "publica: {} (troque CHAVE_PUBLICA_PADRAO em src/atualizar.rs por esta antes do lancamento)",
        para_hex(&publica)
    );
    Ok(())
}
