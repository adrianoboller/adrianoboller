//! Historico de conexoes do modo servidor: quem entrou, de onde, quando saiu
//! e quanto trafegou -- o que o `status.log` do OpenVPN nao guarda (ele so
//! sabe o «agora»).
//!
//! # O caminho, pelo MESMO soquete do verificador
//!
//! ```text
//! openvpn (ja como phxvpn-ovpn)  client-connect / client-disconnect
//!   -> `phxvpn ovpn-historico SOQUETE REDE`      (le SO o que precisa do ambiente)
//!   -> filho `phxvpn ovpn-historico-enviar SOQUETE` (o openvpn nao espera o painel)
//!   -> verificar.sock do painel (0660 + SO_PEERCRED)  -> phx_conexao no PostgreSQL
//! ```
//!
//! Um motor so: o soquete, as duas portas de quem pergunta, o teto de
//! perguntas e o prazo sao os do `verificar.rs`; aqui so mora o que o
//! historico tem de proprio. Um segundo soquete seria uma segunda copia da
//! decisao «quem pode falar com o painel», e a que alguem esquecesse de
//! apertar abriria o banco a qualquer daemon.
//!
//! # Nunca barra a VPN
//!
//! O codigo de saida do `client-connect` decide se o membro entra (script-
//! options.rst: «non-zero ... will cause the client to be disconnected»).
//! Historico e auditoria, nao portao: o gancho sai 0 sempre, e o painel fora
//! do ar deixa um buraco no historico dito no log do OpenVPN -- nunca uma
//! rede parada. E o gancho roda DENTRO do laco do `openvpn`: esperar o
//! painel (trava, PostgreSQL) pararia o trafego de todos, por isso o envio
//! vai por um filho, como o adiado do verificador.
//!
//! # A chave de uma sessao, e por que a ordem nao importa
//!
//! `(rede, cn, ip real, porta real, entrou_em)`, com `entrou_em` =
//! `time_unix`, o instante em que o OpenVPN criou a instancia -- a MESMA
//! variavel nos dois ganchos (`multi.c`, `multi_client_connect_setenv`; o
//! ambiente da instancia persiste ate a saida). Os dois filhos correm
//! soltos, e numa conexao curta a saida pode chegar antes da entrada: por
//! isso a saida e um `INSERT ... ON CONFLICT DO UPDATE` e a entrada um
//! `ON CONFLICT DO NOTHING`. A linha certa sai em qualquer ordem.
//!
//! # O IP real de quem veio pela ponte TCP
//!
//! Quem cai do UDP para o TCP entra pela ponte (`queda_tcp.rs`), e o
//! `openvpn` o ve como `127.x.y.z:porta`. A chave da sessao fica no que o
//! `openvpn` viu (`ip_visto`/`porta_vista` -- e o que os dois ganchos trazem);
//! o `ip_real` sai do mapa da propria ponte ([`crate::queda_tcp::origem_real`]),
//! no mesmo processo. Reler o `openvpn.log` seria uma segunda fonte da mesma
//! resposta, e a que girasse o log antes da saida perderia o IP.
//!
//! # O que NUNCA entra
//!
//! Senha, codigo do autenticador, token e `session_id`: o pedido e montado
//! por lista FECHADA de variaveis ([`CAMPOS`]), nunca copiando o ambiente
//! -- com `auth-user-pass-verify via-env` o OpenVPN poria a senha nele.

