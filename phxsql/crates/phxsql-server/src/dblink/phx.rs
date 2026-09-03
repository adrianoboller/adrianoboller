//! O DbLink que fala com OUTRO PhxSql -- pelo protocolo proprio.
//!
//! # Por que ele nao e mais um dialeto de SQL
//!
//! Os outros dois motores respondem o catalogo em SQL (`SHOW FULL COLUMNS`,
//! `pg_attribute`), e por isso cabem no mesmo cano: monta-se a instrucao e
//! le-se o resultado. O PhxSql nao tem catalogo em SQL -- ele tem `bancos`,
//! `sistabelas`, `esquema` e `varrer`, que sao as MESMAS operacoes que a tela
//! usa e que trazem mais do que o `SHOW` traria (chave primaria, papel da
//! coluna nos indices, dado pessoal).
//!
//! Traduzir isso para SQL de ida e de volta seria inventar uma lingua so para
//! desinventa-la do outro lado. Entao aqui o "dialeto" e o proprio protocolo.
//!
//! # O que ele reaproveita, e por que isso e o desenho certo
//!
//! O cliente e o [`crate::replica::Cliente`], que existe desde a replicacao:
//! ele abre o soquete, faz o desafio-resposta (a senha NAO viaja) e classifica
//! o erro que o outro lado devolveu. Escrever um segundo cliente do nosso
//! proprio protocolo seria o segundo caminho ate o dado -- e o segundo caminho
//! e sempre o que esquece uma conferencia.
//!
//! # As duas credenciais, e por que sao duas
//!
//! O PhxSql tem DOIS portoes em serie: o token de servico (a chave da porta da
//! rede, conferido antes de tudo) e o login (a identidade). Uma ligacao para
//! PhxSql precisa dos dois quando o outro servidor tem cadastro -- e so do
//! token quando ele nao tem. Medido contra um servidor de verdade: `login`
//! sem token responde `token invalido`, entao guardar so usuario e senha
//! produziria uma ligacao que nunca conecta.
//!
//! Com `usuario` vazio a ligacao entra so pelo token, que e o modo do servidor
//! sem cadastro. Com `usuario` preenchido ela faz o login por cima.
//!
//! # O limite honesto: o fio vai em claro
//!
//! Igual aos outros dois motores, e pelo mesmo motivo (a `std` nao traz TLS).
//! A SENHA nunca viaja -- o desafio-resposta cuida disso --, mas o DADO
//! devolvido sim, e o TOKEN vai no pedido. Rede interna, VPN ou tunel.

use std::time::Duration;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

use super::conexao::{Coluna, Resultado};
use super::{nome_seguro, Definicao};
use crate::replica::Cliente;

/// A conexao com o outro PhxSql.
pub struct Conexao {
    cliente: Cliente,
    /// A versao que o `ping` do outro lado anunciou.
    pub versao: String,
    /// O papel dele: isolado, source, replica, spare.
    pub papel: String,
}

impl Conexao {
    pub fn abrir(
        host: &str,
        porta: u16,
        token: &str,
        usuario: &str,
        senha: &str,
        espera: Duration,
    ) -> Result<Conexao> {
        let mut cliente = Cliente::conectar(host, porta, token, espera)?;
        // O login vem DEPOIS do token porque e assim que o outro lado confere:
        // o portao do token e o primeiro, e um login com token errado responde
        // "token invalido" -- que mandaria procurar a senha no lugar errado.
        if !usuario.is_empty() {
            cliente
                .autenticar(usuario, "", senha)
                .map_err(|e| ensinar_onde_vai_o_token(e, token))?;
        }
        let p = cliente
            .pedir(vec![("op", Json::texto_de("ping"))])
            .map_err(|e| ensinar_onde_vai_o_token(e, token))?;
        Ok(Conexao {
            versao: p.texto_ou("phxsql", "").to_string(),
            papel: p.texto_ou("papel", "").to_string(),
            cliente,
        })
    }

    pub fn ping(&mut self) -> Result<Json> {
        self.cliente.pedir(vec![("op", Json::texto_de("ping"))])
    }

    /// Um pedido qualquer do protocolo, ja com o `resultado` desembrulhado.
    pub fn pedir(&mut self, campos: Vec<(&str, Json)>) -> Result<Json> {
        self.cliente.pedir(campos)
    }

