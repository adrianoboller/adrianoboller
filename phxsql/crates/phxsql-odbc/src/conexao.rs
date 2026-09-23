//! A conversa com o phxsqld: TCP, uma linha JSON por pedido, resposta com
//! `"ok"` -- o mesmo protocolo da porta de dados que todo cliente do PhxSql
//! ja fala. O driver nao inventa transporte: ele e um cliente comum.

use phxsql_core::base64;
use phxsql_core::fio::{Canal as FioCanal, Iniciador, Recebido};
use phxsql_core::json::Json;
use std::io::{BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Erro interno do driver, ja com o SQLSTATE que vai para o diagnostico.
///
/// A mensagem NUNCA carrega o pedido enviado, e a regra sobreviveu ao motivo
/// que a fez nascer: ate o pedido 275 o login levava a senha no corpo. Hoje
/// nao leva -- leva a prova --, mas o token de servico continua em toda linha
/// e o corpo de um `inserir` e o dado do cliente. Um erro que ecoasse o corpo
/// poria os dois no diagnostico, que e o que aplicativo cliente escreve em log
/// sem perguntar.
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
/// o assunto e credencial no fio. A senha em si deixou de viajar no pedido
/// 275 (ver `autenticar`), e o tunel NAO ficou sobrando: o `Token` de servico
/// vai em cada linha, e o dado de todo `inserir` e `sql` tambem. Os dois
/// pedidos sao eixos diferentes -- o 373 e o canal, o 275 e a forma do login.
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
            canal.autenticar(r).map_err(|f| Falha {
                // 28000 e o que o gerenciador de driver le para pedir a
                // credencial de novo. Trocar a FORMA do login nao pode trocar
                // o codigo com que o aplicativo ja decide isso.
                estado: "28000",
                mensagem: f.mensagem,
                nativo: f.nativo,
            })?;
        }
        Ok(canal)
    }

    /// Desafio-resposta: a senha nao sai desta maquina (pedido 275).
    ///
    /// Ate aqui o driver mandava a forma (3) do `op_login` -- literalmente
    /// `("senha", ...)` --, e era o UNICO cliente desta casa que o fazia: a
    /// replica (`replica.rs`) e o console (`phxsql-cmd`) ja provavam pelo
    /// desafio. A cifra do fio, que nasce ligada (pedido 373), tirava a senha
    /// do FIO; nao tirava a senha da MAQUINA, e o que chegava ao servidor
    /// continuava sendo a senha -- que e a petrea «senha nunca em texto puro,
    /// nem em resposta do protocolo».
    ///
    /// A conta inteira e a do core, a MESMA que o servidor refaz do hash que
    /// guarda. Escrever uma segunda aqui seria escrever um segundo jeito de
    /// errar, e as duas nao podem divergir.
    ///
    /// **O que isto NAO impoe.** O aplicativo continua entregando UID/PWD
    /// pelo `SQLConnect`/`SQLDriverConnect` e nao muda uma linha: a garantia
    /// nova e do driver para dentro. E do lado do servidor a operacao
    /// `desafio` e TRES DIAS mais velha que o `cifrar` (commits `0dfcf15`, de
    /// 27/08/2026, e `d3b7d62`, de 30/08), e mais velha que este proprio
    /// driver (`69f6d1e`, 29/08) -- nao existe phxsqld que um ODBC consiga
    /// alcancar e que nao saiba responder ao desafio.
    fn autenticar(&mut self, r: &Receita) -> Result<(), Falha> {
        let d = self.pedir(vec![
            ("op", Json::texto_de("desafio")),
            ("usuario", Json::texto_de(&r.usuario)),
        ])?;
        let nonce = d.texto_ou("nonce", "").to_string();
        let sal = d.texto_ou("sal", "").to_string();
        let iteracoes = d.inteiro_ou("iteracoes", 0).max(0) as u32;
        // Desafio incompleto e ERRO com nome, e nao uma prova calculada com
        // zero iteracoes: essa viraria "credencial invalida" no servidor e
        // mandaria a pessoa conferir a senha, que e mandar procurar no lugar
        // errado.
        if nonce.is_empty() || sal.is_empty() || iteracoes == 0 {
            return Err(Falha::nova(
                "08001",
                "o servidor respondeu ao desafio sem sal, nonce ou iteracoes",
            ));
        }

        // A amarracao ao canal, quando ha tunel: a prova nasce presa a
        // transcricao do aperto e o servidor a confere contra a DELE, o que
        // derruba quem terminou o tunel do cliente e reencaminha. Em claro
        // (`None`) a mensagem e byte a byte a de sempre -- pedida, nao
        // imposta. Mesmo caminho da `replica::Cliente`; ver
        // `docs/CIFRA-DO-FIO.md` §10.
        let transcricao = self.fio.transcricao();
        let canal_ref = transcricao.as_ref().map(|t| &t[..]);

        let nonce_cliente = phxsql_core::desafio::nonce();
        let prova = phxsql_core::desafio::prova_de_senha(
            &r.senha,
            &sal,
            iteracoes,
            &nonce,
            &nonce_cliente,
            &r.usuario,
            canal_ref,
        )
        // Mensagem FIXA: o erro de dentro nasce do sal que o servidor mandou,
        // mas este e o caminho por onde a senha passa, e diagnostico de ODBC
        // vira log de aplicativo. Aqui nao se interpola nada.
        .map_err(|_| Falha::nova("08001", "o sal do desafio nao e hexadecimal"))?;

        let mut campos = vec![
            ("op", Json::texto_de("login")),
            ("usuario", Json::texto_de(&r.usuario)),
            ("prova", Json::texto_de(prova)),
            ("nonce_cliente", Json::texto_de(nonce_cliente)),
        ];
        if canal_ref.is_some() {
            campos.push(("amarrar_canal", Json::Bool(true)));
        }
        self.pedir(campos)?;
        Ok(())
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

        // O TETO vale aqui tambem -- pedido 312, fechado no ODBC pelo 434.
        //
        // Este e o TERCEIRO irmao do `cifrar`: o `replica::Cliente::cifrar` e o
        // `servidor::Cliente::cifrar` ganharam o teto no pedido 312, e este
        // ficou com o `read_line` cru. Irmao e quem chama as mesmas funcoes na
        // mesma ordem, e os tres chamam: manda a mensagem 1 em claro, le a
        // resposta ANTES de o tunel existir e ANTES de qualquer autenticacao.
        // Sem teto, quem escolhe quanta memoria o driver reserva e o servidor
        // do outro lado -- que, no aperto, e justamente quem ainda nao provou
        // ser quem diz. Medido no irmao da replica: 192 MiB numa linha so, em
        // 294 ms.
        let resposta = match self
            .fio
            .ler_ate(&mut self.leitor, phxsql_core::fio::TETO_DO_APERTO)
            .map_err(|e| Falha::nova("08001", format!("lendo o aperto de mao: {e}")))?
        {
            Recebido::Linha(l) => l,
            Recebido::Fim => {
                return Err(Falha::nova(
                    "08001",
                    "o servidor fechou a conexao durante o aperto de mao",
                ))
            }
        };
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
    // So os servidores de MENTIRA deste modulo leem linha crua: eles sao o
    // outro lado do fio, e o teto protege quem RECEBE. O driver de producao
    // le pelo `fio::Canal`, e por isso o `BufRead` saiu de cima.
    use std::io::BufRead as _;

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

    /// Sobe um servidor que responde o aperto com uma linha ACIMA DO TETO.
    ///
    /// Le o `cifrar` e despeja 128 KiB numa linha so -- o dobro do
    /// `TETO_DO_APERTO`. Nao e JSON de proposito: com o teto no lugar ninguem
    /// chega a analisa-la, e com o defeito reposto a reprovacao sai nomeando o
    /// erro que veio no lugar («nao e JSON») em vez de sair por prazo.
    fn servidor_que_estoura_o_aperto() -> u16 {
        let escuta = TcpListener::bind("127.0.0.1:0").expect("bind");
        let porta = escuta.local_addr().unwrap().port();
        let (pronto, espere) = mpsc::channel();
        std::thread::spawn(move || {
            pronto.send(()).ok();
            let Ok((soquete, _)) = escuta.accept() else {
                return;
            };
            let _ = soquete.set_read_timeout(Some(Duration::from_secs(3)));
            let _ = soquete.set_write_timeout(Some(Duration::from_secs(3)));
            let mut escrita = soquete.try_clone().unwrap();
            let mut leitor = BufReader::new(soquete);
            let mut linha = String::new();
            if leitor.read_line(&mut linha).unwrap_or(0) == 0 {
                return;
            }
            let gorda = "A".repeat(2 * phxsql_core::fio::TETO_DO_APERTO as usize);
            // O cano quebra quando o driver recusa e vai embora, e quebrar e o
            // esperado: por isso nada aqui usa `unwrap`.
            let _ = escrita.write_all(gorda.as_bytes());
            let _ = escrita.write_all(b"\n");
            let _ = escrita.flush();
        });
        espere.recv().ok();
        porta
    }

    /// **A resposta do aperto acima do teto e recusada pelo LIMITE.**
    ///
    /// # O defeito que este teste trava
    ///
    /// Pedido 434. Este `cifrar` era o TERCEIRO irmao dos dois que o pedido
    /// 312 consertou (`replica::Cliente::cifrar` e `servidor::Cliente::cifrar`)
    /// e ficou com o `read_line` cru: quem escolhia quanta memoria o driver
    /// reserva era o servidor do outro lado, antes de o tunel existir e antes
    /// de qualquer credencial. Medido no irmao da replica: 192 MiB numa linha
    /// so, em 294 ms.
    ///
    /// Com o defeito reposto a recusa ainda acontece, mas por OUTRO motivo --
    /// «a resposta do aperto de mao nao e JSON», depois de os 128 KiB ja
    /// estarem na memoria. Por isso a assercao e sobre o MOTIVO, e nao sobre
    /// haver falha: falha havia nos dois estados.
    #[test]
    fn a_resposta_do_aperto_acima_do_teto_e_recusada_pelo_limite() {
        let porta = servidor_que_estoura_o_aperto();
        let r = Receita {
            servidor: "127.0.0.1".into(),
            porta,
            cifra: true,
            ..Receita::default()
        };
        let f = match Canal::abrir(&r) {
            Err(f) => f,
            Ok(_) => panic!("o aperto gordo devia ser recusado, e o driver ABRIU"),
        };
        assert_eq!(f.estado, "08001", "{}", f.mensagem);
        assert!(
            f.mensagem.contains("limite excedido"),
            "a recusa devia ser do TETO, e veio: {}",
            f.mensagem
        );
        assert!(
            f.mensagem
                .contains(&phxsql_core::fio::TETO_DO_APERTO.to_string()),
            "a recusa nao traz o teto em bytes: {}",
            f.mensagem
        );
        // E a medida legivel, que antes de hoje dizia «0 MiB» num teto de
        // 64 KiB -- achado B2 da revisao de seguranca de 23/09/2026.
        assert!(f.mensagem.contains("64 KiB"), "{}", f.mensagem);
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

    // --- A prova real do LOGIN: a senha nao atravessa o fio (pedido 275) ---

    /// Iteracoes de PBKDF2 do servidor de mentira.
    ///
    /// Baixas de proposito: o que este teste prova e a FORMA do login, e
    /// 210.000 iteracoes so acrescentariam segundos sem acrescentar garantia.
    const ITERACOES_DE_TESTE: u32 = 64;

    /// Um phxsqld de mentira que GRAVA tudo o que o driver lhe mandou.
    ///
    /// Fala o aperto quando `cifra`, responde `desafio` com o sal e as
    /// iteracoes de um hash de verdade, e confere a prova com o MESMO
    /// `conferir_prova` do servidor -- nao com uma conta reescrita aqui, que
    /// e como um teste passa por engano.
    ///
    /// Devolve as linhas que VIU, ja abertas quando ha tunel: e nelas que o
    /// teste procura a senha, porque o que o pedido 275 cobra nao e so o que
    /// passou pelo fio -- e o que CHEGOU ao servidor. Dentro do tunel a senha
    /// sai do fio e continua sendo a senha.
    fn servidor_de_login(cifra: bool, senha_certa: &str) -> (u16, mpsc::Receiver<Vec<String>>) {
        let guardado = phxsql_core::senha::cifrar_com(senha_certa, ITERACOES_DE_TESTE);
        let estatica = phxsql_core::x25519::gerar_privada();
        let escuta = TcpListener::bind("127.0.0.1:0").expect("bind");
        let porta = escuta.local_addr().unwrap().port();
        let (manda, recebe) = mpsc::channel();
        let (pronto, espere) = mpsc::channel();
        std::thread::spawn(move || {
            pronto.send(()).ok();
            let Ok((soquete, _)) = escuta.accept() else {
                return;
            };
            let _ = soquete.set_read_timeout(Some(Duration::from_secs(5)));
            let mut escrita = soquete.try_clone().unwrap();
            let mut leitor = BufReader::new(soquete);
            let mut fio = FioCanal::Claro;
            let mut transcricao: Option<[u8; 32]> = None;

            if cifra {
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
                transcricao = Some(transporte.transcricao());
                fio = FioCanal::Cifrado(Box::new(transporte));
            }

            let mut vistas: Vec<String> = Vec::new();
            let mut nonce_servidor = String::new();
            while let Ok(Recebido::Linha(l)) = fio.ler(&mut leitor) {
                vistas.push(l.clone());
                let p = Json::analisar(&l).unwrap();
                let op = p.texto_ou("op", "").to_string();
                let resposta = match op.as_str() {
                    "desafio" => {
                        let (sal, it) = phxsql_core::senha::sal_e_iteracoes(&guardado).unwrap();
                        nonce_servidor = phxsql_core::desafio::nonce();
                        Json::objeto(vec![
                            ("ok", Json::Bool(true)),
                            ("op", Json::texto_de("desafio")),
                            (
                                "resultado",
                                Json::objeto(vec![
                                    ("sal", Json::texto_de(phxsql_core::hash::para_hex(&sal))),
                                    ("iteracoes", Json::de_u64(u64::from(it))),
                                    ("nonce", Json::texto_de(&nonce_servidor)),
                                ]),
                            ),
                        ])
                    }
                    "login" => {
                        let dk = phxsql_core::senha::derivado_do_hash(&guardado).unwrap();
                        // A amarracao so entra quando o CLIENTE pede, que e o
                        // que o servidor de verdade faz: sem o campo, a
                        // mensagem e byte a byte a de sempre.
                        let amarra = if p.booleano_ou("amarrar_canal", false) {
                            transcricao.as_ref().map(|t| &t[..])
                        } else {
                            None
                        };
                        let boa = phxsql_core::desafio::conferir_prova(
                            &dk,
                            &nonce_servidor,
                            p.texto_ou("nonce_cliente", ""),
                            p.texto_ou("usuario", ""),
                            amarra,
                            p.texto_ou("prova", ""),
                        );
                        if boa {
                            Json::objeto(vec![
                                ("ok", Json::Bool(true)),
                                ("op", Json::texto_de("login")),
                                (
                                    "resultado",
                                    Json::objeto(vec![("login", Json::texto_de("ana"))]),
                                ),
                            ])
                        } else {
                            Json::objeto(vec![
                                ("ok", Json::Bool(false)),
                                ("op", Json::texto_de("login")),
                                ("erro", Json::texto_de("credencial invalida")),
                            ])
                        }
                    }
                    _ => Json::objeto(vec![
                        ("ok", Json::Bool(true)),
                        ("op", Json::texto_de(&op)),
                        ("resultado", Json::Nulo),
                    ]),
                };
                let _ = fio.escrever(&mut escrita, &resposta.escrever());
                if op == "login" {
                    break;
                }
            }
            manda.send(vistas).ok();
        });
        espere.recv().ok();
        (porta, recebe)
    }

    /// A senha do teste. Tem o FORMATO de uma senha e nao e o valor de
    /// ninguem: o que o teste procura no fio e esta cadeia literal.
    const SENHA_DO_TESTE: &str = "PWD-QUE-NAO-PODE-ATRAVESSAR-O-FIO";

    // O defeito do pedido 275, medido nos DOIS modos do driver: em claro a
    // senha ia no fio, e dentro do tunel ela saia do fio mas chegava inteira
    // ao servidor. Nenhum dos dois pode acontecer.
    #[test]
    fn a_senha_nunca_chega_ao_servidor() {
        for cifra in [false, true] {
            let (porta, viu) = servidor_de_login(cifra, SENHA_DO_TESTE);
            let liga = if cifra { 1 } else { 0 };
            let r = analisar_receita(&format!(
                "Server=127.0.0.1;Port={porta};UID=ana;PWD={SENHA_DO_TESTE};CIFRA={liga}"
            ));
            let aberto = Canal::abrir(&r).is_ok();
            let vistas = viu
                .recv_timeout(Duration::from_secs(10))
                .expect("o servidor de mentira devia devolver o que viu");
            let tudo = vistas.join("\n");
            assert!(
                !tudo.contains(SENHA_DO_TESTE),
                "cifra={cifra}: a senha chegou ao servidor: {tudo}"
            );
            let login = vistas
                .iter()
                .find(|l| l.contains("\"login\""))
                .unwrap_or_else(|| panic!("cifra={cifra}: nenhum login foi mandado: {tudo}"));
            assert!(
                login.contains("\"prova\"") && login.contains("\"nonce_cliente\""),
                "cifra={cifra}: o login tem de ser desafio-resposta: {login}"
            );
            assert!(
                aberto,
                "cifra={cifra}: a prova tinha de conferir no servidor: {tudo}"
            );
        }
    }

    // O IRMAO da replica (`replica.rs`): dentro do tunel a prova nasce presa a
    // transcricao do aperto, e o servidor a confere contra a DELE. O servidor
    // de mentira so confere com a amarracao quando o campo vem, entao este
    // teste so passa se o driver o mandar -- e a conexao cifrada e o padrao.
    #[test]
    fn dentro_do_tunel_a_prova_se_amarra_ao_canal() {
        let (porta, viu) = servidor_de_login(true, SENHA_DO_TESTE);
        let r = analisar_receita(&format!(
            "Server=127.0.0.1;Port={porta};UID=ana;PWD={SENHA_DO_TESTE}"
        ));
        assert!(r.cifra, "a receita de hoje nasce cifrada (pedido 373)");
        Canal::abrir(&r).expect("o login amarrado ao canal devia fechar");
        let vistas = viu
            .recv_timeout(Duration::from_secs(10))
            .expect("o que viu");
        let login = vistas
            .iter()
            .find(|l| l.contains("\"login\""))
            .expect("nenhum login");
        assert!(
            login.contains("\"amarrar_canal\":true"),
            "a prova tem de se amarrar ao tunel: {login}"
        );
    }

    // O COMPORTAMENTO VELHO, e e o teste que mais importa: a connection string
    // que sempre funcionou continua funcionando, com o mesmo UID/PWD e sem
    // uma chave nova. O aplicativo que carrega este `.so` nao muda uma linha
    // -- a garantia nova e do driver para dentro.
    #[test]
    fn a_connection_string_de_sempre_continua_conectando() {
        for receita in [
            "Server=127.0.0.1;Port={p};UID=ana;PWD={s}",
            "Server=127.0.0.1;Port={p};UID=ana;PWD={s};CIFRA=0",
            "Driver=PhxSql;Server=127.0.0.1;Port={p};uid=ana;pwd={s};Database=loja",
        ] {
            let cifra = !receita.contains("CIFRA=0");
            let (porta, _viu) = servidor_de_login(cifra, SENHA_DO_TESTE);
            let texto = receita
                .replace("{p}", &porta.to_string())
                .replace("{s}", SENHA_DO_TESTE);
            let r = analisar_receita(&texto);
            assert!(
                Canal::abrir(&r).is_ok(),
                "«{texto}» tinha de continuar conectando"
            );
        }
    }

    // O outro lado do comportamento velho: sem UID nao ha login NENHUM -- nem
    // desafio. Uma conexao so de token nao pode passar a pedir desafio para um
    // usuario vazio, que e como uma guarda nova vira erro para quem nao a
    // pediu.
    #[test]
    fn sem_usuario_nao_sai_desafio_nem_login() {
        let (porta, viu) = servidor_de_login(false, SENHA_DO_TESTE);
        let r = analisar_receita(&format!("Server=127.0.0.1;Port={porta};Token=t;CIFRA=0"));
        let mut canal = Canal::abrir(&r).expect("conexao so de token devia abrir");
        // Um pedido qualquer, so para o servidor de mentira sair do laco.
        let _ = canal.pedir(vec![("op", Json::texto_de("login"))]);
        let vistas = viu
            .recv_timeout(Duration::from_secs(10))
            .expect("o que viu");
        assert!(
            !vistas.iter().any(|l| l.contains("\"desafio\"")),
            "sem UID nao podia sair desafio: {vistas:?}"
        );
    }

    // Senha errada continua sendo recusada, e com o SQLSTATE que o aplicativo
    // ja trata (28000). Trocar a forma do login nao pode trocar o codigo com
    // que o gerenciador de driver decide pedir a credencial de novo.
    #[test]
    fn senha_errada_continua_28000() {
        let (porta, _viu) = servidor_de_login(false, SENHA_DO_TESTE);
        let r = analisar_receita(&format!(
            "Server=127.0.0.1;Port={porta};UID=ana;PWD=PWD-ERRADA;CIFRA=0"
        ));
        let erro = match Canal::abrir(&r) {
            Ok(_) => panic!("senha errada nao podia conectar"),
            Err(e) => e,
        };
        assert_eq!(erro.estado, "28000", "mensagem: {}", erro.mensagem);
        assert!(
            !erro.mensagem.contains("PWD-ERRADA"),
            "o diagnostico nao pode carregar a senha: {}",
            erro.mensagem
        );
    }
}
