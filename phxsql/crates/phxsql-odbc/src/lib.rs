//! Driver ODBC 3.x do PhxSql.
//!
//! Uma `cdylib` de ABI C que o gerenciador de driver (unixODBC no Linux, o
//! Driver Manager do Windows) carrega por dlopen/LoadLibrary. Por dentro ela
//! e um cliente comum da porta de dados: TCP, uma linha JSON por pedido.
//!
//! O recorte e o nucleo que um consumidor real usa para LER: conectar
//! (DSN-less), `SQLExecDirect` de um SELECT, instrucao preparada com
//! parametros (`?`), descrever colunas com tipo honesto, fetch e SQLGetData
//! com truncamento avisado, diagnostico. O que ficou de fora e por que esta
//! em `docs/ODBC.md`.
//!
//! Sao as funcoes ANSI (`SQLDriverConnect`, nao `...W`): o gerenciador de
//! driver converte as chamadas wide do aplicativo para elas sozinho. O texto
//! e UTF-8 dos dois lados, que e o que o servidor fala.
//!
//! Regra herdada da casa e que aqui vira contrato de ABI: nenhum caminho de
//! erro escreve senha ou token em diagnostico -- ha teste para isso.

mod conexao;
mod parametro;
mod registro;
mod resultado;
mod texto;
mod tipos;

use conexao::{analisar_receita, receita_mascarada, Canal, Falha, Receita};
use phxsql_core::json::Json;
use registro::{Amarra, Comando, Diag, Ligacao, Punho};
use resultado::{alvo_do_from, fichas_do_esquema, montar, Ficha};
use std::sync::{Arc, Mutex};
use texto::{escrever_texto, ler_texto};
use tipos::*;

/// Celula ja entregue por inteiro ao aplicativo: a proxima SQLGetData da
/// mesma celula responde SQL_NO_DATA, como manda a especificacao.
const ENTREGUE: usize = usize::MAX;

/// Toda entrada da ABI passa por aqui: panico interno nao pode atravessar a
/// fronteira C (abortaria o aplicativo do usuario); vira SQL_ERROR.
fn blindado(f: impl FnOnce() -> SqlReturn) -> SqlReturn {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or(SQL_ERROR)
}

fn diag_de(p: &mut Punho) -> &mut Vec<Diag> {
    match p {
        Punho::Ambiente(a) => &mut a.diag,
        Punho::Ligacao(l) => &mut l.diag,
        Punho::Comando(c) => &mut c.diag,
    }
}

/// Registra um diagnostico no handle. A mensagem leva o prefixo de praxe dos
/// drivers ODBC, que e como o aplicativo descobre QUEM falou.
fn anotar(id: usize, estado: &str, mensagem: &str) {
    anotar_nativo(id, estado, mensagem, 0);
}

fn anotar_nativo(id: usize, estado: &str, mensagem: &str, nativo: i32) {
    registro::com(id, |p| {
        diag_de(p).push(Diag {
            estado: estado.to_string(),
            mensagem: format!("[PhxSql][ODBC] {mensagem}"),
            nativo,
        });
    });
}

fn limpar_diag(id: usize) -> bool {
    registro::com(id, |p| diag_de(p).clear()).is_some()
}

/// Escreve um inteiro no buffer do aplicativo, se ele deu buffer.
///
/// # Safety
///
/// `ptr` nulo e aceito; fora isso precisa apontar para memoria valida.
unsafe fn escrever_num<T>(ptr: *mut T, valor: T) {
    if !ptr.is_null() {
        std::ptr::write_unaligned(ptr, valor);
    }
}

/// Entrega uma celula no buffer do aplicativo, a partir do byte `ja`.
///
/// Devolve `(codigo, novo_ja, diagnostico)`. E o unico caminho de dados do
/// driver: SQLGetData chama com continuacao, SQLFetch chama com `ja = 0`
/// para cada coluna amarrada.
unsafe fn entregar(
    celula: &Option<String>,
    ja: usize,
    tipo_c: SqlSmallint,
    buf: SqlPointer,
    cap: SqlLen,
    indicador: *mut SqlLen,
) -> (SqlReturn, usize, Option<(&'static str, String)>) {
    if ja == ENTREGUE {
        return (SQL_NO_DATA, ENTREGUE, None);
    }
    let Some(texto_celula) = celula else {
        // NULL so se conta pelo indicador; sem ele o aplicativo nao teria
        // como saber, e a especificacao manda recusar (22002).
        if indicador.is_null() {
            return (
                SQL_ERROR,
                ja,
                Some(("22002", "valor NULL exige o ponteiro indicador".into())),
            );
        }
        escrever_num(indicador, SQL_NULL_DATA);
        return (SQL_SUCCESS, ENTREGUE, None);
    };

    match tipo_c {
        SQL_C_CHAR | SQL_C_DEFAULT => {
            let dados = texto_celula.as_bytes();
            let restante = &dados[ja.min(dados.len())..];
            if buf.is_null() {
                return (
                    SQL_ERROR,
                    ja,
                    Some(("HY009", "buffer nulo no SQLGetData".into())),
                );
            }
            if cap <= 0 {
                return (
                    SQL_ERROR,
                    ja,
                    Some(("HY090", "tamanho de buffer invalido".into())),
                );
            }
            let (n, truncou) = escrever_texto(restante, buf as *mut SqlChar, cap);
            // O indicador leva o que havia ANTES desta chamada: e assim que o
            // aplicativo dimensiona o proximo pedaco.
            escrever_num(indicador, restante.len() as SqlLen);
            if truncou {
                (
                    SQL_SUCCESS_WITH_INFO,
                    ja + n,
                    Some((
                        "01004",
                        "texto truncado; o resto vem na proxima chamada".into(),
                    )),
                )
            } else {
                (SQL_SUCCESS, ENTREGUE, None)
            }
        }
        SQL_C_SLONG | SQL_C_LONG | SQL_C_SSHORT | SQL_C_SHORT | SQL_C_SBIGINT => {
            let n: i64 = match texto_celula.trim().parse() {
                Ok(n) => n,
                Err(_) => {
                    let mostra: String = texto_celula.chars().take(40).collect();
                    return (
                        SQL_ERROR,
                        ja,
                        Some(("22018", format!("{mostra:?} nao e um inteiro"))),
                    );
                }
            };
            let (cabe, largura) = match tipo_c {
                SQL_C_SSHORT | SQL_C_SHORT => {
                    (i64::from(i16::MIN) <= n && n <= i64::from(i16::MAX), 2)
                }
                SQL_C_SLONG | SQL_C_LONG => {
                    (i64::from(i32::MIN) <= n && n <= i64::from(i32::MAX), 4)
                }
                _ => (true, 8),
            };
            if !cabe {
                return (
                    SQL_ERROR,
                    ja,
                    Some(("22003", format!("{n} nao cabe no tipo C pedido"))),
                );
            }
            match largura {
                2 => escrever_num(buf as *mut i16, n as i16),
                4 => escrever_num(buf as *mut i32, n as i32),
                _ => escrever_num(buf as *mut i64, n),
            }
            escrever_num(indicador, largura as SqlLen);
            (SQL_SUCCESS, ENTREGUE, None)
        }
        SQL_C_DOUBLE | SQL_C_FLOAT => {
            let n: f64 = match texto_celula.trim().parse() {
                Ok(n) => n,
                Err(_) => {
                    let mostra: String = texto_celula.chars().take(40).collect();
                    return (
                        SQL_ERROR,
                        ja,
                        Some(("22018", format!("{mostra:?} nao e um numero"))),
                    );
                }
            };
            if tipo_c == SQL_C_DOUBLE {
                escrever_num(buf as *mut f64, n);
                escrever_num(indicador, 8);
            } else {
                escrever_num(buf as *mut f32, n as f32);
                escrever_num(indicador, 4);
            }
            (SQL_SUCCESS, ENTREGUE, None)
        }
        outro => (
            SQL_ERROR,
            ja,
            Some((
                "HY003",
                format!("tipo C {outro} nao suportado por este driver"),
            )),
        ),
    }
}

/// Abre a conexao a partir de uma receita ja montada -- o caminho comum de
/// SQLDriverConnect e SQLConnect. A rede roda FORA da trava do registro.
fn conectar(id_dbc: usize, receita: &Receita) -> SqlReturn {
    let estado = registro::com(id_dbc, |p| match p {
        Punho::Ligacao(l) => Some(l.canal.is_some()),
        _ => None,
    });
    match estado {
        None | Some(None) => return SQL_INVALID_HANDLE,
        Some(Some(true)) => {
            anotar(id_dbc, "08002", "esta conexao ja esta aberta");
            return SQL_ERROR;
        }
        Some(Some(false)) => {}
    }
    match Canal::abrir(receita) {
        Ok(canal) => {
            let arco = Arc::new(Mutex::new(canal));
            let gravou = registro::com(id_dbc, |p| {
                if let Punho::Ligacao(l) = p {
                    l.canal = Some(arco.clone());
                    l.database = receita.database.clone();
                    l.servidor = format!("{}:{}", receita.servidor, receita.porta);
                    l.usuario = receita.usuario.clone();
                }
            });
            if gravou.is_none() {
                return SQL_INVALID_HANDLE;
            }
            SQL_SUCCESS
        }
        Err(f) => {
            anotar_nativo(id_dbc, f.estado, &f.mensagem, f.nativo);
            SQL_ERROR
        }
    }
}

/// Aloca um handle de ambiente, conexao ou comando.
///
/// # Safety
///
/// Contrato da ABI do ODBC: `saida` aponta para um `SQLHANDLE` gravavel.
#[no_mangle]
pub unsafe extern "system" fn SQLAllocHandle(
    tipo: SqlSmallint,
    pai: SqlHandle,
    saida: *mut SqlHandle,
) -> SqlReturn {
    blindado(|| {
        if saida.is_null() {
            return SQL_ERROR;
        }
        escrever_num(saida, std::ptr::null_mut());
        let novo = match tipo {
            SQL_HANDLE_ENV => Punho::Ambiente(Default::default()),
            SQL_HANDLE_DBC => {
                let pai_ok =
                    registro::com(registro::id_de(pai), |p| matches!(p, Punho::Ambiente(_)));
                if pai_ok != Some(true) {
                    return SQL_INVALID_HANDLE;
                }
                Punho::Ligacao(Ligacao {
                    canal: None,
                    database: String::new(),
                    servidor: String::new(),
                    usuario: String::new(),
                    diag: Vec::new(),
                })
            }
            SQL_HANDLE_STMT => {
                let id_dbc = registro::id_de(pai);
                match registro::com(id_dbc, |p| match p {
                    Punho::Ligacao(l) => Some(l.canal.is_some()),
                    _ => None,
                }) {
                    None | Some(None) => return SQL_INVALID_HANDLE,
                    Some(Some(false)) => {
                        anotar(id_dbc, "08003", "conecte antes de alocar um comando");
                        return SQL_ERROR;
                    }
                    Some(Some(true)) => {}
                }
                Punho::Comando(Comando {
                    dono: id_dbc,
                    ..Default::default()
                })
            }
            _ => return SQL_ERROR,
        };
        escrever_num(saida, registro::como_handle(registro::criar(novo)));
        SQL_SUCCESS
    })
}

/// Libera um handle. Conexao ainda aberta nao se libera: desconecte antes.
///
/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLFreeHandle(tipo: SqlSmallint, h: SqlHandle) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(h);
        let confere = registro::com(id, |p| match (tipo, &*p) {
            (SQL_HANDLE_ENV, Punho::Ambiente(_)) => Some(true),
            (SQL_HANDLE_DBC, Punho::Ligacao(l)) => Some(l.canal.is_none()),
            (SQL_HANDLE_STMT, Punho::Comando(_)) => Some(true),
            _ => None,
        });
        match confere {
            None | Some(None) => SQL_INVALID_HANDLE,
            Some(Some(false)) => {
                anotar(
                    id,
                    "HY010",
                    "desconecte (SQLDisconnect) antes de liberar a conexao",
                );
                SQL_ERROR
            }
            Some(Some(true)) => {
                registro::remover(id);
                SQL_SUCCESS
            }
        }
    })
}

