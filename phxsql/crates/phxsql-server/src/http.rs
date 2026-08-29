//! Servidor HTTP/1.1 minimo, para a interface web.
//!
//! Existe porque navegador nao abre soquete TCP cru: a porta 5000 fala JSON
//! Lines, e o navegador precisa de HTTP. Este modulo e a ponte, e nada mais --
//! ele nao serve arquivo do disco, nao lista diretorio e nao interpreta
//! caminho. So ha duas rotas, e a pagina esta embutida no binario.
//!
//! ```text
//! GET  /            a interface (embutida com include_str!)
//! GET  /saude       um "ok" para monitoramento, sem autenticacao
//! POST /api         o mesmo protocolo da porta 5000, uma operacao por pedido
//! ```
//!
//! # Sessao
//!
//! O protocolo TCP autentica uma vez por CONEXAO. HTTP nao tem conexao
//! duradoura, entao o `login` devolve um identificador de sessao que o
//! navegador manda no cabecalho `X-Sessao`. A sessao guarda o usuario ja
//! autenticado, e assim o PBKDF2 de 210.000 iteracoes roda uma vez por login,
//! nao a cada clique.
//!
//! # O que este modulo NAO faz
//!
//! Nao serve arquivo do sistema de arquivos. Nao ha `..` para explorar, nao ha
//! caminho para escapar: ou e uma das rotas acima, ou e 404. E a forma mais
//! simples de nao ter travessia de diretorio -- nao tendo diretorio.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

use phxsql_core::json::Json;

/// A interface, embutida no binario em tempo de compilacao.
///
/// E um FRAGMENTO de proposito -- comeca no `<title>`, sem `<!doctype>` nem
/// `<html>`. O mesmo arquivo e publicado como artefato na web, onde o
/// esqueleto vem de fora; aqui ele e montado por [`montar_pagina`]. Um arquivo
/// so, servido nos dois lugares, sem risco de divergirem.
pub const PAGINA: &str = include_str!("../ui/index.html");

/// O phx-grid, do ecossistema Phoenix: ES5 estrito, zero dependencia,
/// arquivo unico. Entra no cabecalho para estar definido antes de a pagina
/// rodar o proprio script. Fonte e historico em `ui/grid/`.
const GRID_CSS: &str = include_str!("../ui/grid/phx-grid.css");
const GRID_JS: &str = include_str!("../ui/grid/phx-grid.js");

/// O desenho do diagrama ER, em arquivo proprio.
///
/// Separado do `index.html` porque o layout do grafo e a unica parte da
/// interface que e ALGORITMO -- e algoritmo nao deveria morar no meio de sete
/// mil linhas de tela. Entra no cabecalho junto com o grid, pelo mesmo motivo:
/// estar definido antes de a pagina rodar o proprio script.
const DIAGRAMA_JS: &str = include_str!("../ui/diagrama-er.js");

/// Envolve o fragmento no esqueleto que o navegador espera.
///
/// Sem `<!doctype html>` o navegador entra em modo de compatibilidade e o
/// layout muda -- e o tipo de defeito que so aparece na maquina do usuario.
/// O `<title>`, o `<meta>` e os `<link>` do fragmento sao subidos para o
/// cabecalho pelo proprio analisador de HTML, exatamente como acontece quando
/// a pagina e publicada como artefato.
pub fn montar_pagina() -> String {
    format!(
        "<!doctype html>\n<html lang=\"pt-BR\">\n<head>\n<meta charset=\"utf-8\">\n\
         <style>\n{GRID_CSS}\n</style>\n<script>\n{GRID_JS}\n</script>\n\
         <script>\n{DIAGRAMA_JS}\n</script>\n\
         </head>\n<body>\n{PAGINA}\n</body>\n</html>\n"
    )
}

/// Teto do cabecalho, para um pedido malformado nao consumir memoria.
const MAX_CABECALHO: usize = 16 * 1024;
/// Teto do corpo de um pedido.
const MAX_CORPO: usize = 4 * 1024 * 1024;

#[derive(Debug)]
pub struct Pedido {
    pub metodo: String,
    pub caminho: String,
    pub cabecalhos: HashMap<String, String>,
    pub corpo: String,
}

impl Pedido {
    pub fn cabecalho(&self, nome: &str) -> Option<&str> {
        self.cabecalhos
            .get(&nome.to_lowercase())
            .map(String::as_str)
    }
}

