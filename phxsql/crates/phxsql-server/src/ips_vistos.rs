//! Pedido 765, fatia P6: a memoria de quem entrou de onde.
//!
//! Desenho em `docs/propostas/protecao-765-desenho.md` §4.4. Uma chave por
//! (usuario, database, IP canonico), com o primeiro e o ultimo login com
//! sucesso. Responde a tres perguntas, e so a elas:
//!
//! * **o IP e novo para este usuario?** -- primeiro login com sucesso nunca
//!   visto em 90 dias vira a ocorrencia amarela `IpNovo`. Nunca recusa login:
//!   e aviso para quem administra, nao portao;
//! * **um administrador entrou deste IP nas ultimas 24 h?** e
//! * **dois usuarios distintos entraram deste IP em 30 dias?** -- as duas
//!   guardas de nao se trancar para fora que dependem de QUEM entrou (P9;
//!   `blacklist::Guarda`).
//!
//! # O arquivo
//!
//! `ips-vistos.jsonl`, ao lado do `acessos.log`, 0600 pelo motor da
//! permissao do banco: diz quem entrou de onde, que e dado de acesso como o
//! proprio log. Uma linha JSON por fato, ACRESCENTADA -- a chave nova, ou o
//! ultimo login de uma chave que andou mais de uma hora, ou a chave que
//! passou a ser de administrador. No arranque as linhas se dobram (a ultima
//! de cada chave vence) e, quando o arquivo tem o triplo de linhas que de
//! chaves, ele se reescreve dobrado pela troca duravel. Formato em
//! `docs/FORMATO.md`.
//!
//! # A semente
//!
//! No arranque em que o arquivo ainda nao existe, a memoria nasce dos logins
//! com sucesso do `acessos.log` -- senao, no dia em que esta versao sobe,
//! todo usuario de toda maquina viraria «IP novo» de uma vez, e o
//! administrador aprenderia a ignorar o aviso no primeiro dia.
//!
//! # O teto
//!
//! 50.000 chaves. Cheio, sai primeiro o que passou dos 90 dias; se ainda
//! nao couber, a chave nova NAO entra -- o login continua sendo acusado como
//! novo (ele e), e conta no `coringa`, que a ocorrencia e a tela podem
//! mostrar. Quem varia o IP nao faz a memoria crescer sem fim.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use phxsql_core::error::Result;
use phxsql_core::json::Json;

use crate::acesso::Acesso;
use crate::blacklist::{ip_canonico, Guarda};

/// O nome do arquivo, ao lado do `acessos.log`.
pub const NOME_DO_ARQUIVO: &str = "ips-vistos.jsonl";
/// Quantas chaves a memoria guarda.
pub const TETO_DE_CHAVES: usize = 50_000;
/// Quanto tempo um IP continua «visto»: 90 dias sem login dele, e o
/// proximo volta a ser novo.
pub const JANELA_DO_NOVO_MS: i64 = 90 * 24 * 3_600_000;
/// A janela do IP compartilhado: dois usuarios em 30 dias.
pub const JANELA_DO_COMPARTILHADO_MS: i64 = 30 * 24 * 3_600_000;
/// A janela do administrador: um login dele nas ultimas 24 h.
pub const JANELA_DO_ADMINISTRADOR_MS: i64 = 24 * 3_600_000;
/// De quanto em quanto o `ultimo` de uma chave conhecida volta ao arquivo.
/// Um login por minuto do mesmo usuario no mesmo IP nao vira uma linha por
/// minuto; uma hora de folga nao muda nenhuma das tres respostas.
const PASSO_DO_ULTIMO_MS: i64 = 3_600_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Chave {
    usuario: String,
    database: String,
    ip: String,
}

#[derive(Debug, Clone, Copy)]
struct Visto {
    primeiro_ms: i64,
    ultimo_ms: i64,
    admin: bool,
}

/// A memoria. O servidor a guarda num `Mutex` proprio, fora da trava de
/// dados: o login nao a toca com nada mais na mao.
#[derive(Debug, Default)]
pub struct IpsVistos {
    caminho: Option<PathBuf>,
    mapa: HashMap<Chave, Visto>,
    /// Logins de chave nova que nao couberam (o teto estava cheio).
    coringa: u64,
}

