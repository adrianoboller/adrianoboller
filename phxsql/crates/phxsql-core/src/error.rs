//! Erros do PhxSql.

use std::fmt;

/// Resultado padrao do PhxSql.
pub type Result<T> = std::result::Result<T, PhxError>;

#[derive(Debug)]
pub enum PhxError {
    /// Falha de entrada/saida no sistema de arquivos.
    Io(std::io::Error),
    /// Assinatura ("magic") do arquivo nao confere com a esperada.
    BadMagic {
        arquivo: String,
        esperado: &'static [u8; 8],
        encontrado: [u8; 8],
    },
    /// Versao de formato nao suportada por esta build.
    VersaoNaoSuportada {
        arquivo: String,
        encontrada: u16,
        suportada: u16,
    },
    /// Estrutura interna inconsistente (CRC, offset fora do arquivo, etc).
    Corrompido(String),
    /// Esquema invalido ou incompativel com o arquivo aberto.
    Esquema(String),
    /// Valor incompativel com o tipo declarado da coluna.
    Tipo(String),
    /// Registro, indice ou chave inexistente.
    NaoEncontrado(String),
    /// Violacao de indice unico.
    Duplicado(String),
    /// Violacao de integridade referencial: a linha aponta para uma mae que
    /// nao existe, ou a mae tem filha que a impede de sair.
    ///
    /// # Por que nao reaproveitar `NaoEncontrado`
    ///
    /// O que nao foi encontrado nao e o que o cliente pediu -- ele pediu para
    /// GRAVAR, e a gravacao e valida em tudo menos na referencia. Quem recebe
    /// `NAO_ENCONTRADO` procura o proprio pedido; quem recebe `INTEGRIDADE`
    /// sabe que precisa criar a mae antes, ou corrigir o valor. E a mesma
    /// razao que separou `EmTransacao` de `EmCarga`: os dois pedem coisas
    /// diferentes de quem recebe.
    Integridade(String),
    /// Outra sessao mexeu no registro entre a leitura e a gravacao.
    Conflito(String),
    /// A tabela esta reservada para uma carga, por outra sessao.
    EmCarga(String),
    /// A tabela esta reservada por uma TRANSACAO aberta em outra conexao.
    ///
    /// O gemeo do `EmCarga`, e separado dele de proposito: quem esbarra
    /// precisa saber se o que segura a tabela e uma carga (que termina
    /// sozinha) ou uma transacao (que termina no `COMMIT` ou no `ROLLBACK` de
    /// alguem). Os dois pedem a mesma coisa de quem recebe -- tentar de novo
    /// --, e por isso os dois tem `repetir: true`; o que muda e a quem
    /// perguntar quando a espera passar do razoavel.
    EmTransacao(String),
    /// Credencial invalida ou poder insuficiente.
    Autorizacao(String),
    /// Valor excede o limite fisico do formato.
    LimiteExcedido(String),
    /// Alguem mandou encerrar esta atividade e ela chegou num ponto seguro.
    ///
    /// # Por que uma familia so dela
    ///
    /// Nao e recusa de acesso: quem pediu podia, e a operacao ja tinha
    /// comecado. Nao e erro do dado, nem do esquema, nem do disco. E o unico
    /// erro do PhxSql que descreve uma DECISAO de quem administra, tomada
    /// depois de o trabalho comecar -- e quem integra precisa distingui-lo de
    /// uma falha, porque nao ha nada para consertar.
    ///
    /// O arquivo fica INTEIRO: a marca de encerramento so e olhada em ponto
    /// seguro, entre uma unidade de trabalho e a proxima. O que ja foi
    /// gravado esta gravado, e o que faltava nao comecou.
    Cancelado(String),
    /// O pedido tem de ir para OUTRO servidor -- escrita numa replica de
    /// cluster, por exemplo. O CORPO comeca com `REDIRECIONA host:porta`, de
    /// proposito: e o endereco, e nao prosa para interpretar.
    ///
    /// Atencao ao que mudou: no `Display` a moldura da sprint vem antes
    /// (`[SP000028] REDIRECIONA ...`), porque a moldura e de TODAS as
    /// variantes. Quem recortava a posicao zero recorta depois dela -- ou,
    /// melhor, le o campo `sprint`/`nome` da resposta e para de recortar.
    Redireciona(String),
    /// Um `SIGNAL` de gatilho ou de procedimento: a recusa escrita pelo dono
    /// do banco, com o SQLSTATE e a MESSAGE_TEXT que ele escolheu. Num
    /// gatilho BEFORE, cancela a escrita.
    Sinal { estado: String, mensagem: String },
    /// Este servidor e um SPARE de contingencia: nao atende cliente, nem
    /// para ler. Erro proprio porque a acao de quem recebe e outra --
    /// esperar ou promover, nunca insistir.
    SpareEmEspera(String),
    /// A transacao desta conexao esta em `ABORT_ONLY`: houve erro de
    /// TRANSACAO, e o unico caminho que sobra e o `ROLLBACK`.
    ///
    /// # Por que um erro proprio, e nao um `Esquema` qualquer
    ///
    /// Porque a acao de quem recebe e outra, e e a unica coisa que importa
    /// aqui: nao adianta corrigir o pedido e repetir -- o pedido nao e o
    /// problema. Um `COMMIT` que confirmasse trabalho meio invalido seria pior
    /// do que a recusa, e e exatamente isso que este erro impede.
    TransacaoAbortada(String),
}

