//! Esquema de uma tabela PhxSql: colunas, indices e o layout do slot.
//!
//! O esquema e serializado dentro do proprio `.reg`, logo apos o cabecalho.
//! Assim uma tabela e auto-descritiva: basta o quarteto de arquivos para
//! reabrir e ler os dados, sem dicionario externo.

use crate::error::{PhxError, Result};
use crate::expressao::Expressao;
use crate::keyenc::largura_componente;
use crate::paginacao::{ModoParticao, Paginacao, BALDES};
use crate::types::{ColumnType, DadoPessoal};
use crate::uuid::Uuid;

const MAGIC_ESQUEMA: &[u8; 4] = b"PSCH";
/// Versao do bloco de esquema gravado no `.reg`.
///
/// A 3 acrescentou os metadados de coluna (`id`, `caption`, `descricao`,
/// `mascara`), o marcador de chave primaria no indice e o modo de particao.
/// A 4 acrescentou a coluna de sistema [`COLUNA_SOFTDELETED`] e o sinal de
/// motivo obrigatorio. A 5 acrescentou [`COLUNA_ROWNUM`]. A 6 acrescentou a
/// marca de dado pessoal ([`DadoPessoal`]) de cada coluna.
///
/// A leitura ainda aceita a 2: tabela gravada antes abre, ganha um `id` v7
/// sorteado na hora e os textos vazios. Escrever, so na 6.
///
/// # Por que a marca da v6 vai no FIM, e nao junto da coluna
///
/// Ela e um atributo de coluna e o lugar "natural" seria ao lado da `mascara`.
/// Nao vai la de proposito: no fim, quem le uma v5 simplesmente **para antes**
/// do bloco novo, do mesmo jeito que ja para antes do byte de motivo
/// obrigatorio da v4. No meio do laco de colunas, cada versao antiga precisaria
/// de um desvio proprio dentro do laco -- e desvio dentro de laco de
/// desserializacao e onde nasce o campo deslocado que ainda passa no CRC.
///
/// # Por que a v3 nao ganha a coluna ao ser lida
///
/// A coluna de sistema entra em [`Schema::new`], que e o caminho de CRIAR
/// tabela. A leitura do disco usa outro caminho, que nao acrescenta nada: o
/// `payload_len` sai da lista de colunas gravada, e uma coluna a mais
/// deslocaria o offset de todas as seguintes. Uma tabela v3 continua legivel
/// exatamente como esta -- so nao tem exclusao suave, e a mensagem de erro
/// diz isso em vez de ler lixo.
/// v8: os dois bytes do indice de texto.
///
/// Subiu a versao em vez de roubar bits livres do byte de sinalizadores, e o
/// motivo e de SEGURANCA e nao de estilo: um binario antigo lendo bits que nao
/// conhece abriria a tabela e ignoraria o indice de texto -- gravando linha
/// sem atualizar o `.fts`, que e corrupcao silenciosa do indice. Com a versao
/// nova ele RECUSA o arquivo (a leitura confere a faixa), e recusa alta e
/// melhor que aceite errado.
///
/// # v10: as duas colunas de carimbo, e o `passo` da sequencia
///
/// Duas coisas entram juntas, num bump so, porque duas mudancas de formato em
/// sequencia sao dois eventos de migracao onde cabe um.
///
/// A primeira NAO carrega byte de bloco: [`COLUNA_ROWSTAMP`] e
/// [`COLUNA_ROWTIME`] sao colunas como as outras e entram na lista de colunas,
/// igual ao que a v5 fez com o `rownum`. **O bump existe para RECUSAR o
/// binario velho**: o `e_coluna_de_sistema` da v9 so conhece dois nomes, entao
/// um motor v9 leria `rowstamp` como coluna comum do usuario -- ela apareceria
/// na grade e no formulario, e alguem poderia digitar por cima do carimbo. A
/// versao nos bytes 4..6 e a unica coisa que se le antes de decidir o que o
/// resto significa.
///
/// A segunda carrega bytes: o `passo` da coluna `Sequence`, numa lista propria
/// no FIM, com contagem a frente. O `inicio` da faixa **nao** vem aqui de
/// proposito -- ele sai da identidade do NO (ver `no::inicio_da_sequencia` no
/// `phxsql-store`), porque uma tabela nascida por replicacao e criada do MESMO
/// bloco de esquema do source, byte a byte: um `inicio` gravado aqui chegaria
/// igual nos dois nos e as faixas voltariam a colidir, que e exatamente o
/// defeito que a faixa existe para consertar. O `passo` pode viajar porque e
/// comum aos nos **por definicao**: ele e o denominador da faixa, e a faixa so
/// e disjunta se os dois usarem o mesmo.
///
/// Truncado no bloco da v10 e ERRO, no molde da v9 e nao no da v6/v8: um
/// `passo` que sumisse calado viraria faixa 1 e faria dois nos numerarem a
/// mesma faixa outra vez.
const VERSAO_ESQUEMA: u16 = 10;
const VERSAO_ESQUEMA_MINIMA: u16 = 2;

/// Nome da coluna de sistema que marca a linha como excluida sem excluir.
///
/// Toda tabela criada a partir da v4 tem esta coluna, no FIM da lista: no fim
/// porque assim os offsets das colunas do usuario nao mudam de lugar quando
/// ela entra, e quem monta a linha posicionalmente pode continuar mandando so
/// as colunas que declarou.
pub const COLUNA_SOFTDELETED: &str = "softdeleted";

/// Nome da coluna de sistema com o numero de ordem de chegada da linha.
///
/// # Por que ela existe, se ja ha o rowid
///
/// O `rowid` e a POSICAO FISICA. Enquanto o volume sai de divisao, posicao e
/// ordem de chegada sao a mesma coisa, e o rowid serve de cursor sozinho. Na
/// particao ALFANUMERICA nao sao: a linha vai para o volume da letra dela, e
/// duas linhas digitadas em seguida caem em arquivos diferentes, com rowids
/// que nao se comparam.
///
/// O `rownum` e o que sobra de monotonico: um contador global da tabela,
/// atribuido na insercao, que nunca reaproveita numero. A ordem de digitacao
/// nao se perde na particao alfanumerica -- ela muda de campo.
pub const COLUNA_ROWNUM: &str = "rownum";

/// Contador de ordem de criacao da linha, por NO. Nunca empata, nunca recua.
///
/// # Por que um contador, e nao o relogio
///
/// A ordem do dono de 11/09/2026 e «impossivel o filho ter a mesma data do
/// pai», e resolucao de relogio nao a cumpre: medido nesta casa, **12 eventos
/// caem num unico milissegundo**. Nanossegundos tambem nao bastariam sozinhos
/// -- dois `clock_gettime` seguidos podem devolver o mesmo valor, e um salto
/// de NTP para tras faz o relogio recuar, que e o caso que a garantia existe
/// para cobrir.
///
/// Entao a ordem mora num contador puro do processo, que so anda para a
/// frente, e o relogio de parede fica em [`COLUNA_ROWTIME`], para leitura
/// humana. E o que os tres motores maduros fazem por baixo (`xmin` no
/// PostgreSQL, `DB_TRX_ID` no InnoDB, `BIGINT UNSIGNED` na alternativa do
/// MariaDB ao `TIMESTAMP(6)`): a ordem sai de um contador, nunca do relogio.
///
/// # Por que NAO e um `u64` de nanos no mesmo campo
///
/// Medido com o `Json` desta casa: um carimbo em nanos tem magnitude 198,7x
/// acima de 2^53, a grade do `f64` naquela faixa e de **256 ns**, e duas
/// linhas gravadas a 1 ns de distancia sairiam IDENTICAS no fio. O avanco
/// forcado sobreviveria no disco e morreria no soquete. Um contador que
/// comeca em 1 so cruza 2^53 depois de 9x10^15 escritas -- 285 anos a um
/// milhao por segundo.
///
/// # O alcance da garantia
///
/// **Por NO.** Entre dois masters em modo bidirecional os carimbos vem de dois
/// contadores independentes e podem se intercalar. Nao se reivindica ordem
/// global entre nos, porque ela nao esta provada.
pub const COLUNA_ROWSTAMP: &str = "rowstamp";

/// Relogio de parede de quando a linha foi criada, em milissegundos.
///
/// Pode EMPATAR entre duas linhas, e isso nao e defeito: e o que os tres
/// motores maduros garantem de proposito dentro da mesma unidade de trabalho
/// (o PostgreSQL chama de *feature* por escrito). Quem precisa de ordem le o
/// [`COLUNA_ROWSTAMP`]; quem precisa saber que horas eram le esta.
///
/// Separar as duas e o que mantem as duas honestas: um campo unico que
/// ordenasse E dissesse a hora teria de mentir sobre a hora depois de um salto
/// de NTP para tras, reportando futuro ate o relogio alcancar o carimbo.
pub const COLUNA_ROWTIME: &str = "rowtime";

/// Este nome e de uma coluna do motor?
///
/// Existe para os lugares que precisam ESCONDER as colunas de sistema --
/// a grade, o formulario, a juncao -- nao terem cada um a sua lista. Coluna
/// de sistema nova entra aqui e some dos tres de uma vez; a lista repetida em
/// tres lugares e onde a quarta seria esquecida.
///
/// E ela estava repetida em QUATRO quando o carimbo entrou: a sincronia do
/// DbLink, a chave unica do bidirecional, o `completar` do `Table` e a carga
/// por texto perguntavam a mesma coisa a mao, com os literais crus. Os quatro
/// passam por aqui agora.
pub fn e_coluna_de_sistema(nome: &str) -> bool {
    nome == COLUNA_SOFTDELETED
        || nome == COLUNA_ROWNUM
        || nome == COLUNA_ROWSTAMP
        || nome == COLUNA_ROWTIME
}

/// O valor com que uma coluna de sistema entra numa linha que chegou sem ela.
///
/// `None` quando o nome nao e de coluna de sistema -- e quem chama usa isso
/// para saber que a linha esta curta por OUTRO motivo, e deixar a aridade
/// reclamar com a mensagem dela.
///
/// # Por que mora aqui, ao lado da lista
///
/// Porque os dois lugares que a usavam tinham um ramo de queda que MENTIA: o
/// `completar` do `Table` caia em `Value::Bool(false)` e a carga por texto em
/// `Value::Null`, os dois para qualquer nome que nao fosse um dos dois
/// conhecidos. Uma coluna de sistema nova entrava calada no ramo errado --
/// carimbo `false` numa coluna `UInt8` --, e o erro so aparecia como campo
/// trocado. Com o valor ao lado do nome, quem acrescentar a quinta coluna de
/// sistema acrescenta o valor dela no mesmo lugar ou nao compila a intencao.
///
/// Os zeros NAO sao valor final: o motor os troca antes de a linha ir ao
/// disco (`numerar_linha` e `carimbar_linha`). Zero e o «ainda nao».
pub fn valor_inicial_da_coluna_de_sistema(nome: &str) -> Option<crate::value::Value> {
    use crate::value::Value;
    match nome {
        COLUNA_SOFTDELETED => Some(Value::Bool(false)),
        COLUNA_ROWNUM | COLUNA_ROWSTAMP => Some(Value::UInt(0)),
        COLUNA_ROWTIME => Some(Value::DateTime(0)),
        _ => None,
    }
}

// ------------------------------------- o modo ledger, reconhecido no esquema
//
// O modo ledger e' do `phxsql-store` (`ledger.rs`), que monta e verifica a
// cadeia. O que mora AQUI e' so' o reconhecimento -- «este esquema e' de uma
// tabela-cadeia?» --, e ele desceu para ca por necessidade: a guarda que
// recusa ledger com dado pessoal age na DECLARACAO, e declarar e' montar um
// `Schema`, uma camada abaixo do store. O `ledger.rs` REEXPORTA estes nomes em
// vez de repeti-los: duas listas dos mesmos quatro nomes divergiriam no dia em
// que a cadeia ganhasse uma quinta peca.

/// Coluna que guarda o hash do bloco. Fica de fora do proprio hash.
pub const LEDGER_COL_HASH: &str = "hash";
/// Coluna que liga este bloco ao anterior: guarda o `hash` do bloco de baixo.
pub const LEDGER_COL_ANTERIOR: &str = "anterior";
/// Coluna da altura do bloco (tipo `Sequence`).
pub const LEDGER_COL_ALTURA: &str = "altura";
/// Coluna da assinatura. Fica de fora do hash, porque uma assinatura assina o
/// hash -- entao ela vem DEPOIS dele.
pub const LEDGER_COL_ASSINATURA: &str = "assinatura";
/// Indice unico ascendente sobre `altura`: devolve os blocos na ordem da cadeia.
pub const LEDGER_IDX_POR_ALTURA: &str = "porAltura";

/// Este esquema e' de uma tabela em MODO LEDGER?
///
/// O modo ledger nao e' um `TipoDatabase` novo nem um sinalizador gravado: e'
/// uma CONVENCAO de esquema. Uma tabela esta em modo ledger quando reune as
/// quatro pecas que a cadeia exige -- as tres colunas com os tipos certos
/// (`hash` e `anterior` Uuid256, `altura` Sequence) E o indice unico
/// `porAltura`, que devolve os blocos na ordem da cadeia. Exigir os quatro
/// JUNTOS e' o que separa uma tabela-cadeia de uma tabela comum que por acaso
/// tem uma coluna chamada `altura`.
pub fn e_tabela_ledger(esquema: &Schema) -> bool {
    let tem_coluna = |nome: &str, ty: ColumnType| {
        esquema
            .coluna_por_nome(nome)
            .is_some_and(|i| esquema.colunas()[i].ty == ty)
    };
    let tem_indice_por_altura = esquema
        .indices()
        .iter()
        .any(|idx| idx.nome == LEDGER_IDX_POR_ALTURA && idx.unico);
    tem_coluna(LEDGER_COL_HASH, ColumnType::Uuid256)
        && tem_coluna(LEDGER_COL_ANTERIOR, ColumnType::Uuid256)
        && tem_coluna(LEDGER_COL_ALTURA, ColumnType::Sequence)
        && tem_indice_por_altura
}

/// O valor desta coluna entra no hash do bloco?
///
/// Uma fonte so' para os dois lados: `ledger::conteudo_canonico` pula estas
/// mesmas colunas ao montar o que vai ao SHA-256, e a guarda de dado pessoal
/// pergunta por aqui o que o hash cobriria. Duas listas divergiriam no dia em
/// que uma coluna nova ficasse de fora do hash -- e a guarda passaria a
/// proteger uma coluna que o hash nem toca, ou a liberar uma que ele cobre.
pub fn coluna_no_hash_do_ledger(nome: &str) -> bool {
    nome != LEDGER_COL_HASH && nome != LEDGER_COL_ASSINATURA && !e_coluna_de_sistema(nome)
}

/// O que fazer com as linhas filhas quando a linha pai muda ou some.
///
/// Mesma semantica do `RELATION` do dicionario do Clarion(R) e do
/// `ON DELETE` / `ON UPDATE` do SQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AcaoRi {
    /// Nao mexe nas filhas. O banco pode ficar com referencia orfa.
    #[default]
    NaoFazerNada,
    /// Recusa a operacao enquanto existir filha.
    Restringir,
    /// Repete a operacao nas filhas.
    Cascata,
    /// Anula as colunas da filha que apontavam para o pai.
    AnularCampos,
}

impl AcaoRi {
    fn tag(self) -> u8 {
        match self {
            AcaoRi::NaoFazerNada => 0,
            AcaoRi::Restringir => 1,
            AcaoRi::Cascata => 2,
            AcaoRi::AnularCampos => 3,
        }
    }

    fn de_tag(t: u8) -> Result<AcaoRi> {
        Ok(match t {
            0 => AcaoRi::NaoFazerNada,
            1 => AcaoRi::Restringir,
            2 => AcaoRi::Cascata,
            3 => AcaoRi::AnularCampos,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "acao de integridade referencial desconhecida: {outro}"
                )))
            }
        })
    }
}

/// Chave estrangeira: liga colunas desta tabela a colunas de outra.
///
/// O FraseSQL precisa dessa informacao no catalogo para conseguir gerar JOIN;
/// e ela e tambem o `RELATION` do dicionario do Clarion(R), com CASCADE e
/// RESTRICT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKey {
    pub nome: String,
    /// Posicoes das colunas locais em [`Schema::colunas`].
    pub colunas: Vec<usize>,
    /// Tabela referenciada. Aceita nome simples ou `schema.tabela`.
    pub tabela_ref: String,
    /// Nomes das colunas na tabela referenciada, na mesma ordem.
    pub colunas_ref: Vec<String>,
    /// O que fazer com as filhas quando a linha mae e APAGADA.
    ///
    /// So aceita `Restringir` -- a recusa mora em `valores::acao_ri_de_texto`,
    /// na declaracao, e a imposicao em `Table::conferir_filhas`, na gravacao.
    pub ao_excluir: AcaoRi,
    /// O que fazer com as filhas quando as colunas referenciadas da linha mae
    /// MUDAM de valor.
    ///
    /// Ao contrario do `ao_excluir`, as quatro acoes valem aqui, e desde a
    /// SP000057 as quatro sao EXECUTADAS -- ver `Table::planejar_ao_alterar`.
    /// Antes dela o campo era guardado, serializado, mostrado pelo `cli` e
    /// lido por ninguem: a mae mudava a chave e a filha ficava apontando para
    /// um pai que nao existe mais, calada.
    pub ao_alterar: AcaoRi,
    /// Se o motor CONFERE esta chave ao gravar a linha filha -- e, desde a
    /// SP000057, se ele tambem LEVA a alteracao da mae ate as filhas.
    ///
    /// # Por que um interruptor, e por que ele nasce LIGADO
    ///
    /// Ele nasceu desligado, e o dono virou a decisao: *chave declarada nasce
    /// conferida*. A regra primordial diz «nunca se mata o pai que tem filhos»
    /// sem condicao, e uma chave que precisa ser LEMBRADA de conferir nao
    /// honra um «nunca» -- o esquecimento vira o padrao.
    ///
    /// Isto NAO quebra banco que ja existe, e o motivo e de formato: o `PSCH`
    /// v7 grava o byte por chave, entao o esquema em disco volta com o que foi
    /// gravado nele. Chave declarada antes daquela decisao continua com
    /// `false` ate alguem ligar, e continua fora da conferencia E fora da
    /// cascata -- as duas leem este mesmo campo, de proposito: sao a mesma
    /// pergunta («esta relacao ja e imposta?») em dois momentos.
    ///
    /// Quem QUER declarar sem impor continua podendo, mandando
    /// `"verificar": false`, e ai e escolha escrita em vez de omissao.
    pub verificar: bool,
}

