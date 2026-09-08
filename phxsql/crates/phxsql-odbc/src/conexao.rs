//! A conversa com o phxsqld: TCP, uma linha JSON por pedido, resposta com
//! `"ok"` -- o mesmo protocolo da porta de dados que todo cliente do PhxSql
//! ja fala. O driver nao inventa transporte: ele e um cliente comum.

use phxsql_core::base64;
use phxsql_core::fio::{Canal as FioCanal, Iniciador, Recebido};
use phxsql_core::json::Json;
use std::io::{BufRead, BufReader, Write};
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
///
/// A cifra do fio entra por duas chaves a mais, e so por opcao: `CIFRA=1`
/// liga o aperto de mao estilo Noise (`docs/CIFRA-DO-FIO.md`), e
/// `CHAVE_DO_FIO=<hex>` e o PINO -- a chave publica X25519 que se espera do
/// servidor. Sem elas o driver fala em claro, exatamente como sempre falou.
#[derive(Debug, Default, Clone)]
pub struct Receita {
    pub servidor: String,
    pub porta: u16,
    pub token: String,
    pub usuario: String,
    pub senha: String,
    pub database: String,
    /// Pedir o tunel. Sem pino e so contra escuta PASSIVA; contra quem esta no
    /// meio, so vale com o `CHAVE_DO_FIO` (o pino) e o servidor exigindo.
    pub cifra: bool,
    /// O pino em hexadecimal, ainda por conferir. Fica cru de proposito: pino
    /// torto e ERRO na hora de conectar, nao ausencia silenciosa de pino --
    /// deixar um pino invalido virar "sem pino" seria rebaixar a garantia sem
    /// ninguem pedir, a mesma armadilha que o `pino_torto_na_origem` guarda.
    pub chave_do_fio: String,
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
            "cifra" | "encrypt" => r.cifra = verdadeiro(&valor),
            "chave_do_fio" | "chavedofio" | "pino" => r.chave_do_fio = valor,
            // "driver" e o que o gerenciador usou para nos achar; o resto e
            // ignorado de proposito -- recusar chave desconhecida quebraria
            // toda ferramenta que acrescenta as suas.
            _ => {}
        }
    }
    // Pino dado sem CIFRA=1 ainda LIGA a cifra: quem escreve o pino quer o
    // tunel conferido, e cair para claro por ter esquecido a outra chave seria
    // um rebaixamento silencioso do que a pessoa pediu. A porta para desligar
    // e nao escrever o pino.
    if !r.chave_do_fio.trim().is_empty() {
        r.cifra = true;
    }
    r
}

/// O que conta como "sim" numa chave booleana da connection string.
fn verdadeiro(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "sim" | "yes" | "on" | "ligada" | "ligado"
    )
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
    // O modo da cifra volta, mas o PINO nao: ele e chave publica e nao seria
    // vazamento, mas a string de volta e para dizer o que esta ligado, nao
    // para reconstruir a receita -- token e senha ja saem mascarados pelo
    // mesmo motivo. Quem quiser reconectar guarda a receita inteira.
    if r.cifra {
        s.push_str(";CIFRA=1");
    }
    s
}

/// Le o pino cru da receita, ja conferindo que e uma X25519 de verdade.
///
/// Vazio quer dizer "sem pino" -- so entao a ausencia e legitima. Um hexa
/// torto e ERRO, e nao "siga sem pino": rebaixar a garantia por um dedo errado
/// e o oposto do que o pino existe para fazer. Mesma decisao do
/// `pino_torto_na_origem_e_erro_e_nao_ausencia` do lado do servidor.
fn pino_da_receita(hex: &str) -> Result<Option<[u8; 32]>, Falha> {
    let limpo: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
    if limpo.is_empty() {
        return Ok(None);
    }
    let bytes = phxsql_core::hash::de_hex(&limpo).filter(|b| b.len() == 32);
    let Some(bytes) = bytes else {
        return Err(Falha::nova(
            "08001",
            "CHAVE_DO_FIO nao e uma chave X25519 de 64 digitos hexadecimais",
        ));
    };
    let mut k = [0u8; 32];
    k.copy_from_slice(&bytes);
    Ok(Some(k))
}

/// A ligacao viva com um phxsqld: os dois lados do soquete, o token que vai em
/// cada linha, e o FIO -- em claro (como sempre foi) ou dentro do tunel.
///
/// A leitura passou a ser de um `BufReader`, e nao de uma sobra a mao, porque
/// o `fio::Canal` do core le por linha: e o MESMO caminho da `replica::Cliente`
/// (`docs/CIFRA-DO-FIO.md`), e reusar o fio do core e nao reescrever a camada
/// de registro e o que evita uma segunda implementacao da cifra aqui.
pub struct Canal {
    /// O lado de ESCRITA. O de leitura e o `BufReader` abaixo, sobre um clone
    /// do mesmo soquete -- o timeout vale para os dois, e um clone e o que
    /// deixa ler e escrever sem uma trava so.
    fluxo: TcpStream,
    leitor: BufReader<TcpStream>,
    token: String,
    /// `Claro` ate o aperto fechar; `Cifrado` da linha seguinte em diante.
    fio: FioCanal,
}

