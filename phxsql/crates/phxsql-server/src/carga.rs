//! `BULKINSERT`: a tabela reservada para uma carga, e so para ela.
//!
//! # O que resolve
//!
//! Uma carga longa -- importar um arquivo, migrar de outro banco, semear um
//! ambiente -- quer duas coisas que o servidor nao dava:
//!
//! 1. **exclusividade**: ninguem mais mexendo naquela tabela enquanto ela
//!    entra, para o dado nao ficar meio velho e meio novo para quem consulta;
//! 2. **uma sincronizacao so**, no fim, em vez de uma por janela.
//!
//! E abre a porta para a terceira, que e a maior e ainda nao esta feita:
//! **adiar o indice**. A objecao registrada em `docs/DESEMPENHO.md` contra
//! adiar era que a leitura veria um indice defasado e `buscar` responderia
//! errado em silencio. Com a tabela reservada nao ha leitura para ver.
//!
//! # A parte perigosa, e as duas redes embaixo dela
//!
//! Uma trava que atravessa pedidos pode ficar ORFA: o cliente cai no meio da
//! carga e a tabela fica reservada para sempre, sem ninguem para soltar. Por
//! isso ha duas saidas, e nao uma:
//!
//! * **a queda da conexao solta** -- o laco da conexao solta o que aquela
//!   ligacao reservou, por qualquer caminho de saida;
//! * **o prazo solta** -- mesmo com o soquete pendurado vivo, a reserva expira.
//!   Soquete meio-morto existe, e e justamente o caso em que a primeira rede
//!   nao pega.
//!
//! O prazo e conferido na hora de usar, e nao por um relogio de fundo: uma
//! reserva vencida que ninguem consultou nao atrapalha ninguem, e a primeira
//! consulta a limpa.
//!
//! # Por que so pela porta de dados
//!
//! HTTP nao tem conexao para cair -- cada pedido e um. Sem ligacao a que
//! amarrar, a primeira rede de protecao nao existe, e a reserva ficaria so no
//! prazo. A interface web nao precisa disto: o `inserir_lote` dela ja e UMA
//! operacao, que roda inteira dentro da trava global.

use std::collections::HashMap;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

/// Uma tabela reservada, e por quem.
#[derive(Debug, Clone)]
pub struct Reserva {
    pub database: String,
    pub tabela: String,
    /// Login de quem reservou. Vazio num servidor sem cadastro.
    pub usuario: String,
    /// A conexao que reservou. E a ela que a reserva morre amarrada.
    pub ligacao: u64,
    pub ip: String,
    pub desde_ms: i64,
    pub expira_ms: i64,
}

impl Reserva {
    pub fn para_json(&self, agora_ms: i64) -> Json {
        Json::objeto(vec![
            ("database", Json::texto_de(&self.database)),
            ("tabela", Json::texto_de(&self.tabela)),
            ("usuario", Json::texto_de(&self.usuario)),
            ("ligacao", Json::de_u64(self.ligacao)),
            ("ip", Json::texto_de(&self.ip)),
            (
                "desde",
                Json::texto_de(phxsql_core::datahora::instante_iso(self.desde_ms)),
            ),
            (
                "ha_ms",
                Json::de_u64((agora_ms - self.desde_ms).max(0) as u64),
            ),
            (
                "expira_em_s",
                Json::de_u64(((self.expira_ms - agora_ms).max(0) / 1000) as u64),
            ),
        ])
    }
}

/// Quem esta com qual tabela reservada.
#[derive(Debug, Default)]
pub struct Cargas {
    /// Chave: `database/tabela`. Uma tabela tem no maximo uma reserva.
    dentro: HashMap<String, Reserva>,
}

/// A chave da reserva. Sem acento e sem caixa: `Clientes` e `clientes` sao a
/// mesma tabela para o sistema de arquivos em que isto roda de verdade.
pub fn chave(database: &str, tabela: &str) -> String {
    format!("{}/{}", database.to_lowercase(), tabela.to_lowercase())
}

