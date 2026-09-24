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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tipo {
    Painel,
    Repasse,
    P2p,
}

impl Tipo {
    pub fn de_texto(t: &str) -> R<Tipo> {
        match t {
            "painel" => Ok(Tipo::Painel),
            "repasse" => Ok(Tipo::Repasse),
            "p2p" => Ok(Tipo::P2p),
            _ => Err(format!("serviço {t:?}? use painel, repasse ou p2p")),
        }
    }

    fn nome(self) -> &'static str {
        match self {
            Tipo::Painel => "painel",
            Tipo::Repasse => "repasse",
            Tipo::P2p => "p2p",
        }
    }

    /// Nome da unidade: `phxvpn-painel`, `phxvpn-repasse`, `phxvpn-p2p-<rede>`.
    pub fn unidade(self, rede: Option<&str>) -> String {
        match (self, rede) {
            (Tipo::P2p, Some(r)) => format!("phxvpn-p2p-{}", limpo(r)),
            _ => format!("phxvpn-{}", self.nome()),
        }
    }
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

/// Lê um segredo que o systemd entregou (`$CREDENTIALS_DIRECTORY/<nome>`),
/// ou `None` fora de um serviço.
pub fn credencial(nome: &str) -> Option<String> {
    let d = std::env::var_os("CREDENTIALS_DIRECTORY")?;
    std::fs::read_to_string(PathBuf::from(d).join(nome))
        .ok()
        .map(|t| t.trim_end_matches(['\n', '\r']).to_string())
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
                l.extend(["--dados".into(), format!("{PASTA_DADOS}/painel")]);
            }
            (Tipo::Painel.unidade(None), l)
        }
        Tipo::Repasse => {
            let mut l = vec!["repasse".to_string()];
            if valor("--chave").is_none() {
                l.extend(["--chave".into(), format!("{PASTA_DADOS}/repasse.chave")]);
            }
            (Tipo::Repasse.unidade(None), l)
        }
        Tipo::P2p => {
            let rede = valor("--rede").ok_or("informe --rede NOME")?;
            // A PSK sai do NOME gravado no arquivo da rede: derivar do que
            // foi digitado aqui, com outra caixa, daria um tunel que nunca
            // fecha, calado.
            let arquivo = valor("--arquivo").unwrap_or_else(|| {
                format!("{PASTA_DADOS}/{}", crate::rede_p2p::Rede::caminho(&rede))
            });
            let r = crate::rede_p2p::Rede::ler(&arquivo).map_err(|e| {
                format!("{e} -- ponha o arquivo da rede em {PASTA_DADOS} (p2p criar/entrar ali)")
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
                l.extend(["--chave".into(), format!("{PASTA_DADOS}/p2p.chave")]);
            }
            (Tipo::P2p.unidade(Some(&rede)), l)
        }
    };
    linha.extend(args.iter().cloned());
    let nomes: Vec<&str> = credenciais.iter().map(|(n, _)| n.as_str()).collect();
    let texto = texto_da_unidade(tipo, &unidade, exe, &linha, &nomes);
    Ok(Plano {
        arquivo: PathBuf::from(format!("/etc/systemd/system/{unidade}.service")),
        unidade,
        texto,
        credenciais,
    })
}

/// Grava a unidade e as credenciais cifradas, e (se `iniciar`) liga o
/// serviço. Pede root.
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

pub fn remover(unidade: &str) -> R<String> {
    let _ = rodar("systemctl", &["disable", "--now", unidade], None);
    let _ = std::fs::remove_file(format!("/etc/systemd/system/{unidade}.service"));
    if let Ok(d) = std::fs::read_dir(PASTA_CRED) {
        for e in d.flatten() {
            if e.file_name()
                .to_string_lossy()
                .starts_with(&format!("{unidade}-"))
            {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let _ = rodar("systemctl", &["daemon-reload"], None);
    Ok(format!(
        "{unidade} removido (os dados em {PASTA_DADOS} ficam)"
    ))
}

#[cfg(test)]
mod testes {
    use super::*;

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
}
