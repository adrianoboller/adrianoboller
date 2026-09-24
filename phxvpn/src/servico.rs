//! O phxvpn como serviço do sistema (Linux, systemd): o painel, o servidor
//! intermediário e um nó P2P sempre ligado sobem com a máquina, sem ninguém
//! logado.
//!
//! # Segredo sem texto puro em arquivo
//!
//! Serviço não tem quem digite senha, e a pétrea proíbe senha em texto puro
//! em arquivo -- o `.env` de costume. O caminho é o do próprio systemd:
//! `systemd-creds encrypt` guarda o segredo CIFRADO (chave do host, ou TPM
//! quando há) em `/etc/phxvpn/*.cred`, e o systemd só o decifra, na memória,
//! para o processo, em `$CREDENTIALS_DIRECTORY` (`LoadCredentialEncrypted`).
//! O segredo entra no `systemd-creds` pela entrada padrão -- nunca pela linha
//! de comando, que aparece no `ps`.
//!
//! | Serviço | Roda como | Credenciais |
//! |---|---|---|
//! | painel  | root (o `openvpn` cria a placa e desce para `nobody`) | `pg`, e `mestre` se pedida |
//! | repasse | usuário dinâmico, sem root nenhum | -- |
//! | p2p     | root com só `CAP_NET_ADMIN` (placa virtual) | `psk` (a derivada, não a senha) |

use std::path::{Path, PathBuf};

pub type R<T> = Result<T, String>;

pub const PASTA_CRED: &str = "/etc/phxvpn";
pub const PASTA_DADOS: &str = "/var/lib/phxvpn";

/// Onde o servico guarda os dados: `/var/lib/phxvpn`, ou
/// `%ProgramData%\phxvpn` no Windows.
pub fn pasta_dados() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into()))
            .join("phxvpn")
    } else {
        PathBuf::from(PASTA_DADOS)
    }
}

/// Onde ficam as credenciais cifradas: `/etc/phxvpn` (systemd-creds), ou
/// `%ProgramData%\phxvpn\cred` (DPAPI da maquina, so SYSTEM e
/// Administradores leem).
pub fn pasta_cred() -> PathBuf {
    if cfg!(windows) {
        pasta_dados().join("cred")
    } else {
        PathBuf::from(PASTA_CRED)
    }
}

#[cfg(windows)]
const ENTROPIA_WIN: &[u8] = b"phxvpn-servico-v1";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tipo {
    Painel,
    Repasse,
    P2p,
    /// Modo cliente do OpenVPN (entrar numa rede de outro servidor). E o
    /// gap que o OpenVPN Connect cobre com "connect before logon": sem um
    /// servico, o tunel so sobe depois que ALGUEM loga e roda
    /// `phxvpn entrar --conectar` a mao -- igual falta no painel/repasse/p2p
    /// antes de existirem os servicos deles.
    Cliente,
}

impl Tipo {
    pub fn de_texto(t: &str) -> R<Tipo> {
        match t {
            "painel" => Ok(Tipo::Painel),
            "repasse" => Ok(Tipo::Repasse),
            "p2p" => Ok(Tipo::P2p),
            "cliente" => Ok(Tipo::Cliente),
            _ => Err(format!(
                "serviço {t:?}? use painel, repasse, p2p ou cliente"
            )),
        }
    }

    fn nome(self) -> &'static str {
        match self {
            Tipo::Painel => "painel",
            Tipo::Repasse => "repasse",
            Tipo::P2p => "p2p",
            Tipo::Cliente => "cliente",
        }
    }

    /// Nome da unidade: `phxvpn-painel`, `phxvpn-repasse`, `phxvpn-p2p-<rede>`,
    /// `phxvpn-cliente-<rede>`.
    pub fn unidade(self, rede: Option<&str>) -> String {
        match (self, rede) {
            (Tipo::P2p, Some(r)) => format!("phxvpn-p2p-{}", limpo(r)),
            (Tipo::Cliente, Some(r)) => format!("phxvpn-cliente-{}", limpo(r)),
            _ => format!("phxvpn-{}", self.nome()),
        }
    }
}

/// Devolve `args` sem a opcao `nome` e o valor dela. Usado quando o plano ja
/// recalculou essa opcao (ex.: caminho canonicalizado) e nao pode deixar a
/// versao crua do usuario voltar pela copia generica do resto dos args.
fn sem_opcao(args: &[String], nome: &str) -> Vec<String> {
    let mut saida = Vec::with_capacity(args.len());
    let mut pular = false;
    for a in args {
        if pular {
            pular = false;
            continue;
        }
        if a == nome {
            pular = true;
            continue;
        }
        saida.push(a.clone());
    }
    saida
}