impl ForeignKey {
    pub fn new(
        nome: impl Into<String>,
        colunas: Vec<usize>,
        tabela_ref: impl Into<String>,
        colunas_ref: Vec<String>,
    ) -> ForeignKey {
        ForeignKey {
            nome: nome.into(),
            colunas,
            tabela_ref: tabela_ref.into(),
            colunas_ref,
            ao_excluir: AcaoRi::Restringir,
            // Cascata, e nao Restringir: e o par da regra do dono -- «1 para
            // muitos, Cascade/Restrict sempre» -- e era a UNICA divergencia
            // entre as duas portas de entrada. O JSON do servidor ja entregava
            // `Cascata` quando `ao_alterar` vinha ausente (ver
            // `valores::acao_ri_de_texto`), e esta aqui entregava `Restringir`:
            // a MESMA tabela nascia com integridade referencial diferente
            // conforme quem a criasse. Duas verdades sobre o mesmo modelo e o
            // defeito que esta casa persegue, e a que estava errada era esta.
            ao_alterar: AcaoRi::Cascata,
            verificar: true,
        }
    }

    /// Liga a conferencia desta chave na gravacao. Ver [`ForeignKey::verificar`].
    pub fn conferindo(mut self, sim: bool) -> Self {
        self.verificar = sim;
        self
    }

    pub fn ao_excluir(mut self, acao: AcaoRi) -> Self {
        self.ao_excluir = acao;
        self
    }

    pub fn ao_alterar(mut self, acao: AcaoRi) -> Self {
        self.ao_alterar = acao;
        self
    }
}

/// O que uma coluna e dentro das chaves da tabela.
///
/// Tudo aqui e DERIVADO dos indices e das chaves estrangeiras -- nada disso e
/// gravado na coluna. Marcar "primaria" no proprio campo criaria uma segunda
/// verdade ao lado do indice, e as duas divergiriam no primeiro `ALTER`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PapelDeChave {
    pub primaria: bool,
    /// A chave primaria de que participa tem mais de uma coluna.
    pub primaria_composta: bool,
    pub estrangeira: bool,
    /// Alguma chave estrangeira de que participa tem mais de uma coluna.
    pub estrangeira_composta: bool,
    pub chaves_estrangeiras: Vec<String>,
    /// Todos os indices em que a coluna aparece, primario incluido.
    pub indices: Vec<String>,
}

fn pertence(idx: &IndexDef, coluna: usize) -> bool {
    idx.colunas.iter().any(|ic| ic.coluna == coluna)
}

/// Uma coluna: o que ela guarda, e o que a tela precisa saber para exibi-la.
///
/// Os quatro campos de apresentacao -- `id`, `caption`, `descricao` e
/// `mascara` -- moram no `.reg` junto com o resto do esquema, e nao num
/// dicionario a parte. E a mesma razao de o esquema morar ali: a tabela tem de
/// se descrever sozinha. Um dicionario externo se perde, se desatualiza, e
/// obriga quem copia os cinco arquivos a copiar um sexto.
///
/// O `id` e um UUID v7 sorteado na criacao e **nunca reaproveitado**: e por
/// ele que uma tela, um relatorio ou um mapeamento se referem a coluna, para
/// que renomear a coluna nao quebre nada. Renomear troca o `nome`; o `id`
/// segue o mesmo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// Identidade estavel da coluna. Sobrevive a renomear.
    pub id: Uuid,
    pub nome: String,
    /// Rotulo de tela. Vazio significa "use o nome".
    pub caption: String,
    /// Para que serve a coluna, em uma linha.
    pub descricao: String,
    /// Mascara de edicao e exibicao, no formato PICTURE do Clarion(R):
    /// `@N-11.2`, `@D6`, `@P###-####P`. Vazia = sem mascara.
    pub mascara: String,
    pub ty: ColumnType,
    pub nullable: bool,
    /// A coluna guarda dado pessoal? (LGPD / GDPR.)
    ///
    /// E declaracao, nao deducao: o motor NAO tenta adivinhar pelo nome da
    /// coluna. "cpf" e obvio, "documento" nao e, e um palpite errado num
    /// relatorio de conformidade e pior que nenhum relatorio -- porque quem
    /// le acredita.
    pub dado_pessoal: DadoPessoal,
    /// v9. O valor que a coluna ganha no `inserir` quando vem nula -- uma
    /// expressao, avaliada contra a propria linha. So no inserir: o
    /// `atualizar` recebe a linha inteira, e nulo ali e nulo.
    pub padrao: Option<Expressao>,
    /// v9. A restricao CHECK: avaliada no inserir e no atualizar com a linha
    /// inteira; `FALSE` recusa, `NULL` passa (e o SQL).
    pub check: Option<Expressao>,
    /// v9. Coluna calculada: SEMPRE recalculada na gravacao, e o valor que
    /// vier no pedido e ignorado -- a coluna nao tem dado proprio, e o
    /// protocolo nao distingue ausente de presente (o merge do UPDATE devolve
    /// o valor velho junto com a linha).
    pub calculada: Option<Expressao>,
}

impl Column {
    /// Coluna nova, com um `id` v7 recem-sorteado.
    pub fn new(nome: impl Into<String>, ty: ColumnType) -> Self {
        Column {
            id: Uuid::v7(),
            nome: nome.into(),
            caption: String::new(),
            descricao: String::new(),
            mascara: String::new(),
            ty,
            nullable: true,
            dado_pessoal: DadoPessoal::Nao,
            padrao: None,
            check: None,
            calculada: None,
        }
    }

    /// O DEFAULT, em texto de expressao. Texto vazio tira o padrao.
    pub fn com_padrao(mut self, texto: &str) -> Result<Self> {
        self.padrao = expressao_ou_nada(texto, "padrao", &self.nome)?;
        Ok(self)
    }

    /// A restricao CHECK, em texto de expressao. Texto vazio tira a restricao.
    pub fn com_check(mut self, texto: &str) -> Result<Self> {
        self.check = expressao_ou_nada(texto, "check", &self.nome)?;
        Ok(self)
    }

    /// A expressao da coluna calculada. Texto vazio a torna coluna comum.
    pub fn com_calculada(mut self, texto: &str) -> Result<Self> {
        self.calculada = expressao_ou_nada(texto, "calculada", &self.nome)?;
        Ok(self)
    }

    /// Alguma regra de escrita (padrao, check ou calculada)? E o portao que
    /// mantem o caminho quente de quem nao declarou nada custando zero.
    pub fn tem_regra(&self) -> bool {
        self.padrao.is_some() || self.check.is_some() || self.calculada.is_some()
    }

    /// Classifica a coluna para a LGPD / GDPR.
    pub fn com_dado_pessoal(mut self, grau: DadoPessoal) -> Self {
        self.dado_pessoal = grau;
        self
    }

    /// Marca a coluna como obrigatoria (NOT NULL).
    pub fn obrigatoria(mut self) -> Self {
        self.nullable = false;
        self
    }

