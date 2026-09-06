//! Esquema de uma tabela PhxSql: colunas, indices e o layout do slot.
//!
//! O esquema e serializado dentro do proprio `.reg`, logo apos o cabecalho.
//! Assim uma tabela e auto-descritiva: basta o quarteto de arquivos para
//! reabrir e ler os dados, sem dicionario externo.

use crate::error::{PhxError, Result};
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
const VERSAO_ESQUEMA: u16 = 8;
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

/// Este nome e de uma coluna do motor?
///
/// Existe para os lugares que precisam ESCONDER as colunas de sistema --
/// a grade, o formulario, a juncao -- nao terem cada um a sua lista. Coluna
/// de sistema nova entra aqui e some dos tres de uma vez; a lista repetida em
/// tres lugares e onde a quarta seria esquecida.
pub fn e_coluna_de_sistema(nome: &str) -> bool {
    nome == COLUNA_SOFTDELETED || nome == COLUNA_ROWNUM
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
        }
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
    /// Este e um indice de TEXTO (`.fts`), e nao da arvore comum.
    ///
    /// Ele nao guarda o valor da coluna: guarda cada PALAVRA dela, dobrada.
    /// Por isso vale sobre uma coluna so, nao pode ser unico nem primario, e a
    /// coluna tem de ser texto -- e `Schema::new` recusa os tres casos na
    /// DECLARACAO, que e onde custa um erro lido enquanto se cria a tabela.
    /// Recusar na gravacao custaria um banco inteiro modelado errado.
    pub texto: bool,
    /// O indice de texto dobra acento? Nasce LIGADO.
    ///
    /// E o inverso da regra «guarda nova entra pedida», e o motivo esta
    /// medido: a busca de hoje nao dobra acento, entao um indice sem dobra
    /// acharia MENOS que a varredura -- e indice que acha menos que a
    /// varredura e pior que nao ter indice (`docs/FTS.md` §5.1).
    pub dobrar: bool,
}

impl IndexDef {
    pub fn new(nome: impl Into<String>, colunas: Vec<IndexColumn>) -> Self {
        IndexDef {
            nome: nome.into(),
            colunas,
            unico: false,
            primario: false,
            texto: false,
            dobrar: true,
        }
    }

    /// Marca este indice como de TEXTO. Nasce dobrando acento.
    pub fn de_texto(mut self) -> Self {
        self.texto = true;
        self
    }

    /// Desliga a dobra de acento -- escolha escrita, em vez de omissao.
    pub fn sem_dobrar(mut self) -> Self {
        self.dobrar = false;
        self
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
    paginacao: Paginacao,
    offsets: Vec<usize>,
    bitmap_len: usize,
    payload_len: usize,
    /// Exigir motivo escrito para marcar uma linha como excluida.
    motivo_obrigatorio: bool,
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
        Schema::do_disco(nome, colunas, indices)
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
                // O crivo de tipo do indice COMUM nao vale para o de texto,
                // e a diferenca e de significado, nao de rigor: a arvore comum
                // indexa o VALOR da coluna, e por isso um `Memo` -- que tem
                // tamanho livre e mora fora do slot -- nao e indexavel. O
                // indice de texto indexa as PALAVRAS, e o `.memo` e justamente
                // o caso que ele existe para resolver. Cada um tem a sua regra
                // de tipo, e a do texto esta logo abaixo.
                if !idx.texto && !col.ty.indexavel() {
                    return Err(PhxError::Esquema(format!(
                        "indice {} usa coluna {} do tipo {:?}, que nao e indexavel",
                        idx.nome, col.nome, col.ty
                    )));
                }
            }