fn limpo(t: &str) -> String {
    t.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Argumento para a linha `ExecStart`: o systemd separa por espaço e aceita
/// aspas; aspas e barra invertida dentro do argumento se escapam.
fn arg_systemd(a: &str) -> String {
    if !a.is_empty()
        && a.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=@,+".contains(c))
    {
        return a.to_string();
    }
    format!(
        "\"{}\"",
        a.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
    )
}

/// Argumento na linha de comando do Windows (regras do
/// `CommandLineToArgvW`): aspas quando ha espaco ou aspas; barras antes de
/// aspas se dobram.
fn arg_windows(a: &str) -> String {
    if !a.is_empty() && !a.contains([' ', '\t', '"']) {
        return a.to_string();
    }
    let mut s = String::from("\"");
    let mut barras = 0;
    for c in a.chars() {
        match c {
            '\\' => barras += 1,
            '"' => {
                s.push_str(&"\\".repeat(barras * 2 + 1));
                s.push('"');
                barras = 0;
            }
            _ => {
                s.push_str(&"\\".repeat(barras));
                s.push(c);
                barras = 0;
            }
        }
    }
    s.push_str(&"\\".repeat(barras * 2));
    s.push('"');
    s
}

/// O texto da unidade. `credenciais`: os nomes que vão em
/// `LoadCredentialEncrypted` (arquivo `<pasta>/<unidade>-<nome>.cred`).
pub fn texto_da_unidade(
    tipo: Tipo,
    unidade: &str,
    exe: &Path,
    args: &[String],
    credenciais: &[&str],
) -> String {
    let mut linha = arg_systemd(&exe.to_string_lossy());
    for a in args {
        linha.push(' ');
        linha.push_str(&arg_systemd(a));
    }
    let creds: String = credenciais
        .iter()
        .map(|c| format!("LoadCredentialEncrypted={c}:{PASTA_CRED}/{unidade}-{c}.cred\n"))
        .collect();
    // Endurecimento comum; o que muda por tipo vem abaixo.
    let comum = "NoNewPrivileges=yes\n\
                 ProtectSystem=strict\n\
                 ProtectHome=yes\n\
                 PrivateTmp=yes\n\
                 ProtectKernelModules=yes\n\
                 ProtectKernelLogs=yes\n\
                 ProtectControlGroups=yes\n\
                 ProtectClock=yes\n\
                 LockPersonality=yes\n\
                 MemoryDenyWriteExecute=yes\n\
                 RestrictRealtime=yes\n\
                 RestrictSUIDSGID=yes\n\
                 SystemCallArchitectures=native\n";
    let proprio = match tipo {
        // O openvpn filho cria a placa (NET_ADMIN), escuta porta (BIND) e
        // desce para nobody (SETUID/SETGID); o dono dos arquivos e root.
        Tipo::Painel => "User=root\n\
             CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_BIND_SERVICE CAP_SETUID CAP_SETGID CAP_DAC_OVERRIDE CAP_CHOWN CAP_FOWNER\n\
             DevicePolicy=closed\n\
             DeviceAllow=/dev/net/tun rw\n\
             RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX AF_NETLINK\n"
            .to_string(),
        // So UDP numa porta alta: nem root precisa.
        Tipo::Repasse => "DynamicUser=yes\n\
             CapabilityBoundingSet=\n\
             PrivateDevices=yes\n\
             RestrictAddressFamilies=AF_INET AF_INET6\n"
            .to_string(),
        Tipo::P2p => "User=root\n\
             CapabilityBoundingSet=CAP_NET_ADMIN\n\
             AmbientCapabilities=CAP_NET_ADMIN\n\
             DevicePolicy=closed\n\
             DeviceAllow=/dev/net/tun rw\n\
             RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX AF_NETLINK\n"
            .to_string(),
        // O openvpn cliente cria a placa TUN e desce privilegio sozinho
        // (--user/--group, quando o perfil pede); root aqui e so para a
        // placa nascer.
        Tipo::Cliente => "User=root\n\
             CapabilityBoundingSet=CAP_NET_ADMIN\n\
             AmbientCapabilities=CAP_NET_ADMIN\n\
             DevicePolicy=closed\n\
             DeviceAllow=/dev/net/tun rw\n\
             RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX AF_NETLINK\n"
            .to_string(),
    };
    let depois = if tipo == Tipo::Painel {
        "After=network-online.target postgresql.service\n"
    } else {
        "After=network-online.target\n"
    };
    format!(
        "# phxvpn -- gerado por `phxvpn servico instalar {tipo}`; nao edite, reinstale.\n\
         [Unit]\n\
         Description=phxvpn {tipo} ({unidade})\n\
         {depois}\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={linha}\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         StateDirectory=phxvpn\n\
         StateDirectoryMode=0700\n\
         WorkingDirectory={PASTA_DADOS}\n\
         {creds}\
         {proprio}\
         {comum}\
         \n\
         [Install]\n\
         WantedBy=multi-user.target\n",
        tipo = tipo.nome(),
    )
}

/// Lê um segredo que o sistema entregou ao serviço, ou `None` fora de um:
/// no Linux, o que o systemd decifrou em `$CREDENTIALS_DIRECTORY/<nome>`;
/// no Windows, o selo DPAPI da máquina em `pasta_cred()`.
pub fn credencial(nome: &str) -> Option<String> {
    #[cfg(not(windows))]
    let t = {
        let d = std::env::var_os("CREDENTIALS_DIRECTORY")?;
        std::fs::read_to_string(PathBuf::from(d).join(nome)).ok()?
    };
    #[cfg(windows)]
    let t = {
        let u = std::env::var("PHXVPN_UNIDADE").ok()?;
        let selo = std::fs::read(pasta_cred().join(format!("{u}-{nome}.cred"))).ok()?;
        String::from_utf8(crate::dpapi::abrir(&selo, ENTROPIA_WIN, true).ok()?).ok()?
    };
    Some(t.trim_end_matches(['\n', '\r']).to_string())
}

/// O segredo de um serviço: a credencial do systemd, senão a variável de
/// ambiente (que sai do ambiente assim que lida).
pub fn segredo(credencial_nome: &str, variavel: &str) -> Option<String> {
    if let Some(c) = credencial(credencial_nome) {
        return Some(c);
    }
    let v = std::env::var(variavel).ok();
    std::env::remove_var(variavel);
    v
}

fn rodar(programa: &str, args: &[&str], entrada: Option<&[u8]>) -> R<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut c = Command::new(programa);
    c.args(args).stdout(Stdio::null()).stderr(Stdio::piped());
    if entrada.is_some() {
        c.stdin(Stdio::piped());
    }
    let mut filho = c.spawn().map_err(|e| format!("{programa}: {e}"))?;
    if let (Some(dados), Some(mut si)) = (entrada, filho.stdin.take()) {
        si.write_all(dados).map_err(|e| e.to_string())?;
    }
    let s = filho.wait_with_output().map_err(|e| e.to_string())?;
    if s.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{programa} {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&s.stderr).trim()
        ))
    }
}

