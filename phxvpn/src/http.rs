//! O painel pela rede: um servidor HTTP minimo (so `std`) com a API JSON e a
//! tela embutida.
//!
//! # As guardas
//!
//! As de transporte (teto de conexoes, prazo total, tetos de tamanho, `Host`
//! contra DNS rebinding, POST so com JSON contra CSRF) moram em `web.rs` --
//! as MESMAS do programa de mesa. As deste arquivo sao as do painel:
//!
//! 6. **Instalacao so com o codigo de uso unico** impresso no terminal do
//!    painel: quem so alcanca a porta nao instala com admin e senha dele (A5).
//! 7. **Tentativas de senha limitadas** por login, por IP e por rede, e a
//!    conta cara (PBKDF2) FORA da trava do painel, com no maximo
//!    `CONFERENCIAS` ao mesmo tempo (A2, A3).
//!
//! # Texto claro
//!
//! O painel fala HTTP sem TLS (a petrea de zero dependencia ainda nao tem TLS
//! escrito aqui). Por isso escuta em 127.0.0.1, e so escuta fora disso com
//! `--aceito-sem-tls` escrito (A4, versao minima).

use crate::credencial::{Recusa, Sessoes};
use crate::guarda::{frase_de_bloqueio, Limitador};
use crate::painel::{conferir_login, conferir_rede, Instalacao, Painel, Usuario};
use crate::supervisor::Supervisor;
use crate::web::{self, Pedido, Resposta};
use phxsql_core::hash::{iguais_em_tempo_constante, para_hex};
use phxsql_core::json::Json;
use phxsql_core::semaforo::Semaforo;
use phxsql_core::senha::bytes_aleatorios;
use std::net::TcpListener;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

const VIDA_SESSAO: Duration = Duration::from_secs(8 * 3600);
/// Conexoes atendidas ao mesmo tempo.
pub const TETO_CONEXOES: usize = 256;
/// Conferencias de senha (PBKDF2, ~430 ms cada) ao mesmo tempo.
const CONFERENCIAS: usize = 4;
const TELA: &str = include_str!("tela.html");
/// A frase de todo login recusado -- senha, usuario ou codigo.
const FRASE_LOGIN: &str = "usuário, senha ou código do autenticador não conferem";
const TELA_JS: &str = include_str!("tela.js");

type Gancho = (crate::credencial::Momento, Box<dyn FnOnce(&Estado) + Send>);

pub struct Estado {
    painel: Mutex<Painel>,
    /// Sessoes do painel (token do navegador) e da VPN (`session_id` do
    /// `auth-gen-token`): as duas pelo mesmo motor (`credencial.rs`).
    pub(crate) sessoes: Sessoes,
    pub(crate) vpn: Sessoes,
    /// Ultimo retrato `id -> credencial` da vigia (`None` ate a primeira
    /// volta).
    pub(crate) vistos: Mutex<Option<std::collections::HashMap<i64, i64>>>,
    pub(crate) gancho_de_teste: Mutex<Option<Gancho>>,
    pub supervisor: Option<Supervisor>,
    pub(crate) tentativas: Limitador,
    pub(crate) conferencias: Semaforo,
    /// Valores aceitos no cabecalho `Host`, em minusculas.
    hosts: Vec<String>,
    /// Codigo de uso unico da instalacao; some depois de usado.
    codigo_instalacao: Mutex<Option<String>>,
}

impl Estado {
    pub fn novo(painel: Painel, supervisor: Option<Supervisor>) -> Estado {
        Estado {
            painel: Mutex::new(painel),
            sessoes: Sessoes::nova(VIDA_SESSAO),
            vpn: Sessoes::nova(Duration::from_secs(u64::from(crate::verificar::VIDA_TOKEN))),
            vistos: Mutex::new(None),
            gancho_de_teste: Mutex::new(None),
            supervisor,
            tentativas: Limitador::default(),
            conferencias: Semaforo::novo(CONFERENCIAS),
            hosts: Vec::new(),
            codigo_instalacao: Mutex::new(None),
        }
    }

