//! Servidor MCP: a tradução de vocabulário entre o Model Context Protocol e o
//! protocolo do PhxSql.
//!
//! # Por que isto é pequeno
//!
//! O MCP fala JSON-RPC 2.0, uma mensagem por linha. O PhxSql fala JSON, uma
//! mensagem por linha. **Não falta transporte nem formato: falta vocabulário.**
//! O que este módulo faz é dizer que `tools/call` com `nome:"phx_ler"` é
//! `{"op":"ler"}` daqui, e devolver o resultado no envelope que o MCP espera.
//!
//! # O portão continua sendo UM
//!
//! Esta ponte **não executa nada**: ela recebe um [`Executor`], que é o
//! `despachar` do servidor. Todo pedido MCP passa pelos mesmos quatro portões
//! -- política, token, login e permissão por base e tabela -- que um cliente
//! pela porta 5000.
//!
//! Isso é deliberado e é a decisão mais importante do módulo. A alternativa --
//! a ponte chamar `Table` direto, "que é mais rápido" -- criaria um segundo
//! caminho até o dado, e o segundo caminho é sempre o que esquece uma
//! conferência. A regra do projeto já está escrita: o portão que alguém
//! esquecer vira a porta dos fundos, e ninguém acha por leitura.
//!
//! # Somente leitura vem LIGADO
//!
//! Pelo mesmo motivo do DbLink, e com mais razão: do outro lado desta ponte há
//! um modelo de linguagem, não uma pessoa. `Ponte::nova` nasce recusando
//! `inserir`, `atualizar` e `excluir`; liberar é uma decisão de quem monta o
//! servidor, e não um padrão herdado.
//!
//! # O que ainda NÃO tem
//!
//! - **Transporte.** Este módulo é a tradução; quem lê de `stdin` ou de um
//!   soquete e chama [`Ponte::atender`] ainda não existe. Foi feito assim
//!   porque a tradução é o que dá para testar sem processo nenhum.
//! - `resources/*` e `prompts/*`. Uma tabela dá um belo *resource*, e isso é
//!   outra rodada.

use phxsql_core::error::Result;
use phxsql_core::json::Json;

/// A revisão do MCP que esta ponte fala.
pub const VERSAO_MCP: &str = "2025-06-18";

/// Revisões que esta ponte aceita se o cliente pedir.
///
/// Quando o cliente pede uma que está aqui, a resposta ECOA a dele -- é assim
/// que o MCP negocia. Quando pede uma que não está, a resposta traz a nossa, e
/// cabe ao cliente decidir se continua.
const VERSOES_ACEITAS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

// Códigos de erro do JSON-RPC 2.0. São os do padrão, e não inventados.
const ERRO_ANALISE: i64 = -32_700;
const ERRO_PEDIDO_INVALIDO: i64 = -32_600;
const ERRO_METODO: i64 = -32_601;
const ERRO_PARAMETROS: i64 = -32_602;

/// Quem sabe executar um pedido do protocolo do PhxSql.
///
/// Existe para a ponte não conhecer o `Servidor`: assim ela se testa com um
/// executor de mentira, sem abrir soquete nem criar tabela.
pub trait Executor {
    /// Recebe `{"op": ...}` e devolve a resposta, ou o erro do PhxSql.
    fn executar(&self, pedido: &Json) -> Result<Json>;
}

/// Uma ferramenta MCP: o nome que o modelo vê e a operação que ela vira.
pub struct Ferramenta {
    /// Nome no MCP. Prefixado com `phx_` para não colidir com as ferramentas
    /// de outros servidores dentro do mesmo cliente.
    pub nome: &'static str,
    /// A `op` do protocolo do PhxSql.
    pub op: &'static str,
    pub descricao: &'static str,
    /// Escreve no banco? Decide se a ponte somente-leitura a oferece.
    pub escreve: bool,
    /// Os parâmetros, como `(nome, tipo JSON, obrigatório, para que serve)`.
    pub parametros: &'static [(&'static str, &'static str, bool, &'static str)],
}