impl PhxError {
    /// O codigo numerico do erro, estavel para sempre.
    ///
    /// # Por que numero, e nao so texto
    ///
    /// Sem codigo, quem integra com o PhxSql precisa comparar TEXTO para saber
    /// o que aconteceu -- e a hora que alguem melhorar a redacao de uma
    /// mensagem, o cliente quebra sem ninguem perceber. O MySQL(R) mantem mais
    /// de cinco mil codigos justamente por isso: `1062` e chave duplicada
    /// desde sempre, seja qual for a lingua ou a redacao da mensagem.
    ///
    /// A regra que faz o numero valer alguma coisa: **numero nunca muda e
    /// numero aposentado nunca volta**. Trocar o significado de um codigo e
    /// pior do que nao ter codigo, porque o cliente antigo continua tratando
    /// pelo sentido velho.
    ///
    /// As faixas agrupam por familia, para quem quiser tratar em bloco:
    ///
    /// | faixa | familia   | o que e |
    /// |-------|-----------|---------|
    /// | 1000  | `formato` | o arquivo nao e o que dizia ser |
    /// | 2000  | `esquema` | o pedido nao casa com a estrutura |
    /// | 3000  | `dado`    | o dado em si recusa |
    /// | 4000  | `acesso`  | quem pediu nao podia |
    /// | 5000  | `sistema` | o sistema de arquivos falhou
    /// | 6000  | `execucao`| a operacao foi interrompida antes de completar |
    pub fn codigo(&self) -> u16 {
        match self {
            PhxError::Corrompido(_) => 1001,
            PhxError::BadMagic { .. } => 1002,
            PhxError::VersaoNaoSuportada { .. } => 1003,
            PhxError::Esquema(_) => 2001,
            PhxError::Tipo(_) => 2002,
            PhxError::NaoEncontrado(_) => 3001,
            PhxError::Duplicado(_) => 3002,
            PhxError::Integridade(_) => 3006,
            PhxError::LimiteExcedido(_) => 3003,
            PhxError::Conflito(_) => 3004,
            // A recusa de SIGNAL e da familia do DADO: o dado em si (ou a
            // regra que o dono escreveu sobre ele) recusou a operacao.
            PhxError::Sinal { .. } => 3005,
            PhxError::Autorizacao(_) => 4001,
            PhxError::EmCarga(_) => 4002,
            PhxError::Redireciona(_) => 4003,
            PhxError::SpareEmEspera(_) => 4004,
            PhxError::EmTransacao(_) => 4005,
            PhxError::Io(_) => 5001,
            PhxError::Cancelado(_) => 6001,
            PhxError::TransacaoAbortada(_) => 6002,
        }
    }

