//! O TLS de SAIDA para servidor de fora -- PostgreSQL(R), MySQL(R) e o rele
//! SMTP (pedido 572, T6d).
//!
//! Uma decisao so, para os tres clientes: o que a configuracao pediu
//! (`tls`, `tls_ca`, `pino_tls`) vira um [`TlsDeSaida`], e e ele que passa o
//! fio para TLS pelo motor do core ([`FioDeCliente::passar_a_tls_com`]). Tres
//! copias desta leitura divergiriam no primeiro conserto.
//!
//! Os modos tem o nome e o sentido do `sslmode` do libpq, que e a regua que o
//! MySQL(R) (`--ssl-mode`) e o MariaDB repetem com outras palavras:
//!
//! | modo | libpq | o que garante |
//! |---|---|---|
//! | `desligado` (o padrao) | `disable` | nada -- como sempre foi |
//! | `exigir` | `require` | cifra contra quem ESCUTA; nao contra quem se poe no meio |
//! | `verificar` | `verify-full` | cifra, cadeia ate uma ancora do `tls_ca` e o nome do host |
//!
//! E `pino_tls` escrito vale por si: TLS conferido pela chave, sem cadeia.
//! Nasce `desligado` porque guarda nova entra pedida: uma ligacao que hoje
//! fala com um servidor sem TLS continua falando.

use phxsql_core::error::{PhxError, Result};
use phxsql_core::tls::{Confianca, FioDeCliente, OpcoesCliente};

/// O TLS pedido por um cliente de saida.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TlsDeSaida {
    Desligado,
    /// Cifra sem conferir quem responde (`sslmode=require`).
    Exigir,
    /// Cifra conferida pelo `SHA-256` do SPKI.
    Pino([u8; 32]),
    /// Cifra, cadeia ate uma destas ancoras e o nome do host.
    Verificar(Vec<Vec<u8>>),
}

impl TlsDeSaida {
    /// Le os tres campos. `onde` nomeia o campo na recusa. `tls_ca` vazio
    /// com `verificar` e o pacote do sistema -- a ancora de quem nao escreveu
    /// nenhuma e a do sistema, como no libpq (`sslrootcert=system`) e no
    /// `curl`.
    pub fn de_config(modo: &str, tls_ca: &str, pino_tls: &str, onde: &str) -> Result<TlsDeSaida> {
        let pino = match pino_tls.trim() {
            "" => None,
            t => Some(
                phxsql_core::tls::pino_de_texto(t)
                    .map_err(|e| PhxError::Esquema(format!("{onde}.pino_tls: {e}")))?,
            ),
        };
        let modo = modo.trim();
        match (modo, pino) {
            ("" | "desligado", None) => Ok(TlsDeSaida::Desligado),
            (_, Some(p)) if modo != "verificar" => Ok(TlsDeSaida::Pino(p)),
            ("exigir", None) => Ok(TlsDeSaida::Exigir),
            ("verificar", p) => {
                if p.is_some() {
                    return Err(PhxError::Esquema(format!(
                        "{onde}: pino_tls e tls \"verificar\" se contradizem -- o pino \
                         confere a CHAVE, a verificacao confere a CADEIA; escolha um"
                    )));
                }
                let ca = if tls_ca.trim().is_empty() {
                    "sistema"
                } else {
                    tls_ca
                };
                let (ancoras, _) = phxsql_core::cadeia::ancoras(ca)
                    .map_err(|e| PhxError::Esquema(format!("{onde}.tls_ca: {e}")))?;
                Ok(TlsDeSaida::Verificar(ancoras))
            }
            (outro, _) => Err(PhxError::Esquema(format!(
                "{onde}.tls: {outro:?} nao e um modo -- use \"desligado\", \"exigir\" ou \
                 \"verificar\""
            ))),
        }
    }

    pub fn ligado(&self) -> bool {
        *self != TlsDeSaida::Desligado
    }

    /// Passa o fio para TLS falando com `host`. O nome vai no SNI (menos IP
    /// literal, que a RFC 6066 §3 proibe) e e o que a verificacao confere.
    pub fn passar(
        &self,
        leitor: &mut std::io::BufReader<FioDeCliente>,
        escrita: &mut FioDeCliente,
        host: &str,
    ) -> Result<()> {
        let confianca = match self {
            TlsDeSaida::Desligado => return Ok(()),
            TlsDeSaida::Exigir => Confianca::AnotarNoPrimeiroContato,
            TlsDeSaida::Pino(p) => Confianca::Pino(*p),
            TlsDeSaida::Verificar(a) => Confianca::Cadeia(a),
        };
        let op = OpcoesCliente {
            nome: Some(host),
            alpn: &[],
            confianca,
        };
        FioDeCliente::passar_a_tls_com(leitor, escrita, &op)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn os_modos_e_as_recusas() {
        assert_eq!(
            TlsDeSaida::de_config("", "", "", "x").unwrap(),
            TlsDeSaida::Desligado
        );
        assert_eq!(
            TlsDeSaida::de_config("exigir", "", "", "x").unwrap(),
            TlsDeSaida::Exigir
        );
        let pino = phxsql_core::tls::pino_em_texto(&[3; 32]);
        assert_eq!(
            TlsDeSaida::de_config("", "", &pino, "x").unwrap(),
            TlsDeSaida::Pino([3; 32])
        );
        assert!(TlsDeSaida::de_config("verificar", "", &pino, "x").is_err());
        let e = TlsDeSaida::de_config("sim", "", "", "dblink[erp]")
            .unwrap_err()
            .to_string();
        assert!(e.contains("dblink[erp].tls"), "{e}");
        assert!(TlsDeSaida::de_config("verificar", "/nao/existe.pem", "", "x").is_err());
    }
}