impl IpsVistos {
    /// Abre a memoria. Arquivo ausente: nasce da `semente` (os acessos do
    /// `acessos.log`) e e gravado inteiro, para a semente nao rodar de novo
    /// no arranque seguinte.
    pub fn abrir(
        caminho: impl AsRef<Path>,
        semente: impl FnOnce() -> Vec<Acesso>,
    ) -> Result<IpsVistos> {
        let caminho = caminho.as_ref().to_path_buf();
        let mut m = IpsVistos {
            caminho: Some(caminho.clone()),
            ..IpsVistos::default()
        };
        match std::fs::read_to_string(&caminho) {
            Ok(texto) => {
                let mut linhas = 0usize;
                for linha in texto.lines().filter(|l| !l.trim().is_empty()) {
                    linhas += 1;
                    // Linha torta (a ultima, cortada por uma queda) nao
                    // derruba a memoria: o fato dela se reaprende no proximo
                    // login.
                    if let Some((k, v)) = Json::analisar(linha).ok().as_ref().and_then(de_json) {
                        m.dobrar(k, v);
                    }
                }
                if linhas > 3 * m.mapa.len() + 1_000 {
                    m.gravar_inteiro()?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                for a in semente() {
                    if !(a.op == "login" && a.ok) || a.usuario.is_empty() {
                        continue;
                    }
                    let Some(ip) = ip_canonico(&a.ip) else {
                        continue;
                    };
                    let k = Chave {
                        usuario: a.usuario.clone(),
                        database: a.database.clone(),
                        ip: ip.to_string(),
                    };
                    m.dobrar(
                        k,
                        Visto {
                            primeiro_ms: a.quando_ms,
                            ultimo_ms: a.quando_ms,
                            admin: false,
                        },
                    );
                }
                m.gravar_inteiro()?;
            }
            Err(e) => return Err(e.into()),
        }
        Ok(m)
    }

    /// Junta um fato ja ocorrido (do arquivo ou da semente): o primeiro e o
    /// menor, o ultimo e o maior, o administrador fica.
    fn dobrar(&mut self, k: Chave, v: Visto) {
        if let Some(e) = self.mapa.get_mut(&k) {
            e.primeiro_ms = e.primeiro_ms.min(v.primeiro_ms);
            e.ultimo_ms = e.ultimo_ms.max(v.ultimo_ms);
            e.admin |= v.admin;
            return;
        }
        if self.mapa.len() < TETO_DE_CHAVES {
            self.mapa.insert(k, v);
        }
    }

    /// Um login com sucesso. Devolve `true` quando a chave e NOVA -- nunca
    /// vista, ou vista ha mais de 90 dias --, que e quando nasce o `IpNovo`.
    ///
    /// A falha de gravar volta como erro, mas a memoria em processo ja mudou:
    /// o aviso e de disco, e a resposta ao login nao depende dele.
    pub fn registrar(
        &mut self,
        usuario: &str,
        database: &str,
        ip: &str,
        admin: bool,
        agora_ms: i64,
    ) -> Result<bool> {
        let Some(ip) = ip_canonico(ip) else {
            return Ok(false);
        };
        let k = Chave {
            usuario: usuario.to_string(),
            database: database.to_string(),
            ip: ip.to_string(),
        };
        let (novo, gravar) = match self.mapa.get_mut(&k) {
            Some(v) => {
                let novo = agora_ms - v.ultimo_ms >= JANELA_DO_NOVO_MS;
                let gravar =
                    novo || agora_ms - v.ultimo_ms >= PASSO_DO_ULTIMO_MS || (admin && !v.admin);
                if novo {
                    v.primeiro_ms = agora_ms;
                }
                v.ultimo_ms = v.ultimo_ms.max(agora_ms);
                v.admin |= admin;
                (novo, gravar)
            }
            None => {
                if self.mapa.len() >= TETO_DE_CHAVES {
                    self.mapa
                        .retain(|_, v| agora_ms - v.ultimo_ms < JANELA_DO_NOVO_MS);
                }
                if self.mapa.len() >= TETO_DE_CHAVES {
                    self.coringa += 1;
                    return Ok(true);
                }
                self.mapa.insert(
                    k.clone(),
                    Visto {
                        primeiro_ms: agora_ms,
                        ultimo_ms: agora_ms,
                        admin,
                    },
                );
                (true, true)
            }
        };
        if gravar {
            let v = self.mapa[&k];
            self.acrescentar(&para_json(&k, &v))?;
        }
        Ok(novo)
    }

    /// A guarda que depende de quem entrou deste IP (pedido 766, P9): o
    /// administrador nas ultimas 24 h primeiro, depois o IP de dois usuarios
    /// em 30 dias. `None` quando nenhuma das duas vale.
    pub fn guarda(&self, ip: &str, agora_ms: i64) -> Option<Guarda> {
        let ip = ip_canonico(ip)?.to_string();
        let mut usuarios: Vec<&str> = Vec::new();
        for (k, v) in self.mapa.iter().filter(|(k, _)| k.ip == ip) {
            if v.admin && agora_ms - v.ultimo_ms < JANELA_DO_ADMINISTRADOR_MS {
                return Some(Guarda::Administrador);
            }
            if agora_ms - v.ultimo_ms < JANELA_DO_COMPARTILHADO_MS
                && !usuarios.contains(&k.usuario.as_str())
            {
                usuarios.push(&k.usuario);
            }
        }
        (usuarios.len() >= 2).then_some(Guarda::Compartilhado)
    }

    /// Quantas chaves a memoria guarda.
    pub fn quantas(&self) -> usize {
        self.mapa.len()
    }

    /// Logins de chave nova que nao couberam.
    pub fn coringa(&self) -> u64 {
        self.coringa
    }

    fn acrescentar(&self, j: &Json) -> Result<()> {
        let Some(caminho) = &self.caminho else {
            return Ok(());
        };
        let mut f = phxsql_store::permissao::opcoes_do_banco()
            .create(true)
            .append(true)
            .open(caminho)?;
        let mut linha = j.escrever();
        linha.push('\n');
        f.write_all(linha.as_bytes())?;
        Ok(())
    }

    /// O arquivo inteiro, dobrado, pela troca duravel: a semente e a
    /// compactacao. Uma queda no meio deixa o arquivo de antes.
    fn gravar_inteiro(&self) -> Result<()> {
        let Some(caminho) = &self.caminho else {
            return Ok(());
        };
        if let Some(dir) = caminho.parent().filter(|d| !d.as_os_str().is_empty()) {
            phxsql_store::permissao::criar_diretorio_do_banco(dir)?;
        }
        let mut chaves: Vec<(&Chave, &Visto)> = self.mapa.iter().collect();
        chaves.sort_by_key(|(_, v)| v.primeiro_ms);
        let mut corpo = String::new();
        for (k, v) in chaves {
            corpo.push_str(&para_json(k, v).escrever());
            corpo.push('\n');
        }
        phxsql_store::sincronia::gravar_duravel(caminho, corpo.as_bytes())
    }
}

fn para_json(k: &Chave, v: &Visto) -> Json {
    Json::objeto(vec![
        ("usuario", Json::texto_de(&k.usuario)),
        ("database", Json::texto_de(&k.database)),
        ("ip", Json::texto_de(&k.ip)),
        ("primeiro_ms", Json::de_i64(v.primeiro_ms)),
        ("ultimo_ms", Json::de_i64(v.ultimo_ms)),
        ("admin", Json::Bool(v.admin)),
    ])
}

fn de_json(j: &Json) -> Option<(Chave, Visto)> {
    let ip = ip_canonico(j.campo("ip")?.texto()?)?.to_string();
    let primeiro_ms = j.campo("primeiro_ms")?.numero()? as i64;
    Some((
        Chave {
            usuario: j.texto_ou("usuario", "").to_string(),
            database: j.texto_ou("database", "").to_string(),
            ip,
        },
        Visto {
            primeiro_ms,
            ultimo_ms: j
                .campo("ultimo_ms")
                .and_then(Json::numero)
                .map_or(primeiro_ms, |n| n as i64),
            admin: j.booleano_ou("admin", false),
        },
    ))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;

    const T0: i64 = 1_800_000_000_000;
    const DIA: i64 = 24 * 3_600_000;

    fn login(usuario: &str, ip: &str, quando_ms: i64) -> Acesso {
        Acesso {
            quando_ms,
            ip: ip.into(),
            op: "login".into(),
            usuario: usuario.into(),
            autenticado: true,
            ok: true,
            ..Acesso::default()
        }
    }

    /// **O aceite da P6:** o primeiro login de um IP novo e novo; o segundo,
    /// nao. RED: `registrar` devolvendo sempre `true` (sem memoria) reprova
    /// o segundo; sempre `false`, o primeiro.
    #[test]
    fn o_primeiro_login_de_um_ip_e_novo_e_o_segundo_nao() {
        let d = DirTemp::novo("ips-vistos-1");
        let mut m = IpsVistos::abrir(d.join(NOME_DO_ARQUIVO), Vec::new).unwrap();
        assert!(m.registrar("ana", "", "203.0.113.5", false, T0).unwrap());
        assert!(!m
            .registrar("ana", "", "203.0.113.5", false, T0 + 1)
            .unwrap());
        // Outro usuario no mesmo IP e outra chave: e novo PARA ELE.
        assert!(m
            .registrar("bia", "", "203.0.113.5", false, T0 + 2)
            .unwrap());
        // A forma mapeada e o mesmo IP.
        assert!(!m
            .registrar("ana", "", "::ffff:203.0.113.5", false, T0 + 3)
            .unwrap());
        // E sobrevive ao reinicio, pelo arquivo.
        let mut m = IpsVistos::abrir(d.join(NOME_DO_ARQUIVO), Vec::new).unwrap();
        assert!(!m
            .registrar("ana", "", "203.0.113.5", false, T0 + 4)
            .unwrap());
        // 90 dias depois do ultimo, volta a ser novo.
        assert!(m
            .registrar("ana", "", "203.0.113.5", false, T0 + 4 + JANELA_DO_NOVO_MS)
            .unwrap());
    }

    /// A semente: quem ja entrou pelo `acessos.log` nao e novo no dia em que a
    /// memoria nasce. So login com sucesso e com usuario conta.
    #[test]
    fn o_ip_que_ja_esta_no_acessos_log_nao_e_novo() {
        let d = DirTemp::novo("ips-vistos-semente");
        let mut recusado = login("caio", "203.0.113.8", T0);
        recusado.ok = false;
        let semente = vec![
            login("ana", "203.0.113.7", T0 - DIA),
            recusado,
            Acesso {
                op: "ping".into(),
                ..login("dora", "203.0.113.9", T0)
            },
        ];
        let mut m = IpsVistos::abrir(d.join(NOME_DO_ARQUIVO), || semente).unwrap();
        assert_eq!(m.quantas(), 1, "so o login com sucesso semeia");
        assert!(!m.registrar("ana", "", "203.0.113.7", false, T0).unwrap());
        assert!(m.registrar("caio", "", "203.0.113.8", false, T0).unwrap());
        assert!(m.registrar("dora", "", "203.0.113.9", false, T0).unwrap());
        // A semente roda UMA vez: o arquivo existe, e o segundo arranque nao
        // a chama.
        let m = IpsVistos::abrir(d.join(NOME_DO_ARQUIVO), || {
            panic!("a semente rodou de novo com o arquivo ja existindo")
        })
        .unwrap();
        assert_eq!(m.quantas(), 3);
    }

    /// O teto: a chave 50.001 nao entra, conta no coringa, e continua
    /// acusada como nova. RED: tirar o `if self.mapa.len() >= TETO` faz o
    /// mapa crescer.
    #[test]
    fn com_o_teto_cheio_o_mapa_nao_cresce() {
        let mut m = IpsVistos::default();
        for i in 0..TETO_DE_CHAVES as u32 {
            let ip = std::net::Ipv4Addr::from(0x0A00_0000 + i).to_string();
            m.registrar("ana", "", &ip, false, T0).unwrap();
        }
        assert_eq!(m.quantas(), TETO_DE_CHAVES);
        assert!(m.registrar("ana", "", "203.0.113.200", false, T0).unwrap());
        assert_eq!(m.quantas(), TETO_DE_CHAVES, "o mapa cresceu alem do teto");
        assert_eq!(m.coringa(), 1);
        // Passados os 90 dias, a chave velha sai e a nova cabe.
        assert!(m
            .registrar("ana", "", "203.0.113.200", false, T0 + JANELA_DO_NOVO_MS)
            .unwrap());
        assert_eq!(m.quantas(), 1);
    }

    /// As duas guardas que dependem de quem entrou (P9).
    #[test]
    fn administrador_em_24h_e_dois_usuarios_em_30_dias_guardam_o_ip() {
        let mut m = IpsVistos::default();
        let ip = "198.51.100.4";
        m.registrar("ana", "", ip, false, T0).unwrap();
        assert_eq!(m.guarda(ip, T0), None, "um usuario so nao e compartilhado");
        m.registrar("bia", "", ip, false, T0 + 1).unwrap();
        assert_eq!(m.guarda(ip, T0 + 2), Some(Guarda::Compartilhado));
        assert_eq!(m.guarda(ip, T0 + JANELA_DO_COMPARTILHADO_MS + 2), None);
        let adm = "198.51.100.5";
        m.registrar("root", "", adm, true, T0).unwrap();
        assert_eq!(m.guarda(adm, T0 + 1), Some(Guarda::Administrador));
        assert_eq!(m.guarda(adm, T0 + JANELA_DO_ADMINISTRADOR_MS), None);
        assert_eq!(m.guarda("nao e ip", T0), None);
    }
}
