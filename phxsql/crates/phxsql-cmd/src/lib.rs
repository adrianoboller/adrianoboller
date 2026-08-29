//! # phxsql-cmd
//!
//! O console interativo: uma linha digitada vira um pedido do protocolo, e a
//! resposta vira uma tabela que dá para ler.
//!
//! # O que ele NÃO tem, e por que está escrito
//!
//! **Histórico e setas não existem nesta rodada.** Um `readline` de verdade --
//! seta para cima, ctrl+R, edição no meio da linha -- é um terminal em modo
//! cru, e isso é uma crate. A regra do projeto é zero dependências externas, e
//! `std::io::stdin().read_line` basta para o console fazer o que ele existe
//! para fazer. O `--help` diz isso na cara, porque descobrir sozinho apertando
//! a seta e vendo `^[[A` na tela é pior.
//!
//! # A ajuda vem do servidor, e não daqui
//!
//! `/help` é a op `catalogo` **pela rede**. Não há uma lista de comandos neste
//! arquivo, e é deliberado: uma lista aqui envelheceria calada, e o console
//! passaria a documentar um servidor que já mudou. Quem responde o que existe é
//! quem executa -- e ele só mostra o que aquela sessão pode chamar.

use std::time::Duration;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_server::replica::Cliente;

/// O que o console faz com a linha que acabou de ler.
#[derive(Debug, PartialEq)]
pub enum Saida {
    /// Nada a mostrar -- linha em branco, comentário.
    Nada,
    /// Mostre isto.
    Texto(String),
    /// Acabou.
    Sair,
}

impl Saida {
    /// O texto, ou vazio. Existe para o teste não repetir o `match`.
    pub fn texto(&self) -> &str {
        match self {
            Saida::Texto(t) => t,
            _ => "",
        }
    }
}

/// Quanto de uma célula cabe antes de ela ser cortada.
///
/// Cortar é uma decisão desconfortável: o que aparece na tela deixa de ser o
/// que está gravado. Por isso o corte é **marcado com `…`** -- quem lê tem de
/// saber que viu um pedaço, e não o valor. Uma coluna de memo com 4.000
/// caracteres arrebentaria a tela e esconderia as outras dez.
const LARGURA_MAX: usize = 40;

/// O console: uma conexão viva e o banco corrente.
pub struct Console {
    cliente: Cliente,
    /// O banco que entra no pedido quando a linha não diz qual. É o `/use`.
    pub database: String,
    /// Mostrar o JSON cru em vez da tabela. É o `/cru`.
    pub cru: bool,
    pub destino: String,
}

impl Console {
    /// Abre a conexão. Não autentica -- quem autentica é [`Console::entrar`],
    /// que é uma decisão de quem monta o console e não um efeito de conectar.
    pub fn ligar(host: &str, porta: u16, token: &str, espera: Duration) -> Result<Console> {
        Ok(Console {
            cliente: Cliente::conectar(host, porta, token, espera)?,
            database: String::new(),
            cru: false,
            destino: format!("{host}:{porta}"),
        })
    }

    /// Desafio-resposta, pelo MESMO caminho que a réplica usa.
    ///
    /// A senha não viaja: ela vira a chave derivada, e o que atravessa é o
    /// HMAC do nonce. Escrever um segundo caminho de autenticação aqui seria
    /// escrever um segundo jeito de errar.
    pub fn entrar(&mut self, usuario: &str, senha: &str) -> Result<()> {
        self.cliente.autenticar(usuario, "", senha)
    }

    /// Manda um pedido e devolve o `resultado`.
    pub fn pedir(&mut self, campos: &[(String, Json)]) -> Result<Json> {
        let refs: Vec<(&str, Json)> = campos
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect();
        self.cliente.pedir(refs)
    }

