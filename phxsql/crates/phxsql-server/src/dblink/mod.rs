//! DbLink: ligacoes para bancos de fora.
//!
//! O nome vem do Centro de Controle do HFSQL(R), e a ideia e a mesma: guardar
//! um apelido com endereco e credencial, e depois falar com o outro banco por
//! esse apelido, sem repetir endereco em lugar nenhum.
//!
//! # O que uma ligacao NAO carrega
//!
//! Permissao. Uma ligacao guarda UMA credencial, e todo mundo que a usa fala
//! com o outro banco como aquele usuario -- as permissoes por base do PhxSql
//! nao atravessam para o outro lado. Por isso toda operacao de DbLink exige
//! `administrar`: quem usa a ligacao esta usando o poder de quem a criou, e
//! isso nao pode ser um direito de leitor.
//!
//! # Somente leitura vem LIGADO
//!
//! Uma ligacao nasce recusando qualquer coisa que nao seja consulta. Ligar a
//! escrita e uma decisao, e nao um padrao herdado: a mesma tela que lista
//! tabelas de um banco de producao apagaria uma se a escrita viesse ligada por
//! omissao.

#[cfg(test)]
use crate::apoio_teste::DirTemp;
pub mod conexao;
pub mod dialeto;
pub mod mysql;
pub mod operacoes;
pub mod phx;
pub mod sincronia;

pub use conexao::{Conexao, Resultado};

use std::path::{Path, PathBuf};
use std::time::Duration;

use phxsql_core::cifra::{TAG_LEN, XNONCE_LEN};
use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_store::cofre::{Material, SAL_LEN};

use crate::config::{ChaveMestra, CifraDoDblink, Segredo};

/// O formato mais novo do `dblink.json` que este binario le e escreve.
///
/// Sem o campo, o arquivo e o de sempre (formato 1). O 2 e o que carrega a
/// cifra do cadastro (pedido 372). Um numero MAIOR e arquivo escrito por um
/// binario mais novo, e a abertura recusa ALTO: ler adivinhando -- e depois
/// regravar por cima -- apagaria em silencio o que este binario nao entende,
/// que e exatamente o estrago que o binario anterior faria no formato 2 se
/// achasse a lista (ver [`LISTA_DO_FORMATO_2`]). A recusa sobe pelo `?` do
/// arranque: o servidor NAO sobe, e isso esta escrito no MANUAL e no FORMATO.
pub const FORMATO_DO_CADASTRO: u16 = 2;

/// A parte estavel que a prova do material amarra. Um material copiado de
/// outro arquivo cifrado da casa (um `.reg`, um diario) nao passa por prova
/// deste cadastro.
const ROTULO_DA_PROVA: &[u8] = b"phxsql/dblink.json/formato-2";

/// Onde mora a lista de ligacoes no formato 1 -- o de sempre.
const LISTA_DO_FORMATO_1: &str = "dblink";

/// Onde mora a lista de ligacoes no formato 2 -- e ela NAO e a do formato 1,
/// de proposito.
///
/// Um binario anterior a este le `"dblink"`, ignora o que nao conhece e, na
/// primeira gravacao, reescreve as ligacoes SEM os envelopes: a credencial
/// cifrada some de vez, calada (revisao SEC do 372, M3, e parecer do DBA do
/// formato 2, pedido 1). Com a lista noutra chave, o binario anterior nao a
/// acha e RECUSA SUBIR («esperava uma lista de ligacoes, ou um objeto com
/// "dblink"») -- falha fechada, o mesmo preco que este binario ja cobra do
/// formato maior que o dele.
const LISTA_DO_FORMATO_2: &str = "ligacoes";

/// O piso de iteracoes do PBKDF2 do cadastro: o PADRAO da casa, e nao o piso
/// do cofre.
///
/// A prova do material e um oraculo offline para quem tem o `dblink.json`: ela
/// diz se uma senha candidata e a certa sem conectar em lugar nenhum. O piso do
/// cofre (10.000) existe para nao quebrar diario ja gravado; aqui o formato e
/// novo e nao ha legado a preservar, entao nao ha motivo para aceitar 21 vezes
/// menos trabalho por tentativa que o padrao medido (210.000, 290,3 ms). O
/// campo `iteracoes` so SOBE a partir daqui -- um interruptor para descer seria
/// um interruptor para enfraquecer, e os testes pagam o padrao como todo mundo.
pub const ITERACOES_MINIMAS_DO_CADASTRO: u32 = phxsql_store::cofre::ITERACOES_PADRAO;

/// O teto de iteracoes que a abertura aceita do arquivo: 10 vezes o padrao.
///
/// As iteracoes vem do ARQUIVO, e quem escreve nele escolhia quanto o arranque
/// pagava: `u32::MAX` sao 4,29e9 / 210.000 x 290,3 ms, cerca de 99 minutos de
/// PBKDF2 antes de a porta abrir (conta a partir do custo medido, e nao
/// medida). Com o teto, o pior sao ~2,9 s.
pub const ITERACOES_MAXIMAS_DO_CADASTRO: u32 = 10 * phxsql_store::cofre::ITERACOES_PADRAO;

/// O degrau do claro do envelope: a credencial vai com o comprimento na frente
/// (2 bytes) e zeros ate o proximo multiplo de 128.
///
/// Sem o degrau, o cifrado tinha o tamanho exato da credencial -- e «o tamanho
/// ja e informacao» e regra desta casa (revisao SEC do 372, B1). 128 porque
/// toda senha humana e um token de ate 126 bytes caem no MESMO degrau: o
/// cifrado nao separa senha curta de longa, nem senha de token. O preco e
/// fixo e pequeno -- 128 + 40 bytes (nonce e etiqueta) por credencial, 336
/// caracteres hexadecimais, num arquivo lido uma vez por arranque.
const DEGRAU_DO_ENVELOPE: usize = 128;

/// Qual banco esta do outro lado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motor {
    MySql,
    Postgres,
    /// Outro PhxSql, pelo protocolo proprio -- e nao pelo fio do MySQL(R).
    ///
    /// Este e o unico motor que NAO fala SQL para o catalogo: as perguntas do
    /// DbLink viram `bancos`, `sistabelas`, `esquema` e `varrer`, que sao as
    /// mesmas operacoes que a tela ja usa. Ver `dblink::phx`.
    Phx,
}

impl Motor {
    pub fn de_texto(s: &str) -> Result<Motor> {
        Ok(match s.trim().to_lowercase().as_str() {
            "" | "mysql" | "mariadb" => Motor::MySql,
            "postgres" | "postgresql" | "pgsql" => Motor::Postgres,
            "phxsql" | "phx" => Motor::Phx,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "motor de dblink desconhecido: {outro:?} \
                     (use \"mysql\", \"postgres\" ou \"phxsql\")"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Motor::MySql => "mysql",
            Motor::Postgres => "postgres",
            Motor::Phx => "phxsql",
        }
    }

    pub fn porta_padrao(self) -> u16 {
        match self {
            Motor::MySql => 3306,
            Motor::Postgres => 5432,
            Motor::Phx => crate::config::PORTA_PADRAO,
        }
    }

    /// As operacoes do DbLink ja funcionam com este motor?
    ///
    /// **Nao e "ha cliente escrito"**, e essa distincao custou uma versao: o
    /// cliente do PostgreSQL(R) existia, conferido contra o RFC 7677, e este
    /// sinal continuava `false` porque as operacoes montavam SQL de MySQL(R).
    /// Acende-lo ali teria ligado um botao que falha na primeira consulta --
    /// a mesma armadilha do campo de configuracao que ninguem le.
    ///
    /// Agora os dois respondem `true` porque as duas coisas existem: o cliente
    /// em `crate::pg` e o dialeto em `dblink::dialeto`, com as operacoes em
    /// `dblink::operacoes` escolhendo por motor.
    ///
    /// O sinal continua dizendo **o que a tela pode fazer**, e nao o que o
    /// repositorio tem -- e por isso ele fica aqui, num lugar so, em vez de a
    /// tela adivinhar pelo nome do motor.
    pub fn conecta(self) -> bool {
        matches!(self, Motor::MySql | Motor::Postgres | Motor::Phx)
    }

    /// Este motor responde o catalogo em SQL?
    ///
    /// Os dois de fora sim; o PhxSql nao -- ele responde `sistabelas` e
    /// `esquema`, que ja trazem o que o `SHOW FULL COLUMNS` traz e mais. A
    /// pergunta existe para que as operacoes que SO sabem montar SQL recusem
    /// dizendo por que, em vez de mandarem crase para quem nao usa crase.
    pub fn catalogo_em_sql(self) -> bool {
        !matches!(self, Motor::Phx)
    }

    /// Este motor fala o NOSSO aperto de mao, e por isso sabe o tunel cifrado?
    ///
    /// So o PhxSql. O `{"op":"cifrar"}` e uma operacao deste protocolo
    /// (`docs/CIFRA-DO-FIO.md`); contra um MySQL(R) ou um PostgreSQL(R) nao ha
    /// com quem aperta-la, porque quem manda no fio la e o protocolo deles.
    /// Acender o interruptor ali seria um campo de configuracao que nao faz
    /// nada -- a armadilha do `recursos.cache_paginas`, que anunciava um cache
    /// que nenhuma linha de codigo lia.
    ///
    /// E ela decide DUAS coisas, e nao uma: o padrao de fabrica desta ligacao
    /// (ver [`Definicao::cifra`]) e a recusa na declaracao (ver
    /// [`Definicao::conferir_cifra_do_motor`]).
    pub fn cifra_o_fio(self) -> bool {
        matches!(self, Motor::Phx)
    }
}

/// Uma ligacao cadastrada.
#[derive(Clone)]
pub struct Definicao {
    /// Apelido, unico. E por ele que os comandos chamam a ligacao.
    pub nome: String,
    pub motor: Motor,
    pub host: String,
    pub porta: u16,
    pub usuario: String,
    /// PRIVADA, como a do rele de e-mail: o servidor precisa apresenta-la ao
    /// outro banco, entao nao da para guardar so o hash -- mas ela nunca sai
    /// em JSON nem em log.
    ///
    /// Um [`Segredo`], e nao `String`, desde o pedido 372: a variavel de
    /// `senha_env` que falta deixou de virar senha vazia e passou a ser o erro
    /// que a nomeia, na hora de conectar.
    senha: Segredo,
    /// Nome da variavel de ambiente de onde a senha veio, quando veio de la.
    pub senha_env: String,
    /// O token de servico do outro PhxSql -- so o motor `phxsql` o usa.
    ///
    /// PRIVADO pelo mesmo motivo da senha, e o motivo aqui e mais forte: no
    /// PhxSql o token e o portao 1, conferido ANTES do login. Quem o tem
    /// alcanca a porta de dados do outro servidor sem usuario nenhum, entao
    /// ele nunca sai em JSON, em log nem na tela.
    ///
    /// # Por que `token_remoto`, e nao `token`
    ///
    /// Porque `token` JA EXISTE em todo pedido deste protocolo, e e o portao 1
    /// DESTE servidor. Um campo `token` no `dblink_salvar` seria lido primeiro
    /// pelo portao, que compararia o token do OUTRO servidor com o daqui e
    /// responderia «token invalido» -- um erro que manda procurar no lugar
    /// errado, e que nenhum teste de unidade acharia. Foi a prova por soquete
    /// que pisou nele.
    token: Segredo,
    /// Nome da variavel de ambiente de onde o token veio, quando veio de la.
    pub token_env: String,
    pub database: String,
    pub descricao: String,
    pub somente_leitura: bool,
    pub timeout_s: u64,
    pub max_linhas: u64,
    /// Tabelas ligadas por sincronia. Campo ausente no arquivo = nenhuma,
    /// entao todo `dblink.json` escrito antes continua abrindo igual.
    pub sincronias: Vec<sincronia::Sincronia>,
    /// Falar com o outro lado por dentro do tunel cifrado -- em TRES estados,
    /// e nao dois.
    ///
    /// `None` e «ninguem escreveu decisao nenhuma», e nao «claro»: o valor que
    /// vale sai de [`Definicao::cifra`], que le o motor. `Some(_)` e decisao
    /// ESCRITA, e e ela que o `para_disco` guarda e que o salvar pela tela
    /// herda.
    ///
    /// PRIVADO, e nao `pub bool`, porque o zero do tipo e `false`: quem monta
    /// por `..Definicao::default()` -- e ha dois sitios que montam -- pularia
    /// o padrao em silencio, e aqui o padrao e a cifra LIGADA. E a armadilha
    /// que o [`crate::config::CIFRA_DE_SAIDA_PADRAO`] nomeia e que o ODBC ja
    /// pagou no pedido 373.
    cifra: Option<bool>,
    /// A chave publica que se ESPERA do outro PhxSql, em hexadecimal -- o pino.
    ///
    /// Vazia com a cifra ligada e tunel SEM pino: protege da escuta passiva e
    /// nao protege de quem esta no meio, porque o atacante apresenta a chave
    /// dele e nao ha com o que comparar. Mesmo contrato do
    /// `replicacao.origens[].chave_do_fio`.
    ///
    /// `pub` e visivel no `Debug` porque e chave PUBLICA -- esconde-la so
    /// atrapalharia o diagnostico de pino torto. O que ela NAO faz e sair no
    /// `para_json`: ver o motivo la.
    pub chave_do_fio: String,
}

/// `Debug` escrito a mao, pelo mesmo motivo do da [`crate::config::Cifra`]: o
/// derivado imprimiria a senha e o token, e o `Registro` que guarda as
/// ligacoes num `Vec` os despejaria TODOS de uma vez num unico `dbg!`.
///
/// O comentario dos dois campos ja dizia «nunca sai em JSON nem em log», e era
/// o `para_json` que cumpria a promessa -- sozinho. `Debug` e a outra saida, e
/// ela estava aberta: declarar-se resolvido e o que fez ninguem olhar de novo.
///
/// `senha_env` e `token_env` ficam VISIVEIS de proposito: o nome da variavel
/// de ambiente nao e segredo e e o que permite diagnosticar de onde a
/// credencial deveria ter vindo. E nao se mascara com asteriscos do tamanho
/// certo -- o tamanho ja e informacao.
impl std::fmt::Debug for Definicao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Desestruturar SEM `..` e a catraca deste conserto: um campo novo na
        // `Definicao` para de compilar aqui, e quem o acrescentar decide na
        // hora se ele e segredo. Uma lista de campos escrita a mao envelhece
        // calada -- o campo novo simplesmente nao apareceria, e no dia em que
        // ele fosse uma credencial ninguem seria avisado.
        let Definicao {
            nome,
            motor,
            host,
            porta,
            usuario,
            senha: _,
            senha_env,
            token: _,
            token_env,
            database,
            descricao,
            somente_leitura,
            timeout_s,
            max_linhas,
            sincronias,
            cifra,
            chave_do_fio,
        } = self;
        f.debug_struct("Definicao")
            .field("nome", nome)
            .field("motor", motor)
            .field("host", host)
            .field("porta", porta)
            .field("usuario", usuario)
            .field("senha", &"(oculta)")
            .field("senha_env", senha_env)
            .field("token", &"(oculto)")
            .field("token_env", token_env)
            .field("database", database)
            .field("descricao", descricao)
            .field("somente_leitura", somente_leitura)
            .field("timeout_s", timeout_s)
            .field("max_linhas", max_linhas)
            .field("sincronias", sincronias)
            // Os DOIS estados da cifra, de proposito: `cifra` diz se alguem
            // escreveu a decisao (`None` = herdou o padrao do motor) e
            // `cifra_efetiva` diz o que a conexao vai fazer. Imprimir so o
            // segundo deixaria «herdou a virada» e «escreveu a decisao» com a
            // mesma cara -- que e exatamente a diferenca que o `para_disco`
            // existe para nao perder.
            .field("cifra", cifra)
            .field("cifra_efetiva", &self.cifra())
            // Visivel: e chave PUBLICA, e esconde-la trocaria um vazamento
            // que nao existe por um diagnostico cego de pino torto. Mesma
            // escolha do `Debug` da `Origem`.
            .field("chave_do_fio", chave_do_fio)
            .finish()
    }
}

impl Default for Definicao {
    fn default() -> Self {
        Definicao {
            nome: String::new(),
            motor: Motor::MySql,
            host: "127.0.0.1".into(),
            porta: 3306,
            usuario: String::new(),
            senha: Segredo::default(),
            senha_env: String::new(),
            token: Segredo::default(),
            token_env: String::new(),
            database: String::new(),
            descricao: String::new(),
            somente_leitura: true,
            timeout_s: 10,
            max_linhas: 1_000,
            sincronias: Vec::new(),
            // `None`, e nao `false`: o padrao desta ligacao depende do MOTOR,
            // e o `Default` nao sabe qual sera. Quem monta por
            // `..Definicao::default()` herda «ninguem decidiu», que e a
            // verdade, em vez de herdar «claro», que seria rebaixamento.
            cifra: None,
            chave_do_fio: String::new(),
        }
    }
}

impl Definicao {
    /// A ligacao como chega num PEDIDO (`dblink_salvar`) -- e a forma de sempre.
    ///
    /// O envelope cifrado nao vale aqui: ver [`ler_credencial`].
    pub fn de_json(j: &Json) -> Result<Definicao> {
        Definicao::montar(j, None)
    }

    /// A ligacao como esta no `dblink.json`, com a cifra do cadastro para
    /// abrir os envelopes.
    fn do_arquivo(j: &Json, cifra: &CifraDoCadastro) -> Result<Definicao> {
        Definicao::montar(j, Some(cifra))
    }

