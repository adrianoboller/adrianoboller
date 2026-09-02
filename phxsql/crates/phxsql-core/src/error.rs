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
    /// cluster, por exemplo. A mensagem comeca com `REDIRECIONA host:porta`,
    /// de proposito: e o pedaco que um cliente recorta para se apontar ao
    /// destino certo sem interpretar prosa.
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
    pub fn adianta_repetir(&self) -> bool {
        matches!(
            self,
            PhxError::Io(_) | PhxError::EmCarga(_) | PhxError::EmTransacao(_)
        )
    }
}

impl fmt::Display for PhxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PhxError::Io(e) => write!(f, "erro de E/S: {e}"),
            PhxError::BadMagic {
                arquivo,
                esperado,
                encontrado,
            } => write!(
                f,
                "assinatura invalida em {arquivo}: esperado {:?}, encontrado {:?}",
                String::from_utf8_lossy(esperado.as_slice()),
                String::from_utf8_lossy(encontrado.as_slice())
            ),
            PhxError::VersaoNaoSuportada {
                arquivo,
                encontrada,
                suportada,
            } => write!(
                f,
                "versao de formato {encontrada} nao suportada em {arquivo} (esta build le ate {suportada})"
            ),
            PhxError::Corrompido(m) => write!(f, "arquivo corrompido: {m}"),
            PhxError::Esquema(m) => write!(f, "esquema invalido: {m}"),
            PhxError::Tipo(m) => write!(f, "tipo invalido: {m}"),
            PhxError::NaoEncontrado(m) => write!(f, "nao encontrado: {m}"),
            PhxError::Duplicado(m) => write!(f, "chave duplicada: {m}"),
            PhxError::Integridade(m) => write!(f, "integridade referencial: {m}"),
            PhxError::Conflito(m) => write!(f, "conflito de escrita: {m}"),
            PhxError::Autorizacao(m) => write!(f, "acesso negado: {m}"),
            PhxError::EmCarga(m) => write!(f, "tabela em carga: {m}"),
            PhxError::EmTransacao(m) => write!(f, "tabela em transacao: {m}"),
            PhxError::LimiteExcedido(m) => write!(f, "limite excedido: {m}"),
            // Sem prefixo: a mensagem ja comeca com `REDIRECIONA host:porta`,
            // e e esse comeco que o cliente recorta.
            PhxError::Redireciona(m) => write!(f, "{m}"),
            // A MESSAGE_TEXT na frente, porque ela e a mensagem que o dono do
            // banco escreveu para quem esbarrar na regra; o SQLSTATE vem
            // atras, para o driver que trata por codigo.
            PhxError::Sinal { estado, mensagem } => {
                write!(f, "{mensagem} (SIGNAL SQLSTATE {estado})")
            }
            PhxError::SpareEmEspera(m) => write!(f, "spare em espera: {m}"),
            PhxError::TransacaoAbortada(m) => write!(f, "transacao abortada: {m}"),
            // Sem prefixo de erro na frente: quem le a resposta esta vendo o
            // resultado de um botao que ele mesmo apertou, e nao uma falha.
            PhxError::Cancelado(m) => write!(f, "{m}"),
        }
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

    /// Dois erros diferentes nao podem compartilhar codigo, senao o numero
    /// nao serve para distinguir nada.
    #[test]
    fn cada_erro_tem_o_seu_codigo() {
        let todos = [
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
            PhxError::EmCarga(String::new()),
            PhxError::EmTransacao(String::new()),
            PhxError::TransacaoAbortada(String::new()),
            PhxError::Autorizacao(String::new()),
            PhxError::Redireciona(String::new()),
            PhxError::SpareEmEspera(String::new()),
            PhxError::Io(std::io::Error::other("x")),
            PhxError::Cancelado(String::new()),
        ];
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