            // As tres recusas do indice de TEXTO, todas na declaracao.
            //
            // Uma tabela nasce uma vez e grava um milhao de vezes: recusar
            // cedo custa um erro lido enquanto se cria a tabela; recusar tarde
            // custa um banco modelado errado, descoberto no dia da primeira
            // busca. E a mesma decisao do `ao_excluir`.
            if idx.texto {
                if idx.colunas.len() != 1 {
                    return Err(PhxError::Esquema(format!(
                        "o indice de texto {} declara {} colunas; ele vale sobre \
                         UMA so, porque indexa as palavras dela e nao o valor",
                        idx.nome,
                        idx.colunas.len()
                    )));
                }
                if idx.unico || idx.primario {
                    return Err(PhxError::Esquema(format!(
                        "o indice de texto {} esta marcado como unico ou primario; \
                         a mesma palavra aparece em muitas linhas, e e disso que \
                         a lista de ocorrencias e feita",
                        idx.nome
                    )));
                }
                let col = &colunas[idx.colunas[0].coluna];
                if !matches!(col.ty, ColumnType::Str(_) | ColumnType::Memo) {
                    return Err(PhxError::Esquema(format!(
                        "o indice de texto {} usa a coluna {} do tipo {:?}; \
                         indice de texto vale sobre Str ou Memo",
                        idx.nome, col.nome, col.ty
                    )));
                }
            }
        }

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
            paginacao: Paginacao::DESLIGADA,
            offsets,
            bitmap_len,
            payload_len: pos,
            motivo_obrigatorio: false,
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

    /// Troca o grau de uma coluna pelo NOME.
    ///
    /// Pelo nome e nao pelo indice porque quem classifica e gente olhando a
    /// ficha, e o indice de uma coluna nao aparece em tela nenhuma.
    pub fn marcar_dado_pessoal(&mut self, coluna: &str, grau: DadoPessoal) -> Result<()> {
        match self.colunas.iter_mut().find(|c| c.nome == coluna) {
            Some(c) => {
                c.dado_pessoal = grau;
                Ok(())
            }
            None => Err(PhxError::NaoEncontrado(format!(
                "a tabela {} nao tem a coluna {coluna:?}",
                self.nome
            ))),
        }
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
    /// Tres coisas guardam POSICAO de coluna, e nao nome: `IndexColumn.coluna`,
    /// `ForeignKey.colunas` e a coluna de referencia da particao. Inserir uma
    /// coluna no meio empurra todas as posicoes a partir dela, e quem ficar
    /// para tras passa a apontar a vizinha -- indice sobre o campo errado,
    /// particao pela coluna errada, e nenhum erro no caminho.
    ///
    /// Por isso o remapeamento mora aqui e nao em quem chama: a proxima coisa
    /// que guardar posicao entra nesta funcao, e nao num quarto lugar que
    /// alguem vai esquecer.
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

        let novo = Schema::do_disco(self.nome.clone(), colunas, indices)?
            .com_chaves_estrangeiras(fks)?
            .com_paginacao(paginacao)?;
        Ok(novo.com_motivo_obrigatorio(self.motivo_obrigatorio))
    }

    /// Acrescenta as chaves estrangeiras da tabela.
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
        // v8: os dois bytes do indice de texto, um par por indice, na ordem
        // dos indices.
        //
        // **No fim, e nao dentro do registro de cada indice** -- e isto nao e
        // estilo, e a convencao que a v4 e a v6 escreveram ao lado delas
        // mesmas: *quem le uma versao antiga simplesmente para antes daqui*.
        // A primeira versao desta mudanca pos os dois bytes no meio do
        // registro do indice, e duas guardas de compatibilidade cairam na
        // hora -- elas simulam arquivo velho truncando a CAUDA, e campo no
        // meio quebra a simulacao. As guardas estavam certas: a convencao era
        // carga, e nao enfeite.
        for idx in &self.indices {
            out.push(idx.texto as u8);
            out.push(idx.dobrar as u8);
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
            indices.push(IndexDef {
                nome,
                colunas: cols,
                unico,
                primario,
                // v8, e ela vem no FIM do bloco: aqui os dois nascem no padrao
                // de quem foi gravado antes dela.
                texto: false,
                dobrar: true,
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

        // v8: os dois bytes do indice de texto, na ordem dos indices. Quem le
        // uma v7 para antes daqui, e os indices ficam com o padrao de quem foi
        // gravado antes: `texto` desligado, `dobrar` ligado.
        if versao >= 8 {
            for idx in indices.iter_mut() {
                match (leitor.u8(), leitor.u8()) {
                    (Ok(t), Ok(d)) => {
                        idx.texto = t != 0;
                        idx.dobrar = d != 0;
                    }
                    _ => break,
                }
            }
        }

        // `do_disco`, e nao `new`: a lista de colunas gravada e a verdade
        // inteira. Ver a nota em `VERSAO_ESQUEMA`.
        Schema::do_disco(nome, colunas, indices)?
            .com_chaves_estrangeiras(fks)
            .map(|e| e.com_paginacao_do_disco(paginacao))
            .map(|e| e.com_motivo_obrigatorio(motivo_obrigatorio))
    }
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
        // 6 declaradas + softdeleted + rownum = 8, e o bitmap ainda cabe em 1.
        assert_eq!(s.colunas().len(), 8);
        assert_eq!(s.bitmap_len(), 1);
        assert_eq!(s.offset_coluna(0).unwrap(), 1);
        assert_eq!(s.offset_coluna(1).unwrap(), 9);
        assert_eq!(s.offset_coluna(2).unwrap(), 69);
        // 1 + 8 + 60 + 14 + 16 + 16 + 16 + 1 do softdeleted + 8 do rownum
        assert_eq!(s.payload_len(), 140);
    }

    /// A ordem das duas colunas de sistema e parte do formato: `rownum` entra
    /// DEPOIS de `softdeleted`, e nao antes. Trocar a ordem deslocaria o
    /// offset da softdeleted em toda tabela ja gravada na v4.
    #[test]
    fn as_colunas_de_sistema_saem_nesta_ordem() {
        let s = esquema_clientes();
        let n = s.colunas().len();
        assert_eq!(s.coluna_softdeleted(), Some(n - 2));
        assert_eq!(s.coluna_rownum(), Some(n - 1));
        assert_eq!(s.colunas()[n - 1].ty, ColumnType::UInt8);
        assert!(!s.colunas()[n - 1].nullable);
    }

    #[test]
    fn rownum_com_outro_tipo_e_recusada() {
        let mut cols = colunas_clientes();
        cols.push(Column::new(COLUNA_ROWNUM, ColumnType::Int4).obrigatoria());
        let e = Schema::new("t", cols, vec![]).unwrap_err();
        assert!(format!("{e}").contains("UInt8"), "{e}");
    }

    /// A coluna de sistema entra por ultimo, e so por ultimo: as colunas do
    /// usuario nao podem mudar de offset por causa dela.
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
        assert_eq!(i, com.colunas().len() - 2, "a softdeleted saiu do lugar");
        assert_eq!(com.colunas()[i].ty, ColumnType::Bool);
        assert!(!com.colunas()[i].nullable);
        assert!(sem.coluna_softdeleted().is_none());

        for j in 0..sem.colunas().len() {
            assert_eq!(
                com.offset_coluna(j).unwrap(),
                sem.offset_coluna(j).unwrap(),
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

        let bytes = s.serializar();
        let n = s.colunas().len();
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

    fn com(idx: IndexDef) -> Result<Schema> {
        Schema::new("docs", colunas(), vec![idx])
    }

    #[test]
    fn indice_de_texto_sobre_str_e_sobre_memo_e_aceito() {
        assert!(com(IndexDef::new("porTitulo", vec![IndexColumn::asc(1)]).de_texto()).is_ok());
        assert!(com(IndexDef::new("porCorpo", vec![IndexColumn::asc(2)]).de_texto()).is_ok());
    }

    /// A primeira das tres recusas na DECLARACAO. Indice de texto guarda as
    /// palavras de UMA coluna; duas colunas nao tem significado aqui.
    #[test]
    fn indice_de_texto_com_duas_colunas_recusa_na_declaracao() {
        let e =
            com(IndexDef::new("dois", vec![IndexColumn::asc(1), IndexColumn::asc(2)]).de_texto())
                .unwrap_err()
                .to_string();
        assert!(e.contains("UMA so"), "{e}");
    }

    /// A segunda: a mesma palavra aparece em muitas linhas, e e disso que a
    /// lista de ocorrencias e feita -- unico mataria o indice.
    #[test]
    fn indice_de_texto_unico_ou_primario_recusa_na_declaracao() {
        for idx in [
            IndexDef::new("u", vec![IndexColumn::asc(1)])
                .de_texto()
                .unico(),
            IndexDef::new("p", vec![IndexColumn::asc(1)])
                .de_texto()
                .primaria(),
        ] {
            let e = com(idx).unwrap_err().to_string();
            assert!(e.contains("unico ou primario"), "{e}");
        }
    }

    /// A terceira: indice de texto sobre numero nao indexa palavra nenhuma.
    #[test]
    fn indice_de_texto_sobre_numero_recusa_na_declaracao() {
        let e = com(IndexDef::new("n", vec![IndexColumn::asc(3)]).de_texto())
            .unwrap_err()
            .to_string();
        assert!(e.contains("Str ou Memo"), "{e}");
    }

    /// A dobra NASCE ligada, e desliga-la e escolha escrita.
    #[test]
    fn a_dobra_nasce_ligada_e_desligar_e_escrito() {
        let i = IndexDef::new("t", vec![IndexColumn::asc(1)]).de_texto();
        assert!(i.dobrar, "indice de texto tem de nascer dobrando");
        assert!(!i.clone().sem_dobrar().dobrar);
    }

    /// O PSCH v8 leva os dois bytes de ida e volta.
    #[test]
    fn o_esquema_volta_do_disco_com_o_indice_de_texto() {
        let e = Schema::new(
            "docs",
            colunas(),
            vec![
                IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
                IndexDef::new("porTitulo", vec![IndexColumn::asc(1)]).de_texto(),
                IndexDef::new("porCorpo", vec![IndexColumn::asc(2)])
                    .de_texto()
                    .sem_dobrar(),
            ],
        )
        .unwrap();
        let volta = Schema::desserializar(&e.serializar()).unwrap();
        assert_eq!(volta, e, "o esquema tem de voltar igual do disco");
        assert!(!volta.indices()[0].texto, "o comum nao pode virar de texto");
        assert!(volta.indices()[1].texto && volta.indices()[1].dobrar);
        assert!(volta.indices()[2].texto && !volta.indices()[2].dobrar);
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