    fn montar(j: &Json, cifra: Option<&CifraDoCadastro>) -> Result<Definicao> {
        let padrao = Definicao::default();
        let motor = Motor::de_texto(j.texto_ou("motor", "mysql"))?;
        let nome = j.texto_ou("nome", "").trim().to_string();
        validar_nome(&nome)?;
        // A variavel que falta NAO derruba a leitura -- esta funcao le o
        // `dblink.json` inteiro no arranque, e uma ligacao mal configurada
        // recusaria o servidor todo. A falta fica guardada no [`Segredo`], e a
        // ligacao nasce TRANCADA: so ela recusa, na hora de conectar, dizendo
        // qual variavel faltou. Ver `Registro::avisos` para o aviso. O
        // envelope que nao abre segue o MESMO contrato (pedido 372).
        let quem = format!("a ligacao {nome:?} do DbLink");
        let (senha, senha_env) = ler_credencial(j, "senha", "senha_cifrada", &nome, &quem, cifra)?;
        let (token, token_env) = ler_credencial(
            j,
            "token_remoto",
            "token_remoto_cifrado",
            &nome,
            &quem,
            cifra,
        )?;
        // Valor que ninguem reconhece NAO desliga a cifra -- fica em `None`, e
        // `None` vale o padrao do motor. E a regra do `interruptor()` do ODBC
        // (pedido 373), pelo mesmo motivo: com o padrao ligado, um
        // `"cifra": "zero"` mal digitado viraria claro em silencio, e
        // rebaixamento por dedo errado e o que a virada de 18/09 veio acabar.
        let cifra = j.campo("cifra").and_then(Json::booleano);
        let d = Definicao {
            nome,
            motor,
            host: j.texto_ou("host", &padrao.host).trim().to_string(),
            porta: j
                .inteiro_ou("porta", motor.porta_padrao() as i64)
                .clamp(1, 65_535) as u16,
            usuario: j.texto_ou("usuario", "").trim().to_string(),
            senha,
            senha_env,
            token,
            token_env,
            database: j.texto_ou("database", "").trim().to_string(),
            descricao: j.texto_ou("descricao", "").trim().to_string(),
            // Sem o campo, somente leitura. Negar por omissao e a regra do
            // projeto, e aqui ela protege o banco DO OUTRO.
            somente_leitura: j.booleano_ou("somente_leitura", true),
            timeout_s: j.inteiro_ou("timeout_s", padrao.timeout_s as i64).max(1) as u64,
            max_linhas: j
                .inteiro_ou("max_linhas", padrao.max_linhas as i64)
                .clamp(1, 100_000) as u64,
            sincronias: match j.campo("sincronias").and_then(Json::lista) {
                None => Vec::new(),
                Some(l) => l
                    .iter()
                    .map(sincronia::Sincronia::de_json)
                    .collect::<Result<Vec<_>>>()?,
            },
            cifra,
            chave_do_fio: j.texto_ou("chave_do_fio", "").trim().to_string(),
        };
        // A recusa acontece na DECLARACAO, e nao na conexao: uma ligacao nasce
        // uma vez e conecta mil. Aqui ela alcanca o arquivo e a tela de uma
        // vez so, porque `op_dblink_salvar` chama ESTE `de_json`.
        d.conferir_cifra_do_motor()?;
        // Pino torto vira erro AQUI tambem, e nao so na hora de conectar: um
        // hexadecimal errado gravado no cadastro so apareceria na primeira
        // conexao, e ate la a ligacao diria «cifrada com pino» na tela.
        d.pino_do_fio()?;
        Ok(d)
    }

    /// Como a definicao vai para o disco: com a senha, quando ela nao veio do
    /// ambiente. E o unico lugar em que ela e escrita.
    ///
    /// `Result` porque a senha so sai pelo portao do [`Segredo`]. Do arquivo
    /// ele nunca recusa -- so recusa a variavel que faltou, e essa nunca chega
    /// aqui, porque com `senha_env` o disco guarda o NOME. Se um dia chegar, a
    /// gravacao recusa em vez de escrever uma senha vazia por cima da boa.
    ///
    /// Com `selo` (a chave mestra abriu ou criou o material do cadastro), a
    /// credencial vai SELADA em `senha_cifrada`/`token_remoto_cifrado`, e o
    /// claro nao chega ao disco (pedido 372). Sem `selo`, o disco e o de
    /// sempre -- e e o [`Registro`] que decide se sem selo e permitido.
    fn para_disco(&self, selo: Option<&Material>) -> Result<Json> {
        let mut campos = vec![
            ("nome", Json::texto_de(&self.nome)),
            ("motor", Json::texto_de(self.motor.nome())),
            ("host", Json::texto_de(&self.host)),
            ("porta", Json::de_u64(self.porta as u64)),
            ("usuario", Json::texto_de(&self.usuario)),
            ("database", Json::texto_de(&self.database)),
            ("descricao", Json::texto_de(&self.descricao)),
            ("somente_leitura", Json::Bool(self.somente_leitura)),
            ("timeout_s", Json::de_u64(self.timeout_s)),
            ("max_linhas", Json::de_u64(self.max_linhas)),
        ];
        // As DUAS credenciais pelo mesmo caminho: a decisao de quando selar e
        // uma so, e escrita duas vezes uma das copias acabaria gravando em
        // claro o que a outra sela.
        for (segredo, var, campos_do_disco, sempre) in [
            (
                &self.senha,
                &self.senha_env,
                ["senha", "senha_env", "senha_cifrada"],
                true,
            ),
            (
                &self.token,
                &self.token_env,
                ["token_remoto", "token_remoto_env", "token_remoto_cifrado"],
                false,
            ),
        ] {
            let [campo, campo_env, campo_cifrado] = campos_do_disco;
            if !var.is_empty() {
                campos.push((campo_env, Json::texto_de(var)));
                continue;
            }
            // O envelope que nao abriu volta IGUAL: sem a chave nao ha como
            // selar de novo, e perde-lo seria perder a credencial.
            if let Some(envelope) = segredo.envelope_trancado() {
                campos.push((campo_cifrado, Json::texto_de(envelope)));
                continue;
            }
            let valor = segredo.valor()?;
            if valor.is_empty() {
                // Vazio nao tem o que selar -- e um envelope de nada nao teria
                // nem etiqueta que o autenticasse.
                if sempre {
                    campos.push((campo, Json::texto_de("")));
                }
                continue;
            }
            match selo {
                Some(m) => campos.push((
                    campo_cifrado,
                    Json::texto_de(selar_credencial(m, &self.nome, campo, valor)?),
                )),
                None if segredo.veio_selado() => {
                    return Err(PhxError::Esquema(format!(
                        "a credencial {campo} da ligacao {:?} veio CIFRADA do \
                         cadastro, e esta gravacao nao tem com o que selar: \
                         grava-la em claro seria rebaixar o arquivo",
                        self.nome
                    )))
                }
                None => campos.push((campo, Json::texto_de(valor))),
            }
        }
        if !self.sincronias.is_empty() {
            campos.push((
                "sincronias",
                Json::Lista(self.sincronias.iter().map(|s| s.para_json()).collect()),
            ));
        }
        // A cifra so vai para o disco quando DIVERGE do padrao, e o pino so
        // quando existe; para o motor que nao fala o nosso aperto, nenhum dos
        // dois -- campo gravado ali seria campo sem leitor.
        //
        // O motivo e medido e e deste arquivo: `Registro::gravar` reescreve
        // TODAS as ligacoes a cada salvar. Gravar o padrao efetivo fossilizaria,
        // numa edicao de OUTRA ligacao, uma decisao que ninguem tomou -- e
        // mataria a diferenca entre «herdou a virada» e «escreveu a decisao»,
        // que e a unica coisa que separa um aviso util de um aviso perpetuo.
        if self.motor.cifra_o_fio() {
            if self.cifra() != crate::config::CIFRA_DE_SAIDA_PADRAO {
                campos.push(("cifra", Json::Bool(self.cifra())));
            }
            if !self.chave_do_fio.is_empty() {
                campos.push(("chave_do_fio", Json::texto_de(&self.chave_do_fio)));
            }
        }
        Ok(Json::objeto(campos))
    }

    /// Como a definicao aparece na tela e no protocolo: sem a senha, nunca.
    pub fn para_json(&self) -> Json {
        let sincronias = Json::Lista(self.sincronias.iter().map(|s| s.para_json()).collect());
        Json::objeto(vec![
            ("sincronias", sincronias),
            ("nome", Json::texto_de(&self.nome)),
            ("motor", Json::texto_de(self.motor.nome())),
            ("conecta", Json::Bool(self.motor.conecta())),
            ("host", Json::texto_de(&self.host)),
            ("porta", Json::de_u64(self.porta as u64)),
            ("usuario", Json::texto_de(&self.usuario)),
            ("database", Json::texto_de(&self.database)),
            ("descricao", Json::texto_de(&self.descricao)),
            ("somente_leitura", Json::Bool(self.somente_leitura)),
            ("timeout_s", Json::de_u64(self.timeout_s)),
            ("max_linhas", Json::de_u64(self.max_linhas)),
            // A cifra EFETIVA, e nao o que esta escrito: quem le a tela quer
            // saber se esta conexao vai pelo tunel, e nao de onde a decisao
            // veio.
            ("cifra", Json::Bool(self.cifra())),
            // So o FATO de haver pino, NUNCA o pino -- mesma regra do irmao do
            // cluster (`config.rs`): uma lista de quem tem e quem nao tem pino
            // e mapa para atacante, porque diz onde trocar a chave sai barato.
            // E e por isso que a tela nao tem como devolver o pino no salvar,
            // que e o que obriga a heranca em `op_dblink_salvar`.
            ("tem_pino", Json::Bool(!self.chave_do_fio.is_empty())),
            ("senha_env", Json::texto_de(&self.senha_env)),
            ("token_remoto_env", Json::texto_de(&self.token_env)),
            (
                "token_remoto",
                Json::texto_de(self.token.rotulo("(vazio)", "(oculto)", "(do ambiente)")),
            ),
            (
                "senha",
                Json::texto_de(self.senha.rotulo("(vazia)", "(oculta)", "(do ambiente)")),
            ),
            // O NOME das variaveis que faltaram, e so o nome -- que nao e
            // segredo, e ja sai em `senha_env`. Existe para a tela avisar no
            // MOMENTO em que o nome errado e gravado, e nao so no dia do
            // primeiro teste: a tela le a lista, e nao compara o rotulo.
            ("variaveis_ausentes", Json::Lista(self.variaveis_ausentes())),
            // Por que a credencial CIFRADA nao abriu, quando nao abriu (pedido
            // 372): o motivo nomeia a chave ou o envelope, nunca o valor. A
            // tela le este campo, e nao o rotulo `(cifra trancada)`, pela
            // mesma razao da lista de cima.
            (
                "cifra_trancada",
                Json::texto_de(
                    self.senha
                        .trancado_por()
                        .or(self.token.trancado_por())
                        .unwrap_or(""),
                ),
            ),
        ])
    }

    /// As variaveis declaradas que este processo nao tem, pelo nome.
    fn variaveis_ausentes(&self) -> Vec<Json> {
        [
            (&self.senha, &self.senha_env),
            (&self.token, &self.token_env),
        ]
        .into_iter()
        .filter(|(s, _)| s.falta().is_some())
        .map(|(_, var)| Json::texto_de(var))
        .collect()
    }

    /// A senha para APRESENTAR ao outro banco -- ou o erro que nomeia a
    /// variavel de `senha_env` que este processo nao tem.
    ///
    /// E o portao da ligacao TRANCADA: todo caminho que conecta passa por
    /// aqui ou pelo [`Definicao::token_remoto`], e nenhum dos dois devolve o
    /// vazio no lugar da falta.
    pub fn senha(&self) -> Result<&str> {
        self.senha.valor()
    }

    /// O token de servico do outro PhxSql, pelo mesmo portao da senha.
    pub fn token_remoto(&self) -> Result<&str> {
        self.token.valor()
    }

    /// O token cru, SEM portao -- so existe no binario de teste.
    ///
    /// Continua porque um teste do `servidor.rs`, que nao e desta frente,
    /// compara o token herdado com `assert_eq!(d.token(), "TOK")`. Fora do
    /// teste ele nao compila, e e isso que impede um caminho de conexao novo
    /// de ler o vazio da ligacao trancada.
    #[cfg(test)]
    pub fn token(&self) -> &str {
        self.token
            .valor()
            .unwrap_or_else(|e| panic!("token de ligacao trancada: {e}"))
    }

    /// A cifra que VALE para esta ligacao -- e o unico lugar que responde.
    ///
    /// Um metodo so, e nao um campo lido direto, porque a resposta depende do
    /// MOTOR: um `bool` cru na struct daria a quem o lesse a impressao de que
    /// a pergunta ja estava respondida, e cada leitor decidiria o resto por
    /// conta propria. Tres regras, nesta ordem:
    ///
    /// 1. motor que nao fala o nosso aperto nao cifra -- nao ha com quem;
    /// 2. **o pino vence o interruptor**, como no ODBC: quem escreveu o pino
    ///    quer o tunel CONFERIDO, e um `"cifra": false` ao lado seria
    ///    contradicao. A porta para falar claro continua sendo nao escrever
    ///    pino nenhum;
    /// 3. decisao escrita vale como escrita; sem decisao, vale o padrao de
    ///    saida da casa ([`crate::config::CIFRA_DE_SAIDA_PADRAO`], ligado
    ///    desde 18/09/2026).
    pub fn cifra(&self) -> bool {
        if !self.motor.cifra_o_fio() {
            return false;
        }
        if !self.chave_do_fio.trim().is_empty() {
            return true;
        }
        self.cifra.unwrap_or(crate::config::CIFRA_DE_SAIDA_PADRAO)
    }

    /// Alguem ESCREVEU a decisao da cifra nesta ligacao?
    ///
    /// Separado de [`Definicao::cifra`] porque as duas perguntas sao
    /// diferentes: uma e «vai pelo tunel?» e a outra e «alguem escolheu?».
    /// Quem herdou a virada precisa ser avisado; quem escolheu, nao.
    pub fn cifra_escrita(&self) -> Option<bool> {
        self.cifra
    }

    /// Recusa a cifra do fio no motor que nao fala o nosso aperto de mao.
    ///
    /// Molde do [`Definicao::exigir_catalogo_em_sql`], que e a recusa por
    /// motor que ja existe aqui, e pelo mesmo motivo: o campo aceito e o campo
    /// que nao faz nada -- a tela mostraria «cifrada» para uma ligacao que
    /// fala protocolo alheio em claro, e isso e pior que o buraco conhecido.
    ///
    /// A recusa e na DECLARACAO, nao na conexao: uma ligacao nasce uma vez e
    /// conecta mil vezes. Recusar cedo custa um erro lido enquanto se cadastra;
    /// recusar tarde custa um painel mentindo ate o dia da primeira consulta.
    ///
    /// E a mensagem aponta o caminho que EXISTE para esse motor -- VPN ou
    /// tunel de fora --, em vez de so dizer nao.
    pub fn conferir_cifra_do_motor(&self) -> Result<()> {
        if self.motor.cifra_o_fio() {
            return Ok(());
        }
        let campo = if self.cifra == Some(true) {
            "cifra"
        } else if !self.chave_do_fio.trim().is_empty() {
            "chave_do_fio"
        } else {
            return Ok(());
        };
        Err(PhxError::Esquema(format!(
            "{campo} nao vale para o motor {}: a ligacao {:?} fala o protocolo \
             do outro banco, e o aperto de mao cifrado do PhxSql e operacao \
             DESTE protocolo -- so o motor phxsql o tem. Para esse fio, a cifra \
             vem de FORA (VPN ou tunel); tire {campo} da ligacao",
            self.motor.nome(),
            self.nome
        )))
    }

    /// O pino do outro PhxSql, ja em bytes -- ou o erro que diz o que corrigir.
    ///
    /// Mesma disciplina do `Origem::pino_do_fio`: `None` quer dizer «sem
    /// pino», e nao «qualquer chave serve por engano». Hexadecimal torto vira
    /// ERRO em vez de virar `None`, senao um pino escrito errado viraria
    /// silenciosamente um tunel sem pino -- que e exatamente o estrago que o
    /// pino existe para impedir.
    pub fn pino_do_fio(&self) -> Result<Option<[u8; 32]>> {
        if self.chave_do_fio.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(crate::config::chave_de_hex(
            &self.chave_do_fio,
            &format!("dblink[{}].chave_do_fio", self.nome),
        )?))
    }

    /// Recusa a operacao que so sabe falar SQL contra o motor que nao fala.
    ///
    /// A sincronia de tabelas primas monta `SELECT … LIMIT 0`, `CREATE` e
    /// `INSERT … ON DUPLICATE KEY UPDATE` -- SQL de MySQL(R) da primeira a
    /// ultima linha. Entre dois PhxSql a convergencia ja existe e e outra: a
    /// REPLICACAO, que le o diario dos dois lados e por isso propaga exclusao,
    /// coisa que a sincronia do DbLink nao faz por desenho.
    ///
    /// Recusar aqui e a decisao: uma sincronia que rodasse meio caminho entre
    /// dois PhxSql seria pior que nenhuma, porque pareceria a replicacao sem
    /// as garantias dela.
    pub fn exigir_catalogo_em_sql(&self, operacao: &str) -> Result<()> {
        if self.motor.catalogo_em_sql() {
            return Ok(());
        }
        Err(PhxError::Esquema(format!(
            "{operacao} nao vale para o motor {}: a ligacao {:?} aponta para outro \
             PhxSql, e entre dois PhxSql a convergencia e a REPLICACAO nativa \
             (docs/REPLICACAO.md), que propaga exclusao -- a sincronia do DbLink \
             nao propaga",
            self.motor.nome(),
            self.nome
        )))
    }

    /// Esta definicao, com a senha de outra.
    ///
    /// Existe para a tela de edicao: ela nunca RECEBE a senha (o `para_json`
    /// nao a manda), entao nao teria como devolve-la, e sem isto mudar a porta
    /// apagaria a credencial.
    pub fn com_a_senha_de(mut self, outra: &Definicao) -> Definicao {
        self.senha = outra.senha.clone();
        self.senha_env = outra.senha_env.clone();
        self
    }

    /// Esta definicao, com o token de outra.
    ///
    /// Separada do `com_a_senha_de` de proposito, e nao por simetria: as duas
    /// credenciais chegam em campos diferentes do pedido, entao quem troca so
    /// a senha nao pode perder o token. Juntar as duas numa funcao so faria a
    /// condicao de UMA decidir pelas DUAS -- e o campo esquecido seria apagado
    /// em silencio, que e exatamente o estrago que estas funcoes impedem.
    pub fn com_o_token_de(mut self, outra: &Definicao) -> Definicao {
        self.token = outra.token.clone();
        self.token_env = outra.token_env.clone();
        self
    }

    /// Herda as tabelas ligadas de uma definicao anterior.
    ///
    /// Mesmo desenho do `com_a_senha_de`, pela mesma armadilha: a tela salva a
    /// ligacao sem mandar as sincronias, e um salvar comum nao pode apagar o
    /// que o assistente montou.
    pub fn com_as_sincronias_de(mut self, outra: &Definicao) -> Definicao {
        self.sincronias = outra.sincronias.clone();
        self
    }

    /// Esta definicao, com a DECISAO da cifra de outra.
    ///
    /// Herda o `Option`, e nao o valor efetivo: quem tinha escrito `false`
    /// continua com `false` escrito, e quem nunca escreveu continua sem
    /// escrever. Gravar o efetivo aqui fossilizaria a virada num salvar que
    /// nao falava de cifra nenhuma.
    ///
    /// Separada do [`Definicao::com_o_pino_de`] pelo mesmo motivo que separa
    /// `com_o_token_de` de `com_a_senha_de`: os dois campos chegam no pedido
    /// de forma diferente, e uma condicao so decidindo pelos dois apagaria em
    /// silencio o que ela nao olhou.
    pub fn com_a_cifra_de(mut self, outra: &Definicao) -> Definicao {
        self.cifra = outra.cifra;
        self
    }

    /// Esta definicao, com o pino de outra.
    ///
    /// E a heranca que impede o rebaixamento SILENCIOSO do salvar pela tela: o
    /// `para_json` so devolve `tem_pino` (o pino em si e mapa para atacante),
    /// entao a tela nao tem como mandar o pino de volta. Sem isto, todo salvar
    /// apagaria o pino e a ligacao continuaria anunciando «cifrada» -- tunel
    /// sem ancora, painel identico.
    pub fn com_o_pino_de(mut self, outra: &Definicao) -> Definicao {
        self.chave_do_fio = outra.chave_do_fio.clone();
        self
    }

    /// Abre a ligacao pelo cliente MySQL(R), com o tipo concreto.
    ///
    /// O caminho normal e [`Definicao::abrir`], que devolve a conexao comum aos
    /// dois motores. Este continua existindo para quem precisa do tipo
    /// concreto -- o teste de protocolo, e quem quiser um campo que so o
    /// MySQL(R) tem.
    pub fn conectar(&self) -> Result<mysql::Conexao> {
        match self.motor {
            Motor::MySql => mysql::Conexao::abrir(
                &self.host,
                self.porta,
                &self.usuario,
                self.senha()?,
                &self.database,
                Duration::from_secs(self.timeout_s),
            ),
            Motor::Postgres => Err(PhxError::Esquema(
                "esta ligacao e PostgreSQL(R): use `conectar_pg`, ou `abrir`, \
                 que escolhe o cliente pelo motor da ligacao"
                    .into(),
            )),
            Motor::Phx => Err(PhxError::Esquema(
                "esta ligacao e PhxSql: use `abrir`, que escolhe o cliente pelo \
                 motor -- o outro PhxSql fala o protocolo proprio, nao o fio do MySQL(R)"
                    .into(),
            )),
        }
    }

    /// Abre a ligacao pelo cliente PostgreSQL(R).
    pub fn conectar_pg(&self) -> Result<crate::pg::Conexao> {
        match self.motor {
            Motor::Postgres => crate::pg::Conexao::abrir(
                &self.host,
                self.porta,
                &self.usuario,
                self.senha()?,
                &self.database,
                Duration::from_secs(self.timeout_s),
            ),
            Motor::MySql => Err(PhxError::Esquema(
                "esta ligacao e MySQL(R); use `conectar`".into(),
            )),
            Motor::Phx => Err(PhxError::Esquema(
                "esta ligacao e PhxSql; use `abrir`".into(),
            )),
        }
    }
}

