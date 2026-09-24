//! O programa de mesa do phxvpn: a janela no estilo do Radmin.
//!
//! # Desenho (decisao do papel J, 24/09/2026)
//!
//! Hipoteses, escritas antes de escolher:
//! * **Janela nativa Win32 por FFI** -- so Windows, e listas, dialogos e
//!   menus em Win32 cru sao milhares de linhas. Morreu pelo custo e por deixar
//!   o Linux sem tela.
//! * **GTK no Linux** -- biblioteca externa ligada ao binario: choca com a
//!   petrea de zero dependencia. Morreu.
//! * **Interface local no modo aplicativo do navegador** -- o phxvpn serve a
//!   tela so em 127.0.0.1, numa porta sorteada, e a abre como JANELA de
//!   aplicativo (`--app=`) do Edge, que todo Windows 10/11 tem, ou do Chromium
//!   no Linux; sem Edge/Chromium, o navegador padrao. Uma tela so para os dois
//!   sistemas, zero dependencia, e o mesmo motor da linha de comando. VENCEU.
//!
//! # A guarda da tela local
//!
//! Loopback nao basta: qualquer programa e qualquer site aberto nesta maquina
//! alcancam 127.0.0.1. Entao, alem das guardas do `web.rs` (Host contra DNS
//! rebinding, POST so com JSON contra CSRF), toda rota da API pede a FICHA da
//! sessao -- 32 bytes sorteados a cada abertura, que vao para a janela no
//! FRAGMENTO da URL (`#f=...`). Fragmento nao vai ao servidor em pedido
//! nenhum nem vaza no Referer; a pagina le e manda no cabecalho.
//!
//! # Um motor so
//!
//! Criar, convidar, entrar e ligar chamam `comandos.rs` -- o MESMO codigo de
//! `phxvpn p2p ...` e do `phxvpncmd`.

use crate::comandos::{self, Opcoes};
use crate::p2p::No;
use crate::rede_p2p::Rede;
use crate::web::{self, Pedido, Resposta};
use phxsql_core::hash::{iguais_em_tempo_constante, para_hex};
use phxsql_core::json::Json;
use phxsql_core::senha::bytes_aleatorios;
use std::collections::HashMap;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub type R<T> = Result<T, String>;

const TELA: &str = include_str!("mesa.html");
const TELA_JS: &str = include_str!("mesa.js");

/// Uma rede ligada nesta janela.
struct Ligada {
    no: Arc<No>,
    fio: std::thread::JoinHandle<Result<(), String>>,
}

trait Terminou {
    fn is_none_or_terminou(&self) -> bool;
}

impl Terminou for Option<&Ligada> {
    fn is_none_or_terminou(&self) -> bool {
        self.map_or(true, |l| l.fio.is_finished())
    }
}

pub struct Mesa {
    pasta: PathBuf,
    ficha: String,
    ligadas: Mutex<HashMap<String, Ligada>>,
    /// O kernel do USB (`Sysfs::do_ambiente`).
    sysfs: crate::usb::Sysfs,
}

fn opcoes(pares: &[(&str, String)]) -> Opcoes {
    let mut palavras = Vec::new();
    for (k, v) in pares {
        if !v.is_empty() {
            palavras.push(format!("/{k}:{v}"));
        }
    }
    Opcoes::de_dos(&palavras)
}

impl Mesa {
    pub fn nova(pasta: PathBuf) -> R<Mesa> {
        std::fs::create_dir_all(&pasta).map_err(|e| format!("{}: {e}", pasta.display()))?;
        // A pasta guarda a chave P2P e as senhas lembradas: so do dono.
        crate::acl::so_do_dono(&pasta)?;
        Ok(Mesa {
            pasta,
            ficha: para_hex(&bytes_aleatorios(32)),
            ligadas: Mutex::new(HashMap::new()),
            sysfs: crate::usb::Sysfs::do_ambiente(),
        })
    }

    pub fn ficha(&self) -> &str {
        &self.ficha
    }

    fn caminho(&self, arquivo: &str) -> String {
        self.pasta.join(arquivo).to_string_lossy().into_owned()
    }

    fn chave(&self) -> String {
        self.caminho("p2p.chave")
    }

    fn arquivo_da_rede(&self, nome: &str) -> String {
        self.caminho(&Rede::caminho(nome))
    }