impl Canal {
    /// Abre o soquete, liga o tunel quando a receita pede, e faz o login
    /// quando ha usuario. Erro ja sai com o SQLSTATE certo: 08001 nao alcancou
    /// (ou o aperto caiu), 28000 credencial recusada.
    pub fn abrir(r: &Receita) -> Result<Canal, Falha> {
        if r.servidor.is_empty() || r.porta == 0 {
            return Err(Falha::nova(
                "HY000",
                "connection string sem Server= ou Port= validos",
            ));
        }
        // O pino e conferido ANTES de tocar a rede: pino torto e um erro de
        // configuracao, e devolve-lo sem nem conectar e o retorno mais claro.
        let pino = pino_da_receita(&r.chave_do_fio)?;

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
        let leitor =
            BufReader::new(tcp.try_clone().map_err(|e| {
                Falha::nova("08001", format!("nao clonei o soquete de {alvo}: {e}"))
            })?);
        let mut canal = Canal {
            fluxo: tcp,
            leitor,
            token: r.token.clone(),
            fio: FioCanal::Claro,
        };

        // O tunel ANTES do login, de proposito: e a senha e o token que ele
        // existe para esconder, e depois do login ja seria tarde. E o mesmo
        // que a `replica::Cliente` faz.
        if r.cifra {
            canal.cifrar(pino)?;
        }

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

    /// Faz o aperto de mao e passa a falar por dentro do tunel.
    ///
    /// Manda o `cifrar` em CLARO (a mensagem 1 e um pedido comum, sem moldura
    /// nova), le a resposta em claro, fecha com `terminal` e so ENTAO o `fio`
    /// vira cifrado. A partir da proxima linha, `pedir` sela e abre registros.
    /// Reusa o `fio` do core inteiro -- aqui nao ha cripto nenhuma.
    fn cifrar(&mut self, pino: Option<[u8; 32]>) -> Result<(), Falha> {
        let (iniciador, m1) = Iniciador::comecar(pino);
        let pedido = Json::objeto(vec![
            ("op", Json::texto_de("cifrar")),
            ("e", Json::texto_de(base64::codificar(&m1))),
        ])
        .escrever();
        self.fluxo
            .write_all(pedido.as_bytes())
            .and_then(|_| self.fluxo.write_all(b"\n"))
            .and_then(|_| self.fluxo.flush())
            .map_err(|e| Falha::nova("08001", format!("mandando o aperto de mao: {e}")))?;

        let mut resposta = String::new();
        let lidos = self
            .leitor
            .read_line(&mut resposta)
            .map_err(|e| Falha::nova("08001", format!("lendo o aperto de mao: {e}")))?;
        if lidos == 0 {
            return Err(Falha::nova(
                "08001",
                "o servidor fechou a conexao durante o aperto de mao",
            ));
        }
        let j = Json::analisar(&resposta)
            .map_err(|_| Falha::nova("08001", "a resposta do aperto de mao nao e JSON"))?;
        if !j.booleano_ou("ok", false) {
            // O servidor sem cifra (ou com ela desligada) recusa aqui: o
            // motivo dele vem em claro, e nao carrega segredo -- so a politica.
            return Err(Falha::nova(
                "08001",
                format!(
                    "o servidor recusou o aperto de mao: {}",
                    j.texto_ou("erro", "sem motivo")
                ),
            ));
        }
        let m2 = base64::decodificar(
            j.campo("resultado")
                .map(|res| res.texto_ou("m2", ""))
                .unwrap_or(""),
        )
        .map_err(|_| Falha::nova("08001", "a mensagem 2 do aperto nao e Base64 valido"))?;

        // Pino errado cai aqui, dentro do `terminar`. A mensagem NAO ecoa
        // chave nenhuma -- mesmo a publica fica de fora, para o diagnostico
        // nunca virar um lugar de onde se leia material de chave.
        let (transporte, _apresentada) = iniciador.terminar(&m2).map_err(|e| {
            use phxsql_core::error::PhxError;
            match e {
                PhxError::Autorizacao(_) => Falha::nova(
                    "08001",
                    "a chave apresentada pelo servidor nao confere com o pino \
                     (CHAVE_DO_FIO): pode ser outro servidor ou alguem no meio",
                ),
                _ => Falha::nova("08001", "o aperto de mao da cifra nao fechou"),
            }
        })?;
        self.fio = FioCanal::Cifrado(Box::new(transporte));
        Ok(())
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
        // O `\n` sai por conta do `Canal`: em claro ele escreve a linha crua,
        // cifrado ele sela um registro Base64 -- de qualquer jeito, uma linha.
        let linha = Json::Objeto(todos).escrever();

        // Falha de escrita ou leitura menciona so a OPERACAO: o corpo do
        // pedido pode carregar senha, e mensagem de erro vira log alheio. O
        // erro do `fio` nunca carrega o corpo -- e falha de rede ou de
        // etiqueta, nao o texto do pedido.
        self.fio
            .escrever(&mut self.fluxo, &linha)
            .map_err(|e| Falha::nova("08S01", format!("mandando {op}: {e}")))?;

        let resposta = match self
            .fio
            .ler(&mut self.leitor)
            .map_err(|e| Falha::nova("08S01", format!("lendo a resposta de {op}: {e}")))?
        {
            Recebido::Linha(l) => l,
            // Fim limpo no lugar de uma resposta e, para um pedido, o servidor
            // ter ido embora: para o driver e o mesmo estrago que um corte.
            Recebido::Fim => {
                return Err(Falha::nova(
                    "08S01",
                    format!("o servidor fechou a conexao durante {op}"),
                ))
            }
        };
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

    #[test]
    fn cifra_e_pino_saem_da_connection_string() {
        let hex = phxsql_core::hash::para_hex(&[0x11u8; 32]);
        let r = analisar_receita(&format!("Server=h;Port=1;CIFRA=1;CHAVE_DO_FIO={hex}"));
        assert!(r.cifra);
        assert_eq!(r.chave_do_fio, hex);
    }

    // Sem as chaves novas, a receita nasce em CLARO -- a regra petrea "guarda
    // nova entra pedida, nao imposta", no ponto onde o pedido se le.
    #[test]
    fn sem_as_chaves_a_receita_e_em_claro() {
        let r = analisar_receita("Server=h;Port=1;UID=u;PWD=s");
        assert!(!r.cifra);
        assert!(r.chave_do_fio.is_empty());
    }

    // O pino escrito sem CIFRA=1 ainda liga a cifra: cair para claro por ter
    // esquecido o interruptor seria rebaixar o que a pessoa pediu, em silencio.
    #[test]
    fn pino_liga_a_cifra_mesmo_sem_o_interruptor() {
        let hex = phxsql_core::hash::para_hex(&[0x22u8; 32]);
        let r = analisar_receita(&format!("Server=h;Port=1;CHAVE_DO_FIO={hex}"));
        assert!(r.cifra, "pino dado devia ligar a cifra");
    }

    // O modo volta na string mascarada, o PINO nunca -- nem o publico.
    #[test]
    fn mascarada_diz_o_modo_e_nao_o_pino() {
        let hex = phxsql_core::hash::para_hex(&[0x33u8; 32]);
        let r = analisar_receita(&format!("Server=h;Port=1;CIFRA=1;CHAVE_DO_FIO={hex}"));
        let m = receita_mascarada(&r);
        assert!(m.contains("CIFRA=1"), "o modo devia voltar: {m}");
        assert!(!m.contains(&hex), "o pino nao devia voltar: {m}");
    }

    // Pino torto e ERRO, e nao "siga sem pino" -- a mesma decisao do lado do
    // servidor (`pino_torto_na_origem_e_erro_e_nao_ausencia`).
    #[test]
    fn pino_torto_e_erro_e_nao_ausencia() {
        assert!(pino_da_receita("").unwrap().is_none());
        assert!(pino_da_receita("nao-e-hexa").is_err());
        // 62 digitos: hexa valido, tamanho errado -- tambem cai.
        assert!(pino_da_receita(&"ab".repeat(31)).is_err());
        // 64 digitos: passa.
        let bom = "cd".repeat(32);
        assert!(pino_da_receita(&bom).unwrap().is_some());
    }

    // --- A prova real do APERTO pelo `Canal`, sem gerenciador de driver ---
    //
    // Um servidor em processo, feito so do `fio` do core, faz o papel do
    // phxsqld: le o `cifrar` em claro, responde a mensagem 2, e da linha
    // seguinte em diante so fala REGISTROS. Um driver que ignorasse a cifra
    // mandaria JSON cru, e o `abrir` do lado servidor nao o autenticaria --
    // e por isso este teste so passa com o tunel de verdade.
    use std::net::TcpListener;
    use std::sync::mpsc;

    /// Sobe o servidor de aperto numa thread e devolve (porta, pino).
    ///
    /// A thread atende UMA conexao: aperto, depois responde a UM pedido pelo
    /// tunel ecoando o campo `marca`. O soquete ganha prazo de leitura para a
    /// thread nunca ficar pendurada quando o cliente desiste (pino errado).
    fn servidor_de_aperto() -> (u16, [u8; 32]) {
        let estatica = phxsql_core::x25519::gerar_privada();
        let pino = phxsql_core::x25519::chave_publica(&estatica);
        let escuta = TcpListener::bind("127.0.0.1:0").expect("bind");
        let porta = escuta.local_addr().unwrap().port();
        let (pronto, espere) = mpsc::channel();
        std::thread::spawn(move || {
            pronto.send(()).ok();
            let (soquete, _) = escuta.accept().expect("accept");
            let _ = soquete.set_read_timeout(Some(Duration::from_secs(3)));
            let mut escrita = soquete.try_clone().unwrap();
            let mut leitor = BufReader::new(soquete);

            // 1. o `cifrar`, em claro.
            let mut linha = String::new();
            if leitor.read_line(&mut linha).unwrap_or(0) == 0 {
                return;
            }
            let pedido = Json::analisar(&linha).unwrap();
            let m1 = base64::decodificar(pedido.texto_ou("e", "")).unwrap();
            let (transporte, m2) = phxsql_core::fio::responder(&estatica, &m1).unwrap();
            let resposta = Json::objeto(vec![
                ("ok", Json::Bool(true)),
                ("op", Json::texto_de("cifrar")),
                (
                    "resultado",
                    Json::objeto(vec![("m2", Json::texto_de(base64::codificar(&m2)))]),
                ),
            ])
            .escrever();
            writeln!(escrita, "{resposta}").unwrap();
            escrita.flush().unwrap();

            // 2. da linha seguinte em diante, so registros.
            let mut fio = FioCanal::Cifrado(Box::new(transporte));
            if let Ok(Recebido::Linha(l)) = fio.ler(&mut leitor) {
                let jp = Json::analisar(&l).unwrap();
                let r = Json::objeto(vec![
                    ("ok", Json::Bool(true)),
                    ("op", Json::texto_de(jp.texto_ou("op", "?"))),
                    (
                        "resultado",
                        Json::objeto(vec![("eco", Json::texto_de(jp.texto_ou("marca", "")))]),
                    ),
                ])
                .escrever();
                let _ = fio.escrever(&mut escrita, &r);
            }
        });
        espere.recv().ok();
        (porta, pino)
    }

    #[test]
    fn aperto_pelo_canal_fecha_e_fala_por_dentro() {
        let (porta, pino) = servidor_de_aperto();
        let r = Receita {
            servidor: "127.0.0.1".into(),
            porta,
            cifra: true,
            chave_do_fio: phxsql_core::hash::para_hex(&pino),
            ..Receita::default()
        };
        let mut canal = Canal::abrir(&r).expect("o aperto com o pino certo devia fechar");
        // O `pedir` viaja SELADO: se o driver falasse claro, o servidor de
        // aperto (que so le registros agora) nao devolveria este eco.
        let resposta = canal
            .pedir(vec![
                ("op", Json::texto_de("ping")),
                ("marca", Json::texto_de("ola-pelo-tunel")),
            ])
            .expect("o pedido pelo tunel devia responder");
        assert_eq!(resposta.texto_ou("eco", ""), "ola-pelo-tunel");
    }

    // O defeito reposto do pino: apresentar a chave certa nao basta se o
    // cliente esperava OUTRA. O `abrir` cai no `terminar`, e a mensagem nao
    // ecoa chave nenhuma.
    #[test]
    fn pino_errado_derruba_o_aperto_sem_vazar_chave() {
        let (porta, _pino_certo) = servidor_de_aperto();
        let outro = phxsql_core::x25519::chave_publica(&phxsql_core::x25519::gerar_privada());
        let r = Receita {
            servidor: "127.0.0.1".into(),
            porta,
            cifra: true,
            chave_do_fio: phxsql_core::hash::para_hex(&outro),
            ..Receita::default()
        };
        // `match` e nao `expect_err` de proposito: o `Canal` guarda o soquete
        // e o tunel, e nenhum dos dois tem por que ser `Debug` -- e o `Debug`
        // do tunel e justamente o que o core esconde para o segredo nao vazar.
        let erro = match Canal::abrir(&r) {
            Ok(_) => panic!("pino errado tem de derrubar o aperto"),
            Err(e) => e,
        };
        assert_eq!(erro.estado, "08001");
        let hex_outro = phxsql_core::hash::para_hex(&outro);
        assert!(
            !erro.mensagem.contains(&hex_outro),
            "o diagnostico nao pode carregar material de chave: {}",
            erro.mensagem
        );
    }
}