use crate::http::Estado;
use crate::painel::{Painel, Usuario};
use crate::pg::R;
use phxsql_core::json::Json;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// Tabela nova e idempotente, como o resto do esquema. `usuario_id` pode
/// ser nulo: o login pode ter mudado depois da conexao, e perder a linha da
/// auditoria seria pior que ficar sem o vinculo -- o `cn` fica sempre. As
/// duas chaves estrangeiras sao `ON DELETE RESTRICT`, a regra de toda a
/// `phx_*`; e o historico nao aponta para `phx_membro`, senao ninguem sairia
/// de uma rede em que ja conectou.
pub const ESQUEMA: &str = "
CREATE TABLE IF NOT EXISTS phx_conexao (
    id              bigserial PRIMARY KEY,
    rede_id         int NOT NULL REFERENCES phx_rede ON DELETE RESTRICT,
    usuario_id      int REFERENCES phx_usuario ON DELETE RESTRICT,
    cn              text NOT NULL,
    ip_real         text NOT NULL,
    porta_real      int NOT NULL CHECK (porta_real BETWEEN 0 AND 65535),
    ip_visto        text NOT NULL,
    porta_vista     int NOT NULL CHECK (porta_vista BETWEEN 0 AND 65535),
    ip_vpn          text NOT NULL DEFAULT '',
    entrou_em       timestamptz NOT NULL,
    saiu_em         timestamptz,
    bytes_do_membro bigint CHECK (bytes_do_membro >= 0),
    bytes_ao_membro bigint CHECK (bytes_ao_membro >= 0),
    UNIQUE (rede_id, cn, ip_visto, porta_vista, entrou_em),
    CHECK (saiu_em IS NULL OR saiu_em >= entrou_em)
);
CREATE INDEX IF NOT EXISTS phx_conexao_quando ON phx_conexao (entrou_em DESC);
CREATE INDEX IF NOT EXISTS phx_conexao_usuario ON phx_conexao (usuario_id, entrou_em DESC);
CREATE TABLE IF NOT EXISTS phx_historico_config (
    unica         boolean PRIMARY KEY DEFAULT true CHECK (unica),
    retencao_dias int NOT NULL DEFAULT 90 CHECK (retencao_dias BETWEEN 1 AND 3650)
);
INSERT INTO phx_historico_config DEFAULT VALUES ON CONFLICT DO NOTHING;
";

/// Retencao que nasce com o banco (e o `DEFAULT` do esquema acima).
pub const RETENCAO_PADRAO: i64 = 90;
/// Faixa aceita: um dia a dez anos (o `CHECK` do esquema diz o mesmo).
pub const RETENCAO: std::ops::RangeInclusive<i64> = 1..=3650;
/// De quanto em quanto tempo a poda roda.
const PODA: Duration = Duration::from_secs(3600);
/// Linhas que a tela recebe de uma vez (as mais recentes).
pub const TETO_LISTA: i64 = 500;

/// As UNICAS variaveis do ambiente do `openvpn` que o gancho le. Lista
/// fechada de proposito: copiar o ambiente levaria `password`/`username`
/// (via-env) ou o `session_id` do token para o painel e para o log.
pub const CAMPOS: [&str; 10] = [
    "script_type",
    "common_name",
    "trusted_ip",
    "trusted_ip6",
    "trusted_port",
    "ifconfig_pool_remote_ip",
    "time_unix",
    "time_duration",
    "bytes_received",
    "bytes_sent",
];

/// As linhas do `servidor.conf` que ligam o historico. No Windows nao ha o
/// soquete local (o verificador tambem recusa tudo la): sem linhas, em vez
/// de um processo por conexao que nao tem a quem falar.
pub fn conf(dados: &Path, rede_id: &str) -> String {
    if cfg!(windows) {
        return String::new();
    }
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "phxvpn".into());
    let cmd = format!(
        "\"{exe} ovpn-historico {} {rede_id}\"",
        crate::verificar::socket(dados).display()
    );
    format!(
        "# historico de conexoes (phxvpn ovpn-historico): quem, de onde, quando, quanto\n\
         script-security 2\n\
         client-connect {cmd}\n\
         client-disconnect {cmd}\n"
    )
}