/// Atributos de ambiente. So a versao do ODBC importa para este driver; o
/// resto e aceito com aviso, porque recusar derrubaria aplicativo que so
/// queria um detalhe de pool.
///
/// # Safety
///
/// Contrato da ABI do ODBC: `valor` carrega um inteiro no lugar do ponteiro.
#[no_mangle]
pub unsafe extern "system" fn SQLSetEnvAttr(
    env: SqlHandle,
    atributo: SqlInteger,
    valor: SqlPointer,
    _tamanho: SqlInteger,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(env);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        if atributo == SQL_ATTR_ODBC_VERSION {
            let versao = valor as usize as i32;
            let ok = registro::com(id, |p| {
                if let Punho::Ambiente(a) = p {
                    a.versao_odbc = versao;
                    true
                } else {
                    false
                }
            });
            return if ok == Some(true) {
                SQL_SUCCESS
            } else {
                SQL_INVALID_HANDLE
            };
        }
        anotar(
            id,
            "01S02",
            &format!("atributo de ambiente {atributo} ignorado"),
        );
        SQL_SUCCESS_WITH_INFO
    })
}

/// Conexao DSN-less:
/// `Driver=PhxSql;Server=host;Port=5000;Token=...;UID=...;PWD=...;Database=...`.
///
/// A string devolvida sai com senha e token mascarados -- ela costuma parar
/// em arquivo de configuracao do aplicativo, e o driver nao decide onde.
///
/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
#[allow(clippy::too_many_arguments)] // a assinatura e da especificacao
pub unsafe extern "system" fn SQLDriverConnect(
    dbc: SqlHandle,
    _janela: SqlHandle,
    entrada: *const SqlChar,
    tamanho_entrada: SqlSmallint,
    saida: *mut SqlChar,
    capacidade_saida: SqlSmallint,
    tamanho_saida: *mut SqlSmallint,
    _completar: SqlUSmallint,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(dbc);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        let receita = analisar_receita(&ler_texto(entrada, SqlInteger::from(tamanho_entrada)));
        let codigo = conectar(id, &receita);
        if codigo != SQL_SUCCESS {
            return codigo;
        }
        let volta = receita_mascarada(&receita);
        let (n, truncou) = escrever_texto(volta.as_bytes(), saida, capacidade_saida as SqlLen);
        escrever_num(tamanho_saida, n as SqlSmallint);
        if truncou && !saida.is_null() {
            anotar(id, "01004", "a connection string de volta foi truncada");
            return SQL_SUCCESS_WITH_INFO;
        }
        SQL_SUCCESS
    })
}

/// O caminho com DSN do `SQLConnect`, sem ler odbc.ini: o nome do servidor
/// aceita `host:porta/database` ou uma connection string inteira. O token,
/// quando o servidor exigir, so entra pela connection string -- este driver
/// nao le arquivo de DSN (esta em docs/ODBC.md, com o motivo).
///
/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLConnect(
    dbc: SqlHandle,
    servidor: *const SqlChar,
    tamanho_servidor: SqlSmallint,
    usuario: *const SqlChar,
    tamanho_usuario: SqlSmallint,
    senha: *const SqlChar,
    tamanho_senha: SqlSmallint,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(dbc);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        let nome = ler_texto(servidor, SqlInteger::from(tamanho_servidor));
        let mut receita = if nome.contains('=') {
            analisar_receita(&nome)
        } else {
            let (endereco, database) = nome.split_once('/').unwrap_or((nome.as_str(), ""));
            let (host, porta) = endereco.split_once(':').unwrap_or((endereco, "5000"));
            Receita {
                servidor: host.trim().to_string(),
                porta: porta.trim().parse().unwrap_or(0),
                database: database.trim().to_string(),
                ..Receita::default()
            }
        };
        let u = ler_texto(usuario, SqlInteger::from(tamanho_usuario));
        let s = ler_texto(senha, SqlInteger::from(tamanho_senha));
        if !u.is_empty() {
            receita.usuario = u;
        }
        if !s.is_empty() {
            receita.senha = s;
        }
        conectar(id, &receita)
    })
}

/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLDisconnect(dbc: SqlHandle) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(dbc);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        let resultado = registro::com(id, |p| match p {
            Punho::Ligacao(l) => Some(l.canal.take().is_some()),
            _ => None,
        });
        match resultado {
            None | Some(None) => SQL_INVALID_HANDLE,
            Some(Some(false)) => {
                anotar(id, "08003", "esta conexao nao estava aberta");
                SQL_ERROR
            }
            Some(Some(true)) => SQL_SUCCESS,
        }
    })
}

/// A lista `parametros` do pedido, ou a recusa com o SQLSTATE ja escolhido.
///
/// A ordem e a das POSICOES do texto (o primeiro `?` e o 1), e nao a das
/// chamadas de `SQLBindParameter`: o aplicativo tem o direito de ligar de tras
/// para a frente, e nenhum driver depende dessa ordem.
///
/// Ligacao que sobra alem do numero de `?` e ignorada, como manda a
/// especificacao -- quem preparou uma instrucao de dois parametros e depois
/// uma de um nao precisa desligar nada.
///
/// # Safety
///
/// So se chama de dentro da execucao: e la que os ponteiros ligados valem.
unsafe fn montar_parametros(
    sql: &str,
    ligacoes: &[registro::Parametro],
) -> Result<Vec<Json>, (&'static str, String)> {
    let quantos = parametro::contar_interrogacoes(sql);
    if quantos == 0 {
        return Ok(Vec::new());
    }
    let mut saida = Vec::with_capacity(quantos);
    for posicao in 1..=quantos {
        let Some(ligacao) = ligacoes.iter().find(|q| usize::from(q.numero) == posicao) else {
            // 07002 e literalmente "COUNT field incorrect": ligaram menos
            // parametros do que a instrucao tem. A mensagem diz a POSICAO e os
            // dois numeros, porque "faltou parametro" nao conserta codigo.
            return Err((
                "07002",
                format!(
                    "o `?` da posicao {posicao} nao tem valor ligado: o texto tem {quantos} \
                     parametro(s) e o SQLBindParameter cobriu {}",
                    ligacoes.len()
                ),
            ));
        };
        saida.push(parametro::ler(ligacao)?);
    }
    Ok(saida)
}