/// O catálogo. Uma linha por ferramenta, e nada de código por ferramenta.
///
/// # Por que uma tabela, e não um `match` por nome
///
/// Porque `tools/list` e `tools/call` têm de concordar **sempre**. Com duas
/// listas, a ferramenta que alguém acrescentar num lugar e esquecer no outro
/// vira uma que o modelo enxerga e não consegue chamar -- ou pior, uma que ele
/// consegue chamar e não está anunciada.
pub const FERRAMENTAS: &[Ferramenta] = &[
    Ferramenta {
        nome: "phx_bancos",
        op: "bancos",
        descricao: "Lista os bancos de dados deste servidor PhxSql.",
        escreve: false,
        parametros: &[],
    },
    Ferramenta {
        nome: "phx_tabelas",
        op: "tabelas",
        descricao: "Lista as tabelas e os schemas de um banco.",
        escreve: false,
        parametros: &[("database", "string", true, "nome do banco")],
    },
    Ferramenta {
        nome: "phx_esquema",
        op: "esquema",
        descricao:
            "Descreve uma tabela: colunas, tipos, índices, chaves e a marca de dado pessoal.",
        escreve: false,
        parametros: &[
            ("database", "string", true, "nome do banco"),
            ("tabela", "string", true, "nome da tabela"),
        ],
    },
    Ferramenta {
        nome: "phx_ler",
        op: "ler",
        descricao: "Lê uma linha pelo rowid.",
        escreve: false,
        parametros: &[
            ("database", "string", true, "nome do banco"),
            ("tabela", "string", true, "nome da tabela"),
            (
                "rowid",
                "integer",
                true,
                "número do registro, a partir de 1",
            ),
        ],
    },
    Ferramenta {
        nome: "phx_varrer",
        op: "varrer",
        descricao: "Percorre a tabela na ordem de digitação, com paginação.",
        escreve: false,
        parametros: &[
            ("database", "string", true, "nome do banco"),
            ("tabela", "string", true, "nome da tabela"),
            ("pular", "integer", false, "quantas linhas saltar"),
            ("limite", "integer", false, "quantas linhas trazer"),
        ],
    },
    Ferramenta {
        nome: "phx_buscar",
        op: "buscar",
        descricao: "Busca por um índice, com a chave exata.",
        escreve: false,
        parametros: &[
            ("database", "string", true, "nome do banco"),
            ("tabela", "string", true, "nome da tabela"),
            ("indice", "string", true, "nome do índice"),
            ("chave", "array", true, "os valores da chave, em ordem"),
        ],
    },
    Ferramenta {
        nome: "phx_dados_pessoais",
        op: "dados_pessoais",
        descricao: "Audita onde estão os dados pessoais (LGPD/GDPR) de um banco.",
        escreve: false,
        parametros: &[
            ("database", "string", true, "nome do banco"),
            ("tabela", "string", false, "limitar a uma tabela"),
        ],
    },
    Ferramenta {
        nome: "phx_inserir",
        op: "inserir",
        descricao: "Inclui uma linha. Só quando a ponte permite escrita.",
        escreve: true,
        parametros: &[
            ("database", "string", true, "nome do banco"),
            ("tabela", "string", true, "nome da tabela"),
            ("valores", "object", true, "coluna: valor"),
        ],
    },
    Ferramenta {
        nome: "phx_atualizar",
        op: "atualizar",
        descricao: "Altera uma linha pelo rowid. Só quando a ponte permite escrita.",
        escreve: true,
        parametros: &[
            ("database", "string", true, "nome do banco"),
            ("tabela", "string", true, "nome da tabela"),
            ("rowid", "integer", true, "número do registro"),
            ("valores", "object", true, "coluna: valor"),
        ],
    },
];

/// A ponte entre o MCP e o protocolo do PhxSql.
pub struct Ponte<E: Executor> {
    executor: E,
    somente_leitura: bool,
    /// Campos que a ponte carimba em todo pedido: `token`, e o que mais o
    /// servidor exigir. Ficam AQUI e não no argumento da ferramenta -- se
    /// fossem argumento, o modelo poderia trocá-los.
    fixos: Vec<(String, Json)>,
}

impl<E: Executor> Ponte<E> {
    /// Uma ponte somente de leitura.
    pub fn nova(executor: E) -> Ponte<E> {
        Ponte {
            executor,
            somente_leitura: true,
            fixos: Vec::new(),
        }
    }

