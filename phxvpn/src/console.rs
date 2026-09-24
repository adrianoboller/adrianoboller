//! `phxvpncmd` -- o console do phxvpn, no estilo do prompt do MS-DOS.
//!
//! Tres modos, como o `vpncmd` do SoftEther (a referencia de uso escolhida):
//!
//! ```text
//! 1. Painel       gerir o servidor: redes, usuarios, servidores, cofre
//! 2. P2P          este computador: chave, ligar, pares
//! 3. Ferramentas  autoteste dos vetores oficiais, bancada da cifra, chave
//! ```
//!
//! Parametros no jeito DOS: `COMANDO posicional /nome:valor /bandeira`,
//! sem diferenca de caixa. Roda interativo, por arquivo de lote
//! (`/entrada:script.txt`, o `.bat` do phxvpn) ou uma linha so (`/comando:`).
//!
//! # Um motor so
//!
//! Nada de regra aqui: cada comando chama `comandos.rs` -- o MESMO codigo do
//! `phxvpn` de linha de comando. O console so traduz a linha e desenha a
//! tabela.
//!
//! # O que ele nao faz (e diz)
//!
//! Sem historico nem setas, e a senha digitada aparece na tela: esconder o eco
//! e ler tecla a tecla pede o terminal em modo cru, que e uma crate -- e a
//! petrea e zero dependencia. E o mesmo limite do `phxsqlcmd`. Para nao
//! digitar senha, use as variaveis `PHXVPN_SENHA`, `PHXVPN_SENHA_REDE`,
//! `PHXVPN_SENHA_MESTRE` e `PHXVPN_SENHA_NOVA`.

use crate::comandos::{self, palavras, Opcoes};
use phxsql_core::json::Json;
use std::io::{BufRead, Write};
use std::time::{Duration, Instant};

pub type R<T> = Result<T, String>;

/// Pergunta uma senha: (rotulo, variavel de ambiente) -> texto.
pub type Perguntar<'a> = Box<dyn FnMut(&str, &str) -> String + 'a>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Modo {
    Painel,
    P2p,
    Ferramentas,
}

impl Modo {
    fn nome(self) -> &'static str {
        match self {
            Modo::Painel => "Painel",
            Modo::P2p => "P2P",
            Modo::Ferramentas => "Ferramentas",
        }
    }

    fn de_texto(t: &str) -> Option<Modo> {
        match t.to_lowercase().as_str() {
            "1" | "painel" => Some(Modo::Painel),
            "2" | "p2p" => Some(Modo::P2p),
            "3" | "ferramentas" => Some(Modo::Ferramentas),
            _ => None,
        }
    }
}

/// O que o console devolve a cada linha.
#[derive(Debug, PartialEq, Eq)]
pub enum Fim {
    Continua(String),
    Sair,
}

pub struct Console<'a> {
    pub modo: Modo,
    painel: String,
    token: Option<String>,
    login: Option<String>,
    #[cfg(any(target_os = "linux", windows))]
    no: Option<std::sync::Arc<crate::p2p::No>>,
    /// Pergunta uma senha: (rotulo, variavel de ambiente) -> texto. Trocavel
    /// nos testes.
    perguntar: Perguntar<'a>,
}

const AJUDA_GERAL: &str = "\
Comandos de todo modo:
  MODO <PAINEL|P2P|FERRAMENTAS>   troca de modo (ou 1, 2, 3)
  AJUDA [comando]  ou  ?          esta lista
  CLS                             limpa a tela
  VERSAO                          versao do phxvpn
  SAIR  ou  EXIT                  termina
";

const AJUDA_PAINEL: &str = "\
Modo Painel (gerir o servidor):
  CONECTAR <http://host:porta>          escolhe o painel (padrao http://127.0.0.1:8470)
  LOGIN <usuario>                       entra (senha: PHXVPN_SENHA ou pergunta)
  LOGOUT                                sai da sessao
  ESTADO                                instalado, cofre, empresa
  REDES                                 lista as redes
  MEMBROS <id_da_rede>                  quem esta na rede, IP, conectado
  CRIARREDE <nome> [/FINALIDADE:x] [/SAIDA:arq.ovpn]
  ENTRARREDE <nome> [/SAIDA:arq.ovpn]   baixa o perfil (senha da rede: PHXVPN_SENHA_REDE)
  SAIRREDE <id_da_rede>
  USUARIOS                              (admin)
  USUARIONOVO <login> [/EMAIL:x] [/ADMIN]  (admin; senha: PHXVPN_SENHA_NOVA)
  SERVIDORES
  SERVIDORNOVO <nome> /IP:x [/DNS:x]    (admin)
  DESTRANCAR                            abre o cofre (PHXVPN_SENHA_MESTRE)