    /// Fixa o `id` -- para reabrir uma coluna que ja existe, nao para criar.
    pub fn com_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    pub fn com_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = caption.into();
        self
    }

    pub fn com_descricao(mut self, descricao: impl Into<String>) -> Self {
        self.descricao = descricao.into();
        self
    }

    pub fn com_mascara(mut self, mascara: impl Into<String>) -> Self {
        self.mascara = mascara.into();
        self
    }

    /// O rotulo que a tela deve mostrar: o caption, ou o nome se nao houver.
    pub fn rotulo(&self) -> &str {
        if self.caption.is_empty() {
            &self.nome
        } else {
            &self.caption
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexColumn {
    /// Posicao da coluna dentro de [`Schema::colunas`].
    pub coluna: usize,
    /// Ordem decrescente.
    pub desc: bool,
    /// Comparacao sem distinguir maiusculas (fold ASCII).
    pub nocase: bool,
}

impl IndexColumn {
    pub fn asc(coluna: usize) -> Self {
        IndexColumn {
            coluna,
            desc: false,
            nocase: false,
        }
    }

    pub fn desc(coluna: usize) -> Self {
        IndexColumn {
            coluna,
            desc: true,
            nocase: false,
        }
    }

    pub fn sem_caixa(mut self) -> Self {
        self.nocase = true;
        self
    }
}

/// Um indice de TEXTO: as PALAVRAS de uma coluna, no `.fts`.
///
/// # Por que ele nao e um sinalizador dentro do [`IndexDef`]
///
/// Porque `Schema::indices()` quer dizer «as arvores B+ do `.ndx`», e uma
/// duzia de lugares assume que `esquema.indices()[i]` casa com
/// `ndx.indices()[i]`. Um indice de texto no meio daquela lista deslocaria as
/// posicoes e mandaria chave para a **arvore errada** -- e fazer cada um
/// desses lugares filtrar seria espalhar o portao por uma duzia de pontos,
/// onde o que alguem esquecesse viraria a porta dos fundos.
///
/// Lista propria, e `indices()` continua querendo dizer o que sempre quis. O
/// molde ja estava aqui: [`ForeignKey`] e uma lista propria pelo mesmo motivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndiceDeTexto {
    pub nome: String,
    /// A coluna cujas palavras entram no indice. Uma so: o indice guarda as
    /// palavras DELA, e duas colunas nao teriam significado.
    pub coluna: usize,
    /// Dobra acento? Nasce LIGADO.
    ///
    /// E o inverso de «guarda nova entra pedida», e o motivo esta medido: a
    /// busca de hoje nao dobra acento, entao um indice sem dobra acharia
    /// MENOS que a varredura -- e indice que acha menos que a varredura e
    /// pior que nao ter indice (`docs/FTS.md` §5.1).
    pub dobrar: bool,
}

impl IndiceDeTexto {
    pub fn new(nome: impl Into<String>, coluna: usize) -> Self {
        IndiceDeTexto {
            nome: nome.into(),
            coluna,
            dobrar: true,
        }
    }

    /// Desliga a dobra -- escolha escrita, em vez de omissao.
    pub fn sem_dobrar(mut self) -> Self {
        self.dobrar = false;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDef {
    pub nome: String,
    pub colunas: Vec<IndexColumn>,
    pub unico: bool,
    /// Este e o indice da CHAVE PRIMARIA da tabela.
    ///
    /// Ate aqui o motor so tinha "indice unico", e chave primaria e mais do
    /// que isso: e a identidade da linha, a que as chaves estrangeiras das
    /// outras tabelas apontam, e a que a tela precisa saber para dizer quais
    /// campos formam a chave. So um indice pode ser primario, e ele e sempre
    /// unico -- `Schema::new` recusa o contrario.
    pub primario: bool,
    /// v9. Indice PARCIAL: a linha so entra no indice quando a expressao da
    /// `TRUE`. No `atualizar`, sai se saiu do filtro e entra se entrou.
    pub onde: Option<Expressao>,
    /// v9. Uma entrada por coluna do indice: `None` e a coluna crua; `Some`
    /// e a expressao de UMA coluna (`lower(nome)`) cujo resultado, coagido
    /// para o tipo da coluna, vira a chave. Lista paralela a `colunas` de
    /// proposito: `IndexColumn` continua `Copy` e nenhum chamador muda.
    pub expressoes: Vec<Option<Expressao>>,
}

impl IndexDef {
    pub fn new(nome: impl Into<String>, colunas: Vec<IndexColumn>) -> Self {
        let n = colunas.len();
        IndexDef {
            nome: nome.into(),
            colunas,
            unico: false,
            primario: false,
            onde: None,
            expressoes: vec![None; n],
        }
    }

    /// O filtro do indice parcial, em texto de expressao.
    pub fn com_onde(mut self, texto: &str) -> Result<Self> {
        self.onde = expressao_ou_nada(texto, "onde", &self.nome)?;
        Ok(self)
    }

    /// A expressao da coluna `k` do indice.
    pub fn com_expressao(mut self, k: usize, texto: &str) -> Result<Self> {
        if k >= self.colunas.len() {
            return Err(PhxError::Esquema(format!(
                "indice {} nao tem coluna {k}",
                self.nome
            )));
        }
        self.expressoes[k] = expressao_ou_nada(texto, "expressao", &self.nome)?;
        Ok(self)
    }

    /// Alguma coluna com expressao?
    pub fn tem_expressao(&self) -> bool {
        self.expressoes.iter().any(Option::is_some)
    }

    pub fn unico(mut self) -> Self {
        self.unico = true;
        self
    }

    /// Marca como chave primaria. Primaria implica unica -- nao ha chave
    /// primaria que aceite duplicata.
    pub fn primaria(mut self) -> Self {
        self.primario = true;
        self.unico = true;
        self
    }

    /// A chave e composta quando tem mais de uma coluna.
    pub fn composta(&self) -> bool {
        self.colunas.len() > 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    nome: String,
    colunas: Vec<Column>,
    indices: Vec<IndexDef>,
    chaves_estrangeiras: Vec<ForeignKey>,
    indices_de_texto: Vec<IndiceDeTexto>,
    paginacao: Paginacao,
    offsets: Vec<usize>,
    bitmap_len: usize,
    payload_len: usize,
    /// Exigir motivo escrito para marcar uma linha como excluida.
    motivo_obrigatorio: bool,
    /// Denominador da faixa da coluna `Sequence` (v10). 1 = sem faixa.
    ///
    /// Mora no esquema, e nao na configuracao do no, porque um `.reg`
    /// restaurado noutro servidor precisa continuar sabendo de que faixa e.
    /// O `inicio` NAO mora aqui -- ver a nota em [`VERSAO_ESQUEMA`].
    passo_da_sequencia: u64,
}

/// Duas colunas da mesma tabela com o MESMO id, recusado na declaracao.
///
/// # Por que aqui e nao no `do_disco`
///
/// `do_disco` e o caminho de LER o que ja esta gravado. Uma guarda ali
/// recusaria ABRIR uma tabela que nasceu antes dela -- e o `id` de coluna e
/// gravado no `PSCH` desde a v3. Guarda nova entra pedida, nao imposta: ela
/// protege o dado que ainda vai nascer, nao o que ja esta no disco. Por isso
/// mora nos dois caminhos de DECLARAR: aqui, para a tabela nova, e em
/// `Schema::com_coluna`, para a coluna que chega depois.
///
/// O laco e O(n^2), como o da duplicata de NOME logo abaixo dele, e pelo
/// mesmo motivo: uma tabela nasce uma vez e grava um milhao de vezes.
fn conferir_ids_repetidos(nome: &str, colunas: &[Column]) -> Result<()> {
    for (i, c) in colunas.iter().enumerate() {
        if let Some(o) = colunas.iter().take(i).find(|o| o.id == c.id) {
            return Err(PhxError::Esquema(format!(
                "as colunas {} e {} da tabela {nome} tem o mesmo id ({}): \
                 identidade que se repete nao identifica -- omita o campo \"id\" \
                 numa delas para o motor sortear",
                o.nome, c.nome, c.id
            )));
        }
    }
    Ok(())
}

/// A coluna marcada como dado pessoal que o hash do bloco cobriria, se houver.
///
/// `None` quando a tabela nao e' ledger ou quando nenhuma coluna marcada entra
/// no hash -- marcar a propria `hash`, a `assinatura` ou uma coluna de sistema
/// nao abre oraculo nenhum, porque o conteudo canonico nao as inclui.
fn pessoal_coberta_pelo_hash(esquema: &Schema) -> Option<&str> {
    if !e_tabela_ledger(esquema) {
        return None;
    }
    esquema
        .colunas
        .iter()
        .find(|c| c.dado_pessoal.e_pessoal() && coluna_no_hash_do_ledger(&c.nome))
        .map(|c| c.nome.as_str())
}

/// **Modo ledger e dado pessoal nao convivem.** Decisao do dono, 18/09/2026.
///
/// O hash de cada bloco e' um SHA-256 **sem sal** do conteudo em claro, e ele
/// fica gravado na coluna `hash`, que ninguem marca e por isso ninguem cifra.
/// Quem tem a lista dos valores possiveis confirma qual esta ali por tentativa:
/// CPF sao ~10^9 candidatos, data de nascimento ~36.500, salario em centavos
/// menos ainda. Salgar nao era saida -- o leiaute do conteudo canonico esta
/// documentado justamente para se reproduzir de fora, que e' o que torna a
/// cadeia verificavel por quem nao tem o motor.
///
/// A recusa e' na DECLARACAO e nao na gravacao, como a do `ao_excluir`: uma
/// tabela nasce uma vez e grava um milhao de vezes.
///
/// # O alcance, escrito: isto NAO desfaz cadeia que ja existe
///
/// A guarda mora nos caminhos de DECLARAR, nunca no `do_disco`. Uma cadeia
/// gravada antes dela volta do disco inteira, abre, le e grava -- ali o
/// oraculo ja queimou, e recusar a abertura nao o apaga: so' tiraria do ar uma
/// tabela que esta perfeita. Guarda nova entra pedida, nao imposta.
fn conferir_ledger_sem_dado_pessoal(esquema: &Schema) -> Result<()> {
    match pessoal_coberta_pelo_hash(esquema) {
        Some(coluna) => Err(erro_ledger_com_dado_pessoal(esquema.nome(), coluna)),
        None => Ok(()),
    }
}

/// A recusa da combinacao, num lugar so': os tres caminhos de declarar erram
/// com o MESMO texto, e quem o le fica sabendo qual coluna e por que.
fn erro_ledger_com_dado_pessoal(tabela: &str, coluna: &str) -> PhxError {
    PhxError::Esquema(format!(
        "a tabela {tabela} esta em modo ledger e a coluna {coluna} esta marcada como \
         dado pessoal: o `{LEDGER_COL_HASH}` de cada bloco e um SHA-256 SEM SAL do \
         conteudo em claro, e ele fica gravado numa coluna que nao e marcada e por \
         isso nao e cifrada -- quem tem a lista dos valores possiveis (CPF, data de \
         nascimento, salario em centavos) confirma por tentativa qual deles esta ali. \
         Ou a tabela e ledger, ou a coluna e dado pessoal"
    ))
}

/// **Particao por POSICAO e dado pessoal nao convivem.** Pedido 358, aprovado
/// pelo DBA em 18/09/2026.
///
/// O rowid desta casa nao e um contador opaco: quando a particao e por
/// posicao, ele SAI do volume -- `reg.rs` atribui
/// `rowid = (balde - 1) * registros_por_arquivo + slot` --, e a conta e
/// inversivel. Dividir o rowid devolve o volume, e o volume e o primeiro
/// caractere da coluna (uma classe entre 37, exata) ou o periodo dela. Isso
/// viaja em toda resposta, no cursor `antes`/`depois` e no `.ndx`, e chega
/// inteiro a quem tem a coluna NEGADA pelo direito por coluna -- e o mesmo
/// oraculo que a conferencia de pergunta sobre coluna negada existe para
/// fechar, entrando pela porta que ela nao olha.
///
/// # As duas saidas mais obvias estao RECUSADAS, com numero
///
/// **Esconder o rowid de quem tem a coluna negada** nao compra nada: a
/// informacao volta inteira pelo `op_esquema`, que publica os registros por
/// balde e o primeiro rowid de cada volume, e pela propria ordem de varredura,
/// que e a ordem dos baldes escrita no `.pag`. E custaria o cursor
/// `antes`/`depois`, que E o rowid.
///
/// **Trocar a conta do balde** e o nao mais duro: o rowid E o endereco, e uma
/// conta nova relocalizaria cada linha e cada rowid ja gravado no `.ndx`, no
/// `.log`, na `.trash`, no `.reason`, no `.lgpd` e no evento de replicacao JA
/// ENVIADO -- os cinco ultimos append-only. Nao e migracao cara, e migracao
/// impossivel.
///
/// Sobra recusar a combinacao na DECLARACAO, como o `ao_excluir` e como o
/// ledger logo acima: a tabela nasce uma vez e grava um milhao de vezes.
///
/// # O alcance, escrito: isto NAO fecha tabela que ja existe
///
/// A guarda mora nos caminhos de DECLARAR, nunca no `do_disco` nem no
/// `com_paginacao_do_disco`. Tabela gravada antes dela volta do disco inteira,
/// abre, le e grava. E ela tambem nao mora dentro do `com_paginacao`:
/// `Schema::acrescentar_coluna` o chama de novo, e recusar ali faria o
/// `ADD COLUMN` falhar numa tabela que ja esta em producao. Guarda nova entra
/// pedida, nao imposta.
///
/// Quem chama ja sabe que `coluna` e a coluna que o rowid revela; esta funcao
/// decide o grau e as palavras. Ela recebe o grau como ALVO e nao como estado
/// porque a porta da marcacao posterior pergunta ANTES de aplicar: ali a
/// coluna ainda esta limpa, e uma guarda que olhasse o esquema de agora nao
/// veria vazamento nenhum.
fn erro_oraculo_do_rowid(
    tabela: &str,
    coluna: &str,
    grau: DadoPessoal,
    modo: ModoParticao,
) -> Option<PhxError> {
    if !grau.e_pessoal() {
        return None;
    }
    let (particao, revela) = match modo {
        // Aqui o rowid e a ordem de chegada, e ela nao sai de coluna nenhuma.
        // O `match` e exaustivo de proposito: modo de particao novo tem de
        // decidir o que o rowid dele entrega antes de compilar.
        ModoParticao::PorQuantidade => return None,
        ModoParticao::PorLetra { .. } => (
            "alfanumerica".to_string(),
            format!(
                "o PRIMEIRO CARACTERE de {coluna} -- uma classe entre {}",
                BALDES.len()
            ),
        ),
        ModoParticao::PorPeriodo { periodo, .. } => (
            format!("por periodo ({})", periodo.nome()),
            format!("em que periodo {} a data de {coluna} caiu", periodo.nome()),
        ),
    };
    Some(PhxError::Esquema(format!(
        "a particao {particao} de {tabela} e pela coluna {coluna}, e {coluna} esta \
         marcada como dado {}: o rowid de cada linha SAI do volume, entao dividir o \
         rowid devolve o volume -- e o volume revela {revela}. Isso chega de graca em \
         toda leitura, inclusive a quem tem {coluna} NEGADA pelo direito por coluna. \
         Particione por outra coluna (ou por quantidade), ou nao marque {coluna}",
        grau.nome()
    )))
}

impl Schema {
    /// Esquema de uma tabela NOVA.
    ///
    /// Acrescenta a coluna de sistema [`COLUNA_SOFTDELETED`] no fim, se quem
    /// chamou nao a declarou. Quem le esquema do disco nao passa por aqui --
    /// ver [`Schema::do_disco`].
    pub fn new(
        nome: impl Into<String>,
        mut colunas: Vec<Column>,
        indices: Vec<IndexDef>,
    ) -> Result<Schema> {
        if !colunas.iter().any(|c| c.nome == COLUNA_SOFTDELETED) {
            colunas.push(
                Column::new(COLUNA_SOFTDELETED, ColumnType::Bool)
                    .obrigatoria()
                    .com_caption("Excluido")
                    .com_descricao(
                        "Marca a linha como excluida sem apagar. \
                         O motivo fica no .reason.",
                    ),
            );
        }
        // DEPOIS da softdeleted, e nao antes: coluna de sistema nova entra
        // sempre no fim, senao uma tabela gravada na versao anterior teria os
        // offsets deslocados ao ser relida.
        //
        // `UInt8` e nao `Sequence`: uma tabela so pode ter uma coluna
        // `Sequence` -- o contador do `.reg` e unico --, e reservar essa unica
        // vaga para o motor tiraria do usuario um tipo que e dele. O `rownum`
        // tem contador proprio.
        if !colunas.iter().any(|c| c.nome == COLUNA_ROWNUM) {
            colunas.push(
                Column::new(COLUNA_ROWNUM, ColumnType::UInt8)
                    .obrigatoria()
                    .com_caption("Nº")
                    .com_descricao(
                        "Ordem de chegada da linha. O motor preenche; \
                         nunca reaproveita numero.",
                    ),
            );
        }
        // v10, e nesta ordem: o carimbo DEPOIS do `rownum`, e o relogio depois
        // do carimbo. Coluna de sistema nova entra sempre no fim -- a casa ja
        // pagou tres vezes o preco de quem filtra pela primeira.
        //
        // O rotulo vai no `caption`, que e para isso: o NOME da coluna e
        // estrutura e nao se traduz; o rotulo se traduz.
        if !colunas.iter().any(|c| c.nome == COLUNA_ROWSTAMP) {
            colunas.push(
                Column::new(COLUNA_ROWSTAMP, ColumnType::UInt8)
                    .obrigatoria()
                    .com_caption("Ordem de criacao")
                    .com_descricao(
                        "Contador de criacao deste servidor. Nunca empata e \
                         nunca recua: e por ele que se prova que o pai veio \
                         antes do filho. O motor preenche.",
                    ),
            );
        }
        if !colunas.iter().any(|c| c.nome == COLUNA_ROWTIME) {
            colunas.push(
                Column::new(COLUNA_ROWTIME, ColumnType::DateTime)
                    .obrigatoria()
                    .com_caption("Criada em")
                    .com_descricao(
                        "Relogio de parede de quando a linha foi criada. \
                         Pode empatar entre duas linhas; para ordem use \
                         a coluna de ordem de criacao.",
                    ),
            );
        }
        let nome = nome.into();
        conferir_ids_repetidos(&nome, &colunas)?;
        let esquema = Schema::do_disco(nome, colunas, indices)?;
        // Depois do `do_disco` e nao antes: a pergunta e' sobre o esquema
        // MONTADO -- as quatro pecas da cadeia e as marcas juntas --, e montar
        // e' o que o `do_disco` faz. Ali dentro a guarda nao pode entrar: o
        // `do_disco` e' tambem o caminho de LER o que ja esta gravado.
        conferir_ledger_sem_dado_pessoal(&esquema)?;
        Ok(esquema)
    }

    /// Esquema montado EXATAMENTE com as colunas dadas, sem acrescentar nada.
    ///
    /// E o caminho da leitura do disco. Acrescentar uma coluna aqui deslocaria
    /// o offset de todas as colunas seguintes e faria o motor ler o campo
    /// errado de cada linha ja gravada -- silenciosamente, porque o CRC do
    /// slot continuaria batendo: os bytes nao mudaram, so a interpretacao.
    pub fn do_disco(
        nome: impl Into<String>,
        colunas: Vec<Column>,
        indices: Vec<IndexDef>,
    ) -> Result<Schema> {
        let nome = nome.into();
        if nome.is_empty() {
            return Err(PhxError::Esquema("tabela sem nome".into()));
        }
        if colunas.is_empty() {
            return Err(PhxError::Esquema(format!("tabela {nome} sem colunas")));
        }
        if colunas.len() > u16::MAX as usize {
            return Err(PhxError::Esquema("colunas demais".into()));
        }

        for (i, c) in colunas.iter().enumerate() {
            if c.nome.is_empty() {
                return Err(PhxError::Esquema(format!("coluna {i} sem nome")));
            }
            if colunas.iter().take(i).any(|o| o.nome == c.nome) {
                return Err(PhxError::Esquema(format!("coluna duplicada: {}", c.nome)));
            }
            if let ColumnType::Str(0) = c.ty {
                return Err(PhxError::Esquema(format!("coluna {} tem Str(0)", c.nome)));
            }
            if let ColumnType::Decimal { precisao, escala } = c.ty {
                if precisao == 0 || precisao > 38 || escala > precisao {
                    return Err(PhxError::Esquema(format!(
                        "Decimal invalido em {}: precisao {precisao}, escala {escala}",
                        c.nome
                    )));
                }
            }
        }

        for (i, idx) in indices.iter().enumerate() {
            if idx.nome.is_empty() {
                return Err(PhxError::Esquema(format!("indice {i} sem nome")));
            }
            if indices.iter().take(i).any(|o| o.nome == idx.nome) {
                return Err(PhxError::Esquema(format!("indice duplicado: {}", idx.nome)));
            }
            if idx.colunas.is_empty() {
                return Err(PhxError::Esquema(format!(
                    "indice {} sem colunas",
                    idx.nome
                )));
            }
            for ic in &idx.colunas {
                let col = colunas.get(ic.coluna).ok_or_else(|| {
                    PhxError::Esquema(format!(
                        "indice {} referencia coluna inexistente {}",
                        idx.nome, ic.coluna
                    ))
                })?;
                if !col.ty.indexavel() {
                    return Err(PhxError::Esquema(format!(
                        "indice {} usa coluna {} do tipo {:?}, que nao e indexavel",
                        idx.nome, col.nome, col.ty
                    )));
                }
            }
        }

        // As expressoes de esquema se conferem AQUI, na declaracao, e nao na
        // gravacao -- a mesma decisao do `ao_excluir`: uma tabela nasce uma
        // vez e grava um milhao de vezes. Coluna inexistente numa expressao
        // e um esquema que so quebraria na primeira insercao.
        conferir_expressoes(&colunas, &indices)?;

        // A particao por periodo aponta uma coluna, e ela tem de existir e ser
        // uma data. Conferir aqui e nao na gravacao: um esquema que so quebra
        // na primeira insercao ja nasceu quebrado.
        // So uma chave primaria, e ela e unica. Duas primarias seriam duas
        // identidades para a mesma linha, e uma primaria que aceita duplicata
        // nao identifica nada -- os dois casos sao erro de esquema, nao
        // preferencia.
        let primarias: Vec<&str> = indices
            .iter()
            .filter(|i| i.primario)
            .map(|i| i.nome.as_str())
            .collect();
        if primarias.len() > 1 {
            return Err(PhxError::Esquema(format!(
                "a tabela {nome} tem {} chaves primarias ({}); pode ter no maximo uma",
                primarias.len(),
                primarias.join(", ")
            )));
        }
        if let Some(idx) = indices.iter().find(|i| i.primario && !i.unico) {
            return Err(PhxError::Esquema(format!(
                "a chave primaria {} nao esta marcada como unica",
                idx.nome
            )));
        }
        // Coluna de chave primaria nao pode ser nula: uma identidade nula nao
        // identifica.
        if let Some(idx) = indices.iter().find(|i| i.primario) {
            for ic in &idx.colunas {
                if colunas[ic.coluna].nullable {
                    return Err(PhxError::Esquema(format!(
                        "a coluna {} faz parte da chave primaria {} e aceita nulo",
                        colunas[ic.coluna].nome, idx.nome
                    )));
                }
            }
        }

        // Uma sequencia por tabela. O contador mora no cabecalho do `.reg`, e
        // e um so: duas colunas Sequence dividiriam o mesmo numerador, o que
        // ninguem espera ao escrever o esquema.
        let sequencias: Vec<&str> = colunas
            .iter()
            .filter(|c| c.ty == ColumnType::Sequence)
            .map(|c| c.nome.as_str())
            .collect();
        if sequencias.len() > 1 {
            return Err(PhxError::Esquema(format!(
                "a tabela tem {} colunas Sequence ({}), e so pode ter uma: \
                 o contador do `.reg` e unico",
                sequencias.len(),
                sequencias.join(", ")
            )));
        }

        // A coluna de sistema pode ser declarada a mao -- por quem esta
        // recriando uma tabela, por exemplo --, mas nao com outro tipo. Um
        // `softdeleted` Str seria uma coluna comum com nome reservado, e o
        // motor passaria a marcar exclusao num campo que o usuario le como
        // texto.
        if let Some(c) = colunas.iter().find(|c| c.nome == COLUNA_SOFTDELETED) {
            if c.ty != ColumnType::Bool {
                return Err(PhxError::Esquema(format!(
                    "a coluna {COLUNA_SOFTDELETED} e do motor e tem de ser Bool; \
                     esta declarada como {:?}",
                    c.ty
                )));
            }
            if c.nullable {
                return Err(PhxError::Esquema(format!(
                    "a coluna {COLUNA_SOFTDELETED} nao pode aceitar nulo: \
                     nulo seria um terceiro estado entre excluida e nao excluida"
                )));
            }
        }

        if let Some(c) = colunas.iter().find(|c| c.nome == COLUNA_ROWNUM) {
            if c.ty != ColumnType::UInt8 {
                return Err(PhxError::Esquema(format!(
                    "a coluna {COLUNA_ROWNUM} e do motor e tem de ser UInt8; \
                     esta declarada como {:?}",
                    c.ty
                )));
            }
            if c.nullable {
                return Err(PhxError::Esquema(format!(
                    "a coluna {COLUNA_ROWNUM} nao pode aceitar nulo: \
                     linha sem numero de ordem nao pagina"
                )));
            }
        }

        // v10, no mesmo molde das duas de cima: quem recria a tabela a mao
        // pode declarar as colunas do motor, mas nao com outro tipo. Um
        // `rowstamp` Str seria uma coluna comum com nome reservado, e o motor
        // passaria a carimbar ordem num campo que o usuario le como texto.
        if let Some(c) = colunas.iter().find(|c| c.nome == COLUNA_ROWSTAMP) {
            if c.ty != ColumnType::UInt8 {
                return Err(PhxError::Esquema(format!(
                    "a coluna {COLUNA_ROWSTAMP} e do motor e tem de ser UInt8; \
                     esta declarada como {:?}",
                    c.ty
                )));
            }
            if c.nullable {
                return Err(PhxError::Esquema(format!(
                    "a coluna {COLUNA_ROWSTAMP} nao pode aceitar nulo: \
                     nulo nao se compara com nulo, e e a comparacao que prova \
                     que o pai veio antes do filho"
                )));
            }
        }
        if let Some(c) = colunas.iter().find(|c| c.nome == COLUNA_ROWTIME) {
            if c.ty != ColumnType::DateTime {
                return Err(PhxError::Esquema(format!(
                    "a coluna {COLUNA_ROWTIME} e do motor e tem de ser DateTime; \
                     esta declarada como {:?}",
                    c.ty
                )));
            }
            if c.nullable {
                return Err(PhxError::Esquema(format!(
                    "a coluna {COLUNA_ROWTIME} nao pode aceitar nulo: \
                     linha sem hora de criacao nao se ordena por data"
                )));
            }
        }

        let bitmap_len = colunas.len().div_ceil(8);
        let mut offsets = Vec::with_capacity(colunas.len());
        let mut pos = bitmap_len;
        for c in &colunas {
            offsets.push(pos);
            pos += c.ty.largura();
        }

        Ok(Schema {
            nome,
            colunas,
            indices,
            chaves_estrangeiras: Vec::new(),
            indices_de_texto: Vec::new(),
            paginacao: Paginacao::DESLIGADA,
            offsets,
            bitmap_len,
            payload_len: pos,
            motivo_obrigatorio: false,
            // Ausente = 1, o mesmo padrao do byte `verificar` da v7: tabela
            // gravada antes da v10 nao tem faixa, e faixa 1 e exatamente o
            // que ela sempre teve.
            passo_da_sequencia: 1,
        })
    }

    /// Posicao da coluna de sistema `softdeleted`.
    ///
    /// `None` numa tabela gravada antes da v4 do esquema: ela nao tem a
    /// coluna, e exclusao suave nela e recusada com essa explicacao.
    pub fn coluna_softdeleted(&self) -> Option<usize> {
        self.colunas
            .iter()
            .position(|c| c.nome == COLUNA_SOFTDELETED)
    }

    /// Posicao da coluna de sistema `rownum`.
    ///
    /// `None` numa tabela gravada antes da v5 do esquema.
    pub fn coluna_rownum(&self) -> Option<usize> {
        self.colunas.iter().position(|c| c.nome == COLUNA_ROWNUM)
    }

    /// Posicao da coluna de sistema `rowstamp`.
    ///
    /// `None` numa tabela gravada antes da v10 do esquema -- e quem pede a
    /// garantia da ordem recebe essa explicacao, em vez de ler lixo.
    pub fn coluna_rowstamp(&self) -> Option<usize> {
        self.colunas.iter().position(|c| c.nome == COLUNA_ROWSTAMP)
    }

    /// Posicao da coluna de sistema `rowtime`.
    ///
    /// `None` numa tabela gravada antes da v10 do esquema.
    pub fn coluna_rowtime(&self) -> Option<usize> {
        self.colunas.iter().position(|c| c.nome == COLUNA_ROWTIME)
    }

    /// Denominador da faixa da `Sequence`. 1 = sem faixa (o padrao).
    pub fn passo_da_sequencia(&self) -> u64 {
        self.passo_da_sequencia
    }

    /// Declara a faixa da `Sequence`: este no entrega um numero a cada `passo`.
    ///
    /// # Por que recusa aqui, e nao na gravacao
    ///
    /// A mesma decisao do `ao_excluir`: uma tabela nasce uma vez e grava um
    /// milhao de vezes. Passo declarado numa tabela sem `Sequence` e um
    /// esquema que nunca faria nada -- e campo aceito e ignorado e pior que
    /// campo recusado, porque o recusado ninguem acha que funcionou.
    pub fn com_passo_da_sequencia(mut self, passo: u64) -> Result<Schema> {
        if passo == 0 {
            return Err(PhxError::Esquema(
                "o passo da sequencia nao pode ser 0: a faixa seria vazia e \
                 nenhum numero caberia nela"
                    .into(),
            ));
        }
        if passo > 1 && self.coluna_sequencia().is_none() {
            return Err(PhxError::Esquema(format!(
                "a tabela {} nao tem coluna Sequence, entao nao ha faixa para \
                 dividir entre os nos",
                self.nome
            )));
        }
        self.passo_da_sequencia = passo;
        Ok(self)
    }

    /// As colunas marcadas como dado pessoal, com a posicao e o grau.
    ///
    /// Devolve na ordem das colunas -- que e a ordem que o relatorio de
    /// auditoria mostra, porque e a mesma ordem em que a ficha aparece na
    /// tela.
    pub fn colunas_pessoais(&self) -> Vec<(usize, &Column)> {
        self.colunas
            .iter()
            .enumerate()
            .filter(|(_, c)| c.dado_pessoal.e_pessoal())
            .collect()
    }

    /// A tabela guarda dado pessoal de algum grau?
    pub fn tem_dado_pessoal(&self) -> bool {
        self.colunas.iter().any(|c| c.dado_pessoal.e_pessoal())
    }

    /// A particao `modo` revelaria pelo rowid uma coluna marcada?
    ///
    /// **A PORTA 1 do pedido 358: a criacao.** Ver [`erro_oraculo_do_rowid`]
    /// para o porque da recusa e para as duas saidas que o DBA recusou com
    /// numero.
    ///
    /// Recebe o modo por ARGUMENTO, e nao le `self.paginacao`, porque quem
    /// pergunta e quem esta lendo o `CREATE TABLE`: ali o esquema ja tem as
    /// colunas e ainda nao tem a paginacao. E e por isso que a recusa mora em
    /// quem le o pedido e nao dentro do `com_paginacao` -- aquele metodo e
    /// chamado de novo por [`Schema::acrescentar_coluna`], e recusar la faria
    /// o `ADD COLUMN` falhar numa tabela que ja esta em producao.
    pub fn conferir_oraculo_do_rowid(&self, modo: ModoParticao) -> Result<()> {
        let Some(i) = modo.coluna_que_o_rowid_revela() else {
            return Ok(());
        };
        // Coluna fora da lista nao e problema DESTA guarda: quem recusa a
        // particao que aponta coluna inexistente e o `com_paginacao`, e a
        // mensagem dele fala do que falta.
        let Some(c) = self.colunas.get(i) else {
            return Ok(());
        };
        match erro_oraculo_do_rowid(&self.nome, &c.nome, c.dado_pessoal, modo) {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Troca o grau de uma coluna pelo NOME.
    ///
    /// Pelo nome e nao pelo indice porque quem classifica e gente olhando a
    /// ficha, e o indice de uma coluna nao aparece em tela nenhuma.
    ///
    /// # O IRMAO do `Schema::new`: marcar uma coluna DEPOIS
    ///
    /// Recusar so' no nascimento deixaria a porta dos fundos aberta -- bastava
    /// criar a cadeia limpa e marcar a coluna no pedido seguinte. A guarda aqui
    /// olha a TRANSICAO, e nao o estado: o que deixa de existir e' uma coluna
    /// passar de nao-marcada a marcada numa tabela em modo ledger.
    ///
    /// Desmarcar continua podendo, sempre -- e' o unico remedio de uma cadeia
    /// que nasceu com a marca antes desta guarda, e uma guarda que olhasse o
    /// estado travaria justamente o conserto. Trocar o grau de uma coluna que
    /// JA e' marcada tambem continua podendo: refinar `pessoal` para
    /// `sensivel` nao expoe um valor a mais do que ja estava exposto.
    pub fn marcar_dado_pessoal(&mut self, coluna: &str, grau: DadoPessoal) -> Result<()> {
        let Some(i) = self.colunas.iter().position(|c| c.nome == coluna) else {
            return Err(PhxError::NaoEncontrado(format!(
                "a tabela {} nao tem a coluna {coluna:?}",
                self.nome
            )));
        };
        let estreando = grau.e_pessoal() && !self.colunas[i].dado_pessoal.e_pessoal();
        if estreando && coluna_no_hash_do_ledger(coluna) && e_tabela_ledger(self) {
            return Err(erro_ledger_com_dado_pessoal(&self.nome, coluna));
        }
        // **A PORTA 2 do pedido 358**, e ela existe mesmo: medido em
        // 18/09/2026, `RegFile::remarcar_dado_pessoal` so recusa quando a
        // tabela nasceu cifrada, entao numa tabela nascida em claro bastava
        // criar limpa e marcar a coluna do balde no pedido seguinte. Fechar so
        // a criacao seria deixar a porta dos fundos escancarada.
        //
        // Olha a TRANSICAO como a do ledger: refinar o grau e desmarcar
        // continuam podendo, porque nenhum dos dois expoe um caractere a mais
        // do que ja estava exposto -- e desmarcar e o unico remedio de quem ja
        // esta na combinacao.
        if estreando && self.paginacao.modo.coluna_que_o_rowid_revela() == Some(i) {
            if let Some(e) = erro_oraculo_do_rowid(&self.nome, coluna, grau, self.paginacao.modo) {
                return Err(e);
            }
        }
        self.colunas[i].dado_pessoal = grau;
        Ok(())
    }

    /// Exigir motivo escrito na exclusao. Escolhido ao criar a tabela.
    pub fn com_motivo_obrigatorio(mut self, exigir: bool) -> Schema {
        self.motivo_obrigatorio = exigir;
        self
    }

    pub fn motivo_obrigatorio(&self) -> bool {
        self.motivo_obrigatorio
    }

    /// Posicao da coluna `Sequence`, se a tabela tiver uma.
    pub fn coluna_sequencia(&self) -> Option<usize> {
        self.colunas
            .iter()
            .position(|c| c.ty == ColumnType::Sequence)
    }

    /// Onde uma coluna NOVA entra: logo DEPOIS da ultima coluna do usuario --
    /// que, na tabela comum, e logo antes da `softdeleted` e do `rownum`.
    ///
    /// # Por que nao no fim de tudo
    ///
    /// A `softdeleted` e o `rownum` entraram no fim para nao deslocar as
    /// colunas do usuario. A coluna que o usuario acrescenta agora e do
    /// usuario, e ela vai onde as dele estao: no fim das dele. Po-la depois do
    /// `rownum` faria a lista do usuario ter um buraco no meio -- toda tela,
    /// todo `inserir` posicional com N-2 valores e toda juncao teriam de saber
    /// que as duas do motor ficaram entre as dele.
    ///
    /// # Por que DEPOIS DA ULTIMA, e nao antes da primeira de sistema
    ///
    /// As duas regras dao o mesmo lugar na tabela comum, em que as de sistema
    /// estao no fim. Elas discordam na tabela que declarou `softdeleted` a mao
    /// no meio da lista -- o que e permitido, porque quem recria uma tabela
    /// precisa. Ali, "antes da primeira de sistema" empurraria as colunas do
    /// usuario que vem depois dela, que e exatamente o que esta regra existe
    /// para evitar.
    ///
    /// O preco, nos dois casos, e que a posicao das colunas de SISTEMA anda --
    /// e quem guarda posicao (indice, chave estrangeira, coluna de particao)
    /// tem de ser remapeado. E o que [`Schema::com_coluna`] faz, num lugar so.
    ///
    /// Numa tabela anterior a v4, que nao tem coluna de sistema nenhuma, a
    /// resposta e o fim da lista.
    pub fn posicao_de_coluna_nova(&self) -> usize {
        self.colunas
            .iter()
            .rposition(|c| !e_coluna_de_sistema(&c.nome))
            .map(|i| i + 1)
            .unwrap_or(0)
    }

    /// O mesmo esquema com uma coluna a mais, inserida em `posicao`.
    ///
    /// # O que ele remapeia, e por que num lugar so
    ///
    /// Quatro coisas guardam POSICAO de coluna, e nao nome:
    /// `IndexColumn.coluna`, `ForeignKey.colunas`, a coluna de referencia da
    /// particao e `IndiceDeTexto.coluna`. Inserir uma coluna no meio empurra
    /// todas as posicoes a partir dela, e quem ficar para tras passa a apontar
    /// a vizinha -- indice sobre o campo errado, particao pela coluna errada,
    /// e nenhum erro no caminho.
    ///
    /// A quarta entrou depois das outras tres, e a falta dela nao era um
    /// deslocamento: a lista nao era CARREGADA. Todo `ALTER TABLE ADD COLUMN`
    /// apagava a declaracao do indice de texto, o `.fts` do disco ficava orfao
    /// e a busca passava a recusar por nome inexistente. Carregar e deslocar
    /// sao a mesma linha, e e por isso que ela mora aqui.
    ///
    /// Por isso o remapeamento mora aqui e nao em quem chama: a proxima coisa
    /// que guardar posicao entra nesta funcao, e nao num QUINTO lugar que
    /// alguem vai esquecer. E entrar aqui e duas coisas, nao uma: ser
    /// carregada e ser deslocada. A quarta provou que a primeira e a que se
    /// esquece, porque a falta dela nao da erro -- da lista vazia.
    pub fn com_coluna(&self, coluna: Column, posicao: usize) -> Result<Schema> {
        if posicao > self.colunas.len() {
            return Err(PhxError::Esquema(format!(
                "posicao {posicao} fora da lista de {} colunas",
                self.colunas.len()
            )));
        }
        if self.colunas.iter().any(|c| c.nome == coluna.nome) {
            return Err(PhxError::Esquema(format!(
                "a tabela {} ja tem uma coluna chamada {}",
                self.nome, coluna.nome
            )));
        }
        // O IRMAO da conferencia de nome, e ele fica: `criar_tabela` e
        // `acrescentar_coluna` chamam o MESMO montador de coluna
        // (`valores::coluna_de_json`), entao o `id` de fora entra pelos dois
        // caminhos e so um deles desce por `Schema::new`. A conferencia olha
        // so a coluna que CHEGA contra as que ja existem -- e nao a tabela
        // inteira contra si mesma -- porque tabela gravada antes desta guarda
        // tem de continuar ganhando coluna: guarda nova entra pedida, nao
        // imposta, e o que ela protege e o esquecimento de amanha.
        if let Some(o) = self.colunas.iter().find(|c| c.id == coluna.id) {
            return Err(PhxError::Esquema(format!(
                "a coluna {} traz o mesmo id da coluna {} da tabela {} ({}): \
                 identidade que se repete nao identifica -- omita o campo \"id\" \
                 para o motor sortear um",
                coluna.nome, o.nome, self.nome, coluna.id
            )));
        }

        let desloca = |i: usize| if i >= posicao { i + 1 } else { i };

        let mut colunas = self.colunas.clone();
        colunas.insert(posicao, coluna);

        let indices = self
            .indices
            .iter()
            .map(|idx| {
                let mut novo = idx.clone();
                for ic in &mut novo.colunas {
                    ic.coluna = desloca(ic.coluna);
                }
                novo
            })
            .collect();

        let fks: Vec<ForeignKey> = self
            .chaves_estrangeiras
            .iter()
            .map(|fk| {
                let mut novo = fk.clone();
                novo.colunas = fk.colunas.iter().map(|c| desloca(*c)).collect();
                novo
            })
            .collect();

        let mut paginacao = self.paginacao;
        paginacao.modo = match paginacao.modo {
            ModoParticao::PorQuantidade => ModoParticao::PorQuantidade,
            ModoParticao::PorPeriodo { coluna, periodo } => ModoParticao::PorPeriodo {
                coluna: desloca(coluna as usize) as u16,
                periodo,
            },
            ModoParticao::PorLetra { coluna } => ModoParticao::PorLetra {
                coluna: desloca(coluna as usize) as u16,
            },
        };

        // O indice de TEXTO tambem guarda posicao, e ele entra pelo MESMO
        // caminho da particao -- carregado e deslocado, nas duas pontas da
        // mesma linha. Nao carrega-lo apagava a declaracao a cada coluna nova;
        // carrega-lo sem `desloca` trocaria o defeito por um mais silencioso,
        // e o silencio depende do tipo da coluna vizinha: se ela for `Str` ou
        // `Memo`, a conferencia de `com_indices_de_texto` nao tem do que
        // reclamar e o indice passa a indexar outra coluna dizendo o nome da
        // primeira. Medido em 18/09/2026 -- ver
        // `o_indice_de_texto_anda_com_a_coluna_que_entrou_antes_dele`.
        let textos: Vec<IndiceDeTexto> = self
            .indices_de_texto
            .iter()
            .map(|it| IndiceDeTexto {
                coluna: desloca(it.coluna),
                ..it.clone()
            })
            .collect();

        let novo = Schema::do_disco(self.nome.clone(), colunas, indices)?
            .com_chaves_estrangeiras(fks)?
            .com_indices_de_texto(textos)?
            .com_paginacao(paginacao)?;
        let novo = novo.com_motivo_obrigatorio(self.motivo_obrigatorio);
        // A faixa entra pelo MESMO caminho do indice de texto, e pelo mesmo
        // motivo medido em 18/09/2026: `do_disco` monta um esquema novo e o
        // que nao se carrega aqui volta ao padrao. Uma faixa que virasse 1 ao
        // acrescentar uma coluna poria os dois nos a numerar a mesma faixa
        // outra vez -- calado, e so aparecendo na proxima colisao.
        let novo = novo.com_passo_da_sequencia(self.passo_da_sequencia)?;
        // O TERCEIRO caminho de declarar, e o unico que ve duas tabelas: a
        // guarda do ledger confere o RESULTADO, e so' quando a origem ainda
        // era legitima. Assim a coluna marcada nao entra numa cadeia, e uma
        // tabela que ja estava na combinacao antes desta guarda continua
        // ganhando coluna -- guarda nova entra pedida, nao imposta.
        if pessoal_coberta_pelo_hash(self).is_none() {
            conferir_ledger_sem_dado_pessoal(&novo)?;
        }
        Ok(novo)
    }

    /// Acrescenta as chaves estrangeiras da tabela.
    /// Declara os indices de TEXTO. As recusas acontecem AQUI.
    ///
    /// Uma tabela nasce uma vez e grava um milhao de vezes: recusar cedo custa
    /// um erro lido enquanto se cria a tabela; recusar tarde custa um banco
    /// modelado errado, descoberto no dia da primeira busca. E a mesma decisao
    /// do `ao_excluir`.
    pub fn com_indices_de_texto(mut self, textos: Vec<IndiceDeTexto>) -> Result<Schema> {
        for (i, it) in textos.iter().enumerate() {
            if it.nome.is_empty() {
                return Err(PhxError::Esquema(format!("indice de texto {i} sem nome")));
            }
            if textos.iter().take(i).any(|o| o.nome == it.nome) {
                return Err(PhxError::Esquema(format!(
                    "indice de texto duplicado: {}",
                    it.nome
                )));
            }
            // O nome tambem nao pode colidir com o de uma arvore: quem procura
            // por nome ficaria com duas respostas para a mesma pergunta.
            if self.indices.iter().any(|o| o.nome == it.nome) {
                return Err(PhxError::Esquema(format!(
                    "o indice de texto {} tem o mesmo nome de um indice comum",
                    it.nome
                )));
            }
            let col = self.colunas.get(it.coluna).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "o indice de texto {} referencia a coluna inexistente {}",
                    it.nome, it.coluna
                ))
            })?;
            if !matches!(col.ty, ColumnType::Str(_) | ColumnType::Memo) {
                return Err(PhxError::Esquema(format!(
                    "o indice de texto {} usa a coluna {} do tipo {:?}; \
                     indice de texto vale sobre Str ou Memo",
                    it.nome, col.nome, col.ty
                )));
            }
        }
        self.indices_de_texto = textos;
        Ok(self)
    }

    pub fn com_chaves_estrangeiras(mut self, fks: Vec<ForeignKey>) -> Result<Schema> {
        for (i, fk) in fks.iter().enumerate() {
            if fk.nome.is_empty() {
                return Err(PhxError::Esquema(format!("chave estrangeira {i} sem nome")));
            }
            if fks.iter().take(i).any(|o| o.nome == fk.nome) {
                return Err(PhxError::Esquema(format!(
                    "chave estrangeira duplicada: {}",
                    fk.nome
                )));
            }
            if fk.colunas.is_empty() {
                return Err(PhxError::Esquema(format!("{} sem colunas", fk.nome)));
            }
            if fk.colunas.len() != fk.colunas_ref.len() {
                return Err(PhxError::Esquema(format!(
                    "{}: {} colunas locais para {} referenciadas",
                    fk.nome,
                    fk.colunas.len(),
                    fk.colunas_ref.len()
                )));
            }
            if fk.tabela_ref.trim().is_empty() {
                return Err(PhxError::Esquema(format!(
                    "{} nao diz qual tabela referencia",
                    fk.nome
                )));
            }
            for c in &fk.colunas {
                if *c >= self.colunas.len() {
                    return Err(PhxError::Esquema(format!(
                        "{} referencia coluna inexistente {c}",
                        fk.nome
                    )));
                }
            }
        }
        self.chaves_estrangeiras = fks;
        Ok(self)
    }

    /// Liga a paginacao da tabela (os numeros do `CREATE TABLE`).
    /// Fixa a paginacao, conferindo o que ela promete sobre as colunas.
    ///
    /// A particao por periodo aponta uma coluna, e ela tem de existir e ser uma
    /// data. Conferir aqui, e nao na gravacao: um esquema que so quebra na
    /// primeira insercao ja nasceu quebrado, e o erro apareceria longe de quem
    /// o causou.
    ///
    /// # O oraculo do rowid (pedido 358) NAO se confere aqui dentro, de proposito
    ///
    /// Decisao do dono, 18/09/2026: da terceira porta do pedido 358 -- API
    /// Rust e FFI montando o esquema campo a campo, sem passar pelo
    /// `esquema_de_json` que tem a chamada a `conferir_oraculo_do_rowid` -- o
    /// FFI (`crates/phxsql-ffi/`) fica FECHADO e esta API Rust fica ABERTA.
    ///
    /// O FFI e' porta externa de verdade -- Android, iOS, IoT --, e por ela o
    /// oraculo voltaria inteiro sem quem chama de fora ter como saber que
    /// devia se proteger; medido em 18/09/2026, ele fecha por AUSENCIA de
    /// superficie, e nao por guarda nova: nenhuma das `phx_esquema_*` aceita
    /// grau de dado pessoal, e `phx_tabela_criar` nao recebe paginacao
    /// nenhuma, entao a combinacao simplesmente nao tem por onde entrar pela
    /// ABI de C hoje (ver o comentario em `phx_tabela_criar`, que aponta onde
    /// ligar esta guarda no dia em que a ABI ganhar uma das duas metades).
    ///
    /// Esta API `com_paginacao`, por outro lado, e' o NOSSO proprio codigo --
    /// quem chama e' sempre outro Rust desta casa --, e fica aberta de
    /// proposito: e' o unico jeito de montar, num teste, a combinacao antiga
    /// (`Schema::new(...).com_paginacao(...)` sem passar pela guarda) e provar
    /// que uma tabela que JA existe com ela continua abrindo, lendo, gravando
    /// e ganhando coluna -- o comportamento VELHO que "guarda nova entra
    /// pedida, nao imposta" promete. Quem monta o `CREATE TABLE` de fora
    /// (`valores::esquema_de_json`) e' quem chama `conferir_oraculo_do_rowid`
    /// ANTES desta funcao; ela mesma nunca teve essa responsabilidade.
    pub fn com_paginacao(mut self, paginacao: Paginacao) -> Result<Schema> {
        if let ModoParticao::PorPeriodo { coluna, periodo } = paginacao.modo {
            let c = self.colunas.get(coluna as usize).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "particao {} aponta a coluna {coluna}, que nao existe em {}",
                    periodo.nome(),
                    self.nome
                ))
            })?;
            if !matches!(c.ty, ColumnType::Date | ColumnType::DateTime) {
                return Err(PhxError::Esquema(format!(
                    "particao {} pede uma coluna de data; {} e {:?}",
                    periodo.nome(),
                    c.nome,
                    c.ty
                )));
            }
            if c.nullable {
                return Err(PhxError::Esquema(format!(
                    "a coluna de particao {} aceita nulo; sem data nao ha periodo \
                     em que a linha caiba",
                    c.nome
                )));
            }
        }
        if let ModoParticao::PorLetra { coluna } = paginacao.modo {
            let c = self.colunas.get(coluna as usize).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "a particao alfanumerica aponta a coluna {coluna}, \
                     que nao existe em {}",
                    self.nome
                ))
            })?;
            // Coluna externa nao serve: o valor dela nao esta no slot, e
            // decidir o arquivo de destino exigiria ler o `.memo` antes de
            // saber em que arquivo gravar -- que e a ordem invertida.
            if c.ty.externo() {
                return Err(PhxError::Esquema(format!(
                    "a particao alfanumerica nao pode apontar {}, que e {:?}: \
                     o valor mora fora do slot, e o balde precisa ser decidido \
                     ANTES de a linha ser gravada",
                    c.nome, c.ty
                )));
            }
            if c.nullable {
                return Err(PhxError::Esquema(format!(
                    "a coluna de particao {} aceita nulo; a linha sem valor \
                     cairia toda no balde Outros sem ninguem ter escolhido isso",
                    c.nome
                )));
            }
            if paginacao.max_arquivos as usize != BALDES.len() {
                return Err(PhxError::Esquema(format!(
                    "a particao alfanumerica tem exatamente {} volumes \
                     (A-Z, 0-9 e Outros); o esquema pede {}",
                    BALDES.len(),
                    paginacao.max_arquivos
                )));
            }
        }
        self.paginacao = paginacao;
        Ok(self)
    }

    /// Fixa a paginacao sem conferir -- so para reabrir o que ja esta no disco.
    ///
    /// O que foi gravado ja passou pela conferencia uma vez, e recusar na
    /// leitura transformaria um esquema antigo em tabela ilegivel.
    pub(crate) fn com_paginacao_do_disco(mut self, paginacao: Paginacao) -> Schema {
        self.paginacao = paginacao;
        self
    }

    pub fn chaves_estrangeiras(&self) -> &[ForeignKey] {
        &self.chaves_estrangeiras
    }

    /// Os indices de TEXTO, que moram no `.fts` e nao no `.ndx`.
    pub fn indices_de_texto(&self) -> &[IndiceDeTexto] {
        &self.indices_de_texto
    }

    pub fn paginacao(&self) -> Paginacao {
        self.paginacao
    }

    /// Troca o nome da tabela sem mexer em mais nada.
    ///
    /// Existe para a criacao separar `filial.clientes` em schema e tabela: o
    /// esquema chega com o nome qualificado e o que vai para o disco e so a
    /// parte da tabela -- o schema ja e o diretorio.
    pub fn renomear(&mut self, nome: &str) {
        self.nome = nome.to_string();
    }

    pub fn nome(&self) -> &str {
        &self.nome
    }

    pub fn colunas(&self) -> &[Column] {
        &self.colunas
    }

    /// O indice marcado como chave primaria, se houver.
    pub fn chave_primaria(&self) -> Option<&IndexDef> {
        self.indices.iter().find(|i| i.primario)
    }

    /// O papel de uma coluna nas chaves da tabela.
    ///
    /// Nao e campo gravado: sai dos indices e das chaves estrangeiras, que sao
    /// a verdade. Guardar "e primaria" na coluna criaria uma segunda verdade
    /// que pode discordar da primeira -- e um dia discordaria.
    pub fn papel_da_coluna(&self, i: usize) -> PapelDeChave {
        let na_pk = self.chave_primaria().filter(|k| pertence(k, i));
        let fks: Vec<&ForeignKey> = self
            .chaves_estrangeiras
            .iter()
            .filter(|fk| fk.colunas.contains(&i))
            .collect();
        PapelDeChave {
            primaria: na_pk.is_some(),
            // Composta se a chave de que ela participa tem mais de uma coluna.
            primaria_composta: na_pk.map(IndexDef::composta).unwrap_or(false),
            estrangeira: !fks.is_empty(),
            estrangeira_composta: fks.iter().any(|fk| fk.colunas.len() > 1),
            chaves_estrangeiras: fks.iter().map(|fk| fk.nome.clone()).collect(),
            indices: self
                .indices
                .iter()
                .filter(|idx| pertence(idx, i))
                .map(|idx| idx.nome.clone())
                .collect(),
        }
    }

    pub fn indices(&self) -> &[IndexDef] {
        &self.indices
    }

    /// Bytes do bitmap de nulos no inicio do payload.
    pub fn bitmap_len(&self) -> usize {
        self.bitmap_len
    }

    /// Bytes totais do payload (bitmap + todas as colunas).
    pub fn payload_len(&self) -> usize {
        self.payload_len
    }

    /// Deslocamento da coluna dentro do payload.
    pub fn offset_coluna(&self, i: usize) -> Result<usize> {
        self.offsets
            .get(i)
            .copied()
            .ok_or_else(|| PhxError::Esquema(format!("coluna {i} inexistente")))
    }

    pub fn coluna_por_nome(&self, nome: &str) -> Option<usize> {
        self.colunas.iter().position(|c| c.nome == nome)
    }

    /// Posicao da coluna sem distinguir caixa -- e como uma expressao a
    /// nomeia.
    pub fn posicao_sem_caixa(&self, nome: &str) -> Option<usize> {
        posicao_sem_caixa(&self.colunas, nome)
    }

    /// Alguma coluna com padrao, check ou calculada? Portao do caminho de
    /// escrita: tabela sem regra nao paga a chamada.
    pub fn tem_regras(&self) -> bool {
        self.colunas.iter().any(Column::tem_regra)
    }

    /// Algum indice parcial ou por expressao?
    pub fn tem_indice_condicional(&self) -> bool {
        self.indices
            .iter()
            .any(|i| i.onde.is_some() || i.tem_expressao())
    }

    pub fn indice_por_nome(&self, nome: &str) -> Option<usize> {
        self.indices.iter().position(|i| i.nome == nome)
    }

    /// Bytes de uma chave do indice, sem contar o rowid de desempate.
    pub fn largura_chave(&self, indice: usize) -> Result<usize> {
        let idx = self
            .indices
            .get(indice)
            .ok_or_else(|| PhxError::Esquema(format!("indice {indice} inexistente")))?;
        let mut total = 0;
        for ic in &idx.colunas {
            total += largura_componente(&self.colunas[ic.coluna].ty)?;
        }
        Ok(total)
    }

    pub fn serializar(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(256);
        out.extend_from_slice(MAGIC_ESQUEMA);
        out.extend_from_slice(&VERSAO_ESQUEMA.to_le_bytes());
        escrever_texto(&mut out, &self.nome);

        out.extend_from_slice(&(self.colunas.len() as u16).to_le_bytes());
        for c in &self.colunas {
            escrever_texto(&mut out, &c.nome);
            let (a, b) = c.ty.params();
            out.push(c.ty.tag());
            out.extend_from_slice(&a.to_le_bytes());
            out.push(b);
            out.push(c.nullable as u8);
            // v3: os metadados de apresentacao, na mesma ordem em que a tela
            // pede por eles.
            out.extend_from_slice(c.id.bytes());
            escrever_texto(&mut out, &c.caption);
            escrever_texto(&mut out, &c.descricao);
            escrever_texto(&mut out, &c.mascara);
        }

        out.extend_from_slice(&(self.indices.len() as u16).to_le_bytes());
        for idx in &self.indices {
            escrever_texto(&mut out, &idx.nome);
            // Dois sinalizadores num byte: unico no bit 0, primario no 1.
            out.push((idx.unico as u8) | ((idx.primario as u8) << 1));
            out.extend_from_slice(&(idx.colunas.len() as u16).to_le_bytes());
            for ic in &idx.colunas {
                out.extend_from_slice(&(ic.coluna as u16).to_le_bytes());
                out.push((ic.desc as u8) | ((ic.nocase as u8) << 1));
            }
        }

        out.extend_from_slice(&(self.chaves_estrangeiras.len() as u16).to_le_bytes());
        for fk in &self.chaves_estrangeiras {
            escrever_texto(&mut out, &fk.nome);
            escrever_texto(&mut out, &fk.tabela_ref);
            out.push(fk.ao_excluir.tag());
            out.push(fk.ao_alterar.tag());
            // v7. Um byte, e nao um bit roubado da tag da acao: bit escondido
            // dentro de outro campo e o que faz `de_tag` recusar um arquivo
            // valido no dia em que alguem acrescentar uma quinta acao.
            out.push(fk.verificar as u8);
            out.extend_from_slice(&(fk.colunas.len() as u16).to_le_bytes());
            for c in &fk.colunas {
                out.extend_from_slice(&(*c as u16).to_le_bytes());
            }
            for c in &fk.colunas_ref {
                escrever_texto(&mut out, c);
            }
        }

        let p = self.paginacao;
        out.extend_from_slice(&p.registros_por_arquivo.to_le_bytes());
        out.extend_from_slice(&p.max_arquivos.to_le_bytes());
        out.push(p.digitos);
        out.extend_from_slice(&p.bytes_por_arquivo.to_le_bytes());
        let (tag, coluna) = p.modo.tag();
        out.push(tag);
        out.extend_from_slice(&coluna.to_le_bytes());
        // v4: exigir motivo escrito na exclusao. Vem no fim porque quem le uma
        // v3 simplesmente para antes daqui.
        out.push(self.motivo_obrigatorio as u8);
        // v6: a marca de dado pessoal, uma por coluna, na ordem das colunas.
        // Mesmo motivo: quem le uma v5 para antes daqui.
        for c in &self.colunas {
            out.push(c.dado_pessoal.tag());
        }
        // v8: os indices de TEXTO, numa lista PROPRIA.
        //
        // **Lista propria, e nao um sinalizador dentro de `indices`** -- e a
        // primeira versao desta mudanca fez o contrario, e o teste derrubou:
        // uma duzia de lugares assume que `esquema.indices()[i]` casa com
        // `ndx.indices()[i]`, e um indice de texto no meio da lista deslocaria
        // as posicoes e mandaria chave para a ARVORE ERRADA. Fazer cada um
        // desses lugares filtrar seria espalhar o portao por uma duzia de
        // pontos, e o que alguem esquecesse viraria a porta dos fundos.
        //
        // Assim, `indices()` continua querendo dizer exatamente o que sempre
        // quis, e ZERO chamadores mudam. E o molde ja estava aqui: as chaves
        // estrangeiras sao uma lista propria pelo mesmo motivo.
        out.extend_from_slice(&(self.indices_de_texto.len() as u16).to_le_bytes());
        for it in &self.indices_de_texto {
            escrever_texto(&mut out, &it.nome);
            out.extend_from_slice(&(it.coluna as u16).to_le_bytes());
            out.push(it.dobrar as u8);
        }
        // v9: as expressoes de esquema -- padrao, check e calculada por
        // coluna, e o filtro e as expressoes por indice --, em TEXTO, na ordem
        // das colunas e dos indices. Texto vazio e «nao tem». Quem le uma v8
        // para antes daqui e fica sem regra nenhuma, que e o que aquela
        // tabela tinha.
        for c in &self.colunas {
            escrever_texto(&mut out, c.padrao.as_ref().map_or("", Expressao::texto));
            escrever_texto(&mut out, c.check.as_ref().map_or("", Expressao::texto));
            escrever_texto(&mut out, c.calculada.as_ref().map_or("", Expressao::texto));
        }
        for idx in &self.indices {
            escrever_texto(&mut out, idx.onde.as_ref().map_or("", Expressao::texto));
            for e in &idx.expressoes {
                escrever_texto(&mut out, e.as_ref().map_or("", Expressao::texto));
            }
        }
        // v10: a faixa da `Sequence`, numa lista PROPRIA com contagem.
        //
        // Lista com contagem, e nao um `u64` solto, mesmo so havendo uma
        // `Sequence` por tabela hoje: lista com contagem detecta truncamento,
        // campo solto nao. E lista no FIM, nao campo dentro do laco de
        // colunas: campo no meio do laco obriga cada versao antiga a um desvio
        // dentro da desserializacao, que e onde nasce o campo deslocado que
        // ainda passa no CRC (o erro que a primeira v8 cometeu).
        //
        // So o `passo`. O `inicio` nao vem aqui -- ver a nota em
        // `VERSAO_ESQUEMA`.
        match self.coluna_sequencia() {
            Some(i) => {
                out.extend_from_slice(&1u16.to_le_bytes());
                out.extend_from_slice(&(i as u16).to_le_bytes());
                out.extend_from_slice(&self.passo_da_sequencia.to_le_bytes());
            }
            None => out.extend_from_slice(&0u16.to_le_bytes()),
        }
        out
    }

    pub fn desserializar(buf: &[u8]) -> Result<Schema> {
        let mut leitor = Leitor { buf, pos: 0 };
        let magic = leitor.bytes(4)?;
        if magic != MAGIC_ESQUEMA {
            return Err(PhxError::Esquema("bloco de esquema invalido".into()));
        }
        let versao = leitor.u16()?;
        if !(VERSAO_ESQUEMA_MINIMA..=VERSAO_ESQUEMA).contains(&versao) {
            return Err(PhxError::Esquema(format!(
                "versao de esquema {versao} nao suportada \
                 (este motor le da {VERSAO_ESQUEMA_MINIMA} a {VERSAO_ESQUEMA})"
            )));
        }
        let nome = leitor.texto()?;

        let n_col = leitor.u16()? as usize;
        let mut colunas = Vec::with_capacity(n_col);
        for _ in 0..n_col {
            let nome = leitor.texto()?;
            let tag = leitor.u8()?;
            let a = leitor.u16()?;
            let b = leitor.u8()?;
            let nullable = leitor.u8()? != 0;
            // Tabela gravada na v2 nao tem metadados: ganha um id novo e
            // textos vazios, e passa a ter os campos assim que for regravada.
            let (id, caption, descricao, mascara) = if versao >= 3 {
                (
                    Uuid::de_bytes(
                        leitor
                            .bytes(16)?
                            .try_into()
                            .map_err(|_| PhxError::Esquema("id de coluna truncado".into()))?,
                    ),
                    leitor.texto()?,
                    leitor.texto()?,
                    leitor.texto()?,
                )
            } else {
                (Uuid::v7(), String::new(), String::new(), String::new())
            };
            colunas.push(Column {
                id,
                nome,
                caption,
                descricao,
                mascara,
                ty: ColumnType::de_tag(tag, a, b)?,
                nullable,
                // A marca da v6 vem no fim do bloco, e nao aqui. Ver a nota
                // em `VERSAO_ESQUEMA`.
                dado_pessoal: DadoPessoal::Nao,
                // As expressoes da v9 tambem vem no fim.
                padrao: None,
                check: None,
                calculada: None,
            });
        }

        let n_idx = leitor.u16()? as usize;
        let mut indices = Vec::with_capacity(n_idx);
        for _ in 0..n_idx {
            let nome = leitor.texto()?;
            let sinais = leitor.u8()?;
            let (unico, primario) = (sinais & 1 != 0, sinais & 2 != 0);
            let n = leitor.u16()? as usize;
            let mut cols = Vec::with_capacity(n);
            for _ in 0..n {
                let coluna = leitor.u16()? as usize;
                let flags = leitor.u8()?;
                cols.push(IndexColumn {
                    coluna,
                    desc: flags & 1 != 0,
                    nocase: flags & 2 != 0,
                });
            }
            let n_cols = cols.len();
            indices.push(IndexDef {
                nome,
                colunas: cols,
                unico,
                primario,
                onde: None,
                expressoes: vec![None; n_cols],
            });
        }

        let n_fk = leitor.u16()? as usize;
        let mut fks = Vec::with_capacity(n_fk);
        for _ in 0..n_fk {
            let nome_fk = leitor.texto()?;
            let tabela_ref = leitor.texto()?;
            let ao_excluir = AcaoRi::de_tag(leitor.u8()?)?;
            let ao_alterar = AcaoRi::de_tag(leitor.u8()?)?;
            // Esquema gravado antes da v7 nao tem o byte, e le como DESLIGADO
            // -- que e exatamente o comportamento que aquele arquivo tinha.
            let verificar = versao >= 7 && leitor.u8()? != 0;
            let n = leitor.u16()? as usize;
            let mut cols = Vec::with_capacity(n);
            for _ in 0..n {
                cols.push(leitor.u16()? as usize);
            }
            let mut cols_ref = Vec::with_capacity(n);
            for _ in 0..n {
                cols_ref.push(leitor.texto()?);
            }
            fks.push(ForeignKey {
                nome: nome_fk,
                colunas: cols,
                tabela_ref,
                colunas_ref: cols_ref,
                ao_excluir,
                ao_alterar,
                verificar,
            });
        }

        let mut paginacao = Paginacao {
            registros_por_arquivo: leitor.u64()?,
            max_arquivos: leitor.u32()?,
            digitos: leitor.u8()?,
            bytes_por_arquivo: leitor.u64()?,
            modo: ModoParticao::PorQuantidade,
        };
        if versao >= 3 {
            paginacao.modo = ModoParticao::de_tag(leitor.u8()?, leitor.u16()?)?;
        }
        let motivo_obrigatorio = versao >= 4 && leitor.u8()? != 0;

        // v6: uma marca por coluna, na ordem das colunas. Um arquivo v6
        // truncado no meio deste bloco para de ler e deixa o resto em `Nao`:
        // classificacao perdida vira "nao classificado", que e o padrao e o
        // estado seguro -- e nunca a marca da coluna errada.
        if versao >= 6 {
            for c in colunas.iter_mut() {
                match leitor.u8() {
                    Ok(tag) => c.dado_pessoal = DadoPessoal::de_tag(tag),
                    Err(_) => break,
                }
            }
        }

        // v8: a lista dos indices de texto. Quem le uma v7 para antes daqui,
        // e a lista fica vazia -- que e o que uma tabela gravada antes tem.
        let mut textos = Vec::new();
        if versao >= 8 {
            if let Ok(n) = leitor.u16() {
                for _ in 0..n {
                    let (nome, coluna, dobrar) = match (leitor.texto(), leitor.u16(), leitor.u8()) {
                        (Ok(n), Ok(c), Ok(d)) => (n, c as usize, d != 0),
                        _ => break,
                    };
                    textos.push(IndiceDeTexto {
                        nome,
                        coluna,
                        dobrar,
                    });
                }
            }
        }

        // v9: as expressoes de esquema. Ao contrario da v6 e da v8, um bloco
        // TRUNCADO aqui e erro, e nao «fica sem»: uma restricao CHECK que
        // sumisse calada seria uma garantia perdida sem ninguem saber -- e
        // arquivo que se diz v9 tem o bloco inteiro ou esta corrompido.
        if versao >= 9 {
            let truncado = || PhxError::Esquema("bloco de expressoes (v9) truncado".into());
            for c in colunas.iter_mut() {
                let (p, k, g) = (
                    leitor.texto().map_err(|_| truncado())?,
                    leitor.texto().map_err(|_| truncado())?,
                    leitor.texto().map_err(|_| truncado())?,
                );
                c.padrao = expressao_ou_nada(&p, "padrao", &c.nome)?;
                c.check = expressao_ou_nada(&k, "check", &c.nome)?;
                c.calculada = expressao_ou_nada(&g, "calculada", &c.nome)?;
            }
            for idx in indices.iter_mut() {
                let o = leitor.texto().map_err(|_| truncado())?;
                idx.onde = expressao_ou_nada(&o, "onde", &idx.nome)?;
                for k in 0..idx.colunas.len() {
                    let e = leitor.texto().map_err(|_| truncado())?;
                    idx.expressoes[k] = expressao_ou_nada(&e, "expressao", &idx.nome)?;
                }
            }
        }

        // v10: a faixa da `Sequence`. Truncado aqui e ERRO, e nao «fica sem»
        // como na v6 e na v8: um `passo` que sumisse calado viraria faixa 1, e
        // dois nos voltariam a numerar a mesma faixa -- a colisao que a faixa
        // existe para impedir, agora produzida pelo conserto e em silencio.
        let mut passo = 1u64;
        if versao >= 10 {
            let truncado =
                || PhxError::Esquema("bloco da faixa da sequencia (v10) truncado".into());
            let n = leitor.u16().map_err(|_| truncado())?;
            for _ in 0..n {
                let _coluna = leitor.u16().map_err(|_| truncado())?;
                passo = leitor.u64().map_err(|_| truncado())?;
                if passo == 0 {
                    return Err(PhxError::Esquema(
                        "o bloco da faixa (v10) traz passo 0, que e faixa vazia".into(),
                    ));
                }
            }
        }

        // `do_disco`, e nao `new`: a lista de colunas gravada e a verdade
        // inteira. Ver a nota em `VERSAO_ESQUEMA`.
        Schema::do_disco(nome, colunas, indices)?
            .com_indices_de_texto(textos)?
            .com_chaves_estrangeiras(fks)
            .map(|e| e.com_paginacao_do_disco(paginacao))
            .map(|e| e.com_motivo_obrigatorio(motivo_obrigatorio))
            // Direto no campo, e nao pelo `com_passo_da_sequencia`: o
            // construtor e o caminho de DECLARAR e recusa passo em tabela sem
            // `Sequence`; aqui e o caminho de LER o que ja esta gravado, e
            // recusar abrir por causa de um byte que so existe quando ha
            // `Sequence` seria a guarda nova batendo no dado antigo.
            .map(|mut e| {
                e.passo_da_sequencia = passo;
                e
            })
    }
}

