//! O `ExecutorLocal`, a porta do MCP e dos exemplos para o servidor.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// A ponte MCP falando com ESTE servidor, no mesmo processo.
///
/// # Por que ela serializa o pedido para depois reanalisá-lo
///
/// Porque `despachar` recebe uma LINHA, e é ali que moram os quatro portões:
/// política, token, login e permissão por base e tabela. Chamar `executar`
/// direto pularia todos eles -- e seria o segundo caminho até o dado, que é
/// sempre o que esquece uma conferência.
///
/// O custo é um `escrever` mais um `analisar` por chamada. Do outro lado desta
/// ponte há um modelo de linguagem fazendo uma pergunta por vez, e não uma
/// carga de cinco mil linhas: o gasto é irrelevante e a garantia não.
///
/// # O login mora aqui
///
/// A sessão é uma só, e viva entre chamadas: o `login` é uma operação como
/// qualquer outra, e o resultado dela é o que decide o que as próximas podem.
/// Sem isto, cada `tools/call` chegaria anônimo.
pub struct ExecutorLocal {
    servidor: Arc<Servidor>,
    sessao: Mutex<Sessao>,
    /// O que aparece no lugar do IP no log de acessos.
    ///
    /// Não é um endereço porque não há um: a ponte fala pelo cano do processo.
    /// Mas o log tem de dizer que veio dali -- uma leitura pelo MCP que não
    /// deixa rastro seria um buraco na auditoria, e é justamente a origem
    /// sobre a qual mais se vai querer perguntar depois.
    origem: String,
}

impl ExecutorLocal {
    pub fn novo(servidor: Arc<Servidor>, origem: &str) -> ExecutorLocal {
        ExecutorLocal::com_ligacao(servidor, origem, 0)
    }

    /// Um executor com id de LIGACAO -- e ele existe por causa da transacao.
    ///
    /// A transacao pertence a CONEXAO, e o executor local nao tem soquete:
    /// sem um id, dois executores do mesmo processo seriam a mesma conexao, e
    /// a transacao de um apareceria para o outro. Zero continua sendo o
    /// padrao, e zero quer dizer «nao ha conexao a que amarrar» -- que e a
    /// verdade da ponte MCP e do job agendado, e e por isso que os dois nao
    /// abrem transacao.
    pub fn com_ligacao(servidor: Arc<Servidor>, origem: &str, ligacao: u64) -> ExecutorLocal {
        ExecutorLocal {
            servidor,
            sessao: Mutex::new(Sessao {
                ligacao,
                ..Sessao::default()
            }),
            origem: origem.to_string(),
        }
    }

    /// Entra com login e senha, pelo mesmo `login` do protocolo.
    ///
    /// A senha em claro nunca é guardada nem ecoada: ela vira um pedido de
    /// `login`, e o que fica na sessão é a ficha do usuário.
    pub fn entrar(&self, usuario: &str, senha: &str) -> Result<Json> {
        use crate::mcp::Executor;
        self.executar(&Json::objeto(vec![
            ("op", Json::texto_de("login")),
            ("usuario", Json::texto_de(usuario)),
            ("senha", Json::texto_de(senha)),
        ]))
    }

    /// O token de serviço deste servidor, para a ponte carimbar nos pedidos.
    pub fn token(&self) -> String {
        self.servidor.config.token.clone()
    }
}

impl crate::mcp::Executor for ExecutorLocal {
    fn executar(&self, pedido: &Json) -> Result<Json> {
        let mut sessao = self.sessao.tomar("sessao")?;
        let quando_ms = crate::agora_ms();
        let inicio = Instant::now();
        let linha = pedido.escrever();
        let (op, autenticado, resultado) =
            self.servidor.despachar(&linha, &mut sessao, &self.origem);
        let duracao = inicio.elapsed().as_millis() as u64;
        self.servidor.anotar(&Acesso {
            quando_ms,
            ip: self.origem.clone(),
            porta_origem: 0,
            op,
            usuario: sessao.login().to_string(),
            autenticado,
            ok: resultado.is_ok(),
            duracao_ms: duracao,
            erro: resultado.as_ref().err().map(|e| e.to_string()),
            ..objeto_do_pedido(&linha, &resultado)
        });
        resultado
    }
}
