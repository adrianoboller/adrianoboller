//! `Table` -- a tabela de dados, que e a soma dos seus arquivos.
//!
//! ```text
//! cadastroClientes.reg + .ndx + .bin + .memo + .log + .trash + .reason
//! ```
//!
//! Mais o espelho `.bkp`, quando ligado.
//!
//! Esta camada e quem traduz `Value` para bytes, decide o que vai inline no
//! `.reg` e o que vai para os arquivos externos, e mantem os indices em dia a
//! cada insercao, alteracao e exclusao.

use std::path::{Path, PathBuf};

use phxsql_core::datahora::civil_de_dias;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::keyenc::{escrever_componente, largura_componente};
use phxsql_core::schema::{ForeignKey, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::{escrever_inline, ler_inline, Ponteiro, Value};
use phxsql_core::{RowId, EXT_BIN, EXT_MEMO, EXT_NDX, EXT_REG};

use crate::blob::{BlobFile, MAGIC_BIN, MAGIC_MEMO};
use crate::lixeira::{Descartada, LixeiraFile, EXT_TRASH};
use crate::log::{Evento, LogFile, Operacao, EXT_LOG};
use crate::motivo::{Motivo, MotivoFile, Tipo, EXT_REASON};
use crate::ndx::NdxFile;
use crate::reg::RegFile;

/// Uma linha: um valor por coluna do esquema.
pub type Linha = Vec<Value>;

/// O que saiu de uma carga em lote.
#[derive(Debug, Clone, Default)]
pub struct Lote {
    /// Os rowids gravados, na ordem em que as linhas chegaram.
    pub rowids: Vec<RowId>,
    /// As que ficaram de fora: `(posicao na lista, motivo)`.
    ///
    /// A POSICAO, e nao o rowid: a linha recusada nao tem rowid, e quem mandou
    /// a carga precisa achar a linha no arquivo dele para consertar.
    pub recusadas: Vec<(usize, String)>,
}

/// O que uma varredura enxerga.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visao {
    /// So as linhas nao marcadas. E o que todo mundo ve.
    #[default]
    Ativas,
    /// So as marcadas como excluidas. A tela do administrador.
    Excluidas,
    /// Tudo que esta no `.reg`, marcado ou nao.
    Todas,
}

impl Visao {
    /// Esta linha entra nesta visao?
    pub fn aceita(self, excluida: bool) -> bool {
        match self {
            Visao::Ativas => !excluida,
            Visao::Excluidas => excluida,
            Visao::Todas => true,
        }
    }
}

/// O que sai de [`Table::abrir_imagem`]: o payload cru e, para cada coluna
/// externa, o conteudo dela -- e nao o ponteiro, que so vale na maquina de
/// origem.
pub type ImagemAberta = (Vec<u8>, Vec<(u16, Vec<u8>)>);

/// Como a pagina por posicao chegou ao inicio dela.
///
/// Sai na resposta do protocolo porque a diferenca entre os dois nao e de
/// estilo: num milhao de linhas sao vinte leituras contra um milhao de
/// passos. Quem esta montando uma tela grande precisa saber qual dos dois
/// esta pagando, e o que fazer com a tabela para pagar o outro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Salto {
    /// Busca binaria pelo `rownum`. O inicio da pagina custa `log2 N`.
    Bissecao,
    /// Andou ate a posicao, uma linha por vez. Sempre certo, sempre caro.
    Passo,
}

impl Salto {
    pub fn nome(self) -> &'static str {
        match self {
            Salto::Bissecao => "bisseccao",
            Salto::Passo => "passo",
        }
    }
}

/// Resultado de uma verificacao de integridade da tabela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relatorio {
    pub tabela: String,
    pub registros: u64,
    pub slots: u64,
    pub indices: Vec<(String, u64)>,
    pub blocos_bin: (u64, u64),
    pub blocos_memo: (u64, u64),
    /// Eventos conferidos no `.log`.
    pub eventos: u64,
    /// Linhas conferidas no `.trash`.
    pub descartadas: u64,
    /// Registros conferidos no `.reason`.
    pub motivos: u64,
    /// Linhas marcadas como excluidas, RECONTADAS -- e nao lidas do cabecalho.
    ///
    /// A conferencia existe justamente para nao acreditar em contador: se o
    /// numero do cabecalho tiver divergido, este e o caminho que descobre e
    /// conserta.
    pub marcadas: u64,
    /// Volumes de cada arquivo paginado: `.reg`, `.bin`, `.memo`, `.log`.
    pub volumes: (usize, usize, usize, usize),
}

pub struct Table {
    nome: String,
    diretorio: PathBuf,
    /// Copia do esquema que mora no `.reg`. Fica aqui para nao ser clonada a
    /// cada linha lida ou gravada.
    esquema: Schema,
    reg: RegFile,
    ndx: NdxFile,
    bin: BlobFile,
    memo: BlobFile,
    log: LogFile,
    lixeira: LixeiraFile,
    motivos: MotivoFile,
    /// Gravar a imagem da linha no diario? Ver [`Table::com_imagem_no_diario`].
    imagem_no_diario: bool,
    /// Gravar a imagem TAMBEM na exclusao fisica?
    ///
    /// A exclusao classica vai sem imagem porque o rowid basta -- entre
    /// servidores que replicam pelo rowid, ele e a identidade. No bidirecional
    /// a identidade e a CHAVE, e a chave mora dentro da imagem: sem ela o
    /// outro lado recebe "excluiu o rowid 42" e nao sabe QUAL linha e essa la.
    imagem_na_exclusao: bool,
    /// Carimbo e origem que o PROXIMO evento do diario deve levar, uma vez so.
    ///
    /// E o gancho do bidirecional: um evento aplicado aqui guarda o instante e
    /// o servidor em que a escrita NASCEU, e nao o relogio local da chegada --
    /// senao o conflito "mais recente vence" elegeria sempre quem sincronizou
    /// por ultimo. `None` = escrita local, relogio local, origem zero.
    evento_forcado: Option<(i64, u16)>,
}

fn caminho(diretorio: &Path, nome: &str, ext: &str) -> PathBuf {
    diretorio.join(format!("{nome}.{ext}"))
}

impl Table {
    /// Cria as quatro pecas da tabela em `diretorio`.
    ///
    /// Falha se qualquer um dos quatro arquivos ja existir, para nunca
    /// sobrescrever dados por engano.
    pub fn criar(diretorio: impl AsRef<Path>, esquema: Schema) -> Result<Table> {
        let diretorio = diretorio.as_ref().to_path_buf();
        std::fs::create_dir_all(&diretorio)?;
        let nome = esquema.nome().to_string();

        let paginacao = esquema.paginacao();
        for ext in [
            EXT_REG, EXT_NDX, EXT_BIN, EXT_MEMO, EXT_LOG, EXT_TRASH, EXT_REASON,
        ] {
            for c in [
                caminho(&diretorio, &nome, ext),
                diretorio.join(format!("{nome}{}.{ext}", paginacao.sufixo(1))),
            ] {
                if c.exists() {
                    return Err(PhxError::Esquema(format!(
                        "{} ja existe; use Table::abrir",
                        c.display()
                    )));
                }
            }
        }

        let ndx = NdxFile::criar(caminho(&diretorio, &nome, EXT_NDX), &esquema)?;
        // Os arquivos que NAO se partem por letra levam o sufixo numerico.
        // Ver `Paginacao::para_externos`: um `Clientes_B.log` se leria como o
        // diario do balde B, e o diario e da tabela inteira.
        let externos = paginacao.para_externos();
        let bin = BlobFile::criar(&diretorio, &nome, EXT_BIN, MAGIC_BIN, externos)?;
        let memo = BlobFile::criar(&diretorio, &nome, EXT_MEMO, MAGIC_MEMO, externos)?;
        let log = LogFile::criar(&diretorio, &nome, externos)?;
        let lixeira = LixeiraFile::criar(&diretorio, &nome, externos)?;
        let motivos = MotivoFile::criar(&diretorio, &nome, externos)?;
        let reg = RegFile::criar(&diretorio, &nome, esquema.clone())?;

        let mut t = Table {
            nome,
            diretorio,
            esquema,
            reg,
            ndx,
            bin,
            memo,
            log,
            lixeira,
            motivos,
            imagem_no_diario: false,
            imagem_na_exclusao: false,
            evento_forcado: None,
        };
        t.gravar_pag()?;
        Ok(t)
    }

    /// Abre uma tabela existente. O esquema vem de dentro do proprio `.reg`.
    /// Abre com o espelho `.bkp` ligado -- a segunda chance do `.reg`.
    pub fn abrir_espelhada(diretorio: impl AsRef<Path>, nome: &str) -> Result<Table> {
        let mut t = Table::abrir(diretorio, nome)?;
        t.reg.espelhar()?;
        Ok(t)
    }

    /// Cria com o espelho ligado desde o primeiro registro.
    pub fn criar_espelhada(diretorio: impl AsRef<Path>, esquema: Schema) -> Result<Table> {
        let mut t = Table::criar(diretorio, esquema)?;
        t.reg.espelhar()?;
        Ok(t)
    }

    /// Liga o espelho numa tabela ja aberta.
    pub fn espelhar(&mut self) -> Result<()> {
        self.reg.espelhar()
    }

    /// Leituras que o espelho salvou nesta sessao. Zero e o esperado.
    pub fn recuperados(&self) -> u64 {
        self.reg.recuperados()
    }

    pub fn tem_espelho(&self) -> bool {
        self.reg.tem_espelho()
    }

    /// Confere os dois lados e conserta o que der. Ver `RegFile::reparar`.
    ///
    /// Reconta as marcadas no fim: o reparo pode ter trazido de volta um slot
    /// que estava ilegivel, e o contador do cabecalho nao sabia dele.
    pub fn reparar(&mut self) -> Result<(u64, u64, u64)> {
        let r = self.reg.reparar()?;
        self.recontar_marcadas()?;
        Ok(r)
    }

    /// Redeclara as chaves estrangeiras da tabela -- e SO a declaracao.
    ///
    /// Nao mexe em linha, indice nem tipo de coluna: a chave estrangeira do
    /// PhxSql e catalogo (o `esquema` a devolve, o diagrama a desenha, o
    /// gerador de JOIN a le), e o motor nao a impoe na gravacao -- ha teste
    /// que trava isso. O que acontece no disco esta em
    /// [`RegFile::redeclarar_chaves_estrangeiras`]. Devolve `true` quando o
    /// `.reg` precisou ser reescrito para o bloco de esquema maior caber.
    pub fn redeclarar_chaves_estrangeiras(&mut self, fks: Vec<ForeignKey>) -> Result<bool> {
        let moveu = self.reg.redeclarar_chaves_estrangeiras(fks)?;
        self.esquema = self.reg.esquema().clone();
        // O `.pag` descreve o esquema para quem le o diretorio sem abrir a
        // tabela; desatualizado, ele viraria uma segunda verdade.
        self.gravar_pag()?;
        Ok(moveu)
    }