/// Toda expressao do esquema so pode falar de coluna que existe -- e a
/// calculada nao pode falar de outra calculada, porque a ordem de calculo
/// viraria uma pergunta que ninguem respondeu. A expressao de indice e de
/// UMA coluna, e tem de ser a coluna daquela posicao do indice: a chave sai
/// codificada com o tipo dela.
fn conferir_expressoes(colunas: &[Column], indices: &[IndexDef]) -> Result<()> {
    let existe = |e: &Expressao, onde: String| -> Result<()> {
        for nome in e.colunas() {
            if posicao_sem_caixa(colunas, nome).is_none() {
                return Err(PhxError::Esquema(format!(
                    "{onde} usa a coluna {nome:?}, que a tabela nao tem"
                )));
            }
        }
        Ok(())
    };
    for c in colunas {
        if let Some(e) = &c.padrao {
            // O DEFAULT e uma EXPRESSAO (MANUAL.txt), entao um texto solto
            // sem aspas (`"padrao":"ativo"`) e lido como NOME DE COLUNA, e
            // a mensagem generica de `existe` (irma do check e do filtro de
            // indice, dois passos abaixo, que ficam como estao) nao diz
            // isso -- pedido 475. A dica entra so aqui, sobre o erro ja
            // formado, sem mudar o texto do erro em si.
            existe(e, format!("o padrao de {}", c.nome)).map_err(|erro| match erro {
                PhxError::Esquema(msg) => {
                    let ausente = e
                        .colunas()
                        .iter()
                        .find(|nome| posicao_sem_caixa(colunas, nome).is_none())
                        .map(String::as_str)
                        .unwrap_or_default();
                    PhxError::Esquema(format!("{msg}; texto vai entre aspas simples: '{ausente}'"))
                }
                outro => outro,
            })?;
        }
        if let Some(e) = &c.check {
            existe(e, format!("o check de {}", c.nome))?;
        }
        if let Some(e) = &c.calculada {
            existe(e, format!("a expressao da coluna calculada {}", c.nome))?;
            if matches!(c.ty, ColumnType::Bin) {
                return Err(PhxError::Esquema(format!(
                    "a coluna calculada {} e Bin, e expressao nao produz binario",
                    c.nome
                )));
            }
            for nome in e.colunas() {
                let i = posicao_sem_caixa(colunas, nome).unwrap_or(usize::MAX);
                if colunas.get(i).is_some_and(|o| o.calculada.is_some()) {
                    return Err(PhxError::Esquema(format!(
                        "a coluna calculada {} usa {nome:?}, que tambem e calculada: \
                         calculada nao se apoia em calculada",
                        c.nome
                    )));
                }
            }
        }
    }
    for idx in indices {
        if let Some(e) = &idx.onde {
            existe(e, format!("o filtro do indice {}", idx.nome))?;
        }
        if idx.expressoes.len() != idx.colunas.len() {
            return Err(PhxError::Esquema(format!(
                "indice {}: {} expressoes para {} colunas",
                idx.nome,
                idx.expressoes.len(),
                idx.colunas.len()
            )));
        }
        for (k, e) in idx.expressoes.iter().enumerate() {
            let Some(e) = e else { continue };
            existe(e, format!("a expressao do indice {}", idx.nome))?;
            let usadas: Vec<usize> = e
                .colunas()
                .iter()
                .filter_map(|n| posicao_sem_caixa(colunas, n))
                .collect();
            if usadas.len() != 1 || usadas[0] != idx.colunas[k].coluna {
                return Err(PhxError::Esquema(format!(
                    "a expressao {:?} do indice {} tem de usar exatamente a coluna {} \
                     e nenhuma outra: a chave sai com o tipo dela",
                    e.texto(),
                    idx.nome,
                    colunas
                        .get(idx.colunas[k].coluna)
                        .map_or("?", |c| c.nome.as_str())
                )));
            }
        }
    }
    Ok(())
}