/// O miolo comum de SQLExecDirect e SQLExecute: manda o texto INTEIRO para o
/// servidor -- o parser mora la, e o erro dele volta com a coluna do
/// problema. O driver so olha o FROM para pedir o esquema, que e de onde
/// saem os tipos honestos.
fn executar_sql(id: usize, sql: String) -> SqlReturn {
    {
        // As ligacoes saem do registro por COPIA para os ponteiros serem lidos
        // FORA da trava: ler memoria do aplicativo com a trava do registro na
        // mao penduraria todos os outros handles do processo num ponteiro
        // torto alheio.
        let dono = registro::com(id, |p| match p {
            Punho::Comando(c) => {
                c.resultado = None;
                c.cursor = 0;
                c.entregues.clear();
                Some((c.dono, c.parametros.clone()))
            }
            _ => None,
        });
        let Some(Some((dono, ligacoes))) = dono else {
            return SQL_INVALID_HANDLE;
        };

        // Os `?` e as ligacoes se conferem ANTES de tocar a rede. Mandar um
        // pedido que o servidor vai recusar gasta uma ida e volta e devolve o
        // erro DELE, que nao sabe qual posicao ficou sem valor -- e essa e a
        // unica informacao que conserta o programa de quem chamou.
        //
        // SAFETY: e a janela da execucao, o unico ponto em que o contrato da
        // ABI promete que os ponteiros ligados ainda valem.
        let parametros = match unsafe { montar_parametros(&sql, &ligacoes) } {
            Ok(v) => v,
            Err((estado, mensagem)) => {
                anotar(id, estado, &mensagem);
                return SQL_ERROR;
            }
        };
        let ligacao = registro::com(dono, |p| match p {
            Punho::Ligacao(l) => Some((l.canal.clone(), l.database.clone())),
            _ => None,
        });
        let Some(Some((Some(canal), database))) = ligacao else {
            anotar(id, "08003", "a conexao deste comando ja fechou");
            return SQL_ERROR;
        };

        // A rede inteira acontece aqui, sem a trava do registro: outro handle
        // continua livre enquanto este espera o servidor.
        let conversa = (|| -> Result<(Json, Vec<Ficha>), Falha> {
            let mut canal = canal
                .lock()
                .map_err(|_| Falha::nova("HY000", "o canal desta conexao esta envenenado"))?;
            let mut campos = vec![
                ("op", Json::texto_de("sql")),
                ("database", Json::texto_de(&database)),
                ("texto", Json::texto_de(&sql)),
            ];
            // Sem `?` o campo NAO vai: o pedido de quem nunca ligou parametro
            // sai byte a byte como sempre saiu. Guarda nova entra pedida, nao
            // imposta -- e o teste que trava isso e o do comportamento velho.
            if !parametros.is_empty() {
                campos.push(("parametros", Json::Lista(parametros)));
            }
            let resposta = canal.pedir(campos)?;
            // COUNT(*) nao precisa de esquema; o resto ganha tipos se o
            // esquema responder. Falha aqui NAO derruba a consulta: a
            // resposta ja veio, e texto sem tipo e melhor que nada.
            let mut fichas = Vec::new();
            if resposta.campo("contagem").is_none() {
                if let Some((db_from, tabela)) = alvo_do_from(&sql) {
                    let db = if db_from.is_empty() {
                        database
                    } else {
                        db_from
                    };
                    if let Ok(esquema) = canal.pedir(vec![
                        ("op", Json::texto_de("esquema")),
                        ("database", Json::texto_de(&db)),
                        ("tabela", Json::texto_de(&tabela)),
                    ]) {
                        fichas = fichas_do_esquema(&esquema);
                    }
                }
            }
            Ok((resposta, fichas))
        })();

        match conversa {
            Ok((resposta, fichas)) => {
                let sem_tipos = fichas.is_empty() && resposta.campo("contagem").is_none();
                let pronto = montar(&resposta, &fichas);
                let guardou = registro::com(id, |p| {
                    if let Punho::Comando(c) = p {
                        c.resultado = Some(pronto);
                        c.cursor = 0;
                        c.entregues.clear();
                    }
                });
                if guardou.is_none() {
                    return SQL_INVALID_HANDLE;
                }
                if sem_tipos {
                    anotar(
                        id,
                        "01000",
                        "esquema indisponivel para esta consulta; colunas declaradas como texto",
                    );
                    return SQL_SUCCESS_WITH_INFO;
                }
                SQL_SUCCESS
            }
            Err(f) => {
                // O SQLSTATE ja vem decidido pelo canal, a partir do erro
                // ESTRUTURADO do servidor (nome e codigo) -- 42S02 para
                // tabela que nao existe, 42000 para sintaxe.
                anotar_nativo(id, f.estado, &f.mensagem, f.nativo);
                SQL_ERROR
            }
        }
    }
}

/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLExecDirect(
    stmt: SqlHandle,
    texto: *const SqlChar,
    tamanho: SqlInteger,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        let sql = ler_texto(texto, tamanho);
        executar_sql(id, sql)
    })
}

/// Preparar aqui e guardar o TEXTO: o PLANO continua sendo do servidor, que
/// analisa na execucao.
///
/// O que este comentario dizia ate a rodada dos parametros -- «nao ha
/// parametros nem plano no driver» -- deixou de ser verdade na primeira
/// metade, e o `docs/COMPARATIVO.md` citava esta linha como prova da
/// ausencia. Hoje o texto guardado e a FONTE da contagem de `?` do
/// `SQLNumParams` e do `SQLDescribeParam`, e as ligacoes do
/// `SQLBindParameter` se leem no `SQLExecute` que vier depois.
///
/// Preparar NAO desfaz ligacao, e isso nao e esquecimento: a especificacao
/// manda o contrario, e o laco de carga depende disso -- prepara uma vez, liga
/// uma vez, executa mil trocando so o buffer. Ligacao que sobra alem do numero
/// de `?` e ignorada; quem quer zerar chama `SQLFreeStmt(SQL_RESET_PARAMS)`.
///
/// Existe porque o isql e outros clientes so falam prepare/execute -- sem ele,
/// "Connected!" e a ultima coisa que funciona.
///
/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLPrepare(
    stmt: SqlHandle,
    texto: *const SqlChar,
    tamanho: SqlInteger,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        let sql = ler_texto(texto, tamanho);
        let ok = registro::com(id, |p| match p {
            Punho::Comando(c) => {
                c.preparado = Some(sql.clone());
                c.resultado = None;
                c.cursor = 0;
                c.entregues.clear();
                true
            }
            _ => false,
        });
        if ok == Some(true) {
            SQL_SUCCESS
        } else {
            SQL_INVALID_HANDLE
        }
    })
}

/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLExecute(stmt: SqlHandle) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        let sql = registro::com(id, |p| match p {
            Punho::Comando(c) => Some(c.preparado.clone()),
            _ => None,
        });
        match sql {
            None | Some(None) => SQL_INVALID_HANDLE,
            Some(Some(None)) => {
                anotar(id, "HY010", "SQLExecute sem um SQLPrepare antes");
                SQL_ERROR
            }
            Some(Some(Some(sql))) => executar_sql(id, sql),
        }
    })
}

/// Liga um valor de entrada ao `?` da posicao `numero`.
///
/// O que se guarda e o ENDERECO, e isso e o contrato do ODBC e nao uma
/// economia: a especificacao diz que o valor se le na EXECUCAO. E o que
/// permite ligar uma vez e executar mil, trocando so o conteudo do buffer
/// entre uma execucao e a proxima -- o laco de qualquer ferramenta de carga.
/// Ler aqui mandaria o primeiro valor em todas as execucoes, sem erro nenhum.
///
/// A contrapartida de seguranca, dita para ficar preso: **o driver so
/// desreferencia estes ponteiros dentro de `SQLExecute`/`SQLExecDirect`**, que
/// e a unica janela em que o contrato da ABI promete que eles ainda valem.
/// Nada mais no driver os toca -- nem o diagnostico, nem o desmonte.
///
/// Quatro argumentos entram e nao sao usados, e cada um por um motivo:
/// `tipo_sql` e `casas` seriam uma promessa de tipo que o driver nao tem como
/// cumprir (ele nao planeja nada na preparacao, e quem coage o literal ao tipo
/// da coluna e o servidor); `tamanho_coluna` idem; e `tamanho_buffer` a
/// especificacao manda ignorar em parametro de ENTRADA de tipo caractere --
/// quem diz quantos bytes valem e o indicador.
///
/// # Safety
///
/// Contrato da ABI do ODBC: os ponteiros ligados precisam continuar validos
/// ate a execucao, como em todo driver ODBC.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "system" fn SQLBindParameter(
    stmt: SqlHandle,
    numero: SqlUSmallint,
    tipo_io: SqlSmallint,
    tipo_c: SqlSmallint,
    _tipo_sql: SqlSmallint,
    _tamanho_coluna: SqlULen,
    _casas: SqlSmallint,
    valor: SqlPointer,
    _tamanho_buffer: SqlLen,
    indicador: *mut SqlLen,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        if numero == 0 {
            anotar(id, "07009", "a posicao de parametro comeca em 1, nao em 0");
            return SQL_ERROR;
        }
        if tipo_io != SQL_PARAM_INPUT {
            let como = match tipo_io {
                SQL_PARAM_OUTPUT => "de saida",
                SQL_PARAM_INPUT_OUTPUT => "de entrada e saida",
                SQL_PARAM_TYPE_UNKNOWN => "de sentido desconhecido",
                _ => "de sentido invalido",
            };
            anotar(
                id,
                "HYC00",
                &format!(
                    "parametro {numero} {como}: este driver so tem SQL_PARAM_INPUT. \
                     Saida exigiria o servidor devolver valor por posicao, e a op sql \
                     devolve linhas"
                ),
            );
            return SQL_ERROR;
        }
        if let Some(motivo) = parametro::recusa_do_tipo_c(tipo_c) {
            anotar(id, "HYC00", &format!("parametro {numero}: {motivo}"));
            return SQL_ERROR;
        }
        // Ponteiro de valor nulo SEM indicador nao pode significar nada: e no
        // indicador que o SQL_NULL_DATA se escreve. Recusar aqui e melhor que
        // guardar uma ligacao que so falharia na execucao.
        if valor.is_null() && indicador.is_null() {
            anotar(
                id,
                "HY009",
                &format!(
                    "parametro {numero} ligado a ponteiro nulo sem indicador: \
                     NULL se manda pelo indicador (SQL_NULL_DATA)"
                ),
            );
            return SQL_ERROR;
        }
        let ok = registro::com(id, |p| match p {
            Punho::Comando(c) => {
                // Religar a mesma posicao SUBSTITUI, como no SQLBindCol: duas
                // ligacoes vivas para o mesmo `?` deixariam a ordem do vetor
                // decidir qual vale.
                c.parametros.retain(|q| q.numero != numero);
                c.parametros.push(registro::Parametro {
                    numero,
                    tipo_c,
                    buf: valor as usize,
                    indicador: indicador as usize,
                });
                true
            }
            _ => false,
        });
        if ok == Some(true) {
            SQL_SUCCESS
        } else {
            SQL_INVALID_HANDLE
        }
    })
}

