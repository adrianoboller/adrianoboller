//! A conversa com o phxsqld: TCP, uma linha JSON por pedido, resposta com
//! `"ok"` -- o mesmo protocolo da porta de dados que todo cliente do PhxSql
//! ja fala. O driver nao inventa transporte: ele e um cliente comum.

use phxsql_core::json::Json;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Erro interno do driver, ja com o SQLSTATE que vai para o diagnostico.
///
/// A mensagem NUNCA carrega o pedido enviado: o login leva a senha no corpo,
/// e um erro que ecoasse o corpo a poria no diagnostico -- que aplicativo
/// cliente escreve em log sem perguntar.
#[derive(Debug)]
pub struct Falha {
    pub estado: &'static str,
    pub mensagem: String,
    /// O `codigo` numerico do erro do servidor, que o SQLGetDiagRec entrega
    /// como erro nativo. Zero quando o erro nasceu aqui no driver.
    pub nativo: i32,
}

impl Falha {
    pub fn nova(estado: &'static str, mensagem: impl Into<String>) -> Falha {
        Falha {
            estado,
            mensagem: mensagem.into(),
            nativo: 0,
        }
    }
}

/// O que a connection string diz. DSN-less, chaves sem distincao de caixa:
/// `Driver=PhxSql;Server=h;Port=5000;Token=t;UID=u;PWD=s;Database=d`.
#[derive(Debug, Default, Clone)]
pub struct Receita {
    pub servidor: String,
    pub porta: u16,
    pub token: String,
    pub usuario: String,
    pub senha: String,
    pub database: String,
}

/// Divide `chave=valor;...` aceitando valor entre chaves `{...}`, que e como
/// o ODBC escapa `;` dentro de senha.
fn pares(texto: &str) -> Vec<(String, String)> {
    let mut saida = Vec::new();
    let mut resto = texto;
    while !resto.is_empty() {
        let Some(igual) = resto.find('=') else { break };
        let chave = resto[..igual].trim().to_ascii_lowercase();
        resto = &resto[igual + 1..];
        let valor = if let Some(depois) = resto.strip_prefix('{') {
            let fim = depois.find('}').unwrap_or(depois.len());
            let v = &depois[..fim];
            resto = depois[fim..].strip_prefix('}').unwrap_or("");
            resto = resto.strip_prefix(';').unwrap_or(resto);
            v.to_string()
        } else {
            let fim = resto.find(';').unwrap_or(resto.len());
            let v = resto[..fim].trim();
            resto = resto.get(fim + 1..).unwrap_or("");
            v.to_string()
        };
        if !chave.is_empty() {
            saida.push((chave, valor));
        }
    }
    saida
}

pub fn analisar_receita(texto: &str) -> Receita {
    let mut r = Receita {
        porta: 5000,
        ..Receita::default()
    };
    for (chave, valor) in pares(texto) {
        match chave.as_str() {
            "server" | "servidor" | "host" => r.servidor = valor,
            "port" | "porta" => r.porta = valor.trim().parse().unwrap_or(0),
            "token" => r.token = valor,
            "uid" | "user" | "usuario" => r.usuario = valor,
            "pwd" | "password" | "senha" => r.senha = valor,
            "database" | "db" => r.database = valor,
            // "driver" e o que o gerenciador usou para nos achar; o resto e
            // ignorado de proposito -- recusar chave desconhecida quebraria
            // toda ferramenta que acrescenta as suas.
            _ => {}
        }
    }
    r
}

/// A connection string que o driver devolve no `SQLDriverConnect`.
///
/// Remontada com senha e token MASCARADOS: o aplicativo costuma guardar essa
/// string em arquivo de configuracao proprio, e o driver nao decide onde ela
/// vai parar. O preco documentado e que ela nao serve para reconectar
/// sozinha -- quem quiser guardar segredo que guarde o proprio.
pub fn receita_mascarada(r: &Receita) -> String {
    let mut s = format!("Driver=PhxSql;Server={};Port={}", r.servidor, r.porta);
    if !r.token.is_empty() {
        s.push_str(";Token=***");
    }
    if !r.usuario.is_empty() {
        s.push_str(&format!(";UID={}", r.usuario));
    }
    if !r.senha.is_empty() {
        s.push_str(";PWD=***");
    }
    if !r.database.is_empty() {
        s.push_str(&format!(";Database={}", r.database));
    }
    s
}

/// A ligacao viva com um phxsqld: soquete mais o token que vai em cada linha.
pub struct Canal {
    tcp: TcpStream,
    token: String,
    /// Bytes lidos alem do `\n` da resposta anterior. O servidor responde uma
    /// linha por pedido, mas o leitor pega o que o SO entregar.
    sobra: Vec<u8>,
}