/// Le um pedido HTTP. Devolve `None` quando a conexao fecha ou o pedido e
/// grande demais.
pub fn ler_pedido(fluxo: &TcpStream) -> Option<Pedido> {
    let mut leitor = BufReader::new(fluxo);

    let mut linha = String::new();
    if leitor.read_line(&mut linha).ok()? == 0 {
        return None;
    }
    let mut partes = linha.split_whitespace();
    let metodo = partes.next()?.to_string();
    let caminho = partes.next()?.to_string();

    let mut cabecalhos = HashMap::new();
    let mut lidos = linha.len();
    loop {
        let mut l = String::new();
        if leitor.read_line(&mut l).ok()? == 0 {
            return None;
        }
        lidos += l.len();
        if lidos > MAX_CABECALHO {
            return None;
        }
        let t = l.trim_end();
        if t.is_empty() {
            break;
        }
        if let Some((chave, valor)) = t.split_once(':') {
            cabecalhos.insert(chave.trim().to_lowercase(), valor.trim().to_string());
        }
    }

    let tamanho: usize = cabecalhos
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if tamanho > MAX_CORPO {
        return None;
    }
    let mut corpo = vec![0u8; tamanho];
    if tamanho > 0 && leitor.read_exact(&mut corpo).is_err() {
        return None;
    }

    Some(Pedido {
        metodo,
        caminho: caminho.split('?').next().unwrap_or("/").to_string(),
        cabecalhos,
        corpo: String::from_utf8_lossy(&corpo).into_owned(),
    })
}

/// Monta o texto completo da resposta HTTP.
///
/// Separada do envio para poder ser conferida em teste -- os cabecalhos de
/// seguranca sao o tipo de coisa que some numa refatoracao sem ninguem notar.
pub fn montar_resposta(codigo: u16, tipo: &str, corpo: &str) -> String {
    let motivo = match codigo {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        _ => "Error",
    };
    // Cabecalhos de seguranca: a pagina nao vai para dentro de um quadro
    // alheio, nao adivinha tipo de conteudo e so conversa com esta origem.
    //
    // A unica coisa que ela busca fora e a fonte da marca, e so no HTML --
    // por isso a folga do `style-src`/`font-src` nao existe nas respostas de
    // dados. Servidor sem internet: a fonte nao carrega, a pilha de reserva
    // assume e a pagina continua inteira.
    let externo = tipo.starts_with("text/html");
    let estilo = if externo {
        "style-src 'unsafe-inline' https://fonts.googleapis.com; \
         font-src https://fonts.gstatic.com; "
    } else {
        "style-src 'unsafe-inline'; "
    };
    format!(
        "HTTP/1.1 {codigo} {motivo}\r\n\
         Content-Type: {tipo}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         X-Frame-Options: DENY\r\n\
         Referrer-Policy: no-referrer\r\n\
         Content-Security-Policy: default-src 'none'; {estilo}\
         script-src 'unsafe-inline'; \
         img-src data:; connect-src 'self'; form-action 'none'; \
         frame-ancestors 'none'; base-uri 'none'\r\n\
         Connection: close\r\n\
         \r\n{corpo}",
        corpo.len()
    )
}

/// Envia a resposta.
pub fn responder(
    fluxo: &mut TcpStream,
    codigo: u16,
    tipo: &str,
    corpo: &str,
) -> std::io::Result<()> {
    fluxo.write_all(montar_resposta(codigo, tipo, corpo).as_bytes())?;
    fluxo.flush()
}

pub fn responder_json(fluxo: &mut TcpStream, codigo: u16, valor: &Json) -> std::io::Result<()> {
    responder(
        fluxo,
        codigo,
        "application/json; charset=utf-8",
        &valor.escrever(),
    )
}

pub fn erro_json(fluxo: &mut TcpStream, codigo: u16, mensagem: &str) -> std::io::Result<()> {
    responder_json(
        fluxo,
        codigo,
        &Json::objeto(vec![
            ("ok", Json::Bool(false)),
            ("erro", Json::texto_de(mensagem)),
        ]),
    )
}

// --------------------------------------------------------------- sessoes

/// Uma sessao do navegador: quem entrou e ate quando vale.
///
/// `login` vazio e uma sessao ainda anonima -- ela nasce assim no `desafio`,
/// que acontece ANTES de haver identidade, e so ganha nome quando o `login`
/// da certo. E o que permite o desafio-resposta por HTTP: o nonce precisa
/// sobreviver de um pedido para o outro, e a sessao e o unico lugar que
/// atravessa os dois.
#[derive(Debug, Clone)]
pub struct Sessao {
    pub login: String,
    pub expira_ms: i64,
    /// Quando a sessao comecou. So o prazo de expiracao nao responde "ha
    /// quanto tempo esta aberta", porque cada clique o renova.
    pub desde_ms: i64,
    /// Desafio em aberto: (usuario, nonce do servidor, quando expira).
    pub desafio: Option<(String, String, i64)>,
}

/// As sessoes vivas, por identificador.
#[derive(Debug, Default)]
pub struct Sessoes {
    dentro: HashMap<String, Sessao>,
}