/// Cifra o segredo pelo `systemd-creds`, lendo da entrada padrão.
pub fn cifrar_credencial(nome: &str, segredo: &[u8], destino: &Path) -> R<()> {
    let destino = destino.to_string_lossy();
    rodar(
        "systemd-creds",
        &["encrypt", &format!("--name={nome}"), "-", &destino],
        Some(segredo),
    )
}

/// O que `instalar` escreve, para quem quiser conferir antes.
pub struct Plano {
    pub unidade: String,
    pub arquivo: PathBuf,
    pub texto: String,
    /// (nome da credencial, segredo) -- cifrados na instalação.
    pub credenciais: Vec<(String, Vec<u8>)>,
}

/// Monta o plano de um serviço. `args`: o resto da linha (`--dados`,
/// `--rede`, ...); os segredos vêm do ambiente, como no uso em primeiro
/// plano, e aqui viram credenciais.
pub fn planejar(tipo: Tipo, args: &[String], exe: &Path) -> R<Plano> {
    let valor = |nome: &str| {
        args.iter()
            .position(|a| a == nome)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let mut credenciais: Vec<(String, Vec<u8>)> = Vec::new();
    let (unidade, mut linha) = match tipo {
        Tipo::Painel => {
            let pg = std::env::var("PHXVPN_PG")
                .map_err(|_| "informe o PostgreSQL por PHXVPN_PG: ele vira credencial cifrada")?;
            credenciais.push(("pg".into(), pg.into_bytes()));
            if let Ok(m) = std::env::var("PHXVPN_SENHA_MESTRE") {
                credenciais.push(("mestre".into(), m.into_bytes()));
            }
            let mut l = vec!["painel".to_string()];
            if valor("--dados").is_none() {
                l.extend([
                    "--dados".into(),
                    pasta_dados().join("painel").display().to_string(),
                ]);
            }
            (Tipo::Painel.unidade(None), l)
        }
        Tipo::Repasse => {
            let mut l = vec!["repasse".to_string()];
            if valor("--chave").is_none() {
                l.extend([
                    "--chave".into(),
                    pasta_dados().join("repasse.chave").display().to_string(),
                ]);
            }
            (Tipo::Repasse.unidade(None), l)
        }
        Tipo::P2p => {
            let rede = valor("--rede").ok_or("informe --rede NOME")?;
            // A PSK sai do NOME gravado no arquivo da rede: derivar do que
            // foi digitado aqui, com outra caixa, daria um tunel que nunca
            // fecha, calado.
            let arquivo = valor("--arquivo").unwrap_or_else(|| {
                pasta_dados()
                    .join(crate::rede_p2p::Rede::caminho(&rede))
                    .display()
                    .to_string()
            });
            let r = crate::rede_p2p::Rede::ler(&arquivo).map_err(|e| {
                format!(
                    "{e} -- ponha o arquivo da rede em {} (p2p criar/entrar ali)",
                    pasta_dados().display()
                )
            })?;
            let senha = std::env::var("PHXVPN_SENHA_REDE").map_err(|_| {
                "informe a senha da rede por PHXVPN_SENHA_REDE: vira a PSK, cifrada"
            })?;
            let psk = crate::p2p::psk_da_rede(&r.nome, &senha, crate::p2p::ITERACOES_PSK);
            credenciais.push(("psk".into(), phxsql_core::hash::para_hex(&psk).into_bytes()));
            let mut l = vec!["p2p".to_string(), "ligar".to_string()];
            if valor("--arquivo").is_none() {
                l.extend(["--arquivo".into(), arquivo]);
            }
            if valor("--chave").is_none() {
                l.extend([
                    "--chave".into(),
                    pasta_dados().join("p2p.chave").display().to_string(),
                ]);
            }
            (Tipo::P2p.unidade(Some(&rede)), l)
        }
        Tipo::Cliente => {
            let perfil = valor("--perfil")
                .ok_or("informe --perfil ARQUIVO.ovpn (baixado uma vez por `phxvpn entrar`)")?;
            let caminho = std::path::PathBuf::from(&perfil);
            if !caminho.is_file() {
                return Err(format!(
                    "{perfil}: nao existe -- rode `phxvpn entrar --saida {perfil} ...` primeiro"
                ));
            }
            // Rede que exige o autenticador: antes do logon nao ha quem
            // digite o codigo, e o servico ficaria recusado em laco.
            if crate::verificar::perfil_pede_codigo(
                &std::fs::read_to_string(&caminho).unwrap_or_default(),
            ) {
                return Err(format!(
                    "{perfil}: a rede exige o código do autenticador a cada conexão -- \
                     o serviço antes do logon não tem quem o digite; conecte pelo OpenVPN GUI ou Connect"
                ));
            }
            let caminho_absoluto = std::fs::canonicalize(&caminho)
                .map_err(|e| format!("{perfil}: {e}"))?
                .display()
                .to_string();
            // O nome da unidade vem do PERFIL, nao de uma rede pedida a
            // parte -- e o unico identificador que o modo cliente tem: nao
            // fala com o painel para saber o nome da rede, so consome o
            // `.ovpn` ja emitido.
            let nome_rede = caminho
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "perfil".to_string());
            let l = vec![
                "cliente-rodar".to_string(),
                "--perfil".into(),
                caminho_absoluto,
            ];
            (Tipo::Cliente.unidade(Some(&nome_rede)), l)
        }
    };
    // O `--perfil` do Cliente ja entrou em `linha` CANONICALIZADO (caminho
    // absoluto); copiar o `args` original de novo duplicaria a opcao com o
    // caminho relativo que o usuario digitou.
    let resto: Vec<String> = if tipo == Tipo::Cliente {
        sem_opcao(args, "--perfil")
    } else {
        args.to_vec()
    };
    linha.extend(resto);
    let nomes: Vec<&str> = credenciais.iter().map(|(n, _)| n.as_str()).collect();
    // No Windows o «texto» e a linha que o SCM roda; no Linux, a unidade.
    let texto = if cfg!(windows) {
        let mut l = vec![
            exe.display().to_string(),
            "servico-rodar".into(),
            unidade.clone(),
        ];
        l.extend(linha);
        l.iter()
            .map(|a| arg_windows(a))
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        texto_da_unidade(tipo, &unidade, exe, &linha, &nomes)
    };
    Ok(Plano {
        arquivo: if cfg!(windows) {
            PathBuf::from(format!("servico do Windows {unidade}"))
        } else {
            PathBuf::from(format!("/etc/systemd/system/{unidade}.service"))
        },
        unidade,
        texto,
        credenciais,
    })
}

