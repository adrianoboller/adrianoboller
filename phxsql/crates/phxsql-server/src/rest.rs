//! O webservice REST e a especificacao OpenAPI que sai do proprio codigo.
//!
//! # O que este modulo NAO faz
//!
//! Ele **nao executa nada**. Nao ha uma segunda tabela de operacoes, nao ha um
//! segundo lugar que confira permissao, e nao ha caminho de dado que nao passe
//! pelo `despachar`. Este modulo so traduz HTTP em pedido JSON e devolve a
//! resposta -- exatamente o papel que o `/api` da interface web ja tinha.
//!
//! Isso e decisao, e nao economia de codigo. A regra da casa e que o portao de
//! permissao e UM so; um caminho REST que despachasse por fora seria a porta
//! dos fundos maior que este projeto ja teve, e ninguem a acharia por leitura.
//!
//! # A especificacao SAI DA TABELA DE DESPACHO
//!
//! Sao mais de cem operacoes. Uma especificacao OpenAPI digitada a mao
//! envelhece na primeira operacao nova e **passa a mentir com aparencia de
//! documento oficial** -- que e pior do que nao ter documento. Entao ela e
//! gerada de [`crate::catalogo::OPERACOES`], que por sua vez ja e travado
//! contra o `match` do `despachar` por
//! `o_catalogo_e_o_despachar_sao_a_mesma_lista`.
//!
//! E ha as **duas guardas**, uma para cada lado do laco (nos testes deste
//! modulo, e a mesma forma do `conferidor.rs`):
//!
//! * operacao que o `despachar` atende e a especificacao nao documenta;
//! * rota que a especificacao documenta e o `despachar` nao atende.
//!
//! Sem as duas, a especificacao vira chave morta: alguem le, acredita, e nada
//! corresponde.
//!
//! # As tres coisas que a porta REST faz antes de despachar
//!
//! 1. **O caminho vira a operacao.** `POST /v1/ler` e `{"op":"ler"}`. Um corpo
//!    que traga um `"op"` DIFERENTE do caminho e recusado, e nao ignorado: o
//!    caminho e o que o operador ve no log do proxy e nas regras do firewall,
//!    e deixar o corpo mandar faria as duas coisas discordarem em silencio.
//! 2. **O `Bearer` vira o `token` do pedido**, e o portao 1 do `despachar` o
//!    confere como sempre. Ver [`Rest::token`](crate::config::Rest::token)
//!    para o caso do segredo proprio.
//! 3. **O estreitamento** de [`estreitar`]: banco e tabelas do `config.json`.
//!    Ele SO ESTREITA -- ver o comentario da funcao.

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

use crate::catalogo::{Operacao, OPERACOES};
use crate::config::Rest;

/// O prefixo de todas as rotas de operacao. Versionado desde o primeiro dia:
/// caminho sem versao e caminho que nao pode mudar nunca mais.
pub const PREFIXO: &str = "/v1";

/// A versao da OpenAPI que este documento declara.
pub const VERSAO_OPENAPI: &str = "3.1.0";

// =====================================================================
// A rota
// =====================================================================

/// A operacao pedida por um caminho `/v1/<op>`, ou `None`.
///
/// Aceita o nome canonico e os apelidos, porque quem le a documentacao de um
/// e o `MANUAL.txt` do outro tem de chegar no mesmo lugar. Devolve a
/// [`Operacao`] do catalogo, e nao o texto do caminho: assim o nome que segue
/// para o `despachar` e um nome que o catalogo conhece, e nao o que veio na
/// URL.
pub fn operacao_do_caminho(caminho: &str) -> Option<&'static Operacao> {
    let resto = caminho.strip_prefix(PREFIXO)?.strip_prefix('/')?;
    if resto.is_empty() || resto.contains('/') {
        return None;
    }
    crate::catalogo::por_nome(resto)
}

/// O caminho que a especificacao documenta para uma operacao.
pub fn caminho_da_operacao(o: &Operacao) -> String {
    format!("/{}", o.nome)
}

// =====================================================================
// O corpo do pedido
// =====================================================================

/// Monta o pedido JSON que vai para o `despachar`.
///
/// Corpo vazio vale como `{}` -- um `POST /v1/ping` sem corpo e o pedido mais
/// natural que existe, e exigir `{}` seria burocracia.
pub fn pedido_do_corpo(op: &str, corpo: &str) -> Result<Json> {
    let bruto = corpo.trim();
    let mut pedido = if bruto.is_empty() {
        Json::objeto(Vec::new())
    } else {
        Json::analisar(bruto)?
    };
    if !matches!(pedido, Json::Objeto(_)) {
        return Err(PhxError::Esquema(
            "o corpo do pedido REST tem de ser um objeto JSON".into(),
        ));
    }
    // O caminho manda, e o corpo discordante e RECUSADO em vez de ignorado.
    // Ignorar deixaria `POST /v1/ping` com `{"op":"excluir"}` no corpo passar
    // por um `ping` no log e no firewall, e ser outra coisa no servidor.
    match pedido.campo("op").and_then(Json::texto) {
        Some(outro) if outro != op => {
            return Err(PhxError::Esquema(format!(
                "o caminho pede a operacao {op:?} e o corpo traz \"op\":{outro:?}; \
                 no REST quem manda e o caminho -- tire o campo do corpo"
            )));
        }
        _ => {}
    }
    pedido.definir("op", Json::texto_de(op));
    Ok(pedido)
}

// =====================================================================
// O estreitamento: banco e tabelas
// =====================================================================