    /// Uma linha digitada, do começo ao fim.
    pub fn executar_linha(&mut self, linha: &str) -> Saida {
        let linha = linha.trim();
        if linha.is_empty() || linha.starts_with('#') {
            return Saida::Nada;
        }
        if let Some(resto) = linha.strip_prefix('/') {
            return self.comando_interno(resto.trim());
        }
        match self.pedido_da_linha(linha) {
            Err(e) => Saida::Texto(format!("erro: {e}")),
            Ok(campos) => match self.pedir(&campos) {
                Ok(r) if self.cru => Saida::Texto(r.escrever_identado()),
                Ok(r) => Saida::Texto(desenhar(&r)),
                Err(e) => Saida::Texto(format!("erro: {e}")),
            },
        }
    }

    /// Os comandos que o console atende sozinho, sem ir à rede.
    ///
    /// São poucos de propósito: tudo que o SERVIDOR sabe fazer se pede pelo
    /// nome da operação, e não por um comando inventado aqui que teria de ser
    /// mantido em dia com ele.
    fn comando_interno(&mut self, linha: &str) -> Saida {
        let (cmd, resto) = match linha.split_once(char::is_whitespace) {
            Some((c, r)) => (c, r.trim()),
            None => (linha, ""),
        };
        match cmd {
            "sair" | "quit" | "exit" | "q" => Saida::Sair,
            "help" | "ajuda" | "h" | "?" => self.ajuda(resto),
            "use" | "banco" | "database" => {
                self.database = resto.to_string();
                Saida::Texto(if self.database.is_empty() {
                    "banco corrente: (nenhum)".into()
                } else {
                    format!("banco corrente: {}", self.database)
                })
            }
            "cru" | "json" => {
                self.cru = !self.cru;
                Saida::Texto(format!(
                    "saida {}",
                    if self.cru { "em JSON cru" } else { "em tabela" }
                ))
            }
            outro => Saida::Texto(format!(
                "comando interno {outro:?} nao existe. Ha cinco: \
                 /help, /use, /cru, /sair -- e /help <operacao> detalha uma."
            )),
        }
    }

    /// `/help` e `/help <operacao>`, ambos vindos da op `catalogo`.
    fn ajuda(&mut self, qual: &str) -> Saida {
        let mut campos = vec![("op".to_string(), Json::texto_de("catalogo"))];
        if !self.database.is_empty() {
            campos.push(("database".to_string(), Json::texto_de(&self.database)));
        }
        if !qual.is_empty() {
            campos.push(("operacao".to_string(), Json::texto_de(qual)));
        }
        let r = match self.pedir(&campos) {
            Ok(r) => r,
            Err(e) => return Saida::Texto(format!("nao consegui pedir o catalogo: {e}")),
        };
        if qual.is_empty() {
            Saida::Texto(lista_de_operacoes(&r))
        } else {
            Saida::Texto(detalhe_da_operacao(&r, qual))
        }
    }

    /// A linha digitada vira os campos do pedido.
    ///
    /// Três formas, e cada uma existe por um motivo:
    ///
    /// * `{"op":...}` -- JSON cru, para o que a gramática de baixo não alcança;
    /// * `SELECT ...` -- vira `{"op":"sql"}`, porque num console de banco quem
    ///   digita SELECT quer consultar, e não escrever `sql texto=...`;
    /// * `operacao chave=valor ...` -- o caso comum.
    fn pedido_da_linha(&self, linha: &str) -> Result<Vec<(String, Json)>> {
        if linha.starts_with('{') {
            let j = Json::analisar(linha)?;
            let Json::Objeto(pares) = j else {
                return Err(PhxError::Esquema(
                    "o pedido cru tem de ser um objeto".into(),
                ));
            };
            return Ok(pares);
        }

        let (op, resto) = match linha.split_once(char::is_whitespace) {
            Some((o, r)) => (o.to_string(), r.trim().to_string()),
            None => (linha.to_string(), String::new()),
        };

        // Um comando SQL nao se quebra em `chave=valor`: o resto da linha E o
        // comando, com espacos, aspas e sinais de igual dentro.
        let sql_direto = op.eq_ignore_ascii_case("select") || op.eq_ignore_ascii_case("sql");
        if sql_direto {
            let texto = if op.eq_ignore_ascii_case("sql") {
                resto
            } else {
                linha.to_string()
            };
            if texto.trim().is_empty() {
                return Err(PhxError::Esquema(
                    "escreva o comando depois de `sql`".into(),
                ));
            }
            let mut campos = vec![
                ("op".to_string(), Json::texto_de("sql")),
                ("texto".to_string(), Json::texto_de(texto)),
            ];
            self.completar_database(&mut campos);
            return Ok(campos);
        }

        let mut campos = vec![("op".to_string(), Json::texto_de(&op))];
        for parte in partir(&resto) {
            let (chave, valor) = parte.split_once('=').ok_or_else(|| {
                PhxError::Esquema(format!(
                    "{parte:?} nao tem `=`. Os argumentos sao `chave=valor`; \
                     use /help {op} para ver quais"
                ))
            })?;
            campos.push((chave.trim().to_string(), valor_de_texto(valor)));
        }
        self.completar_database(&mut campos);
        Ok(campos)
    }