impl Sessoes {
    pub fn nova(&mut self, login: &str, duracao_ms: i64, agora_ms: i64) -> String {
        self.limpar(agora_ms);
        let id = phxsql_core::hash::para_hex(&phxsql_core::senha::bytes_aleatorios(24));
        self.dentro.insert(
            id.clone(),
            Sessao {
                login: login.to_string(),
                expira_ms: agora_ms + duracao_ms,
                desde_ms: agora_ms,
                desafio: None,
            },
        );
        id
    }

    /// Devolve o login da sessao, renovando o prazo a cada uso.
    pub fn usar(&mut self, id: &str, duracao_ms: i64, agora_ms: i64) -> Option<String> {
        let s = self.dentro.get_mut(id)?;
        if agora_ms > s.expira_ms {
            self.dentro.remove(id);
            return None;
        }
        s.expira_ms = agora_ms + duracao_ms;
        Some(s.login.clone())
    }

    /// Amarra um login a uma sessao que ja existe. Falso se ela sumiu.
    pub fn definir_login(&mut self, id: &str, login: &str) -> bool {
        match self.dentro.get_mut(id) {
            Some(s) => {
                s.login = login.to_string();
                true
            }
            None => false,
        }
    }

    /// Guarda o desafio em aberto desta sessao.
    pub fn guardar_desafio(&mut self, id: &str, desafio: (String, String, i64)) {
        if let Some(s) = self.dentro.get_mut(id) {
            s.desafio = Some(desafio);
        }
    }

    /// Retira o desafio em aberto. Vale UMA vez: sai daqui e nao volta,
    /// dando certo ou errado -- igual ao caminho TCP.
    pub fn tomar_desafio(&mut self, id: &str) -> Option<(String, String, i64)> {
        self.dentro.get_mut(id).and_then(|s| s.desafio.take())
    }

    pub fn encerrar(&mut self, id: &str) -> bool {
        self.dentro.remove(id).is_some()
    }

    pub fn limpar(&mut self, agora_ms: i64) {
        self.dentro.retain(|_, s| agora_ms <= s.expira_ms);
    }

    pub fn quantas(&self) -> usize {
        self.dentro.len()
    }

    /// As sessoes vivas, como (id, login, quando comecou, quando expira).
    ///
    /// O id sai CORTADO de proposito: ele e a credencial da sessao, e quem
    /// olha a lista de conexoes nao precisa de um cookie que da para colar
    /// noutro navegador. Oito letras bastam para achar a linha.
    pub fn listar(&self, agora_ms: i64) -> Vec<(String, String, i64, i64)> {
        let mut v: Vec<(String, String, i64, i64)> = self
            .dentro
            .iter()
            .filter(|(_, s)| s.expira_ms >= agora_ms)
            .map(|(id, s)| {
                (
                    id.chars().take(8).collect::<String>(),
                    s.login.clone(),
                    s.desde_ms,
                    s.expira_ms,
                )
            })
            .collect();
        v.sort_by_key(|x| x.2);
        v
    }