/// As chaves de um pedido que carregam NOME DE TABELA.
///
/// Elas estao aqui, e nao numa lista por operacao, porque a lista por operacao
/// e exatamente o que envelhece: a casa ja pagou quatro furos quando o portao
/// passou a olhar o campo `"tabela"` e havia operacao que nao o tinha
/// (`juntar` guarda em `a.tabela`/`b.tabela`, `unir` guarda numa lista). A
/// varredura aqui e ESTRUTURAL -- desce a arvore inteira do pedido e olha o
/// NOME da chave --, entao ela pega os tres casos sem saber que eles existem.
///
/// `destino` fica de FORA de proposito: a mesma chave e o nome de uma tabela
/// no `duplicar_tabela` e uma PASTA no `backup`, e barrar uma pasta por nao
/// estar na lista de tabelas seria recusar por um motivo falso. Quem cria
/// tabela nova continua precisando do direito de criar, que e o portao.
pub const CHAVES_DE_TABELA: &[&str] = &["tabela", "tabelas", "tabela_ref"];

/// Todo nome de tabela que este pedido cita, venha de onde vier.
pub fn tabelas_citadas(pedido: &Json) -> Vec<String> {
    let mut achados = Vec::new();
    juntar_tabelas(pedido, &mut achados);
    achados
}

fn juntar_tabelas(no: &Json, saida: &mut Vec<String>) {
    match no {
        Json::Objeto(pares) => {
            for (chave, valor) in pares {
                if CHAVES_DE_TABELA.contains(&chave.as_str()) {
                    nomes_de(valor, saida);
                }
                juntar_tabelas(valor, saida);
            }
        }
        Json::Lista(itens) => itens.iter().for_each(|i| juntar_tabelas(i, saida)),
        _ => {}
    }
}

/// O nome (ou os nomes) que um valor de chave de tabela carrega.
///
/// Texto e o caso comum; lista de textos e o `unir`; lista de objetos e o
/// `dblink_ligar`, e ali o nome mora em `remota`/`local_tabela`, que a
/// recursao pega sozinha.
fn nomes_de(valor: &Json, saida: &mut Vec<String>) {
    match valor {
        Json::Texto(t) if !t.trim().is_empty() => saida.push(t.trim().to_string()),
        Json::Lista(itens) => itens.iter().for_each(|i| nomes_de(i, saida)),
        _ => {}
    }
}

/// Aplica o estreitamento do `config.json` ao pedido, ANTES do portao.
///
/// # A frase que decide se isto ajuda ou estraga
///
/// **A lista de tabelas e um filtro que so ESTREITA, aplicado antes do portao
/// e nunca no lugar dele.** Tabela fora da lista nao existe para o REST --
/// responde como inexistente, sem contar que existe. Tabela dentro da lista
/// continua passando pelo `despachar` e pelo direito do usuario exatamente
/// como hoje. **Nunca alarga:** se o usuario nao tem direito, estar na lista
/// nao da.
///
/// Se ela decidisse acesso sozinha, passariam a existir duas verdades sobre
/// direito -- e no primeiro desacordo entre elas alguem ganha acesso que
/// ninguem concedeu.
///
/// # Por que a recusa e "nao existe", e nao "nao pode"
///
/// Porque "nao pode" conta que existe. Quem publica tres tabelas de um banco
/// de trinta nao quer que a porta REST responda o nome das outras vinte e
/// sete.
pub fn estreitar(rest: &Rest, pedido: &mut Json) -> Result<()> {
    // O banco: preenche o que veio vazio, recusa o que veio diferente.
    if !rest.database.is_empty() {
        match pedido.campo("database").and_then(Json::texto) {
            Some(pedido_db) if !pedido_db.trim().is_empty() => {
                if !rest.database_exposto(pedido_db) {
                    return Err(nao_existe("o banco", pedido_db));
                }
            }
            _ => {
                // So preenche onde a operacao TEM esse campo, e quem diz isso
                // e o catalogo -- nao uma lista escrita aqui. Preencher
                // `database` num `sistema` ou num `bancos` mudaria um pedido
                // que nunca falou de banco nenhum.
                let op = pedido.texto_ou("op", "").to_string();
                if crate::catalogo::por_nome(&op)
                    .is_some_and(|o| o.parametros.iter().any(|p| p.nome == "database"))
                {
                    pedido.definir("database", Json::texto_de(&rest.database));
                }
            }
        }
    }
    // As tabelas: toda citacao do pedido, venha do campo que vier.
    for nome in tabelas_citadas(pedido) {
        if !rest.tabela_exposta(&nome) {
            return Err(nao_existe("a tabela", &nome));
        }
    }
    Ok(())
}

/// A recusa que nao conta o que existe do outro lado.
fn nao_existe(que: &str, nome: &str) -> PhxError {
    PhxError::NaoEncontrado(format!("{que} {nome:?} nao existe neste webservice"))
}

// =====================================================================
// O codigo HTTP
// =====================================================================

/// O codigo HTTP de um erro do PhxSql.
///
/// Derivado do codigo NUMERICO do erro, e nao de uma lista por variante: erro
/// novo cai na faixa certa sozinho, e as duas nao tem como divergir. E a mesma
/// razao pela qual `PhxError::classe` deriva da faixa em vez de repetir a
/// lista.
pub fn status_do_erro(e: &PhxError) -> u16 {
    match e {
        // O redirecionamento e o unico que tem resposta HTTP consagrada: o
        // cliente tem de ir a outro servidor, e 421 e exatamente isso.
        PhxError::Redireciona(_) => 421,
        PhxError::Autorizacao(_) => 403,
        PhxError::EmCarga(_) | PhxError::SpareEmEspera(_) => 503,
        PhxError::NaoEncontrado(_) => 404,
        PhxError::Duplicado(_) | PhxError::Conflito(_) => 409,
        PhxError::Cancelado(_) => 499,
        _ => match e.codigo() / 1000 {
            2 | 3 => 400,
            _ => 500,
        },
    }
}