    /// Libera as ferramentas que escrevem. É uma decisão, e por isso é um
    /// método com nome e não um campo com padrão.
    pub fn com_escrita(mut self, permitir: bool) -> Ponte<E> {
        self.somente_leitura = !permitir;
        self
    }

    /// Um campo carimbado em todo pedido -- o `token`, tipicamente.
    pub fn com_campo_fixo(mut self, nome: &str, valor: Json) -> Ponte<E> {
        self.fixos.push((nome.to_string(), valor));
        self
    }

    /// As ferramentas que esta ponte oferece.
    pub fn ferramentas(&self) -> Vec<&'static Ferramenta> {
        FERRAMENTAS
            .iter()
            .filter(|f| !(self.somente_leitura && f.escreve))
            .collect()
    }

    /// Atende uma mensagem JSON-RPC e devolve a resposta.
    ///
    /// `None` quando a mensagem é uma **notificação** (sem `id`). Responder a
    /// uma notificação é erro de protocolo, e é o engano que quebra o cliente
    /// logo no `notifications/initialized` -- a primeira mensagem que ele
    /// manda depois do aperto de mão.
    pub fn atender(&self, linha: &str) -> Option<String> {
        let pedido = match Json::analisar(linha) {
            Ok(p) => p,
            // Sem `id` para devolver: o padrão manda usar nulo.
            Err(e) => {
                return Some(
                    resposta_erro(Json::Nulo, ERRO_ANALISE, &format!("JSON inválido: {e}"))
                        .escrever(),
                )
            }
        };

        let id = pedido.campo("id").cloned();
        let metodo = pedido.texto_ou("method", "").to_string();
        let vazio = Json::Objeto(Vec::new());
        let params = pedido.campo("params").unwrap_or(&vazio);

        // Notificação -- mensagem sem `id`: nada volta, nem erro. O `?` aqui
        // devolve `None`, e silêncio é a resposta certa.
        let id = id?;

        let r = match metodo.as_str() {
            "initialize" => Ok(self.initialize(params)),
            "tools/list" => Ok(self.tools_list()),
            "tools/call" => self.tools_call(params),
            "ping" => Ok(Json::Objeto(Vec::new())),
            outro => Err((
                ERRO_METODO,
                format!("o método {outro:?} não existe nesta ponte"),
            )),
        };

        Some(match r {
            Ok(resultado) => resposta_ok(id, resultado).escrever(),
            Err((codigo, msg)) => resposta_erro(id, codigo, &msg).escrever(),
        })
    }

    fn initialize(&self, params: &Json) -> Json {
        // O MCP negocia a versão: se o cliente pediu uma que sabemos falar, a
        // resposta ECOA a dele. Devolver sempre a nossa faria um cliente mais
        // velho desistir de uma conversa que funcionaria.
        let pedida = params.texto_ou("protocolVersion", "");
        let versao = if VERSOES_ACEITAS.contains(&pedida) {
            pedida
        } else {
            VERSAO_MCP
        };

        Json::objeto(vec![
            ("protocolVersion", Json::texto_de(versao)),
            (
                "capabilities",
                Json::objeto(vec![(
                    "tools",
                    // `listChanged: false` porque o catálogo é constante em
                    // tempo de compilação. Dizer `true` prometeria uma
                    // notificação que este servidor nunca manda.
                    Json::objeto(vec![("listChanged", Json::Bool(false))]),
                )]),
            ),
            (
                "serverInfo",
                Json::objeto(vec![
                    ("name", Json::texto_de("phxsql")),
                    ("version", Json::texto_de(env!("CARGO_PKG_VERSION"))),
                ]),
            ),
            (
                "instructions",
                Json::texto_de(
                    "Motor de dados PhxSql. Comece por phx_bancos, depois phx_tabelas e \
                     phx_esquema: o esquema traz os tipos das colunas e a marca de dado \
                     pessoal, e é o que evita montar um filtro com o tipo errado. \
                     A ordem das linhas é a de digitação (rowid), e não a de nenhuma chave.",
                ),
            ),
        ])
    }

    fn tools_list(&self) -> Json {
        Json::objeto(vec![(
            "tools",
            Json::Lista(self.ferramentas().iter().map(|f| esquema_de(f)).collect()),
        )])
    }

    fn tools_call(&self, params: &Json) -> std::result::Result<Json, (i64, String)> {
        let nome = params.texto_ou("name", "");
        let Some(f) = self.ferramentas().into_iter().find(|f| f.nome == nome) else {
            // A ferramenta não existir é erro de PROTOCOLO -- o modelo chamou
            // algo que não foi anunciado --, e por isso vai no erro do
            // JSON-RPC e não no `isError` do resultado.
            let motivo = if FERRAMENTAS.iter().any(|f| f.nome == nome) {
                format!("a ferramenta {nome:?} escreve, e esta ponte é somente de leitura")
            } else {
                format!("a ferramenta {nome:?} não existe")
            };
            return Err((ERRO_PARAMETROS, motivo));
        };

        let vazio = Json::Objeto(Vec::new());
        let argumentos = params.campo("arguments").unwrap_or(&vazio);
        if !matches!(argumentos, Json::Objeto(_)) {
            return Err((
                ERRO_PEDIDO_INVALIDO,
                "\"arguments\" precisa ser um objeto".into(),
            ));
        }

        // Falta de argumento obrigatório é recusada AQUI, e não lá dentro: a
        // mensagem daqui diz o nome do parâmetro e para que ele serve, que é
        // o que o modelo precisa para corrigir sozinho.
        for (nome_p, _, obrigatorio, para_que) in f.parametros {
            if *obrigatorio && argumentos.campo(nome_p).is_none() {
                return Err((
                    ERRO_PARAMETROS,
                    format!("falta o argumento {nome_p:?} ({para_que})"),
                ));
            }
        }

        let mut pedido: Vec<(String, Json)> = vec![("op".into(), Json::texto_de(f.op))];
        if let Json::Objeto(pares) = argumentos {
            for (k, v) in pares {
                // Um argumento chamado "op" ou "token" sobrescreveria o que a
                // ponte carimba, e aí o modelo escolheria a operação e a
                // credencial. Os fixos entram DEPOIS, mas filtrar aqui deixa
                // a intenção escrita em vez de dependente da ordem.
                if k == "op" || self.fixos.iter().any(|(f, _)| f == k) {
                    continue;
                }
                pedido.push((k.clone(), v.clone()));
            }
        }
        for (k, v) in &self.fixos {
            pedido.push((k.clone(), v.clone()));
        }

        // Daqui para baixo, quem manda é o servidor: mesmos portões, mesmos
        // erros.
        Ok(match self.executor.executar(&Json::Objeto(pedido)) {
            Ok(saida) => conteudo(&saida.escrever_identado(), false),
            // Falha de EXECUÇÃO -- tabela que não existe, permissão negada --
            // volta como resultado com `isError`, e não como erro do JSON-RPC.
            // É a diferença que o MCP faz de propósito: assim o modelo LÊ o
            // erro e corrige, em vez de a conversa inteira ser abortada pelo
            // cliente.
            Err(e) => conteudo(&format!("erro {} do PhxSql: {e}", e.codigo()), true),
        })
    }
}