    /// USB pela janela: as mesmas operacoes do `phxvpn usb` (o motor e o
    /// `usb.rs`), com resposta estruturada para a tela desenhar.
    pub fn usb(&self, corpo: &Json) -> R<Json> {
        use crate::usb;
        let t = |c: &str| corpo.texto_ou(c, "").to_string();
        let sysfs = self.sysfs.clone();
        let ok = |m: String| Json::objeto(vec![("ok", Json::texto_de(m))]);
        let ip = || -> R<std::net::Ipv4Addr> {
            t("ip")
                .parse()
                .map_err(|_| "IP do membro invalido".to_string())
        };
        let dispositivo = |d: &usb::Dispositivo| {
            vec![
                ("busid", Json::texto_de(&d.busid)),
                ("resumo", Json::texto_de(d.resumo())),
            ]
        };
        match t("acao").as_str() {
            "locais" => {
                let rede = Rede::ler(&self.arquivo_da_rede(&t("rede"))).ok();
                Ok(Json::Lista(
                    usb::locais_da_rede(&sysfs, rede.as_ref())?
                        .iter()
                        .map(|l| {
                            let mut c = dispositivo(&l.dispositivo);
                            c.push(("preso", Json::de_bool(l.preso)));
                            c.push(("nesta_rede", Json::de_bool(l.nesta_rede)));
                            Json::objeto(c)
                        })
                        .collect(),
                ))
            }
            "compartilhar" => {
                usb::compartilhar_na_rede(&sysfs, &self.arquivo_da_rede(&t("rede")), &t("busid"))
                    .map(ok)
            }
            "parar" => {
                usb::parar_na_rede(&sysfs, &self.arquivo_da_rede(&t("rede")), &t("busid")).map(ok)
            }
            "remotos" => Ok(Json::Lista(
                usb::remotos(ip()?)?
                    .iter()
                    .map(|d| Json::objeto(dispositivo(d)))
                    .collect(),
            )),
            "usar" => usb::usar(&sysfs, ip()?, &t("busid")).map(ok),
            // Sem o controlador virtual nao ha nada em uso: a tela diz como
            // usar, sem tratar como falha (achado ao exercitar: aparecia o
            // erro cru do lado de quem so compartilha).
            #[cfg(unix)]
            "portas" if !sysfs.tem_vhci() => {
                Ok(Json::objeto(vec![("sem_vhci", Json::de_bool(true))]))
            }
            "portas" => Ok(Json::Lista(
                usb::em_uso(&sysfs)?
                    .into_iter()
                    .map(|u| {
                        Json::objeto(vec![
                            ("porta", Json::de_i64(u.porta as i64)),
                            ("texto", Json::texto_de(u.texto)),
                        ])
                    })
                    .collect(),
            )),
            "soltar" => usb::soltar(&sysfs, corpo.inteiro_ou("porta", -1) as u32).map(ok),
            outra => Err(format!("acao de USB desconhecida: {outra}")),
        }
    }