/// Quantos `?` a instrucao preparada tem.
///
/// A conta sai do TEXTO guardado pelo `SQLPrepare`, refeita a cada chamada em
/// vez de guardada: numero guardado envelhece calado no dia em que alguem
/// preparar outra coisa no mesmo comando, e refazer custa uma varredura de
/// uma string curta.
///
/// Sem `SQLPrepare` antes e `HY010` (erro de sequencia), como manda a
/// especificacao -- e nao zero, que o aplicativo leria como "esta instrucao
/// nao tem parametros".
///
/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLNumParams(stmt: SqlHandle, saida: *mut SqlSmallint) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        let texto = registro::com(id, |p| match p {
            Punho::Comando(c) => Some(c.preparado.clone()),
            _ => None,
        });
        match texto {
            None | Some(None) => SQL_INVALID_HANDLE,
            Some(Some(None)) => {
                anotar(id, "HY010", "SQLNumParams sem um SQLPrepare antes");
                SQL_ERROR
            }
            Some(Some(Some(sql))) => {
                escrever_num(saida, parametro::contar_interrogacoes(&sql) as SqlSmallint);
                SQL_SUCCESS
            }
        }
    })
}

/// Descreve o `?` da posicao `numero` -- e declara TEXTO, sempre.
///
/// Nao e preguica, e o que o driver tem: ele nao planeja nada na preparacao
/// (o parser mora no servidor, e o texto so vai para la na execucao), entao
/// ele nao sabe a que coluna cada `?` se compara nem qual o tipo dela. Um
/// `SQL_INTEGER` chutado aqui seria a mesma mentira que o driver ja recusou
/// contar sobre apelido de coluna (docs/ODBC.md, secao 8): tipo declarado sem
/// esquema por tras.
///
/// `SQL_VARCHAR` nao estreita nada: o `SQLBindParameter` continua aceitando
/// SQL_C_SLONG, SQL_C_DOUBLE e os outros, e quem coage o literal ao tipo da
/// coluna e o servidor, na gravacao.
///
/// O tamanho volta ZERO, que aqui quer dizer "nao sei" -- e nao ha risco de
/// buffer nisso: o driver nunca ESCREVE num buffer de parametro, porque
/// parametro de saida e recusado na ligacao.
///
/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLDescribeParam(
    stmt: SqlHandle,
    numero: SqlUSmallint,
    tipo: *mut SqlSmallint,
    tamanho: *mut SqlULen,
    casas: *mut SqlSmallint,
    nulavel: *mut SqlSmallint,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        let texto = registro::com(id, |p| match p {
            Punho::Comando(c) => Some(c.preparado.clone()),
            _ => None,
        });
        let quantos = match texto {
            None | Some(None) => return SQL_INVALID_HANDLE,
            Some(Some(None)) => {
                anotar(id, "HY010", "SQLDescribeParam sem um SQLPrepare antes");
                return SQL_ERROR;
            }
            Some(Some(Some(sql))) => parametro::contar_interrogacoes(&sql),
        };
        if numero == 0 || usize::from(numero) > quantos {
            anotar(
                id,
                "07009",
                &format!("nao ha parametro {numero}: esta instrucao tem {quantos}"),
            );
            return SQL_ERROR;
        }
        escrever_num(tipo, SQL_VARCHAR);
        escrever_num(tamanho, 0usize as SqlULen);
        escrever_num(casas, 0);
        // NULAVEL e a verdade, e nao o "desconhecido" de praxe: qualquer `?`
        // deste driver aceita NULL pelo indicador (SQL_NULL_DATA), sem
        // excecao.
        escrever_num(nulavel, SQL_NULLABLE);
        SQL_SUCCESS
    })
}

/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLNumResultCols(
    stmt: SqlHandle,
    saida: *mut SqlSmallint,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        let n = registro::com(id, |p| match p {
            Punho::Comando(c) => Some(
                c.resultado
                    .as_ref()
                    .map(|r| r.colunas.len() as SqlSmallint)
                    .unwrap_or(0),
            ),
            _ => None,
        });
        match n {
            None | Some(None) => SQL_INVALID_HANDLE,
            Some(Some(n)) => {
                escrever_num(saida, n);
                SQL_SUCCESS
            }
        }
    })
}

/// Quantas linhas o SELECT devolveu. Num driver que recebe o conjunto
/// inteiro numa resposta, o numero e exato -- nao ha o -1 de "nao sei".
///
/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLRowCount(stmt: SqlHandle, saida: *mut SqlLen) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        let n = registro::com(id, |p| match p {
            Punho::Comando(c) => Some(
                c.resultado
                    .as_ref()
                    .map(|r| r.linhas.len() as SqlLen)
                    .unwrap_or(0),
            ),
            _ => None,
        });
        match n {
            None | Some(None) => SQL_INVALID_HANDLE,
            Some(Some(n)) => {
                escrever_num(saida, n);
                SQL_SUCCESS
            }
        }
    })
}

/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
#[allow(clippy::too_many_arguments)] // a assinatura e da especificacao
pub unsafe extern "system" fn SQLDescribeCol(
    stmt: SqlHandle,
    coluna: SqlUSmallint,
    nome: *mut SqlChar,
    capacidade_nome: SqlSmallint,
    tamanho_nome: *mut SqlSmallint,
    tipo_sql: *mut SqlSmallint,
    tamanho_coluna: *mut SqlULen,
    casas_decimais: *mut SqlSmallint,
    nulavel: *mut SqlSmallint,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        let ficha = registro::com(id, |p| match p {
            Punho::Comando(c) => Some(
                c.resultado
                    .as_ref()
                    .and_then(|r| r.colunas.get((coluna as usize).wrapping_sub(1)).cloned()),
            ),
            _ => None,
        });
        match ficha {
            None | Some(None) => SQL_INVALID_HANDLE,
            Some(Some(None)) => {
                anotar(id, "07009", &format!("nao ha coluna {coluna} no resultado"));
                SQL_ERROR
            }
            Some(Some(Some(c))) => {
                let (_escritos, truncou) =
                    escrever_texto(c.nome.as_bytes(), nome, capacidade_nome as SqlLen);
                escrever_num(tamanho_nome, c.nome.len() as SqlSmallint);
                escrever_num(tipo_sql, c.tipo_sql);
                escrever_num(tamanho_coluna, c.tamanho);
                escrever_num(casas_decimais, c.decimais);
                escrever_num(nulavel, c.nulavel);
                if truncou {
                    anotar(id, "01004", "o nome da coluna foi truncado");
                    return SQL_SUCCESS_WITH_INFO;
                }
                SQL_SUCCESS
            }
        }
    })
}

/// Os atributos de coluna que as ferramentas de grade pedem. Cobre os pares
/// antigo e novo (SQL_COLUMN_* / SQL_DESC_*) porque ha cliente de cada epoca.
///
/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
#[allow(clippy::too_many_arguments)] // a assinatura e da especificacao
pub unsafe extern "system" fn SQLColAttribute(
    stmt: SqlHandle,
    coluna: SqlUSmallint,
    campo: SqlUSmallint,
    texto_saida: SqlPointer,
    capacidade: SqlSmallint,
    tamanho_saida: *mut SqlSmallint,
    numero_saida: *mut SqlLen,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        let ficha = registro::com(id, |p| match p {
            Punho::Comando(c) => Some(
                c.resultado
                    .as_ref()
                    .and_then(|r| r.colunas.get((coluna as usize).wrapping_sub(1)).cloned()),
            ),
            _ => None,
        });
        match ficha {
            None | Some(None) => SQL_INVALID_HANDLE,
            Some(Some(None)) => {
                anotar(id, "07009", &format!("nao ha coluna {coluna} no resultado"));
                SQL_ERROR
            }
            Some(Some(Some(c))) => match campo {
                SQL_COLUMN_NAME | SQL_DESC_NAME | SQL_DESC_LABEL => {
                    let (n, _) = escrever_texto(
                        c.nome.as_bytes(),
                        texto_saida as *mut SqlChar,
                        capacidade as SqlLen,
                    );
                    escrever_num(tamanho_saida, n as SqlSmallint);
                    SQL_SUCCESS
                }
                SQL_COLUMN_TYPE | SQL_DESC_TYPE => {
                    escrever_num(numero_saida, SqlLen::from(c.tipo_sql));
                    SQL_SUCCESS
                }
                SQL_COLUMN_LENGTH | SQL_DESC_LENGTH | SQL_DESC_DISPLAY_SIZE => {
                    escrever_num(numero_saida, c.tamanho as SqlLen);
                    SQL_SUCCESS
                }
                SQL_COLUMN_NULLABLE | SQL_DESC_NULLABLE => {
                    escrever_num(numero_saida, SqlLen::from(c.nulavel));
                    SQL_SUCCESS
                }
                outro => {
                    anotar(
                        id,
                        "HYC00",
                        &format!("atributo de coluna {outro} nao suportado"),
                    );
                    SQL_ERROR
                }
            },
        }
    })
}