";

const AJUDA_P2P: &str = "\
Modo P2P (este computador):
  CHAVE [/ARQUIVO:p2p.chave]            cria/mostra a identidade P2P
  CRIAR /REDE:x [/IP:10.78.0.1/24] [/MODO:auto] [/REPASSE:CHAVE@HOST:PORTA]
                                        a rede nasce aqui (arquivo x.p2p)
  CONVIDAR /REDE:x [/ENDERECO:meu_host:porta] [/VALIDADE:24h]
                                        codigo do convite (senha: PHXVPN_SENHA_REDE)
  ENTRAR <codigo>                       aceita um convite e grava a rede aqui
  LIGAR /REDE:x                         com o arquivo da rede, basta o nome; sem ele:
  LIGAR /REDE:x /IP:10.78.0.1/24 /PAR:CHAVE@IP[@HOST:PORTA] [/PAR:...]
        [/MODO:direto|repasse|auto] [/REPASSE:CHAVE@HOST:PORTA]
        [/PORTA:51820] [/INTERFACE:phx0] [/CHAVE:p2p.chave]
                                        liga o tunel (Linux, root) e volta ao prompt
  PARES                                 situacao de cada par: caminho e sessao
";

const AJUDA_FERRAMENTAS: &str = "\
Modo Ferramentas:
  AUTOTESTE                 confere Noise, SCRAM, cofre e certificado contra vetor
  BANCADA [/SEGUNDOS:1]     mede a cifra e o aperto de mao nesta maquina
  GERARCHAVE                um par X25519 novo (nao grava nada)
";

/// Tabela no jeito do console: colunas alinhadas, bordas de ASCII puro
/// (o `cmd.exe` antigo nao desenha as linhas do Unicode).
pub fn tabela(cabecalho: &[&str], linhas: &[Vec<String>]) -> String {
    let mut larguras: Vec<usize> = cabecalho.iter().map(|c| c.chars().count()).collect();
    for l in linhas {
        for (i, c) in l.iter().enumerate() {
            if i < larguras.len() {
                larguras[i] = larguras[i].max(c.chars().count());
            }
        }
    }
    let borda: String = larguras
        .iter()
        .map(|w| format!("+{}", "-".repeat(w + 2)))
        .collect::<String>()
        + "+\n";
    let linha = |cels: Vec<String>| -> String {
        cels.iter()
            .zip(&larguras)
            .map(|(c, w)| format!("| {c}{} ", " ".repeat(w - c.chars().count())))
            .collect::<String>()
            + "|\n"
    };
    let mut s = borda.clone();
    s += &linha(cabecalho.iter().map(|c| c.to_string()).collect());
    s += &borda;
    for l in linhas {
        s += &linha(l.clone());
    }
    s += &borda;
    s += &format!("{} linha(s)\n", linhas.len());
    s
}

fn texto(j: &Json, campo: &str) -> String {
    match j.campo(campo) {
        Some(Json::Texto(t)) => t.clone(),
        Some(Json::Bool(b)) => if *b { "sim" } else { "nao" }.into(),
        Some(Json::Numero(n)) => format!("{n}"),
        Some(Json::Nulo) | None => "-".into(),
        Some(outro) => outro.escrever(),
    }
}

fn lista_para_tabela(j: &Json, colunas: &[(&str, &str)]) -> String {
    let linhas: Vec<Vec<String>> = j
        .lista()
        .unwrap_or_default()
        .iter()
        .map(|item| {
            colunas
                .iter()
                .map(|(campo, _)| texto(item, campo))
                .collect()
        })
        .collect();
    let cab: Vec<&str> = colunas.iter().map(|(_, rotulo)| *rotulo).collect();
    tabela(&cab, &linhas)
}

/// Senha: primeiro a variavel de ambiente, depois a pergunta no terminal.
pub fn perguntar_no_terminal(rotulo: &str, var: &str) -> String {
    if let Ok(s) = std::env::var(var) {
        return s;
    }
    eprint!("{rotulo} (aparece na tela): ");
    std::io::stderr().flush().ok();
    let mut l = String::new();
    let _ = std::io::stdin().lock().read_line(&mut l);
    l.trim_end_matches(['\r', '\n']).to_string()
}