    /// O nome simbolico, para quem prefere ler a decorar numero.
    ///
    /// Anda junto com o codigo e obedece a mesma regra: nao muda.
    pub fn nome(&self) -> &'static str {
        match self {
            PhxError::Corrompido(_) => "CORROMPIDO",
            PhxError::BadMagic { .. } => "ASSINATURA_INVALIDA",
            PhxError::VersaoNaoSuportada { .. } => "VERSAO_NAO_SUPORTADA",
            PhxError::Esquema(_) => "ESQUEMA_INVALIDO",
            PhxError::Tipo(_) => "TIPO_INVALIDO",
            PhxError::NaoEncontrado(_) => "NAO_ENCONTRADO",
            PhxError::Duplicado(_) => "DUPLICADO",
            PhxError::Integridade(_) => "INTEGRIDADE",
            PhxError::LimiteExcedido(_) => "LIMITE_EXCEDIDO",
            PhxError::Conflito(_) => "CONFLITO",
            PhxError::Sinal { .. } => "SINAL",
            PhxError::Autorizacao(_) => "ACESSO_NEGADO",
            PhxError::EmCarga(_) => "EM_CARGA",
            PhxError::Redireciona(_) => "REDIRECIONA",
            PhxError::SpareEmEspera(_) => "SPARE_EM_ESPERA",
            PhxError::EmTransacao(_) => "EM_TRANSACAO",
            PhxError::Io(_) => "ERRO_DE_ES",
            PhxError::Cancelado(_) => "CANCELADO",
            PhxError::TransacaoAbortada(_) => "TRANSACAO_ABORTADA",
        }
    }

    /// A familia, derivada da faixa do codigo.
    ///
    /// Derivada e nao escrita a mao de proposito: um codigo novo cai na
    /// familia certa sozinho, e as duas nao tem como divergir.
    pub fn classe(&self) -> &'static str {
        match self.codigo() / 1000 {
            1 => "formato",
            2 => "esquema",
            3 => "dado",
            4 => "acesso",
            6 => "execucao",
            _ => "sistema",
        }
    }

    /// Vale a pena tentar de novo?
    ///
    /// Dois erros, e os dois pelo mesmo motivo: sao os unicos que descrevem
    /// uma situacao PASSAGEIRA.
    ///
    /// * o de E/S -- disco cheio que liberou, arquivo que estava travado;
    /// * o de tabela em carga -- alguem reservou a tabela e vai soltar;
    /// * o de tabela em transacao -- alguem a segura ate o `COMMIT` dele.
    ///
    /// Os outros vao dar o mesmo resultado quantas vezes forem tentados, e
    /// repetir e so gastar o servidor. E a distincao que importa para quem
    /// integra: «em carga» e «acesso negado» sao os dois uma recusa, mas a
    /// primeira funciona daqui a pouco e a segunda nao funciona nunca.
    /// A sprint do roteiro que responde por esta recusa.
    ///
    /// Vem no comeco de toda mensagem, e o molde e o do MySQL(R): o
    /// identificador primeiro, dentro de uma moldura fixa, e so depois a
    /// frase -- `ERROR 1146 (42S02) at line 1: Table ... doesn't exist`.
    /// Quem le um log ganha o «onde procurar» sem sair do log.
    ///
    /// # A regra que escolheu cada uma
    ///
    /// Nao e «de que area parece»: e **qual sprint MUDARIA este
    /// comportamento**. Quando duas podiam reivindicar, ganha a que teria de
    /// mexer no codigo para o erro passar a dizer outra coisa.
    ///
    /// # Por que nao entra na tabela de mensagens
    ///
    /// Pela mesma linha que separa rotulo de dado: `SP000008` e
    /// identificador, nao frase. Traduzir seria copiar o numero em seis
    /// idiomas e deixar as seis copias envelhecerem em silencio -- e o MySQL
    /// tambem nao traduz o `1146`. A moldura e posta uma vez, por fora, sobre
    /// o texto que sair (de fabrica ou traduzido).
    pub fn sprint(&self) -> &'static str {
        match self {
            // Arquivo corrompido e falha de durabilidade: quem decide o que
            // fazer com ela e a matriz de falhas que a SP000010 deve.
            PhxError::Corrompido(_) => "SP000010",
            // A assinatura do arquivo e contrato congelado da 1.0.
            PhxError::BadMagic { .. } => "SP000001",
            // Que versao esta build le e decisao de upgrade N/N-1.
            PhxError::VersaoNaoSuportada { .. } => "SP000032",
            // Esquema e tipo saem os dois do binder e do catalogo.
            PhxError::Esquema(_) => "SP000018",
            PhxError::Tipo(_) => "SP000018",
            PhxError::NaoEncontrado(_) => "SP000018",
            // O limite e do TIPO declarado (`Str(10)`), nao do dado.
            PhxError::LimiteExcedido(_) => "SP000018",
            // A unicidade nasce na declaracao do indice, no DDL.
            PhxError::Duplicado(_) => "SP000020",
            // A janela de conflito de escrita e o que o MVCC substitui.
            PhxError::Conflito(_) => "SP000016",
            // `SIGNAL SQLSTATE` e comando do SQL procedural.
            PhxError::Sinal { .. } => "SP000021",
            PhxError::Integridade(_) => "SP000008",
            PhxError::Autorizacao(_) => "SP000025",
            // Reserva de tabela para carga e governanca de recurso.
            PhxError::EmCarga(_) => "SP000012",
            // Quem manda escrever no master e a topologia de replicacao.
            PhxError::Redireciona(_) => "SP000028",
            // Spare e promocao sao eleicao: consenso e split-brain.
            PhxError::SpareEmEspera(_) => "SP000029",
            PhxError::EmTransacao(_) => "SP000006",
            PhxError::TransacaoAbortada(_) => "SP000006",
            PhxError::Io(_) => "SP000010",
            // Cancelamento e literalmente o titulo da SP000012.
            PhxError::Cancelado(_) => "SP000012",
        }
    }

    /// A moldura que abre toda mensagem: `[SP000008] `.
    ///
    /// Existe como funcao para haver **um** lugar que a escreve -- o
    /// `Display` daqui e a tabela de mensagens do servidor precisam produzir
    /// o mesmo byte, e duas copias divergiriam calado.
    pub fn moldura(&self) -> String {
        format!("[{}] ", self.sprint())
    }

    pub fn adianta_repetir(&self) -> bool {
        matches!(
            self,
            PhxError::Io(_) | PhxError::EmCarga(_) | PhxError::EmTransacao(_)
        )
    }
}