/// O apelido vira parte de comando e de caminho, entao nao pode ser qualquer
/// coisa.
fn validar_nome(nome: &str) -> Result<()> {
    if nome.is_empty() {
        return Err(PhxError::Esquema("dblink sem nome".into()));
    }
    if nome.len() > 40 {
        return Err(PhxError::Esquema(format!(
            "nome de dblink longo demais: {nome:?} (maximo 40)"
        )));
    }
    if !nome
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(PhxError::Esquema(format!(
            "nome de dblink com caractere que nao vale: {nome:?} (use letras, numeros, _ e -)"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// A cifra do cadastro (pedido 372)
// ---------------------------------------------------------------------------

/// Le uma credencial do arquivo ou do pedido: `campo`, `campo_env` -- pelo
/// [`Segredo::ler`], o unico leitor de segredo da casa -- ou `campo_cifrado`,
/// o envelope selado com a chave mestra.
///
/// # O envelope so vale no ARQUIVO
///
/// Num pedido (`cifra` = `None`) ele e recusado nomeando o campo: o cliente
/// manda a credencial, e quem sela e o servidor, ao gravar. Aceitar o envelope
/// pelo protocolo deixaria um cliente plantar no cadastro um texto que ninguem
/// conferiu que abre.
///
/// # Mutuamente exclusivos
///
/// `senha_cifrada` ao lado de `senha` ou de `senha_env` e arquivo editado pela
/// metade, e a leitura NAO escolhe calada qual vale: recusa, nomeando os dois.
fn ler_credencial(
    j: &Json,
    campo: &str,
    campo_cifrado: &str,
    nome: &str,
    quem: &str,
    cifra: Option<&CifraDoCadastro>,
) -> Result<(Segredo, String)> {
    let Some(envelope) = j.campo(campo_cifrado) else {
        return Ok(Segredo::ler(j, campo, quem));
    };
    let Some(cifra) = cifra else {
        return Err(PhxError::Esquema(format!(
            "{campo_cifrado} e campo do dblink.json, e nao do pedido: mande \
             {campo} (ou {campo}_env), e o servidor a sela ao gravar quando ha \
             chave mestra"
        )));
    };
    for vizinho in [campo.to_string(), format!("{campo}_env")] {
        if j.campo(&vizinho).is_some() {
            return Err(PhxError::Esquema(format!(
                "a ligacao {nome:?} traz {campo_cifrado} E {vizinho}: os dois sao \
                 mutuamente exclusivos, e a leitura nao escolhe calada qual vale -- \
                 apague um dos dois"
            )));
        }
    }
    let Some(texto) = envelope.texto() else {
        return Err(PhxError::Corrompido(format!(
            "{campo_cifrado} da ligacao {nome:?} nao e texto"
        )));
    };
    Ok((cifra.abrir_envelope(nome, campo, texto), String::new()))
}

/// O dado associado de um envelope: o NOME da ligacao e o do CAMPO.
///
/// # Por que os dois
///
/// Sem o nome, mover o envelope da ligacao A para a linha da ligacao B dentro
/// do mesmo arquivo e edicao de texto -- e a B passa a apresentar ao banco
/// dela a senha de outro banco. Sem o campo, a senha e o token da MESMA
/// ligacao trocam de lugar. O nome vai em minusculas porque o apelido e unico
/// sem distinguir caixa (`conferir_repetidos`): trocar `ERP` por `erp` pela
/// tela nao pode trancar a ligacao. O prefixo separa este envelope de qualquer
/// outro que a casa um dia selar com a mesma chave.
fn dado_associado(nome: &str, campo: &str) -> Vec<u8> {
    let mut aad = b"phxsql/dblink\0".to_vec();
    aad.extend_from_slice(nome.to_ascii_lowercase().as_bytes());
    aad.push(0);
    aad.extend_from_slice(campo.as_bytes());
    aad
}

/// Sela uma credencial: `hex(nonce || cifrado || etiqueta)`, nonce sorteado a
/// cada gravacao.
///
/// O nonce e sorteado, e nao contado, porque `Registro::gravar` reescreve o
/// arquivo inteiro a cada salvar: nao ha contador que sobreviva a isso sem um
/// segundo estado para guardar -- e com 192 bits, repetir exige colisao de
/// aniversario. E o mesmo argumento do nonce estendido do cofre.
fn selar_credencial(m: &Material, nome: &str, campo: &str, claro: &str) -> Result<String> {
    let mut nonce = [0u8; XNONCE_LEN];
    nonce.copy_from_slice(&phxsql_core::senha::bytes_aleatorios(XNONCE_LEN));
    let mut envelope = nonce.to_vec();
    envelope.extend_from_slice(&m.selar(
        &nonce,
        &dado_associado(nome, campo),
        &no_degrau(claro.as_bytes(), nome, campo)?,
    ));
    Ok(phxsql_core::hash::para_hex(&envelope))
}

/// O claro no degrau: `comprimento u16 LE || credencial || zeros`, ate o
/// proximo multiplo de [`DEGRAU_DO_ENVELOPE`].
fn no_degrau(claro: &[u8], nome: &str, campo: &str) -> Result<Vec<u8>> {
    let n = u16::try_from(claro.len()).map_err(|_| {
        PhxError::Esquema(format!(
            "a credencial {campo} da ligacao {nome:?} tem {} bytes, e o envelope \
             guarda ate {}",
            claro.len(),
            u16::MAX
        ))
    })?;
    let total = (2 + claro.len()).div_ceil(DEGRAU_DO_ENVELOPE) * DEGRAU_DO_ENVELOPE;
    let mut v = Vec::with_capacity(total);
    v.extend_from_slice(&n.to_le_bytes());
    v.extend_from_slice(claro);
    v.resize(total, 0);
    Ok(v)
}

/// A credencial de dentro do degrau -- `None` quando o comprimento escrito nao
/// cabe no que abriu.
fn fora_do_degrau(aberto: &[u8]) -> Option<&[u8]> {
    let n = u16::from_le_bytes([*aberto.first()?, *aberto.get(1)?]) as usize;
    aberto.get(2..2 + n)
}

/// Abre um envelope, ou devolve o MOTIVO de nao abrir -- que vira a tranca da
/// ligacao, e nao erro de quem abre o cadastro.
fn abrir_credencial(
    m: &Material,
    nome: &str,
    campo: &str,
    envelope: &str,
) -> std::result::Result<String, String> {
    let torto = |porque: &str| {
        format!(
            "o envelope de {campo} da ligacao {nome:?} {porque}. A ligacao fica \
             TRANCADA; salve a credencial de novo pela tela"
        )
    };
    let bytes = phxsql_core::hash::de_hex(envelope.trim())
        .ok_or_else(|| torto("nao e hexadecimal valido"))?;
    if bytes.len() <= XNONCE_LEN + TAG_LEN {
        return Err(torto("e curto demais para nonce, texto e etiqueta"));
    }
    let (nonce, selado) = bytes.split_at(XNONCE_LEN);
    let mut n = [0u8; XNONCE_LEN];
    n.copy_from_slice(nonce);
    // O erro do cofre NAO entra no motivo, e de proposito: ele diz «a chave de
    // "cifra" nao e a que gravou», que aqui mandaria procurar a senha do cofre
    // dos diarios. A prova do material ja conferiu a chave mestra; o que sobra
    // quando a etiqueta nao confere e o envelope fora do lugar.
    let claro = m
        .abrir(&n, &dado_associado(nome, campo), selado, "dblink.json")
        .map_err(|_| {
            torto(
                "nao abre com a chave que abriu o resto do cadastro: ele foi \
                 MOVIDO de outra ligacao, o nome foi editado a mao, ou o texto \
                 foi alterado",
            )
        })?;
    let claro = fora_do_degrau(&claro)
        .ok_or_else(|| torto("abriu, mas o comprimento escrito nao cabe no degrau"))?;
    String::from_utf8(claro.to_vec()).map_err(|_| torto("abriu, mas nao e texto UTF-8"))
}

/// O material do cadastro como esta ESCRITO no arquivo.
///
/// Guardado assim, e nao so aberto, porque o arquivo cifrado cuja chave nao
/// abriu tem de voltar ao disco IGUAL: sem ele a proxima gravacao perderia o
/// sal, e com o sal as credenciais de todas as ligacoes trancadas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MaterialEscrito {
    sal: [u8; SAL_LEN],
    iteracoes: u32,
    prova: [u8; TAG_LEN],
}

impl MaterialEscrito {
    fn de_json(j: &Json, caminho: &Path) -> Result<MaterialEscrito> {
        let torto = |porque: String| {
            PhxError::Corrompido(format!(
                "{}: \"cifra_do_cadastro\" {porque}",
                caminho.display()
            ))
        };
        let modo = j.texto_ou("modo", "");
        if modo != "aead" {
            return Err(torto(format!(
                "declara o modo {modo:?}, e este binario so sela o cadastro em \"aead\""
            )));
        }
        let hex = |campo: &str, n: usize| -> Result<Vec<u8>> {
            match phxsql_core::hash::de_hex(j.texto_ou(campo, "")) {
                Some(b) if b.len() == n => Ok(b),
                _ => Err(torto(format!(
                    "traz \"{campo}\" que nao e hexadecimal de {n} bytes"
                ))),
            }
        };
        let mut sal = [0u8; SAL_LEN];
        sal.copy_from_slice(&hex("sal", SAL_LEN)?);
        let mut prova = [0u8; TAG_LEN];
        prova.copy_from_slice(&hex("prova", TAG_LEN)?);
        let iteracoes = j.campo("iteracoes").and_then(Json::inteiro).unwrap_or(-1);
        let (piso, teto) = (ITERACOES_MINIMAS_DO_CADASTRO, ITERACOES_MAXIMAS_DO_CADASTRO);
        // Conferido AQUI, antes de qualquer PBKDF2: o teto existe para que
        // quem escreve no arquivo nao escolha quanto o arranque paga.
        if !(piso as i64..=teto as i64).contains(&iteracoes) {
            return Err(torto(format!(
                "declara {iteracoes} iteracoes, fora da faixa ({piso} a {teto})"
            )));
        }
        Ok(MaterialEscrito {
            sal,
            iteracoes: iteracoes as u32,
            prova,
        })
    }

    fn para_json(&self) -> Json {
        Json::objeto(vec![
            (
                "sal",
                Json::texto_de(phxsql_core::hash::para_hex(&self.sal)),
            ),
            ("iteracoes", Json::de_u64(self.iteracoes as u64)),
            ("modo", Json::texto_de("aead")),
            (
                "prova",
                Json::texto_de(phxsql_core::hash::para_hex(&self.prova)),
            ),
        ])
    }
}

/// O estado da cifra de UM cadastro: a chave que o processo tem, e o que o
/// arquivo carrega.
#[derive(Default)]
struct CifraDoCadastro {
    /// A chave mestra, resolvida na abertura.
    chave: ChaveMestra,
    /// De onde ela foi declarada, para as mensagens -- nunca o valor.
    declarada_em: String,
    /// Iteracoes do material NOVO.
    iteracoes: u32,
    /// O material como esta no arquivo; `None` no cadastro em claro.
    escrito: Option<MaterialEscrito>,
    /// O material aberto pela chave -- e com ele que se sela e se abre.
    aberto: Option<Material>,
    /// Por que o material escrito NAO abriu (sem chave, chave errada).
    trancado: Option<String>,
    /// As ligacoes cuja credencial estava em TEXTO PURO no disco, pelo nome em
    /// minusculas. E o que a migracao conta e DIZ, separado das credenciais
    /// novas que ja nascem seladas: essas nunca estiveram em claro em backup
    /// nenhum, e dizer que «sairam do texto puro» seria mentir para cima.
    em_claro_no_disco: Vec<String>,
}

/// `Debug` a mao: a [`ChaveMestra`] ja se redige, e o material tambem, mas o
/// campo novo tem de parar de compilar aqui.
impl std::fmt::Debug for CifraDoCadastro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let CifraDoCadastro {
            chave,
            declarada_em,
            iteracoes,
            escrito,
            aberto,
            trancado,
            em_claro_no_disco,
        } = self;
        f.debug_struct("CifraDoCadastro")
            .field("chave", chave)
            .field("declarada_em", declarada_em)
            .field("iteracoes", iteracoes)
            .field("escrito", &escrito.is_some())
            .field("aberto", &aberto.is_some())
            .field("trancado", trancado)
            .field("em_claro_no_disco", em_claro_no_disco)
            .finish()
    }
}

impl CifraDoCadastro {
    fn de(config: &CifraDoDblink) -> CifraDoCadastro {
        CifraDoCadastro {
            chave: config.chave(),
            declarada_em: config.declarada_em(),
            iteracoes: config.iteracoes,
            ..CifraDoCadastro::default()
        }
    }

    /// Abre o material escrito com a chave deste processo -- ou guarda POR QUE
    /// nao abriu. Nunca erro: a chave que falta tranca as ligacoes cifradas, e
    /// nao o arranque.
    fn abrir_material(&mut self, escrito: MaterialEscrito, caminho: &Path) {
        self.escrito = Some(escrito);
        let onde = caminho.display();
        let motivo = match &self.chave {
            ChaveMestra::NaoDeclarada => format!(
                "o cadastro do DbLink em {onde} esta CIFRADO e este config.json nao \
                 declara a chave mestra: preencha \"cifra_do_dblink\" com \
                 senha_mestra_env, senha_mestra_arquivo, chave_mestra_env ou \
                 chave_mestra_arquivo -- a MESMA chave que cifrou o arquivo"
            ),
            ChaveMestra::Indisponivel(m) => format!(
                "o cadastro do DbLink em {onde} esta CIFRADO e a chave mestra nao \
                 esta disponivel: {m}"
            ),
            chave => {
                let Some(fora) = chave.de_fora() else {
                    return;
                };
                match Material::de_partes(
                    &escrito.sal,
                    escrito.iteracoes,
                    &escrito.prova,
                    ROTULO_DA_PROVA,
                    &onde.to_string(),
                    &fora,
                    &format!("a chave mestra de {}", self.declarada_em),
                ) {
                    Ok(m) => {
                        self.aberto = Some(m);
                        return;
                    }
                    Err(e) => e.corpo(),
                }
            }
        };
        self.trancado = Some(motivo);
    }

    /// Abre o envelope de uma credencial -- ou a devolve TRANCADA, guardando o
    /// envelope para voltar igual ao disco.
    fn abrir_envelope(&self, nome: &str, campo: &str, envelope: &str) -> Segredo {
        let Some(m) = &self.aberto else {
            let motivo = self.trancado.clone().unwrap_or_else(|| {
                format!(
                    "{campo} da ligacao {nome:?} esta cifrada, e o arquivo nao traz \
                     \"cifra_do_cadastro\": sem o sal nao ha chave que a abra"
                )
            });
            return Segredo::trancado(motivo, envelope.to_string());
        };
        match abrir_credencial(m, nome, campo, envelope) {
            Ok(claro) => Segredo::aberto_do_envelope(claro),
            Err(motivo) => Segredo::trancado(motivo, envelope.to_string()),
        }
    }

    fn chave_disponivel(&self) -> bool {
        self.chave.de_fora().is_some()
    }
}

/// O que uma gravacao do cadastro fez com a cifra -- para quem gravou DIZER.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Gravacao {
    /// Ligacoes cuja credencial estava em TEXTO PURO no disco e saiu dele
    /// nesta gravacao, selada.
    pub ligacoes_cifradas: usize,
    /// O arquivo gravado e cifrado (formato 2).
    pub cifrado: bool,
}

impl Gravacao {
    /// Os campos que as respostas de protocolo acrescentam, os mesmos em toda
    /// operacao que grava o cadastro.
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("cifrado", Json::Bool(self.cifrado)),
            (
                "ligacoes_cifradas_agora",
                Json::de_u64(self.ligacoes_cifradas as u64),
            ),
        ])
    }
}

/// O cadastro inteiro, lido e gravado num arquivo so.
#[derive(Debug, Default)]
pub struct Registro {
    pub caminho: PathBuf,
    pub ligacoes: Vec<Definicao>,
    /// A cifra do cadastro -- privada: quem grava passa por [`Registro::salvar`]
    /// e [`Registro::excluir`], que decidem selar.
    cifra: CifraDoCadastro,
}

impl Registro {
    /// Le o arquivo SEM chave mestra -- o cadastro de sempre. O cifrado abre
    /// com as ligacoes cifradas trancadas, dizendo que falta a chave.
    pub fn abrir(caminho: &Path) -> Result<Registro> {
        Registro::abrir_com(caminho, &CifraDoDblink::default())
    }