impl<'a> Console<'a> {
    pub fn novo(modo: Modo, perguntar: Perguntar<'a>) -> Console<'a> {
        Console {
            modo,
            painel: "http://127.0.0.1:8470".into(),
            token: None,
            login: None,
            #[cfg(any(target_os = "linux", windows))]
            no: None,
            perguntar,
        }
    }

    pub fn prompt(&self) -> String {
        match (&self.modo, &self.login) {
            (Modo::Painel, Some(l)) => format!("phxvpn Painel {l}>"),
            (m, _) => format!("phxvpn {}>", m.nome()),
        }
    }

    fn ajuda(&self) -> String {
        let do_modo = match self.modo {
            Modo::Painel => AJUDA_PAINEL,
            Modo::P2p => AJUDA_P2P,
            Modo::Ferramentas => AJUDA_FERRAMENTAS,
        };
        format!("{do_modo}\n{AJUDA_GERAL}")
    }

    fn token(&self) -> R<&str> {
        self.token
            .as_deref()
            .ok_or_else(|| "faca LOGIN <usuario> antes".to_string())
    }

    /// Executa uma linha. Erro volta como `Err` com a frase para a tela.
    pub fn executar(&mut self, linha: &str) -> R<Fim> {
        let p = palavras(linha);
        let Some(cmd) = p.first().map(|c| c.to_lowercase()) else {
            return Ok(Fim::Continua(String::new()));
        };
        let o = Opcoes::de_dos(&p[1..]);
        let arg = |i: usize| o.posicionais.get(i).map(String::as_str);
        let ok = |s: String| Ok(Fim::Continua(s));
        match cmd.as_str() {
            "sair" | "exit" | "quit" => return Ok(Fim::Sair),
            "ajuda" | "help" | "?" => return ok(self.ajuda()),
            "cls" => return ok("\x1b[2J\x1b[H".into()),
            "versao" | "ver" => return ok(format!("phxvpn {}\n", env!("CARGO_PKG_VERSION"))),
            "modo" => {
                let m = arg(0)
                    .and_then(Modo::de_texto)
                    .ok_or("use MODO PAINEL, MODO P2P ou MODO FERRAMENTAS")?;
                self.modo = m;
                return ok(format!("modo {}\n", m.nome()));
            }
            _ => {}
        }
        match self.modo {
            Modo::Painel => self.painel(&cmd, &o),
            Modo::P2p => self.p2p(&cmd, &o),
            Modo::Ferramentas => self.ferramentas(&cmd, &o),
        }
        .map(Fim::Continua)
    }

    fn painel(&mut self, cmd: &str, o: &Opcoes) -> R<String> {
        let arg = |i: usize| o.posicionais.get(i).map(String::as_str);
        let get = |s: &Self, rota: &str| {
            comandos::pedir(&s.painel, "GET", rota, s.token.as_deref(), None)
        };
        match cmd {
            "conectar" => {
                let url = arg(0).ok_or("CONECTAR http://host:porta")?;
                if !url.starts_with("http://") {
                    return Err("o endereco tem de comecar com http://".into());
                }
                self.painel = url.trim_end_matches('/').to_string();
                self.token = None;
                self.login = None;
                let e = get(self, "/api/estado")?;
                Ok(format!(
                    "conectado a {} -- instalado: {}, cofre: {}\n",
                    self.painel,
                    texto(&e, "instalado"),
                    if e.booleano_ou("destrancado", false) {
                        "destrancado"
                    } else {
                        "trancado"
                    }
                ))
            }
            "login" => {
                let usuario = arg(0).ok_or("LOGIN <usuario>")?.to_string();
                let senha = (self.perguntar)("senha do usuario", "PHXVPN_SENHA");
                let (token, admin) = comandos::login(&self.painel, &usuario, &senha)?;
                self.token = Some(token);
                self.login = Some(usuario.clone());
                Ok(format!(
                    "sessao aberta: {usuario}{}\n",
                    if admin { " (admin)" } else { "" }
                ))
            }
            "logout" => {
                if let Some(t) = self.token.take() {
                    let _ = comandos::pedir(&self.painel, "POST", "/api/sair", Some(&t), None);
                }
                self.login = None;
                Ok("sessao encerrada\n".into())
            }
            "estado" => {
                let e = get(self, "/api/estado")?;
                let emp = e.campo("empresa").cloned().unwrap_or(Json::Nulo);
                let mut l = vec![
                    vec!["Painel".into(), self.painel.clone()],
                    vec!["Instalado".into(), texto(&e, "instalado")],
                    vec!["Cofre destrancado".into(), texto(&e, "destrancado")],
                    vec!["OpenVPN supervisionado".into(), texto(&e, "openvpn")],
                ];
                for (c, r) in [
                    ("nome", "Empresa"),
                    ("finalidade", "Finalidade"),
                    ("responsavel", "Responsavel"),
                    ("email", "E-mail"),
                    ("telefone", "Telefone"),
                ] {
                    if emp.campo(c).is_some() {
                        l.push(vec![r.into(), texto(&emp, c)]);
                    }
                }
                Ok(tabela(&["Item", "Valor"], &l))
            }
            "redes" => {
                self.token()?;
                Ok(lista_para_tabela(
                    &get(self, "/api/redes")?,
                    &[
                        ("id", "Id"),
                        ("nome", "Rede"),
                        ("subrede", "Sub-rede"),
                        ("porta", "Porta"),
                        ("membros", "Membros"),
                        ("meu_ip", "Meu IP"),
                        ("servidor", "Servidor"),
                    ],
                ))
            }
            "membros" => {
                let id: i64 = arg(0)
                    .and_then(|s| s.parse().ok())
                    .ok_or("MEMBROS <id_da_rede>")?;
                let r = comandos::pedir(
                    &self.painel,
                    "POST",
                    "/api/redes/membros",
                    Some(self.token()?),
                    Some(&Json::objeto(vec![("rede_id", Json::de_i64(id))])),
                )?;
                Ok(lista_para_tabela(
                    &r,
                    &[
                        ("login", "Usuario"),
                        ("ip", "IP"),
                        ("online", "Conectado"),
                        ("entrou_em", "Entrou em"),
                    ],
                ))
            }
            "criarrede" | "entrarrede" => {
                let token = self.token()?.to_string();
                let rede = arg(0).ok_or("informe o nome da rede")?.to_string();
                let senha = (self.perguntar)("senha da rede", "PHXVPN_SENHA_REDE");
                let arq = comandos::perfil_de_rede(
                    &self.painel,
                    &token,
                    cmd == "criarrede",
                    &rede,
                    &senha,
                    o.um("finalidade").unwrap_or(""),
                    o.um("saida"),
                )?;
                Ok(format!(
                    "perfil gravado em {arq} (contem a sua chave privada: guarde-o como senha)\n"
                ))
            }
            "sairrede" => {
                let id: i64 = arg(0)
                    .and_then(|s| s.parse().ok())
                    .ok_or("SAIRREDE <id_da_rede>")?;
                comandos::pedir(
                    &self.painel,
                    "POST",
                    "/api/redes/sair",
                    Some(self.token()?),
                    Some(&Json::objeto(vec![("rede_id", Json::de_i64(id))])),
                )?;
                Ok("voce saiu da rede\n".into())
            }
            "usuarios" => {
                self.token()?;
                Ok(lista_para_tabela(
                    &get(self, "/api/usuarios")?,
                    &[
                        ("login", "Login"),
                        ("email", "E-mail"),
                        ("admin", "Admin"),
                        ("ativo", "Ativo"),
                    ],
                ))
            }
            "usuarionovo" => {
                let token = self.token()?.to_string();
                let login = arg(0).ok_or("USUARIONOVO <login>")?.to_string();
                let senha = (self.perguntar)("senha do usuario novo", "PHXVPN_SENHA_NOVA");
                comandos::pedir(
                    &self.painel,
                    "POST",
                    "/api/usuarios",
                    Some(&token),
                    Some(&Json::objeto(vec![
                        ("login", Json::texto_de(&login)),
                        ("senha", Json::texto_de(senha)),
                        ("email", Json::texto_de(o.um("email").unwrap_or(""))),
                        ("admin", Json::de_bool(o.tem("admin"))),
                    ])),
                )?;
                Ok(format!("usuario {login} incluido\n"))
            }
            "servidores" => {
                self.token()?;
                Ok(lista_para_tabela(
                    &get(self, "/api/servidores")?,
                    &[
                        ("id", "Id"),
                        ("nome", "Servidor"),
                        ("ip", "IP"),
                        ("dns", "DNS"),
                        ("redes", "Redes"),
                    ],
                ))
            }
            "servidornovo" => {
                let token = self.token()?.to_string();
                let nome = arg(0).ok_or("SERVIDORNOVO <nome> /IP:x")?;
                comandos::pedir(
                    &self.painel,
                    "POST",
                    "/api/servidores",
                    Some(&token),
                    Some(&Json::objeto(vec![
                        ("nome", Json::texto_de(nome)),
                        ("ip", Json::texto_de(o.um("ip").ok_or("informe /IP:x")?)),
                        ("dns", Json::texto_de(o.um("dns").unwrap_or(""))),
                    ])),
                )?;
                Ok(format!("servidor {nome} incluido\n"))
            }
            "destrancar" => {
                let token = self.token()?.to_string();
                let m = (self.perguntar)("senha mestre", "PHXVPN_SENHA_MESTRE");
                comandos::pedir(
                    &self.painel,
                    "POST",
                    "/api/destrancar",
                    Some(&token),
                    Some(&Json::objeto(vec![("senha_mestre", Json::texto_de(m))])),
                )?;
                Ok("cofre destrancado\n".into())
            }
            outro => Err(format!(
                "comando desconhecido no modo Painel: {outro} (AJUDA lista)"
            )),
        }
    }

    fn p2p(&mut self, cmd: &str, o: &Opcoes) -> R<String> {
        match cmd {
            "chave" => {
                let k =
                    comandos::identidade(o.um("arquivo").or(o.um("chave")).unwrap_or("p2p.chave"))?;
                Ok(tabela(
                    &["Item", "Valor"],
                    &[vec![
                        "Chave publica".into(),
                        comandos::chave_publica_hex(&k),
                    ]],
                ))
            }
            #[cfg(any(target_os = "linux", windows))]
            "ligar" => {
                if self.no.is_some() {
                    return Err(
                        "ja ligado; para trocar a configuracao, SAIR e abrir de novo".into(),
                    );
                }
                let senha = (self.perguntar)("senha da rede", "PHXVPN_SENHA_REDE");
                let (no, tun, resumo) = comandos::p2p_preparar(o, &senha)?;
                let n2 = std::sync::Arc::clone(&no);
                std::thread::spawn(move || {
                    if let Err(e) = crate::p2p::rodar(n2, tun) {
                        eprintln!("phxvpn: P2P caiu: {e}");
                    }
                });
                self.no = Some(no);
                Ok(format!("{resumo}\n"))
            }
            #[cfg(any(target_os = "linux", windows))]
            "pares" => {
                let no = self
                    .no
                    .as_ref()
                    .ok_or("o P2P nao esta ligado (LIGAR ...)")?;
                let linhas: Vec<Vec<String>> =
                    no.situacao().into_iter().map(|l| l.to_vec()).collect();
                Ok(tabela(
                    &["IP virtual", "Caminho", "Sessao", "Chave"],
                    &linhas,
                ))
            }
            #[cfg(not(any(target_os = "linux", windows)))]
            "ligar" | "pares" => Err("o P2P ainda so roda no Linux".into()),
            outro => Err(format!(
                "comando desconhecido no modo P2P: {outro} (AJUDA lista)"
            )),
        }
    }

    fn ferramentas(&mut self, cmd: &str, o: &Opcoes) -> R<String> {
        match cmd {
            "autoteste" => Ok(autoteste()),
            "bancada" => {
                let s: u64 = o
                    .um("segundos")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1)
                    .clamp(1, 60);
                Ok(bancada(Duration::from_secs(s)))
            }
            "gerarchave" => {
                let k = phxsql_core::x25519::gerar_privada();
                Ok(tabela(
                    &["Item", "Valor"],
                    &[
                        vec!["Privada".into(), phxsql_core::hash::para_hex(&k)],
                        vec!["Publica".into(), comandos::chave_publica_hex(&k)],
                    ],
                ))
            }
            outro => Err(format!(
                "comando desconhecido no modo Ferramentas: {outro} (AJUDA lista)"
            )),
        }
    }
}