/// Texto vazio quer dizer «sem expressao»; qualquer outro tem de analisar.
fn expressao_ou_nada(texto: &str, campo: &str, coluna: &str) -> Result<Option<Expressao>> {
    if texto.trim().is_empty() {
        return Ok(None);
    }
    Expressao::analisar(texto)
        .map(Some)
        .map_err(|e| PhxError::Esquema(format!("{campo} de {coluna}: {e}")))
}

/// Posicao da coluna pelo nome, sem distinguir caixa -- e como a expressao
/// se refere a ela.
fn posicao_sem_caixa(colunas: &[Column], nome: &str) -> Option<usize> {
    colunas
        .iter()
        .position(|c| c.nome.eq_ignore_ascii_case(nome))
}

fn escrever_texto(out: &mut Vec<u8>, s: &str) {
    let b = s.as_bytes();
    out.extend_from_slice(&(b.len() as u16).to_le_bytes());
    out.extend_from_slice(b);
}

struct Leitor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Leitor<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.pos + n > self.buf.len() {
            return Err(PhxError::Esquema("bloco de esquema truncado".into()));
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }

    fn texto(&mut self) -> Result<String> {
        let n = self.u16()? as usize;
        let b = self.bytes(n)?;
        String::from_utf8(b.to_vec())
            .map_err(|e| PhxError::Esquema(format!("nome nao e UTF-8 valido: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colunas_clientes() -> Vec<Column> {
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)).obrigatoria(),
            Column::new("cnpj", ColumnType::Str(14)),
            Column::new(
                "limite",
                ColumnType::Decimal {
                    precisao: 15,
                    escala: 2,
                },
            ),
            Column::new("foto", ColumnType::Bin),
            Column::new("observacao", ColumnType::Memo),
        ]
    }

    fn esquema_clientes() -> Schema {
        Schema::new(
            "cadastroClientes",
            colunas_clientes(),
            vec![
                IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
                IndexDef::new("porNome", vec![IndexColumn::asc(1).sem_caixa()]),
            ],
        )
        .unwrap()
    }

    #[test]
    fn layout_do_payload() {
        let s = esquema_clientes();
        // 6 declaradas + softdeleted + rownum + rowstamp + rowtime = 10, e com
        // 10 o bitmap ja precisa de 2 bytes (`div_ceil(8)`).
        assert_eq!(s.colunas().len(), 10);
        assert_eq!(s.bitmap_len(), 2);
        assert_eq!(s.offset_coluna(0).unwrap(), 2);
        assert_eq!(s.offset_coluna(1).unwrap(), 10);
        assert_eq!(s.offset_coluna(2).unwrap(), 70);
        // 2 de bitmap + 8 + 60 + 14 + 16 + 16 + 16 + 1 do softdeleted
        // + 8 do rownum + 8 do rowstamp + 8 do rowtime
        assert_eq!(s.payload_len(), 157);
    }

    /// A NONA coluna empurra o bitmap de 1 para 2 bytes, e com ele todos os
    /// offsets.
    ///
    /// Nao e defeito e nao quebra tabela nenhuma -- o `bitmap_len` sai da
    /// lista de colunas GRAVADA, entao um arquivo antigo continua lido com o
    /// bitmap dele --, mas quem escrever migracao precisa saber: a coluna de 8
    /// bytes faz o payload crescer 9 quando ela e a nona, e 8 quando nao e.
    #[test]
    fn a_nona_coluna_empurra_o_bitmap() {
        let oito = Schema::do_disco(
            "t",
            (0..8)
                .map(|i| Column::new(format!("c{i}"), ColumnType::Int1))
                .collect(),
            vec![],
        )
        .unwrap();
        let nove = Schema::do_disco(
            "t",
            (0..9)
                .map(|i| Column::new(format!("c{i}"), ColumnType::Int1))
                .collect(),
            vec![],
        )
        .unwrap();
        assert_eq!(oito.bitmap_len(), 1);
        assert_eq!(nove.bitmap_len(), 2);
        assert_eq!(
            nove.payload_len() - oito.payload_len(),
            2,
            "1 de dado + 1 de bitmap"
        );
    }

    /// A ordem das colunas de sistema e parte do formato: cada uma entra
    /// DEPOIS da anterior, nunca antes. Trocar a ordem deslocaria o offset de
    /// todas as seguintes em toda tabela ja gravada.
    #[test]
    fn as_colunas_de_sistema_saem_nesta_ordem() {
        let s = esquema_clientes();
        let n = s.colunas().len();
        assert_eq!(s.coluna_softdeleted(), Some(n - 4));
        assert_eq!(s.coluna_rownum(), Some(n - 3));
        assert_eq!(s.coluna_rowstamp(), Some(n - 2));
        assert_eq!(s.coluna_rowtime(), Some(n - 1));
        assert_eq!(s.colunas()[n - 3].ty, ColumnType::UInt8);
        assert_eq!(s.colunas()[n - 2].ty, ColumnType::UInt8);
        assert_eq!(s.colunas()[n - 1].ty, ColumnType::DateTime);
        assert!(s.colunas()[n - 4..].iter().all(|c| !c.nullable));
    }

    #[test]
    fn rownum_com_outro_tipo_e_recusada() {
        let mut cols = colunas_clientes();
        cols.push(Column::new(COLUNA_ROWNUM, ColumnType::Int4).obrigatoria());
        let e = Schema::new("t", cols, vec![]).unwrap_err();
        assert!(format!("{e}").contains("UInt8"), "{e}");
    }

    /// A coluna de sistema entra por ultimo, e so por ultimo: as colunas do
    /// usuario nao podem mudar de ORDEM nem de posicao dentro do payload por
    /// causa dela.
    ///
    /// A comparacao desconta o bitmap de proposito, e isso e medido e nao
    /// estilo: com dez colunas o bitmap passa a ocupar 2 bytes, entao todo
    /// offset absoluto anda 1. O que nao pode andar e a posicao RELATIVA de
    /// cada coluna do usuario -- e ela nao anda, que e o que faz um arquivo
    /// gravado com seis colunas continuar sendo lido com os offsets dele.
    #[test]
    fn softdeleted_entra_no_fim_e_nao_desloca_ninguem() {
        let com = esquema_clientes();
        let sem = Schema::do_disco(
            "clientes",
            colunas_clientes(),
            vec![IndexDef::new("por_nome", vec![IndexColumn::asc(1)])],
        )
        .unwrap();

        let i = com.coluna_softdeleted().unwrap();
        assert_eq!(i, com.colunas().len() - 4, "a softdeleted saiu do lugar");
        assert_eq!(com.colunas()[i].ty, ColumnType::Bool);
        assert!(!com.colunas()[i].nullable);
        assert!(sem.coluna_softdeleted().is_none());

        for j in 0..sem.colunas().len() {
            assert_eq!(
                com.offset_coluna(j).unwrap() - com.bitmap_len(),
                sem.offset_coluna(j).unwrap() - sem.bitmap_len(),
                "a coluna {j} mudou de lugar"
            );
        }
    }

    /// Este e o teste que protege a tabela ja gravada: ler um esquema v3 do
    /// disco NAO pode inventar uma coluna. Se inventasse, cada linha passaria
    /// a ser lida com os offsets deslocados -- e o CRC do slot continuaria
    /// batendo, porque os bytes seriam os mesmos.
    #[test]
    fn esquema_sem_a_coluna_de_sistema_volta_do_disco_sem_ela() {
        // Uma tabela gravada antes da v4 tem SO as colunas do usuario. O que
        // este teste prova e que a volta do disco nao inventa a setima.
        let antiga = Schema::do_disco("cadastroClientes", colunas_clientes(), vec![]).unwrap();
        assert!(antiga.coluna_softdeleted().is_none());

        let lido = Schema::desserializar(&antiga.serializar()).unwrap();
        assert!(
            lido.coluna_softdeleted().is_none(),
            "a leitura acrescentou a coluna de sistema numa tabela que nao a tem"
        );
        assert_eq!(lido.colunas().len(), 6);
        assert_eq!(lido.payload_len(), antiga.payload_len());
        assert_eq!(lido, antiga);
    }

    /// A v3 nao tem o byte do motivo obrigatorio no fim nem o bloco de marcas
    /// da v6. Ler uma nao pode estourar nem trazer lixo.
    #[test]
    fn v3_no_disco_para_antes_do_byte_novo() {
        let s = esquema_clientes();
        let mut bytes = s.serializar();
        bytes[4..6].copy_from_slice(&3u16.to_le_bytes());
        // Tira o bloco da v6 (uma marca por coluna) e o byte da v4.
        bytes.truncate(bytes.len() - s.colunas().len() - 1);
        let lido = Schema::desserializar(&bytes).unwrap();
        assert!(!lido.motivo_obrigatorio());
        assert!(lido.colunas().iter().all(|c| !c.dado_pessoal.e_pessoal()));
    }

    // ------------------------------------------------------- dado pessoal
    //
    // O teste que mais importa aqui e o do comportamento VELHO: uma tabela
    // gravada antes da v6 tem de abrir igual, e sem coluna marcada.

    /// **O teste do arquivo velho.** Um esquema v5 -- sem o bloco de marcas --
    /// abre inteiro, com todas as colunas em `Nao`.
    #[test]
    fn esquema_v5_abre_sem_marca_nenhuma() {
        let s = esquema_clientes();
        let v6 = s.serializar();

        // Um v5 de verdade: versao 5 e sem o bloco de marcas no fim.
        let mut v5 = v6.clone();
        v5[4..6].copy_from_slice(&5u16.to_le_bytes());
        v5.truncate(v6.len() - s.colunas().len());

        let lido = Schema::desserializar(&v5).unwrap();
        assert_eq!(lido.colunas().len(), s.colunas().len());
        assert_eq!(lido.payload_len(), s.payload_len());
        assert!(
            lido.colunas().iter().all(|c| !c.dado_pessoal.e_pessoal()),
            "a leitura de um v5 inventou marca de dado pessoal"
        );
        assert!(!lido.tem_dado_pessoal());
        // E o resto do esquema tem de ser identico ao de antes da mudanca.
        assert_eq!(lido, s);
    }

    #[test]
    fn a_marca_atravessa_o_disco() {
        let mut s = esquema_clientes();
        s.marcar_dado_pessoal("nome", DadoPessoal::Pessoal).unwrap();
        s.marcar_dado_pessoal("cnpj", DadoPessoal::Pessoal).unwrap();
        s.marcar_dado_pessoal("foto", DadoPessoal::Sensivel)
            .unwrap();

        let volta = Schema::desserializar(&s.serializar()).unwrap();
        assert_eq!(volta, s);

        let pessoais: Vec<&str> = volta
            .colunas_pessoais()
            .iter()
            .map(|(_, c)| c.nome.as_str())
            .collect();
        assert_eq!(pessoais, vec!["nome", "cnpj", "foto"]);
        assert_eq!(
            volta.colunas()[volta.coluna_por_nome("foto").unwrap()].dado_pessoal,
            DadoPessoal::Sensivel
        );
        assert!(volta.tem_dado_pessoal());
    }

    /// A marca nao pode deslocar nada: o payload de uma tabela marcada e o
    /// mesmo de uma nao marcada. E metadado, e nao dado.
    #[test]
    fn marcar_nao_mexe_no_layout_do_slot() {
        let sem = esquema_clientes();
        let mut com = esquema_clientes();
        com.marcar_dado_pessoal("cnpj", DadoPessoal::Sensivel)
            .unwrap();

        assert_eq!(com.payload_len(), sem.payload_len());
        for j in 0..sem.colunas().len() {
            assert_eq!(
                com.offset_coluna(j).unwrap(),
                sem.offset_coluna(j).unwrap(),
                "a coluna {j} mudou de lugar por causa de uma marca"
            );
        }
        // E o bloco novo custa exatamente um byte por coluna.
        assert_eq!(
            com.serializar().len(),
            sem.serializar().len(),
            "a marca mudou o tamanho do bloco"
        );
    }

    #[test]
    fn marcar_coluna_que_nao_existe_recusa() {
        let mut s = esquema_clientes();
        let e = s
            .marcar_dado_pessoal("telefone", DadoPessoal::Pessoal)
            .unwrap_err();
        assert!(format!("{e}").contains("telefone"), "{e}");
    }

    /// Um v6 truncado no meio do bloco de marcas deixa o resto em `Nao` --
    /// nunca a marca da coluna errada, que e o unico jeito de este bloco
    /// mentir.
    #[test]
    fn v6_truncado_no_bloco_de_marcas_nao_desloca_marca() {
        let mut s = esquema_clientes();
        s.marcar_dado_pessoal("nome", DadoPessoal::Pessoal).unwrap();
        s.marcar_dado_pessoal("cnpj", DadoPessoal::Sensivel)
            .unwrap();

        let mut bytes = s.serializar();
        let n = s.colunas().len();
        // Um v6 DE VERDADE: sem os blocos da v8 (a lista vazia de indices de
        // texto, 2 bytes) e da v9 (tres textos vazios por coluna, um por
        // indice mais um por coluna do indice, 2 bytes cada), e com a versao
        // 6 no cabecalho. Cortar o rabo de um v9 nao simula um v6 truncado:
        // simula um v9 truncado, e esse e erro de proposito.
        let v9 = 6 * n
            + s.indices()
                .iter()
                .map(|i| 2 * (1 + i.colunas.len()))
                .sum::<usize>();
        bytes.truncate(bytes.len() - v9 - 2);
        bytes[4..6].copy_from_slice(&6u16.to_le_bytes());
        // Corta o bloco de marcas ao meio.
        let cortado = &bytes[..bytes.len() - n / 2];
        let lido = Schema::desserializar(cortado).unwrap();

        // As que sobraram continuam nas colunas certas.
        let i_nome = lido.coluna_por_nome("nome").unwrap();
        let i_cnpj = lido.coluna_por_nome("cnpj").unwrap();
        assert_eq!(lido.colunas()[i_nome].dado_pessoal, DadoPessoal::Pessoal);
        assert_eq!(lido.colunas()[i_cnpj].dado_pessoal, DadoPessoal::Sensivel);
    }

    #[test]
    fn softdeleted_com_outro_tipo_e_recusada() {
        let mut cols = colunas_clientes();
        cols.push(Column::new(COLUNA_SOFTDELETED, ColumnType::Str(4)).obrigatoria());
        let e = Schema::new("t", cols, vec![]).unwrap_err();
        assert!(format!("{e}").contains("Bool"), "{e}");

        let mut cols = colunas_clientes();
        cols.push(Column::new(COLUNA_SOFTDELETED, ColumnType::Bool));
        let e = Schema::new("t", cols, vec![]).unwrap_err();
        assert!(format!("{e}").contains("nulo"), "{e}");
    }

    #[test]
    fn motivo_obrigatorio_atravessa_o_disco() {
        let s = esquema_clientes().com_motivo_obrigatorio(true);
        let volta = Schema::desserializar(&s.serializar()).unwrap();
        assert!(volta.motivo_obrigatorio());
        assert_eq!(s, volta);
    }

    #[test]
    fn serializacao_roundtrip() {
        let s = esquema_clientes();
        let bytes = s.serializar();
        let volta = Schema::desserializar(&bytes).unwrap();
        assert_eq!(s, volta);
    }

    #[test]
    fn indice_sobre_memo_e_rejeitado() {
        let r = Schema::new(
            "t",
            vec![Column::new("m", ColumnType::Memo)],
            vec![IndexDef::new("i", vec![IndexColumn::asc(0)])],
        );
        assert!(r.is_err());
    }

    #[test]
    fn coluna_duplicada_e_rejeitada() {
        let r = Schema::new(
            "t",
            vec![
                Column::new("a", ColumnType::Int4),
                Column::new("a", ColumnType::Int4),
            ],
            vec![],
        );
        assert!(r.is_err());
    }

    #[test]
    fn largura_de_chave_composta() {
        let s = esquema_clientes();
        // porId: 1 + 8
        assert_eq!(s.largura_chave(0).unwrap(), 9);
        // porNome: 1 + 60
        assert_eq!(s.largura_chave(1).unwrap(), 61);
    }
}