/// O pedido que o gancho leva ao painel, montado so de [`CAMPOS`]. `None`
/// quando o `script_type` nao e um dos dois ganchos (nada a registrar).
pub fn pedido(rede: &str, var: impl Fn(&str) -> String) -> Option<Json> {
    let tipo = match var("script_type").as_str() {
        "client-connect" => "entrou",
        "client-disconnect" => "saiu",
        _ => return None,
    };
    // Cliente por IPv6: so o `trusted_ip6` vem preenchido.
    let ip = Some(var("trusted_ip"))
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| var("trusted_ip6"));
    let mut campos = vec![
        ("tipo", Json::texto_de(tipo)),
        ("rede", Json::texto_de(rede)),
        ("cn", Json::texto_de(var("common_name"))),
        ("ip", Json::texto_de(ip)),
        ("porta", Json::texto_de(var("trusted_port"))),
        ("ip_vpn", Json::texto_de(var("ifconfig_pool_remote_ip"))),
        ("desde", Json::texto_de(var("time_unix"))),
    ];
    if tipo == "saiu" {
        campos.push(("duracao", Json::texto_de(var("time_duration"))));
        campos.push(("bytes_do_membro", Json::texto_de(var("bytes_received"))));
        campos.push(("bytes_ao_membro", Json::texto_de(var("bytes_sent"))));
    }
    Some(Json::objeto(campos))
}

// ------------------------------------ lado do OpenVPN (usuario proprio) ----

/// `phxvpn ovpn-historico SOQUETE REDE [ARQUIVO]` -- o `client-connect` e o
/// `client-disconnect`. Sai 0 SEMPRE (ver «Nunca barra a VPN»).
pub fn principal(args: &[String]) -> i32 {
    let (Some(sock), Some(rede)) = (args.first(), args.get(1)) else {
        eprintln!("phxvpn ovpn-historico: faltam o soquete e a rede");
        return 0;
    };
    let var = |n: &str| {
        if CAMPOS.contains(&n) {
            std::env::var(n).unwrap_or_default()
        } else {
            String::new()
        }
    };
    let Some(p) = pedido(rede, var) else {
        return 0;
    };
    let p = p.escrever();
    if !crate::verificar::por_um_filho(&["ovpn-historico-enviar", sock], &p, true)
        && !crate::verificar::perguntar(Path::new(sock), &p)
    {
        eprintln!(
            "phxvpn ovpn-historico: o painel nao respondeu; esta conexao fica fora do historico"
        );
    }
    0
}

/// `phxvpn ovpn-historico-enviar SOQUETE`, o filho: le o pedido do stdin e
/// o entrega. A saida de erro e a do `openvpn` (vai para o `openvpn.log`).
pub fn enviar(args: &[String]) -> i32 {
    let Some(sock) = args.first() else {
        return 1;
    };
    let mut p = String::new();
    let _ = std::io::Read::read_to_string(
        &mut std::io::Read::take(std::io::stdin(), crate::verificar::TETO_PEDIDO),
        &mut p,
    );
    if crate::verificar::perguntar(Path::new(sock), &p) {
        0
    } else {
        eprintln!("phxvpn ovpn-historico: o painel recusou ou nao respondeu; esta conexao fica fora do historico");
        1
    }
}

// ---------------------------------------------------- lado do painel ----

/// O que o soquete do verificador faz com um pedido do historico. `None`:
/// nao e do historico (segue para a conferencia de senha e codigo).
pub fn atender(e: &Estado, j: &Json) -> Option<Result<(), String>> {
    matches!(j.texto_ou("tipo", ""), "entrou" | "saiu").then(|| e.painel().registrar_conexao(j))
}

/// Um evento conferido: tudo o que veio do ambiente, lido e validado.
#[derive(Debug, PartialEq)]
pub struct Evento {
    pub saiu: bool,
    pub rede: String,
    pub cn: String,
    pub login: String,
    pub ip: String,
    pub porta: u16,
    pub ip_vpn: String,
    pub desde: i64,
    pub duracao: Option<i64>,
    pub bytes_do_membro: Option<i64>,
    pub bytes_ao_membro: Option<i64>,
}