/// O JSON Schema que o MCP espera em cada ferramenta.
fn esquema_de(f: &Ferramenta) -> Json {
    let propriedades: Vec<(String, Json)> = f
        .parametros
        .iter()
        .map(|(nome, tipo, _, para_que)| {
            (
                nome.to_string(),
                Json::objeto(vec![
                    ("type", Json::texto_de(*tipo)),
                    ("description", Json::texto_de(*para_que)),
                ]),
            )
        })
        .collect();
    let obrigatorios: Vec<Json> = f
        .parametros
        .iter()
        .filter(|(_, _, obrigatorio, _)| *obrigatorio)
        .map(|(nome, _, _, _)| Json::texto_de(*nome))
        .collect();

    Json::objeto(vec![
        ("name", Json::texto_de(f.nome)),
        ("description", Json::texto_de(f.descricao)),
        (
            "inputSchema",
            Json::objeto(vec![
                ("type", Json::texto_de("object")),
                ("properties", Json::Objeto(propriedades)),
                ("required", Json::Lista(obrigatorios)),
            ]),
        ),
    ])
}

fn conteudo(texto: &str, e_erro: bool) -> Json {
    Json::objeto(vec![
        (
            "content",
            Json::Lista(vec![Json::objeto(vec![
                ("type", Json::texto_de("text")),
                ("text", Json::texto_de(texto)),
            ])]),
        ),
        ("isError", Json::Bool(e_erro)),
    ])
}

