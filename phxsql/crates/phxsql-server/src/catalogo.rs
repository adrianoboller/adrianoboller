//! O catálogo das operações do protocolo: cada `op` descrita por DADOS.
//!
//! # Por que isto existe
//!
//! O `despachar` conhecia ~80 operações e nada as descrevia. Quem quisesse
//! saber o que existe lia o `MANUAL.txt`, e quem quisesse saber os parâmetros
//! lia o código. As duas coisas **envelhecem caladas**: operação nova nasce
//! sem descrição e ninguém percebe, porque nada quebra.
//!
//! É a mesma regra que o projeto já escreveu para número: *ou sai de um
//! gerador, ou está errado e ninguém percebeu ainda*. Ajuda escrita à mão
//! envelhece igual — e por isso há um teste que deriva a lista de operações do
//! **texto do `match`** do `despachar` e exige que ela e o catálogo sejam a
//! mesma lista, nos dois sentidos.
//!
//! # O que NÃO se declara aqui
//!
//! A **permissão**. Ela sai de [`Atividade::da_operacao`], e a escrita sai de
//! `OPS_ESCRITA`. Declarar de novo aqui criaria uma segunda verdade ao lado do
//! portão -- e a segunda verdade é sempre a que fica desatualizada, dizendo ao
//! cliente que ele pode o que o servidor nega (ou o contrário).
//!
//! # Quem consome
//!
//! - a op `catalogo` do protocolo, filtrada pelo poder de quem perguntou;
//! - o `tools/list` do MCP, que antes era uma segunda lista escrita à mão;
//! - o `/help` do `phxsqlcmd`, que pede o catálogo pela rede.

use phxsql_core::json::Json;

use crate::servidor::OPS_ESCRITA;
use crate::usuarios::{Atividade, Usuario};

/// Um parâmetro do pedido JSON.
pub struct Parametro {
    pub nome: &'static str,
    /// Tipo em JSON: `string`, `integer`, `boolean`, `array`, `object`.
    pub tipo: &'static str,
    pub obrigatorio: bool,
    /// Para que serve, numa frase. É o que um cliente -- gente ou modelo --
    /// precisa para preencher o campo certo sem abrir o código.
    pub para_que: &'static str,
}

const fn obr(nome: &'static str, tipo: &'static str, para_que: &'static str) -> Parametro {
    Parametro {
        nome,
        tipo,
        obrigatorio: true,
        para_que,
    }
}

const fn opc(nome: &'static str, tipo: &'static str, para_que: &'static str) -> Parametro {
    Parametro {
        nome,
        tipo,
        obrigatorio: false,
        para_que,
    }
}

// Os dois campos que quase toda operação de dados repete.
const DB: Parametro = obr("database", "string", "o banco de dados");
const TAB: Parametro = obr(
    "tabela",
    "string",
    "a tabela; aceita `schema.tabela` para tabela dentro de schema",
);
const ROWID: Parametro = obr("rowid", "integer", "o número do registro, a partir de 1");
const MAX: Parametro = opc(
    "max",
    "integer",
    "teto de linhas na resposta; o servidor aplica o dele por cima",
);

/// Uma operação do protocolo.
pub struct Operacao {
    /// O nome canônico -- o que a documentação usa e o que o catálogo devolve.
    pub nome: &'static str,
    /// Os outros nomes que o `despachar` aceita para a MESMA operação.
    pub apelidos: &'static [&'static str],
    /// O que ela faz, numa frase.
    pub resumo: &'static str,
    pub parametros: &'static [Parametro],
    /// Um pedido inteiro que funciona, sem o `token`.
    pub exemplo: &'static str,
    /// Vale a pena como ferramenta MCP? O nome sai de `phx_` + [`Self::nome`],
    /// e não de um segundo campo que pudesse discordar dele.
    pub ferramenta_mcp: bool,
}

impl Operacao {
    /// A atividade que o portão de permissão exige. `None` = basta estar
    /// autenticado, e é o caso do `ping` e do `login`.
    pub fn atividade(&self) -> Option<Atividade> {
        Atividade::da_operacao(self.nome)
    }

    /// Grava alguma coisa? Sai da lista que o modo somente-leitura usa, e não
    /// de um campo próprio: com dois lugares, um deles ficaria para trás.
    pub fn escreve(&self) -> bool {
        OPS_ESCRITA.contains(&self.nome)
    }

    /// O nome desta operação como ferramenta MCP.
    ///
    /// O prefixo existe porque um cliente MCP junta as ferramentas de vários
    /// servidores no mesmo espaço de nomes.
    pub fn nome_mcp(&self) -> String {
        format!("phx_{}", self.nome)
    }