    pub fn abrir(diretorio: impl AsRef<Path>, nome: &str) -> Result<Table> {
        let diretorio = diretorio.as_ref().to_path_buf();
        let reg = RegFile::abrir(&diretorio, nome)?;
        let paginacao = reg.esquema().paginacao();
        let ndx = NdxFile::abrir(caminho(&diretorio, nome, EXT_NDX))?;
        let externos = paginacao.para_externos();
        let bin = BlobFile::abrir(&diretorio, nome, EXT_BIN, MAGIC_BIN, externos)?;
        let memo = BlobFile::abrir(&diretorio, nome, EXT_MEMO, MAGIC_MEMO, externos)?;
        let log = LogFile::abrir(&diretorio, nome, externos)?;
        // `abrir` destes dois CRIA quando falta: tabela feita antes deles
        // existirem tem de continuar abrindo.
        let lixeira = LixeiraFile::abrir(&diretorio, nome, externos)?;
        let motivos = MotivoFile::abrir(&diretorio, nome, externos)?;

        if ndx.indices().len() != reg.esquema().indices().len() {
            return Err(PhxError::Corrompido(format!(
                "{nome}: .ndx tem {} indices, o esquema do .reg declara {}",
                ndx.indices().len(),
                reg.esquema().indices().len()
            )));
        }

        let esquema = reg.esquema().clone();
        Ok(Table {
            nome: nome.to_string(),
            diretorio,
            esquema,
            reg,
            ndx,
            bin,
            memo,
            log,
            lixeira,
            motivos,
            imagem_no_diario: false,
            imagem_na_exclusao: false,
            evento_forcado: None,
        })
    }

    /// Liga a imagem da linha no diario. Ver o modulo `log`.
    ///
    /// Desligado por padrao porque custa: um registro de 200 bytes gasta ~244
    /// bytes de diario por alteracao em vez de 36. Quem so quer auditoria nao
    /// paga isso; quem replica precisa dele, porque sem a imagem o evento diz
    /// que o rowid mudou e nao diz para que.
    pub fn com_imagem_no_diario(mut self, ligado: bool) -> Table {
        self.imagem_no_diario = ligado;
        self
    }

    /// O mesmo, sem consumir a tabela -- para quem ja a tem aberta.
    pub fn ligar_imagem_no_diario(&mut self, ligado: bool) {
        self.imagem_no_diario = ligado;
    }

    pub fn imagem_no_diario(&self) -> bool {
        self.imagem_no_diario
    }

    /// Liga a imagem da linha tambem no evento de EXCLUSAO fisica.
    ///
    /// So faz efeito com [`Table::ligar_imagem_no_diario`] ligada tambem. E o
    /// que o papel bidirecional exige: la a identidade entre servidores e a
    /// chave, e a chave mora dentro da imagem.
    pub fn ligar_imagem_na_exclusao(&mut self, ligado: bool) {
        self.imagem_na_exclusao = ligado;
    }

    /// O PROXIMO evento do diario leva este carimbo e esta origem, uma vez so.
    ///
    /// Ver o campo `evento_forcado`. Chamar antes de `inserir`, `atualizar` ou
    /// `excluir_de_vez` quando a operacao esta APLICANDO uma escrita que
    /// nasceu em outro servidor.
    pub fn forcar_proximo_evento(&mut self, carimbo: i64, origem: u16) {
        self.evento_forcado = Some((carimbo, origem));
    }

    pub fn nome(&self) -> &str {
        &self.nome
    }

    pub fn diretorio(&self) -> &Path {
        &self.diretorio
    }

    pub fn esquema(&self) -> &Schema {
        &self.esquema
    }

    pub fn registros(&self) -> u64 {
        self.reg.registros()
    }

    /// Maior rowid ja atribuido, incluindo os excluidos.
    pub fn slots(&self) -> u64 {
        self.reg.slots()
    }

    // ------------------------------------------------------- codificacao

    /// Quantas colunas de sistema estao no FIM da lista, seguidas.
    ///
    /// Conta do fim para tras e para na primeira coluna do usuario: e o que
    /// permite a linha chegar sem elas. Uma coluna de sistema que estivesse no
    /// meio nao entraria nesta conta -- e nao esta, por construcao: elas
    /// entram sempre no fim, e ha teste que trava a ordem.
    fn colunas_de_sistema_no_fim(&self) -> usize {
        self.esquema
            .colunas()
            .iter()
            .rev()
            .take_while(|c| phxsql_core::schema::e_coluna_de_sistema(&c.nome))
            .count()
    }

    fn conferir_aridade(&self, valores: &[Value]) -> Result<()> {
        let n = self.esquema().colunas().len();
        // As colunas de sistema podem vir ou nao. Quem monta a linha declarou
        // as colunas dele e nao tem por que saber das do motor -- e um cliente
        // escrito antes de elas existirem continua funcionando. Ver `completar`.
        let minimo = n - self.colunas_de_sistema_no_fim();
        if valores.len() < minimo || valores.len() > n {
            return Err(PhxError::Tipo(format!(
                "{}: esperado {n} valores{}, recebido {}",
                self.nome,
                if minimo < n {
                    format!(" (ou {minimo}, sem as colunas do motor)")
                } else {
                    String::new()
                },
                valores.len()
            )));
        }
        Ok(())
    }

    /// Completa as colunas de sistema que quem chamou nao mandou.
    ///
    /// `None` quando nao ha nada a fazer. Aceita a linha faltando UMA ou as
    /// DUAS colunas do fim: quem monta a linha declarou as colunas dele e nao
    /// tem por que saber das do motor.
    ///
    /// Numa alteracao o valor herdado e o que a linha JA TINHA -- nas duas. Um
    /// `atualizar` comum nao pode ressuscitar linha marcada nem renumerar a
    /// ordem de chegada por distracao de quem montou os valores.
    fn completar(&self, valores: &[Value], anterior: Option<&Linha>) -> Option<Vec<Value>> {
        let n = self.esquema.colunas().len();
        if valores.len() >= n {
            return None;
        }
        let mut novos = valores.to_vec();
        for i in valores.len()..n {
            let c = &self.esquema.colunas()[i];
            if c.nome != phxsql_core::schema::COLUNA_SOFTDELETED
                && c.nome != phxsql_core::schema::COLUNA_ROWNUM
            {
                // A linha esta curta por outro motivo que nao as colunas de
                // sistema. Deixa a aridade reclamar, com a mensagem dela.
                return None;
            }
            novos.push(match anterior {
                Some(linha) => linha[i].clone(),
                // Zero e o "ainda nao numerado": `numerar_linha` troca por um
                // numero de verdade antes de a linha ir para o disco.
                None if c.nome == phxsql_core::schema::COLUNA_ROWNUM => Value::UInt(0),
                None => Value::Bool(false),
            });
        }
        Some(novos)
    }

    /// Poe o proximo `rownum` na linha, se ela ainda nao tiver um.
    ///
    /// Quem chama nao escolhe o numero: `rownum` e ordem de chegada, e um
    /// valor escolhido a mao seria uma ordem inventada. Valor diferente de
    /// zero que chegue de fora e ignorado -- e o caso de uma linha remontada
    /// por um cliente antigo que devolveu tudo que recebeu.
    fn numerar_linha(&mut self, valores: &mut [Value], anterior: Option<&Linha>) {
        let Some(i) = self.esquema.coluna_rownum() else {
            return;
        };
        if let Some(linha) = anterior {
            // Alteracao: mantem o numero que a linha ja tinha.
            if let Value::UInt(n) = linha[i] {
                if n > 0 {
                    valores[i] = Value::UInt(n);
                    return;
                }
            }
        }
        if !matches!(valores[i], Value::UInt(n) if n > 0) || anterior.is_none() {
            valores[i] = Value::UInt(self.reg.proximo_do_rownum());
        }
    }

    /// Proximo `rownum` que a tabela vai entregar.
    pub fn rownum_atual(&self) -> u64 {
        self.reg.rownum_atual()
    }

    /// O `rownum` desta linha, lido direto do payload -- sem decodificar nada.
    fn rownum_do_payload(&self, payload: &[u8]) -> Result<u64> {
        let Some(i) = self.esquema.coluna_rownum() else {
            return Ok(0);
        };
        let off = self.esquema.offset_coluna(i)?;
        Ok(u64::from_le_bytes(
            payload[off..off + 8]
                .try_into()
                .map_err(|_| PhxError::Corrompido("payload curto demais para o rownum".into()))?,
        ))
    }

    /// A linha esta marcada como excluida?
    ///
    /// Falso numa tabela sem a coluna de sistema -- ali nenhuma linha esta
    /// marcada, porque nao ha onde marcar.
    pub fn esta_excluida(&self, linha: &[Value]) -> bool {
        match self.esquema.coluna_softdeleted() {
            Some(i) => matches!(linha.get(i), Some(Value::Bool(true))),
            None => false,
        }
    }

    /// Posicao da coluna de sistema, ou o erro que explica por que nao ha.
    fn exigir_softdeleted(&self) -> Result<usize> {
        self.esquema.coluna_softdeleted().ok_or_else(|| {
            PhxError::Esquema(format!(
                "a tabela {} foi criada antes da coluna {} existir e nao tem \
                 exclusao suave; recrie a tabela para ganhar a coluna",
                self.nome,
                phxsql_core::schema::COLUNA_SOFTDELETED
            ))
        })
    }

    /// Resolve a coluna `Sequence`, se houver uma.
    ///
    /// Devolve `None` quando nao ha nada a mudar, para o caminho comum nao
    /// pagar uma copia da linha inteira.
    ///
    /// Duas regras, e a segunda e a que evita o estrago: valor nulo ganha o
    /// proximo numero do contador; valor escolhido a mao EMPURRA o contador
    /// para depois dele. Sem a segunda, gravar a sequencia 500 na mao e
    /// deixar o motor numerar em seguida devolveria 1, 2, 3 -- por cima do
    /// que ja existe.
    ///
    /// Numa alteracao (`anterior` presente) o nulo nao gera numero novo: ele
    /// mantem o que a linha ja tinha. A sequencia identifica a linha, e
    /// renumerar no meio do caminho seria trocar a identidade dela.
    fn numerar(
        &mut self,
        valores: &[Value],
        anterior: Option<&Linha>,
    ) -> Result<Option<Vec<Value>>> {
        let Some(i) = self.esquema.coluna_sequencia() else {
            return Ok(None);
        };
        match &valores[i] {
            Value::Null => {
                let mut novos = valores.to_vec();
                novos[i] = match anterior {
                    Some(linha) => linha[i].clone(),
                    None => Value::UInt(self.reg.proxima_da_sequencia()),
                };
                Ok(Some(novos))
            }
            Value::UInt(n) => {
                self.reg.anotar_sequencia(*n);
                Ok(None)
            }
            Value::Int(n) if *n >= 0 => {
                self.reg.anotar_sequencia(*n as u64);
                Ok(None)
            }
            outro => Err(PhxError::Tipo(format!(
                "coluna de sequencia espera numero inteiro, recebeu {outro:?}"
            ))),
        }
    }

    /// Proximo numero que a sequencia da tabela vai entregar. 0 = nunca usada.
    pub fn sequencia_atual(&self) -> u64 {
        self.reg.sequencia_atual()
    }