// =====================================================================
// A especificacao
// =====================================================================

/// O tipo JSON Schema de um parametro do catalogo.
fn tipo_json(tipo: &str) -> &'static str {
    match tipo {
        "integer" => "integer",
        "boolean" => "boolean",
        "array" => "array",
        "object" => "object",
        _ => "string",
    }
}

/// O corpo de um pedido, como JSON Schema.
fn esquema_do_pedido(o: &Operacao) -> Json {
    let propriedades: Vec<(String, Json)> = o
        .parametros
        .iter()
        .map(|p| {
            let mut campos = vec![
                ("type", Json::texto_de(tipo_json(p.tipo))),
                ("description", Json::texto_de(p.para_que)),
            ];
            if p.tipo == "array" {
                campos.push(("items", Json::objeto(Vec::new())));
            }
            (p.nome.to_string(), Json::objeto(campos))
        })
        .collect();
    let obrigatorios: Vec<Json> = o
        .parametros
        .iter()
        .filter(|p| p.obrigatorio)
        .map(|p| Json::texto_de(p.nome))
        .collect();
    let mut esquema = Json::objeto(vec![
        ("type", Json::texto_de("object")),
        ("properties", Json::Objeto(propriedades)),
        // `false` de proposito: campo a mais no corpo e quase sempre nome
        // escrito errado, e o servidor o ignoraria calado. A especificacao ao
        // menos avisa quem valida contra ela.
        ("additionalProperties", Json::Bool(false)),
    ]);
    if !obrigatorios.is_empty() {
        esquema.definir("required", Json::Lista(obrigatorios));
    }
    esquema
}

/// O exemplo do catalogo SEM o `"op"`, que no REST mora no caminho.
fn exemplo_do_corpo(o: &Operacao) -> Json {
    match Json::analisar(o.exemplo) {
        Ok(Json::Objeto(pares)) => {
            Json::Objeto(pares.into_iter().filter(|(k, _)| k != "op").collect())
        }
        _ => Json::objeto(Vec::new()),
    }
}

/// Uma resposta de erro, na forma que o servidor devolve.
fn resposta_de_erro(descricao: &str) -> Json {
    Json::objeto(vec![
        ("description", Json::texto_de(descricao)),
        (
            "content",
            Json::objeto(vec![(
                "application/json",
                Json::objeto(vec![(
                    "schema",
                    Json::objeto(vec![("$ref", Json::texto_de("#/components/schemas/Erro"))]),
                )]),
            )]),
        ),
    ])
}

/// A operacao, em OpenAPI.
fn operacao_openapi(o: &Operacao) -> Json {
    let permissao = match o.atividade() {
        Some(a) => Json::texto_de(a.nome()),
        None => Json::Nulo,
    };
    let etiqueta = match o.atividade() {
        Some(a) => a.nome(),
        None => "sessao",
    };
    let post = Json::objeto(vec![
        ("operationId", Json::texto_de(o.nome)),
        ("summary", Json::texto_de(o.resumo)),
        ("tags", Json::Lista(vec![Json::texto_de(etiqueta)])),
        // As tres extensoes existem porque a OpenAPI nao tem onde dizer
        // "quem chama isto precisa do direito de alterar" -- e essa e a
        // primeira pergunta de quem integra. Saem do MESMO lugar que o
        // portao le, e nao de um campo proprio que pudesse discordar dele.
        ("x-phxsql-permissao", permissao),
        ("x-phxsql-escreve", Json::Bool(o.escreve())),
        (
            "x-phxsql-apelidos",
            Json::Lista(o.apelidos.iter().map(|a| Json::texto_de(*a)).collect()),
        ),
        (
            "requestBody",
            Json::objeto(vec![
                (
                    "required",
                    Json::Bool(o.parametros.iter().any(|p| p.obrigatorio)),
                ),
                (
                    "content",
                    Json::objeto(vec![(
                        "application/json",
                        Json::objeto(vec![
                            ("schema", esquema_do_pedido(o)),
                            ("example", exemplo_do_corpo(o)),
                        ]),
                    )]),
                ),
            ]),
        ),
        (
            "responses",
            Json::objeto(vec![
                (
                    "200",
                    Json::objeto(vec![
                        ("description", Json::texto_de("a operacao respondeu")),
                        (
                            "content",
                            Json::objeto(vec![(
                                "application/json",
                                Json::objeto(vec![(
                                    "schema",
                                    Json::objeto(vec![(
                                        "$ref",
                                        Json::texto_de("#/components/schemas/Resposta"),
                                    )]),
                                )]),
                            )]),
                        ),
                    ]),
                ),
                ("400", resposta_de_erro("o pedido nao casa com a estrutura")),
                (
                    "401",
                    resposta_de_erro("sem token, ou token que esta porta nao aceita"),
                ),
                (
                    "403",
                    resposta_de_erro("quem pediu nao tem esse direito, ou o IP esta barrado"),
                ),
                (
                    "404",
                    resposta_de_erro(
                        "nao existe -- inclusive a tabela que este webservice nao expoe",
                    ),
                ),
                ("409", resposta_de_erro("duplicado ou conflito de versao")),
                (
                    "421",
                    resposta_de_erro("escreva no primario; a mensagem traz o endereco"),
                ),
                (
                    "503",
                    resposta_de_erro("tabela em carga, ou spare que ainda nao assumiu"),
                ),
            ]),
        ),
    ]);
    Json::objeto(vec![("post", post)])
}