fn resposta_ok(id: Json, resultado: Json) -> Json {
    Json::Objeto(vec![
        ("jsonrpc".into(), Json::texto_de("2.0")),
        ("id".into(), id),
        ("result".into(), resultado),
    ])
}

fn resposta_erro(id: Json, codigo: i64, mensagem: &str) -> Json {
    Json::Objeto(vec![
        ("jsonrpc".into(), Json::texto_de("2.0")),
        ("id".into(), id),
        (
            "error".into(),
            Json::objeto(vec![
                ("code", Json::de_i64(codigo)),
                ("message", Json::texto_de(mensagem)),
            ]),
        ),
    ])
}

#[cfg(test)]
mod testes {
    use super::*;
    use phxsql_core::error::PhxError;
    use std::cell::RefCell;

    /// Executor de mentira: guarda o que recebeu e devolve o que mandarem.
    struct Espiao {
        recebidos: RefCell<Vec<Json>>,
        falhar: bool,
    }

    impl Espiao {
        fn novo() -> Espiao {
            Espiao {
                recebidos: RefCell::new(Vec::new()),
                falhar: false,
            }
        }
        fn que_falha() -> Espiao {
            Espiao {
                recebidos: RefCell::new(Vec::new()),
                falhar: true,
            }
        }
        fn ultimo(&self) -> Json {
            self.recebidos.borrow().last().cloned().unwrap()
        }
    }

    impl Executor for Espiao {
        fn executar(&self, pedido: &Json) -> Result<Json> {
            self.recebidos.borrow_mut().push(pedido.clone());
            if self.falhar {
                return Err(PhxError::NaoEncontrado("tabela xyz nao existe".into()));
            }
            Ok(Json::objeto(vec![("ok", Json::Bool(true))]))
        }
    }

    fn atender(p: &Ponte<Espiao>, linha: &str) -> Json {
        Json::analisar(&p.atender(linha).expect("devia responder")).unwrap()
    }