#[cfg(test)]
mod testes_indice_de_texto {
    use super::*;

    fn colunas() -> Vec<Column> {
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("titulo", ColumnType::Str(80)),
            Column::new("corpo", ColumnType::Memo),
            Column::new("valor", ColumnType::Int8),
        ]
    }

    fn com(textos: Vec<IndiceDeTexto>) -> Result<Schema> {
        Schema::new(
            "docs",
            colunas(),
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )?
        .com_indices_de_texto(textos)
    }

    #[test]
    fn indice_de_texto_sobre_str_e_sobre_memo_e_aceito() {
        assert!(com(vec![IndiceDeTexto::new("porTitulo", 1)]).is_ok());
        assert!(com(vec![IndiceDeTexto::new("porCorpo", 2)]).is_ok());
    }

    /// Indice de texto sobre numero nao indexa palavra nenhuma, e a recusa
    /// acontece na DECLARACAO -- uma tabela nasce uma vez e grava um milhao de
    /// vezes.
    #[test]
    fn indice_de_texto_sobre_numero_recusa_na_declaracao() {
        let e = com(vec![IndiceDeTexto::new("n", 3)])
            .unwrap_err()
            .to_string();
        assert!(e.contains("Str ou Memo"), "{e}");
    }

    #[test]
    fn indice_de_texto_sobre_coluna_inexistente_recusa_na_declaracao() {
        let e = com(vec![IndiceDeTexto::new("fora", 9)])
            .unwrap_err()
            .to_string();
        assert!(e.contains("inexistente"), "{e}");
    }

    /// Nome repetido daria duas respostas para a mesma pergunta em
    /// `procurar_texto`, que resolve pelo NOME.
    #[test]
    fn nome_repetido_recusa_na_declaracao() {
        let e = com(vec![IndiceDeTexto::new("t", 1), IndiceDeTexto::new("t", 2)])
            .unwrap_err()
            .to_string();
        assert!(e.contains("duplicado"), "{e}");
    }

    /// E colidir com o nome de uma ARVORE e o mesmo estrago por outro caminho:
    /// as duas listas sao separadas, mas quem procura por nome e um so.
    #[test]
    fn nome_que_colide_com_indice_comum_recusa_na_declaracao() {
        let e = com(vec![IndiceDeTexto::new("porId", 1)])
            .unwrap_err()
            .to_string();
        assert!(e.contains("mesmo nome de um indice comum"), "{e}");
    }

    /// Duas recusas que a versao anterior desta lista precisava escrever --
    /// "indice de texto nao pode ter duas colunas" e "nao pode ser unico ou
    /// primario" -- sumiram por CONSEQUENCIA da lista propria, nao por uma
    /// segunda regra: [`IndiceDeTexto`] nao tem onde guardar uma segunda
    /// coluna nem a marca de unico. E o mesmo desenho do par Cascata/Cascata,
    /// que some porque nao ha cascata no excluir.
    ///
    /// A dobra NASCE ligada, e desliga-la e escolha escrita.
    #[test]
    fn a_dobra_nasce_ligada_e_desligar_e_escrito() {
        let i = IndiceDeTexto::new("t", 1);
        assert!(i.dobrar, "indice de texto tem de nascer dobrando");
        assert!(!i.clone().sem_dobrar().dobrar);
    }

    /// O PSCH v8 leva a lista inteira de ida e volta.
    #[test]
    fn o_esquema_volta_do_disco_com_o_indice_de_texto() {
        let e = com(vec![
            IndiceDeTexto::new("porTitulo", 1),
            IndiceDeTexto::new("porCorpo", 2).sem_dobrar(),
        ])
        .unwrap();
        let volta = Schema::desserializar(&e.serializar()).unwrap();
        assert_eq!(volta, e, "o esquema tem de voltar igual do disco");
        assert_eq!(volta.indices_de_texto().len(), 2);
        assert!(volta.indices_de_texto()[0].dobrar);
        assert!(!volta.indices_de_texto()[1].dobrar);
        assert_eq!(volta.indices_de_texto()[1].coluna, 2);
        // A lista propria nao contaminou a das arvores.
        assert_eq!(volta.indices().len(), 1);
    }

    /// Tabela sem indice de texto continua gravando um PSCH que volta igual --
    /// o bloco novo existe, vazio, e nao muda nada para quem nao pediu.
    #[test]
    fn esquema_sem_indice_de_texto_volta_igual() {
        let e = Schema::new(
            "docs",
            colunas(),
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap();
        let volta = Schema::desserializar(&e.serializar()).unwrap();
        assert_eq!(volta, e);
        assert!(volta.indices_de_texto().is_empty());
    }

    /// **O IRMAO que ficou para tras no `ALTER TABLE ADD COLUMN`.**
    ///
    /// `Schema::com_coluna` remonta o esquema carregando o indice comum, a
    /// chave estrangeira e a particao -- e nao carregava a lista dos indices
    /// de TEXTO. Toda coluna acrescentada apagava a declaracao, e o `.fts` do
    /// disco virava orfao sem um erro no caminho: `procurar_texto` passava a
    /// recusar por "a tabela nao tem indice de texto chamado ...".
    ///
    /// Reponha o defeito tirando o `.com_indices_de_texto(textos)?` de
    /// `Schema::com_coluna`: a primeira assercao daqui cai em 0.
    #[test]
    fn a_coluna_nova_nao_apaga_o_indice_de_texto() {
        let e = com(vec![
            IndiceDeTexto::new("porTitulo", 1),
            IndiceDeTexto::new("porCorpo", 2).sem_dobrar(),
        ])
        .unwrap();
        let novo = e
            .com_coluna(
                Column::new("situacao", ColumnType::Str(12)),
                e.posicao_de_coluna_nova(),
            )
            .expect("acrescentar coluna");

        assert_eq!(
            novo.indices_de_texto().len(),
            2,
            "a coluna nova apagou os indices de texto do esquema"
        );
        assert_eq!(novo.indices_de_texto()[0].nome, "porTitulo");
        assert_eq!(novo.indices_de_texto()[1].nome, "porCorpo");
        // A dobra e escolha escrita, e ela tem de atravessar a remontagem.
        assert!(novo.indices_de_texto()[0].dobrar);
        assert!(
            !novo.indices_de_texto()[1].dobrar,
            "o `sem_dobrar` declarado sumiu na remontagem"
        );
        // E vai ao disco e volta com a lista inteira.
        let volta = Schema::desserializar(&novo.serializar()).unwrap();
        assert_eq!(volta, novo);
    }

    /// E carregar a lista SEM deslocar seria trocar um defeito por outro mais
    /// silencioso: `IndiceDeTexto.coluna` e POSICAO, e uma coluna que entra
    /// antes dela a empurra.
    ///
    /// **O silencio depende do TIPO da coluna vizinha, e esta era a metade que
    /// eu tinha errado.** Com `uf` entrando na posicao 0, `porTitulo` ficaria
    /// apontando `id Int8` e a propria `com_indices_de_texto` RECUSARIA -- alto
    /// e claro. O caso perigoso e este: `uf` entra na posicao 1, `porTitulo`
    /// fica apontando `uf Str(2)` e `porCorpo` fica apontando `titulo Str(80)`,
    /// os dois passam na conferencia de tipo, e o indice diz indexar o titulo
    /// enquanto indexa a sigla do estado. Por isso o teste confere o NOME da
    /// coluna apontada, e nao so o numero: aqui nao ha erro para esperar.
    ///
    /// Reponha o defeito trocando `desloca(it.coluna)` por `it.coluna` em
    /// `Schema::com_coluna`.
    #[test]
    fn o_indice_de_texto_anda_com_a_coluna_que_entrou_antes_dele() {
        let e = com(vec![
            IndiceDeTexto::new("porTitulo", 1),
            IndiceDeTexto::new("porCorpo", 2),
        ])
        .unwrap();
        // Posicao 1: a coluna entra ANTES das duas indexadas. E o caminho
        // publico do `com_coluna`, que aceita qualquer posicao -- o motor
        // hoje so usa `posicao_de_coluna_nova`, e la o deslocamento e nulo.
        let novo = e
            .com_coluna(Column::new("uf", ColumnType::Str(2)), 1)
            .expect("acrescentar coluna na frente das indexadas");

        let aponta = |i: usize| -> &str {
            let c = novo.indices_de_texto()[i].coluna;
            novo.colunas()[c].nome.as_str()
        };
        assert_eq!(
            aponta(0),
            "titulo",
            "o indice de texto ficou para tras e passou a indexar outra coluna"
        );
        assert_eq!(
            aponta(1),
            "corpo",
            "o indice de texto ficou para tras e passou a indexar outra coluna"
        );
        assert_eq!(novo.indices_de_texto()[0].coluna, 2);
        assert_eq!(novo.indices_de_texto()[1].coluna, 3);
        // E o controle do silencio: sem o `desloca` este esquema seria ACEITO.
        // As duas colunas erradas sao `Str`, e a conferencia de tipo da
        // `com_indices_de_texto` nao teria do que reclamar.
        assert!(matches!(
            novo.colunas()[1].ty,
            ColumnType::Str(_) | ColumnType::Memo
        ));
    }
}

