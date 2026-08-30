//! Driver ODBC 3.x do PhxSql.
//!
//! Uma `cdylib` de ABI C que o gerenciador de driver (unixODBC no Linux, o
//! Driver Manager do Windows) carrega por dlopen/LoadLibrary. Por dentro ela
//! e um cliente comum da porta de dados: TCP, uma linha JSON por pedido.
//!
//! O recorte e o nucleo que um consumidor real usa para LER: conectar
//! (DSN-less), `SQLExecDirect` de um SELECT, descrever colunas com tipo
//! honesto, fetch e SQLGetData com truncamento avisado, diagnostico. O que
//! ficou de fora e por que esta em `docs/ODBC.md`.
//!
//! Sao as funcoes ANSI (`SQLDriverConnect`, nao `...W`): o gerenciador de
//! driver converte as chamadas wide do aplicativo para elas sozinho. O texto
//! e UTF-8 dos dois lados, que e o que o servidor fala.
//!
//! Regra herdada da casa e que aqui vira contrato de ABI: nenhum caminho de
//! erro escreve senha ou token em diagnostico -- ha teste para isso.

mod conexao;
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

/// O miolo comum de SQLExecDirect e SQLExecute: manda o texto INTEIRO para o
/// servidor -- o parser mora la, e o erro dele volta com a coluna do
/// problema. O driver so olha o FROM para pedir o esquema, que e de onde
/// saem os tipos honestos.
fn executar_sql(id: usize, sql: String) -> SqlReturn {
    {
        let dono = registro::com(id, |p| match p {
            Punho::Comando(c) => {
                c.resultado = None;
                c.cursor = 0;
                c.entregues.clear();
                Some(c.dono)
            }
            _ => None,
        });
        let Some(Some(dono)) = dono else {
            return SQL_INVALID_HANDLE;
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
            let resposta = canal.pedir(vec![
                ("op", Json::texto_de("sql")),
                ("database", Json::texto_de(&database)),
                ("texto", Json::texto_de(&sql)),
            ])?;
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

/// Preparar aqui e guardar o texto: nao ha parametros nem plano no driver, e
/// o servidor analisa na execucao. Existe porque o isql e outros clientes so
/// falam prepare/execute -- sem ele, "Connected!" e a ultima coisa que
/// funciona.
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
                    SQL_RESET_PARAMS => {} // este driver nao tem parametros
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