    /// Poe o banco corrente quando a linha nao disse qual.
    ///
    /// Nao sobrescreve o que foi digitado: quem escreveu `database=outro` numa
    /// linha quis aquele, e o `/use` e um padrao e nao uma imposicao.
    fn completar_database(&self, campos: &mut Vec<(String, Json)>) {
        if self.database.is_empty() || campos.iter().any(|(k, _)| k == "database") {
            return;
        }
        campos.push(("database".to_string(), Json::texto_de(&self.database)));
    }
}

/// Parte a linha em pedaços, respeitando aspas E valores JSON.
///
/// # Por que não basta quebrar no espaço
///
/// `nome="Ana Maria"` viraria dois argumentos, e o segundo não teria `=`: o
/// console recusaria um nome com espaço, que é metade dos nomes.
///
/// # E por que não basta tratar aspas
///
/// Esta foi a armadilha, e ela só apareceu com o servidor do outro lado. A
/// primeira versão tirava TODA aspa -- e aí `valores={"id":1,"nome":"Ana"}`
/// chegava como `{id:1,nome:Ana}`, que não é JSON. O console mandava aquilo
/// como TEXTO e o servidor respondia *«a linha precisa ser um objeto»*, um erro
/// que não aponta para o console em lugar nenhum. Ler o código não mostrava: o
/// teste de unidade do partidor passava, porque testava o nome com espaço.
///
/// Então a regra é por posição: a aspa **delimita** quando abre o pedaço (ou
/// vem logo depois do `=`), e é **literal** dentro de um `{...}` ou `[...]`,
/// onde o espaço também deixa de separar.
fn partir(linha: &str) -> Vec<String> {
    let mut saida = Vec::new();
    let mut atual = String::new();
    // Aspas que DELIMITAM o pedaço, fora de JSON.
    let mut aspas: Option<char> = None;
    // Profundidade de `{}` e `[]`: dentro dela o espaço não separa.
    let mut dentro = 0usize;
    // Aspas de um texto DENTRO do JSON -- para um `}` escrito dentro de uma
    // string não fechar o objeto antes da hora.
    let mut texto_json: Option<char> = None;
    let mut escapou = false;

    for c in linha.chars() {
        if let Some(a) = aspas {
            if c == a {
                aspas = None;
            } else {
                atual.push(c);
            }
            continue;
        }
        if dentro > 0 {
            atual.push(c);
            if escapou {
                escapou = false;
                continue;
            }
            match texto_json {
                Some(_) if c == '\\' => escapou = true,
                Some(a) if c == a => texto_json = None,
                Some(_) => {}
                None => match c {
                    '"' | '\'' => texto_json = Some(c),
                    '{' | '[' => dentro += 1,
                    '}' | ']' => dentro -= 1,
                    _ => {}
                },
            }
            continue;
        }
        match c {
            '{' | '[' => {
                dentro += 1;
                atual.push(c);
            }
            '"' | '\'' if atual.is_empty() || atual.ends_with('=') => aspas = Some(c),
            c if c.is_whitespace() => {
                if !atual.is_empty() {
                    saida.push(std::mem::take(&mut atual));
                }
            }
            c => atual.push(c),
        }
    }
    if !atual.is_empty() {
        saida.push(atual);
    }
    saida
}