    /// Todos os nomes pelos quais o `despachar` a atende.
    pub fn nomes(&self) -> impl Iterator<Item = &'static str> + '_ {
        std::iter::once(self.nome).chain(self.apelidos.iter().copied())
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("nome", Json::texto_de(self.nome)),
            (
                "apelidos",
                Json::Lista(self.apelidos.iter().map(|a| Json::texto_de(*a)).collect()),
            ),
            ("resumo", Json::texto_de(self.resumo)),
            (
                "permissao",
                match self.atividade() {
                    Some(a) => Json::texto_de(a.nome()),
                    None => Json::Nulo,
                },
            ),
            ("escreve", Json::Bool(self.escreve())),
            (
                "parametros",
                Json::Lista(
                    self.parametros
                        .iter()
                        .map(|p| {
                            Json::objeto(vec![
                                ("nome", Json::texto_de(p.nome)),
                                ("tipo", Json::texto_de(p.tipo)),
                                ("obrigatorio", Json::Bool(p.obrigatorio)),
                                ("para_que", Json::texto_de(p.para_que)),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("exemplo", Json::texto_de(self.exemplo)),
        ])
    }
}

/// A operação com este nome ou apelido.
pub fn por_nome(nome: &str) -> Option<&'static Operacao> {
    OPERACOES.iter().find(|o| o.nomes().any(|n| n == nome))
}

/// As operações que esta sessão pode chamar.
///
/// Sem usuário -- servidor sem cadastro, ou entrada pelo token de serviço --
/// é tudo, que é exatamente o poder que aquela sessão tem. A filtragem aqui
/// não é o portão: o portão é o do `despachar`, e ele confere de novo quando a
/// operação for de fato chamada. Isto é cortesia, para não oferecer a alguém
/// oitenta operações das quais ele só pode chamar três.
pub fn visiveis(usuario: Option<&Usuario>, database: &str) -> Vec<&'static Operacao> {
    OPERACOES
        .iter()
        .filter(|o| match (usuario, o.atividade()) {
            (None, _) | (_, None) => true,
            (Some(u), Some(a)) => u.pode_em(database, "", a),
        })
        .collect()
}

/// As operações oferecidas como ferramenta MCP.
pub fn ferramentas_mcp() -> impl Iterator<Item = &'static Operacao> {
    OPERACOES.iter().filter(|o| o.ferramenta_mcp)
}

/// O catálogo. Uma entrada por operação do `despachar`, e nada de código por
/// operação.
pub const OPERACOES: &[Operacao] = &[
    // ------------------------------------------------------------ a sessão
    Operacao {
        nome: "ping",
        apelidos: &[],
        resumo: "Diz a versão, o papel na replicação, quantas conexões estão \
                 abertas e há quanto tempo o servidor está no ar.",
        parametros: &[],
        exemplo: r#"{"op":"ping"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "desafio",
        apelidos: &[],
        resumo: "Abre um desafio de login: devolve o sal, as iterações e um \
                 nonce de uso único, para a senha não viajar.",
        parametros: &[obr("usuario", "string", "o login de quem vai entrar")],
        exemplo: r#"{"op":"desafio","usuario":"adriano"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "login",
        apelidos: &[],
        resumo: "Entra na sessão, pela prova do desafio ou pela senha em claro.",
        parametros: &[
            obr("usuario", "string", "o login"),
            opc(
                "prova",
                "string",
                "o HMAC do desafio; é o caminho em que a senha não viaja",
            ),
            opc(
                "nonce_cliente",
                "string",
                "o nonce sorteado pelo cliente, que acompanha a prova",
            ),
            opc(
                "senha",
                "string",
                "a senha em claro, para quem não usa o desafio",
            ),
            opc(
                "assinatura",
                "string",
                "assinatura Ed25519 do nonce, quando o usuário exige segundo fator",
            ),
        ],
        exemplo: r#"{"op":"login","usuario":"adriano","prova":"a1b2...","nonce_cliente":"c3d4..."}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "sair",
        apelidos: &[],
        resumo: "Larga a identidade da sessão sem fechar a conexão.",
        parametros: &[],
        exemplo: r#"{"op":"sair"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "quem_sou",
        apelidos: &[],
        resumo: "Diz quem é o usuário desta sessão, ou que ela entrou pelo \
                 token de serviço.",
        parametros: &[],
        exemplo: r#"{"op":"quem_sou"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "catalogo",
        apelidos: &[],
        resumo: "Descreve as operações do protocolo que esta sessão pode chamar.",
        parametros: &[
            // O campo NAO se chama "op": esse ja e o nome da operacao que se
            // esta chamando, e um pedido nao pode ter a mesma chave duas vezes.
            opc(
                "operacao",
                "string",
                "detalha uma operação só, em vez de listar todas",
            ),
            opc(
                "database",
                "string",
                "contra qual banco medir a permissão; sem ele vale a regra geral",
            ),
        ],
        exemplo: r#"{"op":"catalogo","operacao":"buscar"}"#,
        ferramenta_mcp: false,
    },
    // ------------------------------------------------------ o que existe
    Operacao {
        nome: "bancos",
        apelidos: &[],
        resumo: "Lista os bancos de dados deste servidor.",
        parametros: &[],
        exemplo: r#"{"op":"bancos"}"#,
        ferramenta_mcp: true,
    },
    Operacao {
        nome: "tabelas",
        apelidos: &[],
        resumo: "Lista os schemas e as tabelas de um banco, com o tamanho de cada uma.",
        parametros: &[DB],
        exemplo: r#"{"op":"tabelas","database":"loja"}"#,
        ferramenta_mcp: true,
    },
    Operacao {
        nome: "esquema",
        apelidos: &[],
        resumo: "Descreve uma tabela: colunas, tipos, índices, chaves \
                 estrangeiras e a marca de dado pessoal.",
        parametros: &[DB, TAB],
        exemplo: r#"{"op":"esquema","database":"loja","tabela":"clientes"}"#,
        ferramenta_mcp: true,
    },
    Operacao {
        nome: "sistabelas",
        apelidos: &["systables"],
        resumo: "O catálogo de tabelas do banco, em forma de linhas -- o \
                 equivalente do information_schema.",
        parametros: &[DB],
        exemplo: r#"{"op":"sistabelas","database":"loja"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "siscolunas",
        apelidos: &["syscolumns"],
        resumo: "O catálogo de colunas, de uma tabela ou do banco inteiro.",
        parametros: &[DB, opc("tabela", "string", "limita a uma tabela")],
        exemplo: r#"{"op":"siscolunas","database":"loja","tabela":"clientes"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "dados_pessoais",
        apelidos: &["lgpd"],
        resumo: "Audita onde estão os dados pessoais do banco: mostra QUE a \
                 coluna guarda CPF, nunca o CPF.",
        parametros: &[DB, opc("tabela", "string", "limita a uma tabela")],
        exemplo: r#"{"op":"dados_pessoais","database":"loja"}"#,
        ferramenta_mcp: true,
    },
    Operacao {
        nome: "sequencias",
        apelidos: &["sequences"],
        resumo: "O próximo número de cada coluna Sequence do banco.",
        parametros: &[DB],
        exemplo: r#"{"op":"sequencias","database":"loja"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "ajustar_sequencia",
        apelidos: &[],
        resumo: "Muda o próximo número de uma coluna Sequence.",
        parametros: &[
            DB,
            TAB,
            obr("proxima", "integer", "o próximo número a distribuir"),
        ],
        exemplo: r#"{"op":"ajustar_sequencia","database":"loja","tabela":"clientes","proxima":5000}"#,
        ferramenta_mcp: false,
    },
    // ----------------------------------------------------------- ler dado
    Operacao {
        nome: "ler",
        apelidos: &[],
        resumo: "Lê uma linha pelo rowid.",
        parametros: &[
            DB,
            TAB,
            ROWID,
            opc(
                "com_versao",
                "boolean",
                "traz a versão do slot, para o `atualizar` poder recusar escrita concorrente",
            ),
        ],
        exemplo: r#"{"op":"ler","database":"loja","tabela":"clientes","rowid":42}"#,
        ferramenta_mcp: true,
    },
    Operacao {
        nome: "varrer",
        apelidos: &[],
        resumo: "Percorre a tabela na ordem de digitação -- ou na de um índice \
                 --, com paginação.",
        parametros: &[
            DB,
            TAB,
            opc("pular", "integer", "quantas linhas saltar antes da página"),
            MAX,
            opc(
                "indice",
                "string",
                "percorre na ordem deste índice em vez da de digitação",
            ),
            opc(
                "visao",
                "string",
                "`vivos` (padrão), `marcados` ou `todos`, quanto à exclusão reversível",
            ),
            opc(
                "desde_rownum",
                "integer",
                "começa no número de ordem informado, em vez de contar do começo",
            ),
        ],
        exemplo: r#"{"op":"varrer","database":"loja","tabela":"clientes","pular":0,"max":50}"#,
        ferramenta_mcp: true,
    },
    Operacao {
        nome: "buscar",
        apelidos: &[],
        resumo: "Desce um índice até a chave exata e devolve as linhas dela.",
        parametros: &[
            DB,
            TAB,
            obr("indice", "string", "o nome do índice"),
            obr(
                "chave",
                "array",
                "os valores da chave, na ordem em que o índice as declara",
            ),
            MAX,
        ],
        exemplo: r#"{"op":"buscar","database":"loja","tabela":"clientes","indice":"porId","chave":[42]}"#,
        ferramenta_mcp: true,
    },
    Operacao {
        nome: "sql",
        apelidos: &[],
        resumo: "Traduz um SELECT simples para as operações do protocolo e o \
                 executa pelo MESMO portão de permissão. Também atende os \
                 comandos de rotina no dialeto do MySQL(R): CREATE/DROP \
                 TRIGGER e PROCEDURE, CALL e SHOW TRIGGERS/PROCEDURES — \
                 criar, excluir e listar exigem administrar; CALL roda com o \
                 poder de quem chama. Ver docs/TRIGGERS.md.",
        parametros: &[
            opc(
                "database",
                "string",
                "o banco corrente, quando o FROM não disser qual",
            ),
            obr("texto", "string", "o comando SQL; `sql` também é aceito"),
        ],
        exemplo: r#"{"op":"sql","database":"loja","texto":"SELECT * FROM clientes LIMIT 10"}"#,
        ferramenta_mcp: true,
    },
    Operacao {
        nome: "pivotar",
        apelidos: &["pivot"],
        resumo: "Tabulação cruzada: soma, conta ou tira a média de uma coluna \
                 cruzando duas outras.",
        parametros: &[
            DB,
            TAB,
            obr("chave", "string", "a coluna que vira LINHA"),
            opc("coluna", "string", "a coluna que vira COLUNA do cruzamento"),
            opc("valor", "string", "a coluna somada; sem ela, conta linhas"),
            opc(
                "agregador",
                "string",
                "`contar`, `somar`, `media`, `minimo` ou `maximo`",
            ),
            opc(
                "granularidade",
                "string",
                "para chave de data: `dia`, `mes`, `ano`",
            ),
            MAX,
        ],
        exemplo: r#"{"op":"pivotar","database":"loja","tabela":"vendas","chave":"cidade","valor":"total","agregador":"somar"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "juntar",
        apelidos: &["join"],
        resumo: "Junta duas tabelas do mesmo banco por uma chave.",
        parametros: &[
            DB,
            obr("a", "object", "o lado esquerdo: `{tabela, chave, prefixo}`"),
            obr("b", "object", "o lado direito: `{tabela, chave, prefixo}`"),
            opc(
                "tipo",
                "string",
                "`interna`, `esquerda`, `direita` ou `completa`",
            ),
            MAX,
        ],
        exemplo: r#"{"op":"juntar","database":"loja","a":{"tabela":"pedidos","chave":"cliente_id"},"b":{"tabela":"clientes","chave":"id"},"tipo":"interna"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "unir",
        apelidos: &["union"],
        resumo: "Empilha as linhas de duas ou mais tabelas do mesmo banco.",
        parametros: &[
            DB,
            obr("tabelas", "array", "os nomes das tabelas, em ordem"),
            opc(
                "modo",
                "string",
                "`tudo` mantém repetidas; `distinto` tira as iguais",
            ),
            MAX,
        ],
        exemplo: r#"{"op":"unir","database":"loja","tabelas":["clientes_2025","clientes_2026"],"modo":"tudo"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "checksum",
        apelidos: &["soma_de_verificacao"],
        resumo: "Um número que resume o conteúdo da tabela, para comparar duas \
                 cópias sem transportá-las.",
        parametros: &[DB, TAB],
        exemplo: r#"{"op":"checksum","database":"loja","tabela":"clientes"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "exportar",
        apelidos: &["export"],
        resumo: "Leva a tabela inteira embora, em CSV, JSON ou SQL.",
        parametros: &[
            DB,
            TAB,
            opc("formato", "string", "`csv`, `json`, `jsonl` ou `sql`"),
            MAX,
        ],
        exemplo: r#"{"op":"exportar","database":"loja","tabela":"clientes","formato":"csv"}"#,
        ferramenta_mcp: false,
    },
    // -------------------------------------------------------- gravar dado
    Operacao {
        nome: "inserir",
        apelidos: &[],
        resumo: "Inclui uma linha no fim da ordem de digitação.",
        parametros: &[
            DB,
            TAB,
            opc(
                "valores",
                "object",
                "coluna: valor; `linha` é o nome antigo e continua valendo",
            ),
        ],
        exemplo: r#"{"op":"inserir","database":"loja","tabela":"clientes","valores":{"nome":"Maria"}}"#,
        ferramenta_mcp: true,
    },
    Operacao {
        nome: "inserir_lote",
        apelidos: &["importar", "carga"],
        resumo: "Inclui muitas linhas de uma vez, por lista ou por um texto \
                 CSV/JSON colado.",
        parametros: &[
            DB,
            TAB,
            opc("linhas", "array", "a lista de objetos coluna: valor"),
            opc("texto", "string", "a carga colada, em vez de `linhas`"),
            opc(
                "formato",
                "string",
                "`csv`, `json` ou `jsonl`, para o `texto`",
            ),
            opc(
                "parar_no_erro",
                "boolean",
                "para na primeira linha ruim em vez de seguir e relatar",
            ),
        ],
        exemplo: r#"{"op":"inserir_lote","database":"loja","tabela":"clientes","linhas":[{"nome":"Maria"},{"nome":"João"}]}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "importar_conferir",
        apelidos: &[],
        resumo: "Confere uma carga colada contra o esquema, sem gravar nada.",
        parametros: &[
            DB,
            TAB,
            obr("texto", "string", "a carga a conferir"),
            opc("formato", "string", "`csv`, `json` ou `jsonl`"),
        ],
        exemplo: r#"{"op":"importar_conferir","database":"loja","tabela":"clientes","texto":"nome\nMaria","formato":"csv"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "atualizar",
        apelidos: &[],
        resumo: "Altera uma linha pelo rowid, coluna a coluna.",
        parametros: &[
            DB,
            TAB,
            ROWID,
            opc("valores", "object", "coluna: valor; só as que mudam"),
            opc(
                "versao",
                "integer",
                "a versão lida; quem manda ganha a recusa por escrita concorrente",
            ),
        ],
        exemplo: r#"{"op":"atualizar","database":"loja","tabela":"clientes","rowid":42,"valores":{"nome":"Maria"}}"#,
        ferramenta_mcp: true,
    },
    Operacao {
        nome: "excluir",
        apelidos: &[],
        resumo: "Exclui uma linha -- por padrão de forma reversível, indo para \
                 a lixeira.",
        parametros: &[
            DB,
            TAB,
            ROWID,
            opc("motivo", "string", "a frase que vai para o `.reason`"),
            opc(
                "fisico",
                "boolean",
                "apaga de vez, sem passar pela lixeira; não há desfazer",
            ),
            opc(
                "versao",
                "integer",
                "a versão lida, para recusar concorrência",
            ),
        ],
        exemplo: r#"{"op":"excluir","database":"loja","tabela":"clientes","rowid":42,"motivo":"duplicidade"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "restaurar",
        apelidos: &[],
        resumo: "Desfaz uma exclusão reversível: a linha volta com o mesmo rowid.",
        parametros: &[
            DB,
            TAB,
            ROWID,
            opc("motivo", "string", "por que está voltando"),
        ],
        exemplo: r#"{"op":"restaurar","database":"loja","tabela":"clientes","rowid":42}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "lixeira",
        apelidos: &["trash"],
        resumo: "As linhas excluídas de forma reversível, com quem excluiu e quando.",
        parametros: &[
            DB,
            TAB,
            opc("limite", "integer", "quantas trazer"),
            opc("pular", "integer", "quantas saltar"),
            opc("com_anexos", "boolean", "traz também os binários e memos"),
        ],
        exemplo: r#"{"op":"lixeira","database":"loja","tabela":"clientes","limite":50}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "motivos",
        apelidos: &["reasons"],
        resumo: "As frases gravadas no `.reason` -- por que cada exclusão aconteceu.",
        parametros: &[
            DB,
            TAB,
            opc("rowid", "integer", "só os motivos deste registro"),
            opc("limite", "integer", "quantos trazer"),
            opc("pular", "integer", "quantos saltar"),
        ],
        exemplo: r#"{"op":"motivos","database":"loja","tabela":"clientes","limite":50}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "trilha",
        apelidos: &["trilha_lgpd"],
        resumo: "A trilha de LGPD do `.lgpd` -- quem alterou e quem leu as \
                 colunas marcadas como dado pessoal, com valor antes e depois.",
        parametros: &[
            DB,
            TAB,
            opc("rowid", "integer", "só a trilha desta linha"),
            opc("tipo", "string", "`alteracao` ou `acesso`; vazio traz os dois"),
            opc("limite", "integer", "quantos trazer"),
            opc("pular", "integer", "quantos saltar"),
        ],
        exemplo: r#"{"op":"trilha","database":"loja","tabela":"clientes","limite":50}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "marcar_lgpd",
        apelidos: &["marcar_dado_pessoal"],
        resumo: "Classifica colunas como dado pessoal (`nao`, `pessoal` ou \
                 `sensivel`). É o que liga a trilha `.lgpd` daquela coluna.",
        parametros: &[
            DB,
            TAB,
            obr(
                "colunas",
                "object",
                "`{\"cpf\":\"pessoal\",\"laudo\":\"sensivel\"}`; `true` vale por `pessoal`",
            ),
        ],
        exemplo: r#"{"op":"marcar_lgpd","database":"loja","tabela":"clientes","colunas":{"cpf":"pessoal"}}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "esvaziar_lixeira",
        apelidos: &[],
        resumo: "Apaga de vez o que está na lixeira. É a única operação que \
                 apaga dado sem rede embaixo.",
        parametros: &[
            DB,
            TAB,
            obr(
                "motivo",
                "string",
                "por que está esvaziando; fica no `.reason`",
            ),
        ],
        exemplo: r#"{"op":"esvaziar_lixeira","database":"loja","tabela":"clientes","motivo":"expurgo anual"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "bulkinsert",
        apelidos: &[],
        resumo: "Reserva a tabela para uma carga exclusiva desta conexão, ou solta a reserva.",
        parametros: &[
            DB,
            TAB,
            obr("ligado", "boolean", "true reserva, false solta"),
        ],
        exemplo: r#"{"op":"bulkinsert","database":"loja","tabela":"clientes","ligado":true}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "cargas",
        apelidos: &[],
        resumo: "Quem reservou qual tabela para carga, e desde quando.",
        parametros: &[
            opc("database", "string", "limita a um banco"),
            opc("tabela", "string", "limita a uma tabela"),
        ],
        exemplo: r#"{"op":"cargas"}"#,
        ferramenta_mcp: false,
    },
    // ----------------------------------------------------- criar e apagar
    Operacao {
        nome: "criar_database",
        apelidos: &[],
        resumo: "Cria a pasta de um banco de dados.",
        parametros: &[obr("database", "string", "o nome do banco")],
        exemplo: r#"{"op":"criar_database","database":"loja"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "criar_schema",
        apelidos: &[],
        resumo: "Cria um schema dentro de um banco -- a pasta que agrupa tabelas.",
        parametros: &[DB, obr("schema", "string", "o nome do schema")],
        exemplo: r#"{"op":"criar_schema","database":"loja","schema":"filial"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "criar_tabela",
        apelidos: &[],
        resumo: "Cria uma tabela: colunas, índices, paginação e chaves estrangeiras.",
        parametros: &[
            DB,
            TAB,
            obr(
                "colunas",
                "array",
                "`{nome, tipo, obrigatoria, caption, mascara, dado_pessoal}`",
            ),
            opc(
                "indices",
                "array",
                "`{nome, colunas, unico, primario}`; a coluna aceita `nome desc` e `nome nocase`",
            ),
            opc(
                "chaves_estrangeiras",
                "array",
                "`{nome, colunas, tabela_ref, colunas_ref, ao_excluir, ao_alterar}`;                  a ação aceita restringir, cascata, anular ou nada",
            ),
            opc(
                "schema",
                "string",
                "o schema, quando o nome da tabela não o traz",
            ),
            opc(
                "registros_por_arquivo",
                "integer",
                "liga a paginação: quantas linhas por volume",
            ),
            opc("digitos", "integer", "largura do sufixo `_001` do volume"),
            opc(
                "motivo_obrigatorio",
                "boolean",
                "nenhuma exclusão nesta tabela passa sem frase escrita",
            ),
        ],
        exemplo: r#"{"op":"criar_tabela","database":"loja","tabela":"clientes","colunas":[{"nome":"id","tipo":"Sequence","obrigatoria":true},{"nome":"nome","tipo":"Str(60)"}],"indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "declarar_fk",
        apelidos: &[],
        resumo: "Declara uma chave estrangeira numa tabela que já existe -- \
                 declara, não impõe: o motor não a confere na gravação.",
        parametros: &[
            DB,
            TAB,
            obr("nome", "string", "o nome da chave (ex.: fk_cliente)"),
            obr("colunas", "array", "as colunas locais, por nome"),
            obr("tabela_ref", "string", "a tabela referenciada"),
            opc(
                "colunas_ref",
                "array",
                "as colunas de lá; sem elas, as de mesmo nome",
            ),
            opc(
                "ao_excluir",
                "string",
                "restringir (padrão), cascata, anular ou nada",
            ),
            opc("ao_alterar", "string", "as mesmas quatro ações"),
        ],
        exemplo: r#"{"op":"declarar_fk","database":"loja","tabela":"pedidos","nome":"fk_cliente","colunas":["cliente_id"],"tabela_ref":"clientes","colunas_ref":["id"]}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "excluir_fk",
        apelidos: &[],
        resumo: "Desfaz a declaração de uma chave estrangeira, pelo nome. Não \
                 toca em dado nenhum -- a chave nunca foi imposta.",
        parametros: &[DB, TAB, obr("nome", "string", "o nome da chave declarada")],
        exemplo: r#"{"op":"excluir_fk","database":"loja","tabela":"pedidos","nome":"fk_cliente"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "excluir_tabela",
        apelidos: &[],
        resumo: "Apaga os arquivos de uma tabela. Não há desfazer, e por isso \
                 exige o nome repetido.",
        parametros: &[
            DB,
            TAB,
            obr(
                "confirmar",
                "string",
                "o nome da tabela, repetido; um nome errado aqui perde tudo",
            ),
        ],
        exemplo: r#"{"op":"excluir_tabela","database":"loja","tabela":"clientes","confirmar":"clientes"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "duplicar_tabela",
        apelidos: &[],
        resumo: "Copia uma tabela inteira para outro nome no MESMO banco, com \
                 os mesmos rowids.",
        parametros: &[DB, TAB, obr("destino", "string", "o nome da cópia")],
        exemplo: r#"{"op":"duplicar_tabela","database":"loja","tabela":"clientes","destino":"clientes_bkp"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "copiar_tabela",
        apelidos: &[],
        resumo: "Copia uma tabela para OUTRO banco -- o «colar» da tela.",
        parametros: &[
            DB,
            TAB,
            opc("destino_database", "string", "o banco de destino"),
            opc(
                "destino",
                "string",
                "o nome no destino; sem ele, o mesmo nome",
            ),
        ],
        exemplo: r#"{"op":"copiar_tabela","database":"loja","tabela":"clientes","destino_database":"arquivo"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "reindexar",
        apelidos: &[],
        resumo: "Reconstrói o `.ndx` do zero, varrendo o `.reg`.",
        parametros: &[DB, TAB],
        exemplo: r#"{"op":"reindexar","database":"loja","tabela":"clientes"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "verificar",
        apelidos: &[],
        resumo: "Confere a integridade dos arquivos da tabela, CRC por CRC.",
        parametros: &[DB, opc("tabela", "string", "sem ela, o banco inteiro")],
        exemplo: r#"{"op":"verificar","database":"loja","tabela":"clientes"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "reparar",
        apelidos: &[],
        resumo: "Tenta consertar o que o `verificar` achou, isolando o slot ruim.",
        parametros: &[DB, TAB],
        exemplo: r#"{"op":"reparar","database":"loja","tabela":"clientes"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "diario",
        apelidos: &[],
        resumo: "O `.log` da tabela: toda inclusão, alteração e exclusão, com \
                 autor, data e hora.",
        parametros: &[
            DB,
            TAB,
            opc("rowid", "integer", "só os eventos deste registro"),
            MAX,
        ],
        exemplo: r#"{"op":"diario","database":"loja","tabela":"clientes","max":100}"#,
        ferramenta_mcp: false,
    },
    // --------------------------------------------------------- em memória
    Operacao {
        nome: "memoria_carregar",
        apelidos: &[],
        resumo: "Traz a tabela inteira para a RAM, para consulta sem tocar o disco.",
        parametros: &[
            DB,
            TAB,
            opc(
                "mapear",
                "boolean",
                "mapeia o arquivo em vez de copiar; economiza cópia",
            ),
        ],
        exemplo: r#"{"op":"memoria_carregar","database":"loja","tabela":"clientes"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "memoria_liberar",
        apelidos: &[],
        resumo: "Solta da RAM uma tabela residente.",
        parametros: &[DB, TAB],
        exemplo: r#"{"op":"memoria_liberar","database":"loja","tabela":"clientes"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "memoria",
        apelidos: &[],
        resumo: "O que está residente na RAM, com bytes e idade.",
        parametros: &[],
        exemplo: r#"{"op":"memoria"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "SelectMemory",
        apelidos: &["selectmemory", "selecionar_memoria"],
        resumo: "Consulta a cópia residente: filtra, ordena e pagina sem tocar o disco.",
        parametros: &[
            DB,
            TAB,
            opc("colunas", "array", "as colunas a devolver; sem elas, todas"),
            opc(
                "onde",
                "array",
                "filtros `{coluna, op, valor}` com `=`, `<>`, `<`, `<=`, `>`, `>=`",
            ),
            opc("ordenar", "string", "a coluna de ordenação"),
            opc("desc", "boolean", "ordem decrescente"),
            opc("pular", "integer", "quantas linhas saltar"),
            MAX,
        ],
        exemplo: r#"{"op":"SelectMemory","database":"loja","tabela":"clientes","onde":[{"coluna":"cidade","op":"=","valor":"Blumenau"}]}"#,
        ferramenta_mcp: false,
    },
    // -------------------------------------------------------- replicação
    Operacao {
        nome: "posicao",
        apelidos: &[],
        resumo: "Quantos eventos cada tabela do banco tem -- é o marco que a \
                 réplica compara com o dela.",
        parametros: &[
            DB,
            opc(
                "com_esquema",
                "boolean",
                "traz o esquema serializado de cada tabela",
            ),
        ],
        exemplo: r#"{"op":"posicao","database":"loja","com_esquema":true}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "replicar",
        apelidos: &[],
        resumo: "Entrega o fluxo de eventos de uma tabela a partir de uma posição.",
        parametros: &[
            DB,
            TAB,
            obr("desde", "integer", "o número do primeiro evento a mandar"),
            MAX,
            opc(
                "para",
                "string",
                "o `id_servidor` de quem pede; eventos que NASCERAM nele não \
                 voltam (é o que mata o laço do bidirecional), e a posição \
                 `ate` anda por cima deles mesmo assim",
            ),
        ],
        exemplo: r#"{"op":"replicar","database":"loja","tabela":"clientes","desde":0,"max":500,"para":"servidor-b"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "replicacao_estado",
        apelidos: &[],
        resumo: "O estado do laço de replicação deste servidor: papel vivo, \
                 posição consumida por origem e tabela, última rodada, último \
                 erro e as tabelas recusadas com o motivo.",
        parametros: &[],
        exemplo: r#"{"op":"replicacao_estado"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "replicacao_testar",
        apelidos: &[],
        resumo: "Prova a ligação com o outro servidor pela MESMA conexão e \
                 autenticação do laço de replicação, e diz o que ele serve: \
                 papel, `id_servidor`, se a imagem da linha está ligada, as \
                 tabelas com a chave de cada uma, e os impedimentos por modo.",
        parametros: &[
            opc(
                "origem",
                "string",
                "o nome de uma origem já em `replicacao.origens`; é o caminho \
                 preferido, porque a credencial não sai do servidor",
            ),
            opc("host", "string", "o endereço do outro servidor, para uma ligação nova"),
            opc("porta", "integer", "a porta dele; 5000 quando omitida"),
            opc("token", "string", "o token da porta de dados dele"),
            opc("usuario", "string", "o login de replicação"),
            opc(
                "senha_hash",
                "string",
                "o hash da senha desse login (o mesmo texto do cadastro) — \
                 nunca a senha em claro; nada disso volta na resposta",
            ),
            opc("database", "string", "olhar só este banco; sem ele, todos"),
        ],
        exemplo: r#"{"op":"replicacao_testar","origem":"curitiba"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "spare_promover",
        apelidos: &[],
        resumo: "Promove este servidor a primário: o laço de réplica para, a \
                 escrita abre e o papel vira source. Operação local e manual; \
                 a resposta avisa o que ajustar no config.json para o próximo \
                 arranque.",
        parametros: &[opc(
            "motivo",
            "string",
            "por que a promoção aconteceu; entra no log e na resposta, para \
             «pedido manual» e «o primário não respondeu» não se confundirem",
        )],
        exemplo: r#"{"op":"spare_promover","motivo":"manutencao programada no primario"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "aplicar",
        apelidos: &[],
        resumo: "Grava aqui um evento vindo do source, com o rowid dele.",
        parametros: &[
            DB,
            TAB,
            opc("eventos", "array", "vários eventos de uma vez"),
            opc("rowid", "integer", "o rowid do evento, quando é um só"),
            opc(
                "operacao",
                "string",
                "`inclusao`, `alteracao` ou `exclusao`, quando é um só",
            ),
            opc("imagem", "string", "a linha em hexadecimal, quando é um só"),
        ],
        exemplo: r#"{"op":"aplicar","database":"loja","tabela":"clientes","rowid":42,"operacao":"inclusao","imagem":"00ff..."}"#,
        ferramenta_mcp: false,
    },
    // ---------------------------------------------------------- cluster
    Operacao {
        nome: "cluster_pulso",
        apelidos: &[],
        resumo: "O pulso entre nós do cluster: quem manda diz id, papel, época \
                 e posição do diário, e recebe o mesmo de volta.",
        parametros: &[
            obr("id", "string", "o id do nó que está pulsando"),
            obr("papel", "string", "`master` ou `replica`, o papel VIVO de quem pulsa"),
            opc("epoca", "integer", "a época que o nó conhece; cresce a cada eleição"),
            opc("posicao", "integer", "a posição do diário do nó, somada nas tabelas"),
            opc("prioridade", "integer", "o desempate de eleição do nó"),
        ],
        exemplo: r#"{"op":"cluster_pulso","id":"no2","papel":"replica","epoca":1,"posicao":1234}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "cluster_estado",
        apelidos: &[],
        resumo: "Quem é o master agora, a época e o mapa dos nós -- responde \
                 igual em qualquer nó, e é o endereço único do cluster.",
        parametros: &[],
        exemplo: r#"{"op":"cluster_estado"}"#,
        ferramenta_mcp: false,
    },
    // ------------------------------------------------------- o servidor
    Operacao {
        nome: "config",
        apelidos: &[],
        resumo: "Devolve o `config.json` como o servidor o leu, sem segredo nenhum dentro.",
        parametros: &[],
        exemplo: r#"{"op":"config"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "config_gravar",
        apelidos: &[],
        resumo: "Grava campos do `config.json` no disco, atomicamente. Exige administrar; \
                 token, seguranca, usuarios, cifra e replicacao ficam de fora.",
        parametros: &[obr(
            "campos",
            "object",
            "os campos a gravar, pelo nome do config.json: \
             {\"max_linhas\":500,\"backup.hora\":\"03:00\"}",
        )],
        exemplo: r#"{"op":"config_gravar","campos":{"max_linhas":500}}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "usuarios",
        apelidos: &[],
        resumo: "O cadastro e o poder de cada um. Nunca devolve senha nem hash.",
        parametros: &[],
        exemplo: r#"{"op":"usuarios"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "acessos",
        apelidos: &[],
        resumo: "As últimas linhas do log de acessos, com IP, usuário e duração.",
        parametros: &[MAX],
        exemplo: r#"{"op":"acessos","max":50}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "ips",
        apelidos: &[],
        resumo: "O log de acessos resumido por IP.",
        parametros: &[],
        exemplo: r#"{"op":"ips"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "bloqueios",
        apelidos: &[],
        resumo: "Os IPs bloqueados agora, com o motivo e até quando.",
        parametros: &[],
        exemplo: r#"{"op":"bloqueios"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "desbloquear",
        apelidos: &[],
        resumo: "Tira um IP da lista de bloqueio.",
        parametros: &[obr("ip", "string", "o endereço a soltar")],
        exemplo: r#"{"op":"desbloquear","ip":"203.0.113.9"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "bloqueios_exportar",
        apelidos: &[],
        resumo: "A blacklist ativa em texto pronto para um firewall de verdade aplicar.",
        parametros: &[opc(
            "formato",
            "string",
            "texto (um IP por linha), iptables, nftables ou fail2ban",
        )],
        exemplo: r#"{"op":"bloqueios_exportar","formato":"nftables"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "whitelist_salvar",
        apelidos: &[],
        resumo: "Substitui a whitelist editável: IPs e faixas CIDR que nunca são bloqueados.",
        parametros: &[obr(
            "whitelist",
            "array",
            "a lista completa, ex. [\"127.0.0.1\",\"192.168.50.0/24\"]",
        )],
        exemplo: r#"{"op":"whitelist_salvar","whitelist":["127.0.0.1"]}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "mensagens",
        apelidos: &[],
        resumo: "O estado da tabela de mensagens: idioma em uso, quantas linhas, o que falta semear.",
        parametros: &[],
        exemplo: r#"{"op":"mensagens"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "mensagens_semear",
        apelidos: &[],
        resumo: "Cria phxsys.mensagens se falta e grava as mensagens de fábrica ausentes, sem tocar as existentes.",
        parametros: &[],
        exemplo: r#"{"op":"mensagens_semear"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "idiomas",
        apelidos: &[],
        resumo: "O estado da tabela de textos da tela: quantos há, quantos estão traduzidos.",
        parametros: &[opc(
            "idioma",
            "string",
            "de qual idioma contar as traduções (padrão: Portugues)",
        )],
        exemplo: r#"{"op":"idiomas","idioma":"Frances"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "idiomas_carga",
        apelidos: &[],
        resumo: "Cria phxsys.mensagens se falta e semeia os textos de tela ausentes, \
                 sem tocar nos que já existem.",
        parametros: &[],
        exemplo: r#"{"op":"idiomas_carga"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "idiomas_padrao",
        apelidos: &[],
        resumo: "Devolve os textos de fábrica POR CIMA do que está gravado — de um idioma \
                 só, ou dos seis. Apaga tradução: peça confirmação antes.",
        parametros: &[
            opc("idioma", "string", "sobrescrever só este idioma"),
            opc("tudo", "boolean", "sobrescrever os seis idiomas"),
        ],
        exemplo: r#"{"op":"idiomas_padrao","idioma":"Frances"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "idiomas_exportar",
        apelidos: &[],
        resumo: "A tabela de textos inteira em JSON, para guardar fora do banco.",
        parametros: &[],
        exemplo: r#"{"op":"idiomas_exportar"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "idiomas_importar",
        apelidos: &[],
        resumo: "Devolve um backup de textos para a tabela, gravando por TextName.",
        parametros: &[obr(
            "backup",
            "object",
            "o objeto que o idiomas_exportar devolveu",
        )],
        exemplo: r#"{"op":"idiomas_importar","backup":{"versao":1,"linhas":[]}}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "estatisticas",
        apelidos: &["estatisticas_uso"],
        resumo: "Resume o log de acessos: quem pediu o quê, quantas vezes e quão devagar.",
        parametros: &[opc("horas", "integer", "a janela a resumir, em horas")],
        exemplo: r#"{"op":"estatisticas","horas":24}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "sessoes",
        apelidos: &["processlist"],
        resumo: "As conexões vivas, com login, IP e o que cada uma está fazendo.",
        parametros: &[],
        exemplo: r#"{"op":"sessoes"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "encerrar_sessao",
        apelidos: &["kill"],
        resumo: "Derruba uma conexão pelo número dela.",
        parametros: &[obr("id", "integer", "o id da ligação, do `sessoes`")],
        exemplo: r#"{"op":"encerrar_sessao","id":17}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "telemetria",
        apelidos: &[],
        resumo: "O painel ao vivo: as séries do servidor e as atividades vivas, cada uma com o peso que dá o tamanho da bolha.",
        parametros: &[opc(
            "amostras",
            "integer",
            "quantos instantes da série devolver, do mais antigo ao mais novo (teto de 200)",
        )],
        exemplo: r#"{"op":"telemetria","amostras":120}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "telemetria_ligar",
        apelidos: &[],
        resumo: "Liga a coleta da telemetria.",
        parametros: &[],
        exemplo: r#"{"op":"telemetria_ligar"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "telemetria_desligar",
        apelidos: &[],
        resumo: "Desliga a coleta e descarta a série guardada.",
        parametros: &[],
        exemplo: r#"{"op":"telemetria_desligar"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "telemetria_encerrar",
        apelidos: &[],
        resumo: "Encerra a operação em curso de uma atividade, em ponto seguro — sem interromper gravação pela metade.",
        parametros: &[obr(
            "id",
            "string",
            "o identificador da atividade, da lista de `telemetria`: `dados:17` ou `web:a1b2c3d4`",
        )],
        exemplo: r#"{"op":"telemetria_encerrar","id":"dados:17"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "painel",
        apelidos: &[],
        resumo: "Os números do painel: bancos, tabelas e linhas que quem olha poderia abrir.",
        parametros: &[
            opc("database", "string", "limita a um banco"),
            opc("tabela", "string", "limita a uma tabela"),
        ],
        exemplo: r#"{"op":"painel"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "sistema",
        apelidos: &[],
        resumo: "O monitor da MÁQUINA: CPU, memória, discos e placas de rede.",
        parametros: &[],
        exemplo: r#"{"op":"sistema"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "servico",
        apelidos: &[],
        resumo: "Diz se a porta de dados está no ar e em qual endereço.",
        parametros: &[],
        exemplo: r#"{"op":"servico"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "servico_parar",
        apelidos: &[],
        resumo: "Para de aceitar conexões na porta de dados, sem derrubar o processo.",
        parametros: &[],
        exemplo: r#"{"op":"servico_parar"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "servico_subir",
        apelidos: &[],
        resumo: "Volta a aceitar conexões, aqui ou em outro endereço.",
        parametros: &[opc(
            "bind",
            "string",
            "o endereço novo; sem ele, o mesmo de antes",
        )],
        exemplo: r#"{"op":"servico_subir","bind":"0.0.0.0:5001"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "backup",
        apelidos: &[],
        resumo: "Copia os arquivos para uma pasta de destino, com ou sem compactação.",
        parametros: &[
            obr("destino", "string", "a pasta de destino"),
            opc("database", "string", "só este banco; sem ele, todos"),
            opc("zip", "boolean", "compacta em vez de copiar solto"),
        ],
        exemplo: r#"{"op":"backup","destino":"/backup/phxsql","zip":true}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "conferir_backup",
        apelidos: &[],
        resumo: "Confere um backup contra os CRCs gravados nele.",
        parametros: &[obr("destino", "string", "a pasta do backup")],
        exemplo: r#"{"op":"conferir_backup","destino":"/backup/phxsql"}"#,
        ferramenta_mcp: false,
    },
    // ------------------------------------------------------------- jobs
    Operacao {
        nome: "jobs",
        apelidos: &["job_listar"],
        resumo: "O cadastro de tarefas agendadas com o estado completo de cada uma \
                 (nunca rodou, agendado, rodando, ok, falhou, desligado; última \
                 corrida e próxima prevista), e opcionalmente o histórico.",
        parametros: &[opc(
            "historico",
            "boolean",
            "traz também o que já rodou e como terminou",
        )],
        exemplo: r#"{"op":"jobs","historico":true}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "job_salvar",
        apelidos: &[],
        resumo: "Grava ou substitui um job pelo nome.",
        parametros: &[obr(
            "job",
            "object",
            "`{nome, agenda, usuario, pedido}` -- o pedido é um pedido do protocolo",
        )],
        exemplo: r#"{"op":"job_salvar","job":{"nome":"backup-noturno","agenda":"0 2 * * *","usuario":"adriano","pedido":{"op":"backup","destino":"/backup"}}}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "job_ligar",
        apelidos: &[],
        resumo: "Liga ou desliga um job pelo nome, sem mexer no resto da ficha.",
        parametros: &[
            obr("nome", "string", "o nome do job"),
            obr("ligado", "boolean", "true liga, false desliga"),
        ],
        exemplo: r#"{"op":"job_ligar","nome":"backup-noturno","ligado":true}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "job_excluir",
        apelidos: &[],
        resumo: "Apaga um job; o histórico dele fica.",
        parametros: &[obr("nome", "string", "o nome do job")],
        exemplo: r#"{"op":"job_excluir","nome":"backup-noturno"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "job_rodar",
        apelidos: &[],
        resumo: "Roda um job agora, fora da agenda.",
        parametros: &[obr("nome", "string", "o nome do job")],
        exemplo: r#"{"op":"job_rodar","nome":"backup-noturno"}"#,
        ferramenta_mcp: false,
    },
    // --------------------------------------------------------- profiler
    Operacao {
        nome: "profiler_ligar",
        apelidos: &[],
        resumo: "Liga a captura dos pedidos, com filtro por operação, usuário ou base.",
        parametros: &[
            opc("arquivo", "string", "grava a captura também neste arquivo"),
            opc("guardar", "integer", "quantos pedidos manter em memória"),
            opc("operacao", "string", "captura só esta operação"),
            opc("usuario", "string", "captura só este login"),
            opc("database", "string", "captura só este banco"),
            opc("so_escrita", "boolean", "ignora quem só lê"),
        ],
        exemplo: r#"{"op":"profiler_ligar","guardar":500,"so_escrita":true}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "profiler_desligar",
        apelidos: &[],
        resumo: "Desliga a captura.",
        parametros: &[],
        exemplo: r#"{"op":"profiler_desligar"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "profiler",
        apelidos: &[],
        resumo: "O que foi capturado, com o texto do pedido já redigido.",
        parametros: &[
            MAX,
            opc(
                "desde_serial",
                "integer",
                "só o que veio depois deste número de ordem",
            ),
        ],
        exemplo: r#"{"op":"profiler","max":200,"desde_serial":0}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "profiler_limpar",
        apelidos: &[],
        resumo: "Esvazia o que foi capturado, deixando a captura ligada.",
        parametros: &[],
        exemplo: r#"{"op":"profiler_limpar"}"#,
        ferramenta_mcp: false,
    },
    // ----------------------------------------------------------- dblink
    Operacao {
        nome: "dblink",
        apelidos: &[],
        resumo: "Lista as ligações para outros bancos e os motores suportados.",
        parametros: &[],
        exemplo: r#"{"op":"dblink"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "dblink_salvar",
        apelidos: &[],
        resumo: "Grava ou substitui uma ligação pelo nome.",
        parametros: &[
            obr("nome", "string", "o nome da ligação"),
            obr("motor", "string", "`mysql` ou `postgres`"),
            obr("host", "string", "o endereço do outro servidor"),
            opc("porta", "integer", "a porta; sem ela, a do motor"),
            opc("usuario", "string", "o login no outro servidor"),
            opc(
                "senha",
                "string",
                "a senha; prefira `senha_env` com o nome de uma variável de ambiente",
            ),
            opc("database", "string", "o banco padrão da ligação"),
        ],
        exemplo: r#"{"op":"dblink_salvar","nome":"erp","motor":"mysql","host":"10.1.1.9","usuario":"leitor","senha_env":"ERP_SENHA","database":"producao"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "dblink_excluir",
        apelidos: &[],
        resumo: "Apaga uma ligação do cadastro.",
        parametros: &[obr("nome", "string", "o nome da ligação")],
        exemplo: r#"{"op":"dblink_excluir","nome":"erp"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "dblink_testar",
        apelidos: &[],
        resumo: "Conecta, pergunta com quem o outro banco acha que está falando e desliga.",
        parametros: &[obr(
            "dblink",
            "string",
            "o nome da ligação; `nome` também é aceito",
        )],
        exemplo: r#"{"op":"dblink_testar","dblink":"erp"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "dblink_bancos",
        apelidos: &[],
        resumo: "Lista os bancos do servidor do outro lado da ligação.",
        parametros: &[obr("dblink", "string", "o nome da ligação")],
        exemplo: r#"{"op":"dblink_bancos","dblink":"erp"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "dblink_tabelas",
        apelidos: &[],
        resumo: "Lista as tabelas de um banco do outro lado.",
        parametros: &[
            obr("dblink", "string", "o nome da ligação"),
            opc("database", "string", "o banco lá; sem ele, o da ligação"),
        ],
        exemplo: r#"{"op":"dblink_tabelas","dblink":"erp","database":"producao"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "dblink_estrutura",
        apelidos: &[],
        resumo: "Descreve uma tabela do outro lado: colunas, tipos e chaves.",
        parametros: &[
            obr("dblink", "string", "o nome da ligação"),
            obr("tabela", "string", "a tabela lá"),
            opc("database", "string", "o banco lá; sem ele, o da ligação"),
        ],
        exemplo: r#"{"op":"dblink_estrutura","dblink":"erp","tabela":"pedidos"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "dblink_ler",
        apelidos: &[],
        resumo: "Lê linhas de uma tabela do outro lado, com ordem e paginação.",
        parametros: &[
            obr("dblink", "string", "o nome da ligação"),
            obr("tabela", "string", "a tabela lá"),
            opc("database", "string", "o banco lá"),
            opc("limite", "integer", "quantas linhas trazer"),
            opc("salto", "integer", "quantas saltar"),
            opc("ordem", "string", "a coluna de ordenação"),
            opc("descendente", "boolean", "ordem decrescente"),
        ],
        exemplo: r#"{"op":"dblink_ler","dblink":"erp","tabela":"pedidos","limite":100}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "dblink_consultar",
        apelidos: &[],
        resumo: "Manda um SELECT para o outro banco e devolve as linhas.",
        parametros: &[
            obr("dblink", "string", "o nome da ligação"),
            obr("sql", "string", "o comando; só leitura"),
            opc("limite", "integer", "teto de linhas"),
        ],
        exemplo: r#"{"op":"dblink_consultar","dblink":"erp","sql":"SELECT 1","limite":10}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "dblink_ligar",
        apelidos: &[],
        resumo: "Liga tabelas primas: cria a tabela local espelhando a remota e \
                 registra a sincronia na ligação (é o passo 4 do assistente).",
        parametros: &[
            obr("dblink", "string", "o nome da ligação"),
            obr(
                "tabelas",
                "array",
                "objetos {remota, local_database, local_tabela?, sentido?, dono?}; \
                 sentido: puxar|empurrar|dois (padrão dois); dono: aqui|la (padrão aqui)",
            ),
        ],
        exemplo: r#"{"op":"dblink_ligar","dblink":"erp","tabelas":[{"remota":"clientes","local_database":"loja","sentido":"dois","dono":"aqui"}]}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "dblink_sincronizar",
        apelidos: &[],
        resumo: "Uma rodada de convergência das tabelas ligadas: puxa o que falta \
                 aqui, empurra o que falta lá, e o dono vence o conflito. Exclusão \
                 não viaja. É a operação que o job agenda.",
        parametros: &[
            obr("dblink", "string", "o nome da ligação"),
            opc("tabela", "string", "sincroniza só esta (nome remoto ou local)"),
        ],
        exemplo: r#"{"op":"dblink_sincronizar","dblink":"erp"}"#,
        ferramenta_mcp: false,
    },
];

#[cfg(test)]
mod testes {
    use super::*;