    /// As redes desta pasta, com o estado de cada membro.
    pub fn redes(&self) -> R<Json> {
        let mut ligadas = self.ligadas.lock().unwrap_or_else(|e| e.into_inner());
        // Rede cujo laco terminou (erro de rede, placa perdida) sai da lista.
        ligadas.retain(|_, l| !l.fio.is_finished());
        let mut nomes: Vec<(String, Rede)> = Vec::new();
        for e in std::fs::read_dir(&self.pasta)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("p2p") {
                if let Ok(r) = Rede::ler(&p.to_string_lossy()) {
                    nomes.push((r.nome.clone(), r));
                }
            }
        }
        nomes.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        let lista = nomes
            .into_iter()
            .map(|(nome, r)| {
                let ligada = ligadas.get(&nome);
                let membros: Vec<Json> = match ligada {
                    Some(l) => {
                        l.no.situacao()
                            .into_iter()
                            .map(|[ip, caminho, sessao, chave]| {
                                Json::objeto(vec![
                                    ("ip", Json::texto_de(ip)),
                                    ("caminho", Json::texto_de(caminho)),
                                    ("sessao", Json::texto_de(sessao.clone())),
                                    ("online", Json::de_bool(sessao.ends_with(" s"))),
                                    ("chave", Json::texto_de(chave)),
                                ])
                            })
                            .collect()
                    }
                    None => r
                        .pares
                        .iter()
                        .map(|p| {
                            Json::objeto(vec![
                                ("ip", Json::texto_de(p.ip.to_string())),
                                ("caminho", Json::texto_de("-")),
                                ("sessao", Json::texto_de("rede desligada")),
                                ("online", Json::de_bool(false)),
                                ("chave", Json::texto_de(para_hex(&p.chave[..6]))),
                            ])
                        })
                        .collect(),
                };
                Json::objeto(vec![
                    ("rede", Json::texto_de(nome.clone())),
                    ("ip", Json::texto_de(format!("{}/{}", r.ip, r.prefixo))),
                    ("modo", Json::texto_de(r.modo.clone())),
                    (
                        "repasse_usuario",
                        Json::texto_de(r.repasse_usuario.clone().unwrap_or_default()),
                    ),
                    ("ligada", Json::de_bool(ligada.is_some())),
                    (
                        "desligando",
                        Json::de_bool(ligada.is_some_and(|l| l.no.desligado())),
                    ),
                    (
                        "lembrada",
                        Json::de_bool(crate::lembrar::existe(&self.pasta, &nome)),
                    ),
                    ("membros", Json::Lista(membros)),
                ])
            })
            .collect();
        Ok(Json::Lista(lista))
    }

    /// As frases de volta sao da JANELA, nao da linha de comando: «convide
    /// com p2p convidar» e o caminho do arquivo no rodape eram texto de
    /// terminal numa tela (achado ao exercitar a janela).
    pub fn criar(
        &self,
        rede: &str,
        ip: &str,
        modo: &str,
        repasse: &str,
        repasse_usuario: &str,
    ) -> R<String> {
        comandos::p2p_criar(&opcoes(&[
            ("rede", rede.into()),
            (
                "ip",
                if ip.is_empty() {
                    "10.78.0.1/24".into()
                } else {
                    ip.into()
                },
            ),
            (
                "modo",
                if modo.is_empty() {
                    "direto".into()
                } else {
                    modo.into()
                },
            ),
            ("repasse", repasse.into()),
            ("repasse-usuario", repasse_usuario.into()),
            ("arquivo", self.arquivo_da_rede(rede)),
            ("chave", self.chave()),
        ]))
        .map(|_| format!("Rede {rede} criada. Use Convidar para chamar os outros."))
    }

    pub fn entrar(&self, codigo: &str, senha: &str) -> R<String> {
        let nome = crate::rede_p2p::rede_do_convite(codigo)?;
        comandos::p2p_entrar(
            codigo,
            senha,
            &opcoes(&[
                ("arquivo", self.arquivo_da_rede(&nome)),
                ("chave", self.chave()),
            ]),
        )
        .map(|_| format!("Você entrou na rede {nome}. Clique em Ligar para conectar."))
    }

    pub fn convidar(&self, rede: &str, senha: &str, endereco: &str, validade: &str) -> R<String> {
        comandos::p2p_convidar(
            &opcoes(&[
                ("rede", rede.into()),
                ("arquivo", self.arquivo_da_rede(rede)),
                ("chave", self.chave()),
                ("endereco", endereco.into()),
                ("validade", validade.into()),
            ]),
            senha,
        )
    }

    /// Liga a rede. Senha vazia com segredo lembrado usa o lembrado; com
    /// `lembrar`, guarda o segredo DERIVADO (nunca a senha) depois de ligar.
    #[cfg(any(target_os = "linux", windows))]
    pub fn ligar(
        &self,
        rede: &str,
        senha: &str,
        repasse_usuario: &str,
        repasse_senha: &str,
        lembrar: bool,
    ) -> R<String> {
        use crate::comandos::{Segredo, SegredoRepasse};
        use crate::lembrar::{self, Lembrado};
        let mut ligadas = self.ligadas.lock().unwrap_or_else(|e| e.into_inner());
        ligadas.retain(|_, l| !l.fio.is_finished());
        if let Some(l) = ligadas.get(rede) {
            return Err(if l.no.desligado() {
                format!("a rede {rede} ainda está desligando; tente em um instante")
            } else {
                format!("a rede {rede} já está ligada")
            });
        }
        let arquivo = self.arquivo_da_rede(rede);
        let r = Rede::ler(&arquivo)?;
        let guardado = lembrar::ler(&self.pasta, rede);
        let usuario = if repasse_usuario.is_empty() {
            guardado
                .as_ref()
                .and_then(|g| g.repasse.as_ref().map(|(u, _)| u.clone()))
                .or(r.repasse_usuario.clone())
                .unwrap_or_default()
        } else {
            repasse_usuario.to_string()
        };
        let psk = match (senha.is_empty(), &guardado) {
            (false, _) => crate::p2p::psk_da_rede(&r.nome, senha, crate::p2p::ITERACOES_PSK),
            (true, Some(g)) => g.psk,
            (true, None) => return Err("informe a senha da rede".into()),
        };
        let credencial = match (repasse_senha.is_empty(), &guardado, usuario.is_empty()) {
            (_, _, true) => None,
            (false, _, false) => Some(crate::repasse::credencial(
                &usuario,
                repasse_senha,
                crate::repasse::ITERACOES_CONTA,
            )),
            (true, Some(g), false) => g
                .repasse
                .as_ref()
                .filter(|(u, _)| *u == usuario)
                .map(|(_, c)| *c),
            (true, None, false) => None,
        };
        // Cada rede ligada ao mesmo tempo precisa de porta e placa proprias.
        let n = ligadas.len();
        let o = opcoes(&[
            ("rede", rede.into()),
            ("arquivo", arquivo),
            ("chave", self.chave()),
            ("repasse-usuario", usuario.clone()),
            (
                "interface",
                if cfg!(windows) {
                    "phxvpn".into()
                } else {
                    format!("phx{n}")
                },
            ),
        ]);
        let (no, tun, _) = comandos::p2p_preparar(
            &o,
            Segredo::Psk(psk),
            credencial.map(SegredoRepasse::Credencial),
        )?;
        let resumo = format!("Rede {rede} ligada: seu IP é {}.", no.ip());
        if lembrar {
            lembrar::guardar(
                &self.pasta,
                rede,
                &Lembrado {
                    psk,
                    repasse: credencial.map(|c| (usuario, c)),
                },
            )?;
        }
        let n2 = Arc::clone(&no);
        let fio = std::thread::spawn(move || crate::p2p::rodar(n2, tun));
        ligadas.insert(rede.to_string(), Ligada { no, fio });
        Ok(resumo)
    }

    #[cfg(not(any(target_os = "linux", windows)))]
    pub fn ligar(&self, _rede: &str, _senha: &str, _u: &str, _s: &str, _l: bool) -> R<String> {
        Err("o P2P roda no Linux e no Windows".into())
    }

    /// Desliga e ESPERA o no terminar. A rede so sai da lista DEPOIS: antes,
    /// ela sumia na hora e a janela mostrava «desligada» com a porta UDP ainda
    /// presa -- religar dava «Address already in use» (achado ao exercitar
    /// com um par conectado; sem par o no para rapido e a corrida nao
    /// aparecia). Enquanto isso, a rede aparece como «desligando».
    pub fn desligar(&self, rede: &str) -> R<String> {
        let no = self
            .ligadas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(rede)
            .map(|l| Arc::clone(&l.no))
            .ok_or_else(|| format!("a rede {rede} não está ligada"))?;
        no.desligar();
        drop(no);
        let inicio = std::time::Instant::now();
        loop {
            let terminou = self
                .ligadas
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(rede)
                .is_none_or_terminou();
            if terminou {
                break;
            }
            if inicio.elapsed() > std::time::Duration::from_secs(10) {
                return Err(format!("a rede {rede} nao terminou de desligar em 10 s"));
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if let Some(l) = self
            .ligadas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(rede)
        {
            let _ = l.fio.join();
        }
        Ok(format!(
            "rede {rede} desligada ({} ms)",
            inicio.elapsed().as_millis()
        ))
    }

    fn no_ligado(&self, rede: &str) -> R<Arc<No>> {
        self.ligadas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(rede)
            .map(|l| Arc::clone(&l.no))
            .ok_or_else(|| format!("a rede {rede} não está ligada"))
    }

    /// Ping PELO TUNEL ate o membro (eco de controle, sem ICMP).
    pub fn pingar(&self, rede: &str, ip: &str) -> R<String> {
        let ip = ip.parse().map_err(|_| "IP invalido")?;
        let t = self
            .no_ligado(rede)?
            .pingar(ip, std::time::Duration::from_secs(3))?;
        Ok(format!(
            "{ip} respondeu em {:.1} ms",
            t.as_secs_f64() * 1000.0
        ))
    }

    pub fn conversar(&self, rede: &str, ip: &str, texto: &str) -> R<String> {
        let ip = ip.parse().map_err(|_| "IP invalido")?;
        self.no_ligado(rede)?.conversar(ip, texto)?;
        Ok("enviada".into())
    }

    pub fn mensagens(&self, rede: &str, desde: u64) -> R<Json> {
        let no = match self.no_ligado(rede) {
            Ok(n) => n,
            Err(_) => return Ok(Json::Lista(vec![])),
        };
        Ok(Json::Lista(
            no.mensagens(desde)
                .into_iter()
                .map(|m| {
                    Json::objeto(vec![
                        ("numero", Json::de_u64(m.numero)),
                        ("ip", Json::texto_de(m.ip.to_string())),
                        ("minha", Json::de_bool(m.minha)),
                        ("texto", Json::texto_de(m.texto)),
                        ("quando", Json::de_u64(m.quando)),
                    ])
                })
                .collect(),
        ))
    }

    pub fn esquecer(&self, rede: &str) -> R<String> {
        crate::lembrar::esquecer(&self.pasta, rede)?;
        Ok(format!("Senha da rede {rede} esquecida neste computador."))
    }

    pub fn minha_chave(&self) -> R<String> {
        Ok(comandos::chave_publica_hex(&comandos::identidade(
            &self.chave(),
        )?))
    }

    /// As rotas. Toda a API pede a ficha; a tela e o script, nao (sao
    /// estaticos e nao carregam dado).
    pub fn tratar(&self, p: &Pedido) -> Resposta {
        let caminho = p.caminho.split('?').next().unwrap_or_default();
        match (p.metodo.as_str(), caminho) {
            ("GET", "/") => return Resposta::html(TELA),
            ("GET", "/mesa.js") => return Resposta::js(TELA_JS),
            _ => {}
        }
        let ficha_ok = p
            .token
            .as_deref()
            .is_some_and(|t| iguais_em_tempo_constante(t.as_bytes(), self.ficha.as_bytes()));
        if !ficha_ok {
            return Resposta::erro(
                401,
                "ficha da sessão ausente ou errada: abra pela janela do phxvpn",
            );
        }
        let corpo = if p.corpo.trim().is_empty() {
            Json::Objeto(vec![])
        } else {
            match Json::analisar(&p.corpo) {
                Ok(j) => j,
                Err(e) => return Resposta::erro(400, &format!("JSON invalido: {e}")),
            }
        };
        let t = |c: &str| corpo.texto_ou(c, "").to_string();
        let texto = |r: R<String>| match r {
            Ok(s) => Resposta::json(
                200,
                Json::objeto(vec![("ok", Json::texto_de(s))]).escrever(),
            ),
            Err(e) => Resposta::erro(400, &e),
        };
        match (p.metodo.as_str(), caminho) {
            ("GET", "/api/redes") => match self.redes() {
                Ok(j) => Resposta::json(200, j.escrever()),
                Err(e) => Resposta::erro(500, &e),
            },
            ("GET", "/api/chave") => texto(self.minha_chave()),
            ("POST", "/api/criar") => texto(self.criar(
                &t("rede"),
                &t("ip"),
                &t("modo"),
                &t("repasse"),
                &t("repasse_usuario"),
            )),
            ("POST", "/api/entrar") => texto(self.entrar(&t("codigo"), &t("senha"))),
            ("POST", "/api/convidar") => {
                texto(self.convidar(&t("rede"), &t("senha"), &t("endereco"), &t("validade")))
            }
            ("POST", "/api/ligar") => texto(self.ligar(
                &t("rede"),
                &t("senha"),
                &t("repasse_usuario"),
                &t("repasse_senha"),
                corpo.booleano_ou("lembrar", false),
            )),
            ("POST", "/api/ping") => texto(self.pingar(&t("rede"), &t("ip"))),
            ("POST", "/api/chat") => texto(self.conversar(&t("rede"), &t("ip"), &t("texto"))),
            ("POST", "/api/mensagens") => {
                match self.mensagens(&t("rede"), corpo.inteiro_ou("desde", 0) as u64) {
                    Ok(j) => Resposta::json(200, j.escrever()),
                    Err(e) => Resposta::erro(400, &e),
                }
            }
            ("POST", "/api/esquecer") => texto(self.esquecer(&t("rede"))),
            ("POST", "/api/usb") => match self.usb(&corpo) {
                Ok(j) => Resposta::json(200, j.escrever()),
                Err(e) => Resposta::erro(400, &e),
            },
            ("GET", "/api/sistema") => Resposta::json(
                200,
                Json::objeto(vec![
                    ("inicia", Json::de_bool(crate::iniciar::ligado().is_some())),
                    ("bandeja", Json::de_bool(cfg!(windows))),
                ])
                .escrever(),
            ),
            ("POST", "/api/sistema") => texto(if corpo.booleano_ou("inicia", false) {
                crate::iniciar::ligar()
                    .map(|_| "O phxvpn vai abrir junto com o sistema.".to_string())
            } else {
                crate::iniciar::desligar()
                    .map(|_| "O phxvpn não abre mais com o sistema.".to_string())
            }),
            ("POST", "/api/desligar") => texto(self.desligar(&t("rede"))),
            _ => Resposta::erro(404, "rota desconhecida"),
        }
    }
}

/// Abre a URL como JANELA de aplicativo: Edge no Windows, Chromium/Chrome no
/// Linux; sem eles, o navegador padrao. Devolve o que foi usado.
pub fn abrir_janela(url: &str) -> String {
    use std::process::{Command, Stdio};
    let app = format!("--app={url}");
    let tamanho = "--window-size=480,760";
    let mut candidatos: Vec<std::path::PathBuf> = Vec::new();
    if cfg!(windows) {
        for var in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Ok(b) = std::env::var(var) {
                candidatos
                    .push(std::path::Path::new(&b).join(r"Microsoft\Edge\Application\msedge.exe"));
            }
        }
    } else {
        for n in [
            "chromium",
            "chromium-browser",
            "google-chrome",
            "microsoft-edge",
        ] {
            if let Some(c) = crate::supervisor::achar_no_path(n) {
                candidatos.push(c);
            }
        }
    }
    for c in candidatos.into_iter().filter(|c| c.is_file()) {
        let ok = Command::new(&c)
            .args([&app, tamanho])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok();
        if ok {
            return format!("janela: {}", c.display());
        }
    }
    let padrao = if cfg!(windows) {
        // `start` do cmd: o titulo vazio e obrigatorio antes da URL.
        Command::new("cmd").args(["/c", "start", "", url]).spawn()
    } else {
        Command::new("xdg-open").arg(url).spawn()
    };
    match padrao {
        Ok(_) => "navegador padrao".into(),
        Err(_) => "nenhum navegador achado: abra o endereco acima".into(),
    }
}