/// Grava as credenciais cifradas e a unidade (Linux) ou o servico
/// (Windows), e (se `iniciar`) o liga. Pede root / administrador.
#[cfg(not(windows))]
pub fn instalar(plano: &Plano, iniciar: bool) -> R<String> {
    std::fs::create_dir_all(PASTA_CRED).map_err(|e| format!("{PASTA_CRED}: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(PASTA_CRED, std::fs::Permissions::from_mode(0o700));
    }
    for (nome, segredo) in &plano.credenciais {
        let destino = PathBuf::from(format!("{PASTA_CRED}/{}-{nome}.cred", plano.unidade));
        cifrar_credencial(nome, segredo, &destino)?;
    }
    std::fs::write(&plano.arquivo, &plano.texto)
        .map_err(|e| format!("{}: {e} (precisa de root)", plano.arquivo.display()))?;
    rodar("systemctl", &["daemon-reload"], None)?;
    if iniciar {
        rodar("systemctl", &["enable", "--now", &plano.unidade], None)?;
        Ok(format!(
            "{} instalado e ligado (systemctl status {})",
            plano.unidade, plano.unidade
        ))
    } else {
        rodar("systemctl", &["enable", &plano.unidade], None)?;
        Ok(format!(
            "{} instalado; sobe no próximo arranque",
            plano.unidade
        ))
    }
}

#[cfg(windows)]
pub fn instalar(plano: &Plano, iniciar: bool) -> R<String> {
    let dados = pasta_dados();
    let cred = pasta_cred();
    std::fs::create_dir_all(&cred).map_err(|e| format!("{}: {e}", cred.display()))?;
    crate::acl::so_do_sistema(&dados)?;
    crate::acl::so_do_sistema(&cred)?;
    for (nome, segredo) in &plano.credenciais {
        let destino = cred.join(format!("{}-{nome}.cred", plano.unidade));
        let selo = crate::dpapi::selar(segredo, ENTROPIA_WIN, true)?;
        std::fs::write(&destino, &selo).map_err(|e| format!("{}: {e}", destino.display()))?;
        crate::acl::so_do_sistema(&destino)?;
    }
    crate::servico_windows::instalar(
        &plano.unidade,
        &format!("phxvpn ({})", plano.unidade),
        "Redes virtuais phxvpn",
        &plano.texto,
        iniciar,
    )?;
    Ok(if iniciar {
        format!(
            "{} instalado e ligado (sc query {})",
            plano.unidade, plano.unidade
        )
    } else {
        format!("{} instalado; sobe no próximo arranque", plano.unidade)
    })
}

#[cfg(not(windows))]
pub fn remover(unidade: &str) -> R<String> {
    let _ = rodar("systemctl", &["disable", "--now", unidade], None);
    let _ = std::fs::remove_file(format!("/etc/systemd/system/{unidade}.service"));
    apagar_credenciais(unidade);
    let _ = rodar("systemctl", &["daemon-reload"], None);
    Ok(format!(
        "{unidade} removido (os dados em {} ficam)",
        pasta_dados().display()
    ))
}

#[cfg(windows)]
pub fn remover(unidade: &str) -> R<String> {
    crate::servico_windows::remover(unidade)?;
    apagar_credenciais(unidade);
    Ok(format!(
        "{unidade} removido (os dados em {} ficam)",
        pasta_dados().display()
    ))
}

fn apagar_credenciais(unidade: &str) {
    if let Ok(d) = std::fs::read_dir(pasta_cred()) {
        for e in d.flatten() {
            if e.file_name()
                .to_string_lossy()
                .starts_with(&format!("{unidade}-"))
            {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn argumento_windows_segue_o_commandlinetoargvw() {
        assert_eq!(arg_windows("--rede"), "--rede");
        assert_eq!(
            arg_windows(r"C:\Program Files\phxvpn\phxvpn.exe"),
            "\"C:\\Program Files\\phxvpn\\phxvpn.exe\""
        );
        assert_eq!(arg_windows("a\"b"), "\"a\\\"b\"");
        assert_eq!(arg_windows(r"C:\dir with\"), "\"C:\\dir with\\\\\"");
        assert_eq!(arg_windows(""), "\"\"");
    }

    #[test]
    fn argumento_com_espaco_e_aspas_vai_escapado() {
        assert_eq!(arg_systemd("--rede"), "--rede");
        assert_eq!(arg_systemd("Filial Sul"), "\"Filial Sul\"");
        assert_eq!(arg_systemd("a\"b"), "\"a\\\"b\"");
        assert_eq!(arg_systemd("50%"), "\"50%%\"");
    }

    #[test]
    fn repasse_sem_root_e_painel_com_credencial_cifrada() {
        let exe = Path::new("/usr/bin/phxvpn");
        let r = texto_da_unidade(
            Tipo::Repasse,
            "phxvpn-repasse",
            exe,
            &["repasse".into()],
            &[],
        );
        assert!(r.contains("DynamicUser=yes") && !r.contains("User=root"));
        assert!(r.contains("CapabilityBoundingSet=\n"));
        let p = texto_da_unidade(
            Tipo::Painel,
            "phxvpn-painel",
            exe,
            &["painel".into()],
            &["pg"],
        );
        assert!(p.contains("LoadCredentialEncrypted=pg:/etc/phxvpn/phxvpn-painel-pg.cred"));
        assert!(
            !p.contains("Environment"),
            "segredo nao entra em Environment="
        );
        assert!(p.contains("DeviceAllow=/dev/net/tun rw"));
    }

    // RED: sem o `sem_opcao(args, "--perfil")` na montagem do plano, o
    // `--perfil` cru do usuario volta pela copia generica do resto dos
    // argumentos, DEPOIS do canonicalizado que o proprio codigo ja pos --
    // e a unidade sobe com dois `--perfil` na mesma linha.
    #[test]
    fn servico_cliente_nao_duplica_o_perfil() {
        let dir =
            std::env::temp_dir().join(format!("phxvpn-servico-cliente-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let perfil = dir.join("rede.ovpn");
        std::fs::write(&perfil, b"perfil de mentira").unwrap();

        let exe = Path::new("/usr/bin/phxvpn");
        let plano = planejar(
            Tipo::Cliente,
            &["--perfil".to_string(), perfil.display().to_string()],
            exe,
        )
        .unwrap();
        let ocorrencias = plano.texto.matches("--perfil").count();
        assert_eq!(ocorrencias, 1, "--perfil duplicado:\n{}", plano.texto);
        assert!(plano.texto.contains("cliente-rodar"));
        assert!(plano.texto.contains("CAP_NET_ADMIN"));
        assert!(plano.unidade.starts_with("phxvpn-cliente-"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // RED: sem a conferencia `caminho.is_file()`, o servico "cliente" se
    // instalaria apontando para um perfil que nunca existiu, e so falharia
    // no proximo arranque da maquina -- tarde demais para quem esta
    // instalando decidir algo a respeito.
    #[test]
    fn servico_cliente_recusa_perfil_inexistente() {
        let exe = Path::new("/usr/bin/phxvpn");
        let r = planejar(
            Tipo::Cliente,
            &[
                "--perfil".to_string(),
                "/tmp/phxvpn-perfil-que-nao-existe-de-verdade.ovpn".to_string(),
            ],
            exe,
        );
        assert!(r.is_err());
    }

    /// RED: sem a conferencia, o servico se instalaria com um perfil que
    /// pede codigo -- e o `openvpn` antes do logon falharia em laco.
    #[test]
    fn servico_cliente_recusa_perfil_que_pede_codigo() {
        let d =
            std::env::temp_dir().join(format!("phxvpn-servico-mfa-{}.ovpn", std::process::id()));
        std::fs::write(&d, format!("client\n{}", crate::verificar::PERFIL_MFA)).unwrap();
        let r = planejar(
            Tipo::Cliente,
            &["--perfil".to_string(), d.display().to_string()],
            Path::new("/usr/bin/phxvpn"),
        );
        let _ = std::fs::remove_file(&d);
        assert!(r.err().unwrap_or_default().contains("autenticador"));
    }

    #[test]
    fn tipo_de_texto_aceita_cliente() {
        assert_eq!(Tipo::de_texto("cliente").unwrap(), Tipo::Cliente);
    }
}