#[cfg(test)]
mod testes_das_expressoes_de_esquema {
    use super::*;

    fn cols() -> Vec<Column> {
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
            Column::new(
                "preco",
                ColumnType::Decimal {
                    precisao: 12,
                    escala: 2,
                },
            ),
            Column::new("v", ColumnType::Int8),
        ]
    }

    /// O PSCH v9 leva as cinco expressoes de ida e volta, iguais.
    #[test]
    fn as_expressoes_atravessam_o_disco() {
        let mut c = cols();
        c[3] = Column::new("v", ColumnType::Int8)
            .com_padrao("7")
            .unwrap()
            .com_check("v > 0")
            .unwrap();
        c.push(
            Column::new("dobro", ColumnType::Int8)
                .com_calculada("v * 2")
                .unwrap(),
        );
        let indices = vec![
            IndexDef::new("pk", vec![IndexColumn::asc(0)]).primaria(),
            IndexDef::new("so_positivo", vec![IndexColumn::asc(3)])
                .com_onde("v > 0")
                .unwrap(),
            IndexDef::new("por_baixo", vec![IndexColumn::asc(1)])
                .com_expressao(0, "lower(nome)")
                .unwrap(),
        ];
        let e = Schema::new("t", c, indices).unwrap();
        assert!(e.tem_regras());
        assert!(e.tem_indice_condicional());
        let volta = Schema::desserializar(&e.serializar()).unwrap();
        assert_eq!(volta, e);
        let v = volta.coluna_por_nome("v").unwrap();
        assert_eq!(volta.colunas()[v].padrao.as_ref().unwrap().texto(), "7");
        assert_eq!(volta.colunas()[v].check.as_ref().unwrap().texto(), "v > 0");
        assert_eq!(volta.indices()[1].onde.as_ref().unwrap().texto(), "v > 0");
        assert_eq!(
            volta.indices()[2].expressoes[0].as_ref().unwrap().texto(),
            "lower(nome)"
        );
    }

    /// **O teste do arquivo velho.** Um v8 -- sem o bloco -- abre inteiro e
    /// sem regra nenhuma, e continua igual ao que era.
    #[test]
    fn esquema_v8_abre_sem_regra_nenhuma() {
        let e = Schema::new(
            "t",
            cols(),
            vec![IndexDef::new("pk", vec![IndexColumn::asc(0)]).primaria()],
        )
        .unwrap();
        let mut v9 = e.serializar();
        let n = e.colunas().len();
        let bloco = 6 * n + 2 * (1 + 1);
        v9.truncate(v9.len() - bloco);
        v9[4..6].copy_from_slice(&8u16.to_le_bytes());
        let lido = Schema::desserializar(&v9).unwrap();
        assert_eq!(lido, e);
        assert!(!lido.tem_regras());
        assert!(!lido.tem_indice_condicional());
    }

    /// Um v9 cortado no bloco das expressoes e ERRO, nao «fica sem»: CHECK
    /// que some calado e garantia perdida.
    #[test]
    fn v9_truncado_no_bloco_de_expressoes_recusa() {
        let mut c = cols();
        c[3] = Column::new("v", ColumnType::Int8)
            .com_check("v > 0")
            .unwrap();
        let e = Schema::new("t", c, vec![]).unwrap();
        let bytes = e.serializar();
        let erro = Schema::desserializar(&bytes[..bytes.len() - 3])
            .unwrap_err()
            .to_string();
        assert!(erro.contains("v9") && erro.contains("truncado"), "{erro}");
    }

    /// A recusa e na DECLARACAO, nomeando a coluna e o motivo.
    #[test]
    fn a_declaracao_recusa_o_que_nao_fecha() {
        let com =
            |c: Column| Schema::new("t", vec![Column::new("id", ColumnType::Int8), c], vec![]);
        let e = com(Column::new("v", ColumnType::Int8)
            .com_check("w > 0")
            .unwrap())
        .unwrap_err()
        .to_string();
        assert!(e.contains("check de v") && e.contains("\"w\""), "{e}");
        let e = Column::new("v", ColumnType::Int8)
            .com_padrao("1 +")
            .unwrap_err()
            .to_string();
        assert!(e.contains("padrao de v"), "{e}");
        let e = Schema::new(
            "t",
            vec![
                Column::new("a", ColumnType::Int8)
                    .com_calculada("1")
                    .unwrap(),
                Column::new("b", ColumnType::Int8)
                    .com_calculada("a * 2")
                    .unwrap(),
            ],
            vec![],
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("calculada nao se apoia em calculada"), "{e}");
        let e = com(Column::new("v", ColumnType::Bin)
            .com_calculada("1")
            .unwrap())
        .unwrap_err()
        .to_string();
        assert!(e.contains("nao produz binario"), "{e}");
        // Indice: expressao de duas colunas, e expressao da coluna errada.
        let dois = Schema::new(
            "t",
            cols(),
            vec![IndexDef::new("i", vec![IndexColumn::asc(1)])
                .com_expressao(0, "concat(nome, id)")
                .unwrap()],
        )
        .unwrap_err()
        .to_string();
        assert!(dois.contains("exatamente a coluna nome"), "{dois}");
        let errada = Schema::new(
            "t",
            cols(),
            vec![IndexDef::new("i", vec![IndexColumn::asc(1)])
                .com_expressao(0, "abs(v)")
                .unwrap()],
        )
        .unwrap_err()
        .to_string();
        assert!(errada.contains("exatamente a coluna nome"), "{errada}");
        let filtro = Schema::new(
            "t",
            cols(),
            vec![IndexDef::new("i", vec![IndexColumn::asc(1)])
                .com_onde("w > 0")
                .unwrap()],
        )
        .unwrap_err()
        .to_string();
        assert!(filtro.contains("filtro do indice i"), "{filtro}");
        // E o caso legitimo passa: o controle da recusa.
        Schema::new(
            "t",
            cols(),
            vec![IndexDef::new("i", vec![IndexColumn::asc(1)])
                .com_expressao(0, "LOWER(Nome)")
                .unwrap()
                .com_onde("preco > 0")
                .unwrap()],
        )
        .unwrap();
    }

    /// Pedido 475: o DEFAULT e uma EXPRESSAO (MANUAL.txt:398), entao um
    /// texto solto sem aspas e lido como NOME DE COLUNA -- e quem manda
    /// `"padrao":"ativo"` esperando o texto "ativo" leva "coluna que a
    /// tabela nao tem" sem entender por que. A recusa CONTINUA (e tem de
    /// continuar: PostgreSQL, MySQL e MariaDB tambem recusam identificador
    /// solto num DEFAULT), so que agora vem com a dica.
    #[test]
    fn padrao_com_identificador_solto_recusa_e_ensina_a_aspa() {
        let e = Schema::new(
            "t",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("situacao", ColumnType::Str(12))
                    .com_padrao("ativo")
                    .unwrap(),
            ],
            vec![],
        )
        .unwrap_err()
        .to_string();
        // O comportamento de sempre: continua recusando, com a mesma frase
        // de antes da dica.
        assert!(
            e.contains("o padrao de situacao usa a coluna \"ativo\", que a tabela nao tem"),
            "{e}"
        );
        // O que e novo: a dica de como escrever um texto no padrao.
        assert!(e.contains("texto vai entre aspas simples: 'ativo'"), "{e}");
    }

    /// O irmao (check) fica como estava -- a dica e so do padrao, porque so
    /// ali a confusao "texto vs nome de coluna" e comum (pedido 475). Se
    /// este teste passar a falhar por causa de "aspas simples", a dica
    /// vazou para o caminho irmao sem pedido para isso.
    #[test]
    fn check_com_identificador_inexistente_nao_ganha_a_dica() {
        let e = Schema::new(
            "t",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("v", ColumnType::Int8)
                    .com_check("w > 0")
                    .unwrap(),
            ],
            vec![],
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("check de v") && e.contains("\"w\""), "{e}");
        assert!(!e.contains("aspas simples"), "{e}");
    }
}

