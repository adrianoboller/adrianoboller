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
use phxsql_core::schema::Schema;
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
        let bin = BlobFile::criar(&diretorio, &nome, EXT_BIN, MAGIC_BIN, paginacao)?;
        let memo = BlobFile::criar(&diretorio, &nome, EXT_MEMO, MAGIC_MEMO, paginacao)?;
        let log = LogFile::criar(&diretorio, &nome, paginacao)?;
        let lixeira = LixeiraFile::criar(&diretorio, &nome, paginacao)?;
        let motivos = MotivoFile::criar(&diretorio, &nome, paginacao)?;
        let reg = RegFile::criar(&diretorio, &nome, esquema.clone())?;

        Ok(Table {
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
        })
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
    pub fn reparar(&mut self) -> Result<(u64, u64, u64)> {
        self.reg.reparar()
    }

    pub fn abrir(diretorio: impl AsRef<Path>, nome: &str) -> Result<Table> {
        let diretorio = diretorio.as_ref().to_path_buf();
        let reg = RegFile::abrir(&diretorio, nome)?;
        let paginacao = reg.esquema().paginacao();
        let ndx = NdxFile::abrir(caminho(&diretorio, nome, EXT_NDX))?;
        let bin = BlobFile::abrir(&diretorio, nome, EXT_BIN, MAGIC_BIN, paginacao)?;
        let memo = BlobFile::abrir(&diretorio, nome, EXT_MEMO, MAGIC_MEMO, paginacao)?;
        let log = LogFile::abrir(&diretorio, nome, paginacao)?;
        // `abrir` destes dois CRIA quando falta: tabela feita antes deles
        // existirem tem de continuar abrindo.
        let lixeira = LixeiraFile::abrir(&diretorio, nome, paginacao)?;
        let motivos = MotivoFile::abrir(&diretorio, nome, paginacao)?;

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
        })
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
                    let p = self.memo.gravar(texto.as_bytes())?;
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
                        Value::Bin(self.bin.ler(&p)?)
                    }
                }
                ColumnType::Memo => {
                    if !carregar_externos {
                        Value::Null
                    } else {
                        let p = Ponteiro::ler(&payload[off..fim])?;
                        let bytes = self.memo.ler(&p)?;
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
        let rowid = self
            .reg
            .inserir_no_periodo(&payload, self.chave_do_periodo(valores)?)?;

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
        self.log.registrar(Operacao::Inclusao, rowid, 1)?;
        Ok(rowid)
    }

    /// Le uma linha completa, carregando `.bin` e `.memo`.
    pub fn ler(&mut self, rowid: RowId) -> Result<Option<Linha>> {
        match self.reg.ler(rowid)? {
            None => Ok(None),
            Some(payload) => Ok(Some(self.decodificar(&payload, true)?)),
        }
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

        let ponteiros_antigos = self.ponteiros(&antigo)?;
        let payload = self.montar_payload(valores)?;
        let versao = self.reg.atualizar(rowid, &payload)?;

        for (i, (antiga, nova)) in chaves_antigas.iter().zip(chaves_novas.iter()).enumerate() {
            if antiga != nova {
                self.ndx.remover(i, antiga, rowid)?;
                self.ndx.inserir(i, nova, rowid)?;
            }
        }
        self.liberar_externos(&ponteiros_antigos)?;
        self.log.registrar(Operacao::Alteracao, rowid, versao)?;
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
        self.lixeira.guardar(rowid, &payload, externos)?;

        let valores = self.decodificar(&payload, false)?;
        let chaves = self.todas_as_chaves(&valores)?;
        for (i, chave) in chaves.iter().enumerate() {
            self.ndx.remover(i, chave, rowid)?;
        }
        let ponteiros = self.ponteiros(&payload)?;
        self.liberar_externos(&ponteiros)?;
        let removeu = self.reg.excluir(rowid)?;
        if removeu {
            self.motivos
                .registrar(Tipo::Fisica, rowid, motivo, &identidade)?;
            self.log.registrar(Operacao::Exclusao, rowid, 0)?;
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
        for (j, (a, b)) in chaves_antigas.iter().zip(chaves_novas.iter()).enumerate() {
            if a != b {
                self.ndx.remover(j, a, rowid)?;
                self.ndx.inserir(j, b, rowid)?;
            }
        }
        self.log.registrar(Operacao::Alteracao, rowid, versao)?;
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
            let bytes = match ty {
                ColumnType::Bin => self.bin.ler(&p)?,
                ColumnType::Memo => self.memo.ler(&p)?,
                _ => continue,
            };
            saida.push((i as u16, bytes));
        }
        Ok(saida)
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
        let Some(i) = self.esquema.coluna_softdeleted() else {
            return Ok(visao != Visao::Excluidas);
        };
        // Nulo no bitmap nao acontece nesta coluna, que e obrigatoria -- mas
        // se acontecer, «nao marcada» e a leitura segura.
        let excluida = if payload[i / 8] & (1 << (i % 8)) != 0 {
            false
        } else {
            let off = self.esquema.offset_coluna(i)?;
            payload[off] != 0
        };
        Ok(visao.aceita(excluida))
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

        let quantos_indices = self.esquema.indices().len();
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            let valores = self.decodificar(&payload, false)?;
            for i in 0..quantos_indices {
                let chave = self.codificar_chave(i, &valores)?;
                self.ndx.inserir(i, &chave, id)?;
            }
            rowid = id + 1;
        }
        self.ndx.verificar()
    }

    /// Eventos do diario em ordem cronologica. `limite` zero devolve todos.
    pub fn diario(&mut self, pular: u64, limite: u64) -> Result<Vec<Evento>> {
        self.log.ler(pular, limite)
    }

    /// Eventos de um registro especifico.
    pub fn historico(&mut self, rowid: RowId) -> Result<Vec<Evento>> {
        self.log.historico(rowid)
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
        Ok(())
    }
}