    /// Le o arquivo. Arquivo que nao existe e cadastro vazio, e nao erro: um
    /// servidor sem nenhuma ligacao e o caso normal.
    ///
    /// A chave mestra que falta, ou que e a errada, NAO e erro: as ligacoes
    /// cifradas nascem trancadas, com o motivo, e o resto segue (pedido 372).
    /// Erro continua sendo o arquivo torto -- e o formato mais novo que este
    /// binario, que ele recusa alto em vez de regravar por cima.
    pub fn abrir_com(caminho: &Path, cifra: &CifraDoDblink) -> Result<Registro> {
        let mut r = Registro {
            caminho: caminho.to_path_buf(),
            ligacoes: Vec::new(),
            cifra: CifraDoCadastro::de(cifra),
        };
        let Ok(texto) = std::fs::read_to_string(caminho) else {
            return Ok(r);
        };
        if texto.trim().is_empty() {
            return Ok(r);
        }
        let j = Json::analisar(&texto)?;
        let formato = match j.campo("formato") {
            None => 1,
            Some(f) => match f.inteiro() {
                Some(n) if (1..=FORMATO_DO_CADASTRO as i64).contains(&n) => n,
                Some(n) if n > FORMATO_DO_CADASTRO as i64 => {
                    return Err(PhxError::VersaoNaoSuportada {
                        arquivo: caminho.display().to_string(),
                        encontrada: n.min(u16::MAX as i64) as u16,
                        suportada: FORMATO_DO_CADASTRO,
                    })
                }
                _ => {
                    return Err(PhxError::Corrompido(format!(
                        "{}: \"formato\" {} nao e um numero de formato",
                        caminho.display(),
                        f.escrever()
                    )))
                }
            },
        };
        if let Some(m) = j.campo("cifra_do_cadastro") {
            // O material num arquivo que nao se declara formato 2 e a lista no
            // lugar onde o binario anterior a acharia -- e apagaria os
            // envelopes na primeira gravacao dele.
            if formato < 2 {
                return Err(PhxError::Corrompido(format!(
                    "{}: \"cifra_do_cadastro\" num arquivo sem \"formato\": 2 -- \
                     o cadastro cifrado guarda a lista em \"{LISTA_DO_FORMATO_2}\"",
                    caminho.display()
                )));
            }
            let escrito = MaterialEscrito::de_json(m, caminho)?;
            r.cifra.abrir_material(escrito, caminho);
        }
        let lista = lista_do_arquivo(&j, formato, caminho)?;
        for item in lista {
            r.ligacoes.push(Definicao::do_arquivo(item, &r.cifra)?);
        }
        r.conferir_repetidos()?;
        r.cifra.em_claro_no_disco = em_claro(&r.ligacoes);
        Ok(r)
    }

    /// O cadastro esta cifrado no disco?
    pub fn cifrado(&self) -> bool {
        self.cifra.escrito.is_some()
    }

    /// O estado da cifra do cadastro, para a tela: se o arquivo e cifrado, de
    /// onde a chave vem e por que nao abriu -- nunca a chave.
    pub fn estado_da_cifra(&self) -> Json {
        let chave = match &self.cifra.chave {
            ChaveMestra::NaoDeclarada => "nao_declarada",
            ChaveMestra::Indisponivel(_) => "indisponivel",
            _ => "disponivel",
        };
        let motivo = match (&self.cifra.trancado, &self.cifra.chave) {
            (Some(m), _) => m.clone(),
            (None, ChaveMestra::Indisponivel(m)) => m.clone(),
            _ => String::new(),
        };
        Json::objeto(vec![
            ("cifrado", Json::Bool(self.cifrado())),
            ("chave", Json::texto_de(chave)),
            ("declarada_em", Json::texto_de(&self.cifra.declarada_em)),
            ("trancado", Json::texto_de(motivo)),
        ])
    }

    /// Os avisos do arranque sobre este cadastro (pedido 372). Quem os junta
    /// e o `Config::ler`, na mesma lista que o `main` ja imprime.
    ///
    /// # Para quem cada aviso fala
    ///
    /// **A variavel que falta**, um por falta: a ligacao esta TRANCADA e o
    /// operador precisa saber antes do primeiro teste, e nao por ele.
    ///
    /// **A cifra que nao abriu**: um aviso so para o arquivo inteiro quando a
    /// chave falta ou e a errada (o motivo e o mesmo para todas), e um por
    /// ligacao quando a chave abriu o cadastro e so aquele envelope nao.
    ///
    /// **A chave declarada e indisponivel** com o cadastro ainda em claro:
    /// nada se cifra, e a gravacao recusa credencial nova em texto puro.
    ///
    /// **A credencial em texto puro**, um so para o arquivo inteiro. Fala com
    /// quem NAO registrou decisao -- a ligacao com senha ou token escritos no
    /// arquivo -- e cala para quem escreveu `senha_env`/`token_remoto_env` ou
    /// cifrou, que e o molde do aviso das saidas cifradas (`config.rs`): aviso
    /// que aparece para sempre numa instalacao decidida e aviso que ninguem
    /// le. E nao ha escape que o cale mantendo o claro, de proposito: a petrea
    /// e «senha nunca em texto puro», e um interruptor para calar seria
    /// registrar a decisao de quebra-la.
    ///
    /// Nunca a senha, nem pedaco, nem o tamanho: o aviso nomeia a ligacao e o
    /// CAMPO, que e o que diz onde mexer.
    pub fn avisos(&self) -> Vec<String> {
        let mut avisos: Vec<String> = Vec::new();
        let mut em_claro: Vec<String> = Vec::new();
        let mut trancadas: Vec<String> = Vec::new();
        for l in &self.ligacoes {
            for (segredo, campo) in [(&l.senha, "senha"), (&l.token, "token_remoto")] {
                if let Some(falta) = segredo.falta() {
                    avisos.push(format!(
                        "{falta}. A ligacao fica TRANCADA -- recusa conectar ate o \
                         servidor subir com a variavel --, e o resto do servidor \
                         sobe normalmente."
                    ));
                }
                if let Some(motivo) = segredo.trancado_por() {
                    if self.cifra.trancado.is_some() {
                        trancadas.push(format!("{:?} ({campo})", l.nome));
                    } else {
                        avisos.push(format!("{motivo}. O resto do servidor sobe normalmente."));
                    }
                }
                if segredo.escrito_no_arquivo() {
                    em_claro.push(format!("{:?} ({campo})", l.nome));
                }
            }
        }
        if let Some(motivo) = &self.cifra.trancado {
            avisos.push(format!(
                "{motivo}. Ficam TRANCADAS -- recusam conectar ate o servidor subir \
                 com a chave --: {}. O resto do servidor sobe normalmente, e a \
                 proxima gravacao do cadastro devolve os envelopes ao disco IGUAIS.",
                if trancadas.is_empty() {
                    "nenhuma credencial".to_string()
                } else {
                    trancadas.join(", ")
                }
            ));
        } else if let ChaveMestra::Indisponivel(m) = &self.cifra.chave {
            avisos.push(format!(
                "a chave mestra do DbLink ({}) esta declarada e INDISPONIVEL: {m}. \
                 Nada e cifrado enquanto ela faltar, e o cadastro RECUSA gravar \
                 credencial em texto puro no lugar.",
                self.cifra.declarada_em
            ));
        }
        if !em_claro.is_empty() {
            let saida = if self.cifra.chave_disponivel() {
                format!(
                    "A chave mestra ({}) esta disponivel: a PROXIMA gravacao do \
                     cadastro -- salvar qualquer ligacao pela tela ou pelo \
                     dblink_salvar -- sela as credenciais; ate la o texto puro \
                     continua no disco.",
                    self.cifra.declarada_em
                )
            } else {
                "Tire-a de la escrevendo senha_env / token_remoto_env com o nome \
                 de uma variavel de ambiente -- a tela de Definicoes do DbLink \
                 oferece os dois --, ou declarando a chave mestra em \
                 \"cifra_do_dblink\" (docs/DBLINK.md), que sela as credenciais no \
                 proprio arquivo."
                    .to_string()
            };
            avisos.push(format!(
                "o cadastro do DbLink em {} guarda credencial do outro banco em \
                 TEXTO PURO: {}. O arquivo nasce 0600, mas vai inteiro em toda \
                 copia de backup e em todo disco levado. {saida} E TROQUE a \
                 credencial no outro banco: a que ja esteve neste disco continua \
                 recuperavel nos blocos antigos e nas copias.",
                self.caminho.display(),
                em_claro.join(", ")
            ));
        }
        avisos
    }

    fn conferir_repetidos(&self) -> Result<()> {
        let mut vistos = std::collections::HashSet::new();
        for l in &self.ligacoes {
            if !vistos.insert(l.nome.to_lowercase()) {
                return Err(PhxError::Esquema(format!(
                    "duas ligacoes com o nome {:?}: o apelido tem de ser unico",
                    l.nome
                )));
            }
        }
        Ok(())
    }

    pub fn achar(&self, nome: &str) -> Result<&Definicao> {
        self.ligacoes
            .iter()
            .find(|l| l.nome.eq_ignore_ascii_case(nome))
            .ok_or_else(|| PhxError::NaoEncontrado(format!("dblink {nome:?} nao existe")))
    }

    /// Grava ou substitui uma ligacao. Substituir pelo nome e o que faz a tela
    /// de edicao funcionar sem um identificador a mais.
    ///
    /// A lista nova so entra na memoria se o DISCO a aceitou: a gravacao
    /// recusada (a chave que faltou, o disco cheio) deixaria na memoria uma
    /// ligacao que o proximo arranque nao ve -- e que conecta ate la.
    pub fn salvar(&mut self, d: Definicao) -> Result<Gravacao> {
        let mut novas = self.ligacoes.clone();
        match novas
            .iter()
            .position(|l| l.nome.eq_ignore_ascii_case(&d.nome))
        {
            Some(i) => novas[i] = d,
            None => novas.push(d),
        }
        self.gravar_estas(novas)
    }

    pub fn excluir(&mut self, nome: &str) -> Result<Gravacao> {
        let mut novas = self.ligacoes.clone();
        novas.retain(|l| !l.nome.eq_ignore_ascii_case(nome));
        if novas.len() == self.ligacoes.len() {
            return Err(PhxError::NaoEncontrado(format!(
                "dblink {nome:?} nao existe"
            )));
        }
        self.gravar_estas(novas)
    }

    /// Regrava o cadastro como esta.
    #[cfg(test)]
    fn gravar(&mut self) -> Result<Gravacao> {
        let atuais = self.ligacoes.clone();
        self.gravar_estas(atuais)
    }

    /// Grava o arquivo inteiro, com permissao de dono so -- desde o primeiro
    /// byte, e por troca atomica.
    ///
    /// O arquivo carrega senha e token de outro banco. Deixa-lo legivel por
    /// todo mundo seria guardar a credencial atras de uma porta aberta -- e
    /// era o que este metodo fazia por um instante: escrevia na permissao do
    /// `umask` e apertava DEPOIS, engolindo a falha do aperto. O molde certo
    /// ja existia para a chave do fio e nao tinha voltado para ca (revisao
    /// SEC de 17/09/2026, achado A4). Agora e o mesmo escritor dos irmaos.
    ///
    /// # A cifra (pedido 372)
    ///
    /// Com a chave mestra disponivel, TODA credencial escrita vai selada, e a
    /// primeira gravacao de um cadastro em claro o migra para o formato 2 --
    /// pedida (quem declarou a chave pediu), e nao imposta (sem chave, o
    /// arquivo sai byte a byte o de sempre). A migracao e DITA: o numero volta
    /// em [`Gravacao`] e sai no erro padrao, com o que ela NAO faz -- apagar o
    /// claro das copias ja tiradas.
    ///
    /// Sem selo, credencial em claro so e escrita no cadastro de sempre: no
    /// arquivo ja cifrado, ou com a chave declarada e indisponivel, a gravacao
    /// RECUSA, porque escrever em claro ali seria rebaixar o que o dono pediu
    /// cifrado.
    fn gravar_estas(&mut self, mut novas: Vec<Definicao>) -> Result<Gravacao> {
        let a_selar = em_claro(&novas);
        let mut escrito = self.cifra.escrito;
        let mut selo = self.cifra.aberto;
        if selo.is_none() && escrito.is_none() && !a_selar.is_empty() {
            if let Some(fora) = self.cifra.chave.de_fora() {
                let m = Material::novo_de_fora(&fora, self.cifra.iteracoes)?;
                escrito =
                    m.partes(ROTULO_DA_PROVA)
                        .map(|(sal, iteracoes, prova)| MaterialEscrito {
                            sal,
                            iteracoes,
                            prova,
                        });
                selo = Some(m);
            }
        }
        if selo.is_none() && !a_selar.is_empty() {
            let porque = match (&self.cifra.trancado, &self.cifra.chave) {
                (Some(m), _) => Some(m.clone()),
                (None, ChaveMestra::Indisponivel(m)) => Some(format!(
                    "a chave mestra ({}) esta declarada e indisponivel: {m}",
                    self.cifra.declarada_em
                )),
                _ => None,
            };
            if let Some(porque) = porque {
                return Err(PhxError::Esquema(format!(
                    "nao gravei {}: {porque}. Gravar a credencial de {} em TEXTO \
                     PURO rebaixaria um cadastro que foi pedido cifrado -- \
                     resolva a chave e salve de novo, ou use senha_env / \
                     token_remoto_env",
                    self.caminho.display(),
                    a_selar.join(", ")
                )));
            }
        }
        let ligacoes = novas
            .iter()
            .map(|l| l.para_disco(selo.as_ref()))
            .collect::<Result<Vec<_>>>()?;
        let j = match escrito {
            // O cadastro de sempre, sem um campo a mais: quem nao declarou
            // chave nao pode ter o arquivo mudado por baixo.
            None => Json::objeto(vec![(LISTA_DO_FORMATO_1, Json::Lista(ligacoes))]),
            Some(m) => Json::objeto(vec![
                ("formato", Json::de_u64(FORMATO_DO_CADASTRO as u64)),
                ("cifra_do_cadastro", m.para_json()),
                (LISTA_DO_FORMATO_2, Json::Lista(ligacoes)),
            ]),
        };
        crate::config::gravar_privado(&self.caminho, j.escrever_identado().as_bytes()).map_err(
            |e| PhxError::Esquema(format!("nao gravei {}: {e}", self.caminho.display())),
        )?;

        // O disco aceitou: so agora a memoria muda.
        let mut gravacao = Gravacao {
            cifrado: escrito.is_some(),
            ..Gravacao::default()
        };
        if selo.is_some() {
            let mut migradas: Vec<String> = Vec::new();
            for l in &mut novas {
                let mudou = l.senha.passou_a_selado() | l.token.passou_a_selado();
                let nome = l.nome.to_lowercase();
                if mudou && self.cifra.em_claro_no_disco.contains(&nome) {
                    migradas.push(format!("{:?}", l.nome));
                }
            }
            gravacao.ligacoes_cifradas = migradas.len();
            if !migradas.is_empty() {
                eprintln!(
                    "AVISO: o cadastro do DbLink em {} passou a guardar CIFRADAS as \
                     credenciais de {} ligacao(oes): {}. O texto puro saiu deste \
                     arquivo, mas NAO das copias -- todo backup tirado antes \
                     continua com a credencial em claro: TROQUE-a no outro banco. E \
                     daqui em diante um binario anterior a este le a credencial \
                     VAZIA: nao volte a versao sobre este cadastro.",
                    self.caminho.display(),
                    migradas.len(),
                    migradas.join(", ")
                );
            }
        }
        self.cifra.escrito = escrito;
        self.cifra.aberto = selo;
        self.cifra.em_claro_no_disco = em_claro(&novas);
        self.ligacoes = novas;
        Ok(gravacao)
    }
}

/// A lista de ligacoes do arquivo, no lugar que o formato dele manda.
///
/// Formato 1: `"dblink"`, ou a lista crua, como sempre. Formato 2:
/// [`LISTA_DO_FORMATO_2`], e SO ela -- a lista do formato 1 ao lado e arquivo
/// editado pela metade, e a leitura nao escolhe calada qual das duas vale.
fn lista_do_arquivo<'a>(j: &'a Json, formato: i64, caminho: &Path) -> Result<&'a [Json]> {
    if formato < 2 {
        return j
            .campo(LISTA_DO_FORMATO_1)
            .and_then(Json::lista)
            .or_else(|| j.lista())
            .ok_or_else(|| {
                PhxError::Esquema(format!(
                    "{}: esperava uma lista de ligacoes, ou um objeto com \"dblink\"",
                    caminho.display()
                ))
            });
    }
    if LISTA_DO_FORMATO_2 != LISTA_DO_FORMATO_1 && j.campo(LISTA_DO_FORMATO_1).is_some() {
        return Err(PhxError::Corrompido(format!(
            "{}: formato 2 com \"{LISTA_DO_FORMATO_1}\" ao lado de \
             \"{LISTA_DO_FORMATO_2}\" -- a leitura nao escolhe calada qual lista vale",
            caminho.display()
        )));
    }
    j.campo(LISTA_DO_FORMATO_2)
        .and_then(Json::lista)
        .ok_or_else(|| {
            PhxError::Corrompido(format!(
                "{}: formato 2 sem a lista \"{LISTA_DO_FORMATO_2}\"",
                caminho.display()
            ))
        })
}

/// As ligacoes com credencial que iria ao disco em TEXTO PURO sem selo, pelo
/// nome em minusculas.
fn em_claro(ligacoes: &[Definicao]) -> Vec<String> {
    ligacoes
        .iter()
        .filter(|l| l.senha.escrito_no_arquivo() || l.token.escrito_no_arquivo())
        .map(|l| l.nome.to_lowercase())
        .collect()
}

/// A instrucao so consulta?
///
/// Usada quando a ligacao esta em somente leitura. Duas coisas seguram o
/// caso, e as duas precisam existir:
///
/// 1. a primeira palavra tem de ser de consulta;
/// 2. `INTO OUTFILE`/`DUMPFILE` estao fora, porque um `SELECT` que escreve
///    arquivo no servidor do outro lado continua sendo um `SELECT`.
///
/// Emendar uma segunda instrucao com `;` nao entra na conta porque nao e
/// possivel: o cliente nao pede `CLIENT_MULTI_STATEMENTS`, e o servidor
/// recusa o pacote com duas.
pub fn so_consulta(sql: &str) -> bool {
    let limpo = sem_comentarios(sql);
    let alto = limpo.trim().to_uppercase();
    let primeira = alto
        .split(|c: char| c.is_whitespace() || c == '(')
        .find(|p| !p.is_empty())
        .unwrap_or("");
    let consulta = matches!(
        primeira,
        "SELECT" | "SHOW" | "DESCRIBE" | "DESC" | "EXPLAIN" | "WITH" | "TABLE" | "VALUES"
    );
    consulta && !alto.contains("INTO OUTFILE") && !alto.contains("INTO DUMPFILE")
}

/// Tira comentario de SQL antes de olhar a primeira palavra.
///
/// Sem isto, `/*x*/ DROP TABLE t` teria como primeira palavra `/*X*/` -- que
/// nao e nenhuma das de consulta, entao o comando seria RECUSADO, e nao
/// aceito. O buraco de verdade e o contrario: `/*x*/SELECT` seria recusado
/// sem motivo. Tirar o comentario acerta os dois.
fn sem_comentarios(sql: &str) -> String {
    let b = sql.as_bytes();
    let mut fora = String::with_capacity(sql.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                i += 1;
            }
            i += 2;
            fora.push(' ');
        } else if (b[i] == b'-' && b.get(i + 1) == Some(&b'-')) || b[i] == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            fora.push(' ');
        } else {
            fora.push(b[i] as char);
            i += 1;
        }
    }
    fora
}