    /// Monta o payload do `.reg`, gravando antes o que vai para `.bin`/`.memo`.
    fn montar_payload(&mut self, valores: &[Value]) -> Result<Vec<u8>> {
        let mut payload = vec![0u8; self.esquema.payload_len()];

        for i in 0..self.esquema.colunas().len() {
            let col = &self.esquema.colunas()[i];
            let valor = &valores[i];
            if valor.e_null() {
                if !col.nullable {
                    return Err(PhxError::Tipo(format!(
                        "coluna {} e obrigatoria e recebeu NULL",
                        col.nome
                    )));
                }
                payload[i / 8] |= 1 << (i % 8);
                continue;
            }
            let off = self.esquema.offset_coluna(i)?;
            let fim = off + col.ty.largura();
            let ty = col.ty;
            let nome_col = col.nome.clone();
            match ty {
                ColumnType::Bin => {
                    let dados = match valor {
                        Value::Bin(b) => b.clone(),
                        outro => {
                            return Err(PhxError::Tipo(format!(
                                "coluna {nome_col} espera Bin, recebeu {outro:?}"
                            )))
                        }
                    };
                    let dados = self.reg.selar_externo(i as u16, &dados);
                    let p = self.bin.gravar(&dados)?;
                    p.escrever(&mut payload[off..fim])?;
                }
                ColumnType::Memo => {
                    let texto = match valor {
                        Value::Memo(s) | Value::Str(s) => s.clone(),
                        outro => {
                            return Err(PhxError::Tipo(format!(
                                "coluna {nome_col} espera Memo, recebeu {outro:?}"
                            )))
                        }
                    };
                    let bytes = self.reg.selar_externo(i as u16, texto.as_bytes());
                    let p = self.memo.gravar(&bytes)?;
                    p.escrever(&mut payload[off..fim])?;
                }
                _ => escrever_inline(valor, &ty, &mut payload[off..fim])?,
            }
        }
        Ok(payload)
    }

    /// Le o payload de volta. Se `carregar_externos` for falso, colunas
    /// `Bin`/`Memo` voltam como `Value::Null` -- util quando so precisamos
    /// dos valores que entram em indice.
    fn decodificar(&mut self, payload: &[u8], carregar_externos: bool) -> Result<Linha> {
        let mut linha = Vec::with_capacity(self.esquema.colunas().len());

        for i in 0..self.esquema.colunas().len() {
            if payload[i / 8] & (1 << (i % 8)) != 0 {
                linha.push(Value::Null);
                continue;
            }
            let ty = self.esquema.colunas()[i].ty;
            let off = self.esquema.offset_coluna(i)?;
            let fim = off + ty.largura();
            let valor = match ty {
                ColumnType::Bin => {
                    if !carregar_externos {
                        Value::Null
                    } else {
                        let p = Ponteiro::ler(&payload[off..fim])?;
                        let bytes = self.bin.ler(&p)?;
                        Value::Bin(self.reg.abrir_externo(i as u16, &bytes)?)
                    }
                }
                ColumnType::Memo => {
                    if !carregar_externos {
                        Value::Null
                    } else {
                        let p = Ponteiro::ler(&payload[off..fim])?;
                        let bytes = self.memo.ler(&p)?;
                        let bytes = self.reg.abrir_externo(i as u16, &bytes)?;
                        Value::Memo(String::from_utf8(bytes).map_err(|e| {
                            PhxError::Corrompido(format!("memo nao e UTF-8 valido: {e}"))
                        })?)
                    }
                }
                _ => ler_inline(&ty, &payload[off..fim])?,
            };
            linha.push(valor);
        }
        Ok(linha)
    }

    /// Ponteiros externos guardados num payload, para poder liberar depois.
    fn ponteiros(&self, payload: &[u8]) -> Result<Vec<(ColumnType, Ponteiro)>> {
        let esquema = &self.esquema;
        let mut saida = Vec::new();
        for (i, col) in esquema.colunas().iter().enumerate() {
            if !col.ty.externo() || payload[i / 8] & (1 << (i % 8)) != 0 {
                continue;
            }
            let off = esquema.offset_coluna(i)?;
            saida.push((
                col.ty,
                Ponteiro::ler(&payload[off..off + col.ty.largura()])?,
            ));
        }
        Ok(saida)
    }

    fn liberar_externos(&mut self, ponteiros: &[(ColumnType, Ponteiro)]) -> Result<()> {
        for (ty, p) in ponteiros {
            match ty {
                ColumnType::Bin => self.bin.liberar(p)?,
                ColumnType::Memo => self.memo.liberar(p)?,
                _ => {}
            }
        }
        Ok(())
    }

    /// Codifica a chave do indice `idx` a partir dos valores da linha.
    fn codificar_chave(&self, idx: usize, valores: &[Value]) -> Result<Vec<u8>> {
        let esquema = &self.esquema;
        let def = &esquema.indices()[idx];
        let mut chave = Vec::new();
        for ic in &def.colunas {
            let col = &esquema.colunas()[ic.coluna];
            let n = largura_componente(&col.ty)?;
            let base = chave.len();
            chave.resize(base + n, 0);
            escrever_componente(
                &valores[ic.coluna],
                &col.ty,
                ic.desc,
                ic.nocase,
                &mut chave[base..base + n],
            )?;
        }
        Ok(chave)
    }

    fn todas_as_chaves(&self, valores: &[Value]) -> Result<Vec<Vec<u8>>> {
        (0..self.esquema.indices().len())
            .map(|i| self.codificar_chave(i, valores))
            .collect()
    }

    // ------------------------------------------------------------ escrita

    /// Insere uma linha e devolve o rowid.
    ///
    /// A checagem de indice unico acontece ANTES de tocar no `.reg`; se um
    /// indice falhar no meio do caminho, o que ja foi gravado e desfeito.
    /// Em que balde esta linha cai, quando a particao e alfanumerica.
    ///
    /// `None` nos outros modos. O valor da coluna de referencia vira texto
    /// pela mesma funcao que o `.reason` usa -- entao numero tambem particiona,
    /// e o `12345` cai no balde `_1`.
    fn balde_da_linha(&self, valores: &[Value]) -> Result<Option<u32>> {
        let modo = self.esquema.paginacao().modo;
        if !modo.por_letra() {
            return Ok(None);
        }
        let Some(i) = modo.coluna() else {
            return Err(PhxError::Esquema(
                "particao alfanumerica sem coluna de referencia".into(),
            ));
        };
        let texto = valores.get(i).map(|v| v.para_texto()).unwrap_or_default();
        Ok(Some(phxsql_core::paginacao::balde_de(&texto)))
    }

    /// Quantas linhas cada balde tem. Vazio fora da particao alfanumerica.
    pub fn baldes(&self) -> &[u64] {
        self.reg.baldes()
    }

    /// Regrava o `.pag`, o descritor de particao da tabela.
    ///
    /// Gerado, e nunca lido pelo motor: a verdade continua no bloco de esquema
    /// do `.reg` e nos cabecalhos dos volumes. Ver [`crate::pag`].
    pub fn gravar_pag(&mut self) -> Result<std::path::PathBuf> {
        let volumes = self.reg.volumes();
        crate::pag::escrever(
            &self.diretorio,
            &self.nome,
            &self.esquema,
            self.reg.baldes(),
            &volumes,
        )
    }

    /// Em que periodo esta linha cai, quando a tabela e particionada por data.
    ///
    /// `None` na particao por quantidade -- ali o volume sai de divisao e a
    /// data nao tem nada a ver com o assunto.
    fn chave_do_periodo(&self, valores: &[Value]) -> Result<Option<i64>> {
        let modo = self.esquema.paginacao().modo;
        let (Some(periodo), Some(i)) = (modo.periodo(), modo.coluna()) else {
            return Ok(None);
        };
        let dias = match valores.get(i) {
            Some(Value::Date(d)) => *d,
            // DateTime e milissegundos; vira dia por divisao inteira, com
            // `div_euclid` para que datas antes de 1970 nao arredondem para o
            // lado errado.
            Some(Value::DateTime(ms)) => (ms.div_euclid(86_400_000)) as i32,
            outro => {
                return Err(PhxError::Tipo(format!(
                    "a coluna de particao {} precisa de uma data; recebi {outro:?}",
                    self.esquema.colunas()[i].nome
                )))
            }
        };
        let (ano, mes, _) = civil_de_dias(dias);
        Ok(Some(periodo.chave(ano, mes)))
    }

    /// As fronteiras de volume do `.reg`. Vazio na particao por quantidade,
    /// onde o volume sai de divisao e nao ha tabela nenhuma.
    /// Ajusta o contador da sequencia. Ver `RegFile::ajustar_sequencia`.
    pub fn ajustar_sequencia(&mut self, proxima: u64) -> Result<()> {
        self.reg.ajustar_sequencia(proxima)
    }

    pub fn fronteiras(&self) -> &[crate::reg::Fronteira] {
        self.reg.fronteiras()
    }

    pub fn inserir(&mut self, valores: &[Value]) -> Result<RowId> {
        self.conferir_aridade(valores)?;
        // Numerar ANTES das chaves, pela mesma razao da sequencia: se a coluna
        // estiver num indice, a chave tem de ser a do numero gravado.
        let mut completos = match self.completar(valores, None) {
            Some(v) => v,
            None => valores.to_vec(),
        };
        self.numerar_linha(&mut completos, None);
        let valores = &completos[..];

        // A sequencia entra ANTES das chaves: se a coluna estiver num indice,
        // a chave tem de ser a do numero que vai ser gravado, nao a do nulo.
        let proprios;
        let valores = match self.numerar(valores, None)? {
            Some(v) => {
                proprios = v;
                &proprios[..]
            }
            None => valores,
        };

        let chaves = self.todas_as_chaves(valores)?;

        // A conferencia acontece AQUI, antes de qualquer gravacao, e nao la
        // dentro do `ndx.inserir`, por um motivo de formato: o `.reg` nunca
        // reaproveita slot. Descobrir a duplicidade depois de gravar exigiria
        // desfazer, e o slot desfeito ficaria morto para sempre. Uma tabela que
        // recebe muita insercao repetida iria inchando sem nunca crescer.
        for (i, chave) in chaves.iter().enumerate() {
            if self.ndx.indices()[i].unico && self.ndx.existe(i, chave)? {
                return Err(PhxError::Duplicado(format!(
                    "indice unico {} ja tem essa chave",
                    self.ndx.indices()[i].nome
                )));
            }
        }

        let payload = self.montar_payload(valores)?;
        let ponteiros = self.ponteiros(&payload)?;
        // Linha que ja nasce marcada existe: a importacao traz o campo, e a
        // restauracao de uma lixeira tambem. O contador tem de saber.
        let nasce_marcada = self.marcada_no_payload(&payload)?;
        let rowid = match self.balde_da_linha(valores)? {
            Some(balde) => self.reg.inserir_no_balde(&payload, balde)?,
            None => self
                .reg
                .inserir_no_periodo(&payload, self.chave_do_periodo(valores)?)?,
        };

        for (i, chave) in chaves.iter().enumerate() {
            // `ja_conferido`: a unicidade foi conferida logo acima, antes de
            // qualquer gravacao. Deixar o `inserir` conferir de novo custaria
            // uma segunda descida na arvore para a mesma resposta.
            if let Err(e) = self.ndx.inserir_ja_conferido(i, chave, rowid) {
                // Desfaz o que ja entrou.
                for (j, anterior) in chaves.iter().enumerate().take(i) {
                    let _ = self.ndx.remover(j, anterior, rowid);
                }
                let _ = self.reg.excluir(rowid);
                let _ = self.liberar_externos(&ponteiros);
                return Err(e);
            }
        }
        if nasce_marcada {
            self.reg.mudar_marcadas(1)?;
        }
        self.anotar(Operacao::Inclusao, rowid, 1, &payload)?;
        Ok(rowid)
    }