    /// Encerra pelo comeco do id, que e o que a lista mostra.
    pub fn encerrar_por_prefixo(&mut self, prefixo: &str) -> bool {
        let Some(id) = self.dentro.keys().find(|k| k.starts_with(prefixo)).cloned() else {
            return false;
        };
        self.dentro.remove(&id);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_800_000_000_000;
    const HORA: i64 = 3_600_000;

    #[test]
    fn a_pagina_esta_embutida() {
        assert!(
            PAGINA.contains("PhxSql"),
            "a interface deveria estar embutida"
        );
        assert!(PAGINA.len() > 1_000);
    }

    #[test]
    fn sessao_vale_e_expira() {
        let mut s = Sessoes::default();
        let id = s.nova("adriano", HORA, T0);
        assert_eq!(s.usar(&id, HORA, T0).as_deref(), Some("adriano"));
        // Cada uso renova o prazo.
        assert_eq!(s.usar(&id, HORA, T0 + HORA / 2).as_deref(), Some("adriano"));
        // Passado o prazo desde o ultimo uso, cai.
        assert!(s.usar(&id, HORA, T0 + 3 * HORA).is_none());
        assert!(s.usar(&id, HORA, T0 + 3 * HORA).is_none());
    }

    #[test]
    fn identificador_de_sessao_nao_repete_e_e_longo() {
        let mut s = Sessoes::default();
        let mut vistos = std::collections::HashSet::new();
        for i in 0..200 {
            let id = s.nova("x", HORA, T0 + i);
            assert_eq!(id.len(), 48, "24 bytes em hexadecimal");
            assert!(vistos.insert(id), "identificador de sessao repetiu");
        }
    }

    #[test]
    fn encerrar_derruba_na_hora() {
        let mut s = Sessoes::default();
        let id = s.nova("ana", HORA, T0);
        assert!(s.encerrar(&id));
        assert!(s.usar(&id, HORA, T0).is_none());
        assert!(!s.encerrar(&id));
    }

    #[test]
    fn sessao_desconhecida_nao_entra() {
        let mut s = Sessoes::default();
        assert!(s.usar("nao-existe", HORA, T0).is_none());
        assert!(s.usar("", HORA, T0).is_none());
    }

    #[test]
    fn limpar_tira_as_vencidas() {
        let mut s = Sessoes::default();
        s.nova("a", HORA, T0);
        s.nova("b", 10 * HORA, T0);
        assert_eq!(s.quantas(), 2);
        s.limpar(T0 + 2 * HORA);
        assert_eq!(s.quantas(), 1);
    }

    #[test]
    fn a_resposta_traz_os_cabecalhos_de_seguranca() {
        let r = montar_resposta(200, "application/json", "{\"ok\":true}");
        for esperado in [
            "HTTP/1.1 200 OK",
            "Content-Length: 11",
            "Cache-Control: no-store",
            "X-Content-Type-Options: nosniff",
            "X-Frame-Options: DENY",
            "Referrer-Policy: no-referrer",
            "default-src 'none'",
            "frame-ancestors 'none'",
            "connect-src 'self'",
        ] {
            assert!(r.contains(esperado), "faltou o cabecalho: {esperado}");
        }
        assert!(r.ends_with("\r\n\r\n{\"ok\":true}"));
    }

    #[test]
    fn o_tamanho_declarado_bate_com_o_corpo() {
        for corpo in ["", "{}", "ação com acento", &"x".repeat(5_000)] {
            let r = montar_resposta(200, "text/plain", corpo);
            let declarado: usize = r
                .split("Content-Length: ")
                .nth(1)
                .unwrap()
                .split("\r\n")
                .next()
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(declarado, corpo.len(), "corpo de {} bytes", corpo.len());
        }
    }

    #[test]
    fn codigos_de_erro_tem_motivo() {
        for (codigo, motivo) in [
            (400u16, "Bad Request"),
            (403, "Forbidden"),
            (404, "Not Found"),
            (405, "Method Not Allowed"),
            (413, "Payload Too Large"),
        ] {
            assert!(montar_resposta(codigo, "text/plain", "")
                .starts_with(&format!("HTTP/1.1 {codigo} {motivo}")));
        }
    }
    #[test]
    fn a_pagina_servida_tem_esqueleto_e_o_fragmento_nao() {
        assert!(
            !PAGINA.to_lowercase().contains("<!doctype"),
            "o fragmento nao pode trazer esqueleto: ele e publicado como artefato"
        );
        let inteira = montar_pagina();
        assert!(inteira.starts_with("<!doctype html>"));
        assert!(inteira.contains("<html lang=\"pt-BR\">"));
        assert!(inteira.contains("<meta charset=\"utf-8\">"));
        assert!(
            inteira.contains(PAGINA),
            "o fragmento tem de entrar inteiro"
        );
        assert!(inteira.trim_end().ends_with("</html>"));
    }

    #[test]
    fn so_o_html_pode_buscar_a_fonte_da_marca() {
        let pagina = montar_resposta(200, "text/html; charset=utf-8", "x");
        assert!(pagina.contains("https://fonts.googleapis.com"));
        assert!(pagina.contains("font-src https://fonts.gstatic.com"));

        let dados = montar_resposta(200, "application/json; charset=utf-8", "{}");
        assert!(
            !dados.contains("fonts.g"),
            "resposta de dados nao abre excecao para host nenhum"
        );
        assert!(dados.contains("default-src 'none'"));
    }

    #[test]
    fn desafio_atravessa_dois_pedidos_e_vale_uma_vez_so() {
        let mut s = Sessoes::default();
        let id = s.nova("", HORA, T0);
        assert!(s.tomar_desafio(&id).is_none());
        s.guardar_desafio(&id, ("adriano".into(), "nonce123".into(), T0 + 30_000));
        let d = s
            .tomar_desafio(&id)
            .expect("o desafio deveria estar guardado");
        assert_eq!(d.0, "adriano");
        assert_eq!(d.1, "nonce123");
        assert!(s.tomar_desafio(&id).is_none(), "vale uma vez so");
    }

    #[test]
    fn a_sessao_anonima_ganha_nome_no_login() {
        let mut s = Sessoes::default();
        let id = s.nova("", HORA, T0);
        assert_eq!(s.usar(&id, HORA, T0).as_deref(), Some(""));
        assert!(s.definir_login(&id, "adriano"));
        assert_eq!(s.usar(&id, HORA, T0).as_deref(), Some("adriano"));
        assert!(!s.definir_login("sessao-que-nao-existe", "invasor"));
    }
}