/// Roda as provas embarcadas: cada uma contra vetor oficial ou ida-e-volta.
pub fn autoteste() -> String {
    let provas: Vec<(&str, Result<(), String>)> = vec![
        ("Noise IKpsk2 (vetor cacophony)", crate::noise::autoteste()),
        ("SCRAM-SHA-256 (RFC 7677)", crate::pg::autoteste()),
        ("Cofre XChaCha20-Poly1305", {
            let c = crate::cofre::Cofre::novo("autoteste-senha", 1_000);
            let s = c.selar(b"ok", "autoteste");
            match c.abrir_com(&s, "autoteste") {
                Ok(v) if v == b"ok" => match c.abrir_com(&s, "outro") {
                    Err(_) => Ok(()),
                    Ok(_) => Err("selo abriu com o uso errado".into()),
                },
                _ => Err("selo nao abriu".into()),
            }
        }),
        ("Certificado Ed25519 (AC emite folha)", {
            use crate::pki::{emitir, Ac, Papel};
            emitir(Papel::Ac, "Autoteste", "AC", 1, None).and_then(|ac| {
                emitir(
                    Papel::Membro,
                    "Autoteste",
                    "m",
                    1,
                    Some(&Ac {
                        organizacao: "Autoteste",
                        cn: "AC",
                        privada: &ac.privada,
                    }),
                )
                .map(|_| ())
            })
        }),
    ];
    let falhas = provas.iter().filter(|(_, r)| r.is_err()).count();
    let linhas: Vec<Vec<String>> = provas
        .into_iter()
        .map(|(n, r)| {
            vec![
                n.to_string(),
                match r {
                    Ok(()) => "OK".into(),
                    Err(e) => format!("FALHOU: {e}"),
                },
            ]
        })
        .collect();
    let mut s = tabela(&["Prova", "Resultado"], &linhas);
    s += if falhas == 0 {
        "autoteste: tudo confere\n"
    } else {
        "autoteste: HA FALHA -- este binario nao e confiavel\n"
    };
    s
}