/// Um nome de tabela, coluna ou base vindo da tela, conferido antes de virar
/// SQL.
///
/// A defesa e recusar, e nao escapar. Escapar aspas exige saber em que modo o
/// outro servidor esta -- com `NO_BACKSLASH_ESCAPES` a contrabarra deixa de
/// escapar, e a mesma regra que protegia passa a nao proteger. Nome de objeto
/// nao precisa de aspa, crase, contrabarra nem quebra de linha, entao nada
/// disso entra.
pub fn nome_seguro(nome: &str) -> Result<String> {
    let n = nome.trim();
    if n.is_empty() {
        return Err(PhxError::Esquema("nome vazio".into()));
    }
    if n.len() > 128 {
        return Err(PhxError::Esquema(format!(
            "nome longo demais: {n:?} (o MySQL(R) para em 64)"
        )));
    }
    if n.chars()
        .any(|c| c.is_control() || matches!(c, '`' | '\'' | '"' | '\\'))
    {
        return Err(PhxError::Esquema(format!(
            "nome com caractere que nao vale em identificador: {n:?}"
        )));
    }
    Ok(n.to_string())
}

/// Um nome que precisa virar TEXTO no SQL, como em `TABLE_SCHEMA = '...'`.
///
/// Passa pelo mesmo crivo do identificador e so depois vira literal: sem aspa
/// nem contrabarra dentro, as aspas de fora fecham onde devem em qualquer modo
/// do servidor.
pub fn literal(valor: &str) -> Result<String> {
    Ok(format!("'{}'", nome_seguro(valor)?))
}

/// Um nome de tabela ou coluna vindo da tela, protegido com crase.
///
/// A crase dobrada e como o MySQL(R) escapa uma crase dentro do nome. Sem
/// isto, um nome de tabela escolhido por quem usa a tela emendaria SQL.
pub fn entre_crases(nome: &str) -> String {
    format!("`{}`", nome.replace('`', "``"))
}