impl PhxError {
    /// A frase sem a moldura da sprint.
    ///
    /// Existe porque a tabela de mensagens do servidor compoe o texto dela
    /// (traduzido) e depois poe a moldura por fora: se ela compusesse sobre
    /// o `Display`, a moldura sairia duas vezes. Aqui fica o corpo, e num
    /// lugar so.
    pub fn corpo(&self) -> String {
        match self {
            PhxError::Io(e) => format!("erro de E/S: {e}"),
            PhxError::BadMagic {
                arquivo,
                esperado,
                encontrado,
            } => format!(
                "assinatura invalida em {arquivo}: esperado {:?}, encontrado {:?}",
                String::from_utf8_lossy(esperado.as_slice()),
                String::from_utf8_lossy(encontrado.as_slice())
            ),
            PhxError::VersaoNaoSuportada {
                arquivo,
                encontrada,
                suportada,
            } => format!(
                "versao de formato {encontrada} nao suportada em {arquivo} (esta build le ate {suportada})"
            ),
            PhxError::Corrompido(m) => format!("arquivo corrompido: {m}"),
            PhxError::Esquema(m) => format!("esquema invalido: {m}"),
            PhxError::Tipo(m) => format!("tipo invalido: {m}"),
            PhxError::NaoEncontrado(m) => format!("nao encontrado: {m}"),
            PhxError::Duplicado(m) => format!("chave duplicada: {m}"),
            PhxError::Integridade(m) => format!("integridade referencial: {m}"),
            PhxError::Conflito(m) => format!("conflito de escrita: {m}"),
            PhxError::Autorizacao(m) => format!("acesso negado: {m}"),
            PhxError::EmCarga(m) => format!("tabela em carga: {m}"),
            PhxError::EmTransacao(m) => format!("tabela em transacao: {m}"),
            PhxError::LimiteExcedido(m) => format!("limite excedido: {m}"),
            // Sem prefixo de recusa: a mensagem ja comeca com
            // `REDIRECIONA host:porta`, que e o endereco para onde ir.
            PhxError::Redireciona(m) => m.clone(),
            // A MESSAGE_TEXT na frente, porque ela e a mensagem que o dono do
            // banco escreveu para quem esbarrar na regra; o SQLSTATE vem
            // atras, para o driver que trata por codigo.
            PhxError::Sinal { estado, mensagem } => {
                format!("{mensagem} (SIGNAL SQLSTATE {estado})")
            }
            PhxError::SpareEmEspera(m) => format!("spare em espera: {m}"),
            PhxError::TransacaoAbortada(m) => format!("transacao abortada: {m}"),
            // Sem prefixo de recusa: quem le a resposta esta vendo o
            // resultado de um botao que ele mesmo apertou, e nao uma falha.
            PhxError::Cancelado(m) => m.clone(),
        }
    }
}