    /// Grava o evento no diario, com a imagem da linha se estiver ligada.
    ///
    /// A imagem custa uma leitura de cada anexo da linha -- e o preco de a
    /// replica receber o conteudo em vez de um ponteiro que so vale aqui. Por
    /// isso ela esta atras do interruptor, e por isso este caminho existe em
    /// vez de a chamada ao `log` estar espalhada.
    fn anotar(
        &mut self,
        operacao: Operacao,
        rowid: RowId,
        versao: u64,
        payload: &[u8],
    ) -> Result<()> {
        // Consumido SEMPRE, mesmo sem imagem: um forcado que sobrasse para a
        // operacao seguinte carimbaria uma escrita local com relogio alheio.
        let (carimbo, origem) = match self.evento_forcado.take() {
            Some((c, o)) => (Some(c), o),
            None => (None, 0),
        };
        if !self.imagem_no_diario {
            self.log
                .registrar_detalhado(operacao, rowid, versao, &[], carimbo, origem)?;
            return Ok(());
        }
        let imagem = self.imagem_da_linha(payload)?;
        self.log
            .registrar_detalhado(operacao, rowid, versao, &imagem, carimbo, origem)?;
        Ok(())
    }

    /// Insere varias linhas de uma vez.
    ///
    /// # De onde vem o ganho
    ///
    /// **Nao e do disco.** Cada linha custa o mesmo aqui dentro: montar o
    /// payload, conferir a unicidade, gravar o slot, inserir a chave em cada
    /// indice. Nao ha atalho -- e a insercao ja e o caminho mais caro do
    /// motor, com 65% do tempo na manutencao do `.ndx`.
    ///
    /// O ganho e de tudo que ACONTECIA POR LINHA e passa a acontecer uma vez:
    /// abrir a tabela (sete arquivos), tomar a trava, e o `fsync`. Pela rede
    /// isso dominava -- vinte mil insercoes eram vinte mil aberturas.
    ///
    /// # Nao ha transacao, e isso muda o que se pode prometer
    ///
    /// Se a linha 700 de mil falhar, as 699 anteriores **ficam gravadas**. Nao
    /// ha como desfazer: o `.reg` nao reaproveita slot, entao "desfazer" seria
    /// deixar 699 buracos. Por isso o padrao e `parar_no_erro`: entre uma
    /// carga que para na linha 700 e uma que grava 999 linhas com uma faltando
    /// no meio, a primeira e a que da para consertar.
    ///
    /// Quem esta importando dado sujo de proposito passa `false` e recebe a
    /// lista do que ficou de fora, com o numero da linha.
    pub fn inserir_lote(&mut self, linhas: &[Linha], parar_no_erro: bool) -> Result<Lote> {
        let mut lote = Lote {
            rowids: Vec::with_capacity(linhas.len()),
            recusadas: Vec::new(),
        };
        for (i, linha) in linhas.iter().enumerate() {
            match self.inserir(linha) {
                Ok(r) => lote.rowids.push(r),
                Err(e) => {
                    lote.recusadas.push((i, e.to_string()));
                    if parar_no_erro {
                        return Ok(lote);
                    }
                }
            }
        }
        Ok(lote)
    }

    /// Le uma linha completa, carregando `.bin` e `.memo`.
    pub fn ler(&mut self, rowid: RowId) -> Result<Option<Linha>> {
        match self.reg.ler(rowid)? {
            None => Ok(None),
            Some(payload) => Ok(Some(self.decodificar(&payload, true)?)),
        }
    }

    /// A versao do registro: 1 quando nasce, +1 a cada regravacao.
    ///
    /// `None` quer dizer slot inativo -- nunca usado, ou excluido de vez.
    pub fn versao(&mut self, rowid: RowId) -> Result<Option<u64>> {
        self.reg.versao(rowid)
    }

    /// Confere se o registro ainda esta na versao que quem vai gravar leu.
    ///
    /// # A janela
    ///
    /// Entre ler uma linha na tela e clicar em «salvar» passam segundos ou
    /// minutos. Se outra sessao gravar nesse intervalo, o segundo `atualizar`
    /// simplesmente escreve por cima: o trabalho do primeiro some, sem erro,
    /// sem registro, sem ninguem perceber. Isto e o que o HFSQL(R) chama de
    /// conflito de escrita, e a resposta dele nao e travar a linha na leitura
    /// -- e AVISAR na gravacao, mostrando os tres valores para quem decide.
    ///
    /// A peca ja estava no formato desde a v1 do `.reg`: cada slot guarda uma
    /// versao que sobe a cada regravacao. Conferir custa 24 bytes de leitura.
    ///
    /// # Por que nao e trava
    ///
    /// Travar na leitura resolveria o mesmo problema e criaria dois piores: a
    /// linha fica presa quando o cliente cai com a ficha aberta, e duas
    /// sessoes que travam em ordem trocada se abracam. O contador nao trava
    /// nada -- so recusa a segunda gravacao quando ela chegou depois de
    /// alguem ter mudado a linha.
    ///
    /// Excluida de vez conta como conflito, e nao como "nao encontrado":
    /// quem leu a linha ha um minuto quer saber que ela foi apagada, e nao
    /// que o rowid nunca existiu.
    pub fn conferir_versao(&mut self, rowid: RowId, esperada: u64) -> Result<()> {
        match self.reg.versao(rowid)? {
            Some(atual) if atual == esperada => Ok(()),
            Some(atual) => Err(PhxError::Conflito(format!(
                "o registro {rowid} de {} esta na versao {atual} e voce leu a \
                 {esperada}: outra sessao gravou nesse meio-tempo",
                self.nome
            ))),
            None => Err(PhxError::Conflito(format!(
                "o registro {rowid} de {} foi excluido de vez depois que voce \
                 o leu na versao {esperada}",
                self.nome
            ))),
        }
    }

    /// `atualizar` que so grava se ninguem tiver mexido desde a leitura.
    ///
    /// A conferencia e a gravacao acontecem sem soltar o `&mut self`, entao
    /// nao ha janela entre uma e outra dentro do processo.
    pub fn atualizar_se(&mut self, rowid: RowId, valores: &[Value], esperada: u64) -> Result<()> {
        self.conferir_versao(rowid, esperada)?;
        self.atualizar(rowid, valores)
    }

    /// Regrava a linha inteira mantendo o mesmo rowid e a mesma posicao
    /// fisica no `.reg`.
    pub fn atualizar(&mut self, rowid: RowId, valores: &[Value]) -> Result<()> {
        self.conferir_aridade(valores)?;
        let antigo = self
            .reg
            .ler(rowid)?
            .ok_or_else(|| PhxError::NaoEncontrado(format!("registro {rowid} esta excluido")))?;

        let valores_antigos = self.decodificar(&antigo, false)?;

        // Sem a coluna de sistema nos valores, herda a marca da linha: um
        // `atualizar` de rotina nao ressuscita linha excluida por descuido.
        let mut completos = match self.completar(valores, Some(&valores_antigos)) {
            Some(v) => v,
            None => valores.to_vec(),
        };
        self.numerar_linha(&mut completos, Some(&valores_antigos));
        let valores = &completos[..];

        // Nulo na coluna de sequencia guarda o numero que a linha ja tinha.
        let proprios;
        let valores = match self.numerar(valores, Some(&valores_antigos))? {
            Some(v) => {
                proprios = v;
                &proprios[..]
            }
            None => valores,
        };

        let chaves_antigas = self.todas_as_chaves(&valores_antigos)?;
        let chaves_novas = self.todas_as_chaves(valores)?;

        // Unicidade: so reclama se a chave mudou e ja pertence a outro rowid.
        for (i, nova) in chaves_novas.iter().enumerate() {
            if !self.ndx.indices()[i].unico || *nova == chaves_antigas[i] {
                continue;
            }
            let donos = self.ndx.buscar(i, nova)?;
            if donos.iter().any(|&r| r != rowid) {
                return Err(PhxError::Duplicado(format!(
                    "indice unico {} ja tem essa chave",
                    self.ndx.indices()[i].nome
                )));
            }
        }

        // Na particao alfanumerica, o balde e o ENDERECO: mudar a coluna de
        // referencia de «Silva» para «Andrade» mudaria o arquivo em que a
        // linha mora, e com ele o rowid -- que e a identidade dela e esta em
        // todo indice. Mover nao e opcao; deixar a linha no balde errado
        // tambem nao, porque ai o `_S` deixa de conter os S e a particao para
        // de valer. Entao a alteracao e RECUSADA, com o caminho escrito.
        if let (Some(a), Some(b)) = (
            self.balde_da_linha(&valores_antigos)?,
            self.balde_da_linha(valores)?,
        ) {
            if a != b {
                let baldes = phxsql_core::paginacao::BALDES;
                return Err(PhxError::Esquema(format!(
                    "a alteracao mudaria o balde de {} para {}, e o balde e o \
                     endereco fisico da linha em {}. Exclua e insira de novo: \
                     a linha nova nasce no balde certo, com outro rowid",
                    baldes[a as usize - 1],
                    baldes[b as usize - 1],
                    self.nome
                )));
            }
        }

        let ponteiros_antigos = self.ponteiros(&antigo)?;
        let payload = self.montar_payload(valores)?;
        // `completar` herda a marca quando ela nao vem nos valores, mas quem
        // manda a coluna escrita pode virar o valor por aqui.
        let delta = i64::from(self.marcada_no_payload(&payload)?)
            - i64::from(self.marcada_no_payload(&antigo)?);
        let versao = self.reg.atualizar(rowid, &payload)?;
        if delta != 0 {
            self.reg.mudar_marcadas(delta)?;
        }

        for (i, (antiga, nova)) in chaves_antigas.iter().zip(chaves_novas.iter()).enumerate() {
            if antiga != nova {
                self.ndx.remover(i, antiga, rowid)?;
                self.ndx.inserir(i, nova, rowid)?;
            }
        }
        self.liberar_externos(&ponteiros_antigos)?;
        self.anotar(Operacao::Alteracao, rowid, versao, &payload)?;
        Ok(())
    }

    /// Exclui de vez: guarda a linha inteira no `.trash`, **espera o disco
    /// confirmar**, e so entao libera o slot do `.reg`.
    ///
    /// # A ordem
    ///
    /// Guardar depois de liberar teria uma janela em que a linha nao existe em
    /// lugar nenhum -- e uma queda dentro dela nao tem conserto. Guardar
    /// antes tem a janela oposta: a linha aparece nos dois lugares, o que se
    /// resolve olhando. Entre perder e duplicar, duplica.
    ///
    /// O `sincronizar` esta dentro de `LixeiraFile::guardar`, e nao aqui,
    /// porque a garantia e daquele arquivo: "esta na lixeira" com a pagina
    /// ainda suja na memoria nao e uma garantia.
    pub fn excluir_de_vez(&mut self, rowid: RowId, motivo: &str) -> Result<bool> {
        let payload = match self.reg.ler(rowid)? {
            None => return Ok(false),
            Some(p) => p,
        };
        self.conferir_motivo(motivo)?;

        // O conteudo dos externos entra na lixeira junto: os ponteiros do
        // payload apontam para blocos que esta mesma exclusao vai liberar.
        let externos = self.conteudo_externo(&payload)?;
        let identidade = self.identidade(&payload)?;
        // A imagem se monta AGORA, antes de os blocos externos serem
        // liberados: depois, os ponteiros do payload apontariam para o nada.
        let imagem_do_evento = if self.imagem_no_diario && self.imagem_na_exclusao {
            self.imagem_da_linha(&payload)?
        } else {
            Vec::new()
        };
        self.lixeira.guardar(rowid, &payload, externos)?;

        let valores = self.decodificar(&payload, false)?;
        let chaves = self.todas_as_chaves(&valores)?;
        for (i, chave) in chaves.iter().enumerate() {
            self.ndx.remover(i, chave, rowid)?;
        }
        let ponteiros = self.ponteiros(&payload)?;
        self.liberar_externos(&ponteiros)?;
        let estava_marcada = self.marcada_no_payload(&payload)?;
        let removeu = self.reg.excluir(rowid)?;
        if removeu {
            if estava_marcada {
                self.reg.mudar_marcadas(-1)?;
            }
            self.motivos
                .registrar(Tipo::Fisica, rowid, motivo, &identidade)?;
            let (carimbo, origem) = match self.evento_forcado.take() {
                Some((c, o)) => (Some(c), o),
                None => (None, 0),
            };
            self.log.registrar_detalhado(
                Operacao::Exclusao,
                rowid,
                0,
                &imagem_do_evento,
                carimbo,
                origem,
            )?;
        }
        Ok(removeu)
    }