/// O resultado da consulta, no formato que a grade da tela espera.
pub fn resultado_para_json(r: &mysql::Resultado) -> Json {
    Json::objeto(vec![
        (
            "colunas",
            Json::Lista(
                r.colunas
                    .iter()
                    .map(|c| {
                        Json::objeto(vec![
                            ("nome", Json::texto_de(&c.nome)),
                            ("tabela", Json::texto_de(&c.tabela)),
                            ("tipo", Json::texto_de(&c.tipo)),
                            ("tamanho", Json::de_u64(c.tamanho as u64)),
                            ("decimais", Json::de_u64(c.decimais as u64)),
                            ("nulavel", Json::Bool(c.nulavel)),
                            ("primaria", Json::Bool(c.primaria)),
                            ("numerico", Json::Bool(c.numerico)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "linhas",
            Json::Lista(
                r.linhas
                    .iter()
                    .map(|l| {
                        Json::Lista(
                            l.iter()
                                .map(|v| match v {
                                    Some(t) => Json::texto_de(t),
                                    None => Json::Nulo,
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            ),
        ),
        ("quantas", Json::de_u64(r.linhas.len() as u64)),
        ("afetadas", Json::de_u64(r.afetadas)),
        ("truncado", Json::Bool(r.truncado)),
    ])
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_ligacao_nasce_somente_leitura() {
        let d = Definicao::de_json(&Json::analisar(r#"{"nome":"loja"}"#).unwrap()).unwrap();
        assert!(d.somente_leitura, "uma ligacao nova podia escrever");
        assert_eq!(d.porta, 3306);
    }

    #[test]
    fn o_apelido_nao_aceita_qualquer_coisa() {
        for ruim in [
            r#"{"nome":""}"#,
            r#"{"nome":"a b"}"#,
            r#"{"nome":"../etc"}"#,
        ] {
            assert!(
                Definicao::de_json(&Json::analisar(ruim).unwrap()).is_err(),
                "aceitou {ruim}"
            );
        }
        assert!(Definicao::de_json(&Json::analisar(r#"{"nome":"loja-1_A"}"#).unwrap()).is_ok());
    }

    #[test]
    fn a_senha_da_ligacao_nunca_aparece_no_json() {
        let d = Definicao::de_json(
            &Json::analisar(r#"{"nome":"loja","senha":"segredo-do-outro-banco"}"#).unwrap(),
        )
        .unwrap();
        assert_eq!(d.senha().unwrap(), "segredo-do-outro-banco");
        let t = d.para_json().escrever();
        assert!(!t.contains("segredo-do-outro-banco"), "a senha vazou: {t}");
        assert!(t.contains("(oculta)"));
    }

    /// A senha e o token nao saem no `Debug` -- nem o da `Definicao`, nem o do
    /// `Registro` que a guarda.
    ///
    /// Irma da de cima, e a saida que faltava: o `para_json` cumpria a promessa
    /// do comentario («nunca sai em JSON nem em log») e o `derive(Debug)` a
    /// desfazia. Um unico `dbg!(&registro)` despejava a credencial de TODAS as
    /// ligacoes.
    ///
    /// Confere as DUAS formas de escrever o `Debug` -- `{:?}` com argumento e
    /// `{x:?}` interpolado --, como faz a `a_privada_do_fio_nunca_sai`: sao
    /// dois caminhos do `format_args!`, e provar so um deixa o outro sem
    /// guarda.
    ///
    /// O token e o pior dos dois, e o proprio campo diz por que: no PhxSql ele
    /// e o portao 1, conferido ANTES do login -- quem o tem alcanca a porta de
    /// dados do outro servidor sem usuario nenhum.
    #[test]
    fn o_debug_da_ligacao_nunca_mostra_a_senha_nem_o_token() {
        const SENHA: &str = "segredo-do-outro-banco";
        const TOKEN: &str = "token-do-outro-servidor";
        let d = Definicao::de_json(
            &Json::analisar(&format!(
                r#"{{"nome":"loja","motor":"phxsql","senha":"{SENHA}",
                    "token_remoto":"{TOKEN}"}}"#
            ))
            .unwrap(),
        )
        .unwrap();
        // O valor esta mesmo la -- senao a prova passaria por nao haver segredo.
        assert_eq!(d.senha().unwrap(), SENHA);
        assert_eq!(d.token(), TOKEN);

        for texto in [format!("{:?}", d), format!("{d:?}")] {
            assert!(!texto.contains(SENHA), "a senha vazou no Debug: {texto}");
            assert!(!texto.contains(TOKEN), "o token vazou no Debug: {texto}");
            assert!(texto.contains("(oculta)"), "sem a marca da senha: {texto}");
            assert!(texto.contains("(oculto)"), "sem a marca do token: {texto}");
            // O apelido continua visivel: `Debug` cego nao diagnostica nada.
            assert!(texto.contains("loja"), "o Debug perdeu o nome: {texto}");
        }

        // O `Registro` fica resolvido por consequencia -- ele imprime um
        // `Vec<Definicao>` --, mas isso se PROVA, nao se deduz.
        let r = Registro {
            caminho: PathBuf::from("/tmp/dblink.json"),
            ligacoes: vec![d],
            ..Registro::default()
        };
        for texto in [format!("{:?}", r), format!("{r:?}")] {
            assert!(
                !texto.contains(SENHA),
                "a senha vazou pelo Registro: {texto}"
            );
            assert!(
                !texto.contains(TOKEN),
                "o token vazou pelo Registro: {texto}"
            );
        }
    }

    /// O nome da variavel de ambiente CONTINUA visivel no `Debug`.
    ///
    /// Nao e segredo, e e ele que diz de onde a credencial deveria ter vindo:
    /// esconde-lo trocaria um vazamento por um diagnostico cego. E a mesma
    /// escolha do `Debug` da `Cifra`, que mantem `senha_env`.
    #[test]
    fn o_debug_da_ligacao_mantem_o_nome_da_variavel_de_ambiente() {
        let d = Definicao::de_json(
            &Json::analisar(
                r#"{"nome":"loja","senha_env":"SENHA_DA_LOJA",
                    "token_remoto_env":"TOKEN_DA_LOJA"}"#,
            )
            .unwrap(),
        )
        .unwrap();
        let texto = format!("{d:?}");
        assert!(texto.contains("SENHA_DA_LOJA"), "{texto}");
        assert!(texto.contains("TOKEN_DA_LOJA"), "{texto}");
    }

    // -----------------------------------------------------------------------
    // A cifra do fio da ligacao (pedido 378)
    // -----------------------------------------------------------------------

    /// Um pino valido: 32 bytes em hexadecimal.
    const PINO: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn lig(json: &str) -> Result<Definicao> {
        Definicao::de_json(&Json::analisar(json).unwrap())
    }

    /// O PADRAO, e o alcance dele: ligacao `phxsql` sem campo nenhum pede o
    /// tunel; ligacao de motor alheio, nao -- porque la nao ha com quem
    /// apertar a mao.
    ///
    /// Sem isto, contra um PhxSql desta versao (que exige de fabrica) o DbLink
    /// simplesmente nao entra, e o unico escape era desligar o `exigir` do
    /// servidor de destino INTEIRO.
    #[test]
    fn a_ligacao_phxsql_nasce_pedindo_o_tunel() {
        let d = lig(r#"{"nome":"erp","motor":"phxsql","host":"h"}"#).unwrap();
        assert!(d.cifra(), "a ligacao phxsql nasceu em claro");
        assert_eq!(d.cifra_escrita(), None, "ninguem escreveu, e diz que sim");

        for alheio in ["mysql", "postgres"] {
            let d = lig(&format!(r#"{{"nome":"erp","motor":"{alheio}"}}"#)).unwrap();
            assert!(
                !d.cifra(),
                "{alheio} nasceu com cifra: o campo nao faz nada ali"
            );
        }
        // Motor omitido e MySQL -- e por isso um padrao cego ao motor poria
        // cifra em toda ligacao implicita.
        assert!(!lig(r#"{"nome":"erp"}"#).unwrap().cifra());
    }

    /// `"cifra": false` continua abrindo em claro -- o escape ESCRITO, o mesmo
    /// molde do `"exigir": false` e do `CIFRA=0` do ODBC.
    #[test]
    fn o_escape_da_cifra_e_escrito() {
        let d = lig(r#"{"nome":"erp","motor":"phxsql","cifra":false}"#).unwrap();
        assert!(!d.cifra(), "o escape escrito nao foi respeitado");
        assert_eq!(d.cifra_escrita(), Some(false));
        // E o escape sobrevive ao disco: e decisao, nao ruido.
        let disco = d.para_disco(None).unwrap().escrever();
        assert!(disco.contains("\"cifra\":false"), "{disco}");
    }

    /// Valor que ninguem reconhece NAO desliga a cifra -- fica no padrao.
    ///
    /// Regra do `interruptor()` do ODBC: com o padrao ligado, um dedo errado
    /// nao pode rebaixar. Desligar continua exigindo escolha escrita, e
    /// escrita de um jeito que o motor entenda.
    #[test]
    fn valor_torto_na_cifra_nao_desliga_o_tunel() {
        for torto in ["\"zero\"", "0", "null", "[]"] {
            let d = lig(&format!(
                r#"{{"nome":"erp","motor":"phxsql","cifra":{torto}}}"#
            ))
            .unwrap();
            assert!(d.cifra(), "{torto} desligou a cifra");
        }
    }

    /// A recusa na DECLARACAO, nomeando o motor -- e nao na conexao.
    ///
    /// Aceitar o campo ali seria um interruptor que nao faz nada: a tela diria
    /// «cifrada» para uma ligacao que fala protocolo alheio em claro. Pior que
    /// o buraco conhecido.
    #[test]
    fn a_cifra_e_o_pino_sao_recusados_no_motor_que_nao_fala_o_nosso_aperto() {
        for alheio in ["mysql", "postgres"] {
            for campo in [
                r#""cifra":true"#.to_string(),
                format!(r#""chave_do_fio":"{PINO}""#),
            ] {
                let e =
                    lig(&format!(r#"{{"nome":"erp","motor":"{alheio}",{campo}}}"#)).unwrap_err();
                let t = e.to_string();
                assert!(t.contains(alheio), "a recusa nao nomeia o motor: {t}");
                assert!(
                    t.contains("VPN") || t.contains("tunel"),
                    "a recusa nao aponta o caminho de fora: {t}"
                );
            }
            // `cifra: false` continua valendo em qualquer motor: e a verdade
            // sobre aquele fio, e recusa-la tiraria o direito de escrever o
            // que ja acontece.
            assert!(
                lig(&format!(
                    r#"{{"nome":"erp","motor":"{alheio}","cifra":false}}"#
                ))
                .is_ok(),
                "{alheio} recusou o false, que e o que ele ja faz"
            );
        }
        // E o motor que fala o nosso aperto aceita os dois.
        assert!(lig(&format!(
            r#"{{"nome":"erp","motor":"phxsql","cifra":true,"chave_do_fio":"{PINO}"}}"#
        ))
        .is_ok());
    }

    /// O pino vence o interruptor -- copiado do ODBC, e pelo mesmo motivo:
    /// quem escreveu o pino quer o tunel CONFERIDO, e um `false` ao lado e
    /// contradicao. A porta para falar claro e nao escrever pino.
    #[test]
    fn o_pino_vence_o_interruptor() {
        let d = lig(&format!(
            r#"{{"nome":"erp","motor":"phxsql","cifra":false,"chave_do_fio":"{PINO}"}}"#
        ))
        .unwrap();
        assert!(d.cifra(), "o false desligou o tunel de quem pediu pino");
        assert_eq!(d.pino_do_fio().unwrap(), Some([0xaau8; 32]));
    }

    /// Pino torto vira ERRO nomeando o caminho, nunca `None` em silencio.
    ///
    /// `None` seria «sem pino», e um pino escrito errado viraria um tunel sem
    /// ancora -- exatamente o estrago que o pino existe para impedir.
    #[test]
    fn o_pino_torto_e_erro_nomeando_o_caminho() {
        for (torto, marca) in [
            ("zz", "hexadecimal"),
            ("aabb", "bytes"),
            (&"a".repeat(62), "bytes"),
        ] {
            let e = lig(&format!(
                r#"{{"nome":"erp","motor":"phxsql","chave_do_fio":"{torto}"}}"#
            ))
            .unwrap_err();
            let t = e.to_string();
            assert!(t.contains("dblink[erp].chave_do_fio"), "{t}");
            assert!(t.contains(marca), "{t}");
        }
        // Sem pino nao e erro -- e tunel sem ancora, que e outra coisa.
        assert_eq!(
            lig(r#"{"nome":"erp","motor":"phxsql"}"#)
                .unwrap()
                .pino_do_fio()
                .unwrap(),
            None
        );
    }

    /// O `para_disco` grava a cifra SO quando ela diverge do padrao, e nunca
    /// no motor alheio.
    ///
    /// `Registro::gravar` reescreve TODAS as ligacoes a cada salvar: gravar o
    /// padrao efetivo fossilizaria, numa edicao de OUTRA ligacao, uma decisao
    /// que ninguem tomou -- e mataria a diferenca entre «herdou a virada» e
    /// «escreveu a decisao».
    #[test]
    fn o_disco_nao_fossiliza_o_padrao_da_cifra() {
        // Padrao herdado: nada de `cifra` no arquivo, mesmo com o `true`
        // escrito, porque `true` E o padrao.
        for json in [
            r#"{"nome":"erp","motor":"phxsql"}"#,
            r#"{"nome":"erp","motor":"phxsql","cifra":true}"#,
        ] {
            let disco = lig(json).unwrap().para_disco(None).unwrap().escrever();
            assert!(
                !disco.contains("\"cifra\""),
                "o padrao foi fossilizado por {json}: {disco}"
            );
        }
        // Motor alheio nao grava nenhum dos dois -- campo sem leitor.
        let disco = lig(r#"{"nome":"erp","motor":"mysql","cifra":false}"#)
            .unwrap()
            .para_disco(None)
            .unwrap()
            .escrever();
        assert!(!disco.contains("cifra"), "{disco}");
        assert!(!disco.contains("chave_do_fio"), "{disco}");
        // O pino, esse, vai inteiro: e config, e sem ele o tunel perde a
        // ancora no proximo arranque.
        let disco = lig(&format!(
            r#"{{"nome":"erp","motor":"phxsql","chave_do_fio":"{PINO}"}}"#
        ))
        .unwrap()
        .para_disco(None)
        .unwrap()
        .escrever();
        assert!(disco.contains(PINO), "{disco}");
    }

    /// O `para_json` mostra a cifra EFETIVA e `tem_pino` -- e nunca o pino.
    ///
    /// A lista de quem tem e quem nao tem pino ja seria mapa para atacante; o
    /// pino em si e config, nao resposta. Mesma regra do irmao do cluster.
    #[test]
    fn o_para_json_da_a_cifra_e_tem_pino_e_nunca_o_pino() {
        let d = lig(&format!(
            r#"{{"nome":"erp","motor":"phxsql","chave_do_fio":"{PINO}"}}"#
        ))
        .unwrap();
        let t = d.para_json().escrever();
        assert!(!t.contains(PINO), "o pino vazou no protocolo: {t}");
        assert!(t.contains("\"tem_pino\":true"), "{t}");
        assert!(t.contains("\"cifra\":true"), "{t}");

        let t = lig(r#"{"nome":"erp","motor":"mysql"}"#)
            .unwrap()
            .para_json()
            .escrever();
        assert!(t.contains("\"tem_pino\":false"), "{t}");
        assert!(t.contains("\"cifra\":false"), "{t}");
    }

    /// No `Debug` o pino APARECE, e os dois estados da cifra tambem.
    ///
    /// O `Debug` diagnostica, o protocolo publica: a chave e publica, e
    /// esconde-la trocaria um vazamento que nao existe por um diagnostico cego
    /// de pino torto. Mesma escolha do `Debug` da `Origem`.
    #[test]
    fn o_debug_da_ligacao_mostra_o_pino_e_a_decisao_da_cifra() {
        let d = lig(&format!(
            r#"{{"nome":"erp","motor":"phxsql","chave_do_fio":"{PINO}"}}"#
        ))
        .unwrap();
        let t = format!("{d:?}");
        assert!(t.contains(PINO), "o Debug escondeu a chave publica: {t}");
        assert!(t.contains("cifra: None"), "{t}");
        assert!(t.contains("cifra_efetiva: true"), "{t}");

        let d = lig(r#"{"nome":"erp","motor":"phxsql","cifra":false}"#).unwrap();
        let t = format!("{d:?}");
        assert!(t.contains("cifra: Some(false)"), "{t}");
        assert!(t.contains("cifra_efetiva: false"), "{t}");
    }

    /// A ida e a volta pelo arquivo: o escape e o pino sobrevivem, e o padrao
    /// continua saindo do motor.
    #[test]
    fn a_cifra_e_o_pino_sobrevivem_ao_arquivo() {
        let dir = DirTemp::novo("dblink-cifra");
        let caminho = dir.join("dblink.json");
        let mut r = Registro::abrir(&caminho).unwrap();
        r.salvar(lig(r#"{"nome":"claro","motor":"phxsql","cifra":false}"#).unwrap())
            .unwrap();
        r.salvar(
            lig(&format!(
                r#"{{"nome":"pinado","motor":"phxsql","chave_do_fio":"{PINO}"}}"#
            ))
            .unwrap(),
        )
        .unwrap();
        r.salvar(lig(r#"{"nome":"padrao","motor":"phxsql"}"#).unwrap())
            .unwrap();

        let lido = Registro::abrir(&caminho).unwrap();
        assert!(!lido.achar("claro").unwrap().cifra());
        assert_eq!(lido.achar("claro").unwrap().cifra_escrita(), Some(false));
        assert!(lido.achar("pinado").unwrap().cifra());
        assert_eq!(
            lido.achar("pinado").unwrap().pino_do_fio().unwrap(),
            Some([0xaau8; 32])
        );
        // O que herdou a virada continua herdando -- e nao virou decisao por
        // ter passado pelo disco ao lado dos outros dois.
        assert!(lido.achar("padrao").unwrap().cifra());
        assert_eq!(lido.achar("padrao").unwrap().cifra_escrita(), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn so_consulta_deixa_passar_o_que_le() {
        for bom in [
            "SELECT * FROM t",
            "  select 1",
            "show tables",
            "DESCRIBE clientes",
            "WITH x AS (SELECT 1) SELECT * FROM x",
            "/* comentario */ SELECT 1",
            "-- nota\nSELECT 1",
        ] {
            assert!(so_consulta(bom), "recusou {bom:?}");
        }
    }

    #[test]
    fn so_consulta_barra_o_que_escreve() {
        for ruim in [
            "DELETE FROM t",
            "drop table t",
            "UPDATE t SET a=1",
            "INSERT INTO t VALUES (1)",
            "TRUNCATE t",
            "GRANT ALL ON *.* TO x",
            // Um SELECT que escreve arquivo no servidor do outro lado
            // continua sendo escrita.
            "SELECT * FROM t INTO OUTFILE '/tmp/x'",
            "select a from t into dumpfile '/tmp/x'",
            // A tentativa de esconder o comando atras de comentario.
            "/* SELECT */ DROP TABLE t",
        ] {
            assert!(!so_consulta(ruim), "deixou passar {ruim:?}");
        }
    }

    #[test]
    fn nome_seguro_recusa_o_que_emendaria_sql() {
        for ruim in [
            "cli`entes",
            "cli'entes",
            "cli\"entes",
            "cli\\entes",
            "cli\nentes",
            "  ",
        ] {
            assert!(nome_seguro(ruim).is_err(), "aceitou {ruim:?}");
        }
        assert_eq!(nome_seguro(" clientes ").unwrap(), "clientes");
        // Nome com espaco e acento continua valendo: sao legais no MySQL(R) e
        // a crase de fora resolve.
        assert_eq!(nome_seguro("Notas Fiscais").unwrap(), "Notas Fiscais");
        assert_eq!(literal("loja").unwrap(), "'loja'");
    }

    #[test]
    fn a_crase_no_nome_e_escapada() {
        assert_eq!(entre_crases("clientes"), "`clientes`");
        // O nome que emendaria SQL se entrasse cru.
        assert_eq!(
            entre_crases("a`; DROP TABLE x; --"),
            "`a``; DROP TABLE x; --`"
        );
    }

    #[test]
    fn cadastro_vazio_quando_o_arquivo_nao_existe() {
        let r = Registro::abrir(Path::new("/nao/existe/dblink.json")).unwrap();
        assert!(r.ligacoes.is_empty());
    }

    /// O `dblink.json` nasce 0600 desde o primeiro byte, e a resposta e do
    /// sistema operacional. Sem o conserto, o `.tmp` nascia 0644 sob
    /// `umask 022` com a senha dentro, ate o `set_permissions`.
    #[cfg(unix)]
    #[test]
    fn o_cadastro_nasce_0600() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = DirTemp::novo("dblink-permissao");
        let caminho = dir.join("dblink.json");
        let mut r = Registro::abrir(&caminho).unwrap();
        r.salvar(
            Definicao::de_json(
                &Json::analisar(
                    r#"{"nome":"erp","motor":"phxsql","host":"h","token_remoto":"MARCA"}"#,
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let modo = std::fs::metadata(&caminho).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            modo, 0o600,
            "o dblink.json nasceu legivel por outros: {modo:o}"
        );
        assert!(!crate::config::temporario_de(&caminho).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn grava_le_e_nao_perde_a_senha() {
        let dir = DirTemp::novo("dblink");
        let caminho = dir.join("dblink.json");
        let mut r = Registro::abrir(&caminho).unwrap();
        r.salvar(
            Definicao::de_json(
                &Json::analisar(r#"{"nome":"loja","host":"10.0.0.5","senha":"abc","usuario":"u"}"#)
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let lido = Registro::abrir(&caminho).unwrap();
        assert_eq!(lido.ligacoes.len(), 1);
        assert_eq!(lido.achar("LOJA").unwrap().senha().unwrap(), "abc");
        assert_eq!(lido.achar("loja").unwrap().host, "10.0.0.5");
        // Salvar de novo com o mesmo nome substitui, nao duplica.
        let mut r2 = lido;
        r2.salvar(
            Definicao::de_json(&Json::analisar(r#"{"nome":"loja","host":"10.0.0.9"}"#).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(Registro::abrir(&caminho).unwrap().ligacoes.len(), 1);
        assert_eq!(
            Registro::abrir(&caminho)
                .unwrap()
                .achar("loja")
                .unwrap()
                .host,
            "10.0.0.9"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // -----------------------------------------------------------------------
    // Pedido 372: a variavel declarada que falta, e o texto puro no arquivo
    // -----------------------------------------------------------------------
    //
    // Nomes de variavel UNICOS por teste, e as de falta nunca sao tocadas:
    // `set_var` e estado global do processo e o `libtest` roda em paralelo
    // (pedidos 247, 261, 267). A prova da falta nao mexe no ambiente -- so
    // nomeia uma variavel que ninguem cria.

    /// Nunca criada por ninguem: e a falta que se prova.
    const SENHA_AUSENTE: &str = "PHXSQL_TESTE_372_DBLINK_SENHA_QUE_NINGUEM_EXPORTA";
    const TOKEN_AUSENTE: &str = "PHXSQL_TESTE_372_DBLINK_TOKEN_QUE_NINGUEM_EXPORTA";

    /// A senha de `senha_env` que falta NAO vira senha vazia: vira o erro que
    /// nomeia a variavel e a ligacao, e ele vem ANTES da rede.
    ///
    /// Com o defeito reposto (`unwrap_or_default`) a senha sai `Ok("")`, e a
    /// conexao vai ao outro banco apresentar senha vazia -- a primeira
    /// assercao cai.
    #[test]
    fn senha_env_que_falta_e_erro_nomeado_e_nao_senha_vazia() {
        let d = lig(&format!(
            r#"{{"nome":"loja","motor":"mysql","host":"127.0.0.1","porta":1,
                 "usuario":"u","senha_env":"{SENHA_AUSENTE}"}}"#
        ))
        .expect("a leitura NAO pode recusar: e ela que o arranque chama");
        // `match`, e nao `expect_err`: com o defeito reposto o `Ok` traria
        // o valor, e o `expect_err` o imprimiria na saida do teste.
        let e = match d.senha() {
            Ok(_) => panic!("variavel ausente virou senha -- o defeito do 372"),
            Err(e) => e.to_string(),
        };
        assert!(
            e.contains(SENHA_AUSENTE),
            "o erro nao nomeia a variavel: {e}"
        );
        assert!(e.contains("\"loja\""), "o erro nao nomeia a ligacao: {e}");
        assert!(e.contains("senha_env"), "o erro nao nomeia o campo: {e}");
        // Os dois caminhos que conectam recusam pelo MESMO erro, e antes da
        // rede: na porta 1 nao ha ninguem, entao um erro de conexao aqui
        // seria a prova de que o portao ficou para depois do trabalho.
        for erro in [
            d.conectar().map(|_| ()).unwrap_err().to_string(),
            d.abrir().map(|_| ()).unwrap_err().to_string(),
        ] {
            assert!(erro.contains(SENHA_AUSENTE), "conectou sem a senha: {erro}");
        }
        // A tela ve o estado e o NOME, nunca um valor.
        let ficha = d.para_json();
        assert_eq!(ficha.texto_ou("senha", ""), "(variavel ausente)");
        assert_eq!(
            ficha.campo("variaveis_ausentes").map(Json::escrever),
            Some(format!("[\"{SENHA_AUSENTE}\"]"))
        );
    }

    /// O irmao da senha, que o parecer do 372 nomeou: o token do outro PhxSql.
    /// E o pior dos dois -- sem ele o outro servidor responde «token
    /// invalido», e o erro manda procurar no lugar errado.
    #[test]
    fn token_remoto_env_que_falta_e_erro_nomeado_e_nao_token_vazio() {
        let d = lig(&format!(
            r#"{{"nome":"erp","motor":"phxsql","host":"127.0.0.1","porta":1,
                 "cifra":false,"token_remoto_env":"{TOKEN_AUSENTE}"}}"#
        ))
        .expect("a leitura NAO pode recusar");
        // `match`, e nao `expect_err`: com o defeito reposto o `Ok` traria
        // o valor, e o `expect_err` o imprimiria na saida do teste.
        let e = match d.token_remoto() {
            Ok(_) => panic!("variavel ausente virou token -- o defeito do 372"),
            Err(e) => e.to_string(),
        };
        assert!(e.contains(TOKEN_AUSENTE), "{e}");
        assert!(e.contains("\"erp\""), "{e}");
        assert!(e.contains("token_remoto_env"), "{e}");
        let erro = d.abrir().map(|_| ()).unwrap_err().to_string();
        assert!(erro.contains(TOKEN_AUSENTE), "conectou sem o token: {erro}");
        assert_eq!(
            d.para_json().texto_ou("token_remoto", ""),
            "(variavel ausente)"
        );
    }

    /// A ligacao mal configurada NAO derruba o arranque.
    ///
    /// O `Servidor::novo` abre o cadastro com `?`: se a falta virasse erro na
    /// leitura, uma variavel do DbLink tiraria o motor de dados do ar. Este e
    /// o teste dos DOIS sentidos: cai com o defeito reposto (a ligacao nao
    /// tranca, e a senha sai vazia) e cai com o conserto errado (a leitura
    /// recusando, e o `Registro::abrir` junto).
    ///
    /// E a ligacao VIZINHA, bem configurada, continua inteira.
    #[test]
    fn a_variavel_que_falta_tranca_so_a_ligacao_e_nao_o_arranque() {
        let dir = DirTemp::novo("dblink-372-trancada");
        let caminho = dir.join("dblink.json");
        std::fs::write(
            &caminho,
            format!(
                r#"{{"dblink":[
                    {{"nome":"trancada","senha_env":"{SENHA_AUSENTE}"}},
                    {{"nome":"boa","senha":"abc"}}]}}"#
            ),
        )
        .unwrap();
        let mut r = Registro::abrir(&caminho).expect("o cadastro recusou o arranque");
        assert!(r.achar("trancada").unwrap().senha().is_err());
        assert_eq!(r.achar("boa").unwrap().senha().unwrap(), "abc");

        let avisos = r.avisos();
        let falta = avisos
            .iter()
            .find(|a| a.contains(SENHA_AUSENTE))
            .unwrap_or_else(|| panic!("a falta subiu calada: {avisos:?}"));
        assert!(falta.contains("TRANCADA"), "{falta}");

        // Gravar o cadastro com a ligacao trancada guarda o NOME da variavel,
        // e nunca uma senha vazia no lugar -- o proximo arranque, com a
        // variavel exportada, tem de conseguir le-la.
        r.gravar().unwrap();
        let disco = std::fs::read_to_string(&caminho).unwrap();
        assert!(disco.contains(SENHA_AUSENTE), "{disco}");
        let relido = Registro::abrir(&caminho).unwrap();
        assert_eq!(relido.achar("trancada").unwrap().senha_env, SENHA_AUSENTE);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// O comportamento VELHO, que e o que mais importa numa guarda nova: sem
    /// `_env`, e com `_env` e a variavel presente, nada muda -- nem o valor,
    /// nem o rotulo da tela, nem o que vai para o disco.
    #[test]
    fn sem_env_e_com_a_variavel_presente_nada_muda() {
        // Sem `_env`: a senha do arquivo, como sempre.
        let d = lig(r#"{"nome":"loja","senha":"do-arquivo","motor":"phxsql","token_remoto":"t1"}"#)
            .unwrap();
        assert_eq!(d.senha().unwrap(), "do-arquivo");
        assert_eq!(d.token_remoto().unwrap(), "t1");
        assert_eq!(d.para_json().texto_ou("senha", ""), "(oculta)");
        assert_eq!(d.para_json().texto_ou("token_remoto", ""), "(oculto)");
        assert_eq!(
            d.para_json()
                .campo("variaveis_ausentes")
                .map(Json::escrever),
            Some("[]".to_string())
        );

        // Sem senha nenhuma: vazia, e vazia NAO e falta -- e decisao escrita.
        let d = lig(r#"{"nome":"loja"}"#).unwrap();
        assert_eq!(d.senha().unwrap(), "");
        assert_eq!(d.para_json().texto_ou("senha", ""), "(vazia)");

        // Com `_env` e a variavel PRESENTE: o valor do ambiente, e o disco
        // guarda so o nome. Variavel so deste teste.
        const SENHA_PRESENTE: &str = "PHXSQL_TESTE_372_DBLINK_SENHA_PRESENTE";
        const TOKEN_PRESENTE: &str = "PHXSQL_TESTE_372_DBLINK_TOKEN_PRESENTE";
        std::env::set_var(SENHA_PRESENTE, "valor-do-ambiente");
        std::env::set_var(TOKEN_PRESENTE, "token-do-ambiente");
        let d = lig(&format!(
            r#"{{"nome":"loja","motor":"phxsql","senha_env":"{SENHA_PRESENTE}",
                 "token_remoto_env":"{TOKEN_PRESENTE}"}}"#
        ))
        .unwrap();
        assert_eq!(d.senha().unwrap(), "valor-do-ambiente");
        assert_eq!(d.token_remoto().unwrap(), "token-do-ambiente");
        assert_eq!(d.para_json().texto_ou("senha", ""), "(do ambiente)");
        assert_eq!(d.para_json().texto_ou("token_remoto", ""), "(do ambiente)");
        let disco = d.para_disco(None).unwrap().escrever();
        assert!(!disco.contains("valor-do-ambiente"), "{disco}");
        assert!(!disco.contains("token-do-ambiente"), "{disco}");
        assert!(disco.contains(SENHA_PRESENTE), "{disco}");
        assert!(disco.contains(TOKEN_PRESENTE), "{disco}");
    }

    /// O aviso do texto puro fala com quem NAO decidiu, e cala para quem
    /// escreveu `_env`. E nunca imprime o valor.
    #[test]
    fn o_aviso_do_texto_puro_nomeia_ligacao_e_campo_e_nunca_o_valor() {
        const SENHA: &str = "SENHA-QUE-NAO-PODE-SAIR-NO-AVISO";
        const TOKEN: &str = "TOKEN-QUE-NAO-PODE-SAIR-NO-AVISO";
        let r = Registro {
            caminho: PathBuf::from("/srv/phx/dblink.json"),
            ligacoes: vec![
                lig(&format!(r#"{{"nome":"loja","senha":"{SENHA}"}}"#)).unwrap(),
                lig(&format!(
                    r#"{{"nome":"erp","motor":"phxsql","token_remoto":"{TOKEN}"}}"#
                ))
                .unwrap(),
            ],
            ..Registro::default()
        };
        let avisos = r.avisos();
        assert_eq!(
            avisos.len(),
            1,
            "um aviso para o arquivo inteiro: {avisos:?}"
        );
        let a = &avisos[0];
        assert!(a.contains("\"loja\" (senha)"), "{a}");
        assert!(a.contains("\"erp\" (token_remoto)"), "{a}");
        assert!(a.contains("/srv/phx/dblink.json"), "{a}");
        assert!(!a.contains(SENHA), "o aviso imprimiu a senha: {a}");
        assert!(!a.contains(TOKEN), "o aviso imprimiu o token: {a}");

        // Quem decidiu -- `_env` nos dois, ou sem credencial nenhuma -- nao
        // ouve nada. Aviso perpetuo em instalacao decidida gasta o aviso
        // verdadeiro. A variavel ausente aqui teria aviso proprio, entao a
        // prova usa uma que existe em todo processo.
        let decidido = Registro {
            caminho: PathBuf::from("/srv/phx/dblink.json"),
            ligacoes: vec![
                lig(r#"{"nome":"loja","senha_env":"PATH"}"#).unwrap(),
                lig(r#"{"nome":"erp","motor":"phxsql","token_remoto_env":"PATH"}"#).unwrap(),
                lig(r#"{"nome":"sem","usuario":"leitor"}"#).unwrap(),
            ],
            ..Registro::default()
        };
        assert!(decidido.avisos().is_empty(), "{:?}", decidido.avisos());
    }

    // -----------------------------------------------------------------------
    // Pedido 372, a decisao do dono (24/09/2026): o cadastro CIFRADO, com a
    // chave mestra de FORA do conjunto copiado
    // -----------------------------------------------------------------------
    //
    // A chave vem de um ARQUIVO num diretorio irmao do cadastro, e nao de
    // variavel de ambiente: `set_var` e estado global do processo e o
    // `libtest` roda em paralelo. O unico teste que exporta variavel usa um
    // nome so dele.
    //
    // Nenhuma assercao daqui imprime credencial: comparacao de segredo e
    // `assert!(a == b, "texto sem o valor")`, e nunca `assert_eq!`, que
    // imprimiria os dois lados -- e erro de segredo e `match`, nunca
    // `expect_err`, que imprimiria o `Ok`.

    const SENHA_372: &str = "SENHA-DO-DESTINO-372-QUE-NAO-PODE-IR-AO-DISCO";
    const TOKEN_372: &str = "TOKEN-DO-OUTRO-PHXSQL-372-QUE-NAO-PODE-IR-AO-DISCO";
    const CHAVE_A: &str = "a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0";
    const CHAVE_B: &str = "0f0e0d0c0b0a09080706050403020100ffeeddccbbaa99887766554433221100";

    fn cifra_de(json: &str) -> CifraDoDblink {
        crate::config::Config::de_json(&Json::analisar(json).unwrap())
            .unwrap()
            .cifra_do_dblink
    }

    /// A chave mestra num arquivo de um diretorio que NAO e o do cadastro.
    fn chave_no_arquivo(fora: &Path, nome: &str, hex: &str) -> CifraDoDblink {
        let arq = fora.join(nome);
        std::fs::write(&arq, format!("{hex}\n")).unwrap();
        cifra_de(&format!(
            r#"{{"cifra_do_dblink":{{"chave_mestra_arquivo":{:?}}}}}"#,
            arq.display().to_string()
        ))
    }

    /// Quantas vezes `agulha` aparece em `palheiro` -- o DANO medido, e nao o
    /// veredito.
    fn ocorrencias(palheiro: &[u8], agulha: &[u8]) -> usize {
        palheiro
            .windows(agulha.len())
            .filter(|w| *w == agulha)
            .count()
    }

    fn salvar_json(r: &mut Registro, json: &str) -> Result<Gravacao> {
        r.salvar(lig(json)?)
    }

    /// (a) Com a chave mestra, o arquivo gravado NAO contem a senha nem o
    /// token -- contado em ocorrencias, no arquivo inteiro.
    ///
    /// Com o defeito reposto (o `para_disco` ignorando o selo) as duas
    /// credenciais voltam ao disco em claro, e o vermelho diz quantas vezes.
    #[test]
    fn com_a_chave_o_disco_nao_guarda_a_senha_nem_o_token_em_claro() {
        let casa = DirTemp::novo("dblink-372-a");
        let fora = DirTemp::novo("dblink-372-a-chave");
        let cifra = chave_no_arquivo(&fora, "mestra.hex", CHAVE_A);
        let caminho = casa.join("dblink.json");
        let mut r = Registro::abrir_com(&caminho, &cifra).unwrap();
        let pedidos = [
            format!(r#"{{"nome":"loja","host":"10.0.0.5","usuario":"u","senha":"{SENHA_372}"}}"#),
            format!(
                r#"{{"nome":"filial","motor":"phxsql","host":"10.0.0.6","token_remoto":"{TOKEN_372}"}}"#
            ),
        ];
        // O disco e medido DEPOIS DE CADA gravacao, e antes do veredito dela:
        // o claro que uma gravacao deixa e o dano, mesmo que a seguinte recuse.
        for (i, pedido) in pedidos.iter().enumerate() {
            let salvou = salvar_json(&mut r, pedido);
            let disco = std::fs::read(&caminho).unwrap_or_default();
            let (s, t) = (
                ocorrencias(&disco, SENHA_372.as_bytes()),
                ocorrencias(&disco, TOKEN_372.as_bytes()),
            );
            assert!(
                s == 0 && t == 0,
                "a gravacao {} COM chave mestra deixou a credencial em claro no \
                 dblink.json: a senha {s} vez(es) e o token {t} vez(es), em {} bytes",
                i + 1,
                disco.len()
            );
            if let Err(e) = salvou {
                panic!("a gravacao {} com chave recusou: {e}", i + 1);
            }
        }

        let disco = std::fs::read(&caminho).unwrap();
        // Nao e so ausencia: o arquivo diz o formato e traz os envelopes.
        let j = Json::analisar(std::str::from_utf8(&disco).unwrap()).unwrap();
        assert_eq!(j.campo("formato").and_then(Json::inteiro), Some(2));
        assert!(j.campo("cifra_do_cadastro").is_some());
        let texto = std::str::from_utf8(&disco).unwrap();
        assert!(
            texto.contains("\"senha_cifrada\""),
            "sem o envelope da senha"
        );
        assert!(texto.contains("\"token_remoto_cifrado\""), "sem o do token");
        // Nem a chave vai para o arquivo, nem para o `Debug` do cadastro.
        assert_eq!(ocorrencias(&disco, CHAVE_A.as_bytes()), 0);
        assert!(
            !format!("{r:?}").contains(CHAVE_A),
            "a chave vazou pelo Debug"
        );

        // Ida e volta: com a mesma chave, as duas voltam inteiras.
        let lido = Registro::abrir_com(&caminho, &cifra).unwrap();
        assert!(
            lido.achar("loja").unwrap().senha().ok() == Some(SENHA_372),
            "a senha nao voltou igual do envelope"
        );
        assert!(
            lido.achar("filial").unwrap().token_remoto().ok() == Some(TOKEN_372),
            "o token nao voltou igual do envelope"
        );
        // E a tela continua sem ver valor nenhum.
        let ficha = lido.achar("loja").unwrap().para_json().escrever();
        assert!(!ficha.contains(SENHA_372));
        assert!(lido.avisos().is_empty(), "{:?}", lido.avisos());
    }

    /// (b) O envelope da ligacao A colado na linha da ligacao B NAO abre -- o
    /// dado associado amarra o nome.
    ///
    /// Com o defeito reposto (o nome fora do dado associado) a B abre o
    /// envelope da A e passaria a apresentar ao banco DELA a senha de outro
    /// banco; o vermelho diz quantos bytes da senha alheia ela entregaria.
    #[test]
    fn o_envelope_movido_de_uma_ligacao_para_outra_nao_abre() {
        let casa = DirTemp::novo("dblink-372-b");
        let fora = DirTemp::novo("dblink-372-b-chave");
        let cifra = chave_no_arquivo(&fora, "mestra.hex", CHAVE_A);
        let caminho = casa.join("dblink.json");
        let mut r = Registro::abrir_com(&caminho, &cifra).unwrap();
        salvar_json(
            &mut r,
            &format!(r#"{{"nome":"loja","senha":"{SENHA_372}"}}"#),
        )
        .unwrap();
        salvar_json(&mut r, r#"{"nome":"erp","senha":"outra-senha-do-erp"}"#).unwrap();

        // A edicao de texto que o AAD existe para impedir: o envelope da
        // `loja` no lugar do envelope do `erp`.
        let texto = std::fs::read_to_string(&caminho).unwrap();
        let j = Json::analisar(&texto).unwrap();
        let envelope = |nome: &str| {
            j.campo("ligacoes")
                .and_then(Json::lista)
                .unwrap()
                .iter()
                .find(|l| l.texto_ou("nome", "") == nome)
                .unwrap()
                .texto_ou("senha_cifrada", "")
                .to_string()
        };
        let (da_loja, do_erp) = (envelope("loja"), envelope("erp"));
        assert!(!da_loja.is_empty() && !do_erp.is_empty());
        std::fs::write(&caminho, texto.replace(&do_erp, &da_loja)).unwrap();

        let movido = Registro::abrir_com(&caminho, &cifra).expect("o cadastro recusou o arranque");
        let erp = movido.achar("erp").unwrap();
        let motivo = match erp.senha() {
            Ok(v) => panic!(
                "o envelope da ligacao \"loja\" ABRIU na ligacao \"erp\": ela \
                 apresentaria ao banco dela os {} bytes da senha de outro banco \
                 (igual a da loja: {})",
                v.len(),
                v == SENHA_372
            ),
            Err(e) => e.to_string(),
        };
        assert!(motivo.contains("MOVIDO"), "{motivo}");
        assert!(motivo.contains("\"erp\""), "{motivo}");
        // A dona legitima continua abrindo: o defeito e do lugar, nao da chave.
        assert!(movido.achar("loja").unwrap().senha().ok() == Some(SENHA_372));
        // E o aviso do arranque nomeia a ligacao trancada.
        let avisos = movido.avisos();
        assert!(
            avisos
                .iter()
                .any(|a| a.contains("MOVIDO") && a.contains("\"erp\"")),
            "{avisos:?}"
        );

        // A senha e o token da MESMA ligacao tambem nao trocam de lugar: o
        // campo esta no dado associado.
        let mut r = Registro::abrir_com(&caminho, &cifra).unwrap();
        salvar_json(
            &mut r,
            &format!(
                r#"{{"nome":"cruz","motor":"phxsql","senha":"s1","token_remoto":"{TOKEN_372}"}}"#
            ),
        )
        .unwrap();
        let texto = std::fs::read_to_string(&caminho).unwrap();
        let j = Json::analisar(&texto).unwrap();
        let cruz = j
            .campo("ligacoes")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .find(|l| l.texto_ou("nome", "") == "cruz")
            .unwrap();
        let tok = cruz.texto_ou("token_remoto_cifrado", "").to_string();
        let sen = cruz.texto_ou("senha_cifrada", "").to_string();
        std::fs::write(&caminho, texto.replace(&sen, &tok)).unwrap();
        let trocado = Registro::abrir_com(&caminho, &cifra).unwrap();
        assert!(
            trocado.achar("cruz").unwrap().senha().is_err(),
            "o token virou a senha da mesma ligacao"
        );
    }

    /// (c) A chave ERRADA e recusada na ABERTURA, pela prova -- e nada novo e
    /// selado com ela.
    ///
    /// O dano que a prova impede e o que se mede: sem ela, o material abre
    /// com a chave errada, a ligacao salva em seguida sai selada com ELA, e
    /// quando a chave certa volta essa ligacao fica trancada para sempre. O
    /// vermelho conta quantas.
    #[test]
    fn a_chave_errada_e_recusada_pela_prova_e_nao_sela_nada_com_ela() {
        let casa = DirTemp::novo("dblink-372-c");
        let fora = DirTemp::novo("dblink-372-c-chave");
        let certa = chave_no_arquivo(&fora, "certa.hex", CHAVE_A);
        let errada = chave_no_arquivo(&fora, "errada.hex", CHAVE_B);
        let caminho = casa.join("dblink.json");
        let mut r = Registro::abrir_com(&caminho, &certa).unwrap();
        salvar_json(
            &mut r,
            &format!(r#"{{"nome":"loja","senha":"{SENHA_372}"}}"#),
        )
        .unwrap();

        let mut r =
            Registro::abrir_com(&caminho, &errada).expect("a chave errada derrubou o arranque");
        let salvou = salvar_json(&mut r, r#"{"nome":"nova","senha":"senha-da-nova"}"#);

        // O dano primeiro: com a chave certa de volta, quantas nao abrem?
        let volta = Registro::abrir_com(&caminho, &certa).unwrap();
        let perdidas: Vec<&str> = volta
            .ligacoes
            .iter()
            .filter(|l| l.senha().is_err())
            .map(|l| l.nome.as_str())
            .collect();
        assert!(
            perdidas.is_empty(),
            "a chave ERRADA selou {} ligacao(oes) ({perdidas:?}): com a chave \
             certa de volta, elas ficaram trancadas para sempre",
            perdidas.len()
        );

        // O outro dano possivel: sem ter com o que selar, gravar em claro.
        let disco = std::fs::read(&caminho).unwrap();
        let vazou = ocorrencias(&disco, b"senha-da-nova");
        assert!(
            vazou == 0,
            "com a chave errada, a senha da ligacao nova foi ao cadastro cifrado \
             em TEXTO PURO: {vazou} ocorrencia(s) em {} bytes",
            disco.len()
        );

        // O veredito, depois: a abertura ja sabia, e a gravacao recusou.
        let motivo = match r.achar("loja").unwrap().senha() {
            Ok(_) => panic!("a chave errada abriu o envelope"),
            Err(e) => e.to_string(),
        };
        assert!(motivo.contains("nao e a que gravou"), "{motivo}");
        assert!(motivo.contains("chave mestra"), "{motivo}");
        match salvou {
            Ok(_) => panic!("a gravacao com a chave errada foi aceita"),
            Err(e) => assert!(e.to_string().contains("TEXTO PURO"), "{e}"),
        }
        // A recusa nao deixou a ligacao na memoria: o proximo arranque nao a
        // veria, e ela conectaria ate la.
        assert!(
            r.achar("nova").is_err(),
            "a ligacao recusada ficou na memoria"
        );
    }

    /// (d, no cadastro) Sem chave nenhuma, o cifrado ABRE: a ligacao cifrada
    /// fica trancada com o motivo, a vizinha funciona, e o envelope volta ao
    /// disco IGUAL quando outra ligacao e salva.
    ///
    /// Com o defeito reposto (a tranca virando erro da abertura), o
    /// `Registro::abrir_com` recusa -- e e ele que o `Servidor::novo` chama
    /// com `?`: o motor inteiro fora do ar por uma ligacao do DbLink.
    #[test]
    fn sem_a_chave_o_cadastro_cifrado_abre_trancado_e_o_envelope_volta_igual() {
        let casa = DirTemp::novo("dblink-372-d");
        let fora = DirTemp::novo("dblink-372-d-chave");
        let cifra = chave_no_arquivo(&fora, "mestra.hex", CHAVE_A);
        let caminho = casa.join("dblink.json");
        let mut r = Registro::abrir_com(&caminho, &cifra).unwrap();
        salvar_json(
            &mut r,
            &format!(r#"{{"nome":"loja","senha":"{SENHA_372}"}}"#),
        )
        .unwrap();
        salvar_json(&mut r, r#"{"nome":"publica","usuario":"leitor"}"#).unwrap();
        let antes = std::fs::read_to_string(&caminho).unwrap();
        let envelope = Json::analisar(&antes)
            .unwrap()
            .campo("ligacoes")
            .and_then(Json::lista)
            .unwrap()[0]
            .texto_ou("senha_cifrada", "")
            .to_string();

        for sem in [
            CifraDoDblink::default(),
            cifra_de(
                r#"{"cifra_do_dblink":{"chave_mestra_env":"PHXSQL_TESTE_372_CHAVE_QUE_NINGUEM_EXPORTA"}}"#,
            ),
        ] {
            let mut r = match Registro::abrir_com(&caminho, &sem) {
                Ok(r) => r,
                Err(e) => panic!(
                    "sem a chave mestra o cadastro RECUSOU abrir, e o Servidor::novo \
                     o abre com `?`: o motor inteiro fora do ar por uma ligacao do \
                     DbLink ({e})"
                ),
            };
            let motivo = match r.achar("loja").unwrap().senha() {
                Ok(_) => panic!("a ligacao cifrada abriu sem chave"),
                Err(e) => e.to_string(),
            };
            assert!(motivo.contains("CIFRADO"), "{motivo}");
            assert_eq!(r.achar("publica").unwrap().senha().unwrap(), "");
            let ficha = r.achar("loja").unwrap().para_json();
            assert_eq!(ficha.texto_ou("senha", ""), "(cifra trancada)");
            assert!(!ficha.texto_ou("cifra_trancada", "").is_empty());
            assert!(
                r.avisos()
                    .iter()
                    .any(|a| a.contains("TRANCADAS") && a.contains("\"loja\"")),
                "{:?}",
                r.avisos()
            );
            // Salvar OUTRA ligacao sem credencial passa -- e o envelope da
            // trancada volta ao disco byte a byte.
            salvar_json(
                &mut r,
                r#"{"nome":"outra","usuario":"x","senha_env":"PATH"}"#,
            )
            .unwrap();
            let depois = std::fs::read_to_string(&caminho).unwrap();
            assert!(
                depois.contains(&envelope),
                "a gravacao sem chave perdeu o envelope"
            );
            // Credencial NOVA em claro num cadastro cifrado e recusada -- e o
            // dano se mede no DISCO antes do veredito.
            let salvou = salvar_json(&mut r, r#"{"nome":"clara","senha":"em-claro-372"}"#);
            let disco = std::fs::read(&caminho).unwrap();
            let vazou = ocorrencias(&disco, b"em-claro-372");
            assert!(
                vazou == 0,
                "sem a chave, a gravacao escreveu a credencial nova em TEXTO PURO \
                 num cadastro cifrado: {vazou} ocorrencia(s) em {} bytes -- o \
                 arquivo que o dono pediu cifrado foi rebaixado calado",
                disco.len()
            );
            match salvou {
                Ok(_) => panic!("gravou credencial em claro num cadastro cifrado"),
                Err(e) => assert!(e.to_string().contains("rebaixaria"), "{e}"),
            }
            assert!(
                r.achar("clara").is_err(),
                "a ligacao recusada ficou na memoria"
            );
            let _ = r.excluir("outra");
        }
        // E com a chave de volta, a trancada abre: nada se perdeu.
        let volta = Registro::abrir_com(&caminho, &cifra).unwrap();
        assert!(volta.achar("loja").unwrap().senha().ok() == Some(SENHA_372));
    }

    /// (e) O COMPORTAMENTO VELHO, que e o que mais importa numa guarda nova: o
    /// `dblink.json` de hoje, sem chave declarada, abre e regrava igual -- sem
    /// `formato`, sem `cifra_do_cadastro`, com a senha onde sempre esteve.
    ///
    /// Com o defeito reposto (a gravacao sempre no formato 2), o vermelho diz
    /// quais campos o arquivo de quem nao pediu nada ganhou.
    #[test]
    fn o_cadastro_de_hoje_sem_chave_abre_e_regrava_igual() {
        let casa = DirTemp::novo("dblink-372-e");
        let caminho = casa.join("dblink.json");
        // Na ordem em que o `para_disco` sempre escreveu.
        let de_hoje = r#"{"dblink":[{"nome":"loja","motor":"mysql","host":"10.0.0.5","porta":3306,"usuario":"u","database":"erp","descricao":"","somente_leitura":true,"timeout_s":10,"max_linhas":1000,"senha":"abc"},{"nome":"filial","motor":"phxsql","host":"10.0.0.6","porta":5000,"usuario":"","database":"","descricao":"","somente_leitura":true,"timeout_s":10,"max_linhas":1000,"senha":"","token_remoto":"TOK"}]}"#;
        std::fs::write(&caminho, de_hoje).unwrap();
        let mut r = Registro::abrir(&caminho).unwrap();
        assert_eq!(r.achar("loja").unwrap().senha().unwrap(), "abc");
        assert_eq!(r.achar("filial").unwrap().token_remoto().unwrap(), "TOK");
        assert!(!r.cifrado());
        let g = r.gravar().unwrap();
        assert_eq!(
            g,
            Gravacao {
                ligacoes_cifradas: 0,
                cifrado: false
            }
        );
        let regravado = std::fs::read_to_string(&caminho).unwrap();
        let j = Json::analisar(&regravado).unwrap();
        let ganhou: Vec<&str> = j.chaves().into_iter().filter(|k| *k != "dblink").collect();
        assert!(
            ganhou.is_empty(),
            "o cadastro de hoje, SEM chave declarada, ganhou {ganhou:?} ao ser \
             regravado: um binario anterior le a senha vazia dali em diante"
        );
        assert_eq!(
            j.escrever(),
            Json::analisar(de_hoje).unwrap().escrever(),
            "regravar sem chave mudou o conteudo"
        );
        assert!(!regravado.contains("_cifrad"), "{regravado}");
    }

    /// A migracao e PEDIDA e DITA: com a chave declarada, abrir NAO reescreve
    /// nada; a primeira gravacao sela, e conta so as que estavam em claro no
    /// disco -- a ligacao nova, que nasce selada, nao entra na conta.
    #[test]
    fn a_primeira_gravacao_com_chave_migra_e_diz_quantas() {
        let casa = DirTemp::novo("dblink-372-migra");
        let fora = DirTemp::novo("dblink-372-migra-chave");
        let cifra = chave_no_arquivo(&fora, "mestra.hex", CHAVE_A);
        let caminho = casa.join("dblink.json");
        std::fs::write(
            &caminho,
            format!(
                r#"{{"dblink":[
                    {{"nome":"loja","senha":"{SENHA_372}"}},
                    {{"nome":"filial","motor":"phxsql","token_remoto":"{TOKEN_372}"}},
                    {{"nome":"env","senha_env":"PATH"}}]}}"#
            ),
        )
        .unwrap();
        let antes = std::fs::read(&caminho).unwrap();
        let mut r = Registro::abrir_com(&caminho, &cifra).unwrap();
        assert_eq!(
            std::fs::read(&caminho).unwrap(),
            antes,
            "abrir reescreveu o arquivo"
        );
        let avisos = r.avisos();
        assert!(
            avisos
                .iter()
                .any(|a| a.contains("TEXTO PURO") && a.contains("PROXIMA gravacao")),
            "{avisos:?}"
        );

        let g = salvar_json(&mut r, r#"{"nome":"nova","senha":"nasce-selada"}"#).unwrap();
        assert_eq!(
            g,
            Gravacao {
                ligacoes_cifradas: 2,
                cifrado: true
            },
            "a migracao nao disse quantas sairam do texto puro"
        );
        let disco = std::fs::read(&caminho).unwrap();
        for claro in [SENHA_372, TOKEN_372, "nasce-selada"] {
            assert_eq!(ocorrencias(&disco, claro.as_bytes()), 0, "ficou em claro");
        }
        // Depois de migrado, nada mais «sai do texto puro».
        let g = salvar_json(&mut r, r#"{"nome":"outra","senha":"x2"}"#).unwrap();
        assert_eq!(g.ligacoes_cifradas, 0);
        assert!(r.avisos().is_empty(), "{:?}", r.avisos());
        let lido = Registro::abrir_com(&caminho, &cifra).unwrap();
        assert!(lido.achar("loja").unwrap().senha().ok() == Some(SENHA_372));
        assert_eq!(lido.achar("env").unwrap().senha_env, "PATH");
    }

    /// A senha mestra (PBKDF2) pelo `_env`, lida pelo `Segredo::ler` -- o
    /// caminho que a producao usa. Variavel so deste teste.
    #[test]
    fn a_senha_mestra_do_ambiente_sela_e_abre() {
        const VAR: &str = "PHXSQL_TESTE_372_SENHA_MESTRA_PRESENTE";
        std::env::set_var(VAR, "senha mestra do teste 372");
        let cifra = cifra_de(&format!(
            r#"{{"cifra_do_dblink":{{"senha_mestra_env":"{VAR}"}}}}"#
        ));
        let casa = DirTemp::novo("dblink-372-senha");
        let caminho = casa.join("dblink.json");
        let mut r = Registro::abrir_com(&caminho, &cifra).unwrap();
        salvar_json(
            &mut r,
            &format!(r#"{{"nome":"loja","senha":"{SENHA_372}"}}"#),
        )
        .unwrap();
        let disco = std::fs::read_to_string(&caminho).unwrap();
        assert!(!disco.contains(SENHA_372));
        assert!(disco.contains("\"iteracoes\": 210000"), "{disco}");
        let lido = Registro::abrir_com(&caminho, &cifra).unwrap();
        assert!(lido.achar("loja").unwrap().senha().ok() == Some(SENHA_372));
    }

    /// O formato mais NOVO que este binario e recusado alto -- regravar por
    /// cima apagaria o que ele nao entende.
    #[test]
    fn o_formato_mais_novo_e_recusado_alto() {
        let casa = DirTemp::novo("dblink-372-formato");
        let caminho = casa.join("dblink.json");
        std::fs::write(&caminho, r#"{"formato":3,"dblink":[]}"#).unwrap();
        let e = match Registro::abrir(&caminho) {
            Ok(_) => panic!("o formato 3 abriu num binario que so conhece o 2"),
            Err(e) => e,
        };
        assert_eq!(e.nome(), "VERSAO_NAO_SUPORTADA", "{e}");
        let texto = e.to_string();
        assert!(texto.contains('3') && texto.contains("le ate 2"), "{texto}");
        // O 1 escrito e o 2 continuam abrindo.
        std::fs::write(&caminho, r#"{"formato":1,"dblink":[]}"#).unwrap();
        assert!(Registro::abrir(&caminho).is_ok());
    }

    /// O envelope e campo do ARQUIVO: pelo pedido ele e recusado nomeando o
    /// campo. E ao lado da senha em claro e recusado nos dois sentidos.
    #[test]
    fn o_envelope_so_vale_no_arquivo_e_nunca_ao_lado_do_claro() {
        match lig(r#"{"nome":"loja","senha_cifrada":"00"}"#) {
            Ok(_) => panic!("o pedido plantou um envelope no cadastro"),
            Err(e) => assert!(e.to_string().contains("senha_cifrada"), "{e}"),
        }
        let casa = DirTemp::novo("dblink-372-exclusivos");
        let caminho = casa.join("dblink.json");
        for linha in [
            r#"{"nome":"loja","senha":"x","senha_cifrada":"00"}"#,
            r#"{"nome":"loja","senha_env":"PATH","senha_cifrada":"00"}"#,
            r#"{"nome":"loja","motor":"phxsql","token_remoto":"x","token_remoto_cifrado":"00"}"#,
        ] {
            std::fs::write(&caminho, format!(r#"{{"dblink":[{linha}]}}"#)).unwrap();
            match Registro::abrir(&caminho) {
                Ok(_) => panic!("a leitura escolheu calada entre o claro e o cifrado: {linha}"),
                Err(e) => assert!(e.to_string().contains("mutuamente exclusivos"), "{e}"),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Pedido 372, segunda rodada: as revisoes SEC e DBA do formato 2
    // -----------------------------------------------------------------------

    /// Os campos que o `para_disco` do binario ANTERIOR a este escreve -- e so
    /// eles: o que ele nao conhece, ele nao regrava. Copia do HEAD de antes do
    /// 372 (`dblink/mod.rs`, o `para_disco` sem selo), para simular o estrago.
    const CAMPOS_DO_BINARIO_ANTERIOR: [&str; 17] = [
        "nome",
        "motor",
        "host",
        "porta",
        "usuario",
        "database",
        "descricao",
        "somente_leitura",
        "timeout_s",
        "max_linhas",
        "senha",
        "senha_env",
        "token_remoto",
        "token_remoto_env",
        "sincronias",
        "cifra",
        "chave_do_fio",
    ];

    /// Quantos envelopes (`*_cifrada`/`*_cifrado`) uma lista de ligacoes tem.
    fn envelopes(lista: &[Json]) -> usize {
        lista
            .iter()
            .flat_map(|l| l.chaves())
            .filter(|k| k.ends_with("_cifrada") || k.ends_with("_cifrado"))
            .count()
    }

    /// (M3 da SEC, pedido 1 do DBA) O formato 2 NAO entrega a lista ao leitor
    /// do binario anterior -- e por isso ele recusa subir em vez de apagar.
    ///
    /// O leitor legado e `campo("dblink").lista()` ou a lista crua. Com o
    /// defeito reposto (a lista de volta em `"dblink"`), ele a acha, e a
    /// primeira gravacao dele -- simulada aqui pelos campos que o `para_disco`
    /// antigo conhece -- apaga os envelopes. O vermelho conta quantos sobram.
    #[test]
    fn o_formato_2_nao_entrega_a_lista_ao_leitor_legado() {
        let casa = DirTemp::novo("dblink-372-legado");
        let fora = DirTemp::novo("dblink-372-legado-chave");
        let cifra = chave_no_arquivo(&fora, "mestra.hex", CHAVE_A);
        let caminho = casa.join("dblink.json");
        let mut r = Registro::abrir_com(&caminho, &cifra).unwrap();
        salvar_json(
            &mut r,
            &format!(r#"{{"nome":"loja","senha":"{SENHA_372}"}}"#),
        )
        .unwrap();
        salvar_json(
            &mut r,
            &format!(r#"{{"nome":"filial","motor":"phxsql","token_remoto":"{TOKEN_372}"}}"#),
        )
        .unwrap();
        let j = Json::analisar(&std::fs::read_to_string(&caminho).unwrap()).unwrap();
        // Contados onde quer que a lista esteja: o dano se mede mesmo quando a
        // lista voltou para a chave do binario anterior.
        let lista_nova = j
            .campo("ligacoes")
            .or_else(|| j.campo("dblink"))
            .and_then(Json::lista)
            .unwrap_or(&[]);
        let antes = envelopes(lista_nova);
        assert_eq!(antes, 2, "o cadastro nao selou as duas credenciais");

        // O que o binario anterior faria com este arquivo.
        let legado = j
            .campo("dblink")
            .and_then(Json::lista)
            .or_else(|| j.lista());
        if let Some(lista) = legado {
            let regravada: Vec<Json> = lista
                .iter()
                .map(|l| {
                    Json::objeto(
                        CAMPOS_DO_BINARIO_ANTERIOR
                            .iter()
                            .filter_map(|c| l.campo(c).map(|v| (*c, v.clone())))
                            .collect(),
                    )
                })
                .collect();
            let sobraram = envelopes(&regravada);
            panic!(
                "o binario anterior ACHA a lista do formato 2 ({} ligacoes) e a \
                 primeira gravacao dele deixa {sobraram} de {antes} envelopes: \
                 apaga {} credencial(is) cifrada(s), calado, em vez de recusar subir",
                lista.len(),
                antes - sobraram
            );
        }
        // E este binario continua lendo o que gravou.
        let lido = Registro::abrir_com(&caminho, &cifra).unwrap();
        assert!(lido.achar("loja").unwrap().senha().ok() == Some(SENHA_372));

        // O material num arquivo que NAO se declara formato 2 e o mesmo
        // estrago por outra porta: recusado.
        let sem_formato = Json::objeto(vec![
            (
                "cifra_do_cadastro",
                j.campo("cifra_do_cadastro").unwrap().clone(),
            ),
            ("dblink", Json::Lista(lista_nova.to_vec())),
        ]);
        std::fs::write(&caminho, sem_formato.escrever()).unwrap();
        match Registro::abrir_com(&caminho, &cifra) {
            Ok(_) => panic!("cifra_do_cadastro abriu num arquivo sem formato 2"),
            Err(e) => assert!(e.to_string().contains("formato"), "{e}"),
        }
    }

    /// (B1 da SEC) O envelope NAO entrega o tamanho da credencial: toda
    /// credencial ate 126 bytes cai no mesmo degrau.
    ///
    /// Com o defeito reposto (degrau de 1 byte), o cifrado tem o tamanho exato
    /// do claro, e o vermelho diz quanto cada uma mede no arquivo.
    #[test]
    fn o_envelope_nao_entrega_o_tamanho_da_credencial() {
        let casa = DirTemp::novo("dblink-372-degrau");
        let fora = DirTemp::novo("dblink-372-degrau-chave");
        let cifra = chave_no_arquivo(&fora, "mestra.hex", CHAVE_A);
        let caminho = casa.join("dblink.json");
        let mut r = Registro::abrir_com(&caminho, &cifra).unwrap();
        let curta = "x";
        let longa = "y".repeat(120);
        salvar_json(&mut r, &format!(r#"{{"nome":"curta","senha":"{curta}"}}"#)).unwrap();
        salvar_json(&mut r, &format!(r#"{{"nome":"longa","senha":"{longa}"}}"#)).unwrap();
        let j = Json::analisar(&std::fs::read_to_string(&caminho).unwrap()).unwrap();
        let tamanho = |nome: &str| {
            j.campo("ligacoes")
                .and_then(Json::lista)
                .unwrap()
                .iter()
                .find(|l| l.texto_ou("nome", "") == nome)
                .unwrap()
                .texto_ou("senha_cifrada", "")
                .len()
        };
        let (c, l) = (tamanho("curta"), tamanho("longa"));
        assert!(
            c == l,
            "o envelope entrega o tamanho da credencial: a senha de {} byte vira \
             {c} caracteres no arquivo, a de {} bytes vira {l}",
            curta.len(),
            longa.len()
        );
        // E o degrau volta inteiro: nem um byte a mais, nem um a menos.
        let lido = Registro::abrir_com(&caminho, &cifra).unwrap();
        assert!(lido.achar("curta").unwrap().senha().ok() == Some(curta));
        assert!(lido.achar("longa").unwrap().senha().ok() == Some(longa.as_str()));
        // A que passa do degrau sobe para o proximo, e nao se corta.
        let enorme = "z".repeat(300);
        salvar_json(
            &mut r,
            &format!(r#"{{"nome":"enorme","senha":"{enorme}"}}"#),
        )
        .unwrap();
        let lido = Registro::abrir_com(&caminho, &cifra).unwrap();
        assert!(lido.achar("enorme").unwrap().senha().ok() == Some(enorme.as_str()));
    }

    /// (B2 da SEC) As iteracoes lidas do ARQUIVO tem teto: quem escreve no
    /// `dblink.json` nao escolhe quanto o arranque paga de PBKDF2.
    ///
    /// Sem chave declarada, para a conta nao depender de PBKDF2 nenhum: a
    /// recusa vem na leitura do material, antes de qualquer derivacao. Com o
    /// defeito reposto (sem teto), o cadastro abre, e o vermelho diz quanto o
    /// arranque pagaria com a senha mestra -- pela conta do custo medido
    /// (210.000 iteracoes = 290,3 ms), e nao rodando.
    #[test]
    fn o_teto_de_iteracoes_do_arquivo_nao_deixa_o_arranque_refem() {
        let casa = DirTemp::novo("dblink-372-teto");
        let caminho = casa.join("dblink.json");
        let material = |iteracoes: u64| {
            format!(
                r#"{{"formato":2,"cifra_do_cadastro":{{"sal":"{}","iteracoes":{iteracoes},
                     "modo":"aead","prova":"{}"}},"ligacoes":[]}}"#,
                "00".repeat(16),
                "00".repeat(16)
            )
        };
        std::fs::write(&caminho, material(u32::MAX as u64)).unwrap();
        if Registro::abrir(&caminho).is_ok() {
            let minutos = u32::MAX as f64 / 210_000.0 * 0.2903 / 60.0;
            panic!(
                "o cadastro aceitou {} iteracoes do arquivo: com a senha mestra, o \
                 arranque pagaria ~{minutos:.0} min de PBKDF2 antes de a porta abrir",
                u32::MAX
            );
        }
        // O teto proprio passa, e um acima dele nao.
        std::fs::write(&caminho, material(ITERACOES_MAXIMAS_DO_CADASTRO as u64)).unwrap();
        assert!(Registro::abrir(&caminho).is_ok());
        std::fs::write(&caminho, material(ITERACOES_MAXIMAS_DO_CADASTRO as u64 + 1)).unwrap();
        assert!(Registro::abrir(&caminho).is_err());
    }

    /// (B2 da SEC) O piso de iteracoes do cadastro e o PADRAO da casa, no
    /// arquivo e na declaracao -- a prova e um oraculo offline, e o formato nao
    /// tem legado a preservar.
    ///
    /// Com o defeito reposto (o piso de 10.000 do cofre), as duas portas
    /// aceitam, e o vermelho diz quantas vezes mais barata fica cada tentativa.
    #[test]
    fn o_piso_de_iteracoes_do_cadastro_e_o_padrao_da_casa() {
        let casa = DirTemp::novo("dblink-372-piso");
        let caminho = casa.join("dblink.json");
        let baixo = phxsql_store::cofre::ITERACOES_MINIMAS;
        std::fs::write(
            &caminho,
            format!(
                r#"{{"formato":2,"cifra_do_cadastro":{{"sal":"{}","iteracoes":{baixo},
                     "modo":"aead","prova":"{}"}},"ligacoes":[]}}"#,
                "00".repeat(16),
                "00".repeat(16)
            ),
        )
        .unwrap();
        let barato = phxsql_store::cofre::ITERACOES_PADRAO / baixo;
        assert!(
            Registro::abrir(&caminho).is_err(),
            "o cadastro aceitou {baixo} iteracoes: cada tentativa contra a prova sai \
             {barato}x mais barata que o padrao da casa"
        );
        let declarado = cifra_de(&format!(
            r#"{{"cifra_do_dblink":{{"senha_mestra_env":"PATH","iteracoes":{baixo}}}}}"#
        ));
        assert!(
            declarado.validar().is_err(),
            "a declaracao aceitou {baixo} iteracoes: {barato}x mais barata que o padrao"
        );
        // O padrao passa, sem campo nenhum.
        let padrao = cifra_de(r#"{"cifra_do_dblink":{"senha_mestra_env":"PATH"}}"#);
        padrao.validar().unwrap();
        assert_eq!(padrao.iteracoes, ITERACOES_MINIMAS_DO_CADASTRO);
    }

    /// O procedimento da chave PERDIDA, que o FORMATO §19 escreve: apagar
    /// `cifra_do_cadastro` e os envelopes -- e so isso -- devolve o cadastro a
    /// vida com outra chave. O que estava cifrado com a perdida nao volta, e o
    /// teste nao finge que volta: a senha renasce vazia e se redigita.
    #[test]
    fn a_chave_perdida_se_resolve_apagando_o_material_e_os_envelopes() {
        let casa = DirTemp::novo("dblink-372-perdida");
        let fora = DirTemp::novo("dblink-372-perdida-chave");
        let velha = chave_no_arquivo(&fora, "velha.hex", CHAVE_A);
        let nova = chave_no_arquivo(&fora, "nova.hex", CHAVE_B);
        let caminho = casa.join("dblink.json");
        let mut r = Registro::abrir_com(&caminho, &velha).unwrap();
        salvar_json(
            &mut r,
            &format!(r#"{{"nome":"loja","host":"h1","senha":"{SENHA_372}"}}"#),
        )
        .unwrap();
        // Com a chave nova, a gravacao de credencial nova recusa: e o beco.
        let mut r = Registro::abrir_com(&caminho, &nova).unwrap();
        assert!(salvar_json(&mut r, r#"{"nome":"outra","senha":"x"}"#).is_err());

        // A edicao: fora o material e os envelopes; o resto fica.
        let j = Json::analisar(&std::fs::read_to_string(&caminho).unwrap()).unwrap();
        let sem_envelopes: Vec<Json> = j
            .campo("ligacoes")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .map(|l| {
                Json::objeto(
                    l.chaves()
                        .into_iter()
                        .filter(|k| !k.ends_with("_cifrada") && !k.ends_with("_cifrado"))
                        .map(|k| (k, l.campo(k).unwrap().clone()))
                        .collect(),
                )
            })
            .collect();
        let editado = Json::objeto(vec![
            ("formato", Json::de_u64(2)),
            ("ligacoes", Json::Lista(sem_envelopes)),
        ]);
        std::fs::write(&caminho, editado.escrever()).unwrap();

        let mut r = Registro::abrir_com(&caminho, &nova).expect("a edicao nao abriu");
        assert_eq!(
            r.achar("loja").unwrap().host,
            "h1",
            "a edicao perdeu a ligacao"
        );
        assert_eq!(
            r.achar("loja").unwrap().senha().unwrap(),
            "",
            "a senha renasce vazia"
        );
        salvar_json(
            &mut r,
            r#"{"nome":"loja","host":"h1","senha":"redigitada"}"#,
        )
        .unwrap();
        let lido = Registro::abrir_com(&caminho, &nova).unwrap();
        assert!(lido.cifrado(), "a gravacao com a chave nova nao cifrou");
        assert!(lido.achar("loja").unwrap().senha().ok() == Some("redigitada"));
    }
}