    /// O TEXTO do `servidor.rs`, para derivar dele a lista de operações.
    ///
    /// Rust não deixa perguntar a um `match` quais braços ele tem, e a
    /// alternativa -- escrever a lista à mão num segundo lugar -- é exatamente
    /// a duplicação que este módulo existe para acabar. Ler o fonte é feio e é
    /// honesto: se o `despachar` ganhar uma operação, o teste falha aqui, e
    /// não seis meses depois quando alguém procurar a descrição que não existe.
    const FONTE: &str = include_str!("servidor.rs");

    /// Extrai os nomes de operação dos braços de um trecho de `match`.
    fn nomes_dos_bracos(trecho: &str) -> Vec<String> {
        let mut saida = Vec::new();
        for linha in trecho.lines() {
            let t = linha.trim();
            if t.starts_with("//") {
                continue;
            }
            let Some(padrao) = t.split("=>").next().filter(|_| t.contains("=>")) else {
                continue;
            };
            let mut resto = padrao;
            while let Some(i) = resto.find('"') {
                let depois = &resto[i + 1..];
                let Some(j) = depois.find('"') else { break };
                saida.push(depois[..j].to_string());
                resto = &depois[j + 1..];
            }
        }
        saida
    }

    /// A lista de operações derivada do `despachar`: o `match` do `executar`,
    /// mais as três que o `despachar` atende antes dele.
    fn ops_do_despachar() -> Vec<String> {
        let i = FONTE
            .find("fn executar(&self, op: &str, p: &Json, sessao: &Sessao) -> Result<Json> {")
            .expect("o `executar` mudou de assinatura: conserte este teste antes do resto");
        let fim = FONTE[i..]
            .find("outro => Err(PhxError::NaoEncontrado(")
            .expect("o braço final do `executar` mudou de forma")
            + i;
        let mut nomes = nomes_dos_bracos(&FONTE[i..fim]);

        // As que o `despachar` atende ANTES de chegar ao `executar`, e que
        // por isso não estão no `match`.
        let d = FONTE
            .find("fn despachar(")
            .expect("o `despachar` sumiu do servidor");
        let dfim = FONTE[d..]
            .find("fn portoes_do_pedido(")
            .expect("o `portoes_do_pedido` mudou de lugar")
            + d;
        let trecho = &FONTE[d..dfim];
        for pedaco in trecho.split("op == \"").skip(1) {
            if let Some(j) = pedaco.find('"') {
                nomes.push(pedaco[..j].to_string());
            }
        }
        nomes.sort();
        nomes.dedup();
        nomes
    }