/// Os esquemas do envelope da resposta -- os mesmos campos que o servidor
/// escreve, e nao uma descricao de como ele deveria ser.
fn componentes() -> Json {
    let resposta = Json::objeto(vec![
        ("type", Json::texto_de("object")),
        (
            "properties",
            Json::objeto(vec![
                (
                    "ok",
                    Json::objeto(vec![
                        ("type", Json::texto_de("boolean")),
                        ("const", Json::Bool(true)),
                    ]),
                ),
                (
                    "op",
                    Json::objeto(vec![
                        ("type", Json::texto_de("string")),
                        ("description", Json::texto_de("a operacao atendida")),
                    ]),
                ),
                (
                    "resultado",
                    Json::objeto(vec![(
                        "description",
                        Json::texto_de("o que a operacao devolveu; a forma e de cada uma"),
                    )]),
                ),
                (
                    "ms",
                    Json::objeto(vec![
                        ("type", Json::texto_de("integer")),
                        ("description", Json::texto_de("quanto o servidor levou")),
                    ]),
                ),
                (
                    "sessao",
                    Json::objeto(vec![
                        ("type", Json::texto_de("string")),
                        (
                            "description",
                            Json::texto_de("so no `login` e no `desafio`: repita em `X-Sessao`"),
                        ),
                    ]),
                ),
            ]),
        ),
        (
            "required",
            Json::Lista(vec![Json::texto_de("ok"), Json::texto_de("op")]),
        ),
    ]);
    let erro = Json::objeto(vec![
        ("type", Json::texto_de("object")),
        (
            "properties",
            Json::objeto(vec![
                (
                    "ok",
                    Json::objeto(vec![
                        ("type", Json::texto_de("boolean")),
                        ("const", Json::Bool(false)),
                    ]),
                ),
                ("op", Json::objeto(vec![("type", Json::texto_de("string"))])),
                (
                    "erro",
                    Json::objeto(vec![
                        ("type", Json::texto_de("string")),
                        (
                            "description",
                            Json::texto_de("o texto, que acompanha o idioma do servidor"),
                        ),
                    ]),
                ),
                (
                    "codigo",
                    Json::objeto(vec![
                        ("type", Json::texto_de("integer")),
                        (
                            "description",
                            Json::texto_de(
                                "o codigo estavel: 1000 formato, 2000 esquema, 3000 dado, \
                                 4000 acesso, 5000 sistema, 6000 execucao. Trate por ele, \
                                 nunca pelo texto",
                            ),
                        ),
                    ]),
                ),
                (
                    "nome",
                    Json::objeto(vec![("type", Json::texto_de("string"))]),
                ),
                (
                    "classe",
                    Json::objeto(vec![("type", Json::texto_de("string"))]),
                ),
                (
                    "sprint",
                    Json::objeto(vec![
                        ("type", Json::texto_de("string")),
                        (
                            "description",
                            Json::texto_de(
                                "a sprint do roteiro que responde por esta recusa \
                                 (`SP000008`). Vem tambem no comeco do texto, entre \
                                 colchetes; aqui esta o campo, para ninguem precisar \
                                 recorta-la de volta da frase",
                            ),
                        ),
                    ]),
                ),
                (
                    "repetir",
                    Json::objeto(vec![
                        ("type", Json::texto_de("boolean")),
                        (
                            "description",
                            Json::texto_de("adianta tentar de novo? so o de E/S e o em carga"),
                        ),
                    ]),
                ),
            ]),
        ),
        (
            "required",
            Json::Lista(vec![
                Json::texto_de("ok"),
                Json::texto_de("erro"),
                Json::texto_de("codigo"),
            ]),
        ),
    ]);
    Json::objeto(vec![
        (
            "securitySchemes",
            Json::objeto(vec![
                (
                    "token",
                    Json::objeto(vec![
                        ("type", Json::texto_de("http")),
                        ("scheme", Json::texto_de("bearer")),
                        (
                            "description",
                            Json::texto_de(
                                "O token do servidor, no cabecalho `Authorization: Bearer`. \
                                 E a chave da PORTA, nao a identidade. ATENCAO: em HTTP em \
                                 claro ele viaja em texto puro em todo pedido, e quem escuta \
                                 o fio o tem -- ver a secao 7 do docs/SEGURANCA.md. Nao ha \
                                 TLS aqui: a saida honesta e um proxy que termine TLS na \
                                 frente, ou um tunel.",
                            ),
                        ),
                    ]),
                ),
                (
                    "sessao",
                    Json::objeto(vec![
                        ("type", Json::texto_de("apiKey")),
                        ("in", Json::texto_de("header")),
                        ("name", Json::texto_de("X-Sessao")),
                        (
                            "description",
                            Json::texto_de(
                                "A identidade. Sai do `login` (que vem depois do `desafio`, \
                                 e ai a senha nao viaja) e vale enquanto a sessao viver. \
                                 Sem ela o pedido entra pelo token de servico, que num \
                                 servidor com cadastro nao chama operacao nenhuma que peca \
                                 direito.",
                            ),
                        ),
                    ]),
                ),
            ]),
        ),
        (
            "schemas",
            Json::objeto(vec![("Resposta", resposta), ("Erro", erro)]),
        ),
    ])
}

