//! Saida da rede no modo servidor: TUNEL TOTAL (toda a internet do membro
//! pelo servidor) e DNS EMPURRADO (o da empresa, ou os nomes dos membros
//! pelo resolvedor do `dns.rs`). Lacunas, itens «Tunel total» e «DNS
//! empurrado + nomes dos membros».
//!
//! # O pacote inteiro, ou nada
//!
//! `redirect-gateway` sozinho vaza: o Windows continua perguntando ao DNS da
//! placa de fora (a pergunta sai pela rede local, em claro), e o IPv6 do
//! membro continua saindo direto, porque o tunel so leva IPv4. Por isso a
//! chave «tunel total» escreve SEMPRE o conjunto (manual da 2.6.19,
//! vpn-network-options.rst:12-45 e 305-364; windows-options.rst:13-24):
//!
//! - `redirect-gateway def1 ipv6` (+ `block-local` quando o admin pede): as
//!   duas metades `0/1` e `128/1` e o espaco IPv6 publico pelo tunel;
//! - `ifconfig-ipv6` ficticio + `block-ipv6` (no servidor e empurrado): o
//!   IPv6 entra no tunel e morre ali com «sem rota», em vez de sair pela
//!   placa de fora. E a receita do proprio manual;
//! - `block-outside-dns` EMPURRADO, nunca no perfil: o Windows bloqueia a
//!   porta 53 fora do tunel; no Linux e no macOS a opcao e desconhecida, e
//!   opcao desconhecida EMPURRADA nao e fatal (medido: «Options error ...
//!   block-outside-dns», e o cliente segue);
//! - DNS pela VPN obrigatorio: com o `block-outside-dns` o Windows so tem o
//!   DNS do tunel; tunel total sem DNS empurrado ficaria sem nome nenhum;
//! - NAT de saida e guarda no servidor (`rotas.rs`, a MESMA tabela): a rede
//!   sai para a internet e so para ela -- faixa privada fica de fora, porque
//!   LAN pela VPN e so por rota, que so o admin inclui.
//!
//! # O preco medido do `ifconfig-ipv6`
//!
//! Cliente Linux com o IPv6 desligado no kernel (`ipv6.disable=1`) MORRE ao
//! receber `ifconfig-ipv6`: «Linux can't add IPv6 to interface tun0»,
//! «Exiting due to fatal error» (medido nesta maquina, 2.6.19). Nao ha como
//! empurrar condicional; quem baixa o perfil numa maquina assim marca «sem
//! IPv6», e o perfil leva `pull-filter ignore "ifconfig-ipv6"` -- ali nao
//! existe IPv6 para vazar. As rotas IPv6 do `redirect-gateway ipv6` falham
//! sem derrubar ninguem (medido: «ERROR: Linux route add command failed» e
//! «Initialization Sequence Completed»).
//!
//! # DNS no cliente Linux
//!
//! O OpenVPN no Linux nao aplica `dhcp-option` sozinho: so a poe no ambiente
//! do script `up` (vpn-network-options.rst:126-133). O perfil leva
//! `script-security 2` + `up`/`down` SO quando quem baixa pede
//! (`dns_linux`): executar script e decisao da maquina do membro, nao do
//! servidor.

use crate::painel::{Painel, Usuario};
use crate::pg::R;
use crate::rotas::{Cidr, Rota, ESPACO_VPN};
use phxsql_core::json::Json;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

pub const ESQUEMA: &str = "
ALTER TABLE phx_rede ADD COLUMN IF NOT EXISTS tunel_total boolean NOT NULL DEFAULT false;
ALTER TABLE phx_rede ADD COLUMN IF NOT EXISTS bloquear_local boolean NOT NULL DEFAULT false;
ALTER TABLE phx_rede ADD COLUMN IF NOT EXISTS dns_nomes boolean NOT NULL DEFAULT false;
ALTER TABLE phx_rede ADD COLUMN IF NOT EXISTS dns_empresa text NOT NULL DEFAULT '';
";