impl fmt::Display for PhxError {
    /// Moldura da sprint + corpo, sempre nessa ordem e para TODA variante.
    ///
    /// «Toda» inclusive as tres que nao levam prefixo de recusa
    /// (`REDIRECIONA`, `SINAL`, `CANCELADO`): elas continuam sem o *prefixo
    /// de recusa*, que e outra coisa -- a moldura da sprint e de todas, por
    /// decisao do dono. Quem recortava `REDIRECIONA` da posicao zero passa a
    /// recortar depois da moldura, e por isso o teste do cluster foi
    /// corrigido no mesmo commit em vez de a moldura ser poupada ali.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.moldura(), self.corpo())
    }
}

impl std::error::Error for PhxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PhxError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PhxError {
    fn from(e: std::io::Error) -> Self {
        PhxError::Io(e)
    }
}

#[cfg(test)]
mod testes_codigo {
    use super::*;

    /// Uma de cada variante -- a lista que os testes daqui varrem.
    ///
    /// Estava escrita a mao dentro de um teste so, e tinha FICADO PARA TRAS:
    /// faltava a `Sinal`, que entrou com o `SIGNAL SQLSTATE` e nunca foi
    /// acrescentada. Por isso agora existe o par `nome_da_variante` +
    /// `a_lista_cobre_todas`: o compilador obriga a variante nova a entrar no
    /// `match`, e o teste obriga a entrar AQUI. Lista digitada a mao so nao
    /// envelhece quando alguem a cobra.
    fn todas() -> Vec<PhxError> {
        vec![
            PhxError::Corrompido(String::new()),
            PhxError::BadMagic {
                arquivo: String::new(),
                esperado: b"XXXXXXXX",
                encontrado: *b"YYYYYYYY",
            },
            PhxError::VersaoNaoSuportada {
                arquivo: String::new(),
                encontrada: 1,
                suportada: 2,
            },
            PhxError::Esquema(String::new()),
            PhxError::Tipo(String::new()),
            PhxError::NaoEncontrado(String::new()),
            PhxError::Duplicado(String::new()),
            PhxError::Integridade(String::new()),
            PhxError::LimiteExcedido(String::new()),
            PhxError::Conflito(String::new()),
            PhxError::Sinal {
                estado: "45000".into(),
                mensagem: String::new(),
            },
            PhxError::EmCarga(String::new()),
            PhxError::EmTransacao(String::new()),
            PhxError::TransacaoAbortada(String::new()),
            PhxError::Autorizacao(String::new()),
            PhxError::Redireciona(String::new()),
            PhxError::SpareEmEspera(String::new()),
            PhxError::Io(std::io::Error::other("x")),
            PhxError::Cancelado(String::new()),
        ]
    }