    /// Nomes pelos quais o painel aceita ser chamado: os de loopback na porta
    /// dele, o proprio endereco de escuta e os declarados em `--nome`.
    pub fn com_hosts(mut self, escuta: &str, nomes: &[String]) -> Estado {
        self.hosts = web::hosts_de(escuta, nomes);
        self
    }

    /// Gera o codigo de instalacao (so quando ainda nao esta instalado) e o
    /// devolve para o terminal do painel mostrar.
    pub fn gerar_codigo_instalacao(&self) -> String {
        let codigo = para_hex(&bytes_aleatorios(6));
        *self
            .codigo_instalacao
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(codigo.clone());
        codigo
    }

    /// A trava do painel. Envenenada (panico de quem a segurava), ela nao
    /// derruba o painel para sempre: a conexao com o banco e dada por
    /// quebrada e refeita na proxima chamada (achado M7).
    pub fn painel(&self) -> MutexGuard<'_, Painel> {
        self.painel.lock().unwrap_or_else(|envenenada| {
            let mut p = envenenada.into_inner();
            p.depois_de_panico();
            p
        })
    }
}

pub fn servir(escuta: &str, estado: Arc<Estado>) -> Result<(), String> {
    let ouvinte = TcpListener::bind(escuta).map_err(|e| format!("escutar em {escuta}: {e}"))?;
    let hosts = estado.hosts.clone();
    web::servir(ouvinte, hosts, TETO_CONEXOES, move |p| atender(p, &estado))
}

/// Uma requisicao inteira (publica para os testes contra o PostgreSQL real
/// exercitarem as rotas sem soquete).
pub fn atender(pedido: &Pedido, estado: &Estado) -> Resposta {
    let caminho = pedido.caminho.split('?').next().unwrap_or_default();
    if pedido.metodo == "GET" && (caminho == "/" || caminho == "/index.html") {
        return Resposta::html(TELA);
    }
    if pedido.metodo == "GET" && caminho == "/tela.js" {
        return Resposta::js(TELA_JS);
    }
    if pedido.metodo == "GET" && caminho == "/simbolo.svg" {
        return Resposta::svg(crate::web::SIMBOLO);
    }
    if pedido.metodo == "GET" && caminho == "/logo-128.png" {
        return Resposta::png(crate::web::LOGO_128);
    }
    if pedido.metodo == "GET" && caminho == "/logo-32.png" {
        return Resposta::png(crate::web::LOGO_32);
    }
    let r = match rotear(pedido, estado) {
        Ok(j) => Resposta::json(200, j.escrever()),
        Err((s, m)) => Resposta::json(s, erro_json(&m)),
    };
    // Certificado revogado neste pedido (remover membro, sair, reentrar):
    // a conexao dele cai agora, e nao so na renegociacao.
    estado.derrubar_revogados();
    r
}

/// A mudanca foi gravada, mas o efeito dela (ccd/, queda da VPN) nao se
/// completou: a rota diz ERRO, nunca «ok» -- e a vigia refaz.
fn incompleta(m: String) -> (u16, String) {
    (
        500,
        format!(
            "a mudança foi gravada, mas não se completou ({m}); o painel tenta de novo a cada {} s",
            crate::credencial::VIGIA.as_secs()
        ),
    )
}

fn erro_json(m: &str) -> String {
    Json::objeto(vec![("erro", Json::texto_de(m))]).escrever()
}

type Saida = Result<Json, (u16, String)>;

/// Erro de regra volta como esta; erro cru do PostgreSQL (que revela o
/// esquema) vai para o log do painel e o cliente ve uma frase generica.
fn ruim(e: String) -> (u16, String) {
    if e.starts_with("PostgreSQL ") || e.starts_with("conexao com o PostgreSQL") {
        eprintln!("phxvpn: {e}");
        return (
            500,
            "erro interno do banco (detalhe no log do painel)".into(),
        );
    }
    (400, e)
}

fn bloqueado(falta: Duration) -> (u16, String) {
    (429, frase_de_bloqueio(falta))
}