/// O valor digitado vira JSON.
///
/// # Por que decimal continua texto
///
/// Porque `12.34` em `f64` não é 12.34, e o protocolo trafega decimal como
/// texto justamente por isso. Um valor com ponto vira TEXTO aqui, e quem
/// quiser um número de ponto flutuante de verdade escreve JSON cru. Errar para
/// o lado do texto perde nada; errar para o lado do `f64` perde centavo.
fn valor_de_texto(v: &str) -> Json {
    let t = v.trim();
    match t {
        "true" | "sim" => return Json::Bool(true),
        "false" | "nao" => return Json::Bool(false),
        "null" | "nulo" => return Json::Nulo,
        _ => {}
    }
    if t.starts_with('[') || t.starts_with('{') {
        if let Ok(j) = Json::analisar(t) {
            return j;
        }
    }
    if let Ok(n) = t.parse::<i64>() {
        return Json::de_i64(n);
    }
    Json::texto_de(t)
}

// ------------------------------------------------------------------ desenho

/// A resposta vira texto legível.
///
/// **Toda lista vira uma tabela, e não só a primeira.** O `esquema` traz
/// colunas, índices, chaves estrangeiras e volumes; mostrar só a primeira
/// esconderia três, e quem lê não teria como saber que faltou algo.
pub fn desenhar(r: &Json) -> String {
    match r {
        Json::Lista(itens) => tabela_ou_lista(itens),
        Json::Objeto(pares) => {
            let mut saida = String::new();
            let escalares: Vec<(&str, String)> = pares
                .iter()
                .filter(|(_, v)| !matches!(v, Json::Lista(_)))
                .map(|(k, v)| (k.as_str(), celula(v)))
                .collect();
            if !escalares.is_empty() {
                saida.push_str(
                    &escalares
                        .iter()
                        .map(|(k, v)| format!("{k}: {v}"))
                        .collect::<Vec<_>>()
                        .join("   "),
                );
                saida.push('\n');
            }
            for (k, v) in pares {
                if let Json::Lista(itens) = v {
                    saida.push_str(&format!("\n{k} ({}):\n", itens.len()));
                    saida.push_str(&tabela_ou_lista(itens));
                    saida.push('\n');
                }
            }
            saida.trim_end().to_string()
        }
        outro => celula(outro),
    }
}

/// Lista de objetos vira tabela; lista de valores vira uma coluna.
fn tabela_ou_lista(itens: &[Json]) -> String {
    if itens.is_empty() {
        return "  (vazio)".to_string();
    }
    if itens.iter().any(|i| !matches!(i, Json::Objeto(_))) {
        // **Sem corte aqui.** O corte de 40 caracteres existe para as colunas
        // da tabela ficarem alinhadas, e numa lista de valores nao ha coluna
        // nenhuma. Cortando, as `notas` do `sql` -- que sao FRASES -- viravam
        // «sem ORDER BY a ordem e a de DIGITACAO, …», que perde exatamente a
        // parte que a nota existe para dizer. Achado exercitando o console.
        return itens
            .iter()
            .map(|i| format!("  {}", inteiro(i)))
            .collect::<Vec<_>>()
            .join("\n");
    }

    // As colunas saem da UNIAO das chaves, na ordem em que aparecem: um objeto
    // da lista pode ter um campo que o outro nao tem, e usar so as chaves do
    // primeiro esconderia a diferenca.
    let mut colunas: Vec<String> = Vec::new();
    for i in itens {
        for k in i.chaves() {
            if !colunas.iter().any(|c| c == k) {
                colunas.push(k.to_string());
            }
        }
    }

    let linhas: Vec<Vec<String>> = itens
        .iter()
        .map(|i| {
            colunas
                .iter()
                .map(|c| match i.campo(c) {
                    Some(v) => celula(v),
                    None => String::new(),
                })
                .collect()
        })
        .collect();

    let largura: Vec<usize> = colunas
        .iter()
        .enumerate()
        .map(|(c, nome)| {
            linhas
                .iter()
                .map(|l| l[c].chars().count())
                .chain(std::iter::once(nome.chars().count()))
                .max()
                .unwrap_or(0)
        })
        .collect();

    let mut saida = String::new();
    saida.push_str(&formatar(&colunas, &largura));
    saida.push('\n');
    saida.push_str(
        &largura
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("  "),
    );
    for l in &linhas {
        saida.push('\n');
        saida.push_str(&formatar(l, &largura));
    }
    saida
}