/// Amarra uma coluna a um buffer, para o SQLFetch preencher. Buffer nulo
/// desamarra so aquela coluna.
///
/// # Safety
///
/// Contrato da ABI do ODBC: os ponteiros amarrados precisam continuar validos
/// ate o fim dos fetches -- e responsabilidade do aplicativo, como em todo
/// driver ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLBindCol(
    stmt: SqlHandle,
    coluna: SqlUSmallint,
    tipo_c: SqlSmallint,
    buf: SqlPointer,
    capacidade: SqlLen,
    indicador: *mut SqlLen,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        let ok = registro::com(id, |p| match p {
            Punho::Comando(c) => {
                c.amarras.retain(|a| a.coluna != coluna);
                if !buf.is_null() {
                    c.amarras.push(Amarra {
                        coluna,
                        tipo_c,
                        buf: buf as usize,
                        cap: capacidade,
                        indicador: indicador as usize,
                    });
                }
                true
            }
            _ => false,
        });
        if ok == Some(true) {
            SQL_SUCCESS
        } else {
            SQL_INVALID_HANDLE
        }
    })
}

/// Avanca uma linha e preenche as colunas amarradas.
///
/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLFetch(stmt: SqlHandle) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        // Clona a linha e as amarras para escrever nos buffers do aplicativo
        // sem segurar a trava com um emprestimo pendurado no registro.
        let quadro = registro::com(id, |p| match p {
            Punho::Comando(c) => {
                let Some(r) = c.resultado.as_ref() else {
                    return Err("HY010");
                };
                if c.cursor >= r.linhas.len() {
                    return Ok(None);
                }
                let linha = r.linhas[c.cursor].clone();
                c.cursor += 1;
                c.entregues = vec![0; r.colunas.len()];
                Ok(Some((linha, c.amarras.clone())))
            }
            _ => Err("punho"),
        });
        let quadro = match quadro {
            None => return SQL_INVALID_HANDLE,
            Some(Err("HY010")) => {
                anotar(id, "HY010", "SQLFetch antes de um SQLExecDirect");
                return SQL_ERROR;
            }
            Some(Err(_)) => return SQL_INVALID_HANDLE,
            Some(Ok(None)) => return SQL_NO_DATA,
            Some(Ok(Some(q))) => q,
        };

        let (linha, amarras) = quadro;
        let mut houve_info = false;
        let mut houve_erro = false;
        for a in &amarras {
            let Some(celula) = linha.get((a.coluna as usize).wrapping_sub(1)) else {
                anotar(
                    id,
                    "07009",
                    &format!("coluna amarrada {} nao existe", a.coluna),
                );
                houve_erro = true;
                continue;
            };
            let (codigo, _, diag) = entregar(
                celula,
                0,
                a.tipo_c,
                a.buf as SqlPointer,
                a.cap,
                a.indicador as *mut SqlLen,
            );
            if let Some((estado, mensagem)) = diag {
                anotar(id, estado, &format!("coluna {}: {mensagem}", a.coluna));
            }
            match codigo {
                SQL_SUCCESS_WITH_INFO => houve_info = true,
                SQL_ERROR => houve_erro = true,
                _ => {}
            }
        }
        if houve_erro {
            SQL_ERROR
        } else if houve_info {
            SQL_SUCCESS_WITH_INFO
        } else {
            SQL_SUCCESS
        }
    })
}

/// Le uma celula da linha atual, com continuacao: buffer menor que o texto
/// devolve SQL_SUCCESS_WITH_INFO (01004) e a proxima chamada continua de onde
/// parou, ate SQL_NO_DATA.
///
/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLGetData(
    stmt: SqlHandle,
    coluna: SqlUSmallint,
    tipo_c: SqlSmallint,
    buf: SqlPointer,
    capacidade: SqlLen,
    indicador: *mut SqlLen,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        let quadro = registro::com(id, |p| match p {
            Punho::Comando(c) => {
                let Some(r) = c.resultado.as_ref() else {
                    return Err("HY010");
                };
                if c.cursor == 0 || c.cursor > r.linhas.len() {
                    return Err("HY010");
                }
                let indice = (coluna as usize).wrapping_sub(1);
                if indice >= r.colunas.len() {
                    return Err("07009");
                }
                Ok((
                    r.linhas[c.cursor - 1][indice].clone(),
                    c.entregues.get(indice).copied().unwrap_or(0),
                ))
            }
            _ => Err("punho"),
        });
        let (celula, ja) = match quadro {
            None => return SQL_INVALID_HANDLE,
            Some(Err("HY010")) => {
                anotar(id, "HY010", "SQLGetData antes de um SQLFetch com linha");
                return SQL_ERROR;
            }
            Some(Err("07009")) => {
                anotar(id, "07009", &format!("nao ha coluna {coluna} no resultado"));
                return SQL_ERROR;
            }
            Some(Err(_)) => return SQL_INVALID_HANDLE,
            Some(Ok(q)) => q,
        };

        let (codigo, novo, diag) = entregar(&celula, ja, tipo_c, buf, capacidade, indicador);
        registro::com(id, |p| {
            if let Punho::Comando(c) = p {
                if let Some(e) = c.entregues.get_mut((coluna as usize).wrapping_sub(1)) {
                    *e = novo;
                }
            }
        });
        if let Some((estado, mensagem)) = diag {
            anotar(id, estado, &mensagem);
        }
        codigo
    })
}

/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLFreeStmt(stmt: SqlHandle, opcao: SqlUSmallint) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        // SQL_DROP (1) veio do ODBC 2.x; o gerenciador moderno traduz para
        // SQLFreeHandle, mas cliente antigo chama direto.
        if opcao == 1 {
            return if matches!(registro::remover(id), Some(Punho::Comando(_))) {
                SQL_SUCCESS
            } else {
                SQL_INVALID_HANDLE
            };
        }
        let ok = registro::com(id, |p| match p {
            Punho::Comando(c) => {
                match opcao {
                    SQL_CLOSE => {
                        c.resultado = None;
                        c.cursor = 0;
                        c.entregues.clear();
                    }
                    SQL_UNBIND => c.amarras.clear(),
                    // O comentario que estava aqui dizia "este driver nao tem
                    // parametros", e deixou de ser verdade: SQL_RESET_PARAMS
                    // desliga TODAS as ligacoes, que e o que a especificacao
                    // manda -- e o que um pool de conexao chama entre um
                    // usuario e o proximo.
                    SQL_RESET_PARAMS => c.parametros.clear(),
                    _ => {}
                }
                true
            }
            _ => false,
        });
        if ok == Some(true) {
            SQL_SUCCESS
        } else {
            SQL_INVALID_HANDLE
        }
    })
}

/// Le um registro de diagnostico. E a unica janela do aplicativo para o
/// motivo de um SQL_ERROR -- e a razao de nenhum caminho de erro poder
/// escrever senha aqui.
///
/// # Safety
///
/// Contrato da ABI do ODBC: `estado` aponta para 6 bytes.
#[no_mangle]
#[allow(clippy::too_many_arguments)] // a assinatura e da especificacao
pub unsafe extern "system" fn SQLGetDiagRec(
    _tipo: SqlSmallint,
    h: SqlHandle,
    registro_n: SqlSmallint,
    estado: *mut SqlChar,
    nativo: *mut SqlInteger,
    mensagem: *mut SqlChar,
    capacidade: SqlSmallint,
    tamanho_mensagem: *mut SqlSmallint,
) -> SqlReturn {
    blindado(|| {
        if registro_n < 1 {
            return SQL_ERROR;
        }
        let id = registro::id_de(h);
        let diag = registro::com(id, |p| diag_de(p).get((registro_n as usize) - 1).cloned());
        match diag {
            None => SQL_INVALID_HANDLE,
            Some(None) => SQL_NO_DATA,
            Some(Some(d)) => {
                let (_, _) = escrever_texto(d.estado.as_bytes(), estado, 6);
                escrever_num(nativo, d.nativo);
                let (_, truncou) =
                    escrever_texto(d.mensagem.as_bytes(), mensagem, capacidade as SqlLen);
                escrever_num(tamanho_mensagem, d.mensagem.len() as SqlSmallint);
                if truncou {
                    SQL_SUCCESS_WITH_INFO
                } else {
                    SQL_SUCCESS
                }
            }
        }
    })
}

/// O subconjunto de SQLGetInfo que ferramentas pedem para se apresentar.
/// Info desconhecida responde HYC00 com o numero -- e melhor o cliente saber
/// o que faltou do que receber zero como se fosse resposta.
///
/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLGetInfo(
    dbc: SqlHandle,
    tipo: SqlUSmallint,
    saida: SqlPointer,
    capacidade: SqlSmallint,
    tamanho_saida: *mut SqlSmallint,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(dbc);
        let dados = registro::com(id, |p| match p {
            Punho::Ligacao(l) => Some((l.servidor.clone(), l.usuario.clone())),
            _ => None,
        });
        let Some(Some((servidor, usuario))) = dados else {
            return SQL_INVALID_HANDLE;
        };
        let versao = env!("CARGO_PKG_VERSION");
        let texto = |v: String| -> SqlReturn {
            let (n, _) = escrever_texto(v.as_bytes(), saida as *mut SqlChar, capacidade as SqlLen);
            escrever_num(tamanho_saida, n as SqlSmallint);
            SQL_SUCCESS
        };
        match tipo {
            SQL_DRIVER_NAME => texto("phxsql_odbc".into()),
            SQL_DRIVER_VER | SQL_DBMS_VER => texto(versao.into()),
            SQL_DRIVER_ODBC_VER => texto("03.00".into()),
            SQL_DBMS_NAME => texto("PhxSql".into()),
            SQL_DATA_SOURCE_NAME => texto(String::new()),
            SQL_SERVER_NAME => texto(servidor),
            SQL_USER_NAME => texto(usuario),
            // `SQL_TC_NONE`, e ele deixou de ser a verdade inteira: o
            // SERVIDOR tem transacao desde a 0.19.0 (`docs/TRANSACOES.md`), e
            // quem ESTE driver nao tem e o `SQLEndTran` que as dirige.
            //
            // Continua sendo a resposta certa enquanto for assim, e por um
            // motivo que vale mais que a exatidao da palavra: anunciar
            // `SQL_TC_ALL` faria a ferramenta oferecer um botao de rollback
            // que este driver ignoraria em silencio -- e um rollback que nao
            // reverte e pior do que um rollback que nao existe. Trocar isto
            // pede o `SQLEndTran` implementado, e nao um numero diferente.
            SQL_TXN_CAPABLE => {
                escrever_num(saida as *mut u16, 0);
                escrever_num(tamanho_saida, 2);
                SQL_SUCCESS
            }
            // SQLGetData aceita qualquer coluna em qualquer ordem.
            SQL_GETDATA_EXTENSIONS => {
                escrever_num(saida as *mut u32, 3);
                escrever_num(tamanho_saida, 4);
                SQL_SUCCESS
            }
            outro => {
                anotar(id, "HYC00", &format!("SQLGetInfo {outro} nao suportado"));
                SQL_ERROR
            }
        }
    })
}