impl Cargas {
    /// Reserva a tabela. Devolve erro se outro ja tem.
    ///
    /// Reservar de novo o que ja e seu **renova o prazo** em vez de recusar:
    /// uma carga longa que passa do prazo tem como se segurar, e um cliente
    /// que repetiu o comando nao merece um erro.
    #[allow(clippy::too_many_arguments)]
    pub fn reservar(
        &mut self,
        database: &str,
        tabela: &str,
        usuario: &str,
        ligacao: u64,
        ip: &str,
        agora_ms: i64,
        prazo_ms: i64,
    ) -> Result<Reserva> {
        let k = chave(database, tabela);
        if let Some(dono) = self.dentro.get(&k) {
            if dono.expira_ms > agora_ms && dono.ligacao != ligacao {
                return Err(PhxError::EmCarga(recado(dono, agora_ms)));
            }
        }
        let r = Reserva {
            database: database.to_string(),
            tabela: tabela.to_string(),
            usuario: usuario.to_string(),
            ligacao,
            ip: ip.to_string(),
            desde_ms: self
                .dentro
                .get(&k)
                .filter(|d| d.ligacao == ligacao)
                .map(|d| d.desde_ms)
                .unwrap_or(agora_ms),
            expira_ms: agora_ms + prazo_ms,
        };
        self.dentro.insert(k, r.clone());
        Ok(r)
    }

    /// Solta a reserva desta tabela, se for desta ligacao. `forcar` ignora o
    /// dono -- e o que o administrador usa as tres da manha.
    pub fn soltar(
        &mut self,
        database: &str,
        tabela: &str,
        ligacao: u64,
        forcar: bool,
        agora_ms: i64,
    ) -> Result<Reserva> {
        let k = chave(database, tabela);
        match self.dentro.get(&k) {
            None => Err(PhxError::NaoEncontrado(format!(
                "{database}.{tabela} nao esta reservada"
            ))),
            Some(d) if d.ligacao != ligacao && !forcar && d.expira_ms > agora_ms => {
                Err(PhxError::EmCarga(recado(d, agora_ms)))
            }
            Some(_) => Ok(self.dentro.remove(&k).unwrap()),
        }
    }

    /// A reserva que BARRA esta ligacao, se houver.
    ///
    /// Devolve `None` quando nao ha reserva, quando ela e desta mesma ligacao,
    /// ou quando ja venceu -- e a vencida sai da lista aqui, que e o unico
    /// lugar onde alguem repara nela.
    pub fn barra(
        &mut self,
        database: &str,
        tabela: &str,
        ligacao: u64,
        agora_ms: i64,
    ) -> Option<String> {
        let k = chave(database, tabela);
        let vencida = match self.dentro.get(&k) {
            None => return None,
            Some(d) if d.ligacao == ligacao => return None,
            Some(d) if d.expira_ms <= agora_ms => true,
            Some(d) => return Some(recado(d, agora_ms)),
        };
        if vencida {
            self.dentro.remove(&k);
        }
        None
    }

    /// Solta tudo que esta ligacao reservou. Roda na saida da conexao.
    /// Devolve as tabelas soltas, para quem quiser sincronizar.
    pub fn soltar_da_ligacao(&mut self, ligacao: u64) -> Vec<Reserva> {
        let alvos: Vec<String> = self
            .dentro
            .iter()
            .filter(|(_, r)| r.ligacao == ligacao)
            .map(|(k, _)| k.clone())
            .collect();
        alvos
            .into_iter()
            .filter_map(|k| self.dentro.remove(&k))
            .collect()
    }

    pub fn todas(&self) -> Vec<Reserva> {
        let mut v: Vec<Reserva> = self.dentro.values().cloned().collect();
        v.sort_by_key(|r| r.desde_ms);
        v
    }

    pub fn quantas(&self) -> usize {
        self.dentro.len()
    }
}

/// O recado da recusa. Diz QUEM e DESDE QUANDO -- sem isso, «tabela em carga»
/// manda a pessoa procurar sozinha quem esta segurando.
fn recado(r: &Reserva, agora_ms: i64) -> String {
    let ha = ((agora_ms - r.desde_ms).max(0) / 1000) as u64;
    let quem = if r.usuario.is_empty() {
        format!("pela ligacao {}", r.ligacao)
    } else {
        format!("por {} (ligacao {})", r.usuario, r.ligacao)
    };
    format!(
        "{}.{} esta reservada para carga {quem} desde {}, ha {ha}s; \
         tente de novo quando ela terminar",
        r.database,
        r.tabela,
        phxsql_core::datahora::instante_iso(r.desde_ms)
    )
}