    /// Marca a linha como excluida sem apagar nada.
    ///
    /// Devolve `false` quando o slot ja estava livre ou a linha ja estava
    /// marcada -- marcar duas vezes nao e erro, mas tambem nao gera um segundo
    /// motivo no `.reason`.
    pub fn excluir_suave(&mut self, rowid: RowId, motivo: &str) -> Result<bool> {
        self.exigir_softdeleted()?;
        self.conferir_motivo(motivo)?;
        if !self.marcar(rowid, true)? {
            return Ok(false);
        }
        let identidade = match self.reg.ler(rowid)? {
            Some(p) => self.identidade(&p)?,
            None => String::new(),
        };
        self.motivos
            .registrar(Tipo::Suave, rowid, motivo, &identidade)?;
        Ok(true)
    }

    /// Desfaz uma exclusao suave.
    pub fn restaurar(&mut self, rowid: RowId, motivo: &str) -> Result<bool> {
        self.exigir_softdeleted()?;
        if !self.marcar(rowid, false)? {
            return Ok(false);
        }
        let identidade = match self.reg.ler(rowid)? {
            Some(p) => self.identidade(&p)?,
            None => String::new(),
        };
        self.motivos
            .registrar(Tipo::Restauracao, rowid, motivo, &identidade)?;
        Ok(true)
    }

    /// Troca o valor da coluna de sistema sem reescrever os externos.
    ///
    /// Nao usa `atualizar` de proposito: aquele caminho decodifica a linha com
    /// os anexos, regrava cada um e libera os antigos -- marcar uma linha
    /// copiaria a foto dela de um bloco para outro sem nenhuma razao. Aqui o
    /// unico byte que muda e o da coluna, e os ponteiros ficam onde estao.
    fn marcar(&mut self, rowid: RowId, valor: bool) -> Result<bool> {
        let i = self.exigir_softdeleted()?;
        let Some(mut payload) = self.reg.ler(rowid)? else {
            return Ok(false);
        };
        let antes = self.decodificar(&payload, false)?;
        if matches!(antes[i], Value::Bool(v) if v == valor) {
            return Ok(false);
        }

        let off = self.esquema.offset_coluna(i)?;
        let fim = off + ColumnType::Bool.largura();
        let novo = Value::Bool(valor);
        escrever_inline(&novo, &ColumnType::Bool, &mut payload[off..fim])?;
        // A coluna e obrigatoria, mas a linha pode ter vindo de um caminho que
        // a deixou nula: limpa o bit de nulo junto, senao o valor gravado nao
        // seria lido de volta.
        payload[i / 8] &= !(1 << (i % 8));

        // A marca pode estar dentro de um indice -- e util que esteja, para
        // listar excluidas sem varrer. Entao as chaves mudam.
        let mut depois = antes.clone();
        depois[i] = novo;
        let chaves_antigas = self.todas_as_chaves(&antes)?;
        let chaves_novas = self.todas_as_chaves(&depois)?;

        let versao = self.reg.atualizar(rowid, &payload)?;
        self.reg.mudar_marcadas(if valor { 1 } else { -1 })?;
        for (j, (a, b)) in chaves_antigas.iter().zip(chaves_novas.iter()).enumerate() {
            if a != b {
                self.ndx.remover(j, a, rowid)?;
                self.ndx.inserir(j, b, rowid)?;
            }
        }
        // A marca vai para a replica como ALTERACAO, que e o que ela e no
        // `.reg`: o byte da coluna de sistema mudou e nada mais.
        self.anotar(Operacao::Alteracao, rowid, versao, &payload)?;
        Ok(true)
    }

    /// Recusa a exclusao sem motivo quando a tabela exige um.
    ///
    /// A escolha e da tabela, feita na criacao. Uma tabela de auditoria exige;
    /// uma tabela de rascunho nao, e obrigar ali so ensinaria todo mundo a
    /// digitar um ponto.
    fn conferir_motivo(&self, motivo: &str) -> Result<()> {
        if self.esquema.motivo_obrigatorio() && motivo.trim().is_empty() {
            return Err(PhxError::Esquema(format!(
                "a tabela {} exige motivo escrito para excluir",
                self.nome
            )));
        }
        Ok(())
    }

    /// Como esta linha se identifica, em texto, para o `.reason`.
    ///
    /// Na ordem: a chave primaria, senao a primeira coluna `Uuid`, senao a
    /// sequencia. Vazio quando a tabela nao tem nenhuma das tres -- e ai o
    /// rowid do proprio registro e tudo que se tem.
    fn identidade(&mut self, payload: &[u8]) -> Result<String> {
        let valores = self.decodificar(payload, false)?;
        let esquema = &self.esquema;
        if let Some(pk) = esquema.chave_primaria() {
            let partes: Vec<String> = pk
                .colunas
                .iter()
                .map(|ic| {
                    format!(
                        "{}={}",
                        esquema.colunas()[ic.coluna].nome,
                        valores[ic.coluna].para_texto()
                    )
                })
                .collect();
            return Ok(partes.join(", "));
        }
        for (i, c) in esquema.colunas().iter().enumerate() {
            if matches!(c.ty, ColumnType::Uuid | ColumnType::Sequence) {
                return Ok(format!("{}={}", c.nome, valores[i].para_texto()));
            }
        }
        Ok(String::new())
    }

    /// O conteudo de cada coluna externa da linha, para ir junto na lixeira.
    fn conteudo_externo(&mut self, payload: &[u8]) -> Result<Vec<(u16, Vec<u8>)>> {
        let mut saida = Vec::new();
        for i in 0..self.esquema.colunas().len() {
            let col = &self.esquema.colunas()[i];
            if !col.ty.externo() || payload[i / 8] & (1 << (i % 8)) != 0 {
                continue;
            }
            let ty = col.ty;
            let off = self.esquema.offset_coluna(i)?;
            let p = Ponteiro::ler(&payload[off..off + ty.largura()])?;
            // Vai como esta no bloco -- CIFRADO, quando a coluna e marcada.
            // Decifrar aqui poria o texto claro dentro da imagem do diario e
            // dentro da lixeira, que e exatamente o que a cifra da coluna
            // existe para impedir. Quem abre a imagem passa pelo
            // `abrir_externo` do outro lado.
            let bytes = match ty {
                ColumnType::Bin => self.bin.ler(&p)?,
                ColumnType::Memo => self.memo.ler(&p)?,
                _ => continue,
            };
            saida.push((i as u16, bytes));
        }
        Ok(saida)
    }

    // ------------------------------------------------------- replicacao