fn rotear(p: &Pedido, e: &Estado) -> Saida {
    let corpo = if p.corpo.trim().is_empty() {
        Json::Objeto(vec![])
    } else {
        Json::analisar(&p.corpo).map_err(|x| (400, format!("JSON invalido: {x}")))?
    };
    let t = |c: &str| corpo.texto_ou(c, "").to_string();
    let caminho = p.caminho.split('?').next().unwrap_or_default();
    // Prefixo por canal: erro na VPN (soquete) nao tranca o IP no painel.
    let chave_ip = crate::guarda::chave_de_ip("ip-painel", &p.ip.to_string());

    match (p.metodo.as_str(), caminho) {
        ("GET", "/api/estado") => {
            // Antes da trava do painel: conferir a sessao tambem a toma.
            let com_login = usuario(p, e).is_ok();
            let mut painel = e.painel();
            let instalado = painel.instalado().map_err(ruim)?;
            // Sem login, so o nome: responsavel, e-mail e telefone sao dado
            // pessoal e nao se entregam a quem apenas alcanca a porta.
            let empresa = if instalado {
                let completa = painel.empresa().map_err(ruim)?;
                if com_login {
                    completa
                } else {
                    Json::objeto(vec![(
                        "nome",
                        Json::texto_de(completa.texto_ou("nome", "")),
                    )])
                }
            } else {
                Json::Nulo
            };
            Ok(Json::objeto(vec![
                ("instalado", Json::de_bool(instalado)),
                ("destrancado", Json::de_bool(painel.destrancado())),
                ("empresa", empresa),
                ("openvpn", Json::de_bool(e.supervisor.is_some())),
            ]))
        }
        ("POST", "/api/instalar") => {
            let reserva = e.tentativas.reservar(&[&chave_ip]).map_err(bloqueado)?;
            {
                let codigo = e
                    .codigo_instalacao
                    .lock()
                    .unwrap_or_else(|x| x.into_inner());
                let bate = codigo.as_deref().is_some_and(|c| {
                    iguais_em_tempo_constante(
                        c.as_bytes(),
                        t("codigo_instalacao").trim().as_bytes(),
                    )
                });
                if !bate {
                    drop(codigo);
                    return Err((
                        403,
                        "codigo de instalacao nao confere (ele aparece no terminal do painel)"
                            .into(),
                    ));
                }
            }
            let i = Instalacao {
                empresa: t("empresa"),
                finalidade: t("finalidade"),
                responsavel: t("responsavel"),
                email: t("email"),
                telefone: t("telefone"),
                admin_usuario: t("admin_usuario"),
                admin_senha: t("admin_senha"),
                senha_mestre: t("senha_mestre"),
                servidor_nome: t("servidor_nome"),
                servidor_ip: t("servidor_ip"),
                servidor_dns: t("servidor_dns"),
                certificado_pem: t("certificado_pem"),
            };
            reserva.acertou(&[]);
            e.painel().instalar(&i).map_err(ruim)?;
            // Uso unico: instalado, o codigo morre.
            *e.codigo_instalacao
                .lock()
                .unwrap_or_else(|x| x.into_inner()) = None;
            ok()
        }
        ("POST", "/api/login") => {
            let login = t("usuario");
            // A conta e UMA para os canais (painel, VPN, autenticador): o
            // orcamento de tentativas de uma pessoa nao se multiplica por
            // porta. Reservada ANTES do PBKDF2 (ver `guarda::reservar`).
            let conta = crate::guarda::chave_conta(crate::guarda::Canal::Painel, &login);
            let reserva = e
                .tentativas
                .reservar(&[&conta, &chave_ip])
                .map_err(bloqueado)?;
            // Banco com a trava; PBKDF2 sem ela, e no maximo CONFERENCIAS de
            // uma vez -- o resto espera a vez em vez de parar o painel.
            let (u, hash) = match e.painel().hash_do_login(&login) {
                Ok(x) => x,
                Err(m) => {
                    reserva.devolver();
                    return Err(ruim(m));
                }
            };
            let resultado = {
                let _vez = e.conferencias.adquirir();
                conferir_login(u, &hash, &t("senha"))
            };
            // Segundo fator de quem cadastrou o autenticador. Senha errada e
            // codigo errado dao a MESMA frase: a resposta nao diz que a
            // senha estava certa.
            let resultado = resultado.and_then(|u| {
                let mut painel = e.painel();
                match painel.mfa_ativo(u.id) {
                    Ok(false) => Ok((u, false)),
                    Ok(true) => painel.mfa_conferir(u.id, &t("codigo")).map(|_| (u, true)),
                    Err(x) => Err(x),
                }
            });
            let (u, mfa) = match resultado {
                Ok(x) => {
                    reserva.acertou(&[&conta]);
                    x
                }
                Err(m) if m.starts_with("PostgreSQL ") => {
                    reserva.devolver();
                    return Err(ruim(m));
                }
                // A falha ja foi contada na reserva.
                Err(_) => return Err((401, FRASE_LOGIN.into())),
            };
            let token = para_hex(&bytes_aleatorios(32));
            e.sessoes.abrir(token.clone(), &u);
            Ok(Json::objeto(vec![
                ("token", Json::texto_de(token)),
                ("login", Json::texto_de(u.login)),
                ("admin", Json::de_bool(u.admin)),
                ("mfa", Json::de_bool(mfa)),
            ]))
        }
        ("POST", "/api/sair") => {
            if let Some(tk) = &p.token {
                e.sessoes.fechar(tk);
            }
            ok()
        }
        ("POST", "/api/destrancar") => {
            let u = usuario(p, e)?;
            exigir_admin(&u)?;
            let chave = format!("mestre:{}", u.id);
            let reserva = e.tentativas.reservar(&[&chave]).map_err(bloqueado)?;
            e.painel().destrancar(&t("senha_mestre")).map_err(ruim)?;
            reserva.acertou(&[&chave]);
            materializar_e_subir(e).map_err(ruim)?;
            ok()
        }
        ("GET", "/api/mfa") => {
            let u = usuario(p, e)?;
            let ativo = e.painel().mfa_ativo(u.id).map_err(ruim)?;
            Ok(Json::objeto(vec![("ativo", Json::de_bool(ativo))]))
        }
        ("POST", "/api/mfa/iniciar") => {
            let u = usuario(p, e)?;
            e.painel().mfa_iniciar(&u).map_err(ruim)
        }
        ("POST", "/api/mfa/confirmar") | ("POST", "/api/mfa/desativar") => {
            let u = usuario(p, e)?;
            let conta = crate::guarda::chave_conta(crate::guarda::Canal::Painel, &u.login);
            let reserva = e
                .tentativas
                .reservar(&[&conta, &chave_ip])
                .map_err(bloqueado)?;
            let codigo = t("codigo");
            e.gancho(crate::credencial::Momento::AntesDeGravar);
            let r = if caminho.ends_with("confirmar") {
                e.painel().mfa_confirmar(&u, &codigo)
            } else {
                e.painel().mfa_desativar(&u, &codigo)
            };
            let novo = r.map_err(ruim)?;
            reserva.acertou(&[&conta]);
            // A propria mudanca: esta sessao fica, as outras dele caem.
            e.credencial_mudou(u.id, novo, p.token.as_deref())
                .map_err(incompleta)?;
            ok()
        }
        ("POST", "/api/usuarios/mfa-zerar") => {
            let u = usuario(p, e)?;
            exigir_admin(&u)?;
            let (alvo, novo) = e.painel().mfa_zerar(&u, &t("login")).map_err(ruim)?;
            e.credencial_mudou(alvo, novo, None).map_err(incompleta)?;
            ok()
        }
        ("POST", "/api/usuarios/ativo") => {
            let u = usuario(p, e)?;
            exigir_admin(&u)?;
            let ativo = corpo
                .campo("ativo")
                .and_then(Json::booleano)
                .ok_or((400, "ativo: true ou false".to_string()))?;
            let (alvo, novo) = e
                .painel()
                .usuario_definir_ativo(&u, &t("login"), ativo)
                .map_err(ruim)?;
            e.credencial_mudou(alvo, novo, None).map_err(incompleta)?;
            ok()
        }
        ("POST", "/api/senha") => {
            let u = usuario(p, e)?;
            let nova = t("senha_nova");
            crate::painel::validar_senha("senha_nova", &nova, 8).map_err(|m| (400, m))?;
            // A senha atual (e o codigo, de quem tem autenticador): sessao
            // roubada nao troca a senha do dono. Conta como tentativa.
            let conta = crate::guarda::chave_conta(crate::guarda::Canal::Painel, &u.login);
            let reserva = e
                .tentativas
                .reservar(&[&conta, &chave_ip])
                .map_err(bloqueado)?;
            let (atual, hash) = match e.painel().hash_do_login(&u.login) {
                Ok(x) => x,
                Err(m) => {
                    reserva.devolver();
                    return Err(ruim(m));
                }
            };
            let conferido = {
                let _vez = e.conferencias.adquirir();
                conferir_login(atual, &hash, &t("senha_atual"))
            }
            .and_then(|a| {
                let mut painel = e.painel();
                match painel.mfa_ativo(a.id) {
                    Ok(false) => Ok(a),
                    Ok(true) => painel.mfa_conferir(a.id, &t("codigo")).map(|_| a),
                    Err(x) => Err(x),
                }
            });
            match conferido {
                Ok(_) => reserva.acertou(&[&conta]),
                Err(m) if m.starts_with("PostgreSQL ") => {
                    reserva.devolver();
                    return Err(ruim(m));
                }
                Err(_) => return Err((401, FRASE_LOGIN.into())),
            }
            let iteracoes = e.painel().iteracoes;
            let novo = {
                let _vez = e.conferencias.adquirir();
                phxsql_core::senha::cifrar_com(&nova, iteracoes)
            };
            e.gancho(crate::credencial::Momento::AntesDeGravar);
            let credencial = e.painel().gravar_senha(u.id, &novo).map_err(ruim)?;
            e.credencial_mudou(u.id, credencial, p.token.as_deref())
                .map_err(incompleta)?;
            ok()
        }
        ("POST", "/api/redes/mfa") => {
            let u = usuario(p, e)?;
            let id = rede_id(&corpo)?;
            let exige = corpo.booleano_ou("exige", false);
            // Quem tem autenticador prova o codigo para mudar a exigencia:
            // sessao roubada nao desliga o segundo fator de uma rede.
            let conta = crate::guarda::chave_conta(crate::guarda::Canal::Painel, &u.login);
            let reserva = e
                .tentativas
                .reservar(&[&conta, &chave_ip])
                .map_err(bloqueado)?;
            let (nome, dir) = e
                .painel()
                .rede_definir_mfa(&u, id, exige, &t("codigo"))
                .map_err(|m| (403, m))?;
            reserva.acertou(&[&conta]);
            // O OpenVPN so le o servidor.conf ao subir. Se nao subir, a
            // exigencia volta: a flag nunca diz «exige» com o servidor
            // rodando sem o verificador.
            let reiniciado = match &e.supervisor {
                Some(s) => {
                    if let Err(m) = s.reiniciar(&nome, &dir) {
                        let _ = e.painel().rede_gravar_mfa(id, !exige);
                        let _ = s.reiniciar(&nome, &dir);
                        return Err(ruim(m));
                    }
                    true
                }
                None => false,
            };
            Ok(Json::objeto(vec![
                ("ok", Json::de_bool(true)),
                ("exige_mfa", Json::de_bool(exige)),
                ("openvpn_reiniciado", Json::de_bool(reiniciado)),
                (
                    "aviso",
                    Json::texto_de(
                        "os membros precisam baixar o perfil de novo (entrar na rede outra vez)",
                    ),
                ),
            ]))
        }
        ("GET", "/api/redes") => {
            let u = usuario(p, e)?;
            e.painel().redes(&u).map_err(ruim)
        }
        ("POST", "/api/redes") => {
            let u = usuario(p, e)?;
            let servidor = corpo.campo("servidor_id").and_then(Json::inteiro);
            let transporte = transporte_do_pedido(&corpo)?;
            let cliente = cliente_do_pedido(&corpo)?;
            let perfil = e
                .painel()
                .criar_rede_com(
                    &u,
                    &t("nome"),
                    &t("senha"),
                    &t("finalidade"),
                    servidor,
                    &transporte,
                )
                .map_err(ruim)?;
            materializar_e_subir(e).map_err(ruim)?;
            perfil_json(&t("nome"), com_cliente(e, &t("nome"), perfil, &cliente)?)
        }
        ("POST", "/api/redes/entrar") => {
            let u = usuario(p, e)?;
            let nome = t("nome");
            // Por usuario E rede: quem erra a senha de uma rede nao trava as
            // outras; e o IP segura quem troca de conta.
            let chave_rede = format!("rede:{}:{nome}", u.id);
            let reserva = e
                .tentativas
                .reservar(&[&chave_rede, &chave_ip])
                .map_err(bloqueado)?;
            // Erro aqui tambem conta: a reserva cai com a falha dentro.
            let hash = e.painel().hash_da_rede(&nome).map_err(ruim)?;
            let conferido = {
                let _vez = e.conferencias.adquirir();
                conferir_rede(&hash, &t("senha"))
            };
            conferido.map_err(|m| (400, m))?;
            reserva.acertou(&[&chave_rede]);
            let proxy = crate::alcance::ProxyMembro::do_pedido(&corpo).map_err(|m| (400, m))?;
            let cliente = cliente_do_pedido(&corpo)?;
            let perfil = e
                .painel()
                .entrar_com_proxy(&u, &nome, proxy)
                .map_err(ruim)?;
            perfil_json(&nome, com_cliente(e, &nome, perfil, &cliente)?)
        }
        ("POST", "/api/redes/sair") => {
            let u = usuario(p, e)?;
            let id = rede_id(&corpo)?;
            e.painel().sair_da_rede(&u, id).map_err(ruim)?;
            ok()
        }
        ("POST", "/api/redes/remover") => {
            let u = usuario(p, e)?;
            let id = rede_id(&corpo)?;
            e.painel()
                .remover_membro(&u, id, &t("login"))
                .map_err(|m| (403, m))?;
            ok()
        }
        ("POST", "/api/redes/membros") => {
            let u = usuario(p, e)?;
            let id = rede_id(&corpo)?;
            e.painel().membros(&u, id).map_err(|m| (403, m))
        }
        ("POST", "/api/redes/alcance") => {
            let u = usuario(p, e)?;
            let id = rede_id(&corpo)?;
            e.painel().alcance(&u, id).map_err(|m| (403, m))
        }
        ("POST", "/api/redes/alcance/gravar") => {
            let u = usuario(p, e)?;
            // Abre porta no host (queda, port-share) e muda o perfil de todos:
            // o degrau de abrir a LAN, admin e o codigo de quem o tem.
            exigir_admin(&u)?;
            exigir_codigo(e, &u, &t("codigo"), &chave_ip)?;
            let id = rede_id(&corpo)?;
            let novo = crate::alcance::Alcance::do_pedido(&corpo).map_err(|m| (400, m))?;
            crate::alcance::gravar_e_aplicar(e, &u, id, &novo).map_err(ruim)
        }
        ("POST", "/api/redes/rotas") => {
            let u = usuario(p, e)?;
            let id = rede_id(&corpo)?;
            e.painel().rotas(&u, id).map_err(|m| (403, m))
        }
        ("POST", "/api/redes/rotas/incluir") | ("POST", "/api/redes/rotas/remover") => {
            let u = usuario(p, e)?;
            let id = rede_id(&corpo)?;
            let cidr = t("cidr");
            let incluir = caminho.ends_with("incluir");
            if incluir {
                // Abrir a LAN da empresa e a mudanca mais larga do painel:
                // sessao roubada nao a faz sem o codigo de quem o tem.
                exigir_admin(&u)?;
                exigir_codigo(e, &u, &t("codigo"), &chave_ip)?;
            }
            let (nome, dir) = if incluir {
                e.painel()
                    .rota_incluir(&u, id, &cidr, &t("volta"), &t("membro"))
            } else {
                e.painel().rota_remover(&u, id, &cidr)
            }
            .map_err(ruim)?;
            crate::rotas::depois_de_mudar(e, incluir, id, &cidr, &nome, &dir).map_err(ruim)
        }
        ("POST", "/api/redes/saida") => {
            let u = usuario(p, e)?;
            let id = rede_id(&corpo)?;
            e.painel().saida(&u, id).map_err(|m| (403, m))
        }
        ("POST", "/api/redes/saida/definir") => {
            let u = usuario(p, e)?;
            let id = rede_id(&corpo)?;
            let nova = crate::saida::Saida {
                tunel_total: corpo.booleano_ou("tunel_total", false),
                bloquear_local: corpo.booleano_ou("bloquear_local", false),
                dns_nomes: corpo.booleano_ou("dns_nomes", false),
                dns_empresa: crate::saida::analisar_dns(&t("dns_empresa")).map_err(|m| (400, m))?,
            };
            // Tunel total faz do servidor a saida de internet dos membros:
            // o mesmo degrau de abrir a LAN (admin e o codigo de quem tem).
            // Estreitar nao pede codigo.
            if u.admin {
                exigir_codigo(e, &u, &t("codigo"), &chave_ip)?;
            }
            let (nome, dir, antiga) = e.painel().saida_definir(&u, id, &nova).map_err(ruim)?;
            crate::saida::depois_de_mudar(e, id, &antiga, &nova, &nome, &dir).map_err(ruim)
        }
        ("GET", "/api/usuarios") => {
            exigir_admin(&usuario(p, e)?)?;
            e.painel().usuarios().map_err(ruim)
        }
        ("POST", "/api/usuarios") => {
            exigir_admin(&usuario(p, e)?)?;
            e.painel()
                .criar_usuario(
                    &t("login"),
                    &t("senha"),
                    &t("email"),
                    corpo.booleano_ou("admin", false),
                )
                .map_err(ruim)?;
            ok()
        }
        ("GET", "/api/servidores") => {
            usuario(p, e)?;
            e.painel().servidores().map_err(ruim)
        }
        ("POST", "/api/servidores") => {
            exigir_admin(&usuario(p, e)?)?;
            e.painel()
                .criar_servidor(&t("nome"), &t("ip"), &t("dns"))
                .map_err(ruim)?;
            ok()
        }
        _ => Err((404, format!("rota desconhecida: {} {caminho}", p.metodo))),
    }
}