fn formatar(campos: &[String], largura: &[usize]) -> String {
    campos
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let sobra = largura[i].saturating_sub(c.chars().count());
            format!("{c}{}", " ".repeat(sobra))
        })
        .collect::<Vec<_>>()
        .join("  ")
        .trim_end()
        .to_string()
}

/// Um valor numa célula.
///
/// **O texto sai como está gravado**, sem trocar de caixa e sem aspas: mostrar
/// «BLUMENAU» onde está gravado «Blumenau» é uma mentira sobre o dado, porque
/// quem olha não tem como saber qual dos dois está no disco. Só o corte por
/// largura muda o valor, e ele é marcado com `…`.
fn celula(v: &Json) -> String {
    let bruto = inteiro(v);
    if bruto.chars().count() <= LARGURA_MAX {
        return bruto;
    }
    let corte: String = bruto.chars().take(LARGURA_MAX - 1).collect();
    format!("{corte}…")
}

/// O valor por extenso, sem corte -- para onde não há coluna a alinhar.
///
/// Uma lista de valores simples sai separada por vírgula, e não como o JSON
/// dela: `["cliente_id"]` numa célula é ruído de formato onde se queria ler um
/// nome de coluna.
fn inteiro(v: &Json) -> String {
    let bruto = match v {
        Json::Texto(t) => t.clone(),
        Json::Nulo => String::new(),
        Json::Bool(b) => (if *b { "sim" } else { "nao" }).to_string(),
        Json::Numero(_) => v.escrever(),
        Json::Lista(itens)
            if itens
                .iter()
                .all(|i| !matches!(i, Json::Objeto(_) | Json::Lista(_))) =>
        {
            itens.iter().map(inteiro).collect::<Vec<_>>().join(", ")
        }
        outro => outro.escrever(),
    };
    bruto.replace(['\n', '\r', '\t'], " ")
}

/// O `/help` sem argumento: a lista, com o resumo de cada operação.
fn lista_de_operacoes(r: &Json) -> String {
    let ops = match r.campo("operacoes").and_then(Json::lista) {
        Some(l) => l,
        None => return "o servidor nao devolveu operacoes".to_string(),
    };
    let mut linhas: Vec<Json> = Vec::new();
    for o in ops {
        linhas.push(Json::objeto(vec![
            ("operacao", Json::texto_de(o.texto_ou("nome", ""))),
            (
                "permissao",
                Json::texto_de(match o.campo("permissao") {
                    Some(Json::Texto(t)) => t.clone(),
                    _ => "-".to_string(),
                }),
            ),
            ("grava", Json::Bool(o.booleano_ou("escreve", false))),
            ("o que faz", Json::texto_de(o.texto_ou("resumo", ""))),
        ]));
    }
    let ocultas = r.inteiro_ou("ocultas", 0);
    format!(
        "{}\n\n{} operacoes; {}. `/help <operacao>` detalha uma.",
        tabela_ou_lista(&linhas),
        linhas.len(),
        if ocultas > 0 {
            format!("{ocultas} escondidas por permissao")
        } else {
            "nenhuma escondida".to_string()
        }
    )
}

