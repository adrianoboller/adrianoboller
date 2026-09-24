//! `tls-crypt-v2 ... force-cookie` por rede, escolhido pelo administrador.
//!
//! # Por que pedida, e nao imposta
//!
//! Com a opcao, o servidor so guarda estado de quem devolve o cookie (aperto
//! sem estado contra datagrama forjado). O preco e trancar fora todo cliente
//! que nao manda o cookie: OpenVPN anterior a 2.6 e o OpenVPN 3 core anterior
//! ao 3.8 (onde vive o OpenVPN Connect). E o 2.5 ainda e o que as
//! distribuicoes LTS empacotam -- Ubuntu 22.04: 2.5.11
//! (packages.ubuntu.com/jammy/openvpn); RHEL/Rocky 9 pelo EPEL: 2.5.11; RHEL/
//! Rocky 8 pelo EPEL: 2.4.12 (mdapi.fedoraproject.org, 24/09/2026). Ligada
//! sempre, a rede deixaria membros de fora de um dia para o outro sem
//! ninguem ter pedido. Entao nasce desligada, e o administrador liga com o
//! aviso na frente.
//!
//! So vale em rede `tls-crypt-v2` e em UDP: a v1 nao tem cookie, e em TCP so
//! o `mudp.c:122` le a opcao -- ligar ali seria um interruptor sem efeito, e
//! configuracao que nao e lida mente.

use crate::painel::{Painel, Usuario};
use crate::pg::R;
use phxsql_core::json::Json;
use std::path::PathBuf;

/// O aviso que a tela e a API mostram antes de ligar.
pub const AVISO: &str =
    "clientes OpenVPN anteriores à 2.6 e Connect com núcleo anterior à 3.8 deixam de conectar";

/// Por que a rede nao aceita a opcao, ou `None` quando aceita.
pub fn impedimento(v2: bool, tcp: bool) -> Option<&'static str> {
    if !v2 {
        Some("a rede usa tls-crypt v1, que não tem cookie")
    } else if tcp {
        Some("em TCP o OpenVPN não lê a opção (o próprio TCP já prova o endereço)")
    } else {
        None
    }
}

impl Painel {
    /// (nome, v2, tcp, ligado) da rede. O v2 sai da chave ja escrita na
    /// pasta da rede: a selada no banco so abre com o cofre destrancado.
    fn cookie_da_rede(&mut self, rede_id: i64) -> R<(String, bool, bool, bool)> {
        let id = rede_id.to_string();
        let r = self.pg()?.executar(
            "SELECT nome, protocolo, force_cookie FROM phx_rede WHERE id = $1::int",
            &[Some(&id)],
        )?;
        let nome = r.valor(0, "nome").ok_or("rede inexistente")?.to_string();
        let chave =
            std::fs::read_to_string(self.dir_rede(&id).join("tls-crypt.key")).unwrap_or_default();
        Ok((
            nome,
            crate::ovpn::e_v2(&chave),
            r.valor(0, "protocolo") == Some("tcp"),
            r.valor(0, "force_cookie") == Some("t"),
        ))
    }

    /// O estado da opcao para a tela: so o administrador ve (e so ele muda).
    pub fn cookie(&mut self, u: &Usuario, rede_id: i64) -> R<Json> {
        if !u.admin {
            return Err("so o administrador ve o force-cookie da rede".into());
        }
        let (_, v2, tcp, ligado) = self.cookie_da_rede(rede_id)?;
        Ok(Json::objeto(vec![
            ("force_cookie", Json::de_bool(ligado)),
            (
                "impedimento",
                impedimento(v2, tcp)
                    .map(Json::texto_de)
                    .unwrap_or(Json::Nulo),
            ),
            ("aviso", Json::texto_de(AVISO)),
        ]))
    }

    /// Grava e reescreve os arquivos da rede. Devolve o que o supervisor
    /// precisa para reiniciar e o valor anterior (para voltar atras).
    pub fn cookie_definir(
        &mut self,
        u: &Usuario,
        rede_id: i64,
        liga: bool,
    ) -> R<(String, PathBuf, bool)> {
        if !u.admin {
            return Err("so o administrador muda o force-cookie da rede".into());
        }
        let (nome, v2, tcp, anterior) = self.cookie_da_rede(rede_id)?;
        if liga {
            if let Some(m) = impedimento(v2, tcp) {
                return Err(format!("force-cookie não se aplica: {m}"));
            }
        }
        let id = rede_id.to_string();
        self.cookie_escrever(&id, liga)?;
        match self.materializar_rede(&id) {
            Ok(dir) => Ok((nome, dir, anterior)),
            Err(e) => {
                let _ = self.cookie_escrever(&id, anterior);
                let _ = self.materializar_rede(&id);
                Err(e)
            }
        }
    }

    fn cookie_escrever(&mut self, id: &str, liga: bool) -> R<()> {
        self.pg()?.executar(
            "UPDATE phx_rede SET force_cookie = $2::boolean WHERE id = $1::int",
            &[Some(id), Some(if liga { "true" } else { "false" })],
        )?;
        Ok(())
    }
}

/// Grava, reescreve e reinicia o OpenVPN da rede; se ele nao subir, volta ao
/// valor anterior (banco, arquivos e processo).
pub fn definir_e_aplicar(
    e: &crate::http::Estado,
    u: &Usuario,
    rede_id: i64,
    liga: bool,
) -> R<Json> {
    let (nome, dir, anterior) = e.painel().cookie_definir(u, rede_id, liga)?;
    let reiniciado = match &e.supervisor {
        Some(s) => {
            if let Err(m) = s.reiniciar(&nome, &dir) {
                let mut p = e.painel();
                let _ = p.cookie_escrever(&rede_id.to_string(), anterior);
                let _ = p.materializar_rede(&rede_id.to_string());
                drop(p);
                let _ = s.reiniciar(&nome, &dir);
                return Err(format!("o force-cookie nao entrou: {m}"));
            }
            true
        }
        None => false,
    };
    Ok(Json::objeto(vec![
        ("force_cookie", Json::de_bool(liga)),
        ("openvpn_reiniciado", Json::de_bool(reiniciado)),
        ("aviso", Json::texto_de(if liga { AVISO } else { "" })),
    ]))
}

#[cfg(test)]
mod testes {
    use super::*;

    /// So rede v2 em UDP aceita ligar. RED: sem o impedimento, a rede TCP
    /// ou v1 grava um interruptor que o OpenVPN nao le.
    #[test]
    fn so_v2_em_udp_aceita() {
        assert_eq!(impedimento(true, false), None);
        assert!(impedimento(false, false).unwrap().contains("v1"));
        assert!(impedimento(true, true).unwrap().contains("TCP"));
        assert!(AVISO.contains("2.6") && AVISO.contains("3.8"));
    }
}