/// O que o roteador da filial NAO recebe do tunel total (`rotas::ccd_membro`).
pub const FORA_DO_ROTEADOR_DA_FILIAL: [&str; 4] = [
    "redirect-gateway",
    "ifconfig-ipv6",
    "block-ipv6",
    "block-outside-dns",
];

/// O IPv6 ficticio da placa do membro: so existe para as rotas IPv6 terem
/// onde apontar e o `block-ipv6` responder. ULA, nunca roteado. Todos os
/// membros recebem o mesmo -- nenhum pacote IPv6 passa do servidor.
const IPV6_FICTICIO: &str = "fd70:6878:766e::2/64 fd70:6878:766e::1";

/// Quantos DNS da empresa (o Windows e o OpenVPN aceitam mais, mas dois ja
/// sao o primario e o reserva).
const TETO_DNS: usize = 2;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Saida {
    pub tunel_total: bool,
    pub bloquear_local: bool,
    /// O resolvedor da rede (`10.77.N.1`), com os nomes dos membros.
    pub dns_nomes: bool,
    /// DNS da empresa: empurrado direto, ou o de cima do resolvedor.
    pub dns_empresa: Vec<Ipv4Addr>,
}

/// `a.b.c.d`, separados por espaco ou virgula, ate dois. So unicast de
/// verdade: 0, loopback, link-local, multicast e broadcast nao sao DNS de
/// ninguem, e o espaco da VPN e o do proprio resolvedor.
pub fn analisar_dns(t: &str) -> Result<Vec<Ipv4Addr>, String> {
    let mut v = Vec::new();
    for p in t.split([' ', ',']).filter(|p| !p.is_empty()) {
        let ip: Ipv4Addr = p
            .parse()
            .map_err(|_| format!("«{p}»: DNS e um endereco IPv4 (ex. 192.168.10.53)"))?;
        if ip.is_unspecified()
            || ip.is_loopback()
            || ip.is_link_local()
            || ip.is_multicast()
            || ip.is_broadcast()
        {
            return Err(format!("{ip} nao e endereco de um servidor DNS"));
        }
        if ESPACO_VPN.contem_ip(u32::from(ip)) {
            return Err(format!(
                "{ip} esta no espaco da VPN: para os nomes dos membros, ligue «nomes dos membros»"
            ));
        }
        if !v.contains(&ip) {
            v.push(ip);
        }
    }
    if v.len() > TETO_DNS {
        return Err(format!("no maximo {TETO_DNS} DNS da empresa"));
    }
    Ok(v)
}

/// A combinacao, contra as rotas da propria rede.
pub fn validar(s: &Saida, rotas: &[Rota]) -> Result<(), String> {
    if s.bloquear_local && !s.tunel_total {
        return Err(
            "bloquear a LAN local e parte do tunel total (block-local do redirect-gateway): ligue o tunel total"
                .into(),
        );
    }
    if s.tunel_total && !s.dns_nomes && s.dns_empresa.is_empty() {
        return Err(
            "tunel total pede DNS pela VPN: com o block-outside-dns o Windows so pergunta ao DNS do tunel -- ligue os nomes dos membros ou informe o DNS da empresa"
                .into(),
        );
    }
    // Sem o resolvedor, quem pergunta ao DNS da empresa e o MEMBRO: um DNS
    // em faixa privada tem de estar numa rota da rede, senao a pergunta vai
    // para a LAN de casa dele (ou, com tunel total, e descartada no servidor).
    if !s.dns_nomes {
        for ip in &s.dns_empresa {
            let n = u32::from(*ip);
            if crate::rotas::privada(n) && !rotas.iter().any(|r| r.cidr.contem_ip(n)) {
                return Err(format!(
                    "o DNS {ip} e de faixa privada e nao esta em nenhuma rota desta rede: inclua a rede dele em «redes alcancaveis», ou ligue os nomes dos membros (o servidor pergunta por eles)"
                ));
            }
        }
    }
    Ok(())
}