/// Onde o programa de mesa guarda redes e identidade: `%APPDATA%\phxvpn` no
/// Windows, `$XDG_CONFIG_HOME/phxvpn` (ou `~/.config/phxvpn`) no Linux.
pub fn pasta_padrao() -> PathBuf {
    if cfg!(windows) {
        if let Ok(a) = std::env::var("APPDATA") {
            return PathBuf::from(a).join("phxvpn");
        }
    }
    if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(x).join("phxvpn");
    }
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".config").join("phxvpn"))
        .unwrap_or_else(|_| PathBuf::from("phxvpn"))
}

/// A tela local pronta para atender: a URL ja existe (a bandeja precisa
/// dela), o laco de pedidos roda quando `servir` for chamado.
pub struct Preparada {
    mesa: Arc<Mesa>,
    ouvinte: TcpListener,
    escuta: String,
}

impl Preparada {
    pub fn url(&self) -> String {
        format!("http://{}/#f={}", self.escuta, self.mesa.ficha())
    }

    pub fn servir(self) -> R<()> {
        let hosts = web::hosts_de(&self.escuta, &[]);
        let m = self.mesa;
        web::servir(self.ouvinte, hosts, 64, move |p| m.tratar(p))
    }
}

pub fn preparar(pasta: PathBuf, porta: u16) -> R<Preparada> {
    let mesa = Arc::new(Mesa::nova(pasta)?);
    let ouvinte =
        TcpListener::bind(("127.0.0.1", porta)).map_err(|e| format!("127.0.0.1:{porta}: {e}"))?;
    let escuta = ouvinte.local_addr().map_err(|e| e.to_string())?.to_string();
    Ok(Preparada {
        mesa,
        ouvinte,
        escuta,
    })
}