/// Mede, nesta maquina, a cifra de um pacote de 1.420 B e o aperto IKpsk2.
pub fn bancada(duracao: Duration) -> String {
    use phxsql_core::cifra::selar;
    use phxsql_core::fio::nonce_do_contador;
    use phxsql_core::x25519;
    let chave = [7u8; 32];
    let pacote = vec![0u8; 1420];
    let (mut n, t) = (0u64, Instant::now());
    while t.elapsed() < duracao {
        let _ = selar(&chave, &nonce_do_contador(n), &[], &pacote);
        n += 1;
    }
    let seg = t.elapsed().as_secs_f64();
    let mbit = (n as f64 * 1420.0 * 8.0) / seg / 1e6;
    let (a, b) = (x25519::gerar_privada(), x25519::gerar_privada());
    let pb = x25519::chave_publica(&b);
    let (mut apertos, t) = (0u64, Instant::now());
    while t.elapsed() < duracao {
        if let Ok((ini, m1)) =
            crate::noise::Iniciador::comecar(crate::noise::PROLOGO, a, &pb, [1; 32], b"")
        {
            if let Ok((_, m2)) = crate::noise::ler_chamada(crate::noise::PROLOGO, &b, &m1)
                .and_then(|c| c.responder([1; 32], b""))
            {
                let _ = ini.terminar(&m2);
                apertos += 1;
            }
        }
    }
    let seg2 = t.elapsed().as_secs_f64();
    tabela(
        &["Medida", "Valor"],
        &[
            vec![
                "Cifra (1 nucleo, pacote 1.420 B)".into(),
                format!("{mbit:.0} Mbit/s"),
            ],
            vec!["Pacotes cifrados".into(), format!("{n} em {seg:.1} s")],
            vec![
                "Aperto IKpsk2 completo (os dois lados)".into(),
                format!("{:.2} ms cada", seg2 * 1000.0 / apertos.max(1) as f64),
            ],
        ],
    ) + "So a cifra e o aperto: sem rede nem placa. Vazao real: ver PHXVPN.md.\n"
}