/// Alarga o que a rede faz pelo servidor? Isso e do administrador; desligar
/// (estreitar) o dono tambem pode.
pub fn amplia(antes: &Saida, depois: &Saida) -> bool {
    (depois.tunel_total && !antes.tunel_total)
        || (depois.dns_nomes && !antes.dns_nomes)
        || (!depois.dns_empresa.is_empty() && depois.dns_empresa != antes.dns_empresa)
}

/// O IP do servidor dentro do tunel da rede: onde o resolvedor escuta.
pub fn ip_do_resolvedor(octeto: u8) -> Ipv4Addr {
    Ipv4Addr::new(10, 77, octeto, 1)
}

/// O DNS que os membros recebem.
pub fn dns_empurrado(s: &Saida, octeto: u8) -> Vec<Ipv4Addr> {
    if s.dns_nomes {
        vec![ip_do_resolvedor(octeto)]
    } else {
        s.dns_empresa.clone()
    }
}

/// O trecho do `servidor.conf` de uma rede.
pub fn conf_servidor(s: &Saida, octeto: u8, zona: &str) -> String {
    let mut c = String::new();
    if s.tunel_total {
        c.push_str(&format!(
            "push \"redirect-gateway def1 ipv6{}\"\n",
            if s.bloquear_local { " block-local" } else { "" }
        ));
        c.push_str(&format!("push \"ifconfig-ipv6 {IPV6_FICTICIO}\"\n"));
        c.push_str("push \"block-ipv6\"\nblock-ipv6\n");
        c.push_str("push \"block-outside-dns\"\n");
    }
    for ip in dns_empurrado(s, octeto) {
        c.push_str(&format!("push \"dhcp-option DNS {ip}\"\n"));
    }
    if s.dns_nomes {
        c.push_str(&format!("push \"dhcp-option DOMAIN {zona}\"\n"));
    }
    if !c.is_empty() {
        c.insert_str(0, "# saida da rede (painel: tunel total e DNS)\n");
    }
    c
}

/// Como o cliente Linux aplica o DNS empurrado.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DnsLinux {
    /// `update-resolv-conf` do pacote `openvpn` (pede o `resolvconf`).
    Resolvconf,
    /// `update-systemd-resolved` (pacote `openvpn-systemd-resolved`).
    SystemdResolved,
}

/// O que quem baixa o perfil pede para a PROPRIA maquina.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cliente {
    pub dns_linux: Option<DnsLinux>,
    pub sem_ipv6: bool,
}

/// Do pedido: so valores da lista -- o texto vira diretiva no perfil, e um
/// valor livre injetaria `up /bin/sh ...` no arquivo de alguem.
pub fn cliente(dns_linux: &str, sem_ipv6: bool) -> Result<Cliente, String> {
    let dns_linux = match dns_linux {
        "" => None,
        "resolvconf" => Some(DnsLinux::Resolvconf),
        "systemd-resolved" => Some(DnsLinux::SystemdResolved),
        _ => return Err("dns_linux: resolvconf, systemd-resolved ou vazio".into()),
    };
    Ok(Cliente {
        dns_linux,
        sem_ipv6,
    })
}

/// As linhas a mais do perfil, pelo que a maquina pediu e pela saida da
/// rede.
pub fn perfil_cliente(c: &Cliente, s: &Saida) -> String {
    let mut p = String::new();
    if let Some(d) = c.dns_linux {
        let script = match d {
            DnsLinux::Resolvconf => "/etc/openvpn/update-resolv-conf",
            DnsLinux::SystemdResolved => "/etc/openvpn/update-systemd-resolved",
        };
        p.push_str(&format!(
            "# DNS no Linux (pedido por quem baixou): o OpenVPN so entrega o\n\
             # dhcp-option ao script\n\
             script-security 2\nup {script}\ndown {script}\n"
        ));
        if d == DnsLinux::SystemdResolved {
            p.push_str("down-pre\n");
            // Com tunel total, TODA pergunta vai ao DNS do tunel; sem ele, so
            // a zona da rede (o DOMAIN empurrado).
            if s.tunel_total {
                p.push_str("dhcp-option DOMAIN-ROUTE .\n");
            }
        }
    }
    if c.sem_ipv6 {
        p.push_str(
            "# maquina sem IPv6: o ifconfig-ipv6 do tunel total derrubaria a conexao\n\
             pull-filter ignore \"ifconfig-ipv6\"\n",
        );
    }
    p
}