    /// Uma instrucao SQL, executada pela op `sql` do outro lado.
    ///
    /// O `teto` NAO vira `LIMIT` na instrucao: quem escreveu o SQL pode ter
    /// posto o dele, e emendar um segundo mudaria o que a pessoa pediu. Ele
    /// corta a resposta AQUI, e a resposta diz que cortou (`truncado`) -- o
    /// mesmo contrato dos outros dois clientes.
    pub fn consultar(&mut self, database: &str, sql: &str, teto: u64) -> Result<Resultado> {
        let r = self.cliente.pedir(vec![
            ("op", Json::texto_de("sql")),
            ("database", Json::texto_de(database)),
            ("texto", Json::texto_de(sql)),
        ])?;
        Ok(resultado_do_sql(&r, teto))
    }
}

/// Troca «token invalido» por uma frase que diz ONDE se grava o token.
///
/// A recusa crua e verdadeira e inutil: ela fala do portao 1 do OUTRO
/// servidor, e quem a le nesta ponta procura a senha. Pior ainda quando a
/// ligacao foi criada pela TELA, que ainda nao tem campo de token -- o
/// operador nao tem como adivinhar que falta um campo que ele nao viu.
///
/// So troca quando o token esta VAZIO: com token preenchido, «token invalido»
/// quer dizer mesmo que o token esta errado, e trocar a frase ali esconderia a
/// causa verdadeira. E a mesma regra do erro que nomeia a tabela que falta em
/// vez de vazar «nenhum volume de clientes.reg».
fn ensinar_onde_vai_o_token(e: PhxError, token: &str) -> PhxError {
    if !token.is_empty() || !e.to_string().contains("token invalido") {
        return e;
    }
    PhxError::Autorizacao(
        "o outro PhxSql exige token de servico e esta ligacao nao tem nenhum: \
         grave o campo \"token_remoto\" (ou \"token_remoto_env\") na ligacao, pelo \
         dblink_salvar ou no dblink.json. A tela ainda nao oferece esse campo."
            .into(),
    )
}

/// O valor de um campo do protocolo, como a grade o mostra.
///
/// `Nulo` vira `None` -- NULL de verdade, e nao cadeia vazia; a diferenca
/// importa e ja custou uma linha de comentario nos outros dois clientes.
///
/// O booleano sai como `1`/`0`, e nao como `true`/`false`. Nao e gosto: e o
/// contrato que `dialeto::booleano_lido` ja le, e que o MySQL(R) tambem usa.
/// Mandar `true` faria toda comparacao ingenua (`== "1"`) tratar o booleano
/// como falso, sem erro nenhum -- o pior jeito de estar errado, e a mesma
/// armadilha que o `t`/`f` do PostgreSQL(R) armou uma vez.
fn como_texto(v: &Json) -> Option<String> {
    match v {
        Json::Nulo => None,
        Json::Texto(t) => Some(t.clone()),
        Json::Bool(b) => Some(if *b { "1".into() } else { "0".into() }),
        // Numero inteiro sai sem `.0` (o escritor do Json ja cuida disso), e
        // lista/objeto saem como o JSON deles -- que e a verdade sobre um
        // campo composto, em vez de um "[objeto]" que nao diz nada.
        outro => Some(outro.escrever()),
    }
}

/// Monta o `Resultado` a partir de linhas-objeto e de uma ordem de colunas.
///
/// A ordem vem de FORA porque o protocolo devolve cada linha como objeto, e
/// objeto nao carrega a ordem do esquema -- carrega a ordem em que o servidor
/// escreveu aquela linha. Tirar a ordem da primeira linha funcionaria hoje e
/// mentiria no dia em que uma linha viesse sem um campo nulo.
fn linhas_por_colunas(
    linhas: &[Json],
    colunas: &[Coluna],
    teto: u64,
) -> (Vec<Vec<Option<String>>>, bool) {
    let truncado = linhas.len() as u64 > teto;
    let saida = linhas
        .iter()
        .take(teto as usize)
        .map(|l| {
            colunas
                .iter()
                .map(|c| l.campo(&c.nome).and_then(como_texto))
                .collect()
        })
        .collect();
    (saida, truncado)
}

/// Uma coluna de texto simples, para as respostas que nao tem esquema.
fn coluna_texto(nome: &str) -> Coluna {
    Coluna {
        nome: nome.to_string(),
        tipo: "texto".into(),
        nulavel: true,
        ..Coluna::default()
    }
}