/// Le e confere o pedido. Tudo vem do ambiente do `openvpn` (que o
/// `SO_PEERCRED` garante ser o usuario dele), e mesmo assim nada vai ao
/// banco sem ser analisado: IP e reserializado, numero e numero.
pub fn evento(j: &Json) -> Result<Evento, String> {
    let t = |c: &str| j.texto_ou(c, "").trim().to_string();
    let saiu = match t("tipo").as_str() {
        "entrou" => false,
        "saiu" => true,
        _ => return Err("tipo de evento desconhecido".into()),
    };
    let (rede, cn) = (t("rede"), t("cn"));
    let (login, rede_cn) =
        crate::verificar::cn_login_rede(&cn).ok_or("CN fora do formato do phxvpn")?;
    if rede_cn != rede {
        return Err("certificado de outra rede".into());
    }
    let ip: IpAddr = t("ip").parse().map_err(|_| "IP real ilegivel")?;
    let porta: u16 = t("porta").parse().map_err(|_| "porta real ilegivel")?;
    let ip_vpn = match t("ip_vpn").as_str() {
        "" => String::new(),
        v => v
            .parse::<Ipv4Addr>()
            .map_err(|_| "IP da VPN ilegivel")?
            .to_string(),
    };
    let desde: i64 = t("desde")
        .parse()
        .ok()
        .filter(|d| *d > 0)
        .ok_or("sem o instante da conexao (time_unix)")?;
    let numero = |c: &str| -> Result<Option<i64>, String> {
        match t(c).as_str() {
            "" => Ok(None),
            v => v
                .parse::<u64>()
                .map(|n| Some(i64::try_from(n).unwrap_or(i64::MAX)))
                .map_err(|_| format!("{c} ilegivel")),
        }
    };
    Ok(Evento {
        saiu,
        login: login.to_string(),
        rede,
        cn,
        ip: ip.to_string(),
        porta,
        ip_vpn,
        desde,
        duracao: if saiu { numero("duracao")? } else { None },
        bytes_do_membro: if saiu {
            numero("bytes_do_membro")?
        } else {
            None
        },
        bytes_ao_membro: if saiu {
            numero("bytes_ao_membro")?
        } else {
            None
        },
    })
}

/// Liga a poda: agora e a cada hora. A falha fica no log e a proxima volta
/// tenta de novo -- banco fora do ar nao derruba o painel.
pub fn vigiar(e: Arc<Estado>) {
    std::thread::spawn(move || loop {
        match e.painel().podar_historico() {
            Ok(0) => {}
            Ok(n) => eprintln!("phxvpn: historico: {n} conexao(oes) alem da retencao apagada(s)"),
            Err(m) => eprintln!("phxvpn: historico: poda falhou: {m}"),
        }
        std::thread::sleep(PODA);
    });
}

/// O endereco de fora de quem o `openvpn` ve como `ip:porta`: o da ponte TCP
/// quando ela o conhece; senao, o proprio. `true` quando veio pela ponte.
pub fn origem(ip: &str, porta: u16) -> (String, u16, bool) {
    let visto = ip
        .parse::<IpAddr>()
        .ok()
        .filter(IpAddr::is_loopback)
        .and_then(|i| crate::queda_tcp::origem_real(std::net::SocketAddr::new(i, porta)));
    match visto {
        // `::ffff:a.b.c.d` da escuta de pilha dupla vira o IPv4 que e.
        Some(o) => (crate::soquete::canonico(o).ip().to_string(), o.port(), true),
        None => (ip.to_string(), porta, false),
    }
}

fn opcional(v: Option<i64>) -> Option<String> {
    v.map(|n| n.to_string())
}