impl Painel {
    /// (saida, octeto, nome) de uma rede.
    fn saida_de(&mut self, rede_id: &str) -> R<(Saida, u8, String)> {
        let r = self.pg()?.executar(
            "SELECT nome, octeto, tunel_total, bloquear_local, dns_nomes, dns_empresa \
             FROM phx_rede WHERE id = $1::int",
            &[Some(rede_id)],
        )?;
        let v = |c: &str| r.valor(0, c).unwrap_or_default().to_string();
        let nome = r.valor(0, "nome").ok_or("rede inexistente")?.to_string();
        Ok((
            Saida {
                tunel_total: v("tunel_total") == "t",
                bloquear_local: v("bloquear_local") == "t",
                dns_nomes: v("dns_nomes") == "t",
                dns_empresa: analisar_dns(&v("dns_empresa"))?,
            },
            v("octeto").parse().map_err(|_| "octeto invalido")?,
            nome,
        ))
    }

    /// A saida da rede pelo NOME (o perfil de quem entra).
    pub fn saida_por_nome(&mut self, nome: &str) -> R<Saida> {
        let r = self
            .pg()?
            .executar("SELECT id FROM phx_rede WHERE nome = $1", &[Some(nome)])?;
        let id = r.valor(0, "id").ok_or("rede inexistente")?.to_string();
        Ok(self.saida_de(&id)?.0)
    }

    /// O trecho da saida no `servidor.conf` (chamado pelo `materializar_rede`).
    pub(crate) fn conf_da_saida(&mut self, rede_id: &str) -> R<String> {
        let (s, octeto, nome) = self.saida_de(rede_id)?;
        Ok(conf_servidor(&s, octeto, &crate::dns::zona(&nome, octeto)))
    }