/// O `/help <operacao>`: parâmetros e exemplo.
fn detalhe_da_operacao(r: &Json, pedida: &str) -> String {
    let Some(o) = r.campo("operacao").filter(|o| !o.e_nulo()) else {
        return match r.campo("motivo") {
            Some(Json::Texto(m)) => m.clone(),
            _ => format!("a operacao {pedida:?} nao existe"),
        };
    };
    let mut saida = format!(
        "{}  --  {}\n",
        o.texto_ou("nome", ""),
        o.texto_ou("resumo", "")
    );
    let apelidos = o.textos("apelidos");
    if !apelidos.is_empty() {
        saida.push_str(&format!("tambem: {}\n", apelidos.join(", ")));
    }
    saida.push_str(&format!(
        "permissao: {}   grava: {}\n\n",
        match o.campo("permissao") {
            Some(Json::Texto(t)) => t.clone(),
            _ => "nenhuma".to_string(),
        },
        if o.booleano_ou("escreve", false) {
            "sim"
        } else {
            "nao"
        }
    ));
    match o.campo("parametros").and_then(Json::lista) {
        Some(ps) if !ps.is_empty() => {
            let linhas: Vec<Json> = ps
                .iter()
                .map(|p| {
                    Json::objeto(vec![
                        ("parametro", Json::texto_de(p.texto_ou("nome", ""))),
                        ("tipo", Json::texto_de(p.texto_ou("tipo", ""))),
                        (
                            "exige",
                            Json::texto_de(if p.booleano_ou("obrigatorio", false) {
                                "sim"
                            } else {
                                ""
                            }),
                        ),
                        ("para que serve", Json::texto_de(p.texto_ou("para_que", ""))),
                    ])
                })
                .collect();
            saida.push_str(&tabela_ou_lista(&linhas));
        }
        _ => saida.push_str("(sem parametros)"),
    }
    saida.push_str(&format!("\n\nexemplo: {}", o.texto_ou("exemplo", "")));
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn aspas_seguram_o_valor_com_espaco() {
        assert_eq!(
            partir(r#"nome="Ana Maria" cidade=Blumenau"#),
            vec!["nome=Ana Maria", "cidade=Blumenau"]
        );
        assert_eq!(partir("   "), Vec::<String>::new());
    }

    /// **A armadilha que so o servidor do outro lado mostrou.** A primeira
    /// versao tirava TODA aspa, e `valores={"id":1}` chegava como `{id:1}` --
    /// que nao e JSON. O console mandava texto e o servidor respondia «a linha
    /// precisa ser um objeto», um erro que nao aponta para o console.
    #[test]
    fn a_aspa_dentro_de_json_e_literal_e_o_espaco_nao_separa() {
        assert_eq!(
            partir(r#"valores={"id":1,"nome":"Ana Maria"}"#),
            vec![r#"valores={"id":1,"nome":"Ana Maria"}"#]
        );
        assert_eq!(
            valor_de_texto(r#"{"id":1,"nome":"Ana Maria"}"#),
            Json::objeto(vec![
                ("id", Json::de_i64(1)),
                ("nome", Json::texto_de("Ana Maria")),
            ])
        );
        // Objeto dentro de objeto, com um `}` escrito dentro de um texto.
        assert_eq!(
            partir(r#"a={"b":{"c":"}"}} d=1"#),
            vec![r#"a={"b":{"c":"}"}}"#, "d=1"]
        );
        // E a lista de chaves do `buscar`, que e o caso mais comum de todos.
        assert_eq!(
            partir("chave=[42] indice=porId"),
            vec!["chave=[42]", "indice=porId"]
        );
    }

    /// Decimal continua texto: `f64` nao representa 12.34, e o protocolo
    /// trafega decimal como texto justamente por isso.
    #[test]
    fn o_valor_com_ponto_vira_texto_e_nao_numero() {
        assert_eq!(valor_de_texto("12.34"), Json::texto_de("12.34"));
        assert_eq!(valor_de_texto("42"), Json::de_i64(42));
        assert_eq!(valor_de_texto("true"), Json::Bool(true));
        assert_eq!(valor_de_texto("null"), Json::Nulo);
        assert_eq!(
            valor_de_texto("[1,2]"),
            Json::Lista(vec![Json::de_i64(1), Json::de_i64(2)])
        );
        assert_eq!(valor_de_texto("Blumenau"), Json::texto_de("Blumenau"));
    }

    /// **A tabela nao mente sobre o dado.** Nao troca a caixa, nao poe aspas --
    /// e quando corta, marca o corte.
    #[test]
    fn a_celula_mostra_o_que_esta_gravado_e_marca_o_que_cortou() {
        assert_eq!(celula(&Json::texto_de("Blumenau")), "Blumenau");
        assert_eq!(celula(&Json::Nulo), "");
        assert_eq!(celula(&Json::Bool(true)), "sim");

        let longo = "a".repeat(200);
        let c = celula(&Json::texto_de(&longo));
        assert_eq!(c.chars().count(), LARGURA_MAX);
        assert!(c.ends_with('…'), "cortou sem avisar: {c}");
    }

    /// Uma coluna que so o segundo objeto tem nao pode sumir: usar as chaves do
    /// primeiro esconderia a diferenca entre as linhas.
    #[test]
    fn a_tabela_junta_as_colunas_de_todas_as_linhas() {
        let itens = vec![
            Json::objeto(vec![("a", Json::de_i64(1))]),
            Json::objeto(vec![("b", Json::de_i64(2))]),
        ];
        let t = tabela_ou_lista(&itens);
        let cabecalho = t.lines().next().unwrap();
        assert!(cabecalho.contains('a') && cabecalho.contains('b'), "{t}");
    }

    /// **A frase nao se corta.** O corte de 40 caracteres existe para alinhar
    /// coluna de tabela, e numa lista de valores nao ha coluna. Cortando, a
    /// nota do `sql` virava «sem ORDER BY a ordem e a de DIGITACAO, …» -- e
    /// perdia justamente a parte que ela existe para dizer. Achado
    /// exercitando o console, e nao lendo o codigo.
    #[test]
    fn a_lista_de_frases_sai_inteira_e_a_celula_da_tabela_continua_cortando() {
        let frase = "sem ORDER BY a ordem e a de DIGITACAO, que no PhxSql e estavel";
        let t = tabela_ou_lista(&[Json::texto_de(frase)]);
        assert!(t.contains(frase), "cortou a frase: {t}");

        // Mas dentro de uma tabela o corte continua, senao uma coluna de memo
        // arrebenta a tela e esconde as outras dez.
        let t = tabela_ou_lista(&[Json::objeto(vec![("nota", Json::texto_de(frase))])]);
        assert!(t.contains('…'), "a celula da tabela deixou de cortar: {t}");
    }

    /// Uma lista de valores dentro de uma celula sai por virgula, e nao como o
    /// JSON dela: `["cliente_id"]` e ruido de formato onde se queria ler um
    /// nome de coluna.
    #[test]
    fn lista_de_valores_na_celula_sai_por_virgula() {
        assert_eq!(
            celula(&Json::Lista(vec![Json::texto_de("a"), Json::texto_de("b")])),
            "a, b"
        );
        assert_eq!(celula(&Json::Lista(vec![])), "");
        // Lista de objetos continua saindo como JSON: nao ha como resumi-la
        // numa celula sem esconder campo.
        assert!(celula(&Json::Lista(vec![Json::objeto(vec![(
            "x",
            Json::de_i64(1)
        )])]))
        .contains('{'));
    }

    #[test]
    fn lista_vazia_diz_que_esta_vazia_em_vez_de_nao_mostrar_nada() {
        assert_eq!(tabela_ou_lista(&[]), "  (vazio)");
    }

    /// O `esquema` traz quatro listas. Mostrar so a primeira esconderia tres, e
    /// quem le nao teria como saber que faltou.
    #[test]
    fn toda_lista_do_objeto_vira_uma_tabela() {
        let r = Json::objeto(vec![
            ("tabela", Json::texto_de("clientes")),
            (
                "colunas",
                Json::Lista(vec![Json::objeto(vec![("nome", Json::texto_de("id"))])]),
            ),
            (
                "indices",
                Json::Lista(vec![Json::objeto(vec![("nome", Json::texto_de("porId"))])]),
            ),
        ]);
        let t = desenhar(&r);
        assert!(t.contains("tabela: clientes"), "{t}");
        assert!(t.contains("colunas (1)"), "{t}");
        assert!(t.contains("indices (1)"), "{t}");
        assert!(t.contains("porId"), "{t}");
    }
}
