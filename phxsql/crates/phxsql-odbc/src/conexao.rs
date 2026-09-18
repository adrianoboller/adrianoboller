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
/// A cifra do fio NASCE ligada (pedido 373): o driver faz o aperto de mao
/// estilo Noise (`docs/CIFRA-DO-FIO.md`) sem ninguem escrever nada, e
/// `CIFRA=0` e o escape ESCRITO para voltar ao claro. `CHAVE_DO_FIO=<hex>` e
/// o PINO -- a chave publica X25519 que se espera do servidor.
#[derive(Clone)]
pub struct Receita {
    pub servidor: String,
    pub porta: u16,
    pub token: String,
    pub usuario: String,
    pub senha: String,
    pub database: String,
    /// Pedir o tunel -- e ele nasce PEDIDO. Sem pino e so contra escuta
    /// PASSIVA; contra quem esta no meio, so vale com o `CHAVE_DO_FIO` (o
    /// pino) e o servidor exigindo.
    pub cifra: bool,
    /// O pino em hexadecimal, ainda por conferir. Fica cru de proposito: pino
    /// torto e ERRO na hora de conectar, nao ausencia silenciosa de pino --
    /// deixar um pino invalido virar "sem pino" seria rebaixar a garantia sem
    /// ninguem pedir, a mesma armadilha que o `pino_torto_na_origem` guarda.
    pub chave_do_fio: String,
}

/// O padrao da receita, e ele NAO e o zero do tipo em `cifra`.
///
/// Decisao do dono, 18/09/2026 (pedido 373): *«`CIFRA=1` vira o padrao da
/// receita do ODBC, com escape `CIFRA=0` escrito»*. O motivo e o mesmo da
/// chave que nasce conferida -- o esquecimento nao pode ser o padrao quando
/// o assunto e senha no fio, e a senha viaja no login deste driver.
///
/// Mora no `Default` e nao dentro do `analisar_receita` porque o `SQLConnect`
/// com `host:porta/database` monta a `Receita` DAQUI, sem passar pelo
/// analisador: padrao que morasse so no analisador deixaria esse irmao
/// falando claro, calado.
impl Default for Receita {
    fn default() -> Receita {
        Receita {
            servidor: String::new(),
            porta: 0,
            token: String::new(),
            usuario: String::new(),
            senha: String::new(),
            database: String::new(),
            cifra: true,
            chave_do_fio: String::new(),
        }
    }
}

/// `Debug` a mao: a receita e montada da linha de conexao do ODBC e carrega o
/// token e a senha em claro. `chave_do_fio` fica visivel porque e o PINO -- a
/// chave publica esperada do servidor --, e ve-lo e o que permite diagnosticar
/// pino torto.
impl std::fmt::Debug for Receita {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Desestruturar SEM `..`: campo novo para de compilar aqui, e quem
        // o acrescentar decide na hora se e segredo. Lista de campos escrita
        // a mao envelhece calada.
        let Receita {
            servidor,
            porta,
            token: _,
            usuario,
            senha: _,
            database,
            cifra,
            chave_do_fio,
        } = self;
        f.debug_struct("Receita")
            .field("servidor", servidor)
            .field("porta", porta)
            .field("token", &"(oculto)")
            .field("usuario", usuario)
            .field("senha", &"(oculta)")
            .field("database", database)
            .field("cifra", cifra)
            .field("chave_do_fio", chave_do_fio)
            .finish()
    }
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
            // Valor que ninguem reconhece NAO desliga a cifra. Com o padrao
            // ligado (pedido 373), um `CIFRA=zero` mal digitado viraria claro
            // em silencio -- e rebaixamento por dedo errado e exatamente o que
            // a virada veio acabar. Desligar continua exigindo escolha
            // ESCRITA, e escrita de um jeito que o driver entenda.
            "cifra" | "encrypt" => {
                if let Some(pedido) = interruptor(&valor) {
                    r.cifra = pedido;
                }
            }
            "chave_do_fio" | "chavedofio" | "pino" => r.chave_do_fio = valor,
            // "driver" e o que o gerenciador usou para nos achar; o resto e
            // ignorado de proposito -- recusar chave desconhecida quebraria
            // toda ferramenta que acrescenta as suas.
            _ => {}
        }
    }
    // O pino vence o interruptor, e depois do pedido 373 e o `CIFRA=0` que ele
    // vence: quem escreve o pino quer o tunel CONFERIDO, e desliga-lo por uma
    // chave escrita ao lado seria rebaixar em silencio o que a pessoa pediu. A
    // porta para desligar continua sendo nao escrever o pino.
    if !r.chave_do_fio.trim().is_empty() {
        r.cifra = true;
    }
    r
}