    /// A saida de uma rede, para quem a administra ou a integra.
    pub fn saida(&mut self, u: &Usuario, rede_id: i64) -> R<Json> {
        self.pode_ver_rede(u, rede_id)?;
        let (s, octeto, nome) = self.saida_de(&rede_id.to_string())?;
        let lista = |v: Vec<Ipv4Addr>| {
            Json::texto_de(
                v.iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        };
        Ok(Json::objeto(vec![
            ("tunel_total", Json::de_bool(s.tunel_total)),
            ("bloquear_local", Json::de_bool(s.bloquear_local)),
            ("dns_nomes", Json::de_bool(s.dns_nomes)),
            ("dns_empresa", lista(s.dns_empresa.clone())),
            ("zona", Json::texto_de(crate::dns::zona(&nome, octeto))),
            ("dns_empurrado", lista(dns_empurrado(&s, octeto))),
        ]))
    }

    /// Muda a saida. ALARGAR (ligar tunel total, os nomes, trocar o DNS) e so
    /// do administrador: tunel total faz do servidor a saida de internet dos
    /// membros, e qualquer usuario e dono de ate 3 redes. Desligar, o dono
    /// tambem pode. Devolve (nome, pasta, saida anterior) para o reinicio e
    /// o desfazer.
    pub fn saida_definir(
        &mut self,
        ator: &Usuario,
        rede_id: i64,
        nova: &Saida,
    ) -> R<(String, PathBuf, Saida)> {
        let id = rede_id.to_string();
        let r = self.pg()?.executar(
            "SELECT dono_id FROM phx_rede WHERE id = $1::int",
            &[Some(&id)],
        )?;
        let dono: i64 = r
            .valor(0, "dono_id")
            .and_then(|v| v.parse().ok())
            .ok_or("rede inexistente")?;
        let (antiga, _, nome) = self.saida_de(&id)?;
        if amplia(&antiga, nova) && !ator.admin {
            return Err(
                "so o administrador liga o tunel total, os nomes dos membros ou o DNS da rede"
                    .into(),
            );
        }
        if !ator.admin && ator.id != dono {
            return Err("so o administrador ou o dono da rede muda a saida".into());
        }
        validar(nova, &self.rotas_da_rede(&id)?)?;
        self.saida_gravar(rede_id, nova)?;
        match self.materializar_rede(&id) {
            Ok(dir) => Ok((nome, dir, antiga)),
            Err(e) => {
                let _ = self.saida_gravar(rede_id, &antiga);
                let _ = self.materializar_rede(&id);
                Err(e)
            }
        }
    }

    /// Grava sem conferir (o desfazer volta ao que ja estava gravado).
    pub fn saida_gravar(&mut self, rede_id: i64, s: &Saida) -> R<()> {
        let dns = s
            .dns_empresa
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        self.pg()?.executar(
            "UPDATE phx_rede SET tunel_total = $2::boolean, bloquear_local = $3::boolean, \
             dns_nomes = $4::boolean, dns_empresa = $5 WHERE id = $1::int",
            &[
                Some(&rede_id.to_string()),
                Some(if s.tunel_total { "true" } else { "false" }),
                Some(if s.bloquear_local { "true" } else { "false" }),
                Some(if s.dns_nomes { "true" } else { "false" }),
                Some(&dns),
            ],
        )?;
        Ok(())
    }

    /// A /24 de cada rede com tunel total (o firewall e um so no host).
    pub fn origens_do_tunel_total(&mut self) -> R<Vec<Cidr>> {
        let r = self.pg()?.executar(
            "SELECT octeto FROM phx_rede WHERE tunel_total ORDER BY octeto",
            &[],
        )?;
        Ok((0..r.linhas.len())
            .filter_map(|i| r.valor(i, "octeto")?.parse::<u8>().ok())
            .map(|o| Cidr {
                rede: ESPACO_VPN.rede | (u32::from(o) << 8),
                prefixo: 24,
            })
            .collect())
    }

    /// Os resolvedores que devem estar no ar: um por rede com os nomes.
    /// `repasse_sistema`: o DNS do proprio servidor, para quem nao tem o da
    /// empresa.
    pub fn resolvedores(
        &mut self,
        repasse_sistema: Option<SocketAddr>,
    ) -> R<Vec<crate::dns::Config>> {
        let r = self.pg()?.executar(
            "SELECT id, nome, octeto, dns_empresa FROM phx_rede WHERE dns_nomes ORDER BY id",
            &[],
        )?;
        let mut v = Vec::new();
        for i in 0..r.linhas.len() {
            let c = |k: &str| r.valor(i, k).unwrap_or_default().to_string();
            let octeto: u8 = c("octeto").parse().map_err(|_| "octeto invalido")?;
            let empresa = analisar_dns(&c("dns_empresa"))?;
            v.push(crate::dns::Config {
                escuta: SocketAddr::new(IpAddr::V4(ip_do_resolvedor(octeto)), 53),
                aceita: Cidr {
                    rede: ESPACO_VPN.rede | (u32::from(octeto) << 8),
                    prefixo: 24,
                },
                zona: crate::dns::zona(&c("nome"), octeto),
                ccd: self.dir_rede(&c("id")).join("ccd"),
                repasse: empresa
                    .first()
                    .map(|ip| SocketAddr::new(IpAddr::V4(*ip), 53))
                    .or(repasse_sistema),
            });
        }
        Ok(v)
    }
}

/// Sobe/para os resolvedores pelo cadastro. So com o supervisor: sem ele o
/// OpenVPN nao e deste painel e o `10.77.N.1` nao existe aqui. Falha fica no
/// log -- o DNS nao derruba a rede.
pub fn acertar_dns(e: &crate::http::Estado) {
    if e.supervisor.is_none() {
        return;
    }
    let sistema = std::fs::read_to_string("/etc/resolv.conf")
        .ok()
        .and_then(|t| crate::dns::dns_do_sistema(&t));
    match e.painel().resolvedores(sistema) {
        Ok(cfgs) => crate::dns::acertar(cfgs),
        Err(m) => eprintln!("phxvpn: AVISO dns: {m}"),
    }
}

/// Depois de gravar: firewall (NAT de saida), resolvedores e OpenVPN, pelo
/// mesmo passo das rotas. Alargar que nao se aplica volta ao que era.
pub fn depois_de_mudar(
    e: &crate::http::Estado,
    rede_id: i64,
    antiga: &Saida,
    nova: &Saida,
    nome: &str,
    dir: &std::path::Path,
) -> R<Json> {
    let desfazer = || {
        let mut p = e.painel();
        let _ = p.saida_gravar(rede_id, antiga);
        let _ = p.materializar_rede(&rede_id.to_string());
    };
    let alarga = amplia(antiga, nova);
    let mut j = crate::rotas::efetivar(
        e,
        alarga.then_some(&desfazer as &dyn Fn()),
        "a saida",
        nome,
        dir,
    )?;
    if let Json::Objeto(campos) = &mut j {
        campos.push((
            "aviso".into(),
            Json::texto_de(
                "os membros recebem a mudança ao reconectar; no Linux, o DNS só se aplica com o perfil baixado pedindo o script (dns_linux)",
            ),
        ));
    }
    Ok(j)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn s(tt: bool, bl: bool, nomes: bool, dns: &str) -> Saida {
        Saida {
            tunel_total: tt,
            bloquear_local: bl,
            dns_nomes: nomes,
            dns_empresa: analisar_dns(dns).unwrap(),
        }
    }

    fn rota(cidr: &str) -> Rota {
        Rota {
            rede_id: 1,
            octeto: 3,
            cidr: Cidr::analisar(cidr).unwrap(),
            membro: None,
            login: None,
            nat: true,
        }
    }

    /// O pacote inteiro: sem qualquer uma das pecas, o tunel total vaza DNS
    /// ou IPv6 -- cada linha tem de estar la.
    #[test]
    fn tunel_total_leva_o_pacote_inteiro() {
        let c = conf_servidor(&s(true, false, true, ""), 3, "matriz.phx");
        for linha in [
            "push \"redirect-gateway def1 ipv6\"\n",
            "push \"ifconfig-ipv6 fd70:6878:766e::2/64 fd70:6878:766e::1\"\n",
            "push \"block-ipv6\"\n",
            "\nblock-ipv6\n",
            "push \"block-outside-dns\"\n",
            "push \"dhcp-option DNS 10.77.3.1\"\n",
            "push \"dhcp-option DOMAIN matriz.phx\"\n",
        ] {
            assert!(c.contains(linha), "faltou {linha:?} em:\n{c}");
        }
        assert!(!c.contains("block-local"), "{c}");
        let c = conf_servidor(&s(true, true, false, "1.1.1.1 9.9.9.9"), 3, "matriz.phx");
        assert!(
            c.contains("push \"redirect-gateway def1 ipv6 block-local\"\n"),
            "{c}"
        );
        assert!(c.contains("push \"dhcp-option DNS 1.1.1.1\"\npush \"dhcp-option DNS 9.9.9.9\"\n"));
        assert!(!c.contains("DOMAIN"), "sem os nomes nao ha zona: {c}");
    }

    /// Sem tunel total: so o DNS, sem mexer na rota nem no IPv6 de ninguem.
    #[test]
    fn so_dns_nao_mexe_na_rota_de_ninguem() {
        let c = conf_servidor(&s(false, false, true, ""), 3, "matriz.phx");
        assert!(c.contains("push \"dhcp-option DNS 10.77.3.1\"\n"), "{c}");
        for nunca in ["redirect-gateway", "ipv6", "block-outside-dns"] {
            assert!(!c.contains(nunca), "{nunca} em:\n{c}");
        }
        assert!(conf_servidor(&Saida::default(), 3, "matriz.phx").is_empty());
    }

    #[test]
    fn combinacao_torta_e_recusada_com_o_motivo() {
        let e = validar(&s(true, false, false, ""), &[]).unwrap_err();
        assert!(e.contains("pede DNS"), "{e}");
        let e = validar(&s(false, true, true, ""), &[]).unwrap_err();
        assert!(e.contains("parte do tunel total"), "{e}");
        // DNS privado sem rota e sem o resolvedor: o membro nao o alcancaria.
        let e = validar(&s(true, false, false, "192.168.10.53"), &[]).unwrap_err();
        assert!(e.contains("nenhuma rota"), "{e}");
        validar(
            &s(true, false, false, "192.168.10.53"),
            &[rota("192.168.10.0/24")],
        )
        .unwrap();
        // Com o resolvedor, quem pergunta e o servidor: pode.
        validar(&s(true, false, true, "192.168.10.53"), &[]).unwrap();
        validar(&s(true, false, false, "1.1.1.1"), &[]).unwrap();
        validar(&s(false, false, false, ""), &[]).unwrap();
    }

    #[test]
    fn dns_da_empresa_so_aceita_servidor_de_verdade() {
        for ruim in [
            "0.0.0.0",
            "127.0.0.1",
            "224.0.0.251",
            "255.255.255.255",
            "169.254.1.1",
            "10.77.1.1",
            "1.1.1.1 8.8.8.8 9.9.9.9",
            "1.1.1.1\npush \"route 0.0.0.0 0.0.0.0\"",
            "192.168.010.1",
            "exemplo.com",
        ] {
            assert!(analisar_dns(ruim).is_err(), "{ruim:?} passou");
        }
        assert_eq!(
            analisar_dns(" 1.1.1.1, 192.168.10.53 ").unwrap(),
            vec![Ipv4Addr::new(1, 1, 1, 1), Ipv4Addr::new(192, 168, 10, 53)]
        );
        assert!(analisar_dns("").unwrap().is_empty());
    }

    #[test]
    fn so_ligar_alarga() {
        let nada = Saida::default();
        assert!(amplia(&nada, &s(true, false, true, "")));
        assert!(amplia(&nada, &s(false, false, true, "")));
        assert!(amplia(&nada, &s(false, false, false, "1.1.1.1")));
        assert!(amplia(
            &s(false, false, false, "1.1.1.1"),
            &s(false, false, false, "8.8.8.8")
        ));
        assert!(!amplia(&s(true, true, true, "1.1.1.1"), &nada));
        assert!(!amplia(&s(true, false, true, ""), &s(true, true, true, "")));
    }

    /// O perfil so leva script quando a maquina pede, e so da lista.
    #[test]
    fn perfil_do_cliente_so_leva_o_que_foi_pedido() {
        let tt = s(true, false, true, "");
        assert!(perfil_cliente(&Cliente::default(), &tt).is_empty());
        let p = perfil_cliente(&cliente("resolvconf", false).unwrap(), &tt);
        assert!(p.contains("script-security 2\nup /etc/openvpn/update-resolv-conf\ndown /etc/openvpn/update-resolv-conf\n"), "{p}");
        let p = perfil_cliente(&cliente("systemd-resolved", false).unwrap(), &tt);
        assert!(
            p.contains("up /etc/openvpn/update-systemd-resolved\n") && p.contains("down-pre\n")
        );
        assert!(p.contains("dhcp-option DOMAIN-ROUTE .\n"), "{p}");
        let so_nomes = s(false, false, true, "");
        let p = perfil_cliente(&cliente("systemd-resolved", false).unwrap(), &so_nomes);
        assert!(
            !p.contains("DOMAIN-ROUTE"),
            "sem tunel total so a zona vai ao tunel: {p}"
        );
        let p = perfil_cliente(&cliente("", true).unwrap(), &tt);
        assert!(p.contains("pull-filter ignore \"ifconfig-ipv6\"\n") && !p.contains("script"));
        for ruim in ["sh", "resolvconf\nup /bin/sh", "/etc/openvpn/x"] {
            assert!(cliente(ruim, false).is_err(), "{ruim:?} passou");
        }
    }
}