    /// O nome da variante, num `match` SEM braco coringa.
    ///
    /// E aqui que o compilador entra: variante nova nao compila enquanto
    /// ninguem a nomear, e o teste seguinte cobra a entrada na lista.
    fn nome_da_variante(e: &PhxError) -> &'static str {
        match e {
            PhxError::Io(_) => "Io",
            PhxError::BadMagic { .. } => "BadMagic",
            PhxError::VersaoNaoSuportada { .. } => "VersaoNaoSuportada",
            PhxError::Corrompido(_) => "Corrompido",
            PhxError::Esquema(_) => "Esquema",
            PhxError::Tipo(_) => "Tipo",
            PhxError::NaoEncontrado(_) => "NaoEncontrado",
            PhxError::Duplicado(_) => "Duplicado",
            PhxError::Integridade(_) => "Integridade",
            PhxError::Conflito(_) => "Conflito",
            PhxError::Sinal { .. } => "Sinal",
            PhxError::EmCarga(_) => "EmCarga",
            PhxError::EmTransacao(_) => "EmTransacao",
            PhxError::Autorizacao(_) => "Autorizacao",
            PhxError::LimiteExcedido(_) => "LimiteExcedido",
            PhxError::SpareEmEspera(_) => "SpareEmEspera",
            PhxError::Redireciona(_) => "Redireciona",
            PhxError::TransacaoAbortada(_) => "TransacaoAbortada",
            PhxError::Cancelado(_) => "Cancelado",
        }
    }

    /// O outro lado do laco: toda variante nomeada aparece em `todas()`, uma
    /// vez so. Sem isto, o `match` acima ficaria completo e a lista, curta --
    /// que e exatamente o defeito que a `Sinal` teve por meses.
    #[test]
    fn a_lista_cobre_todas() {
        let mut nomes: Vec<&str> = todas().iter().map(nome_da_variante).collect();
        let quantas = nomes.len();
        nomes.sort_unstable();
        nomes.dedup();
        assert_eq!(
            nomes.len(),
            quantas,
            "variante repetida em todas(): {nomes:?}"
        );
        // O numero e a catraca desta lista: variante nova obriga a mexer aqui
        // e a olhar os testes que varrem `todas()`.
        assert_eq!(quantas, 19, "entrou ou saiu variante: {nomes:?}");
    }

    /// **A sprint citada tem de EXISTIR no roteiro.**
    ///
    /// Sem esta prova, `sprint()` seria uma lista de numeros digitados a mao
    /// -- e numero citado e numero que nao se mede. A lista de verdade e o
    /// `docs/ROTEIRO-1.0.md`; aqui so se confere que nao se cita sprint que
    /// nao existe (erro de digitacao, ou sprint que sumiu numa refacao).
    #[test]
    fn nenhuma_sprint_citada_e_inventada() {
        let roteiro =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/ROTEIRO-1.0.md");
        let texto = std::fs::read_to_string(&roteiro)
            .unwrap_or_else(|e| panic!("nao li {}: {e}", roteiro.display()));
        for e in todas() {
            let sp = e.sprint();
            assert!(
                texto.contains(sp),
                "{} cita {sp}, que nao esta no roteiro",
                nome_da_variante(&e)
            );
        }
    }

    /// A moldura abre TODA mensagem, sem excecao.
    ///
    /// Inclusive as tres que nao levam prefixo de RECUSA (`REDIRECIONA`,
    /// `SINAL`, `CANCELADO`): sao coisas diferentes, e a decisao do dono foi
    /// «prefixo em todas, e conserto os clientes».
    #[test]
    fn toda_mensagem_abre_com_a_sprint() {
        for e in todas() {
            let t = e.to_string();
            let esperado = format!("[{}] ", e.sprint());
            assert!(
                t.starts_with(&esperado),
                "{} nao abre com {esperado:?}: {t:?}",
                nome_da_variante(&e)
            );
            // E a moldura entra UMA vez: prefixo duplicado e o defeito que
            // aparece quando alguem compoe sobre o `Display` em vez do corpo.
            assert_eq!(
                t.matches(&esperado).count(),
                1,
                "{} repetiu a moldura: {t:?}",
                nome_da_variante(&e)
            );
        }
    }

    /// O corpo e o texto de antes, byte a byte -- a moldura nao reescreveu
    /// frase nenhuma. E o que separa «acrescentar um prefixo» de «mexer nas
    /// mensagens todas».
    #[test]
    fn o_corpo_continua_o_de_sempre() {
        assert_eq!(
            PhxError::Integridade("mae 7 nao existe".into()).corpo(),
            "integridade referencial: mae 7 nao existe"
        );
        assert_eq!(
            PhxError::NaoEncontrado("rowid 7".into()).corpo(),
            "nao encontrado: rowid 7"
        );
        // As duas sem prefixo de recusa continuam sem ele NO CORPO.
        assert_eq!(
            PhxError::Redireciona("REDIRECIONA 10.0.0.2:5310 -- va la".into()).corpo(),
            "REDIRECIONA 10.0.0.2:5310 -- va la"
        );
        assert_eq!(
            PhxError::Cancelado("voce cancelou".into()).corpo(),
            "voce cancelou"
        );
        // E o Display e a soma dos dois, sem nada no meio.
        let e = PhxError::Cancelado("voce cancelou".into());
        assert_eq!(e.to_string(), format!("{}{}", e.moldura(), e.corpo()));
    }

    /// **O campo estruturado NAO mudou.** E o que importa para quem integra:
    /// cliente que trata por `codigo`/`nome` nao sente a moldura.
    #[test]
    fn a_moldura_nao_mexeu_no_codigo_nem_no_nome() {
        assert_eq!(PhxError::Integridade(String::new()).codigo(), 3006);
        assert_eq!(PhxError::Redireciona(String::new()).nome(), "REDIRECIONA");
        assert_eq!(PhxError::Autorizacao(String::new()).classe(), "acesso");
        assert!(PhxError::EmCarga(String::new()).adianta_repetir());
    }

    /// Dois erros diferentes nao podem compartilhar codigo, senao o numero
    /// nao serve para distinguir nada.
    #[test]
    fn cada_erro_tem_o_seu_codigo() {
        let todos = todas();
        let mut codigos: Vec<u16> = todos.iter().map(PhxError::codigo).collect();
        let quantos = codigos.len();
        codigos.sort_unstable();
        codigos.dedup();
        assert_eq!(codigos.len(), quantos, "ha codigo repetido");

        let mut nomes: Vec<&str> = todos.iter().map(PhxError::nome).collect();
        nomes.sort_unstable();
        nomes.dedup();
        assert_eq!(nomes.len(), quantos, "ha nome repetido");
    }

    /// Os numeros que ja foram publicados. Mudar qualquer um deles quebra
    /// todo cliente que trata o erro pelo codigo -- e quebra CALADO, porque o
    /// cliente antigo continua achando que sabe o que 3002 quer dizer.
    #[test]
    fn os_codigos_publicados_nao_mudam() {
        assert_eq!(PhxError::Corrompido(String::new()).codigo(), 1001);
        assert_eq!(PhxError::Esquema(String::new()).codigo(), 2001);
        assert_eq!(PhxError::Tipo(String::new()).codigo(), 2002);
        assert_eq!(PhxError::NaoEncontrado(String::new()).codigo(), 3001);
        assert_eq!(PhxError::Duplicado(String::new()).codigo(), 3002);
        assert_eq!(PhxError::LimiteExcedido(String::new()).codigo(), 3003);
        assert_eq!(PhxError::Conflito(String::new()).codigo(), 3004);
        assert_eq!(PhxError::Autorizacao(String::new()).codigo(), 4001);
        assert_eq!(PhxError::EmCarga(String::new()).codigo(), 4002);
        assert_eq!(PhxError::EmTransacao(String::new()).codigo(), 4005);
        assert_eq!(PhxError::TransacaoAbortada(String::new()).codigo(), 6002);
        assert_eq!(PhxError::Redireciona(String::new()).codigo(), 4003);
        assert_eq!(PhxError::SpareEmEspera(String::new()).codigo(), 4004);
        assert_eq!(PhxError::Io(std::io::Error::other("x")).codigo(), 5001);
        assert_eq!(PhxError::Cancelado(String::new()).codigo(), 6001);
    }

    /// Os dois erros de papel sao recusa DEFINITIVA deste servidor: repetir o
    /// pedido aqui nao muda nada -- o conserto e falar com o primario.
    ///
    /// `Redireciona` cobre os dois casos que nasceram separados: a escrita
    /// numa read replica e a escrita numa replica de cluster. Para quem chama,
    /// e o mesmo evento -- "va para o outro servidor" -- e um evento so tem
    /// um codigo.
    #[test]
    fn recusa_por_papel_nao_pede_nova_tentativa() {
        assert!(!PhxError::Redireciona(String::new()).adianta_repetir());
        assert!(!PhxError::SpareEmEspera(String::new()).adianta_repetir());
        assert_eq!(PhxError::Redireciona(String::new()).classe(), "acesso");
        assert_eq!(PhxError::SpareEmEspera(String::new()).classe(), "acesso");
    }

    #[test]
    fn a_classe_sai_da_faixa_do_codigo() {
        assert_eq!(PhxError::Corrompido(String::new()).classe(), "formato");
        assert_eq!(PhxError::Esquema(String::new()).classe(), "esquema");
        assert_eq!(PhxError::Duplicado(String::new()).classe(), "dado");
        assert_eq!(PhxError::Autorizacao(String::new()).classe(), "acesso");
        assert_eq!(PhxError::Io(std::io::Error::other("x")).classe(), "sistema");
        assert_eq!(PhxError::Cancelado(String::new()).classe(), "execucao");
    }

    /// Encerrar uma atividade NAO e pedir para tentar de novo: quem
    /// administra acabou de mandar parar, e um cliente que repete desfaz a
    /// decisao dele sem ninguem perceber.
    #[test]
    fn cancelado_nao_pede_nova_tentativa() {
        assert!(!PhxError::Cancelado(String::new()).adianta_repetir());
    }

    /// Repetir so adianta no que pode ter mudado sozinho. Sao dois, e o nome
    /// deste teste ja disse "so o de E/S" -- ate a tabela em carga existir.
    #[test]
    fn so_o_que_e_passageiro_pede_nova_tentativa() {
        assert!(PhxError::Io(std::io::Error::other("x")).adianta_repetir());
        assert!(!PhxError::Duplicado(String::new()).adianta_repetir());
        // Repetir um conflito e escrever por cima do outro sem olhar. Quem
        // decide e gente, e nao um laco de nova tentativa.
        assert!(!PhxError::Conflito(String::new()).adianta_repetir());
        // «Em carga» e passageiro: quem reservou vai soltar. E a diferenca
        // entre ele e «acesso negado», que nao muda por esperar.
        assert!(PhxError::EmCarga(String::new()).adianta_repetir());
        assert_eq!(PhxError::EmCarga(String::new()).classe(), "acesso");
        // «Em transacao» e passageiro pelo mesmo motivo, e por isso e da mesma
        // familia: quem segura vai soltar no `COMMIT` ou no `ROLLBACK`.
        assert!(PhxError::EmTransacao(String::new()).adianta_repetir());
        assert_eq!(PhxError::EmTransacao(String::new()).classe(), "acesso");
        // Ja a transacao abortada NAO adianta repetir: o pedido nao e o
        // problema, e insistir nele so gasta o servidor. O caminho e o
        // `ROLLBACK`.
        assert!(!PhxError::TransacaoAbortada(String::new()).adianta_repetir());
        assert_eq!(
            PhxError::TransacaoAbortada(String::new()).classe(),
            "execucao"
        );
        assert!(!PhxError::Autorizacao(String::new()).adianta_repetir());
        // Redirecionado: repetir AQUI da a mesma resposta -- o que adianta e
        // ir ao endereco que a mensagem aponta.
        assert!(!PhxError::Redireciona(String::new()).adianta_repetir());
        assert_eq!(PhxError::Redireciona(String::new()).classe(), "acesso");
    }
}