/// A resposta da op `sql` no formato da grade.
///
/// Tres formas cabem aqui, e a terceira e a que engana:
///
/// - com `colunas`, a projecao que o SELECT pediu;
/// - sem `colunas`, a linha inteira -- e ai a ordem sai das chaves da PRIMEIRA
///   linha, que e o unico lugar onde ela existe;
/// - com `contagem`, um `COUNT(*)`, que NAO tem linha nenhuma. Devolver a
///   linha que vem junto faria um `SELECT COUNT(*)` mostrar um registro de
///   dado, e quem olha nao saberia se aquilo quer dizer alguma coisa -- o
///   mesmo defeito que exercitar o console achou no proprio servidor.
fn resultado_do_sql(r: &Json, teto: u64) -> Resultado {
    if let Some(n) = r.campo("contagem").and_then(Json::inteiro) {
        return Resultado {
            colunas: vec![Coluna {
                nome: "contagem".into(),
                tipo: "Int8".into(),
                numerico: true,
                ..Coluna::default()
            }],
            linhas: vec![vec![Some(n.to_string())]],
            ..Resultado::default()
        };
    }
    let linhas = r.campo("linhas").and_then(Json::lista).unwrap_or(&[]);
    let nomes: Vec<String> = match r.campo("colunas").and_then(Json::lista) {
        Some(l) => l
            .iter()
            .filter_map(|c| c.texto().map(str::to_string))
            .collect(),
        None => linhas
            .first()
            .map(|l| l.chaves().iter().map(|c| c.to_string()).collect())
            .unwrap_or_default(),
    };
    let colunas: Vec<Coluna> = nomes.iter().map(|n| coluna_texto(n)).collect();
    let (linhas, truncado) = linhas_por_colunas(linhas, &colunas, teto);
    Resultado {
        colunas,
        linhas,
        afetadas: 0,
        truncado,
    }
}

// ------------------------------------------------------- as seis operacoes

/// `dblink_testar`: quem esta do outro lado, e com que identidade falamos.
pub fn testar(d: &Definicao, mut c: Conexao) -> Result<Json> {
    let comeco = std::time::Instant::now();
    let p = c.ping()?;
    // `quem_sou` responde a ficha do usuario, ou `{"usuario":null,...}` quando
    // so o token entrou. Os dois casos sao verdade, e a resposta diz qual.
    let eu = c.pedir(vec![("op", Json::texto_de("quem_sou"))]).ok();
    let usuario = eu
        .as_ref()
        .map(|e| e.texto_ou("login", "").to_string())
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| "(token de servico)".into());
    Ok(Json::objeto(vec![
        ("ok", Json::Bool(true)),
        ("dblink", Json::texto_de(&d.nome)),
        ("motor", Json::texto_de(d.motor.nome())),
        ("versao", Json::texto_de(p.texto_ou("phxsql", ""))),
        // O outro PhxSql nao numera a conexao como o MySQL(R) e o
        // PostgreSQL(R) numeram: `conexoes` e quantas ha, e nao qual e a
        // nossa. Zero aqui e a verdade -- inventar um numero seria pior.
        ("conexao_id", Json::de_u64(0)),
        ("usuario_efetivo", Json::texto_de(usuario)),
        ("database", Json::texto_de(&d.database)),
        // O que so este motor responde, e que vale saber antes de ler: um
        // source e uma replica do mesmo cluster respondem coisas diferentes.
        ("papel", Json::texto_de(p.texto_ou("papel", ""))),
        ("id_servidor", Json::texto_de(p.texto_ou("id_servidor", ""))),
        ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
    ]))
}

/// `dblink_bancos`: os databases do outro PhxSql.
pub fn bancos(_d: &Definicao, mut c: Conexao) -> Result<Json> {
    let r = c.pedir(vec![("op", Json::texto_de("bancos"))])?;
    // O `bancos` responde uma LISTA direta. O campo `bancos` tambem e aceito
    // porque um servidor de outra versao pode responder assim -- e foi por
    // supor um formato so que a replicacao ja deixou de replicar em silencio.
    let lista = r
        .lista()
        .or_else(|| r.campo("bancos").and_then(Json::lista))
        .unwrap_or(&[]);
    Ok(Json::objeto(vec![(
        "bancos",
        Json::Lista(
            lista
                .iter()
                // O filtro trabalha no TEXTO, e nao no `Json` ja embrulhado.
                // A primeira versao filtrava com `b.texto_ou("", "")`, que
                // procura um CAMPO de nome vazio num valor escalar e devolve
                // sempre o padrao: a lista saia VAZIA, sem erro nenhum. E o
                // mesmo sintoma mudo da lista de tabelas do PostgreSQL(R), e
                // de novo foi a prova por soquete que o achou.
                .map(|b| match b {
                    Json::Texto(t) => t.clone(),
                    outro => outro.texto_ou("nome", "").to_string(),
                })
                .filter(|n| !n.is_empty())
                .map(Json::texto_de)
                .collect(),
        ),
    )]))
}