fn rede_id(corpo: &Json) -> Result<i64, (u16, String)> {
    corpo
        .campo("rede_id")
        .and_then(Json::inteiro)
        .ok_or((400, "rede_id".into()))
}

fn ok() -> Saida {
    Ok(Json::objeto(vec![("ok", Json::de_bool(true))]))
}

/// `dns_linux` e `sem_ipv6` do pedido: o que a maquina de quem baixa pede
/// ao proprio perfil (`saida.rs`). Conferido ANTES de emitir o certificado.
fn cliente_do_pedido(corpo: &Json) -> Result<crate::saida::Cliente, (u16, String)> {
    crate::saida::cliente(
        corpo.texto_ou("dns_linux", ""),
        corpo.booleano_ou("sem_ipv6", false),
    )
    .map_err(|m| (400, m))
}

fn com_cliente(
    e: &Estado,
    rede: &str,
    perfil: String,
    c: &crate::saida::Cliente,
) -> Result<String, (u16, String)> {
    if *c == crate::saida::Cliente::default() {
        return Ok(perfil);
    }
    let s = e.painel().saida_por_nome(rede).map_err(ruim)?;
    Ok(perfil + &crate::saida::perfil_cliente(c, &s))
}

fn perfil_json(rede: &str, perfil: String) -> Saida {
    Ok(Json::objeto(vec![
        (
            "arquivo",
            Json::texto_de(crate::ovpn::arquivo_do_perfil(rede)),
        ),
        ("perfil", Json::texto_de(perfil)),
    ]))
}