/// `phxvpn mesa`: sobe a tela local e abre a janela.
pub fn principal(pasta: PathBuf, porta: u16, abrir: bool) -> R<()> {
    let p = preparar(pasta, porta)?;
    let url = p.url();
    eprintln!("phxvpn: janela em {url}");
    eprintln!("phxvpn: (o endereco carrega a ficha da sessao: nao o compartilhe)");
    if abrir {
        eprintln!("phxvpn: {}", abrir_janela(&url));
    }
    p.servir()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pedido(metodo: &str, caminho: &str, ficha: Option<&str>, corpo: &str) -> Pedido {
        Pedido {
            metodo: metodo.into(),
            caminho: caminho.into(),
            token: ficha.map(str::to_string),
            host: None,
            tipo: Some("application/json".into()),
            ip: "127.0.0.1".parse().unwrap(),
            corpo: corpo.into(),
        }
    }

    fn pasta(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("phxvpn-mesa-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn sem_ficha_a_api_nao_responde_e_a_tela_sim() {
        let m = Mesa::nova(pasta("f")).unwrap();
        assert_eq!(m.tratar(&pedido("GET", "/api/redes", None, "")).status, 401);
        assert_eq!(
            m.tratar(&pedido("GET", "/api/redes", Some("errada"), ""))
                .status,
            401
        );
        assert_eq!(m.tratar(&pedido("GET", "/", None, "")).status, 200);
        let f = m.ficha().to_string();
        assert_eq!(
            m.tratar(&pedido("GET", "/api/redes", Some(&f), "")).status,
            200
        );
    }

    /// O caminho inteiro por duas janelas em pastas diferentes: A cria e
    /// convida, B entra pelo codigo -- o mesmo motor da linha de comando.
    #[test]
    fn criar_convidar_entrar_pela_api() {
        let (pa, pb) = (pasta("a"), pasta("b"));
        let a = Mesa::nova(pa.clone()).unwrap();
        let b = Mesa::nova(pb.clone()).unwrap();
        let (fa, fb) = (a.ficha().to_string(), b.ficha().to_string());
        let r = a.tratar(&pedido(
            "POST",
            "/api/criar",
            Some(&fa),
            r#"{"rede":"Casa","ip":"10.78.9.1/24"}"#,
        ));
        assert_eq!(r.status, 200, "{}", r.corpo);
        let r = a.tratar(&pedido(
            "POST",
            "/api/convidar",
            Some(&fa),
            r#"{"rede":"Casa","senha":"s-casa-1"}"#,
        ));
        assert_eq!(r.status, 200, "{}", r.corpo);
        let codigo = Json::analisar(&r.corpo)
            .unwrap()
            .texto_ou("ok", "")
            .to_string();
        assert!(codigo.starts_with("phxvpn1."));
        let errada = format!(r#"{{"codigo":"{codigo}","senha":"outra"}}"#);
        assert_eq!(
            b.tratar(&pedido("POST", "/api/entrar", Some(&fb), &errada))
                .status,
            400
        );
        let certa = format!(r#"{{"codigo":"{codigo}","senha":"s-casa-1"}}"#);
        let r = b.tratar(&pedido("POST", "/api/entrar", Some(&fb), &certa));
        assert_eq!(r.status, 200, "{}", r.corpo);
        let redes = b.tratar(&pedido("GET", "/api/redes", Some(&fb), ""));
        assert!(
            redes.corpo.contains("\"rede\":\"Casa\"") && redes.corpo.contains("10.78.9.2/24"),
            "{}",
            redes.corpo
        );
        assert!(redes.corpo.contains("\"ligada\":false"));
        let _ = std::fs::remove_dir_all(pa);
        let _ = std::fs::remove_dir_all(pb);
    }
}