/// A descricao do documento -- o que quem abre a especificacao precisa saber
/// antes de mandar o primeiro pedido.
fn descricao(rest: &Rest) -> String {
    let mut d = String::from(
        "Webservice REST do PhxSql. Cada operacao do protocolo e um \
         `POST` em `/v1/<operacao>`, com o corpo em JSON.\n\n\
         **Este documento e GERADO da tabela de despacho do servidor**, e nao \
         escrito a mao: operacao que existe e nao aparece aqui reprova um teste, \
         e rota descrita aqui que o servidor nao atende reprova outro.\n\n\
         **Autenticacao:** o `Bearer` abre a porta; o `login` diz quem e voce. \
         Em HTTP em claro o token viaja em texto puro -- ponha um proxy com TLS \
         na frente, ou um tunel.\n\n",
    );
    if !rest.database.is_empty() {
        d.push_str(&format!(
            "**Este webservice atende o banco `{}`.** Pedido sem `database` \
             recebe esse; pedido que nomeie outro responde como inexistente.\n\n",
            rest.database
        ));
    }
    if !rest.tabelas.is_empty() {
        d.push_str(&format!(
            "**Tabelas expostas:** {}. As outras nao existem para esta porta. \
             A lista SO ESTREITA: estar nela nao da direito nenhum, e quem nao \
             pode continua nao podendo.\n\n",
            rest.tabelas.join(", ")
        ));
    }
    d
}

/// A especificacao inteira.
pub fn openapi(rest: &Rest, versao: &str) -> Json {
    let caminhos: Vec<(String, Json)> = OPERACOES
        .iter()
        .map(|o| (caminho_da_operacao(o), operacao_openapi(o)))
        .collect();
    Json::objeto(vec![
        ("openapi", Json::texto_de(VERSAO_OPENAPI)),
        (
            "info",
            Json::objeto(vec![
                ("title", Json::texto_de(rest.titulo())),
                ("version", Json::texto_de(versao)),
                ("description", Json::texto_de(descricao(rest))),
            ]),
        ),
        (
            "servers",
            Json::Lista(vec![Json::objeto(vec![
                ("url", Json::texto_de(PREFIXO)),
                (
                    "description",
                    Json::texto_de("relativo a esta porta -- ver rest.bind no config.json"),
                ),
            ])]),
        ),
        (
            "security",
            Json::Lista(vec![Json::objeto(vec![("token", Json::Lista(Vec::new()))])]),
        ),
        ("components", componentes()),
        ("paths", Json::Objeto(caminhos)),
    ])
}

// =====================================================================
// O explorador: o "Swagger" desta casa
// =====================================================================

/// A pagina que mostra a especificacao, servida na porta do explorador.
///
/// # A conta que decidiu escrever um em vez de embutir o Swagger UI
///
/// Medido nesta maquina, baixando o `swagger-ui-dist` 5.17.14 de verdade e
/// compilando com ele dentro do binario:
///
/// ```text
/// hoje, sem nada .................  7.296.144 bytes
/// + swagger-ui.css + bundle ......  8.900.968     +1.604.824   +22,0%
/// + o preset standalone junto ....  9.131.896     +1.835.752   +25,2%
/// ```
///
/// O crescimento e byte a byte o dos arquivos -- nao ha compressao no meio, e
/// a medicao confirmou isso (os 635 bytes de diferenca sao o codigo que os
/// serviria). A terceira saida, apontar para uma CDN, custaria zero byte e
/// **quebraria o uso offline**, que e justamente o caso do IoT: a placa sobe
/// sem internet e o visualizador viria em branco.
///
/// Este explorador custa o tamanho dos tres arquivos de `ui/explorador.*` --
/// o numero medido esta no `docs/REST.md`.
///
/// # Ele nao e obrigatorio
///
/// A porta dele e outra (7000 de fabrica) e tem interruptor proprio: quem sobe
/// numa placa liga `rest.ligado` e deixa `rest.swagger_ligado` desligado. Foi
/// a medicao acima que dispensou uma opcao de COMPILACAO: a 1,53 MiB ela seria
/// obrigatoria; a dezenas de KiB, um `--no-default-features` a mais para todo
/// mundo manter nao se paga.
pub mod explorador {
    use crate::config::Rest;

    /// A moldura. Sem uma palavra de texto de tela -- ver o comentario dentro.
    const MOLDURA: &str = include_str!("../ui/explorador.html");
    /// O comportamento, em arquivo de interface e nao no meio do Rust.
    ///
    /// Mesma razao do `diagrama-er.js` e do `telemetria.js`: desenho e
    /// ALGORITMO, e algoritmo nao mora no meio do servidor. E, morando em
    /// `ui/`, ele entra no `conferidor::FONTES` e passa a contar para a
    /// catraca dos idiomas como qualquer outra tela.
    const COMPORTAMENTO: &str = include_str!("../ui/explorador.js");
    /// O estilo, em folha propria pelo mesmo motivo do `telemetria.css`.
    const ESTILO: &str = include_str!("../ui/explorador.css");

    /// A pagina inteira, montada.
    pub fn pagina(rest: &Rest) -> String {
        MOLDURA
            .replace("{titulo}", &escapar(&rest.titulo()))
            .replace("/*ESTILO*/", ESTILO)
            .replace("// COMPORTAMENTO", COMPORTAMENTO)
    }