    /// **O teste que faz o catálogo valer alguma coisa.** Operação nova no
    /// `despachar` sem entrada aqui é operação que nasce sem descrição -- e
    /// entrada aqui sem operação lá é uma promessa que o servidor não cumpre.
    #[test]
    fn o_catalogo_e_o_despachar_sao_a_mesma_lista() {
        let do_codigo = ops_do_despachar();
        assert!(
            do_codigo.len() > 50,
            "o leitor do fonte achou só {} operações: ele quebrou, e um teste \
             que não vê nada passa por engano",
            do_codigo.len()
        );

        let mut do_catalogo: Vec<String> = OPERACOES
            .iter()
            .flat_map(|o| o.nomes().map(str::to_string))
            .collect();
        do_catalogo.sort();

        let faltando: Vec<&String> = do_codigo
            .iter()
            .filter(|n| !do_catalogo.contains(n))
            .collect();
        assert!(
            faltando.is_empty(),
            "estas operações existem no despachar e NÃO estão no catálogo: {faltando:?}"
        );

        let sobrando: Vec<&String> = do_catalogo
            .iter()
            .filter(|n| !do_codigo.contains(n))
            .collect();
        assert!(
            sobrando.is_empty(),
            "o catálogo promete operações que o despachar não atende: {sobrando:?}"
        );
    }

