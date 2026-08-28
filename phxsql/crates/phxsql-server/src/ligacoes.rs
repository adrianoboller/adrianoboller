//! Quem esta falando com o servidor agora -- e como derrubar quem travou.
//!
//! # A lacuna que isto fecha
//!
//! O servidor sabia CONTAR conexoes e nao sabia dizer quem eram. Quando uma
//! consulta prende a trava de dados, quem opera precisa de duas respostas que
//! nao existiam: *quem esta segurando* e *como solto*. E o `SHOW PROCESSLIST`
//! mais o `KILL` do MySQL(R), e a falta dos dois e a diferenca entre
//! diagnosticar em um minuto e reiniciar o servidor.
//!
//! # O que o encerramento alcanca, e o que nao alcanca
//!
//! Encerrar fecha o SOQUETE. Isso e imediato para a conexao que esta esperando
//! o proximo pedido -- que e o caso comum de conexao esquecida aberta.
//!
//! Uma operacao que **ja entrou** na trava de dados termina assim mesmo: nao
//! ha como abandonar uma varredura no meio sem arriscar deixar a tabela aberta
//! pela metade. O que muda e que o resultado nao vai para lugar nenhum e a
//! conexao nao volta. Dizer isso e melhor do que prometer um `KILL` que parece
//! instantaneo e nao e.

use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use phxsql_core::json::Json;

/// Uma conexao viva.
#[derive(Clone)]
pub struct Ligacao {
    pub id: u64,
    pub ip: String,
    pub porta: u16,
    /// Login de quem entrou. Vazio enquanto so o token foi apresentado.
    pub usuario: String,
    pub desde_ms: i64,
    /// A operacao em curso. Vazia quando a conexao esta esperando pedido.
    pub op: String,
    /// Quando a operacao corrente comecou. Zero quando nao ha nenhuma.
    pub op_desde_ms: i64,
    pub database: String,
    pub tabela: String,
    /// Quantos pedidos ja passaram por esta conexao.
    pub pedidos: u64,
    /// Ligado quando alguem pediu para encerrar.
    pub morrer: Arc<AtomicBool>,
    /// O soquete, para conseguir fechar de fora.
    ///
    /// Guardado como `Arc` porque quem encerra nao e quem atende: a thread da
    /// conexao esta parada dentro de um `read_line`, e so um `shutdown` vindo
    /// de fora a acorda.
    soquete: Option<Arc<TcpStream>>,
}

impl Ligacao {
    pub fn para_json(&self, agora_ms: i64) -> Json {
        Json::objeto(vec![
            ("id", Json::de_u64(self.id)),
            ("ip", Json::texto_de(&self.ip)),
            ("porta", Json::de_u64(self.porta as u64)),
            (
                "usuario",
                match self.usuario.is_empty() {
                    true => Json::Nulo,
                    false => Json::texto_de(&self.usuario),
                },
            ),
            (
                "desde",
                Json::texto_de(phxsql_core::datahora::instante_iso(self.desde_ms)),
            ),
            (
                "aberta_s",
                Json::de_u64(((agora_ms - self.desde_ms) / 1_000).max(0) as u64),
            ),
            (
                "estado",
                Json::texto_de(if self.op.is_empty() {
                    "esperando"
                } else {
                    "executando"
                }),
            ),
            (
                "op",
                match self.op.is_empty() {
                    true => Json::Nulo,
                    false => Json::texto_de(&self.op),
                },
            ),
            // Quanto tempo a operacao corrente ja leva. E o numero que aponta
            // o culpado quando tudo fica parado.
            (
                "ha_ms",
                match self.op_desde_ms {
                    0 => Json::Nulo,
                    q => Json::de_u64((agora_ms - q).max(0) as u64),
                },
            ),
            (
                "objeto",
                match (self.database.is_empty(), self.tabela.is_empty()) {
                    (true, true) => Json::Nulo,
                    (false, true) => Json::texto_de(&self.database),
                    (true, false) => Json::texto_de(&self.tabela),
                    _ => Json::texto_de(format!("{}.{}", self.database, self.tabela)),
                },
            ),
            ("pedidos", Json::de_u64(self.pedidos)),
            ("encerrando", Json::Bool(self.morrer.load(Ordering::SeqCst))),
        ])
    }
}

/// O registro das conexoes vivas.
#[derive(Default)]
pub struct Ligacoes {
    dentro: HashMap<u64, Ligacao>,
    proximo: AtomicU64,
}

impl Ligacoes {
    /// Registra uma conexao nova e devolve (id, sinalizador de encerramento).
    pub fn entrar(
        &mut self,
        ip: &str,
        porta: u16,
        agora_ms: i64,
        soquete: Option<Arc<TcpStream>>,
    ) -> (u64, Arc<AtomicBool>) {
        // Comeca em 1: id zero costuma ser confundido com "nenhum".
        let id = self.proximo.fetch_add(1, Ordering::SeqCst) + 1;
        let morrer = Arc::new(AtomicBool::new(false));
        self.dentro.insert(
            id,
            Ligacao {
                id,
                ip: ip.to_string(),
                porta,
                usuario: String::new(),
                desde_ms: agora_ms,
                op: String::new(),
                op_desde_ms: 0,
                database: String::new(),
                tabela: String::new(),
                pedidos: 0,
                morrer: Arc::clone(&morrer),
                soquete,
            },
        );
        (id, morrer)
    }