/// O que uma chave booleana da connection string PEDE -- `None` quando o valor
/// nao e nem sim nem nao.
///
/// Os tres estados existem por causa do padrao ligado: com dois, o "nao
/// reconhecido" cairia no `false` e um dedo errado desligaria a cifra.
fn interruptor(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "sim" | "yes" | "on" | "ligada" | "ligado" => Some(true),
        "0" | "false" | "nao" | "no" | "off" | "desligada" | "desligado" => Some(false),
        _ => None,
    }
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
    // O modo da cifra volta nos DOIS estados desde o pedido 373: com o padrao
    // ligado, omitir o `0` faria a string de volta mentir -- relida, ligaria um
    // tunel que esta conexao nao tem. O PINO nao volta: ele e chave publica e
    // nao seria vazamento, mas a string de volta e para dizer o que esta
    // ligado, nao para reconstruir a receita -- token e senha ja saem
    // mascarados pelo mesmo motivo. Quem quiser reconectar guarda a receita
    // inteira.
    s.push_str(if r.cifra { ";CIFRA=1" } else { ";CIFRA=0" });
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

/// A falha do aperto com a SAIDA escrita dentro dela -- e so quando ela
/// existe de verdade.
///
/// A cifra virou o padrao (pedido 373), entao a connection string que sempre
/// funcionou em claro passa a falhar AQUI, contra um servidor com
/// `cifra_fio.ligada: false`. O erro cru de soquete mandaria procurar rede;
/// quem le o diagnostico precisa do interruptor, e e por isso que ele vem no
/// texto em vez de ficar so na documentacao que ninguem abre no meio de um
/// SQLDriverConnect.
///
/// Com PINO escrito a frase NAO entra, e isso e decisao e nao esquecimento:
/// `CIFRA=0` nao desliga a cifra de quem escreveu o pino -- seria conselho
/// falso --, e a falha com pino e justamente a chave apresentada nao conferir.
/// Ensinar a baixar a guarda ali seria ensinar o rebaixamento que o pino
/// existe para impedir.
fn com_a_saida_escrita(f: Falha, r: &Receita) -> Falha {
    if !r.chave_do_fio.trim().is_empty() {
        return f;
    }
    Falha {
        estado: f.estado,
        mensagem: format!(
            "{}; a cifra do fio e o PADRAO deste driver -- se este servidor \
             nao a oferece, escreva CIFRA=0 na connection string para falar \
             em claro",
            f.mensagem
        ),
        nativo: f.nativo,
    }
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
            canal.cifrar(pino).map_err(|f| com_a_saida_escrita(f, r))?;
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

    /// A receita nao mostra o token nem a senha no `Debug`.
    ///
    /// O driver monta a receita da linha de conexao do ODBC, que traz as duas
    /// em claro. Um `dbg!(&receita)` num diagnostico de "por que nao conecta"
    /// e exatamente onde isso vazaria -- e e o diagnostico que mais se faz.
    ///
    /// O pino (`chave_do_fio`) e chave PUBLICA e continua visivel: e ve-lo que
    /// permite achar pino torto.
    #[test]
    fn o_debug_da_receita_nunca_mostra_o_token_nem_a_senha() {
        let r = analisar_receita(
            "Driver=PhxSql;Server=10.0.0.7;Token=token-secreto-do-fio;\
             UID=ana;PWD=senha-secreta-da-ana;Database=loja;\
             CHAVE_DO_FIO=aabbccddeeff00112233445566778899",
        );
        assert_eq!(r.token, "token-secreto-do-fio");
        assert_eq!(r.senha, "senha-secreta-da-ana");
        for texto in [format!("{:?}", r), format!("{r:?}")] {
            assert!(
                !texto.contains("token-secreto-do-fio"),
                "o token vazou: {texto}"
            );
            assert!(
                !texto.contains("senha-secreta-da-ana"),
                "a senha vazou: {texto}"
            );
            assert!(texto.contains("ana"), "o Debug perdeu o usuario: {texto}");
            assert!(
                texto.contains("aabbccddeeff00112233445566778899"),
                "o pino sumiu do Debug: {texto}"
            );
        }
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

    // Este teste se chamava `sem_as_chaves_a_receita_e_em_claro` e provava a
    // garantia que o dono REVOGOU em 18/09/2026 (pedido 373). Ele nao sumiu:
    // mudou de assunto e passou a provar o ESCAPE escrito, porque teste que
    // some leva a garantia junto -- e a garantia que sobrou e a de que
    // `CIFRA=0` continua valendo para quem precisa do claro.
    #[test]
    fn o_escape_escrito_e_o_cifra_zero() {
        for receita in [
            "Server=h;Port=1;UID=u;PWD=s;CIFRA=0",
            "Server=h;Port=1;UID=u;PWD=s;Encrypt=0",
            "Server=h;Port=1;CIFRA=false",
            "Server=h;Port=1;CIFRA=nao",
            "Server=h;Port=1;CIFRA=off",
        ] {
            let r = analisar_receita(receita);
            assert!(!r.cifra, "«{receita}» tinha de falar claro");
            assert!(r.chave_do_fio.is_empty());
        }
    }

    // O irmao do de cima, e o que falha se o padrao voltar a ser claro: a
    // receita de sempre -- a que ninguem atualizou -- nasce CIFRADA.
    #[test]
    fn sem_escrever_nada_a_receita_nasce_cifrada() {
        let r = analisar_receita("Server=h;Port=1;UID=u;PWD=s");
        assert!(r.cifra, "a receita tem de nascer cifrada (pedido 373)");
        assert!(r.chave_do_fio.is_empty(), "pino que ninguem escreveu");
    }

    // O `SQLConnect` com `host:porta/database` NAO passa pelo analisador: ele
    // monta a `Receita` do `Default`. Padrao que morasse so no analisador
    // deixaria esse irmao falando claro, calado.
    #[test]
    fn o_caminho_do_sqlconnect_tambem_nasce_cifrado() {
        assert!(
            Receita::default().cifra,
            "o Default e o que o SQLConnect usa"
        );
    }

    // Valor que ninguem reconhece nao rebaixa: `CIFRA=zero` e dedo errado, e
    // nao escolha escrita. Com o padrao ligado, aceitar qualquer coisa como
    // "nao" devolveria o rebaixamento silencioso pela porta do VALOR, depois
    // de ele ter sido fechado na porta da CHAVE.
    #[test]
    fn valor_torto_na_cifra_nao_rebaixa_para_claro() {
        for receita in [
            "Server=h;Port=1;CIFRA=zero",
            "Server=h;Port=1;CIFRA=talvez",
            "Server=h;Port=1;CIFRA=",
            "Server=h;Port=1;Encrypt=disabled",
        ] {
            assert!(
                analisar_receita(receita).cifra,
                "«{receita}» nao podia desligar a cifra"
            );
        }
    }

    // O pino escrito sem CIFRA=1 ainda liga a cifra: cair para claro por ter
    // esquecido o interruptor seria rebaixar o que a pessoa pediu, em silencio.
    #[test]
    fn pino_liga_a_cifra_mesmo_sem_o_interruptor() {
        let hex = phxsql_core::hash::para_hex(&[0x22u8; 32]);
        let r = analisar_receita(&format!("Server=h;Port=1;CHAVE_DO_FIO={hex}"));
        assert!(r.cifra, "pino dado devia ligar a cifra");
    }

    // E o outro lado do mesmo: agora que existe `CIFRA=0`, o pino vence ELE.
    // Quem escreve o pino quer o tunel conferido; a porta para desligar
    // continua sendo nao escrever o pino.
    #[test]
    fn o_pino_vence_o_cifra_zero() {
        let hex = phxsql_core::hash::para_hex(&[0x44u8; 32]);
        let r = analisar_receita(&format!("Server=h;Port=1;CIFRA=0;CHAVE_DO_FIO={hex}"));
        assert!(r.cifra, "pino escrito nao se desliga por CIFRA=0");
    }

    // A string de volta diz o modo nos DOIS estados, e a prova e a ida e
    // volta: omitir o `CIFRA=0` a faria ligar, na releitura, um tunel que a
    // conexao nao tinha -- mentira sobre o que esta ligado, gravada no arquivo
    // de configuracao do aplicativo.
    #[test]
    fn a_mascarada_leva_o_modo_nos_dois_sentidos() {
        for (receita, esperado) in [
            ("Server=h;Port=1;UID=ana", true),
            ("Server=h;Port=1;UID=ana;CIFRA=0", false),
        ] {
            let r = analisar_receita(receita);
            assert_eq!(r.cifra, esperado, "«{receita}»");
            let volta = receita_mascarada(&r);
            assert_eq!(
                analisar_receita(&volta).cifra,
                esperado,
                "a volta mudou o modo: {volta}"
            );
        }
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

    /// Sobe um servidor que RECUSA o aperto, como um phxsqld com
    /// `cifra_fio.ligada: false` -- que e o que toda connection string velha
    /// vai encontrar depois do pedido 373.
    ///
    /// Le uma linha, responde a recusa em claro (e o que o servidor de verdade
    /// faz: a resposta do aperto nao viaja selada) e vai embora.
    fn servidor_que_recusa_o_aperto() -> u16 {
        let escuta = TcpListener::bind("127.0.0.1:0").expect("bind");
        let porta = escuta.local_addr().unwrap().port();
        let (pronto, espere) = mpsc::channel();
        std::thread::spawn(move || {
            pronto.send(()).ok();
            let Ok((soquete, _)) = escuta.accept() else {
                return;
            };
            let _ = soquete.set_read_timeout(Some(Duration::from_secs(3)));
            let mut escrita = soquete.try_clone().unwrap();
            let mut leitor = BufReader::new(soquete);
            let mut linha = String::new();
            if leitor.read_line(&mut linha).unwrap_or(0) == 0 {
                return;
            }
            let recusa = Json::objeto(vec![
                ("ok", Json::Bool(false)),
                ("op", Json::texto_de("cifrar")),
                (
                    "erro",
                    Json::texto_de(
                        "este servidor nao atende a cifra do fio \
                         (cifra_fio.ligada esta em false)",
                    ),
                ),
            ])
            .escrever();
            let _ = writeln!(escrita, "{recusa}");
            let _ = escrita.flush();
        });
        espere.recv().ok();
        porta
    }

    // A receita velha, contra o servidor que nao oferece o tunel: o
    // diagnostico traz o motivo do servidor E a saida. Sem a saida escrita,
    // quem migrar fica com um 08001 mandando procurar rede.
    #[test]
    fn o_aperto_recusado_ensina_a_saida() {
        let porta = servidor_que_recusa_o_aperto();
        let r = analisar_receita(&format!(
            "Server=127.0.0.1;Port={porta};UID=ana;PWD=senha-que-nao-pode-vazar"
        ));
        assert!(r.cifra, "a receita velha agora pede o tunel");
        let erro = match Canal::abrir(&r) {
            Ok(_) => panic!("servidor que recusa o aperto nao podia conectar"),
            Err(e) => e,
        };
        assert_eq!(erro.estado, "08001");
        assert!(
            erro.mensagem.contains("CIFRA=0"),
            "o diagnostico tem de ensinar a saida: {}",
            erro.mensagem
        );
        assert!(
            erro.mensagem.contains("cifra_fio.ligada"),
            "o motivo do servidor tem de vir junto, e nao ser trocado: {}",
            erro.mensagem
        );
        assert!(
            !erro.mensagem.contains("senha-que-nao-pode-vazar"),
            "o diagnostico nao pode carregar a senha: {}",
            erro.mensagem
        );
    }

    // Com PINO a saida NAO se ensina, e este e o teste que trava a decisao:
    // `CIFRA=0` nao desligaria a cifra de quem pinou (seria conselho falso), e
    // a falha com pino e a chave nao conferir -- mandar baixar a guarda ali
    // seria ensinar o rebaixamento que o pino existe para impedir.
    #[test]
    fn com_pino_o_diagnostico_nao_manda_baixar_a_guarda() {
        let porta = servidor_que_recusa_o_aperto();
        let hex = phxsql_core::hash::para_hex(&[0x55u8; 32]);
        let r = analisar_receita(&format!("Server=127.0.0.1;Port={porta};CHAVE_DO_FIO={hex}"));
        let erro = match Canal::abrir(&r) {
            Ok(_) => panic!("servidor que recusa o aperto nao podia conectar"),
            Err(e) => e,
        };
        assert_eq!(erro.estado, "08001");
        assert!(
            !erro.mensagem.contains("CIFRA=0"),
            "com pino, a saida nao se ensina: {}",
            erro.mensagem
        );
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