/// Ponto de entrada do `phxvpncmd` (e do `phxvpn cmd`).
pub fn principal(args: &[String]) -> R<()> {
    let o = Opcoes::de_dos(
        &args
            .iter()
            .map(|a| a.replacen("--", "/", 1))
            .collect::<Vec<_>>(),
    );
    let mut modo = o.um("modo").and_then(Modo::de_texto);
    let interativo = o.um("comando").is_none() && o.um("entrada").is_none();
    if modo.is_none() && interativo {
        println!(
            "phxvpncmd {} -- console do phxvpn\n",
            env!("CARGO_PKG_VERSION")
        );
        println!("  1. Painel       gerir o servidor (redes, usuarios, servidores)");
        println!("  2. P2P          este computador (chave, ligar, pares)");
        println!("  3. Ferramentas  autoteste, bancada, chave\n");
        loop {
            print!("Escolha o modo [1-3]: ");
            std::io::stdout().flush().ok();
            let mut l = String::new();
            if std::io::stdin()
                .lock()
                .read_line(&mut l)
                .map_err(|e| e.to_string())?
                == 0
            {
                return Ok(());
            }
            if let Some(m) = Modo::de_texto(l.trim()) {
                modo = Some(m);
                break;
            }
        }
    }
    let mut c = Console::novo(
        modo.unwrap_or(Modo::Ferramentas),
        Box::new(perguntar_no_terminal),
    );
    if let Some(p) = o.um("painel") {
        c.executar(&format!("conectar {p}"))?;
    }
    // Uma linha so: `/comando:"REDES"`. Codigo de saida diz se deu certo.
    if let Some(linha) = o.um("comando") {
        return match c.executar(linha)? {
            Fim::Continua(s) => {
                print!("{s}");
                Ok(())
            }
            Fim::Sair => Ok(()),
        };
    }
    // Lote: um comando por linha; `#` e `REM` sao comentario, como no .bat.
    // Para no primeiro erro -- lote que segue depois de falha faz estrago.
    if let Some(arq) = o.um("entrada") {
        let texto = std::fs::read_to_string(arq).map_err(|e| format!("{arq}: {e}"))?;
        for (n, linha) in texto.lines().enumerate() {
            let t = linha.trim();
            if t.is_empty() || t.starts_with('#') || t.to_lowercase().starts_with("rem ") {
                continue;
            }
            println!("{} {t}", c.prompt());
            match c.executar(t) {
                Ok(Fim::Continua(s)) => print!("{s}"),
                Ok(Fim::Sair) => return Ok(()),
                Err(e) => return Err(format!("{arq}:{}: {e}", n + 1)),
            }
        }
        return Ok(());
    }
    println!("Modo {}. AJUDA lista os comandos.\n", c.modo.nome());
    let entrada = std::io::stdin();
    loop {
        print!("{} ", c.prompt());
        std::io::stdout().flush().ok();
        let mut l = String::new();
        if entrada
            .lock()
            .read_line(&mut l)
            .map_err(|e| e.to_string())?
            == 0
        {
            println!();
            return Ok(());
        }
        match c.executar(l.trim()) {
            Ok(Fim::Continua(s)) => print!("{s}"),
            Ok(Fim::Sair) => return Ok(()),
            Err(e) => println!("Erro: {e}"),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn console() -> Console<'static> {
        Console::novo(Modo::Ferramentas, Box::new(|_, _| "x".to_string()))
    }

    #[test]
    fn modos_trocam_e_o_prompt_diz_onde_esta() {
        let mut c = console();
        assert_eq!(c.prompt(), "phxvpn Ferramentas>");
        c.executar("MODO 2").unwrap();
        assert_eq!(c.prompt(), "phxvpn P2P>");
        c.executar("modo painel").unwrap();
        assert_eq!(c.prompt(), "phxvpn Painel>");
        assert!(c.executar("modo 9").is_err());
        assert_eq!(c.executar("SAIR").unwrap(), Fim::Sair);
    }

    #[test]
    fn autoteste_passa_e_diz_quais_provas() {
        let mut c = console();
        let Fim::Continua(s) = c.executar("AUTOTESTE").unwrap() else {
            panic!()
        };
        let linha = s.lines().find(|l| l.contains("Noise IKpsk2")).unwrap();
        assert!(linha.contains("| OK"), "{s}");
        assert!(s.contains("tudo confere"), "{s}");
    }

    #[test]
    fn painel_sem_login_pede_login() {
        let mut c = console();
        c.executar("modo 1").unwrap();
        assert!(c.executar("REDES").unwrap_err().contains("LOGIN"));
        assert!(c
            .executar("naoexiste")
            .unwrap_err()
            .contains("desconhecido"));
    }

    #[test]
    fn tabela_alinha_colunas() {
        let t = tabela(&["A", "Bbb"], &[vec!["xx".into(), "y".into()]]);
        assert_eq!(
            t,
            "+----+-----+\n| A  | Bbb |\n+----+-----+\n| xx | y   |\n+----+-----+\n1 linha(s)\n"
        );
    }
}