    #[test]
    fn nenhum_nome_se_repete_entre_operacoes() {
        let mut vistos: Vec<&str> = Vec::new();
        for o in OPERACOES {
            for n in o.nomes() {
                assert!(!vistos.contains(&n), "o nome {n:?} aparece duas vezes");
                vistos.push(n);
            }
        }
    }

    /// Exemplo é texto escrito à mão, e texto escrito à mão envelhece calado.
    /// Este teste é o gerador que falta: ele confere que todo exemplo ainda é
    /// JSON, ainda é da operação certa, e ainda traz todo campo obrigatório.
    #[test]
    fn todo_exemplo_e_um_pedido_valido_da_propria_operacao() {
        for o in OPERACOES {
            let j = Json::analisar(o.exemplo)
                .unwrap_or_else(|e| panic!("o exemplo de {} nao e JSON: {e}", o.nome));
            assert_eq!(
                j.texto_ou("op", ""),
                o.nome,
                "o exemplo de {} chama outra operacao",
                o.nome
            );
            for p in o.parametros.iter().filter(|p| p.obrigatorio) {
                assert!(
                    j.campo(p.nome).is_some(),
                    "o exemplo de {} nao traz o obrigatorio {:?}",
                    o.nome,
                    p.nome
                );
            }
        }
    }