    #[test]
    fn o_initialize_devolve_versao_capacidades_e_identidade() {
        let p = Ponte::nova(Espiao::novo());
        let r = atender(
            &p,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize",
                "params":{"protocolVersion":"2025-06-18"}}"#,
        );
        let res = r.campo("result").unwrap();
        assert_eq!(res.texto_ou("protocolVersion", ""), "2025-06-18");
        assert!(res.campo("capabilities").unwrap().campo("tools").is_some());
        assert_eq!(
            res.campo("serverInfo").unwrap().texto_ou("name", ""),
            "phxsql"
        );
        assert_eq!(r.texto_ou("jsonrpc", ""), "2.0");
    }

    /// O MCP negocia: cliente que pede uma revisao que sabemos falar recebe a
    /// DELE de volta. Devolver sempre a nossa faria um cliente mais velho
    /// desistir de uma conversa que funcionaria.
    #[test]
    fn a_versao_do_cliente_e_ecoada_quando_da_para_falar() {
        let p = Ponte::nova(Espiao::novo());
        let r = atender(
            &p,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize",
                "params":{"protocolVersion":"2024-11-05"}}"#,
        );
        assert_eq!(
            r.campo("result").unwrap().texto_ou("protocolVersion", ""),
            "2024-11-05"
        );

        let r = atender(
            &p,
            r#"{"jsonrpc":"2.0","id":2,"method":"initialize",
                "params":{"protocolVersion":"1999-01-01"}}"#,
        );
        assert_eq!(
            r.campo("result").unwrap().texto_ou("protocolVersion", ""),
            VERSAO_MCP,
            "revisao que nao sabemos falar tem de receber a nossa"
        );
    }

    /// **Responder a uma notificação quebra o cliente.** O
    /// `notifications/initialized` é a primeira coisa que ele manda depois do
    /// aperto de mão, e ele não espera resposta nenhuma.
    #[test]
    fn notificacao_nao_recebe_resposta() {
        let p = Ponte::nova(Espiao::novo());
        assert!(p
            .atender(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none());
        // E nem uma notificação de método desconhecido responde.
        assert!(p
            .atender(r#"{"jsonrpc":"2.0","method":"nao/existe"}"#)
            .is_none());
    }

    #[test]
    fn json_quebrado_vira_erro_de_analise_com_id_nulo() {
        let p = Ponte::nova(Espiao::novo());
        let r = atender(&p, "{isto nao e json");
        assert_eq!(
            r.campo("error").unwrap().campo("code").unwrap().inteiro(),
            Some(ERRO_ANALISE)
        );
        assert!(r.campo("id").unwrap().e_nulo());
    }

    #[test]
    fn metodo_desconhecido_vira_menos_32601() {
        let p = Ponte::nova(Espiao::novo());
        let r = atender(&p, r#"{"jsonrpc":"2.0","id":7,"method":"nao/existe"}"#);
        assert_eq!(
            r.campo("error").unwrap().campo("code").unwrap().inteiro(),
            Some(ERRO_METODO)
        );
        assert_eq!(r.campo("id").unwrap().inteiro(), Some(7));
    }

    /// `tools/list` e `tools/call` têm de concordar: toda ferramenta anunciada
    /// é chamável, e nenhuma chamável fica de fora do anúncio.
    #[test]
    fn tudo_que_a_lista_anuncia_da_para_chamar() {
        let p = Ponte::nova(Espiao::novo()).com_escrita(true);
        let r = atender(&p, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#);
        let lista = r.campo("result").unwrap().campo("tools").unwrap();
        let anunciadas: Vec<String> = lista
            .lista()
            .unwrap()
            .iter()
            .map(|t| t.texto_ou("name", "").to_string())
            .collect();

        assert_eq!(anunciadas.len(), FERRAMENTAS.len());
        for f in FERRAMENTAS {
            assert!(
                anunciadas.contains(&f.nome.to_string()),
                "{} nao foi anunciada",
                f.nome
            );
        }
        // E todo esquema anunciado tem a forma que o MCP exige.
        for t in lista.lista().unwrap() {
            let e = t.campo("inputSchema").unwrap();
            assert_eq!(e.texto_ou("type", ""), "object");
            assert!(e.campo("properties").is_some());
            assert!(e.campo("required").is_some());
        }
    }

    #[test]
    fn a_chamada_vira_a_op_do_phxsql() {
        let p = Ponte::nova(Espiao::novo());
        let r = atender(
            &p,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call",
                "params":{"name":"phx_esquema",
                          "arguments":{"database":"loja","tabela":"clientes"}}}"#,
        );
        let pedido = p.executor.ultimo();
        assert_eq!(pedido.texto_ou("op", ""), "esquema");
        assert_eq!(pedido.texto_ou("database", ""), "loja");
        assert_eq!(pedido.texto_ou("tabela", ""), "clientes");

        let res = r.campo("result").unwrap();
        assert_eq!(res.campo("isError").unwrap().booleano(), Some(false));
        assert_eq!(
            res.campo("content").unwrap().lista().unwrap()[0].texto_ou("type", ""),
            "text"
        );
    }

    /// **Somente leitura vem ligado.** E a mensagem diz por que recusou, em
    /// vez de fingir que a ferramenta não existe.
    #[test]
    fn a_ponte_padrao_recusa_escrita() {
        let p = Ponte::nova(Espiao::novo());
        assert!(p.ferramentas().iter().all(|f| !f.escreve));

        let r = atender(
            &p,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call",
                "params":{"name":"phx_inserir",
                          "arguments":{"database":"d","tabela":"t","valores":{}}}}"#,
        );
        let msg = r
            .campo("error")
            .unwrap()
            .texto_ou("message", "")
            .to_string();
        assert!(msg.contains("somente de leitura"), "{msg}");
        assert!(
            p.executor.recebidos.borrow().is_empty(),
            "a ponte executou uma escrita que devia ter recusado"
        );
    }

    #[test]
    fn com_escrita_ligada_a_ferramenta_aparece_e_funciona() {
        let p = Ponte::nova(Espiao::novo()).com_escrita(true);
        assert!(p.ferramentas().iter().any(|f| f.escreve));
        atender(
            &p,
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call",
                "params":{"name":"phx_inserir",
                          "arguments":{"database":"d","tabela":"t","valores":{"a":1}}}}"#,
        );
        assert_eq!(p.executor.ultimo().texto_ou("op", ""), "inserir");
    }

    /// O modelo não escolhe a credencial nem a operação: o que a ponte carimba
    /// vence o que veio no argumento.
    #[test]
    fn argumento_nao_sobrescreve_o_token_nem_a_op() {
        let p = Ponte::nova(Espiao::novo())
            .com_campo_fixo("token", Json::texto_de("o-token-de-verdade"));
        atender(
            &p,
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call",
                "params":{"name":"phx_bancos",
                          "arguments":{"token":"roubado","op":"excluir_tabela"}}}"#,
        );
        let pedido = p.executor.ultimo();
        assert_eq!(pedido.texto_ou("op", ""), "bancos");
        assert_eq!(pedido.texto_ou("token", ""), "o-token-de-verdade");
        // E o campo não pode aparecer duas vezes, com o errado antes.
        assert_eq!(pedido.chaves().iter().filter(|k| **k == "token").count(), 1);
    }

    #[test]
    fn argumento_obrigatorio_que_falta_e_recusado_com_o_nome_dele() {
        let p = Ponte::nova(Espiao::novo());
        let r = atender(
            &p,
            r#"{"jsonrpc":"2.0","id":8,"method":"tools/call",
                "params":{"name":"phx_esquema","arguments":{"database":"loja"}}}"#,
        );
        let msg = r
            .campo("error")
            .unwrap()
            .texto_ou("message", "")
            .to_string();
        assert!(msg.contains("tabela"), "{msg}");
        assert!(p.executor.recebidos.borrow().is_empty());
    }

    /// Falha de EXECUÇÃO volta como resultado com `isError`, e não como erro
    /// do JSON-RPC: assim o modelo LÊ o erro e corrige, em vez de o cliente
    /// abortar a conversa.
    #[test]
    fn erro_do_phxsql_vira_resultado_com_iserror() {
        let p = Ponte::nova(Espiao::que_falha());
        let r = atender(
            &p,
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/call",
                "params":{"name":"phx_tabelas","arguments":{"database":"loja"}}}"#,
        );
        assert!(
            r.campo("error").is_none(),
            "falha de execucao nao pode virar erro do JSON-RPC"
        );
        let res = r.campo("result").unwrap();
        assert_eq!(res.campo("isError").unwrap().booleano(), Some(true));
        let texto = res.campo("content").unwrap().lista().unwrap()[0]
            .texto_ou("text", "")
            .to_string();
        assert!(texto.contains("3001"), "faltou o codigo do erro: {texto}");
        assert!(texto.contains("xyz"), "{texto}");
    }

    #[test]
    fn o_ping_do_mcp_responde_vazio() {
        let p = Ponte::nova(Espiao::novo());
        let r = atender(&p, r#"{"jsonrpc":"2.0","id":10,"method":"ping"}"#);
        assert!(r.campo("result").is_some());
        assert!(r.campo("error").is_none());
    }

    /// Toda `op` do catálogo tem de existir no servidor. Este teste não
    /// alcança o `executar` do servidor, mas alcança a tabela de permissões --
    /// e uma `op` que ela não conhece seria uma ferramenta sem portão.
    #[test]
    fn toda_op_anunciada_tem_atividade_de_permissao() {
        use crate::usuarios::Atividade;
        for f in FERRAMENTAS {
            assert!(
                Atividade::da_operacao(f.op).is_some(),
                "a op {:?} da ferramenta {} nao tem atividade: ela passaria \
                 pelo portao de permissao sem ser conferida",
                f.op,
                f.nome
            );
        }
    }
}