    /// A imagem da linha: os bytes que a replica precisa para reproduzi-la.
    ///
    /// ```text
    /// [tam_payload u32][payload]
    /// [qtd_externos u16][ (coluna u16, tamanho u32, conteudo) ... ]
    /// ```
    ///
    /// O payload vai CRU, do jeito que esta no `.reg` -- sem reencodar, sem
    /// passar por `Value`, sem perder precisao de decimal nem de data. E o
    /// conteudo dos externos vai junto porque os ponteiros do payload sao
    /// offsets do `.bin` e do `.memo` DAQUI: na outra maquina eles apontariam
    /// para qualquer coisa. E a mesma razao de o `.trash` guardar conteudo.
    pub fn imagem_da_linha(&mut self, payload: &[u8]) -> Result<Vec<u8>> {
        let externos = self.conteudo_externo(payload)?;
        let mut out = Vec::with_capacity(payload.len() + 64);
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        out.extend_from_slice(&(externos.len() as u16).to_le_bytes());
        for (coluna, bytes) in &externos {
            out.extend_from_slice(&coluna.to_le_bytes());
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        Ok(out)
    }

    /// A imagem da linha de um rowid, lendo o payload do `.reg`.
    pub fn imagem_da_linha_do_rowid(&mut self, rowid: RowId) -> Result<Vec<u8>> {
        let payload = self
            .reg
            .ler(rowid)?
            .ok_or_else(|| PhxError::NaoEncontrado(format!("registro {rowid} esta excluido")))?;
        self.imagem_da_linha(&payload)
    }

    /// Desmonta a imagem. Inversa exata de [`Table::imagem_da_linha`].
    pub fn abrir_imagem(imagem: &[u8]) -> Result<ImagemAberta> {
        let curta = || PhxError::Corrompido("imagem de linha truncada".into());
        let ler_u32 = |i: usize| -> Result<u32> {
            imagem
                .get(i..i + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or_else(curta)
        };
        let ler_u16 = |i: usize| -> Result<u16> {
            imagem
                .get(i..i + 2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .ok_or_else(curta)
        };

        let tam = ler_u32(0)? as usize;
        let payload = imagem.get(4..4 + tam).ok_or_else(curta)?.to_vec();
        let mut i = 4 + tam;
        let qtd = ler_u16(i)? as usize;
        i += 2;
        let mut externos = Vec::with_capacity(qtd);
        for _ in 0..qtd {
            let coluna = ler_u16(i)?;
            let n = ler_u32(i + 2)? as usize;
            i += 6;
            externos.push((coluna, imagem.get(i..i + n).ok_or_else(curta)?.to_vec()));
            i += n;
        }
        Ok((payload, externos))
    }

    /// Os valores da linha que uma imagem carrega, com os externos DELA.
    ///
    /// E o `abrir_imagem` mais a decodificacao, num passo so: o caminho que o
    /// bidirecional usa para ler a CHAVE e os campos de um evento que chegou
    /// de outro servidor, sem nunca seguir os ponteiros alheios do payload.
    pub fn valores_da_imagem(&mut self, imagem: &[u8]) -> Result<Vec<Value>> {
        let (payload, externos) = Table::abrir_imagem(imagem)?;
        self.decodificar_com_externos(&payload, &externos)
    }

    /// Aplica um evento vindo do source. **So faz sentido numa replica.**
    ///
    /// # O que ela confere, e por que para em vez de seguir
    ///
    /// O `.reg` nunca reaproveita slot e o rowid e sempre `slot_count + 1`.
    /// Entao, se a replica aplicar TODOS os eventos NA ORDEM e mais ninguem
    /// escrever nela, os rowids saem identicos aos do source -- sem transmitir
    /// nem negociar nada. Isso da uma conferencia forte e de graca: se o rowid
    /// que ela gerou nao bate com o do evento, ela JA divergiu, e continuar so
    /// espalharia a divergencia. E o mesmo comportamento da thread SQL do
    /// MySQL(R) parando num erro.
    ///
    /// Devolve o rowid aplicado.
    pub fn aplicar_evento(
        &mut self,
        operacao: Operacao,
        rowid: RowId,
        imagem: &[u8],
    ) -> Result<RowId> {
        match operacao {
            Operacao::Exclusao => {
                // A exclusao nao leva imagem: o rowid basta. E ela e FISICA,
                // porque foi fisica no source -- a suave chega como alteracao,
                // que e o que ela e no `.reg`.
                //
                // Nao ter o que excluir e divergencia, e nao um caso benigno:
                // numa replica fiel a linha existe, porque a inclusao dela
                // passou por aqui antes. E se nao para, o evento nao gera
                // evento local, a posicao nao anda, e a replicacao gira em
                // falso puxando o mesmo evento para sempre.
                if !self.excluir_de_vez(rowid, "replicacao")? {
                    return Err(PhxError::Corrompido(format!(
                        "replica divergiu em {}: o source excluiu o rowid {rowid} e \
                         aqui ele nao existe",
                        self.nome
                    )));
                }
                Ok(rowid)
            }
            Operacao::Inclusao | Operacao::Alteracao => {
                if imagem.is_empty() {
                    return Err(PhxError::Esquema(format!(
                        "evento de {} no rowid {rowid} veio sem imagem: o source gravou o diario com `imagem_da_linha` desligada",
                        operacao.nome()
                    )));
                }
                let (payload, externos) = Table::abrir_imagem(imagem)?;
                let valores = self.decodificar_com_externos(&payload, &externos)?;
                if operacao == Operacao::Inclusao {
                    let meu = self.inserir(&valores)?;
                    if meu != rowid {
                        return Err(PhxError::Corrompido(format!(
                            "replica divergiu em {}: o source diz rowid {rowid} e aqui saiu {meu}. A replicacao para aqui em vez de espalhar a divergencia",
                            self.nome
                        )));
                    }
                    Ok(meu)
                } else {
                    self.atualizar(rowid, &valores)?;
                    Ok(rowid)
                }
            }
        }
    }

    /// Decodifica um payload usando o conteudo externo da imagem no lugar do
    /// que os ponteiros dele apontariam.
    ///
    /// Os ponteiros do payload sao do OUTRO servidor. Ler por eles aqui daria
    /// bloco errado, ou erro, ou -- pior -- bloco de outra linha.
    fn decodificar_com_externos(
        &mut self,
        payload: &[u8],
        externos: &[(u16, Vec<u8>)],
    ) -> Result<Vec<Value>> {
        // Sem carregar externos: o que voltar nas colunas externas e ponteiro
        // alheio, e vai ser substituido logo abaixo.
        let mut valores = self.decodificar(payload, false)?;
        for i in 0..self.esquema.colunas().len() {
            let ty = self.esquema.colunas()[i].ty;
            if !ty.externo() {
                continue;
            }
            let nulo = payload[i / 8] & (1 << (i % 8)) != 0;
            valores[i] = match externos.iter().find(|(c, _)| *c as usize == i) {
                Some((_, bytes)) => {
                    // A imagem carrega o conteudo COMO ESTA no bloco, e numa
                    // coluna marcada isso e texto cifrado. Abrir aqui exige a
                    // chave -- e e por isso que replicar tabela com coluna
                    // cifrada so funciona entre servidores que dividem a
                    // senha. Esta escrito em SEGURANCA.md §10.
                    let bytes = self.reg.abrir_externo(i as u16, bytes)?;
                    match ty {
                        ColumnType::Bin => Value::Bin(bytes),
                        ColumnType::Memo => Value::Memo(
                            String::from_utf8(bytes)
                                .map_err(|_| PhxError::Corrompido("memo nao e UTF-8".into()))?,
                        ),
                        _ => Value::Null,
                    }
                }
                // Nao veio na imagem: ou a coluna e nula, ou o source nao a
                // mandou. Nulo e a leitura segura -- inventar bytes seria pior.
                None if nulo => Value::Null,
                None => Value::Null,
            };
        }
        Ok(valores)
    }

    // ------------------------------------------------------------ leitura

    /// Exclui de vez, sem motivo escrito. Recusa se a tabela exigir um.
    ///
    /// Continua sendo exclusao FISICA, como sempre foi -- o que mudou e que
    /// agora a linha passa pelo `.trash` antes de sair.
    pub fn excluir(&mut self, rowid: RowId) -> Result<bool> {
        self.excluir_de_vez(rowid, "")
    }

    /// Percorre a tabela na ORDEM DE DIGITACAO, direto do `.reg`.
    ///
    /// **Sem as linhas marcadas como excluidas.** Se elas continuassem
    /// aparecendo, marcar nao faria nada: a exclusao suave so vale se o
    /// caminho comum passar a nao enxergar a linha.
    pub fn varrer(&mut self) -> Result<Vec<(RowId, Linha)>> {
        self.varrer_com(Visao::Ativas)
    }

    /// Percorre escolhendo o que enxergar. Ver [`Visao`].
    pub fn varrer_com(&mut self, visao: Visao) -> Result<Vec<(RowId, Linha)>> {
        let mut saida = Vec::new();
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            let linha = self.decodificar(&payload, true)?;
            if visao.aceita(self.esta_excluida(&linha)) {
                saida.push((id, linha));
            }
            rowid = id + 1;
        }
        Ok(saida)
    }

    // ------------------------------------------------------------- paginas

    /// Uma pagina de rowids, sem decodificar linha nenhuma.
    ///
    /// # Por que isto existe separado da varredura
    ///
    /// `varrer_com` decodifica CADA linha da tabela -- com os anexos do `.bin`
    /// e do `.memo` -- e devolve tudo. Quem quer duzentas linhas de um milhao
    /// pagava um milhao de decodificacoes e um milhao de leituras de anexo,
    /// para jogar 999.800 fora. O custo crescia com a TABELA, e nao com a
    /// pagina, que e o defeito que o `LIMIT`/`OFFSET` de qualquer motor tem --
    /// so que aqui era pior, porque o `OFFSET` ao menos nao carrega o blob.
    ///
    /// Aqui a leitura para no teto, e nada e decodificado: para decidir se um
    /// slot entra basta o byte da coluna de sistema.
    ///
    /// `pular` continua existindo porque tela pequena precisa dele, e porque
    /// nem toda ordenacao tem cursor. Mas ele e o modo de compatibilidade --
    /// quem tem tabela grande usa [`Table::pagina_depois_de`].
    pub fn pagina(&mut self, pular: u64, limite: u64, visao: Visao) -> Result<Vec<RowId>> {
        let mut saida = Vec::new();
        let mut vistos = 0u64;
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            rowid = id + 1;
            if !self.visao_aceita_payload(&payload, visao)? {
                continue;
            }
            if vistos >= pular {
                saida.push(id);
                if limite > 0 && saida.len() as u64 >= limite {
                    break;
                }
            }
            vistos += 1;
        }
        Ok(saida)
    }

    /// A pagina que vem DEPOIS do rowid `cursor`. O *keyset* do PhxSql.
    ///
    /// # Por que aqui ele sai de graca
    ///
    /// Num motor relacional, pular para o meio da tabela exige um indice: a
    /// ordem logica nao tem nada a ver com a posicao fisica. Aqui tem --
    /// `offset = data_offset + (rowid-1) x slot_size`. Continuar depois do
    /// rowid 500.000 nao e procurar: e uma conta.
    ///
    /// O custo e o da PAGINA, e nao o da tabela. E a diferenca entre uma tela
    /// que abre igual na pagina 1 e na pagina 10.000, e uma que vai ficando
    /// lenta conforme o usuario desce.
    ///
    /// Cursor zero comeca do inicio. A pagina nunca inclui o proprio cursor,
    /// para o cliente poder mandar de volta o ultimo rowid que recebeu sem
    /// receber a mesma linha duas vezes.
    pub fn pagina_depois_de(
        &mut self,
        cursor: RowId,
        limite: u64,
        visao: Visao,
    ) -> Result<Vec<RowId>> {
        let mut saida = Vec::new();
        let mut rowid = cursor.saturating_add(1);
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            rowid = id + 1;
            if !self.visao_aceita_payload(&payload, visao)? {
                continue;
            }
            saida.push(id);
            if limite > 0 && saida.len() as u64 >= limite {
                break;
            }
        }
        Ok(saida)
    }

    /// O rowid da primeira linha cujo `rownum` e >= `alvo`.
    ///
    /// # Por que isto e uma busca binaria, e nao uma varredura
    ///
    /// O `rownum` cresce com a ordem de chegada, e o `.reg` guarda as linhas
    /// na ordem de chegada. Entao o `rownum` **cresce com o rowid**, e uma
    /// sequencia crescente num arquivo de acesso aleatorio se procura por
    /// bisseccao: log2 de um milhao sao vinte leituras.
    ///
    /// Nao ha indice envolvido, e nao ha indice a manter. E o mesmo motivo de
    /// o endereco sair de uma conta: a ordem logica e a ordem fisica.
    ///
    /// Slot excluido nao tem `rownum` para comparar; a bisseccao anda para o
    /// vizinho vivo mais proximo, o que custa alguns passos a mais num trecho
    /// muito esburacado e nao muda a resposta.
    ///
    /// `None` quando nenhuma linha tem `rownum` >= alvo, ou quando a tabela
    /// nao tem a coluna.
    pub fn rowid_do_rownum(&mut self, alvo: u64) -> Result<Option<RowId>> {
        if self.esquema.coluna_rownum().is_none() {
            return Ok(None);
        }
        // Na particao alfanumerica o `rownum` NAO cresce com o rowid, e ai a
        // bisseccao nao vale: a Silva digitada primeiro mora no `_S`, com
        // rowid alto, e a Alves digitada depois mora no `_A`, com rowid 1 --
        // rownum 1 num rowid maior que o do rownum 2. Bissetar uma sequencia
        // que nao esta ordenada devolve resposta errada em silencio, que e
        // pior que devolver devagar. Ali se varre.
        if self.reg.paginacao().modo.por_letra() {
            return self.rowid_do_rownum_varrendo(alvo);
        }
        let (mut baixo, mut alto) = (1u64, self.reg.slots());
        if alto == 0 {
            return Ok(None);
        }
        let mut achado = None;
        while baixo <= alto {
            let meio = baixo + (alto - baixo) / 2;
            // Anda para a frente ate achar um slot vivo, sem passar do alto.
            let Some((id, payload)) = self.reg.proximo_ativo(meio)? else {
                // So ha buraco daqui para a frente: o alvo esta atras.
                if meio == 0 {
                    break;
                }
                alto = meio - 1;
                continue;
            };
            if id > alto {
                if meio == 0 {
                    break;
                }
                alto = meio - 1;
                continue;
            }
            if self.rownum_do_payload(&payload)? >= alvo {
                achado = Some(id);
                if id == 0 {
                    break;
                }
                alto = id - 1;
            } else {
                baixo = id + 1;
            }
        }
        Ok(achado)
    }

    /// A pagina que comeca no numero de ordem `alvo`, inclusive.
    ///
    /// E o cursor da tela quando quem pagina guarda o `rownum` e nao o rowid --
    /// que e o caso da particao alfanumerica, onde o rowid de volumes
    /// diferentes nao se compara.
    pub fn pagina_desde_rownum(
        &mut self,
        alvo: u64,
        limite: u64,
        visao: Visao,
    ) -> Result<Vec<RowId>> {
        let Some(inicio) = self.rowid_do_rownum(alvo)? else {
            return Ok(Vec::new());
        };
        // `depois_de` exclui o proprio cursor, e aqui o inicio ENTRA.
        self.pagina_depois_de(inicio.saturating_sub(1), limite, visao)
    }