impl Painel {
    /// Grava a entrada ou a saida (ver «A chave de uma sessao»).
    pub fn registrar_conexao(&mut self, j: &Json) -> R<()> {
        let ev = evento(j)?;
        let (ip_real, porta_real, _) = origem(&ev.ip, ev.porta);
        let (porta, desde) = (ev.porta.to_string(), ev.desde.to_string());
        let porta_real = porta_real.to_string();
        // O instante da saida e o do OpenVPN (entrada + duracao), nao o da
        // chegada do pedido: o filho pode atrasar, a conta dele nao.
        let saiu_em = ev.duracao.map(|d| (ev.desde + d).to_string());
        let (bm, ba) = (opcional(ev.bytes_do_membro), opcional(ev.bytes_ao_membro));
        let sql = if ev.saiu {
            "INSERT INTO phx_conexao (rede_id, usuario_id, cn, ip_visto, porta_vista, ip_vpn, entrou_em, \
             ip_real, porta_real, saiu_em, bytes_do_membro, bytes_ao_membro) \
             VALUES ($1::int, coalesce((SELECT usuario_id FROM phx_membro WHERE cn = $2 AND rede_id = $1::int), \
             (SELECT id FROM phx_usuario WHERE login = $3)), $2, $4, $5::int, $6, to_timestamp($7::bigint), \
             $8, $9::int, coalesce(to_timestamp($10::bigint), now()), $11::bigint, $12::bigint) \
             ON CONFLICT (rede_id, cn, ip_visto, porta_vista, entrou_em) DO UPDATE SET \
             saiu_em = EXCLUDED.saiu_em, bytes_do_membro = EXCLUDED.bytes_do_membro, \
             bytes_ao_membro = EXCLUDED.bytes_ao_membro"
        } else {
            "INSERT INTO phx_conexao (rede_id, usuario_id, cn, ip_visto, porta_vista, ip_vpn, entrou_em, \
             ip_real, porta_real) \
             VALUES ($1::int, coalesce((SELECT usuario_id FROM phx_membro WHERE cn = $2 AND rede_id = $1::int), \
             (SELECT id FROM phx_usuario WHERE login = $3)), $2, $4, $5::int, $6, to_timestamp($7::bigint), \
             $8, $9::int) \
             ON CONFLICT (rede_id, cn, ip_visto, porta_vista, entrou_em) DO NOTHING"
        };
        let mut params: Vec<Option<&str>> = vec![
            Some(&ev.rede),
            Some(&ev.cn),
            Some(&ev.login),
            Some(&ev.ip),
            Some(&porta),
            Some(&ev.ip_vpn),
            Some(&desde),
            Some(&ip_real),
            Some(&porta_real),
        ];
        if ev.saiu {
            params.extend([saiu_em.as_deref(), bm.as_deref(), ba.as_deref()]);
        }
        self.pg()?.executar(sql, &params)?;
        Ok(())
    }

    pub fn retencao_do_historico(&mut self) -> R<i64> {
        let r = self.pg()?.executar(
            "SELECT retencao_dias FROM phx_historico_config WHERE unica",
            &[],
        )?;
        Ok(r.valor(0, "retencao_dias")
            .and_then(|v| v.parse().ok())
            .unwrap_or(RETENCAO_PADRAO))
    }

    /// So o administrador muda (quem chama confere), e a poda roda na hora:
    /// encurtar a retencao apaga o que ficou fora dela.
    pub fn mudar_retencao(&mut self, dias: i64) -> R<u64> {
        if !RETENCAO.contains(&dias) {
            return Err(format!(
                "retenção de {} a {} dias",
                RETENCAO.start(),
                RETENCAO.end()
            ));
        }
        let d = dias.to_string();
        self.pg()?.executar(
            "UPDATE phx_historico_config SET retencao_dias = $1::int WHERE unica",
            &[Some(&d)],
        )?;
        self.podar_historico()
    }