/// Atributos de conexao. Autocommit LIGADO e aceito porque e o unico modo que
/// ESTE DRIVER dirige -- e nao porque o servidor nao saiba fazer o outro.
///
/// O servidor tem `BEGIN`/`COMMIT`/`ROLLBACK` desde a 0.19.0; o que falta aqui
/// e o `SQLEndTran` que os chama. Desligar o autocommit sem ele deixaria a
/// ferramenta achando que abriu uma transacao que ninguem abriu -- e o
/// `COMMIT` dela nao confirmaria coisa nenhuma.
///
/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLSetConnectAttr(
    dbc: SqlHandle,
    atributo: SqlInteger,
    valor: SqlPointer,
    _tamanho: SqlInteger,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(dbc);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        const SQL_ATTR_AUTOCOMMIT: SqlInteger = 102;
        if atributo == SQL_ATTR_AUTOCOMMIT {
            return if valor as usize == 1 {
                SQL_SUCCESS
            } else {
                anotar(
                    id,
                    "HYC00",
                    "este driver ainda nao dirige transacao (falta o \
                     SQLEndTran), entao o autocommit nao se desliga. O \
                     servidor tem BEGIN/COMMIT/ROLLBACK: use-os pela porta de \
                     dados ou pela op sql",
                );
                SQL_ERROR
            };
        }
        anotar(
            id,
            "01S02",
            &format!("atributo de conexao {atributo} ignorado"),
        );
        SQL_SUCCESS_WITH_INFO
    })
}