    pub fn sair(&mut self, id: u64) {
        self.dentro.remove(&id);
    }

    /// Marca o inicio de uma operacao.
    pub fn comecou(
        &mut self,
        id: u64,
        op: &str,
        usuario: &str,
        database: &str,
        tabela: &str,
        agora_ms: i64,
    ) {
        if let Some(l) = self.dentro.get_mut(&id) {
            l.op = op.to_string();
            l.op_desde_ms = agora_ms;
            l.database = database.to_string();
            l.tabela = tabela.to_string();
            l.pedidos += 1;
            if !usuario.is_empty() {
                l.usuario = usuario.to_string();
            }
        }
    }

    /// Marca o fim da operacao: a conexao volta a esperar.
    ///
    /// Leva o login junto porque o pedido que autentica e o proprio `login`:
    /// quando ele COMECOU a sessao ainda era anonima, e so aqui se sabe quem
    /// entrou.
    pub fn terminou(&mut self, id: u64, usuario: &str) {
        if let Some(l) = self.dentro.get_mut(&id) {
            l.op.clear();
            l.op_desde_ms = 0;
            l.database.clear();
            l.tabela.clear();
            if !usuario.is_empty() {
                l.usuario = usuario.to_string();
            }
        }
    }

    pub fn quantas(&self) -> usize {
        self.dentro.len()
    }

    /// Todas, da mais antiga para a mais nova.
    pub fn todas(&self) -> Vec<Ligacao> {
        let mut v: Vec<Ligacao> = self.dentro.values().cloned().collect();
        v.sort_by_key(|l| l.id);
        v
    }

    /// Manda encerrar. Devolve falso quando a conexao ja saiu sozinha.
    ///
    /// Faz as duas coisas: liga o sinalizador, que a thread confere antes do
    /// proximo pedido, e fecha o soquete, que acorda a thread parada esperando
    /// um pedido que nunca vem. Só o sinalizador nao bastaria -- a conexao
    /// ociosa ficaria pendurada ate o cliente falar.
    pub fn encerrar(&mut self, id: u64) -> bool {
        let Some(l) = self.dentro.get(&id) else {
            return false;
        };
        l.morrer.store(true, Ordering::SeqCst);
        if let Some(s) = &l.soquete {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
        true
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn entra_conta_e_sai() {
        let mut l = Ligacoes::default();
        assert_eq!(l.quantas(), 0);
        let (a, _) = l.entrar("10.0.0.1", 4000, 1_000, None);
        let (b, _) = l.entrar("10.0.0.2", 4001, 1_100, None);
        assert_eq!(l.quantas(), 2);
        assert_ne!(a, b, "duas conexoes com o mesmo id");
        l.sair(a);
        assert_eq!(l.quantas(), 1);
        assert_eq!(l.todas()[0].id, b);
    }

    /// O id nunca e zero: zero se confunde com "nenhum" em quem le.
    #[test]
    fn o_id_comeca_em_um() {
        let mut l = Ligacoes::default();
        let (a, _) = l.entrar("x", 1, 0, None);
        assert_eq!(a, 1);
    }

    #[test]
    fn a_operacao_corrente_aparece_e_some() {
        let mut l = Ligacoes::default();
        let (id, _) = l.entrar("10.0.0.1", 4000, 1_000, None);
        l.comecou(id, "varrer", "adriano", "loja", "clientes", 2_000);
        let v = l.todas();
        assert_eq!(v[0].op, "varrer");
        assert_eq!(v[0].usuario, "adriano");
        assert_eq!(v[0].pedidos, 1);
        let j = v[0].para_json(5_000).escrever();
        assert!(j.contains("\"executando\""), "{j}");
        assert!(j.contains("\"ha_ms\":3000"), "{j}");
        assert!(j.contains("loja.clientes"), "{j}");

        l.terminou(id, "adriano");
        let v = l.todas();
        assert!(v[0].op.is_empty());
        // O usuario FICA depois da operacao: ele e da conexao, nao do pedido.
        assert_eq!(v[0].usuario, "adriano");
        assert!(v[0].para_json(5_000).escrever().contains("\"esperando\""));
    }

    #[test]
    fn encerrar_liga_o_sinalizador_e_diz_se_achou() {
        let mut l = Ligacoes::default();
        let (id, morrer) = l.entrar("10.0.0.1", 4000, 1_000, None);
        assert!(!morrer.load(Ordering::SeqCst));
        assert!(l.encerrar(id));
        assert!(morrer.load(Ordering::SeqCst), "o sinalizador nao subiu");
        assert!(!l.encerrar(999), "encerrou uma conexao que nao existe");
    }
}