/// `dblink_tabelas`: as tabelas de um database, pelo `sistabelas`.
///
/// # Dois numeros que NAO querem dizer o mesmo dos outros motores
///
/// `registros_estimados` existe para a tela, que le esse nome nos tres
/// motores. Nos outros dois ele e mesmo estimativa (`TABLE_ROWS` do InnoDB,
/// `reltuples` do PostgreSQL(R)); aqui e CONTAGEM, lida do cabecalho da
/// tabela. O campo `registros` vai junto dizendo isso, em vez de a resposta
/// fingir uma imprecisao que nao tem.
///
/// `bytes` e o `.reg`, e so ele: `slots x bytes_por_linha`. Nao inclui `.ndx`,
/// `.memo` nem `.bin`. Somar o que nao se mediu seria numero citado.
pub fn tabelas(d: &Definicao, mut c: Conexao, p: &Json) -> Result<Json> {
    let base = base_do_pedido(d, p);
    let r = c.pedir(vec![
        ("op", Json::texto_de("sistabelas")),
        ("database", Json::texto_de(&base)),
    ])?;
    let linhas = r.campo("tabelas").and_then(Json::lista).unwrap_or(&[]);
    Ok(Json::objeto(vec![
        ("dblink", Json::texto_de(&d.nome)),
        ("database", Json::texto_de(&base)),
        (
            "tabelas",
            Json::Lista(
                linhas
                    .iter()
                    .map(|t| {
                        let registros = t.inteiro_ou("registros", 0).max(0) as u64;
                        let bytes = (t.inteiro_ou("slots", 0).max(0) as u64)
                            .saturating_mul(t.inteiro_ou("bytes_por_linha", 0).max(0) as u64);
                        Json::objeto(vec![
                            ("nome", Json::texto_de(t.texto_ou("tabela", ""))),
                            ("tipo", Json::texto_de("BASE TABLE")),
                            ("motor", Json::texto_de("phxsql")),
                            ("registros_estimados", Json::de_u64(registros)),
                            ("registros", Json::de_u64(registros)),
                            ("bytes", Json::de_u64(bytes)),
                            ("bytes_de", Json::texto_de(".reg")),
                            // O `sistabelas` nao guarda comentario de tabela;
                            // vazio e a verdade. O que ele tem a mais vai
                            // junto, porque e o que a tela do outro lado ja
                            // mostra e ninguem teria como perguntar depois.
                            ("comentario", Json::texto_de("")),
                            ("schema", Json::texto_de(t.texto_ou("schema", ""))),
                            (
                                "chave_primaria",
                                match t.campo("chave_primaria") {
                                    Some(Json::Texto(k)) => Json::texto_de(k),
                                    _ => Json::Nulo,
                                },
                            ),
                            (
                                "indices",
                                Json::de_u64(t.inteiro_ou("indices", 0).max(0) as u64),
                            ),
                            (
                                "chaves_estrangeiras",
                                Json::de_u64(t.inteiro_ou("chaves_estrangeiras", 0).max(0) as u64),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]))
}

/// `dblink_estrutura`: colunas e indices, com os NOMES dos outros dois motores.
///
/// # Por que os nomes do `SHOW` e nao os do `esquema`
///
/// Porque o `docs/DBLINK.md` ja manda o cliente ler por NOME (`Field`, `Type`,
/// `Key_name`...), e um terceiro motor com um terceiro conjunto de nomes faria
/// cada cliente crescer um `if` por motor -- que e a porta pela qual o motor
/// esquecido para de funcionar sem ninguem ver.
///
/// O que o `esquema` tem a mais e que nao cabe nesses nomes (rotulo, mascara,
/// dado pessoal, papel da coluna) fica em colunas EXTRA, depois das seis. Quem
/// le por nome as encontra; quem le por posicao continua vendo as seis
/// primeiras iguais -- e ler por posicao ja era ler por sorte.
pub fn estrutura(d: &Definicao, mut c: Conexao, p: &Json) -> Result<Json> {
    let tabela = nome_seguro(p.texto_ou("tabela", ""))?;
    let base = base_do_pedido(d, p);
    let e = c.pedir(vec![
        ("op", Json::texto_de("esquema")),
        ("database", Json::texto_de(&base)),
        ("tabela", Json::texto_de(&tabela)),
    ])?;
    Ok(Json::objeto(vec![
        ("dblink", Json::texto_de(&d.nome)),
        ("tabela", Json::texto_de(&tabela)),
        ("colunas", colunas_do_esquema(&e).para_json()),
        ("indices", indices_do_esquema(&e).para_json()),
    ]))
}

/// As colunas do `esquema` com os nomes do `SHOW FULL COLUMNS`.
fn colunas_do_esquema(e: &Json) -> Resultado {
    let colunas = [
        "Field",
        "Type",
        "Null",
        "Key",
        "Default",
        "Comment",
        "Sistema",
        "Rotulo",
        "Mascara",
        "DadoPessoal",
    ]
    .iter()
    .map(|n| coluna_texto(n))
    .collect();
    let linhas = e
        .campo("colunas")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .map(|c| {
            // `MUL` cobre os dois casos que o MySQL(R) chama assim: coluna
            // que e chave estrangeira e coluna que so aparece num indice nao
            // unico. Sao motivos diferentes com a MESMA letra, e por isso o
            // `||` -- separar em dois ramos escreveria "MUL" duas vezes sem
            // dizer nada a mais.
            let em_indice = c
                .campo("nos_indices")
                .and_then(Json::lista)
                .is_some_and(|i| !i.is_empty());
            let chave = if c.booleano_ou("primaria", false) {
                "PRI"
            } else if c.booleano_ou("estrangeira", false) || em_indice {
                "MUL"
            } else {
                ""
            };
            vec![
                Some(c.texto_ou("nome", "").to_string()),
                Some(c.texto_ou("tipo", "").to_string()),
                // `YES`/`NO` sao os do MySQL(R), e o PostgreSQL(R) ja os
                // imita no dialeto. Um terceiro vocabulario aqui obrigaria
                // todo leitor a saber de que motor a resposta veio.
                Some(
                    if c.booleano_ou("nullable", false) {
                        "YES"
                    } else {
                        "NO"
                    }
                    .to_string(),
                ),
                Some(chave.to_string()),
                // O PhxSql nao guarda DEFAULT por coluna: NULL e a verdade,
                // e nao cadeia vazia, que seria "o padrao e vazio".
                None,
                Some(c.texto_ou("descricao", "").to_string()),
                Some(
                    if c.booleano_ou("sistema", false) {
                        "1"
                    } else {
                        "0"
                    }
                    .to_string(),
                ),
                Some(c.texto_ou("rotulo", "").to_string()),
                Some(c.texto_ou("mascara", "").to_string()),
                Some(c.texto_ou("dado_pessoal", "").to_string()),
            ]
        })
        .collect();
    Resultado {
        colunas,
        linhas,
        ..Resultado::default()
    }
}

/// Os indices do `esquema` com os nomes do `SHOW INDEX`.
///
/// `Non_unique` tem a polaridade do NOME, como nos outros dois: **0 quer dizer
/// unico**. Inverter aqui faria a tela marcar como unico exatamente o que nao
/// e, e o erro passaria batido porque a coluna se chama "nao unico".
fn indices_do_esquema(e: &Json) -> Resultado {
    let colunas = [
        "Key_name",
        "Column_name",
        "Non_unique",
        "Seq_in_index",
        "Primario",
    ]
    .iter()
    .map(|n| coluna_texto(n))
    .collect();
    let mut linhas = Vec::new();
    for i in e.campo("indices").and_then(Json::lista).unwrap_or(&[]) {
        let nome = i.texto_ou("nome", "").to_string();
        let unico = i.booleano_ou("unico", false);
        let primario = i.booleano_ou("primario", false);
        for (n, coluna) in i
            .campo("colunas")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .enumerate()
        {
            linhas.push(vec![
                Some(nome.clone()),
                Some(match coluna {
                    Json::Texto(t) => t.clone(),
                    outro => outro.texto_ou("nome", "").to_string(),
                }),
                Some(if unico { "0" } else { "1" }.to_string()),
                Some((n + 1).to_string()),
                Some(if primario { "1" } else { "0" }.to_string()),
            ]);
        }
    }
    Resultado {
        colunas,
        linhas,
        ..Resultado::default()
    }
}

/// `dblink_ler`: o conteudo de uma tabela, paginado pelo `varrer`.
///
/// # A ordem, e por que ela RECUSA em vez de ignorar
///
/// O `varrer` le na ordem de DIGITACAO, ou na ordem de um INDICE nomeado --
/// nao ha `ORDER BY` por coluna qualquer, porque nao ha varredura que ordene.
/// Aceitar `ordem` e devolver a ordem de digitacao mostraria a grade com o
/// cabecalho marcado como ordenado e o dado na ordem errada: quem olha nao
/// teria como saber. Entao `ordem` recusa dizendo isso, e a tela -- que nao
/// manda `ordem` para o DbLink -- continua funcionando.
pub fn ler(d: &Definicao, mut c: Conexao, p: &Json) -> Result<Json> {
    let tabela = nome_seguro(p.texto_ou("tabela", ""))?;
    let base = base_do_pedido(d, p);
    if !p.texto_ou("ordem", "").trim().is_empty() {
        return Err(PhxError::Esquema(
            "o motor phxsql le na ordem de digitacao ou por um INDICE, e nao por \
             coluna qualquer: use \"dblink_consultar\" com ORDER BY, que roda no \
             outro servidor"
                .into(),
        ));
    }
    let limite = p
        .inteiro_ou("limite", d.max_linhas as i64)
        .clamp(1, d.max_linhas as i64);
    let salto = p.inteiro_ou("salto", 0).max(0);

    // O esquema vem ANTES das linhas, e nao e desperdicio: e dele que sai a
    // ORDEM das colunas. Tirar a ordem da primeira linha funcionaria ate a
    // primeira tabela vazia -- que mostraria uma grade sem cabecalho nenhum.
    let e = c.pedir(vec![
        ("op", Json::texto_de("esquema")),
        ("database", Json::texto_de(&base)),
        ("tabela", Json::texto_de(&tabela)),
    ])?;
    let colunas = colunas_da_grade(&e);

    // Uma linha a mais do que o teto: se ela vier, ha mais pagina. O mesmo
    // truque do caminho SQL, para a resposta dizer `tem_mais` sem contar.
    let r = c.pedir(vec![
        ("op", Json::texto_de("varrer")),
        ("database", Json::texto_de(&base)),
        ("tabela", Json::texto_de(&tabela)),
        ("pular", Json::de_i64(salto)),
        ("max", Json::de_i64(limite + 1)),
    ])?;
    let brutas = r.campo("linhas").and_then(Json::lista).unwrap_or(&[]);
    let tem_mais = brutas.len() as i64 > limite;
    let (linhas, _) = linhas_por_colunas(brutas, &colunas, limite as u64);
    let resultado = Resultado {
        colunas,
        linhas,
        afetadas: 0,
        truncado: false,
    };
    let mut saida = resultado.para_json();
    if let Json::Objeto(campos) = &mut saida {
        campos.push(("dblink".into(), Json::texto_de(&d.nome)));
        campos.push(("tabela".into(), Json::texto_de(&tabela)));
        campos.push(("salto".into(), Json::de_u64(salto as u64)));
        campos.push(("tem_mais".into(), Json::Bool(tem_mais)));
        // O que so este motor sabe responder, e de graca: o `varrer` ja conta.
        campos.push((
            "registros".into(),
            Json::de_u64(r.inteiro_ou("registros", 0).max(0) as u64),
        ));
    }
    Ok(saida)
}

/// As colunas da grade, na ordem do esquema.
///
/// A coluna de SISTEMA fica, e isso e decisao: `softdeleted` e `rownum` estao
/// no dado do outro lado, e esconde-las aqui faria a grade do DbLink mostrar
/// menos do que a tabela tem sem dizer que escondeu.
fn colunas_da_grade(e: &Json) -> Vec<Coluna> {
    let mut colunas = vec![Coluna {
        // O `varrer` devolve o `rowid` em toda linha, e ele nao esta no
        // esquema: e a identidade da linha no `.reg`. Sem ele a grade perderia
        // a unica coluna que permite voltar a linha depois.
        nome: "rowid".into(),
        tipo: "Int8".into(),
        numerico: true,
        primaria: true,
        ..Coluna::default()
    }];
    for c in e.campo("colunas").and_then(Json::lista).unwrap_or(&[]) {
        let tipo = c.texto_ou("tipo", "").to_string();
        colunas.push(Coluna {
            nome: c.texto_ou("nome", "").to_string(),
            tabela: e.texto_ou("tabela", "").to_string(),
            tamanho: c.inteiro_ou("tamanho", 0).max(0) as u32,
            decimais: 0,
            nulavel: c.booleano_ou("nullable", false),
            primaria: c.booleano_ou("primaria", false),
            numerico: tipo.starts_with("Int")
                || tipo.starts_with("Decimal")
                || tipo.starts_with("Float")
                || tipo == "Sequence",
            tipo,
        });
    }
    colunas
}

/// Qual database usar: o do pedido, ou o da ligacao.
///
/// Aqui os dois querem dizer a MESMA coisa -- database e database --, e por
/// isso esta funcao e de uma linha. Foi no PostgreSQL(R) que "database" queria
/// dizer esquema, e confundir os dois deixou a lista de tabelas vazia e calada.
fn base_do_pedido(d: &Definicao, p: &Json) -> String {
    match p.texto_ou("database", "").trim() {
        "" => d.database.clone(),
        outro => outro.to_string(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn esquema_de_prova() -> Json {
        Json::analisar(
            r#"{"tabela":"clientes",
                "colunas":[
                  {"nome":"id","tipo":"Int8","tamanho":8,"nullable":false,
                   "primaria":true,"estrangeira":false,"sistema":false,
                   "descricao":"","rotulo":"id","mascara":"","dado_pessoal":"nao",
                   "nos_indices":["pk_id"]},
                  {"nome":"cidade","tipo":"Str(30)","tamanho":30,"nullable":true,
                   "primaria":false,"estrangeira":false,"sistema":false,
                   "descricao":"onde mora","rotulo":"Cidade","mascara":"",
                   "dado_pessoal":"nao","nos_indices":["porCidade"]},
                  {"nome":"softdeleted","tipo":"Bool","tamanho":1,"nullable":false,
                   "primaria":false,"estrangeira":false,"sistema":true,
                   "descricao":"","rotulo":"Excluido","mascara":"",
                   "dado_pessoal":"nao","nos_indices":[]}],
                "indices":[
                  {"nome":"pk_id","colunas":["id"],"unico":true,"primario":true},
                  {"nome":"porCidade","colunas":["cidade"],"unico":false,"primario":false}]}"#,
        )
        .unwrap()
    }

    /// O contrato do `docs/DBLINK.md`: o cliente le por NOME, e o nome e o
    /// mesmo nos tres motores. Um terceiro vocabulario obrigaria cada leitor a
    /// crescer um `if` por motor.
    #[test]
    fn a_estrutura_sai_com_os_nomes_do_show() {
        let r = colunas_do_esquema(&esquema_de_prova());
        let nomes: Vec<&str> = r.colunas.iter().map(|c| c.nome.as_str()).collect();
        assert_eq!(
            &nomes[..6],
            &["Field", "Type", "Null", "Key", "Default", "Comment"]
        );
        assert_eq!(r.celula(0, 0).unwrap(), "id");
        assert_eq!(r.celula(0, 2).unwrap(), "NO");
        assert_eq!(r.celula(0, 3).unwrap(), "PRI");
        assert_eq!(r.celula(1, 2).unwrap(), "YES");
        // Coluna so indexada e MUL, como no MySQL(R).
        assert_eq!(r.celula(1, 3).unwrap(), "MUL");
        // Sem DEFAULT no PhxSql: NULL de verdade, nao cadeia vazia.
        assert!(r.linhas[0][4].is_none());
    }

    /// A polaridade que o nome inverte, e que passaria batido: **0 e unico**.
    #[test]
    fn non_unique_e_zero_quando_o_indice_e_unico() {
        let r = indices_do_esquema(&esquema_de_prova());
        assert_eq!(r.celula(0, 0).unwrap(), "pk_id");
        assert_eq!(r.celula(0, 2).unwrap(), "0", "unico tem de sair 0");
        assert_eq!(r.celula(0, 3).unwrap(), "1", "Seq_in_index comeca em 1");
        assert_eq!(r.celula(1, 0).unwrap(), "porCidade");
        assert_eq!(r.celula(1, 2).unwrap(), "1", "nao unico tem de sair 1");
    }

    /// O booleano sai `1`/`0`, e nao `true`/`false`: e o que
    /// `dialeto::booleano_lido` le e o que o MySQL(R) manda. `true` faria toda
    /// comparacao `== "1"` tratar o valor como falso, sem erro nenhum.
    #[test]
    fn o_booleano_sai_como_um_e_zero() {
        assert_eq!(como_texto(&Json::Bool(true)).unwrap(), "1");
        assert_eq!(como_texto(&Json::Bool(false)).unwrap(), "0");
        assert_eq!(
            super::super::dialeto::booleano_lido(&como_texto(&Json::Bool(true)).unwrap()),
            Some(true)
        );
        // NULO e ausencia, e nao cadeia vazia.
        assert!(como_texto(&Json::Nulo).is_none());
        assert_eq!(como_texto(&Json::texto_de("")).unwrap(), "");
        // Inteiro nao ganha `.0`.
        assert_eq!(como_texto(&Json::de_u64(7)).unwrap(), "7");
    }

    /// A ordem das colunas sai do ESQUEMA, e a linha e projetada nela. Uma
    /// linha que venha sem um campo produz NULO naquela celula, e nao um
    /// deslocamento de todas as seguintes.
    #[test]
    fn a_linha_e_projetada_na_ordem_do_esquema() {
        let colunas = colunas_da_grade(&esquema_de_prova());
        let nomes: Vec<&str> = colunas.iter().map(|c| c.nome.as_str()).collect();
        assert_eq!(nomes, ["rowid", "id", "cidade", "softdeleted"]);
        // A linha chega com os campos FORA de ordem, que e o que um objeto
        // JSON permite -- e e por isso que a ordem nao pode sair dela.
        let linha = Json::analisar(r#"[{"cidade":"Blumenau","rowid":3,"id":9}]"#).unwrap();
        let (linhas, _) = linhas_por_colunas(linha.lista().unwrap(), &colunas, 10);
        assert_eq!(
            linhas[0],
            vec![
                Some("3".into()),
                Some("9".into()),
                Some("Blumenau".into()),
                None
            ]
        );
    }

    /// `SELECT COUNT(*)` nao tem linha, e a linha que vem junto e efeito do
    /// caminho -- nao a resposta. O proprio servidor ja pagou este defeito.
    #[test]
    fn a_contagem_nao_devolve_linha_de_dado() {
        let r = Json::analisar(r#"{"sql":"SELECT COUNT(*) FROM c","contagem":3,"registros":3}"#)
            .unwrap();
        let res = resultado_do_sql(&r, 100);
        assert_eq!(res.colunas.len(), 1);
        assert_eq!(res.colunas[0].nome, "contagem");
        assert_eq!(res.linhas, vec![vec![Some("3".to_string())]]);
    }

    /// Sem `colunas` na resposta (a linha inteira), a ordem sai das chaves da
    /// primeira linha -- que e o unico lugar onde ela existe.
    #[test]
    fn a_projecao_do_sql_manda_quando_ela_existe() {
        let r = Json::analisar(r#"{"colunas":["nome"],"linhas":[{"id":1,"nome":"Ana"}]}"#).unwrap();
        let res = resultado_do_sql(&r, 100);
        assert_eq!(res.colunas.len(), 1);
        assert_eq!(res.celula(0, 0).unwrap(), "Ana");

        let inteira = Json::analisar(r#"{"linhas":[{"id":1,"nome":"Ana"}]}"#).unwrap();
        let res = resultado_do_sql(&inteira, 100);
        let nomes: Vec<&str> = res.colunas.iter().map(|c| c.nome.as_str()).collect();
        assert_eq!(nomes, ["id", "nome"]);
    }

    /// O teto corta AQUI e a resposta diz que cortou -- o mesmo contrato dos
    /// outros dois clientes.
    #[test]
    fn o_teto_corta_e_a_resposta_avisa() {
        let r = Json::analisar(r#"{"linhas":[{"i":1},{"i":2},{"i":3}]}"#).unwrap();
        let res = resultado_do_sql(&r, 2);
        assert_eq!(res.linhas.len(), 2);
        assert!(res.truncado);
    }
}