    /// Apaga o que passou da retencao. A linha aberta (sem saida registrada)
    /// conta pela entrada: um `openvpn` morto por SIGKILL nunca chama o
    /// `client-disconnect`, e ela ficaria para sempre.
    pub fn podar_historico(&mut self) -> R<u64> {
        let r = self.pg()?.executar(
            "DELETE FROM phx_conexao WHERE coalesce(saiu_em, entrou_em) < now() - \
             make_interval(days => (SELECT retencao_dias FROM phx_historico_config WHERE unica))",
            &[],
        )?;
        Ok(r.afetadas)
    }

    /// O historico que `u` pode ver: o admin, todos; o membro, so o dele.
    /// Conexao sem saida registrada aparece como «conectado» so se o CN
    /// esta no `status.log` da rede agora -- senao a saida se perdeu (painel
    /// fora do ar, `openvpn` morto) e a tela diz isso em vez de inventar.
    pub fn historico(&mut self, u: &Usuario) -> R<Json> {
        let filtro = (!u.admin).then(|| u.id.to_string());
        let limite = TETO_LISTA.to_string();
        let r = self.pg()?.executar(
            "SELECT c.rede_id, r.nome AS rede, coalesce(u.login, '') AS login, c.ip_real, c.porta_real, \
             (c.ip_visto <> c.ip_real) AS pela_ponte, \
             c.ip_vpn, c.cn, \
             to_char(c.entrou_em AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS entrou_em, \
             to_char(c.saiu_em AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS saiu_em, \
             extract(epoch FROM coalesce(c.saiu_em, now()) - c.entrou_em)::bigint AS segundos, \
             c.bytes_do_membro, c.bytes_ao_membro \
             FROM phx_conexao c JOIN phx_rede r ON r.id = c.rede_id \
             LEFT JOIN phx_usuario u ON u.id = c.usuario_id \
             WHERE $1::int IS NULL OR c.usuario_id = $1::int \
             ORDER BY c.entrou_em DESC, c.id DESC LIMIT $2::int",
            &[filtro.as_deref(), Some(&limite)],
        )?;
        let mut online: std::collections::HashMap<String, Vec<String>> = Default::default();
        let mut lista = Vec::new();
        for i in 0..r.linhas.len() {
            let v = |c: &str| r.valor(i, c).map(str::to_string);
            let n = |c: &str| {
                v(c).and_then(|x| x.parse::<i64>().ok())
                    .map(Json::de_i64)
                    .unwrap_or(Json::Nulo)
            };
            let saiu = v("saiu_em");
            let estado = match &saiu {
                Some(_) => "saiu",
                None => {
                    let rede = v("rede_id").unwrap_or_default();
                    if !online.contains_key(&rede) {
                        let agora = self.conectados_na_rede(&rede);
                        online.insert(rede.clone(), agora);
                    }
                    let cn = v("cn").unwrap_or_default();
                    if online[&rede].contains(&cn) {
                        "conectado"
                    } else {
                        "sem registro de saída"
                    }
                }
            };
            lista.push(Json::objeto(vec![
                ("rede", Json::texto_de(v("rede").unwrap_or_default())),
                ("login", Json::texto_de(v("login").unwrap_or_default())),
                ("ip_real", Json::texto_de(v("ip_real").unwrap_or_default())),
                ("porta_real", n("porta_real")),
                (
                    "pela_ponte",
                    Json::de_bool(v("pela_ponte").as_deref() == Some("t")),
                ),
                ("ip_vpn", Json::texto_de(v("ip_vpn").unwrap_or_default())),
                (
                    "entrou_em",
                    Json::texto_de(v("entrou_em").unwrap_or_default()),
                ),
                ("saiu_em", saiu.map(Json::texto_de).unwrap_or(Json::Nulo)),
                ("segundos", n("segundos")),
                ("bytes_do_membro", n("bytes_do_membro")),
                ("bytes_ao_membro", n("bytes_ao_membro")),
                ("estado", Json::texto_de(estado)),
            ]));
        }
        Ok(Json::objeto(vec![
            ("retencao_dias", Json::de_i64(self.retencao_do_historico()?)),
            ("todos", Json::de_bool(u.admin)),
            ("teto", Json::de_i64(TETO_LISTA)),
            ("conexoes", Json::Lista(lista)),
        ]))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    type Campos = Vec<(&'static str, String)>;

    fn ambiente<'a>(pares: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> String + 'a {
        move |n| {
            pares
                .iter()
                .find(|(k, _)| *k == n)
                .map(|(_, v)| v.to_string())
                .unwrap_or_default()
        }
    }

    /// Senha, usuario digitado, token e sessao nunca vao ao painel, mesmo
    /// presentes no ambiente. RED: montar o pedido copiando o ambiente (ou
    /// acrescentar `password` a [`CAMPOS`]) poe a senha no pedido.
    #[test]
    fn pedido_nao_leva_senha_nem_codigo() {
        let env = [
            ("script_type", "client-disconnect"),
            ("common_name", "ana.3.ab12"),
            ("trusted_ip", "198.51.100.7"),
            ("trusted_port", "40001"),
            ("time_unix", "1790000000"),
            ("time_duration", "61"),
            ("bytes_received", "1000"),
            ("bytes_sent", "2000"),
            ("password", "SCRV1:c2VuaGE=:MTIzNDU2"),
            ("username", "ana"),
            ("session_id", "segredo-do-token"),
            ("auth_control_file", "/tmp/x"),
        ];
        let p = pedido("3", ambiente(&env)).unwrap().escrever();
        for proibido in [
            "SCRV1",
            "c2VuaGE",
            "MTIzNDU2",
            "segredo-do-token",
            "password",
            "username",
            "session",
        ] {
            assert!(!p.contains(proibido), "{proibido} vazou: {p}");
        }
        assert!(p.contains("\"bytes_ao_membro\":\"2000\""), "{p}");
        // O gancho de verdade le so a lista fechada.
        for c in CAMPOS {
            assert!(!["password", "username", "session_id", "auth_token"].contains(&c));
        }
    }

    #[test]
    fn so_os_dois_ganchos_viram_evento() {
        assert!(pedido("1", ambiente(&[("script_type", "up")])).is_none());
        let e = pedido(
            "1",
            ambiente(&[
                ("script_type", "client-connect"),
                ("common_name", "joao.silva.1.ff"),
                ("trusted_ip6", "2001:db8::7"),
                ("trusted_port", "5"),
                ("ifconfig_pool_remote_ip", "10.77.1.3"),
                ("time_unix", "1790000000"),
                ("bytes_sent", "9"),
            ]),
        )
        .unwrap();
        let ev = evento(&e).unwrap();
        assert!(!ev.saiu);
        assert_eq!(ev.login, "joao.silva");
        assert_eq!(ev.ip, "2001:db8::7");
        assert_eq!(ev.ip_vpn, "10.77.1.3");
        // Contador so na saida: na entrada nao ha o que contar.
        assert_eq!(ev.bytes_ao_membro, None);
    }

    /// O painel confere tudo que chega: CN de outra rede, IP que nao e IP e
    /// numero que nao e numero nao viram linha. RED: sem a conferencia da
    /// rede do CN, o `openvpn` de uma rede escreveria no historico da outra.
    #[test]
    fn evento_torto_nao_vira_linha() {
        let base = |mudar: &dyn Fn(&mut Campos)| {
            let mut c: Campos = vec![
                ("tipo", "saiu".into()),
                ("rede", "3".into()),
                ("cn", "ana.3.ab12".into()),
                ("ip", "198.51.100.7".into()),
                ("porta", "40001".into()),
                ("ip_vpn", "10.77.3.2".into()),
                ("desde", "1790000000".into()),
                ("duracao", "61".into()),
                ("bytes_do_membro", "1000".into()),
                ("bytes_ao_membro", "2000".into()),
            ];
            mudar(&mut c);
            evento(&Json::objeto(
                c.into_iter().map(|(k, v)| (k, Json::texto_de(v))).collect(),
            ))
        };
        let certo = base(&|_| {}).unwrap();
        assert_eq!(certo.duracao, Some(61));
        assert_eq!(certo.bytes_do_membro, Some(1000));
        let troca = |k: &'static str, v: &'static str| {
            move |c: &mut Campos| c.iter_mut().find(|(n, _)| *n == k).unwrap().1 = v.into()
        };
        assert!(base(&troca("rede", "4"))
            .unwrap_err()
            .contains("outra rede"));
        assert!(base(&troca("ip", "1.2.3.4'; DROP")).is_err());
        assert!(base(&troca("porta", "70000")).is_err());
        assert!(base(&troca("desde", "")).is_err());
        assert!(base(&troca("bytes_ao_membro", "-1")).is_err());
        assert!(base(&troca("ip_vpn", "10.77.3.x")).is_err());
        assert!(base(&troca("tipo", "outro")).is_err());
        assert!(base(&troca("cn", "ana")).is_err());
    }

    /// Quem caiu para o TCP chega ao `openvpn` como `127.x.y.z`; o IP que
    /// vai ao historico e o de fora, do mapa da propria ponte. RED: sem a
    /// consulta ao `queda_tcp::origem_real`, `origem` devolve o loopback.
    #[test]
    fn quem_veio_pela_ponte_fica_com_o_ip_de_fora() {
        use std::io::Write;
        use std::sync::{Arc, Mutex};
        let d = std::env::temp_dir().join(format!("phxvpn-hist-ponte-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let reg = crate::supervisor::Registro::abrir(&d.join("openvpn.log"), 1 << 20, 1).unwrap();
        let eco = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        eco.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let ponte = crate::queda_tcp::Ponte::abrir(
            crate::queda_tcp::Conf {
                porta: 0,
                udp: eco.local_addr().unwrap().port(),
                port_share: None,
            },
            Arc::new(Mutex::new(reg)),
        )
        .unwrap();
        let mut c = std::net::TcpStream::connect(("127.0.0.1", ponte.porta())).unwrap();
        let de_fora = c.local_addr().unwrap();
        // Um primeiro quadro de cliente OpenVPN de verdade (tamanho 14..255 e
        // opcode HARD_RESET_CLIENT_V2): a ponte fecha o que nao abre assim.
        let mut q = vec![0, 20];
        q.extend_from_slice(&[0x38; 20]);
        c.write_all(&q).unwrap();
        let (_, vista) = eco.recv_from(&mut [0u8; 64]).unwrap();
        let (ip, porta, pela_ponte) = origem(&vista.ip().to_string(), vista.port());
        assert!(pela_ponte, "{vista} nao foi achado na ponte");
        assert_eq!((ip, porta), (de_fora.ip().to_string(), de_fora.port()));
        // Quem nao veio pela ponte fica como o openvpn o viu.
        assert_eq!(
            origem("198.51.100.7", 40001),
            ("198.51.100.7".to_string(), 40001, false)
        );
        drop(ponte);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn conf_liga_os_dois_ganchos_no_mesmo_soquete() {
        let c = conf(Path::new("/var/lib/phxvpn"), "7");
        if cfg!(windows) {
            assert!(c.is_empty());
            return;
        }
        assert!(c.contains("script-security 2\n"));
        assert!(c.contains("client-connect \""));
        assert!(c.contains("client-disconnect \""));
        assert!(c.contains("ovpn-historico /var/lib/phxvpn/verificar.sock 7\"\n"));
        // UM soquete so: o do verificador.
        assert_eq!(
            crate::verificar::socket(Path::new("/var/lib/phxvpn")),
            Path::new("/var/lib/phxvpn/verificar.sock")
        );
    }
}