    /// O mesmo que [`Table::rowid_do_rownum`], varrendo.
    ///
    /// Existe para a particao alfanumerica, onde a sequencia de `rownum` nao
    /// esta ordenada pelo rowid. Procura o MENOR `rownum` maior ou igual ao
    /// alvo -- e nao o primeiro que aparecer na varredura, que ali sairia do
    /// balde e nao da ordem de digitacao.
    fn rowid_do_rownum_varrendo(&mut self, alvo: u64) -> Result<Option<RowId>> {
        let mut melhor: Option<(u64, RowId)> = None;
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            rowid = id + 1;
            let n = self.rownum_do_payload(&payload)?;
            if n < alvo {
                continue;
            }
            if n == alvo {
                // Nao existe candidato melhor: pode parar aqui.
                return Ok(Some(id));
            }
            if melhor.is_none() || n < melhor.unwrap().0 {
                melhor = Some((n, id));
            }
        }
        Ok(melhor.map(|(_, id)| id))
    }

    /// O `rownum` desta linha, sem decodificar o resto dela.
    ///
    /// Zero quando a tabela nao tem a coluna ou o slot esta livre -- e o mesmo
    /// «nao ha numero» dos dois lados, porque a tela trata os dois igual.
    pub fn rownum_de(&mut self, rowid: RowId) -> Result<u64> {
        // Rowid zero e o «pagina vazia» de quem chama, e nao um erro de faixa.
        if rowid == 0 || self.esquema.coluna_rownum().is_none() {
            return Ok(0);
        }
        match self.reg.ler(rowid)? {
            Some(p) => self.rownum_do_payload(&p),
            None => Ok(0),
        }
    }

    /// Quantas linhas a visao enxerga, SEM varrer.
    ///
    /// # Por que agora da para contar
    ///
    /// Contar era o item mais caro da tela: mostrar «pagina 3 de 40» custava
    /// percorrer a tabela inteira, e por isso o `total` tinha saido da
    /// resposta. Com o contador de marcadas no cabecalho a conta fecha em
    /// tempo constante, porque os dois numeros de que ela precisa ja estao
    /// la: `registros` sao os slots ocupados, `marcadas` sao os ocupados que
    /// estao escondidos, e a diferenca e o que a lista mostra.
    ///
    /// Numa tabela sem a coluna de sistema nao ha marca: `Excluidas` da zero
    /// e as outras duas dao o total.
    pub fn contar(&self, visao: Visao) -> u64 {
        if self.esquema.coluna_softdeleted().is_none() {
            return match visao {
                Visao::Excluidas => 0,
                _ => self.reg.registros(),
            };
        }
        match visao {
            Visao::Ativas => self.reg.registros().saturating_sub(self.reg.marcadas()),
            Visao::Excluidas => self.reg.marcadas(),
            Visao::Todas => self.reg.registros(),
        }
    }

    /// Quantas linhas vivas estao marcadas como excluidas. Sai do cabecalho.
    pub fn marcadas(&self) -> u64 {
        self.reg.marcadas()
    }

    /// Reconta as marcadas varrendo, e corrige o cabecalho. Devolve o total.
    ///
    /// O contador do cabecalho e um cache, como o `live_count`. Este e o
    /// caminho que o refaz quando ha duvida -- um arquivo que veio de uma
    /// versao anterior, uma queda no meio de uma exclusao, um reparo.
    pub fn recontar_marcadas(&mut self) -> Result<u64> {
        if self.esquema.coluna_softdeleted().is_none() {
            self.reg.definir_marcadas(0)?;
            return Ok(0);
        }
        let mut n = 0u64;
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            rowid = id + 1;
            if self.marcada_no_payload(&payload)? {
                n += 1;
            }
        }
        self.reg.definir_marcadas(n)?;
        Ok(n)
    }

    /// A posicao de uma linha na listagem e o `rownum` dela menos um?
    ///
    /// # Por que a pergunta importa
    ///
    /// Se a resposta e sim, pular para a posicao 500.000 deixa de ser meio
    /// milhao de passos e vira uma bisseccao de vinte leituras: basta procurar
    /// o `rownum` 500.001. Se e nao, a conta erraria -- e erraria calada, que
    /// e o jeito pior de errar numa tela de paginacao.
    ///
    /// # As quatro coisas que a quebram
    ///
    /// 1. **Tabela sem a coluna** -- nao ha numero de ordem para procurar.
    /// 2. **Particao alfanumerica** -- a leitura sai balde a balde e o
    ///    `rownum` guarda a digitacao; as duas ordens sao diferentes de
    ///    proposito.
    /// 3. **Exclusao fisica** -- a linha saiu, o numero dela nao volta, e
    ///    todo mundo depois dela anda um para tras. Da para ver em tempo
    ///    constante: `rownum_atual() - 1` e quantas linhas ja entraram, e
    ///    `registros()` e quantas ficaram.
    /// 4. **Exclusao suave**, na visao comum -- a linha continua no arquivo e
    ///    continua com o numero, mas some da lista. Por isso o cabecalho
    ///    carrega quantas estao marcadas.
    ///
    /// Nenhuma das quatro custa leitura: as duas ultimas saem de contadores
    /// que ja moram no cabecalho do volume 1.
    pub fn posicao_e_rownum(&self, visao: Visao) -> bool {
        if self.esquema.coluna_rownum().is_none() || self.reg.paginacao().modo.por_letra() {
            return false;
        }
        // A lista de excluidas nao tem relacao nenhuma com a ordem de
        // chegada: a decima marcada pode ser a linha numero tres.
        if visao == Visao::Excluidas {
            return false;
        }
        if self.reg.rownum_atual() - 1 != self.reg.registros() {
            return false;
        }
        visao == Visao::Todas || self.reg.marcadas() == 0
    }

    /// A pagina que comeca na posicao `pular`, pelo caminho mais barato que
    /// ainda estiver certo.
    ///
    /// E o `OFFSET` do SQL, e o que a caixa «ir para a pagina» da grade usa.
    /// Devolve tambem COMO chegou la, porque as duas formas custam ordens de
    /// grandeza diferentes e quem esta do outro lado merece saber qual pagou:
    ///
    /// - [`Salto::Bissecao`] -- a posicao e o `rownum`, e ai o inicio da
    ///   pagina sai de uma busca binaria. Custa `log2 N` leituras, e nao `N`.
    /// - [`Salto::Passo`] -- a tabela tem buraco, ou e alfanumerica, ou a
    ///   visao e a das excluidas. Ai anda ate a posicao, uma linha por vez.
    ///
    /// Nos dois casos a resposta e a MESMA pagina. O que muda e o preco.
    pub fn pagina_por_posicao(
        &mut self,
        pular: u64,
        limite: u64,
        visao: Visao,
    ) -> Result<(Vec<RowId>, Salto)> {
        if pular > 0 && self.posicao_e_rownum(visao) {
            // A posicao e base zero e o rownum comeca em 1.
            let rowids = self.pagina_desde_rownum(pular + 1, limite, visao)?;
            return Ok((rowids, Salto::Bissecao));
        }
        // Pular zero tambem passa por aqui: a primeira pagina nao tem o que
        // pular, e a bisseccao so acrescentaria uma busca para achar o comeco.
        Ok((self.pagina(pular, limite, visao)?, Salto::Passo))
    }

    /// A pagina ANTERIOR ao cursor, para o botao de voltar.
    ///
    /// Devolve em ordem crescente, como a de ir: quem chama nao deveria ter de
    /// saber que a leitura veio de tras para a frente.
    pub fn pagina_antes_de(
        &mut self,
        cursor: RowId,
        limite: u64,
        visao: Visao,
    ) -> Result<Vec<RowId>> {
        if cursor <= 1 {
            return Ok(Vec::new());
        }
        let mut saida = Vec::new();
        let mut rowid = cursor - 1;
        while rowid >= 1 {
            if let Some(payload) = self.reg.ler(rowid)? {
                if self.visao_aceita_payload(&payload, visao)? {
                    saida.push(rowid);
                    if limite > 0 && saida.len() as u64 >= limite {
                        break;
                    }
                }
            }
            if rowid == 1 {
                break;
            }
            rowid -= 1;
        }
        saida.reverse();
        Ok(saida)
    }

    /// A visao aceita este payload? Le SO o byte da coluna de sistema.
    ///
    /// Decodificar a linha inteira para olhar um bit seria pagar o `.memo` e o
    /// `.bin` de cada linha percorrida -- que e justamente o que a paginacao
    /// existe para nao fazer.
    fn visao_aceita_payload(&self, payload: &[u8], visao: Visao) -> Result<bool> {
        if visao == Visao::Todas {
            return Ok(true);
        }
        if self.esquema.coluna_softdeleted().is_none() {
            return Ok(visao != Visao::Excluidas);
        }
        Ok(visao.aceita(self.marcada_no_payload(payload)?))
    }

    /// A linha esta marcada como excluida? Le SO o byte da coluna de sistema.
    ///
    /// Falso tambem quando a tabela nao tem a coluna: ali nao ha marca, e
    /// nenhuma linha esta excluida de forma suave.
    fn marcada_no_payload(&self, payload: &[u8]) -> Result<bool> {
        let Some(i) = self.esquema.coluna_softdeleted() else {
            return Ok(false);
        };
        // Nulo no bitmap nao acontece nesta coluna, que e obrigatoria -- mas
        // se acontecer, «nao marcada» e a leitura segura.
        if payload[i / 8] & (1 << (i % 8)) != 0 {
            return Ok(false);
        }
        let off = self.esquema.offset_coluna(i)?;
        Ok(payload[off] != 0)
    }

    /// Tira da lista os rowids que a visao nao enxerga.
    ///
    /// Os caminhos por indice devolvem rowid, e a marca esta no registro:
    /// filtrar exige ler cada um. Numa passada so -- ler duas vezes para
    /// depois cruzar as duas listas custaria o dobro de leitura e uma busca
    /// linear por elemento.
    ///
    /// Numa tabela sem a coluna de sistema nao ha o que marcar: a lista volta
    /// como veio, sem leitura nenhuma, e `Excluidas` volta vazia.
    pub fn filtrar(&mut self, rowids: &[RowId], visao: Visao) -> Result<Vec<RowId>> {
        if visao == Visao::Todas {
            return Ok(rowids.to_vec());
        }
        if self.esquema.coluna_softdeleted().is_none() {
            return Ok(match visao {
                Visao::Excluidas => Vec::new(),
                _ => rowids.to_vec(),
            });
        }
        let mut saida = Vec::with_capacity(rowids.len());
        for &r in rowids {
            if let Some(p) = self.reg.ler(r)? {
                let linha = self.decodificar(&p, false)?;
                if visao.aceita(self.esta_excluida(&linha)) {
                    saida.push(r);
                }
            }
        }
        Ok(saida)
    }

    /// Atalho para a visao comum. Ver [`Table::filtrar`].
    pub fn filtrar_ativos(&mut self, rowids: &[RowId]) -> Result<Vec<RowId>> {
        self.filtrar(rowids, Visao::Ativas)
    }

    // -------------------------------------------------- so administrador

    /// As linhas que sairam do `.reg`, da mais antiga para a mais recente.
    ///
    /// `com_externos` falso deixa os anexos de fora -- a tela que lista a
    /// lixeira nao precisa carregar as fotos de mil linhas.
    pub fn lixeira(
        &mut self,
        pular: u64,
        limite: u64,
        com_externos: bool,
    ) -> Result<Vec<Descartada>> {
        self.lixeira.ler(pular, limite, com_externos)
    }

    /// Quantas linhas a lixeira guarda, e quantos bytes ela ocupa.
    pub fn lixeira_tamanho(&mut self) -> Result<(u64, u64)> {
        Ok((self.lixeira.total()?, self.lixeira.bytes()?))
    }

    /// Decodifica uma linha da lixeira usando o esquema ATUAL da tabela.
    ///
    /// Se o esquema mudou depois do descarte, o payload guardado nao bate com
    /// ele -- e por isso a conferencia do tamanho vem antes, com uma mensagem
    /// que diz o que aconteceu em vez de devolver campo trocado.
    pub fn linha_da_lixeira(&mut self, d: &Descartada) -> Result<Linha> {
        if d.payload.len() != self.esquema.payload_len() {
            return Err(PhxError::Esquema(format!(
                "a linha descartada tem {} bytes de payload e o esquema atual de {} \
                 espera {}: a tabela mudou depois do descarte",
                d.payload.len(),
                self.nome,
                self.esquema.payload_len()
            )));
        }
        let mut linha = self.decodificar(&d.payload, false)?;
        // Os externos vem do proprio registro da lixeira, e nao do `.bin` /
        // `.memo`: aqueles blocos foram liberados na exclusao e podem ja ter
        // sido reaproveitados por outra linha.
        for (coluna, bytes) in &d.externos {
            let i = *coluna as usize;
            let Some(col) = self.esquema.colunas().get(i) else {
                continue;
            };
            linha[i] = match col.ty {
                ColumnType::Bin => Value::Bin(bytes.clone()),
                ColumnType::Memo => Value::Memo(String::from_utf8_lossy(bytes).into_owned()),
                _ => continue,
            };
        }
        Ok(linha)
    }

    /// Esvazia a lixeira. Registra o expurgo no `.reason` ANTES de apagar:
    /// o motivo tem de sobreviver ao dado.
    pub fn esvaziar_lixeira(&mut self, motivo: &str) -> Result<u64> {
        self.conferir_motivo(motivo)?;
        self.motivos.registrar(Tipo::Expurgo, 0, motivo, "")?;
        self.motivos.sincronizar()?;
        self.lixeira.esvaziar()
    }

    /// Os motivos registrados, em ordem cronologica.
    pub fn motivos(&mut self, pular: u64, limite: u64) -> Result<Vec<Motivo>> {
        self.motivos.ler(pular, limite)
    }

    /// Os motivos de um registro.
    pub fn motivos_de(&mut self, rowid: RowId) -> Result<Vec<Motivo>> {
        self.motivos.de(rowid)
    }

    pub fn total_de_motivos(&mut self) -> Result<u64> {
        self.motivos.total()
    }

    fn idx_por_nome(&self, indice: &str) -> Result<usize> {
        self.ndx
            .indice_por_nome(indice)
            .ok_or_else(|| PhxError::NaoEncontrado(format!("indice {indice} nao existe")))
    }

    /// Rowids com a chave exata, em ordem de digitacao dentro da chave.
    pub fn buscar(&mut self, indice: &str, chave: &[Value]) -> Result<Vec<RowId>> {
        let i = self.idx_por_nome(indice)?;
        let valores = self.espalhar(i, chave)?;
        let codificada = self.codificar_chave(i, &valores)?;
        self.ndx.buscar(i, &codificada)
    }

    /// Rowids no intervalo de chaves `[de, ate]`, na ordem do indice.
    pub fn intervalo(
        &mut self,
        indice: &str,
        de: Option<&[Value]>,
        ate: Option<&[Value]>,
    ) -> Result<Vec<RowId>> {
        let i = self.idx_por_nome(indice)?;
        let de = match de {
            Some(v) => Some(self.codificar_chave(i, &self.espalhar(i, v)?)?),
            None => None,
        };
        let ate = match ate {
            Some(v) => Some(self.codificar_chave(i, &self.espalhar(i, v)?)?),
            None => None,
        };
        self.ndx.intervalo(i, de.as_deref(), ate.as_deref())
    }

    /// Todos os rowids na ordem do indice.
    pub fn varrer_indice(&mut self, indice: &str) -> Result<Vec<RowId>> {
        let i = self.idx_por_nome(indice)?;
        self.ndx.varrer(i)
    }

    /// Recebe os valores na ordem das colunas do INDICE e devolve um vetor
    /// no formato de linha, para reaproveitar `codificar_chave`.
    fn espalhar(&self, idx: usize, chave: &[Value]) -> Result<Linha> {
        let esquema = &self.esquema;
        let def = &esquema.indices()[idx];
        if chave.len() != def.colunas.len() {
            return Err(PhxError::Tipo(format!(
                "indice {} tem {} colunas, recebeu {} valores",
                def.nome,
                def.colunas.len(),
                chave.len()
            )));
        }
        let mut linha = vec![Value::Null; esquema.colunas().len()];
        for (ic, v) in def.colunas.iter().zip(chave.iter()) {
            linha[ic.coluna] = v.clone();
        }
        Ok(linha)
    }

    // ------------------------------------------------------- manutencao

    /// Confere a integridade das quatro pecas: CRC de cada registro, CRC e
    /// ordenacao de cada pagina de indice, e CRC de cada bloco externo.
    pub fn verificar(&mut self) -> Result<Relatorio> {
        let registros = self.reg.verificar()?;
        let indices = self.ndx.verificar()?;
        let blocos_bin = self.bin.verificar()?;
        let blocos_memo = self.memo.verificar()?;
        let eventos = self.log.verificar()?;
        let descartadas = self.lixeira.verificar()?;
        let motivos = self.motivos.verificar()?;
        // Reconta e corrige de passagem: um contador de cache so serve
        // enquanto alguem se dispoe a conferi-lo.
        let marcadas = self.recontar_marcadas()?;

        for (nome, qtd) in &indices {
            if *qtd != registros {
                return Err(PhxError::Corrompido(format!(
                    "{}: indice {nome} tem {qtd} chaves para {registros} registros",
                    self.nome
                )));
            }
        }

        Ok(Relatorio {
            tabela: self.nome.clone(),
            registros,
            slots: self.reg.slots(),
            indices,
            blocos_bin,
            blocos_memo,
            eventos,
            descartadas,
            motivos,
            marcadas,
            volumes: (
                self.reg.volumes().len(),
                self.bin.volumes().len(),
                self.memo.volumes().len(),
                self.log.volumes().len(),
            ),
        })
    }

    /// Recria o `.ndx` inteiro a partir do `.reg`.
    ///
    /// Resolve tres coisas de uma vez: indice corrompido ou apagado, arvore
    /// subocupada depois de muitas exclusoes (a remocao nao rebalanceia), e
    /// indice novo acrescentado a uma tabela que ja tem dados.
    ///
    /// A varredura e feita na ordem de digitacao, entao a arvore sai com os
    /// rowids inseridos em ordem crescente dentro de cada chave.
    pub fn reindexar(&mut self) -> Result<Vec<(String, u64)>> {
        // `NdxFile::criar` trunca o arquivo: a arvore antiga vai embora
        // inteira, em vez de ser remendada.
        self.ndx = NdxFile::criar(caminho(&self.diretorio, &self.nome, EXT_NDX), &self.esquema)?;

        // Uma varredura do `.reg` para TODOS os indices, e depois uma
        // construcao em lote por indice -- em vez de uma descida na arvore por
        // chave, que e o mesmo trabalho do caminho de dentro feito de novo.
        let quantos_indices = self.esquema.indices().len();
        let mut lotes: Vec<Vec<u8>> = vec![Vec::new(); quantos_indices];
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            let valores = self.decodificar(&payload, false)?;
            for (i, lote) in lotes.iter_mut().enumerate() {
                let chave = self.codificar_chave(i, &valores)?;
                lote.extend_from_slice(&NdxFile::chave_completa(&chave, id));
            }
            rowid = id + 1;
        }
        for (i, lote) in lotes.into_iter().enumerate() {
            self.ndx.construir_em_lote(i, lote)?;
        }
        self.ndx.verificar()
    }

    /// Eventos do diario em ordem cronologica. `limite` zero devolve todos.
    pub fn diario(&mut self, pular: u64, limite: u64) -> Result<Vec<Evento>> {
        self.log.ler(pular, limite)
    }

    /// O mesmo, trazendo a imagem de cada evento. E o fluxo da replicacao.
    ///
    /// `pular` e a POSICAO que a replica guardou: o evento N e a posicao N, e
    /// por isso a replica precisa de um numero so por tabela -- nao ha GTID a
    /// inventar nem par arquivo+offset a negociar.
    pub fn diario_com_imagem(&mut self, pular: u64, limite: u64) -> Result<Vec<(Evento, Vec<u8>)>> {
        self.log.ler_com_imagem(pular, limite)
    }

    /// Eventos de um registro especifico.
    pub fn historico(&mut self, rowid: RowId) -> Result<Vec<Evento>> {
        self.log.historico(rowid)
    }

    /// Onde a ultima leitura do diario parou. Ver [`crate::log::MarcaDoDiario`].
    ///
    /// Existe para quem le o diario em lotes seguidos e nao mantem a tabela
    /// aberta entre eles -- o servidor, na replicacao.
    pub fn marca_do_diario(&self) -> Option<crate::log::MarcaDoDiario> {
        self.log.marca()
    }

    /// Aceita a dica de onde comecar a proxima leitura do diario.
    pub fn definir_marca_do_diario(&mut self, marca: Option<crate::log::MarcaDoDiario>) {
        self.log.definir_marca(marca);
    }

    /// Total de eventos registrados no diario.
    pub fn eventos(&mut self) -> Result<u64> {
        self.log.total()
    }

    /// Define quem assina as proximas operacoes no diario.
    pub fn definir_usuario(&mut self, usuario: u32) {
        self.log.usuario = usuario;
        self.lixeira.usuario = usuario;
        self.motivos.usuario = usuario;
    }

    /// Ocupacao dos arquivos externos: `(.bin, .memo)`.
    pub fn estatisticas_externas(
        &mut self,
    ) -> Result<(crate::blob::EstatisticaBlob, crate::blob::EstatisticaBlob)> {
        Ok((self.bin.estatistica()?, self.memo.estatistica()?))
    }

    /// Volumes existentes de cada arquivo paginado.
    pub fn volumes_por_arquivo(&self) -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>) {
        (
            self.reg.volumes(),
            self.bin.volumes(),
            self.memo.volumes(),
            self.log.volumes(),
        )
    }

    /// Paginas ocupadas pelo `.ndx`, incluindo a pagina 0 de cabecalho.
    /// Do `.ndx`: paginas servidas pelo cache, lidas do arquivo, e gravadas.
    pub fn estatisticas_paginas(&self) -> (u64, u64, u64) {
        self.ndx.estatisticas_paginas()
    }

    pub fn paginas_indice(&self) -> u64 {
        self.ndx.paginas()
    }

    /// Descritores dos indices como estao gravados no `.ndx`.
    pub fn descritores_indices(&self) -> &[crate::ndx::DescritorIndice] {
        self.ndx.indices()
    }

    pub fn sincronizar(&mut self) -> Result<()> {
        self.reg.sincronizar()?;
        self.ndx.sincronizar()?;
        self.bin.sincronizar()?;
        self.memo.sincronizar()?;
        self.log.sincronizar()?;
        self.lixeira.sincronizar()?;
        self.motivos.sincronizar()?;
        // O descritor acompanha o disco: ele so vale se disser o que os
        // arquivos dizem, e o `sincronizar` e justamente o instante em que os
        // arquivos param de mudar.
        self.gravar_pag()?;
        Ok(())
    }
}