#[cfg(test)]
mod testes_o_crivo_do_indice_comum_ficou {
    use super::*;

    /// A separacao das regras de tipo NAO pode ter aberto buraco no indice
    /// comum: `Memo` continua recusado nele.
    ///
    /// Esta e a guarda do comportamento VELHO, e ela vale mais que a do novo.
    /// Trocar `if !idx.texto && !col.ty.indexavel()` por `if false` faria os
    /// seis testes do indice de texto passarem e este falhar sozinho.
    #[test]
    fn indice_comum_sobre_memo_continua_recusado() {
        let e = Schema::new(
            "docs",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("corpo", ColumnType::Memo),
            ],
            vec![IndexDef::new("comum", vec![IndexColumn::asc(1)])],
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("nao e indexavel"), "{e}");
    }
}

/* =========================================================== pedido 315
IDENTIDADE QUE SE REPETE NAO E IDENTIDADE.

O `id` de coluna e um UUID v7 sorteado em `Column::new` e NUNCA reaproveitado
-- e o que deixa renomear a coluna sem quebrar tela nem relatorio. Ate
17/09/2026 nada conferia que ele fosse unico DENTRO da tabela: o `"id"` vindo
do pedido (`valores::coluna_de_json`) podia repetir o de outra coluna, e as
duas viravam a mesma identidade para quem aponta por id.

A conferencia entra nos dois caminhos de DECLARAR -- a tabela nova e a coluna
que chega depois -- e nao no `do_disco`, que e o de LER. Guarda no caminho de
leitura recusaria abrir tabela gravada antes dela. */
#[cfg(test)]
mod testes_id_repetido_de_coluna {
    use super::*;

    fn com_id(nome: &str, id: Uuid) -> Column {
        Column::new(nome, ColumnType::Str(20)).com_id(id)
    }

    /// Tabela NOVA com duas colunas carregando o mesmo id: recusada, e a
    /// recusa nomeia as duas -- so uma nao diz qual corrigir.
    #[test]
    fn tabela_nova_com_id_repetido_e_recusada() {
        let id = Uuid::v7();
        let e = Schema::new("t", vec![com_id("a", id), com_id("b", id)], vec![])
            .unwrap_err()
            .to_string();
        assert!(e.contains(" a ") && e.contains(" b "), "{e}");
        assert!(e.contains(&id.to_string()), "a recusa mostra o id: {e}");
    }

    /// O IRMAO: `acrescentar_coluna` monta a coluna pelo MESMO caminho do
    /// `criar_tabela` e nao passa por `Schema::new`. Sem esta conferencia, o
    /// id repetido entrava por aqui mesmo com a tabela nova protegida.
    #[test]
    fn coluna_acrescentada_com_id_de_uma_que_ja_existe_e_recusada() {
        let id = Uuid::v7();
        let esq = Schema::new(
            "t",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                com_id("cidade", id),
            ],
            vec![],
        )
        .unwrap();
        let e = esq
            .com_coluna(com_id("bairro", id), esq.posicao_de_coluna_nova())
            .unwrap_err()
            .to_string();
        assert!(e.contains("bairro") && e.contains("cidade"), "{e}");
    }

    /// O COMPORTAMENTO VELHO, nos dois caminhos: id distinto passa, e coluna
    /// sem id declarado continua ganhando o sorteado. E o caso de 100% das
    /// tabelas de hoje.
    #[test]
    fn id_distinto_e_ausencia_de_id_continuam_passando() {
        let esq = Schema::new(
            "t",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                com_id("cidade", Uuid::v7()),
                com_id("bairro", Uuid::v7()),
            ],
            vec![],
        )
        .expect("ids distintos deviam passar");
        let n = esq.colunas().len();
        let novo = esq
            .com_coluna(
                Column::new("uf", ColumnType::Str(2)),
                esq.posicao_de_coluna_nova(),
            )
            .expect("coluna sem id declarado devia passar");
        assert_eq!(novo.colunas().len(), n + 1);

        // E nenhum id se repete no esquema resultante, contadas tambem as
        // colunas de sistema que `Schema::new` acrescenta sozinho.
        let mut ids: Vec<String> = novo.colunas().iter().map(|c| c.id.to_string()).collect();
        ids.sort();
        let antes = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), antes, "o esquema nasceu com id repetido");
    }

    /// A guarda NAO desce ao caminho da leitura: um esquema com id repetido
    /// que ja esteja no disco continua abrindo. Guarda nova entra pedida, nao
    /// imposta -- ela protege o esquecimento de amanha, nao o dado de ontem.
    #[test]
    fn esquema_com_id_repetido_que_ja_esta_no_disco_continua_abrindo() {
        let id = Uuid::v7();
        let esq = Schema::do_disco("t", vec![com_id("a", id), com_id("b", id)], vec![])
            .expect("o caminho do disco nao julga o id");
        assert_eq!(esq.colunas()[0].id, esq.colunas()[1].id);
        // E ele vai e volta pelo PSCH sem perder nada.
        let volta = Schema::desserializar(&esq.serializar()).expect("devia reler");
        assert_eq!(volta.colunas()[0].id, volta.colunas()[1].id);
    }
}

/* **Modo ledger e dado pessoal nao convivem -- pedido 355, decisao do dono de
18/09/2026.**

O hash de cada bloco e um SHA-256 SEM SAL do conteudo em claro, gravado numa
coluna (`hash`, `Uuid256`) que nao e marcada e por isso nao e cifrada: quem tem
a lista dos valores possiveis confirma qual esta ali por tentativa. A recusa
entra nos TRES caminhos de declarar -- a tabela nova, a coluna marcada depois e
a coluna que chega -- e em nenhum do `do_disco`, que e o de LER: cadeia que ja
existe continua abrindo, porque ali o oraculo ja queimou. */
#[cfg(test)]
mod testes_ledger_com_dado_pessoal {
    use super::*;

    /// As quatro pecas da cadeia, mais um `cpf` cujo grau quem chama escolhe.
    fn colunas_da_cadeia(cpf: DadoPessoal) -> Vec<Column> {
        vec![
            Column::new("hash", ColumnType::Uuid256).obrigatoria(),
            Column::new("anterior", ColumnType::Uuid256),
            Column::new("altura", ColumnType::Sequence),
            Column::new("cpf", ColumnType::Str(11)).com_dado_pessoal(cpf),
        ]
    }

    fn indices_da_cadeia() -> Vec<IndexDef> {
        vec![IndexDef::new("porAltura", vec![IndexColumn::asc(2)]).unico()]
    }

    fn cadeia(cpf: DadoPessoal) -> Result<Schema> {
        Schema::new("blocos", colunas_da_cadeia(cpf), indices_da_cadeia())
    }

    /// (A) A combinacao NAO NASCE MAIS, e a recusa nomeia a coluna e diz por
    /// que. Prova real: sem a chamada a `conferir_ledger_sem_dado_pessoal` no
    /// `Schema::new`, este esquema monta sem queixa nenhuma -- ele e valido em
    /// todo o resto -- e o `unwrap_err` entra em panico.
    #[test]
    fn ledger_com_coluna_marcada_nao_nasce() {
        for grau in [DadoPessoal::Pessoal, DadoPessoal::Sensivel] {
            let e = cadeia(grau).unwrap_err().to_string().to_lowercase();
            assert!(e.contains("cpf"), "a recusa tinha de nomear a coluna: {e}");
            assert!(e.contains("ledger"), "a recusa nao diz o modo: {e}");
            assert!(
                e.contains("hash") && e.contains("sal"),
                "a recusa nao ensina o motivo (hash sem sal): {e}"
            );
        }
    }

    /// (B) O PAR do de cima, sem o qual ele passaria com um portao que recusa
    /// tudo: as tres combinacoes legitimas continuam nascendo.
    #[test]
    fn o_portao_nao_recusa_o_que_e_legitimo() {
        // Ledger SEM coluna marcada -- a cadeia de sempre.
        let l = cadeia(DadoPessoal::Nao).expect("ledger sem marca tinha de nascer");
        assert!(e_tabela_ledger(&l));

        // Tabela COMUM com coluna marcada -- a LGPD de sempre.
        let c = Schema::new(
            "clientes",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("cpf", ColumnType::Str(11)).com_dado_pessoal(DadoPessoal::Pessoal),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .expect("tabela comum com dado pessoal tinha de nascer");
        assert!(!e_tabela_ledger(&c) && c.tem_dado_pessoal());

        // Quase-ledger: as tres colunas SEM o indice unico nao sao cadeia, e
        // ali a marca continua valendo -- o hash nem existe.
        let quase = Schema::new(
            "quase",
            colunas_da_cadeia(DadoPessoal::Pessoal),
            vec![IndexDef::new("porHash", vec![IndexColumn::asc(0)]).unico()],
        )
        .expect("sem o indice da altura nao ha cadeia, e nao ha oraculo");
        assert!(!e_tabela_ledger(&quase));
    }

    /// A coluna que o hash NAO cobre continua podendo ser marcada: marcar a
    /// propria `hash` nao abre oraculo nenhum. E o crivo que sai do mesmo
    /// `coluna_no_hash_do_ledger` que monta o conteudo canonico.
    #[test]
    fn marca_em_coluna_fora_do_hash_continua_passando() {
        let mut colunas = colunas_da_cadeia(DadoPessoal::Nao);
        colunas[0].dado_pessoal = DadoPessoal::Pessoal; // a propria `hash`
        Schema::new("blocos", colunas, indices_da_cadeia())
            .expect("o hash nao cobre a si mesmo, entao nao ha o que confirmar");
    }

    /// (IRMAO) Recusar so no nascimento deixaria a porta dos fundos aberta:
    /// criar a cadeia limpa e marcar a coluna no pedido seguinte. Prova real:
    /// sem a guarda no `marcar_dado_pessoal`, a marca entra e o `unwrap_err`
    /// entra em panico.
    #[test]
    fn marcar_a_coluna_depois_e_recusado() {
        let mut l = cadeia(DadoPessoal::Nao).unwrap();
        let e = l
            .marcar_dado_pessoal("cpf", DadoPessoal::Pessoal)
            .unwrap_err()
            .to_string()
            .to_lowercase();
        assert!(e.contains("cpf") && e.contains("ledger"), "{e}");
        assert!(
            !l.tem_dado_pessoal(),
            "a recusa nao podia ter deixado a marca"
        );
    }

    /// O PAR do irmao: marcar continua valendo na tabela comum, e DESMARCAR
    /// vale sempre -- inclusive na cadeia que nasceu marcada antes da guarda.
    /// Desmarcar e o unico remedio dela, e uma guarda que olhasse o ESTADO em
    /// vez da transicao travaria justamente o conserto.
    #[test]
    fn marcar_na_comum_e_desmarcar_na_cadeia_continuam_podendo() {
        let mut c = Schema::new(
            "clientes",
            vec![Column::new("cpf", ColumnType::Str(11))],
            vec![],
        )
        .unwrap();
        c.marcar_dado_pessoal("cpf", DadoPessoal::Pessoal)
            .expect("marcar numa tabela comum e a LGPD de sempre");
        assert!(c.tem_dado_pessoal());

        // Uma cadeia como as gravadas antes da guarda: vem pelo `do_disco`.
        let mut legada = Schema::do_disco(
            "blocos",
            colunas_da_cadeia(DadoPessoal::Pessoal),
            indices_da_cadeia(),
        )
        .expect("o caminho do disco nao julga a combinacao");
        legada
            .marcar_dado_pessoal("cpf", DadoPessoal::Sensivel)
            .expect("trocar o grau de uma coluna ja marcada nao expoe mais nada");
        legada
            .marcar_dado_pessoal("cpf", DadoPessoal::Nao)
            .expect("desmarcar e o remedio, e nunca se recusa");
        assert!(!legada.tem_dado_pessoal());
    }

    /// (TERCEIRO CAMINHO) A coluna marcada que CHEGA a uma cadeia e recusada.
    /// O `Table::acrescentar_coluna` ja recusa toda coluna em modo ledger --
    /// esta guarda e do verbo de declarar, uma camada abaixo, e sobrevive a
    /// quem um dia afrouxar aquela.
    #[test]
    fn coluna_marcada_que_chega_na_cadeia_e_recusada() {
        let l = cadeia(DadoPessoal::Nao).unwrap();
        let e = l
            .com_coluna(
                Column::new("salario", ColumnType::Int8).com_dado_pessoal(DadoPessoal::Sensivel),
                l.posicao_de_coluna_nova(),
            )
            .unwrap_err()
            .to_string()
            .to_lowercase();
        assert!(e.contains("salario") && e.contains("ledger"), "{e}");
    }

    /// O PAR: coluna SEM marca continua entrando na cadeia (quem a recusa e o
    /// `Table::acrescentar_coluna`, pelo hash, e nao esta guarda), e coluna
    /// marcada continua entrando na tabela comum.
    #[test]
    fn coluna_sem_marca_e_coluna_marcada_na_comum_continuam_entrando() {
        let l = cadeia(DadoPessoal::Nao).unwrap();
        l.com_coluna(
            Column::new("autor", ColumnType::Str(40)),
            l.posicao_de_coluna_nova(),
        )
        .expect("coluna sem marca nao e assunto desta guarda");

        let c = Schema::new(
            "clientes",
            vec![Column::new("id", ColumnType::Int8).obrigatoria()],
            vec![],
        )
        .unwrap();
        c.com_coluna(
            Column::new("cpf", ColumnType::Str(11)).com_dado_pessoal(DadoPessoal::Pessoal),
            c.posicao_de_coluna_nova(),
        )
        .expect("marcar coluna em tabela comum e a LGPD de sempre");
    }

    /// **O COMPORTAMENTO VELHO, que e o que mais importa numa guarda nova.**
    /// A cadeia que ja esta no disco com coluna marcada volta inteira: o
    /// `do_disco` nao julga, e o PSCH devolve o byte do grau como foi gravado.
    /// Se a guarda descesse ao caminho da leitura, esta tabela sairia do ar.
    #[test]
    fn cadeia_marcada_que_ja_esta_no_disco_continua_voltando() {
        let esq = Schema::do_disco(
            "blocos",
            colunas_da_cadeia(DadoPessoal::Pessoal),
            indices_da_cadeia(),
        )
        .expect("o caminho do disco nao julga a combinacao");

        let volta = Schema::desserializar(&esq.serializar()).expect("a cadeia tinha de reabrir");
        assert!(e_tabela_ledger(&volta), "voltou sem ser cadeia");
        let i = volta.coluna_por_nome("cpf").unwrap();
        assert_eq!(
            volta.colunas()[i].dado_pessoal,
            DadoPessoal::Pessoal,
            "a marca gravada tinha de voltar como foi gravada"
        );
    }
}

/// O oraculo do rowid (pedido 358) pelo lado do NUCLEO: o que a guarda alcanca
/// e, sobretudo, o que ela nao pode alcancar.
#[cfg(test)]
mod testes_oraculo_do_rowid {
    use super::*;
    use crate::paginacao::Periodo;

    /// Uma tabela com a combinacao, montada SEM passar pelo `com_paginacao` --
    /// e por isso ela existe: e o retrato do que ja esta no disco.
    fn com_a_combinacao(modo: ModoParticao) -> Schema {
        let colunas = vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Sensivel),
            Column::new("nascimento", ColumnType::Date).obrigatoria(),
        ];
        // A paginacao sai do construtor de cada modo, e nao de um literal: a
        // alfanumerica tem 37 volumes fixos, e um numero na mao aqui faria o
        // teste falhar pelo motivo errado -- ja falhou, em 18/09/2026.
        let pag = match modo {
            ModoParticao::PorLetra { coluna } => Paginacao::por_letra(100, coluna).unwrap(),
            _ => Paginacao::nova(100, 9).unwrap(),
        };
        Schema::do_disco("clientes", colunas, vec![])
            .unwrap()
            .com_paginacao_do_disco(Paginacao { modo, ..pag })
    }

    /// **A prova real do lugar da guarda.** Ela NAO pode morar dentro do
    /// `com_paginacao`, porque `Schema::com_coluna` -- o `ALTER TABLE ADD
    /// COLUMN` -- o chama de novo com as colunas que ja carregam a marca. Uma
    /// guarda ali derrubaria o `ADD COLUMN` de uma tabela em producao.
    ///
    /// O esquema aqui e montado pelo caminho do DISCO de proposito: assim o
    /// teste falha na linha do `com_coluna`, e nao no proprio preparo.
    #[test]
    fn a_tabela_que_ja_tem_a_combinacao_continua_ganhando_coluna() {
        for modo in [
            ModoParticao::PorLetra { coluna: 1 },
            ModoParticao::PorPeriodo {
                coluna: 2,
                periodo: Periodo::Mensal,
            },
        ] {
            let esq = com_a_combinacao(modo);
            let novo = esq
                .com_coluna(
                    Column::new("cidade", ColumnType::Str(30)),
                    esq.posicao_de_coluna_nova(),
                )
                .expect("ADD COLUMN nao pode morrer por causa de guarda nova");
            assert!(novo.tem_dado_pessoal(), "a coluna nova apagou a marca");
        }
    }

    /// E a volta do disco nao revalida: o esquema gravado abre como foi
    /// gravado. Recusar a leitura nao apaga o oraculo que ja queimou -- so
    /// tiraria do ar uma tabela que esta perfeita.
    #[test]
    fn a_combinacao_gravada_volta_do_disco_inteira() {
        let esq = com_a_combinacao(ModoParticao::PorLetra { coluna: 1 });
        let volta = Schema::desserializar(&esq.serializar()).expect("tinha de reabrir");
        assert!(volta.paginacao().modo.por_letra());
        assert_eq!(volta.colunas()[1].dado_pessoal, DadoPessoal::Sensivel);
    }

    /// O portao le a coluna que o ROWID revela, e nao a lista das marcadas: uma
    /// tabela com dez colunas marcadas e a particao por uma decima primeira
    /// continua nascendo.
    #[test]
    fn o_portao_olha_a_coluna_da_particao_e_nao_a_tabela() {
        let esq = com_a_combinacao(ModoParticao::PorQuantidade);
        // `nome` esta marcada, mas a particao e por `nascimento`, que nao esta.
        esq.conferir_oraculo_do_rowid(ModoParticao::PorPeriodo {
            coluna: 2,
            periodo: Periodo::Anual,
        })
        .expect("a coluna da particao nao e a marcada");
        // Pela `nome`, recusa -- e nomeia a coluna.
        let erro = esq
            .conferir_oraculo_do_rowid(ModoParticao::PorLetra { coluna: 1 })
            .expect_err("a coluna do balde e a marcada")
            .to_string();
        assert!(erro.contains("nome") && erro.contains("rowid"), "{erro}");
        // E por quantidade nunca recusa: o rowid e a ordem de chegada.
        esq.conferir_oraculo_do_rowid(ModoParticao::PorQuantidade)
            .expect("por quantidade nao revela coluna nenhuma");
    }
}
