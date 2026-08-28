//! Esquema de uma tabela PhxSql: colunas, indices e o layout do slot.
//!
//! O esquema e serializado dentro do proprio `.reg`, logo apos o cabecalho.
//! Assim uma tabela e auto-descritiva: basta o quarteto de arquivos para
//! reabrir e ler os dados, sem dicionario externo.

use crate::error::{PhxError, Result};
use crate::keyenc::largura_componente;
use crate::paginacao::{ModoParticao, Paginacao};
use crate::types::ColumnType;
use crate::uuid::Uuid;

const MAGIC_ESQUEMA: &[u8; 4] = b"PSCH";
/// Versao do bloco de esquema gravado no `.reg`.
///
/// A 3 acrescentou os metadados de coluna (`id`, `caption`, `descricao`,
/// `mascara`), o marcador de chave primaria no indice e o modo de particao.
/// A leitura ainda aceita a 2: tabela gravada antes abre, ganha um `id` v7
/// sorteado na hora e os textos vazios. Escrever, so na 3.
const VERSAO_ESQUEMA: u16 = 3;
const VERSAO_ESQUEMA_MINIMA: u16 = 2;

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
    pub ao_excluir: AcaoRi,
    pub ao_alterar: AcaoRi,
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
            ao_alterar: AcaoRi::Restringir,
        }
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
        }
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
}

impl IndexDef {
    pub fn new(nome: impl Into<String>, colunas: Vec<IndexColumn>) -> Self {
        IndexDef {
            nome: nome.into(),
            colunas,
            unico: false,
            primario: false,
        }
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
}

impl Schema {
    pub fn new(
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
        })
    }

    /// Posicao da coluna `Sequence`, se a tabela tiver uma.
    pub fn coluna_sequencia(&self) -> Option<usize> {
        self.colunas
            .iter()
            .position(|c| c.ty == ColumnType::Sequence)
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
            });
        }

        let n_fk = leitor.u16()? as usize;
        let mut fks = Vec::with_capacity(n_fk);
        for _ in 0..n_fk {
            let nome_fk = leitor.texto()?;
            let tabela_ref = leitor.texto()?;
            let ao_excluir = AcaoRi::de_tag(leitor.u8()?)?;
            let ao_alterar = AcaoRi::de_tag(leitor.u8()?)?;
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

        Schema::new(nome, colunas, indices)?
            .com_chaves_estrangeiras(fks)
            .map(|e| e.com_paginacao_do_disco(paginacao))
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

    fn esquema_clientes() -> Schema {
        Schema::new(
            "cadastroClientes",
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
            ],
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
        // 6 colunas -> 1 byte de bitmap.
        assert_eq!(s.bitmap_len(), 1);
        assert_eq!(s.offset_coluna(0).unwrap(), 1);
        assert_eq!(s.offset_coluna(1).unwrap(), 9);
        assert_eq!(s.offset_coluna(2).unwrap(), 69);
        // 1 + 8 + 60 + 14 + 16 + 16 + 16
        assert_eq!(s.payload_len(), 131);
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