impl Canal {
    /// Abre o soquete e faz o login quando ha usuario. Erro ja sai com o
    /// SQLSTATE certo: 08001 nao alcancou, 28000 credencial recusada.
    pub fn abrir(r: &Receita) -> Result<Canal, Falha> {
        if r.servidor.is_empty() || r.porta == 0 {
            return Err(Falha::nova(
                "HY000",
                "connection string sem Server= ou Port= validos",
            ));
        }
        let alvo = format!("{}:{}", r.servidor, r.porta);
        let enderecos = alvo
            .to_socket_addrs()
            .map_err(|e| Falha::nova("08001", format!("nao resolvi {alvo}: {e}")))?;
        let mut ultimo = Falha::nova("08001", format!("nenhum endereco para {alvo}"));
        let mut tcp = None;
        for endereco in enderecos {
            match TcpStream::connect_timeout(&endereco, Duration::from_secs(10)) {
                Ok(t) => {
                    tcp = Some(t);
                    break;
                }
                Err(e) => ultimo = Falha::nova("08001", format!("nao conectei em {alvo}: {e}")),
            }
        }
        let Some(tcp) = tcp else { return Err(ultimo) };
        // Sem timeout de leitura o aplicativo congelaria junto com a rede, e
        // quem congela dentro de um SQLExecDirect nao tem como cancelar.
        let _ = tcp.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = tcp.set_write_timeout(Some(Duration::from_secs(30)));
        let mut canal = Canal {
            tcp,
            token: r.token.clone(),
            sobra: Vec::new(),
        };
        if !r.usuario.is_empty() {
            canal
                .pedir(vec![
                    ("op", Json::texto_de("login")),
                    ("usuario", Json::texto_de(&r.usuario)),
                    ("senha", Json::texto_de(&r.senha)),
                ])
                .map_err(|f| Falha {
                    estado: "28000",
                    mensagem: f.mensagem,
                    nativo: f.nativo,
                })?;
        }
        Ok(canal)
    }

    /// Manda um pedido e devolve o campo `resultado` (ou a resposta inteira,
    /// quando o servidor nao embrulha).
    pub fn pedir(&mut self, campos: Vec<(&str, Json)>) -> Result<Json, Falha> {
        let op = campos
            .first()
            .and_then(|(_, v)| v.texto())
            .unwrap_or("?")
            .to_string();
        let mut todos = vec![("token".to_string(), Json::texto_de(&self.token))];
        todos.extend(campos.into_iter().map(|(k, v)| (k.to_string(), v)));
        let linha = Json::Objeto(todos).escrever() + "\n";

        // Falha de escrita ou leitura menciona so a OPERACAO: o corpo do
        // pedido pode carregar senha, e mensagem de erro vira log alheio.
        self.tcp
            .write_all(linha.as_bytes())
            .map_err(|e| Falha::nova("08S01", format!("mandando {op}: {e}")))?;

        let resposta = self.ler_linha(&op)?;
        let r = Json::analisar(&resposta)
            .map_err(|_| Falha::nova("08S01", format!("resposta de {op} nao e JSON")))?;
        if !r.campo("ok").and_then(|o| o.booleano()).unwrap_or(false) {
            let mensagem = r
                .texto_ou("erro", "o servidor recusou sem dizer o motivo")
                .to_string();
            // O erro do servidor vem estruturado (`nome`, `codigo`), e e por
            // ai que o SQLSTATE se decide -- nao por prefixo de texto, que ja
            // falhou uma vez na prova de ABI: a mensagem de sintaxe chega
            // como «esquema invalido: SQL, coluna N: ...», com prefixo.
            let estado = match r.texto_ou("nome", "") {
                "NAO_ENCONTRADO" => "42S02",
                _ if mensagem.contains("SQL, coluna") => "42000",
                _ => "HY000",
            };
            return Err(Falha {
                estado,
                mensagem,
                nativo: r.inteiro_ou("codigo", 0) as i32,
            });
        }
        Ok(r.campo("resultado").cloned().unwrap_or(r))
    }

    fn ler_linha(&mut self, op: &str) -> Result<String, Falha> {
        let mut bloco = [0u8; 64 * 1024];
        loop {
            if let Some(fim) = self.sobra.iter().position(|&b| b == b'\n') {
                let linha = self.sobra.drain(..=fim).collect::<Vec<u8>>();
                return Ok(String::from_utf8_lossy(&linha).into_owned());
            }
            let n = self
                .tcp
                .read(&mut bloco)
                .map_err(|e| Falha::nova("08S01", format!("lendo a resposta de {op}: {e}")))?;
            if n == 0 {
                return Err(Falha::nova(
                    "08S01",
                    format!("o servidor fechou a conexao durante {op}"),
                ));
            }
            self.sobra.extend_from_slice(&bloco[..n]);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn receita_completa() {
        let r = analisar_receita(
            "Driver=PhxSql;Server=10.0.0.7;Port=5305;Token=abc;UID=ana;PWD=s3nh4;Database=loja",
        );
        assert_eq!(r.servidor, "10.0.0.7");
        assert_eq!(r.porta, 5305);
        assert_eq!(r.token, "abc");
        assert_eq!(r.usuario, "ana");
        assert_eq!(r.senha, "s3nh4");
        assert_eq!(r.database, "loja");
    }

    #[test]
    fn chaves_sem_caixa_e_senha_entre_chaves() {
        let r = analisar_receita("SERVER=h;port=1;pwd={com;ponto e virgula};uid=u");
        assert_eq!(r.senha, "com;ponto e virgula");
        assert_eq!(r.usuario, "u");
    }

    // A string devolvida vai parar em arquivo de configuracao de aplicativo:
    // este teste e o que impede senha e token de irem junto.
    #[test]
    fn mascarada_nao_vaza_segredo() {
        let r = analisar_receita("Server=h;Port=5305;Token=segredo9;UID=ana;PWD=s3nh4;Database=d");
        let m = receita_mascarada(&r);
        assert!(!m.contains("s3nh4"), "senha na string de volta: {m}");
        assert!(!m.contains("segredo9"), "token na string de volta: {m}");
        assert!(m.contains("UID=ana") && m.contains("Database=d"));
    }
}