/// Quem pede, conferido a CADA pedido pelo motor das sessoes: usuario
/// desativado, ou com senha, autenticador ou `admin` mudados depois do
/// login, nao passa -- e o `admin` que vale e o do banco agora, nao o do
/// login.
fn usuario(p: &Pedido, e: &Estado) -> Result<Usuario, (u16, String)> {
    let tk = p.token.as_ref().ok_or((401, "faca login".to_string()))?;
    match e.sessoes.conferir(tk, |id| e.painel().usuario_vigente(id)) {
        Ok(u) => Ok(u),
        Err(Recusa::CredencialMudou) => Err((
            401,
            "a conta mudou (senha, autenticador ou acesso): faca login de novo".into(),
        )),
        Err(Recusa::Banco(m)) => Err(ruim(m)),
        Err(_) => Err((401, "sessao expirada: faca login de novo".into())),
    }
}

/// O codigo do autenticador de quem o tem, antes de uma mudanca larga
/// (rota para a LAN, tunel total): sessao roubada nao a faz. Conta como
/// tentativa, com a mesma trava do login.
fn exigir_codigo(
    e: &Estado,
    u: &Usuario,
    codigo: &str,
    chave_ip: &str,
) -> Result<(), (u16, String)> {
    if !e.painel().mfa_ativo(u.id).map_err(ruim)? {
        return Ok(());
    }
    let conta = crate::guarda::chave_conta(crate::guarda::Canal::Painel, &u.login);
    let reserva = e
        .tentativas
        .reservar(&[&conta, chave_ip])
        .map_err(bloqueado)?;
    e.painel()
        .mfa_conferir(u.id, codigo)
        .map_err(|m| (403, m))?;
    reserva.acertou(&[&conta]);
    Ok(())
}