    #[test]
    fn toda_operacao_tem_resumo_e_parametro_descrito() {
        for o in OPERACOES {
            assert!(!o.resumo.is_empty(), "{} sem resumo", o.nome);
            assert!(
                o.resumo.ends_with('.'),
                "o resumo de {} nao e uma frase",
                o.nome
            );
            for p in o.parametros {
                assert!(
                    !p.para_que.is_empty(),
                    "{}.{} sem descricao",
                    o.nome,
                    p.nome
                );
                assert!(
                    ["string", "integer", "boolean", "array", "object"].contains(&p.tipo),
                    "{}.{} tem tipo {:?}, que nao e de JSON",
                    o.nome,
                    p.nome,
                    p.tipo
                );
            }
        }
    }

    /// O `aplicar` GRAVA e não está no `OPS_ESCRITA` -- a ausência é
    /// deliberada, porque uma réplica em modo somente-leitura precisa aplicar.
    /// Enquanto isso for verdade, ele não pode virar ferramenta MCP: a ponte
    /// somente-leitura o ofereceria achando que ele só lê.
    #[test]
    fn o_aplicar_nao_e_ferramenta_mcp() {
        let a = por_nome("aplicar").unwrap();
        assert!(!a.ferramenta_mcp);
        assert!(!a.escreve(), "o OPS_ESCRITA mudou: reveja este teste");
    }
}