    /// O nome do servico vem do `config.json`, entao ele e DADO de fora --
    /// e dado de fora nunca entra cru numa pagina.
    fn escapar(bruto: &str) -> String {
        bruto
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O TEXTO do `servidor.rs`, para derivar dele a lista de operacoes.
    ///
    /// Ler o fonte e feio e e honesto, e a razao e a mesma que o
    /// `catalogo.rs` ja escreveu: Rust nao deixa perguntar a um `match` quais
    /// bracos ele tem, e a alternativa -- a lista escrita a mao num segundo
    /// lugar -- e exatamente a duplicacao que esta frente existe para nao ter.
    const FONTE: &str = include_str!("servidor.rs");

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

    /// A lista de operacoes que o `despachar` REALMENTE atende.
    fn ops_do_despachar() -> Vec<String> {
        let i = FONTE
            .find("fn executar(&self, op: &str, p: &Json, sessao: &Sessao) -> Result<Json> {")
            .expect("o `executar` mudou de assinatura: conserte este teste antes do resto");
        let fim = FONTE[i..]
            .find("outro => Err(PhxError::NaoEncontrado(")
            .expect("o braco final do `executar` mudou de forma")
            + i;
        let mut nomes = nomes_dos_bracos(&FONTE[i..fim]);
        let d = FONTE
            .find("fn despachar(")
            .expect("o `despachar` sumiu do servidor");
        let dfim = FONTE[d..]
            .find("fn portoes_do_pedido(")
            .expect("o `portoes_do_pedido` mudou de lugar")
            + d;
        for pedaco in FONTE[d..dfim].split("op == \"").skip(1) {
            if let Some(j) = pedaco.find('"') {
                nomes.push(pedaco[..j].to_string());
            }
        }
        nomes.sort();
        nomes.dedup();
        nomes
    }

    fn spec() -> Json {
        openapi(&Rest::default(), "0.0.0")
    }

    fn rotas_da_spec(s: &Json) -> Vec<String> {
        s.campo("paths")
            .map(|p| p.chaves().into_iter().map(str::to_string).collect())
            .unwrap_or_default()
    }

    /// **Guarda 1 dos dois lados do laco:** operacao que existe e a
    /// especificacao nao documenta.
    ///
    /// Sem ela a especificacao vira promessa parcial: quem le acredita que o
    /// que nao esta ali nao existe, e integra sem a metade que faltou.
    ///
    /// **Prova real, com o defeito reposto:** tire uma entrada do
    /// `OPERACOES` (ou acrescente um braco ao `executar` sem a entrada) e este
    /// teste reprova nomeando a operacao.
    #[test]
    fn toda_operacao_do_despachar_esta_na_especificacao() {
        let s = spec();
        let rotas = rotas_da_spec(&s);
        let ops = ops_do_despachar();
        assert!(
            ops.len() > 50,
            "o leitor do fonte achou so {} operacoes: ele quebrou, e um teste \
             que nao ve nada passa por engano",
            ops.len()
        );
        let faltando: Vec<&String> = ops
            .iter()
            .filter(|n| {
                // O apelido e documentado na rota do nome canonico, e nao numa
                // rota propria: duas rotas para a mesma operacao seriam duas
                // paginas dizendo a mesma coisa, e uma delas envelheceria.
                match crate::catalogo::por_nome(n) {
                    Some(o) => !rotas.contains(&caminho_da_operacao(o)),
                    None => true,
                }
            })
            .collect();
        assert!(
            faltando.is_empty(),
            "o despachar atende estas operacoes e a especificacao nao as \
             documenta: {faltando:?}"
        );
    }

    /// **Guarda 2 dos dois lados do laco:** rota documentada que nao existe.
    ///
    /// E a pior das duas, e por isso ela existe: uma especificacao que promete
    /// o que o servidor nao faz e uma mentira com aparencia de documento
    /// oficial. Quem integra descobre em producao.
    ///
    /// **Prova real, com o defeito reposto:** acrescente ao `paths` uma rota
    /// que nao seja de operacao nenhuma e este teste reprova.
    #[test]
    fn toda_rota_da_especificacao_existe_no_despachar() {
        let s = spec();
        let ops = ops_do_despachar();
        let rotas = rotas_da_spec(&s);
        assert!(
            rotas.len() > 50,
            "so {} rotas na especificacao: o gerador quebrou",
            rotas.len()
        );
        for rota in &rotas {
            let nome = rota.trim_start_matches('/');
            assert!(
                ops.iter().any(|o| o == nome),
                "a especificacao documenta a rota {rota:?} e o despachar nao \
                 atende essa operacao"
            );
            assert!(
                operacao_do_caminho(&format!("{PREFIXO}{rota}")).is_some(),
                "a rota {rota:?} esta na especificacao e o roteador nao a \
                 resolve: quem seguisse o documento levaria 404"
            );
        }
    }

    /// O apelido tambem chega. Quem leu `systables` no MANUAL nao pode levar
    /// 404 porque a especificacao chama a rota de `sistabelas`.
    #[test]
    fn o_apelido_resolve_para_a_mesma_operacao() {
        let a = operacao_do_caminho("/v1/sistabelas").expect("sistabelas");
        let b = operacao_do_caminho("/v1/systables").expect("systables");
        assert_eq!(a.nome, b.nome);
    }

    #[test]
    fn caminho_fora_do_prefixo_ou_com_barra_nao_e_rota() {
        assert!(operacao_do_caminho("/ping").is_none());
        assert!(operacao_do_caminho("/v1/").is_none());
        assert!(operacao_do_caminho("/v1/ler/1").is_none());
        assert!(operacao_do_caminho("/v1/nao_existe").is_none());
    }

    /// O caminho manda. Um corpo com outra `op` e RECUSADO, e nao ignorado.
    ///
    /// **Prova real:** troque a recusa por `pedido.definir("op", ...)` calado
    /// e este teste reprova -- e o defeito que ele repoe e o que faria um
    /// `POST /v1/ping` virar um `excluir` no servidor e continuar um `ping`
    /// no log do proxy.
    #[test]
    fn corpo_com_outra_operacao_e_recusado() {
        let e = pedido_do_corpo("ping", r#"{"op":"excluir"}"#).unwrap_err();
        assert!(e.to_string().contains("caminho"), "{e}");
        // O mesmo nome no corpo nao atrapalha ninguem.
        let ok = pedido_do_corpo("ping", r#"{"op":"ping"}"#).unwrap();
        assert_eq!(ok.texto_ou("op", ""), "ping");
    }

    #[test]
    fn corpo_vazio_vale_como_objeto_vazio() {
        let p = pedido_do_corpo("ping", "").unwrap();
        assert_eq!(p.texto_ou("op", ""), "ping");
    }

    #[test]
    fn corpo_que_nao_e_objeto_e_recusado() {
        assert!(pedido_do_corpo("ping", "[1,2]").is_err());
        assert!(pedido_do_corpo("ping", "\"oi\"").is_err());
    }

    // --------------------------------------------------- o estreitamento

    fn com_tabelas(tabelas: &[&str]) -> Rest {
        Rest {
            tabelas: tabelas.iter().map(|t| t.to_string()).collect(),
            ..Rest::default()
        }
    }

    /// **O teste do comportamento VELHO, que e o que mais importa.**
    ///
    /// Secao sem lista nenhuma nao estreita nada -- byte a byte o que um
    /// `config.json` de hoje faz. Uma lista vazia que barrasse tudo seria a
    /// mesma proteção que quebra todo cliente antigo, e isso nao e protecao.
    #[test]
    fn sem_lista_de_tabelas_nada_muda() {
        let rest = Rest::default();
        let mut p =
            Json::analisar(r#"{"op":"ler","database":"loja","tabela":"clientes"}"#).unwrap();
        assert!(estreitar(&rest, &mut p).is_ok());
        assert_eq!(p.texto_ou("database", ""), "loja");
    }

    /// Estreita: tabela fora da lista NAO APARECE -- e a recusa nao conta que
    /// ela existe.
    #[test]
    fn tabela_fora_da_lista_nao_aparece() {
        let rest = com_tabelas(&["clientes"]);
        let mut p =
            Json::analisar(r#"{"op":"ler","database":"loja","tabela":"salarios"}"#).unwrap();
        let e = estreitar(&rest, &mut p).unwrap_err();
        assert_eq!(e.nome(), "NAO_ENCONTRADO", "tem de parecer inexistente");
        assert!(
            !e.to_string().contains("permiss") && !e.to_string().contains("direito"),
            "a recusa contou que a tabela existe: {e}"
        );
        // E a que esta na lista passa.
        let mut ok =
            Json::analisar(r#"{"op":"ler","database":"loja","tabela":"clientes"}"#).unwrap();
        assert!(estreitar(&rest, &mut ok).is_ok());
    }

    /// **Nao alarga.** Este e o que mais importa dos dois, e e o mesmo padrao
    /// do `sem_regra_de_tabela_nada_muda`: o filtro nao concede nada.
    ///
    /// Aqui ele e provado pela FORMA: `estreitar` nao recebe usuario, nao
    /// recebe sessao e nao tem como escrever permissao nenhuma no pedido. O
    /// pedido que sai dele e o mesmo que entrou (mais o `database` de
    /// conveniencia), e quem decide direito continua sendo o `despachar`. A
    /// outra metade -- que o portao continua recusando -- e provada pelo
    /// soquete em `bancada/rest/provar.py`, com um usuario sem direito
    /// pedindo uma tabela QUE ESTA na lista.
    #[test]
    fn tabela_na_lista_ainda_pede_direito_do_usuario() {
        let rest = com_tabelas(&["clientes"]);
        let antes = r#"{"op":"ler","database":"loja","tabela":"clientes","rowid":1}"#;
        let mut p = Json::analisar(antes).unwrap();
        estreitar(&rest, &mut p).unwrap();
        assert_eq!(
            p,
            Json::analisar(antes).unwrap(),
            "o estreitamento mexeu no pedido: ele so pode barrar, nunca conceder"
        );
        for chave in ["usuario", "login", "permissao", "token"] {
            assert!(
                p.campo(chave).is_none(),
                "o estreitamento escreveu {chave:?} no pedido -- isso seria \
                 conceder, e ele so pode estreitar"
            );
        }
    }

    /// A porta dos fundos que a lista teria se olhasse so o campo `"tabela"`:
    /// pedir a tabela escondida como o lado B de uma juncao, ou dentro da
    /// lista de uma uniao.
    ///
    /// **Prova real, com o defeito reposto:** troque a varredura estrutural
    /// por `pedido.texto_ou("tabela", "")` e os tres casos abaixo passam a
    /// entrar.
    #[test]
    fn a_lista_pega_a_tabela_escondida_na_juncao_e_na_uniao() {
        let rest = com_tabelas(&["clientes"]);
        for pedido in [
            r#"{"op":"juntar","database":"loja","a":{"tabela":"clientes"},"b":{"tabela":"salarios"}}"#,
            r#"{"op":"unir","database":"loja","tabelas":["clientes","salarios"]}"#,
            r#"{"op":"declarar_fk","database":"loja","tabela":"clientes","tabela_ref":"salarios"}"#,
        ] {
            let mut p = Json::analisar(pedido).unwrap();
            let e = estreitar(&rest, &mut p).unwrap_err();
            assert!(
                e.to_string().contains("salarios"),
                "a tabela escondida passou: {pedido}"
            );
        }
    }

    #[test]
    fn a_tabela_com_schema_casa_com_o_nome_curto() {
        let rest = com_tabelas(&["clientes"]);
        let mut p = Json::analisar(r#"{"op":"ler","tabela":"vendas.clientes"}"#).unwrap();
        assert!(estreitar(&rest, &mut p).is_ok());
    }

    #[test]
    fn o_banco_do_config_preenche_o_pedido_sem_banco() {
        let rest = Rest {
            database: "loja".into(),
            ..Rest::default()
        };
        let mut p = Json::analisar(r#"{"op":"ler","tabela":"clientes"}"#).unwrap();
        estreitar(&rest, &mut p).unwrap();
        assert_eq!(p.texto_ou("database", ""), "loja");

        // E o pedido que nomeia OUTRO banco nao existe para esta porta.
        let mut outro = Json::analisar(r#"{"op":"ler","database":"folha"}"#).unwrap();
        assert!(estreitar(&rest, &mut outro).is_err());
    }

    /// O preenchimento so acontece onde a operacao TEM o campo. Um `sistema`
    /// nunca falou de banco nenhum, e escrever um ali mudaria o pedido.
    #[test]
    fn o_banco_do_config_nao_entra_em_operacao_que_nao_o_tem() {
        let rest = Rest {
            database: "loja".into(),
            ..Rest::default()
        };
        let mut p = Json::analisar(r#"{"op":"sistema"}"#).unwrap();
        estreitar(&rest, &mut p).unwrap();
        assert!(p.campo("database").is_none());
    }

    // --------------------------------------------------- a especificacao

    #[test]
    fn a_especificacao_e_json_valido_e_tem_as_pecas_obrigatorias() {
        let s = spec();
        let texto = s.escrever();
        let relido = Json::analisar(&texto).expect("a especificacao gerada nao e JSON");
        assert_eq!(relido, s);
        assert_eq!(s.texto_ou("openapi", ""), VERSAO_OPENAPI);
        assert!(s.campo("info").is_some());
        assert!(s.campo("paths").is_some());
        let comp = s.campo("components").expect("components");
        let seg = comp.campo("securitySchemes").expect("securitySchemes");
        assert!(seg.campo("token").is_some());
        assert!(seg.campo("sessao").is_some());
    }

    /// A especificacao nao promete o que a operacao nao pede, nem esconde o
    /// que ela exige: os obrigatorios sao os do catalogo, um a um.
    #[test]
    fn os_obrigatorios_da_especificacao_sao_os_do_catalogo() {
        let s = spec();
        let caminhos = s.campo("paths").unwrap();
        for o in OPERACOES {
            let rota = caminhos.campo(&caminho_da_operacao(o)).unwrap();
            let esquema = rota
                .campo("post")
                .and_then(|p| p.campo("requestBody"))
                .and_then(|b| b.campo("content"))
                .and_then(|c| c.campo("application/json"))
                .and_then(|j| j.campo("schema"))
                .unwrap();
            let obrigatorios: Vec<String> = esquema
                .campo("required")
                .and_then(Json::lista)
                .map(|l| {
                    l.iter()
                        .filter_map(|x| x.texto().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            for p in o.parametros.iter().filter(|p| p.obrigatorio) {
                assert!(
                    obrigatorios.contains(&p.nome.to_string()),
                    "{}: {} e obrigatorio no catalogo e nao na especificacao",
                    o.nome,
                    p.nome
                );
            }
            let props = esquema.campo("properties").unwrap();
            for p in o.parametros {
                assert!(
                    props.campo(p.nome).is_some(),
                    "{}: o parametro {} nao aparece na especificacao",
                    o.nome,
                    p.nome
                );
            }
        }
    }

    /// O limite do `Bearer` em claro aparece ESCRITO na especificacao.
    ///
    /// A §7 do `docs/SEGURANCA.md` ja diz isso do resto do servidor; o REST
    /// repete em vez de esconder, porque quem le a especificacao pode nunca
    /// abrir aquele documento.
    #[test]
    fn a_especificacao_diz_que_bearer_em_claro_entrega_o_token() {
        let s = spec();
        let d = s
            .campo("components")
            .and_then(|c| c.campo("securitySchemes"))
            .and_then(|c| c.campo("token"))
            .and_then(|t| t.campo("description"))
            .and_then(Json::texto)
            .unwrap_or("")
            .to_string();
        assert!(d.contains("claro"), "o limite nao esta escrito: {d}");
        assert!(d.contains("SEGURANCA"), "falta apontar onde esta a regra");
    }

    /// A especificacao nao pode carregar segredo. O token do `config.json`
    /// nunca sai por aqui -- nem quando ha um proprio da porta REST.
    #[test]
    fn a_especificacao_nao_carrega_o_token() {
        let rest = Rest {
            token: "SEGREDO-DA-PORTA".into(),
            nome: "vendas".into(),
            ..Rest::default()
        };
        let texto = openapi(&rest, "0.0.0").escrever();
        assert!(
            !texto.contains("SEGREDO-DA-PORTA"),
            "o token vazou para a especificacao"
        );
        assert!(texto.contains("vendas"), "o nome do servico devia aparecer");
    }

    /// O estreitamento aparece na descricao: quem le tem de saber que a porta
    /// mostra tres tabelas de trinta, senao vai procurar defeito onde ha
    /// configuracao.
    #[test]
    fn a_descricao_conta_o_estreitamento() {
        let rest = Rest {
            database: "loja".into(),
            tabelas: vec!["clientes".into()],
            ..Rest::default()
        };
        let d = descricao(&rest);
        assert!(d.contains("loja"));
        assert!(d.contains("clientes"));
        assert!(d.contains("ESTREITA"), "a frase que decide tem de estar la");
    }

    #[test]
    fn o_status_http_sai_da_faixa_do_codigo() {
        assert_eq!(status_do_erro(&PhxError::Autorizacao("x".into())), 403);
        assert_eq!(status_do_erro(&PhxError::NaoEncontrado("x".into())), 404);
        assert_eq!(status_do_erro(&PhxError::Duplicado("x".into())), 409);
        assert_eq!(status_do_erro(&PhxError::Conflito("x".into())), 409);
        assert_eq!(status_do_erro(&PhxError::Esquema("x".into())), 400);
        assert_eq!(status_do_erro(&PhxError::Tipo("x".into())), 400);
        assert_eq!(status_do_erro(&PhxError::EmCarga("x".into())), 503);
        assert_eq!(status_do_erro(&PhxError::Redireciona("x".into())), 421);
        assert_eq!(status_do_erro(&PhxError::Corrompido("x".into())), 500);
    }
}