fn exigir_admin(u: &Usuario) -> Result<(), (u16, String)> {
    if u.admin {
        Ok(())
    } else {
        Err((403, "so o administrador faz isso".into()))
    }
}

/// Reescreve os arquivos de todas as redes e, se o supervisor esta ligado,
/// sobe o OpenVPN das que ainda nao estao no ar.
pub fn materializar_e_subir(e: &Estado) -> Result<(), String> {
    let redes = e.painel().materializar_todas()?;
    if let Some(s) = &e.supervisor {
        for (nome, dir) in redes {
            s.garantir(&nome, &dir)?;
        }
    }
    // O firewall das rotas volta com o painel (o kernel nao guarda a tabela
    // entre reinicios). Falhar aqui nao derruba as redes: fica dito no log.
    if let Err(m) = crate::rotas::aplicar_no_host(e) {
        eprintln!("phxvpn: AVISO rotas: {m}");
    }
    crate::saida::acertar_dns(e);
    Ok(())
}

/// `protocolo` ("udp" | "tcp"), `porta` e o proxy do pedido de criar
/// rede. Ausentes: o de sempre (UDP, porta automatica, sem proxy).
fn transporte_do_pedido(corpo: &Json) -> Result<crate::painel::Transporte, (u16, String)> {
    let tcp = match corpo.texto_ou("protocolo", "udp") {
        "udp" | "" => false,
        "tcp" => true,
        _ => return Err((400, "protocolo: udp ou tcp".into())),
    };
    let porta = match corpo.campo("porta").and_then(Json::inteiro) {
        Some(p) => Some(
            u16::try_from(p)
                .ok()
                .filter(|p| *p > 0)
                .ok_or((400, "porta de 1 a 65535".to_string()))?,
        ),
        None => None,
    };
    let proxy = crate::alcance::ProxyMembro::do_pedido(corpo).map_err(|m| (400, m))?;
    Ok(crate::painel::Transporte { tcp, porta, proxy })
}