/// # Safety
///
/// Contrato da ABI do ODBC.
#[no_mangle]
pub unsafe extern "system" fn SQLSetStmtAttr(
    stmt: SqlHandle,
    atributo: SqlInteger,
    _valor: SqlPointer,
    _tamanho: SqlInteger,
) -> SqlReturn {
    blindado(|| {
        let id = registro::id_de(stmt);
        if !limpar_diag(id) {
            return SQL_INVALID_HANDLE;
        }
        anotar(
            id,
            "01S02",
            &format!("atributo de comando {atributo} ignorado"),
        );
        SQL_SUCCESS_WITH_INFO
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Prazo das esperas do canal do teste: o servidor de mentira responde na
    /// mesma maquina, e prazo generoso e o que impede uma bancada carregada de
    /// reprovar codigo bom.
    const ESPERA: std::time::Duration = std::time::Duration::from_secs(5);

    // O caminho completo de handles sem rede: aloca ambiente, conexao a
    // partir dele, e recusa o que a especificacao manda recusar.
    #[test]
    fn ciclo_de_handles() {
        unsafe {
            let mut env: SqlHandle = std::ptr::null_mut();
            assert_eq!(
                SQLAllocHandle(SQL_HANDLE_ENV, std::ptr::null_mut(), &mut env),
                SQL_SUCCESS
            );
            assert_eq!(
                SQLSetEnvAttr(env, SQL_ATTR_ODBC_VERSION, 3usize as SqlPointer, 0),
                SQL_SUCCESS
            );
            let mut dbc: SqlHandle = std::ptr::null_mut();
            assert_eq!(SQLAllocHandle(SQL_HANDLE_DBC, env, &mut dbc), SQL_SUCCESS);
            // Comando sobre conexao fechada: 08003, nao um handle.
            let mut stmt: SqlHandle = std::ptr::null_mut();
            assert_eq!(SQLAllocHandle(SQL_HANDLE_STMT, dbc, &mut stmt), SQL_ERROR);
            // Handle inventado nao derruba nada.
            assert_eq!(
                SQLAllocHandle(SQL_HANDLE_STMT, 0xDEAD as SqlHandle, &mut stmt),
                SQL_INVALID_HANDLE
            );
            assert_eq!(SQLFreeHandle(SQL_HANDLE_DBC, dbc), SQL_SUCCESS);
            assert_eq!(SQLFreeHandle(SQL_HANDLE_ENV, env), SQL_SUCCESS);
            // Liberar duas vezes e handle invalido, nao memoria alheia.
            assert_eq!(SQLFreeHandle(SQL_HANDLE_ENV, env), SQL_INVALID_HANDLE);
        }
    }

    // O truncamento com continuacao, direto na funcao de entrega: e o
    // comportamento que o teste de defeito reposto da prova real exercita
    // pela ABI, preso aqui para nao regredir.
    #[test]
    fn entregar_trunca_avisa_e_continua() {
        unsafe {
            let celula = Some("Adriano Boller".to_string());
            let mut buf = [0u8; 8];
            let mut ind: SqlLen = 0;

            let (codigo, ja, diag) = entregar(
                &celula,
                0,
                SQL_C_CHAR,
                buf.as_mut_ptr() as SqlPointer,
                8,
                &mut ind,
            );
            assert_eq!(codigo, SQL_SUCCESS_WITH_INFO);
            assert_eq!(diag.unwrap().0, "01004");
            assert_eq!(&buf[..8], b"Adriano\0");
            assert_eq!(ind, 14); // o que havia antes da chamada

            let (codigo, ja, _) = entregar(
                &celula,
                ja,
                SQL_C_CHAR,
                buf.as_mut_ptr() as SqlPointer,
                8,
                &mut ind,
            );
            assert_eq!(codigo, SQL_SUCCESS);
            assert_eq!(&buf[..8], b" Boller\0");
            assert_eq!(ind, 7);

            let (codigo, _, _) = entregar(
                &celula,
                ja,
                SQL_C_CHAR,
                buf.as_mut_ptr() as SqlPointer,
                8,
                &mut ind,
            );
            assert_eq!(codigo, SQL_NO_DATA);
        }
    }

    #[test]
    fn entregar_null_exige_indicador() {
        unsafe {
            let mut buf = [0u8; 4];
            let (codigo, _, diag) = entregar(
                &None,
                0,
                SQL_C_CHAR,
                buf.as_mut_ptr() as SqlPointer,
                4,
                std::ptr::null_mut(),
            );
            assert_eq!(codigo, SQL_ERROR);
            assert_eq!(diag.unwrap().0, "22002");

            let mut ind: SqlLen = 0;
            let (codigo, _, _) = entregar(
                &None,
                0,
                SQL_C_CHAR,
                buf.as_mut_ptr() as SqlPointer,
                4,
                &mut ind,
            );
            assert_eq!(codigo, SQL_SUCCESS);
            assert_eq!(ind, SQL_NULL_DATA);
        }
    }

    // --- Os parametros pela ABI, contra um SOQUETE de verdade ---
    //
    // Teste unitario nao prova o que viaja no fio: a licao do `BULKINSERT`
    // desta casa. Aqui um servidor de mentira em processo fala o protocolo em
    // claro (uma linha JSON por pedido) e DEVOLVE ao teste cada pedido que
    // recebeu -- e e sobre esse texto, o que realmente saiu, que as
    // conferencias sao feitas.
    fn servidor_de_eco() -> (u16, std::sync::mpsc::Receiver<String>) {
        use std::io::{BufRead, BufReader, Write};
        let escuta = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let porta = escuta.local_addr().unwrap().port();
        let (manda, recebe) = std::sync::mpsc::channel();
        let (pronto, espere) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            pronto.send(()).ok();
            let (soquete, _) = escuta.accept().expect("accept");
            let _ = soquete.set_read_timeout(Some(std::time::Duration::from_secs(5)));
            let mut escrita = soquete.try_clone().unwrap();
            let mut leitor = BufReader::new(soquete);
            loop {
                let mut linha = String::new();
                if leitor.read_line(&mut linha).unwrap_or(0) == 0 {
                    return;
                }
                if manda.send(linha).is_err() {
                    return;
                }
                // `contagem` na resposta poupa o pedido de `esquema` que o
                // driver faria em seguida: o assunto aqui e o pedido de ida.
                let r = r#"{"ok":true,"resultado":{"contagem":1}}"#;
                if writeln!(escrita, "{r}").is_err() || escrita.flush().is_err() {
                    return;
                }
            }
        });
        espere.recv().ok();
        (porta, recebe)
    }

    /// O SQLSTATE do primeiro diagnostico do handle.
    unsafe fn estado_do_diag(h: SqlHandle) -> String {
        let mut estado = [0u8; 6];
        let mut nativo: SqlInteger = 0;
        let mut msg = [0u8; 512];
        let mut tam: SqlSmallint = 0;
        let codigo = SQLGetDiagRec(
            SQL_HANDLE_STMT,
            h,
            1,
            estado.as_mut_ptr(),
            &mut nativo,
            msg.as_mut_ptr(),
            512,
            &mut tam,
        );
        assert_eq!(codigo, SQL_SUCCESS, "o diagnostico devia existir");
        String::from_utf8_lossy(&estado[..5]).into_owned()
    }

    /// O campo `parametros` do pedido, como texto JSON; `None` quando o campo
    /// nao foi mandado.
    fn parametros_do_pedido(linha: &str) -> Option<String> {
        let p = Json::analisar(linha).expect("o pedido tem de ser JSON");
        p.campo("parametros").map(|v| v.escrever())
    }

    #[test]
    fn parametros_viajam_no_pedido_e_a_falta_deles_nao_sai_do_driver() {
        let (porta, recebe) = servidor_de_eco();
        unsafe {
            let mut env: SqlHandle = std::ptr::null_mut();
            assert_eq!(
                SQLAllocHandle(SQL_HANDLE_ENV, std::ptr::null_mut(), &mut env),
                SQL_SUCCESS
            );
            let mut dbc: SqlHandle = std::ptr::null_mut();
            assert_eq!(SQLAllocHandle(SQL_HANDLE_DBC, env, &mut dbc), SQL_SUCCESS);
            let receita = format!("Server=127.0.0.1;Port={porta};Database=b\0");
            assert_eq!(
                SQLDriverConnect(
                    dbc,
                    std::ptr::null_mut(),
                    receita.as_ptr(),
                    SQL_NTS as SqlSmallint,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    0
                ),
                SQL_SUCCESS
            );
            let mut stmt: SqlHandle = std::ptr::null_mut();
            assert_eq!(SQLAllocHandle(SQL_HANDLE_STMT, dbc, &mut stmt), SQL_SUCCESS);

            let executar = |texto: &str| {
                let c = format!("{texto}\0");
                SQLExecDirect(stmt, c.as_ptr(), SQL_NTS)
            };

            // (e) O COMPORTAMENTO VELHO: sem `?` e sem ligacao, o pedido sai
            // exatamente como sempre saiu -- o campo nem aparece. Guarda nova
            // entra pedida, nao imposta.
            assert_eq!(executar("SELECT * FROM c"), SQL_SUCCESS);
            let pedido = recebe.recv_timeout(ESPERA).expect("o pedido devia chegar");
            assert_eq!(parametros_do_pedido(&pedido), None, "pedido: {pedido}");

            // (a) `?` dentro de aspas e DADO: continua sem campo, e sem exigir
            // ligacao nenhuma.
            assert_eq!(executar("SELECT * FROM c WHERE n = '?'"), SQL_SUCCESS);
            let pedido = recebe.recv_timeout(ESPERA).unwrap();
            assert_eq!(parametros_do_pedido(&pedido), None, "pedido: {pedido}");

            // (d) FALTA LIGACAO: recusa com 07002 e -- o que mais importa --
            // NADA sai para o servidor. A prova de que nada saiu e o pedido
            // seguinte: se o recusado tivesse ido, ele estaria na frente da
            // fila.
            assert_eq!(executar("SELECT * FROM c WHERE id = ?"), SQL_ERROR);
            assert_eq!(estado_do_diag(stmt), "07002");
            assert_eq!(executar("SELECT 'marca-de-fila'"), SQL_SUCCESS);
            let pedido = recebe.recv_timeout(ESPERA).unwrap();
            assert!(
                pedido.contains("marca-de-fila"),
                "o pedido recusado nao podia ter saido: {pedido}"
            );

            // (b) A ORDEM e a das POSICOES do texto, e nao a das chamadas: as
            // ligacoes entram do fim para o comeco de proposito.
            let mut segundo: [u8; 9] = *b"Blumenau\0";
            let mut primeiro: i32 = 42;
            let buf_primeiro = &mut primeiro as *mut i32;
            assert_eq!(
                SQLBindParameter(
                    stmt,
                    2,
                    SQL_PARAM_INPUT,
                    SQL_C_CHAR,
                    SQL_VARCHAR,
                    0,
                    0,
                    segundo.as_mut_ptr() as SqlPointer,
                    9,
                    std::ptr::null_mut()
                ),
                SQL_SUCCESS
            );
            assert_eq!(
                SQLBindParameter(
                    stmt,
                    1,
                    SQL_PARAM_INPUT,
                    SQL_C_SLONG,
                    SQL_INTEGER,
                    0,
                    0,
                    buf_primeiro as SqlPointer,
                    0,
                    std::ptr::null_mut()
                ),
                SQL_SUCCESS
            );
            assert_eq!(
                executar("SELECT * FROM c WHERE id = ? AND cidade = ?"),
                SQL_SUCCESS
            );
            let pedido = recebe.recv_timeout(ESPERA).unwrap();
            assert_eq!(
                parametros_do_pedido(&pedido).as_deref(),
                Some(r#"[42,"Blumenau"]"#),
                "pedido: {pedido}"
            );

            // O valor se le na EXECUCAO: trocar o conteudo do buffer sem
            // religar nada tem de mudar o que viaja. Ler na ligacao mandaria
            // 42 de novo, e sem erro nenhum.
            std::ptr::write(buf_primeiro, 4242);
            assert_eq!(
                executar("SELECT * FROM c WHERE id = ? AND cidade = ?"),
                SQL_SUCCESS
            );
            let pedido = recebe.recv_timeout(ESPERA).unwrap();
            assert_eq!(
                parametros_do_pedido(&pedido).as_deref(),
                Some(r#"[4242,"Blumenau"]"#),
                "pedido: {pedido}"
            );

            // (c) SQL_NULL_DATA vira nulo no JSON -- e nao "" nem 0.
            let mut nulo: SqlLen = SQL_NULL_DATA;
            assert_eq!(
                SQLBindParameter(
                    stmt,
                    2,
                    SQL_PARAM_INPUT,
                    SQL_C_CHAR,
                    SQL_VARCHAR,
                    0,
                    0,
                    segundo.as_mut_ptr() as SqlPointer,
                    9,
                    &mut nulo
                ),
                SQL_SUCCESS
            );
            assert_eq!(
                executar("SELECT * FROM c WHERE id = ? AND cidade = ?"),
                SQL_SUCCESS
            );
            let pedido = recebe.recv_timeout(ESPERA).unwrap();
            assert_eq!(
                parametros_do_pedido(&pedido).as_deref(),
                Some("[4242,null]"),
                "pedido: {pedido}"
            );

            // SQL_RESET_PARAMS desliga tudo: a mesma instrucao volta a recusar
            // por falta de ligacao. E o par do teste de cima -- guarda que
            // liga tem de desligar.
            assert_eq!(SQLFreeStmt(stmt, SQL_RESET_PARAMS), SQL_SUCCESS);
            assert_eq!(
                executar("SELECT * FROM c WHERE id = ? AND cidade = ?"),
                SQL_ERROR
            );
            assert_eq!(estado_do_diag(stmt), "07002");

            assert_eq!(SQLFreeHandle(SQL_HANDLE_STMT, stmt), SQL_SUCCESS);
            assert_eq!(SQLDisconnect(dbc), SQL_SUCCESS);
            assert_eq!(SQLFreeHandle(SQL_HANDLE_DBC, dbc), SQL_SUCCESS);
            assert_eq!(SQLFreeHandle(SQL_HANDLE_ENV, env), SQL_SUCCESS);
        }
    }

    // O par SQLNumParams/SQLDescribeParam: o `isql` e as ferramentas de grade
    // perguntam os dois antes de ligar qualquer coisa, e sem SQLPrepare antes
    // a resposta e HY010 e nao zero -- zero seria lido como "esta instrucao
    // nao tem parametros", e a ferramenta nem tentaria ligar.
    #[test]
    fn a_contagem_e_a_descricao_dos_parametros_pela_abi() {
        let (porta, _recebe) = servidor_de_eco();
        unsafe {
            let mut env: SqlHandle = std::ptr::null_mut();
            SQLAllocHandle(SQL_HANDLE_ENV, std::ptr::null_mut(), &mut env);
            let mut dbc: SqlHandle = std::ptr::null_mut();
            SQLAllocHandle(SQL_HANDLE_DBC, env, &mut dbc);
            let receita = format!("Server=127.0.0.1;Port={porta};Database=b\0");
            assert_eq!(
                SQLDriverConnect(
                    dbc,
                    std::ptr::null_mut(),
                    receita.as_ptr(),
                    SQL_NTS as SqlSmallint,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    0
                ),
                SQL_SUCCESS
            );
            let mut stmt: SqlHandle = std::ptr::null_mut();
            SQLAllocHandle(SQL_HANDLE_STMT, dbc, &mut stmt);

            let mut quantos: SqlSmallint = -9;
            assert_eq!(SQLNumParams(stmt, &mut quantos), SQL_ERROR);
            assert_eq!(estado_do_diag(stmt), "HY010");
            assert_eq!(quantos, -9, "sem preparar, nao se escreve numero nenhum");

            // O `?` de dentro das aspas nao entra na conta -- a mesma regra do
            // contador, agora pela ABI.
            let sql = "SELECT * FROM c WHERE n = 'e ai?' AND id = ? AND cidade = ?\0";
            assert_eq!(SQLPrepare(stmt, sql.as_ptr(), SQL_NTS), SQL_SUCCESS);
            assert_eq!(SQLNumParams(stmt, &mut quantos), SQL_SUCCESS);
            assert_eq!(quantos, 2);

            let mut tipo: SqlSmallint = 0;
            let mut tamanho: SqlULen = 7;
            let mut casas: SqlSmallint = 7;
            let mut nulavel: SqlSmallint = 7;
            assert_eq!(
                SQLDescribeParam(stmt, 1, &mut tipo, &mut tamanho, &mut casas, &mut nulavel),
                SQL_SUCCESS
            );
            assert_eq!(
                (tipo, tamanho, casas, nulavel),
                (SQL_VARCHAR, 0, 0, SQL_NULLABLE)
            );
            // Fora da faixa e 07009, nomeando quantos ha.
            assert_eq!(
                SQLDescribeParam(stmt, 3, &mut tipo, &mut tamanho, &mut casas, &mut nulavel),
                SQL_ERROR
            );
            assert_eq!(estado_do_diag(stmt), "07009");

            SQLFreeHandle(SQL_HANDLE_STMT, stmt);
            SQLDisconnect(dbc);
            SQLFreeHandle(SQL_HANDLE_DBC, dbc);
            SQLFreeHandle(SQL_HANDLE_ENV, env);
        }
    }

    // As recusas do SQLBindParameter acontecem na LIGACAO, sem servidor
    // nenhum: e a decisao de recusar cedo, e o teste prova que ela e cedo
    // mesmo -- nao ha conexao aberta em lugar nenhum deste teste.
    #[test]
    fn ligacao_recusa_na_hora_o_que_o_driver_nao_sabe_mandar() {
        let (porta, _recebe) = servidor_de_eco();
        unsafe {
            let mut env: SqlHandle = std::ptr::null_mut();
            SQLAllocHandle(SQL_HANDLE_ENV, std::ptr::null_mut(), &mut env);
            let mut dbc: SqlHandle = std::ptr::null_mut();
            SQLAllocHandle(SQL_HANDLE_DBC, env, &mut dbc);
            let receita = format!("Server=127.0.0.1;Port={porta};Database=b\0");
            assert_eq!(
                SQLDriverConnect(
                    dbc,
                    std::ptr::null_mut(),
                    receita.as_ptr(),
                    SQL_NTS as SqlSmallint,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    0
                ),
                SQL_SUCCESS
            );
            let mut stmt: SqlHandle = std::ptr::null_mut();
            SQLAllocHandle(SQL_HANDLE_STMT, dbc, &mut stmt);
            let mut valor: i32 = 1;
            let ptr = &mut valor as *mut i32 as SqlPointer;
            let ligar = |numero, tipo_io, tipo_c, p| {
                SQLBindParameter(
                    stmt,
                    numero,
                    tipo_io,
                    tipo_c,
                    SQL_VARCHAR,
                    0,
                    0,
                    p,
                    0,
                    std::ptr::null_mut(),
                )
            };
            // Posicao zero nao existe no ODBC: o primeiro `?` e o 1.
            assert_eq!(ligar(0, SQL_PARAM_INPUT, SQL_C_SLONG, ptr), SQL_ERROR);
            assert_eq!(estado_do_diag(stmt), "07009");
            // Saida e entrada-saida: HYC00, nomeando o que falta.
            assert_eq!(ligar(1, SQL_PARAM_OUTPUT, SQL_C_SLONG, ptr), SQL_ERROR);
            assert_eq!(estado_do_diag(stmt), "HYC00");
            assert_eq!(
                ligar(1, SQL_PARAM_INPUT_OUTPUT, SQL_C_SLONG, ptr),
                SQL_ERROR
            );
            assert_eq!(estado_do_diag(stmt), "HYC00");
            // UTF-16 num driver ANSI.
            assert_eq!(ligar(1, SQL_PARAM_INPUT, SQL_C_WCHAR, ptr), SQL_ERROR);
            assert_eq!(estado_do_diag(stmt), "HYC00");
            // Ponteiro nulo sem indicador: nao ha como dizer NULL.
            assert_eq!(
                ligar(1, SQL_PARAM_INPUT, SQL_C_SLONG, std::ptr::null_mut()),
                SQL_ERROR
            );
            assert_eq!(estado_do_diag(stmt), "HY009");
            // E o que o driver SABE mandar passa.
            assert_eq!(ligar(1, SQL_PARAM_INPUT, SQL_C_SLONG, ptr), SQL_SUCCESS);

            SQLFreeHandle(SQL_HANDLE_STMT, stmt);
            SQLDisconnect(dbc);
            SQLFreeHandle(SQL_HANDLE_DBC, dbc);
            SQLFreeHandle(SQL_HANDLE_ENV, env);
        }
    }

    // --- A ponta a ponta, contra um phxsqld DE VERDADE ---
    //
    // FECHADO em 08/09/2026: a F-CONSULTA ligou `sql.parametros`, o lexico
    // aceita `?` e este teste passa contra o motor vivo -- confirmado aqui,
    // e de novo pela sonda viva do passo 7c de `bancada/odbc/prova-abi.py`.
    //
    // Continua `#[ignore]`, e o motivo que sobra e' o unico que sempre foi
    // estrutural: este teste PRECISA de um `phxsqld` de pe, o que nenhum
    // outro teste desta suite exige. Rode com o servidor da prova no ar
    // (docs/ODBC.md, secao 7):
    //
    //     python3 bancada/odbc/provar.py        # sobe, monta os dados, prova
    //     # ou, so para este teste, com o mesmo servidor de pe:
    //     python3 bancada/odbc/montar-dados.py
    //     cargo test -p phxsql-odbc -- --ignored
    //
    // A receita sai do ambiente para a prova nao ficar presa a uma porta:
    // PHXSQL_ODBC_PROVA="Driver=PhxSql;Server=...;Port=...;..."
    #[test]
    #[ignore = "exige um phxsqld vivo em PHXSQL_ODBC_PROVA -- nao roda sozinho na suite"]
    fn ponta_a_ponta_where_id_igual_pergunta() {
        let receita = std::env::var("PHXSQL_ODBC_PROVA").unwrap_or_else(|_| {
            "Driver=PhxSql;Server=127.0.0.1;Port=5305;Token=prova-odbc;\
             UID=root;PWD=prova123;Database=loja"
                .replace(char::is_whitespace, "")
        });
        unsafe {
            let mut env: SqlHandle = std::ptr::null_mut();
            SQLAllocHandle(SQL_HANDLE_ENV, std::ptr::null_mut(), &mut env);
            SQLSetEnvAttr(env, SQL_ATTR_ODBC_VERSION, 3usize as SqlPointer, 0);
            let mut dbc: SqlHandle = std::ptr::null_mut();
            SQLAllocHandle(SQL_HANDLE_DBC, env, &mut dbc);
            let c = format!("{receita}\0");
            assert_eq!(
                SQLDriverConnect(
                    dbc,
                    std::ptr::null_mut(),
                    c.as_ptr(),
                    SQL_NTS as SqlSmallint,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    0
                ),
                SQL_SUCCESS,
                "sem phxsqld na receita nao ha o que provar"
            );
            let mut stmt: SqlHandle = std::ptr::null_mut();
            assert_eq!(SQLAllocHandle(SQL_HANDLE_STMT, dbc, &mut stmt), SQL_SUCCESS);

            let sql = "SELECT nome FROM clientes WHERE id = ?\0";
            assert_eq!(SQLPrepare(stmt, sql.as_ptr(), SQL_NTS), SQL_SUCCESS);
            let mut quantos: SqlSmallint = 0;
            assert_eq!(SQLNumParams(stmt, &mut quantos), SQL_SUCCESS);
            assert_eq!(quantos, 1);

            let mut id: i32 = 3;
            assert_eq!(
                SQLBindParameter(
                    stmt,
                    1,
                    SQL_PARAM_INPUT,
                    SQL_C_SLONG,
                    SQL_INTEGER,
                    0,
                    0,
                    &mut id as *mut i32 as SqlPointer,
                    0,
                    std::ptr::null_mut()
                ),
                SQL_SUCCESS
            );
            // A conferencia e sobre o EFEITO -- a linha que voltou --, e nao
            // sobre o veredito: prova que confere so o codigo de retorno passa
            // com o defeito reposto, e esta casa ja pagou por isso.
            assert_eq!(SQLExecute(stmt), SQL_SUCCESS, "{}", estado_do_diag(stmt));
            assert_eq!(SQLFetch(stmt), SQL_SUCCESS);
            let mut buf = [0u8; 64];
            let mut ind: SqlLen = 0;
            assert_eq!(
                SQLGetData(
                    stmt,
                    1,
                    SQL_C_CHAR,
                    buf.as_mut_ptr() as SqlPointer,
                    64,
                    &mut ind
                ),
                SQL_SUCCESS
            );
            let nome = String::from_utf8_lossy(&buf[..ind.max(0) as usize]).into_owned();
            assert_eq!(nome, "Carlos Consulta");
            // Uma linha so: o `?` filtrou de verdade, e nao devolveu a tabela.
            assert_eq!(SQLFetch(stmt), SQL_NO_DATA);

            SQLFreeHandle(SQL_HANDLE_STMT, stmt);
            SQLDisconnect(dbc);
            SQLFreeHandle(SQL_HANDLE_DBC, dbc);
            SQLFreeHandle(SQL_HANDLE_ENV, env);
        }
    }

    #[test]
    fn entregar_inteiro_confere_a_faixa() {
        unsafe {
            let mut alvo: i32 = 0;
            let mut ind: SqlLen = 0;
            let (codigo, _, _) = entregar(
                &Some("123".into()),
                0,
                SQL_C_SLONG,
                &mut alvo as *mut i32 as SqlPointer,
                4,
                &mut ind,
            );
            assert_eq!((codigo, alvo, ind), (SQL_SUCCESS, 123, 4));

            let (codigo, _, diag) = entregar(
                &Some("9999999999".into()),
                0,
                SQL_C_SLONG,
                &mut alvo as *mut i32 as SqlPointer,
                4,
                &mut ind,
            );
            assert_eq!(codigo, SQL_ERROR);
            assert_eq!(diag.unwrap().0, "22003");

            let (codigo, _, diag) = entregar(
                &Some("abc".into()),
                0,
                SQL_C_SLONG,
                &mut alvo as *mut i32 as SqlPointer,
                4,
                &mut ind,
            );
            assert_eq!(codigo, SQL_ERROR);
            assert_eq!(diag.unwrap().0, "22018");
        }
    }
}
