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

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

use crate::config::Segredo;

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
    pub fn de_json(j: &Json) -> Result<Definicao> {
        let padrao = Definicao::default();
        let motor = Motor::de_texto(j.texto_ou("motor", "mysql"))?;
        let nome = j.texto_ou("nome", "").trim().to_string();
        validar_nome(&nome)?;
        // A variavel que falta NAO derruba a leitura -- esta funcao le o
        // `dblink.json` inteiro no arranque, e uma ligacao mal configurada
        // recusaria o servidor todo. A falta fica guardada no [`Segredo`], e a
        // ligacao nasce TRANCADA: so ela recusa, na hora de conectar, dizendo
        // qual variavel faltou. Ver `Registro::avisos` para o aviso.
        let quem = format!("a ligacao {nome:?} do DbLink");
        let (senha, senha_env) = Segredo::ler(j, "senha", &quem);
        let (token, token_env) = Segredo::ler(j, "token_remoto", &quem);
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
    fn para_disco(&self) -> Result<Json> {
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
        if self.senha_env.is_empty() {
            campos.push(("senha", Json::texto_de(self.senha.valor()?)));
        } else {
            campos.push(("senha_env", Json::texto_de(&self.senha_env)));
        }
        if !self.token_env.is_empty() {
            campos.push(("token_remoto_env", Json::texto_de(&self.token_env)));
        } else if self.token.escrito_no_arquivo() {
            campos.push(("token_remoto", Json::texto_de(self.token.valor()?)));
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

/// O cadastro inteiro, lido e gravado num arquivo so.
#[derive(Debug, Default)]
pub struct Registro {
    pub caminho: PathBuf,
    pub ligacoes: Vec<Definicao>,
}

impl Registro {
    /// Le o arquivo. Arquivo que nao existe e cadastro vazio, e nao erro: um
    /// servidor sem nenhuma ligacao e o caso normal.
    pub fn abrir(caminho: &Path) -> Result<Registro> {
        let mut r = Registro {
            caminho: caminho.to_path_buf(),
            ligacoes: Vec::new(),
        };
        let Ok(texto) = phxsql_core::phz::ler_texto(caminho) else {
            return Ok(r);
        };
        if texto.trim().is_empty() {
            return Ok(r);
        }
        let j = Json::analisar(&texto)?;
        let lista = j
            .campo("dblink")
            .and_then(Json::lista)
            .or_else(|| j.lista())
            .ok_or_else(|| {
                PhxError::Esquema(format!(
                    "{}: esperava uma lista de ligacoes, ou um objeto com \"dblink\"",
                    caminho.display()
                ))
            })?;
        for item in lista {
            r.ligacoes.push(Definicao::de_json(item)?);
        }
        r.conferir_repetidos()?;
        Ok(r)
    }

    /// Os avisos do arranque sobre este cadastro (pedido 372). Quem os junta
    /// e o `Config::ler`, na mesma lista que o `main` ja imprime.
    ///
    /// # Dois avisos, e para quem cada um fala
    ///
    /// **A variavel que falta**, um por falta: a ligacao esta TRANCADA e o
    /// operador precisa saber antes do primeiro teste, e nao por ele.
    ///
    /// **A credencial em texto puro**, um so para o arquivo inteiro. Fala com
    /// quem NAO registrou decisao -- a ligacao com senha ou token escritos no
    /// arquivo -- e cala para quem escreveu `senha_env`/`token_remoto_env`, que
    /// e o molde do aviso das saidas cifradas (`config.rs`): aviso que aparece
    /// para sempre numa instalacao decidida e aviso que ninguem le. E nao ha
    /// escape que o cale mantendo o claro, de proposito: a petrea e «senha
    /// nunca em texto puro», e um interruptor para calar seria registrar a
    /// decisao de quebra-la.
    ///
    /// Nunca a senha, nem pedaco, nem o tamanho: o aviso nomeia a ligacao e o
    /// CAMPO, que e o que diz onde mexer.
    pub fn avisos(&self) -> Vec<String> {
        let mut avisos: Vec<String> = Vec::new();
        let mut em_claro: Vec<String> = Vec::new();
        for l in &self.ligacoes {
            for (segredo, campo) in [(&l.senha, "senha"), (&l.token, "token_remoto")] {
                if let Some(falta) = segredo.falta() {
                    avisos.push(format!(
                        "{falta}. A ligacao fica TRANCADA -- recusa conectar ate o \
                         servidor subir com a variavel --, e o resto do servidor \
                         sobe normalmente."
                    ));
                }
                if segredo.escrito_no_arquivo() {
                    em_claro.push(format!("{:?} ({campo})", l.nome));
                }
            }
        }
        if !em_claro.is_empty() {
            avisos.push(format!(
                "o cadastro do DbLink em {} guarda credencial do outro banco em \
                 TEXTO PURO: {}. O arquivo nasce 0600, mas vai inteiro em toda \
                 copia de backup e em todo disco levado. Tire-a de la escrevendo \
                 senha_env / token_remoto_env com o nome de uma variavel de \
                 ambiente -- a tela de Definicoes do DbLink oferece os dois --, e \
                 TROQUE a credencial no outro banco: a que ja esteve neste disco \
                 continua recuperavel nos blocos antigos e nas copias.",
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
    pub fn salvar(&mut self, d: Definicao) -> Result<()> {
        match self
            .ligacoes
            .iter()
            .position(|l| l.nome.eq_ignore_ascii_case(&d.nome))
        {
            Some(i) => self.ligacoes[i] = d,
            None => self.ligacoes.push(d),
        }
        self.gravar()
    }

    pub fn excluir(&mut self, nome: &str) -> Result<()> {
        let antes = self.ligacoes.len();
        self.ligacoes.retain(|l| !l.nome.eq_ignore_ascii_case(nome));
        if self.ligacoes.len() == antes {
            return Err(PhxError::NaoEncontrado(format!(
                "dblink {nome:?} nao existe"
            )));
        }
        self.gravar()
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
    fn gravar(&self) -> Result<()> {
        let ligacoes = self
            .ligacoes
            .iter()
            .map(Definicao::para_disco)
            .collect::<Result<Vec<_>>>()?;
        let j = Json::objeto(vec![("dblink", Json::Lista(ligacoes))]);
        crate::config::gravar_privado(&self.caminho, j.escrever_identado().as_bytes())
            .map_err(|e| PhxError::Esquema(format!("nao gravei {}: {e}", self.caminho.display())))
    }
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
        let disco = d.para_disco().unwrap().escrever();
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
            let disco = lig(json).unwrap().para_disco().unwrap().escrever();
            assert!(
                !disco.contains("\"cifra\""),
                "o padrao foi fossilizado por {json}: {disco}"
            );
        }
        // Motor alheio nao grava nenhum dos dois -- campo sem leitor.
        let disco = lig(r#"{"nome":"erp","motor":"mysql","cifra":false}"#)
            .unwrap()
            .para_disco()
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
        .para_disco()
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
        assert!(!caminho.with_extension("tmp").exists());
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
        let r = Registro::abrir(&caminho).expect("o cadastro recusou o arranque");
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
        let disco = d.para_disco().unwrap().escrever();
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
        };
        assert!(decidido.avisos().is_empty(), "{:?}", decidido.avisos());
    }
}
