//! Hierarquia de bancos, schemas e tabelas em disco.
//!
//! ```text
//! base/
//! └── Z/                        database Z
//!     ├── cadastroClientes.reg  ] tabelas da raiz
//!     ├── cadastroClientes.ndx  ]  (sem schema)
//!     ├── ...                   ]
//!     ├── X/                    schema X
//!     │   └── pedidos.reg ...   tabelas do schema X
//!     └── Y/                    schema Y
//!         └── notas.reg ...     tabelas do schema Y
//! ```
//!
//! A regra e estrutural, sem arquivo de marcacao: um diretorio dentro da base
//! e um database; um diretorio dentro de um database e um schema; um arquivo
//! `.reg` e uma tabela. Tabelas soltas na raiz do database sao as "tabelas
//! raiz" -- equivalentes ao `public` do Postgres ou ao `dbo` do SQL Server.
//!
//! O nome qualificado de uma tabela e `schema.tabela`, ou so `tabela` quando
//! ela esta na raiz. E o mesmo formato que o catalogo do FraseSQL espera.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_core::paginacao::BALDES;
use phxsql_core::schema::Schema;
use phxsql_core::TipoDatabase;
use phxsql_core::EXT_REG;

/// O marcador do TIPO de um database. Ate aqui «a regra era estrutural, sem
/// arquivo de marcacao» -- um diretorio era um database e ponto. Os tres tipos
/// (decisao do dono, 11/09/2026) exigem guardar o tipo em algum lugar, e este
/// arquivo e esse lugar. Nome com prefixo `_` e sufixo `.json`: nao e `.reg`,
/// entao `tabelas_em` o ignora de graca, e nao colide com nome de tabela.
///
/// **Ausencia = Padrao**, de proposito: todo database que nasceu antes desta
/// decisao nao tem marca, e continua padrao sem migracao -- mesma disciplina do
/// byte de chave do PSCH v7 («guarda nova entra pedida, nao imposta»).
const MARCA_DATABASE: &str = "_database.json";

/// Grava o marcador do tipo no diretorio do database.
fn escrever_marca(diretorio: &Path, tipo: TipoDatabase) -> Result<()> {
    let j = Json::objeto(vec![
        ("tipo", Json::texto_de(tipo.como_texto())),
        ("versao", Json::de_i64(1)),
    ]);
    std::fs::write(diretorio.join(MARCA_DATABASE), j.escrever_identado())?;
    Ok(())
}

/// Le o tipo do marcador. Ausencia, leitura falha, JSON quebrado ou tipo
/// desconhecido **caem em Padrao** -- nunca param a abertura de um database que
/// ja existe. Um tipo estranho no marcador e' dado corrompido do marcador, e a
/// resposta segura e' tratar como padrao, nao recusar abrir o banco.
fn ler_marca(diretorio: &Path) -> TipoDatabase {
    std::fs::read_to_string(diretorio.join(MARCA_DATABASE))
        .ok()
        .and_then(|txt| Json::analisar(&txt).ok())
        .map(|j| TipoDatabase::de_texto(j.texto_ou("tipo", "padrao")).unwrap_or_default())
        .unwrap_or_default()
}

use crate::fts::EXT_FTS;
use crate::table::{SemEscrever, Table};

/// O nome nao e um engano de digitacao: e uma tentativa de sair do diretorio.
///
/// Separa as duas coisas de proposito. `"minha tabela!"` e um nome ruim --
/// alguem errou. `"../../etc/passwd"` nao e nome nenhum: ninguem digita isso
/// por acidente. Quem chama precisa poder tratar os dois casos de forma
/// diferente, e e por isso que esta funcao existe separada de
/// [`validar_nome`].
/// `esquema.tabela` vira `(Some("esquema"), "tabela")`.
/// O arquivo `precos_002.reg` pertence a tabela `precos` na extensao `reg`?
///
/// A conferencia do sufixo importa: sem ela, `precos_historico.reg` seria
/// dado como volume de `precos` e a exclusao levaria a tabela errada junto.
fn pertence(arquivo: &str, tabela: &str, ext: &str) -> bool {
    let Some(sem_ext) = arquivo.strip_suffix(&format!(".{ext}")) else {
        return false;
    };
    let Some(sufixo) = sem_ext.strip_prefix(tabela) else {
        return false;
    };
    // Ou e o nome exato, ou e o nome mais `_` e um sufixo de volume: so
    // digitos, ou uma das 37 letras da particao alfanumerica.
    if sufixo.is_empty() {
        return true;
    }
    let Some(s) = sufixo.strip_prefix('_') else {
        return false;
    };
    !s.is_empty() && (s.bytes().all(|b| b.is_ascii_digit()) || e_balde(s))
}

/// Este sufixo e o nome de um balde da particao alfanumerica?
///
/// Comparacao contra a lista EXATA, e nao "uma letra qualquer": a lista tem 37
/// nomes e nenhum outro serve. Sem isso, `precos_historico.reg` viraria volume
/// de `precos` -- que e o defeito que esta conferencia existe para evitar,
/// agora com um caso a mais.
fn e_balde(s: &str) -> bool {
    BALDES.contains(&s)
}

pub fn nome_hostil(nome: &str) -> bool {
    nome == "."
        || nome == ".."
        || nome.contains("..")
        || nome
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':') || c.is_control())
}

/// Recusa nomes que escapariam do diretorio ou quebrariam o sistema de
/// arquivos. Vale para database, schema e tabela.
pub fn validar_nome(rotulo: &str, nome: &str) -> Result<()> {
    if nome.is_empty() {
        return Err(PhxError::Esquema(format!("{rotulo} sem nome")));
    }
    if nome == "." || nome == ".." {
        return Err(PhxError::Esquema(format!("{rotulo} invalido: {nome}")));
    }
    if nome.chars().any(|c| {
        matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
    }) {
        return Err(PhxError::Esquema(format!(
            "{rotulo} {nome:?} tem caractere que nao pode entrar em nome de arquivo"
        )));
    }
    Ok(())
}

/// Extrai o nome da tabela de um arquivo `.reg`, tirando o sufixo de volume.
///
/// `cadastroClientes.reg` e `cadastroClientes_007.reg` devolvem os dois
/// `cadastroClientes`.
fn nome_da_tabela(caminho: &Path) -> Option<String> {
    if caminho.extension().and_then(|s| s.to_str()) != Some(EXT_REG) {
        return None;
    }
    let base = caminho.file_stem()?.to_str()?;
    let Some((antes, sufixo)) = base.rsplit_once('_') else {
        return Some(base.to_string());
    };
    if antes.is_empty() || sufixo.is_empty() {
        return Some(base.to_string());
    }
    if sufixo.chars().all(|c| c.is_ascii_digit()) {
        return Some(antes.to_string());
    }
    // Sufixo de LETRA so conta como balde quando o volume 1 esta ali do lado.
    //
    // A conferencia existe por causa de uma ambiguidade real: uma tabela
    // chamada `dados_X` e o balde X de uma tabela `dados` se escrevem igual.
    // O volume 1 (`_A`) nasce com a tabela alfanumerica e nunca falta, entao a
    // presenca dele e o que separa os dois casos -- e uma tabela `dados_X`
    // sozinha continua sendo ela mesma.
    if e_balde(sufixo)
        && caminho
            .with_file_name(format!("{antes}_{}.{EXT_REG}", BALDES[0]))
            .exists()
    {
        return Some(antes.to_string());
    }
    Some(base.to_string())
}

/// Os nomes de tabela que moram num diretorio, resolvendo o sufixo de
/// volume das paginadas. Visivel ao `table` porque a busca reversa da
/// integridade referencial precisa perguntar "quem mais mora aqui?" --
/// e reescrever a varredura la seria a segunda copia de uma regra sutil.
pub(crate) fn tabelas_em(diretorio: &Path) -> Result<Vec<String>> {
    if !diretorio.is_dir() {
        return Ok(Vec::new());
    }
    let mut nomes: Vec<String> = std::fs::read_dir(diretorio)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter_map(|p| nome_da_tabela(&p))
        .collect();
    nomes.sort();
    nomes.dedup();
    Ok(nomes)
}

fn subdiretorios(diretorio: &Path) -> Result<Vec<String>> {
    if !diretorio.is_dir() {
        return Ok(Vec::new());
    }
    let mut nomes: Vec<String> = std::fs::read_dir(diretorio)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    nomes.sort();
    Ok(nomes)
}

/// A raiz que contem varios databases.
pub struct Instancia {
    base: PathBuf,
    /// O INVARIANTE DA TRAVA, escrito de um jeito que o compilador entende.
    ///
    /// # O que a trava global protege, e o que ela NAO protege
    ///
    /// O `servidor.rs` guarda esta `Instancia` num `Mutex`, e e facil ler isso
    /// como «o mutex protege a instancia». **Nao protege**: ela tem UM campo,
    /// um `PathBuf` imutavel, e todo metodo dela e `&self`. Nao ha estado
    /// mutavel aqui dentro para proteger.
    ///
    /// O que o mutex e, de verdade, e uma FICHA DE EXCLUSAO: quem a tem mexe
    /// no disco, e o estado protegido esta la fora, nos arquivos, alcancado
    /// por um `Table` que se abre e se fecha a cada operacao. Isso e convencao,
    /// e convencao que o compilador nao conhece e convencao que uma refacao
    /// apaga em silencio.
    ///
    /// # A refacao que este campo mata na compilacao
    ///
    /// A troca «obvia» para destravar leitura seria `RwLock<Instancia>`:
    /// leitores param de esperar leitores, e o compilador nao reclamaria de
    /// nada -- porque todo metodo e `&self`, e `&self` e o que um guard de
    /// LEITURA da. Dois escritores tomariam guard de leitura, abririam dois
    /// `Table` sobre os mesmos arquivos, e a corrupcao apareceria em producao.
    ///
    /// `Mutex<T>` exige `T: Send`. `RwLock<T>` exige `T: Send + Sync`. Este
    /// campo torna a `Instancia` **`!Sync`** sem custar um byte em execucao:
    /// o `Mutex` continua compilando e o `RwLock` **para de compilar**, com a
    /// mensagem apontando para ca.
    ///
    /// Nao e proibicao de mudar o desenho -- e exigencia de que quem mudar
    /// **veja** este comentario primeiro, em vez de descobrir a razao dele por
    /// um dado corrompido. Trocar a trava exige antes decidir o que passa a
    /// proteger o disco, e ai este campo sai junto, de propósito e por escrito.
    _so_com_a_ficha: PhantomData<std::cell::Cell<()>>,
}

impl Instancia {
    /// Abre (criando se preciso) a raiz de dados.
    pub fn nova(base: impl AsRef<Path>) -> Result<Instancia> {
        let base = base.as_ref().to_path_buf();
        std::fs::create_dir_all(&base)?;
        Ok(Instancia {
            base,
            _so_com_a_ficha: PhantomData,
        })
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    /// Cria um database do tipo **padrao** -- o caminho que sempre existiu.
    pub fn criar_database(&self, nome: &str) -> Result<Database> {
        self.criar_database_com_tipo(nome, TipoDatabase::Padrao)
    }

    /// Cria um database de um TIPO dado (padrao/hive/vetorial) e grava o
    /// marcador. O tipo nasce com o database e nao muda depois: um database e'
    /// de um tipo so, como uma tabela nasce com um esquema.
    pub fn criar_database_com_tipo(&self, nome: &str, tipo: TipoDatabase) -> Result<Database> {
        validar_nome("database", nome)?;
        let caminho = self.base.join(nome);
        if caminho.exists() {
            return Err(PhxError::Esquema(format!("database {nome} ja existe")));
        }
        std::fs::create_dir_all(&caminho)?;
        escrever_marca(&caminho, tipo)?;
        Ok(Database {
            nome: nome.to_string(),
            caminho,
            tipo,
        })
    }

    pub fn abrir_database(&self, nome: &str) -> Result<Database> {
        validar_nome("database", nome)?;
        let caminho = self.base.join(nome);
        // Sem o caminho da base: e o IRMAO da tabela que nao existe (ver
        // `tabela_que_nao_existe`), e pela mesma razao -- a frase sai pelo
        // protocolo para quem errou o nome, e o diretorio do servidor nao e
        // resposta para ninguem que esta do lado de fora.
        if !caminho.is_dir() {
            return Err(PhxError::NaoEncontrado(format!(
                "database {nome} nao existe neste servidor"
            )));
        }
        let tipo = ler_marca(&caminho);
        Ok(Database {
            nome: nome.to_string(),
            caminho,
            tipo,
        })
    }

    /// Cria o database se ainda nao existir.
    pub fn garantir_database(&self, nome: &str) -> Result<Database> {
        match self.abrir_database(nome) {
            Ok(d) => Ok(d),
            Err(_) => self.criar_database(nome),
        }
    }

    pub fn databases(&self) -> Result<Vec<String>> {
        subdiretorios(&self.base)
    }
}

/// A raiz de dados como as DUAS fichas a enxergam.
///
/// # O que ela e, e por que ela existe
///
/// O servidor guardava a [`Instancia`] num `Mutex`: uma ficha de exclusao, uma
/// operacao de cada vez, leitor esperando leitor. Trocar esse `Mutex` por um
/// `RwLock<Instancia>` **compila e esta errado**, e o marcador `!Sync` da
/// `Instancia` existe justamente para nao deixar: `&self` e o que um guard de
/// LEITURA entrega, e todo metodo da `Instancia` -- inclusive `criar_tabela` e
/// `excluir_tabela` -- e `&self`.
///
/// A `Raiz` e o que se poe dentro do `RwLock`, e ela separa as duas fichas
/// pelo tipo de emprestimo, que e a unica coisa que o `RwLock` sabe distinguir:
///
/// * `&Raiz` -- o que o guard de LEITURA entrega a N threads ao mesmo tempo --
///   so alcanca [`Raiz::abrir_para_ler`], que devolve uma
///   [`TabelaLeitura`](crate::leitura::TabelaLeitura) sem um unico metodo de
///   escrita;
/// * `&mut Raiz` -- o que so o guard de ESCRITA entrega, e a um de cada vez --
///   alcanca [`Raiz::exclusiva`], e por ela a `Instancia` inteira.
///
/// A `Instancia` NAO mora aqui dentro, e nao e por economia: guardar um campo
/// `!Sync` faria a `Raiz` tambem `!Sync`, e ai o `RwLock` voltaria a nao
/// compilar. Ela nasce do `PathBuf` a cada `exclusiva()`, presa a vida do
/// emprestimo mutavel -- que e o que impede alguem de guardar a ficha
/// exclusiva e continuar usando-a depois de soltar a trava.
pub struct Raiz {
    base: PathBuf,
}

/// A ficha EXCLUSIVA, presa a vida do guard que a entregou.
///
/// O `'a` e a garantia inteira: ele vem do `&mut Raiz`, que vem do guard de
/// escrita. A `Instancia` nao pode sobreviver ao guard, entao nao ha como
/// abrir uma tabela gravavel e leva-la para fora da trava.
pub struct Exclusiva<'a> {
    instancia: Instancia,
    _guarda: PhantomData<&'a mut Raiz>,
}

impl std::ops::Deref for Exclusiva<'_> {
    type Target = Instancia;
    fn deref(&self) -> &Instancia {
        &self.instancia
    }
}

impl std::ops::DerefMut for Exclusiva<'_> {
    fn deref_mut(&mut self) -> &mut Instancia {
        &mut self.instancia
    }
}

impl Exclusiva<'_> {
    /// Solta a ficha da vida do emprestimo, para quem a guarda AO LADO do
    /// proprio guard da trava.
    ///
    /// # Por que isto existe, e por que ha um chamador so
    ///
    /// O `TravaMedida` do servidor embrulha o guard de escrita e precisa
    /// guardar a ficha no mesmo `struct`. Uma `Exclusiva<'a>` ali dentro seria
    /// uma estrutura auto-referente -- o `'a` viria do campo vizinho --, e
    /// Rust nao a aceita.
    ///
    /// O que se perde e a garantia do compilador de que a ficha morre com o
    /// guard. Quem chama passa a responder por isso, e por isso o unico
    /// chamador guarda os dois no mesmo `struct`: eles nascem e morrem na
    /// mesma linha, e nao ha caminho pelo qual um sobreviva ao outro.
    pub fn sem_amarra(self) -> Instancia {
        self.instancia
    }
}

/// O que a ficha compartilhada devolve ao tentar abrir uma tabela.
///
/// A segunda variante nao e erro: e «esta tabela quer a ficha exclusiva»,
/// porque abri-la escreveria. Quem chama solta a compartilhada e refaz o
/// trabalho por la -- e o comportamento visto de fora nao muda nada.
// O `Table` ja viaja por valor num `Result<Table>` desde sempre, e a
// variante grande e a comum. Um `Box` compraria 2,9 KiB de enum e
// pagaria uma alocacao por ABERTURA, que acontece a cada operacao.
#[allow(clippy::large_enum_variant)]
pub enum Aberta {
    Pronta(crate::leitura::TabelaLeitura),
    PrecisaDaFichaExclusiva(&'static str),
}

impl Raiz {
    /// Abre (criando se preciso) a raiz de dados.
    pub fn nova(base: impl AsRef<Path>) -> Result<Raiz> {
        let base = base.as_ref().to_path_buf();
        std::fs::create_dir_all(&base)?;
        Ok(Raiz { base })
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    /// A ficha exclusiva. Exige `&mut self`, e so o guard de ESCRITA o da.
    ///
    /// O `PathBuf` e clonado aqui, uma vez por operacao de escrita. Custa uma
    /// alocacao curta num caminho que ja paga abertura de arquivo e, quando a
    /// janela fecha, `fsync` -- e a alternativa seria guardar a `Instancia`
    /// dentro da `Raiz`, que e exatamente o que o marcador `!Sync` proibe.
    pub fn exclusiva(&mut self) -> Exclusiva<'_> {
        Exclusiva {
            instancia: Instancia {
                base: self.base.clone(),
                _so_com_a_ficha: PhantomData,
            },
            _guarda: PhantomData,
        }
    }

    /// Abre uma tabela para LER, e diz quando abrir exigiria escrever.
    ///
    /// # Nada aqui dentro escreve
    ///
    /// A resolucao do caminho passa por uma `Instancia` de propósito: os nomes
    /// de database, schema e tabela se conferem num lugar so, e as mensagens
    /// de recusa sao as mesmas das duas fichas. Uma segunda copia dessas
    /// regras divergiria da primeira, e divergiria na mensagem que alguem le
    /// as duas da manha.
    ///
    /// O que a torna segura nao e este comentario e sim o que ela DEVOLVE: uma
    /// [`TabelaLeitura`](crate::leitura::TabelaLeitura), aberta por
    /// [`Table::abrir_para_ler`], que recusa quando abrir escreveria.
    pub fn abrir_para_ler(&self, database: &str, qualificado: &str) -> Result<Aberta> {
        let so_para_achar_o_caminho = Instancia {
            base: self.base.clone(),
            _so_com_a_ficha: PhantomData,
        };
        let db = so_para_achar_o_caminho.abrir_database(database)?;
        db.exigir_motor_padrao()?;
        let (schema, nome) = separar_qualificado(qualificado);
        validar_nome("tabela", &nome)?;
        let dir = db.diretorio(schema.as_deref())?;
        // O motivo vem do COMPONENTE que recusou, e nao de uma frase generica:
        // quem le isto num log precisa saber se foi a lixeira que faltava ou o
        // diario que precisava de cura, porque as duas se consertam diferente.
        // A UNICA excecao e a tabela que nao existe, e ela e nomeada pela
        // mesma regra do `abrir_tabela` -- ver `tabela_que_nao_existe`.
        let aberta = Table::abrir_para_ler(dir, &nome)
            .map_err(|e| db.tabela_que_nao_existe(e, schema.as_deref(), &nome));
        Ok(match aberta? {
            SemEscrever::Aberta(t) => Aberta::Pronta(crate::leitura::TabelaLeitura::nova(t)),
            SemEscrever::PrecisaEscrever(porque) => Aberta::PrecisaDaFichaExclusiva(porque),
        })
    }
}

/// Um database: tabelas na raiz e, opcionalmente, schemas em subdiretorios.
pub struct Database {
    nome: String,
    caminho: PathBuf,
    tipo: TipoDatabase,
}

impl Database {
    pub fn nome(&self) -> &str {
        &self.nome
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// O tipo deste database (padrao/hive/vetorial). Lido do marcador na
    /// abertura; ausencia = padrao.
    pub fn tipo(&self) -> TipoDatabase {
        self.tipo
    }

    /// O PORTAO do motor, e e UM so: o motor padrao (tabelas relacionais em
    /// arquivos separados) opera SOMENTE database do tipo `Padrao`. Hive e
    /// Vetorial tem o tipo reservado e o marcador gravado, mas o MOTOR de cada
    /// um e frente aberta -- rodar uma operacao de tabela num deles pelo motor
    /// padrao seria **fingir que gravou** (metade pior que nada): criaria um
    /// `.reg` relacional num diretorio que se diz colmeia. Recusa honesta, com
    /// o tipo e o estado.
    ///
    /// Chamado no TOPO das entradas do motor padrao que mexem em tabela --
    /// `criar_tabela`, `abrir_tabela`, `excluir_tabela`, `duplicar_tabela`,
    /// `copiar_tabela_para` e o `abrir_para_ler` da `Instancia`. Listar o que
    /// existe (`tabelas`, `existe_tabela`, `bancos`) NAO passa por aqui de
    /// proposito: um database hive EXISTE e aparece na lista com o seu tipo --
    /// esconde-lo seria outra mentira. A linha que se recusa a cruzar e'
    /// **operar** a tabela, nao **enxergar** o database.
    ///
    /// Quem acrescentar uma entrada de tabela nova ao motor padrao chama este
    /// portao tambem -- e a mesma licao do portao de permissao: a operacao que
    /// alguem esquecer de amarrar vira a porta dos fundos, e aqui a porta dos
    /// fundos seria uma tabela relacional nascendo dentro de uma colmeia.
    fn exigir_motor_padrao(&self) -> Result<()> {
        if self.tipo == TipoDatabase::Padrao {
            return Ok(());
        }
        Err(PhxError::Esquema(format!(
            "o database {} e do tipo {} e o motor dele esta em construcao: \
             operacoes de tabela ainda nao funcionam nele",
            self.nome,
            self.tipo.como_texto()
        )))
    }

    /// Diretorio de um schema, ou a raiz do database quando `schema` e `None`.
    pub fn diretorio(&self, schema: Option<&str>) -> Result<PathBuf> {
        match schema {
            None => Ok(self.caminho.clone()),
            Some(s) => {
                validar_nome("schema", s)?;
                Ok(self.caminho.join(s))
            }
        }
    }

    pub fn criar_schema(&self, nome: &str) -> Result<PathBuf> {
        validar_nome("schema", nome)?;
        let caminho = self.caminho.join(nome);
        if caminho.exists() {
            return Err(PhxError::Esquema(format!(
                "schema {nome} ja existe em {}",
                self.nome
            )));
        }
        std::fs::create_dir_all(&caminho)?;
        Ok(caminho)
    }

    pub fn garantir_schema(&self, nome: &str) -> Result<PathBuf> {
        validar_nome("schema", nome)?;
        let caminho = self.caminho.join(nome);
        std::fs::create_dir_all(&caminho)?;
        Ok(caminho)
    }

    pub fn schemas(&self) -> Result<Vec<String>> {
        subdiretorios(&self.caminho)
    }

    /// Tabelas de um schema, ou da raiz quando `schema` e `None`.
    pub fn tabelas(&self, schema: Option<&str>) -> Result<Vec<String>> {
        tabelas_em(&self.diretorio(schema)?)
    }

    /// Toda tabela do database, com nome qualificado, incluindo as dos schemas.
    pub fn todas_as_tabelas(&self) -> Result<Vec<String>> {
        let mut saida: Vec<String> = self.tabelas(None)?;
        for s in self.schemas()? {
            for t in self.tabelas(Some(&s))? {
                saida.push(format!("{s}.{t}"));
            }
        }
        saida.sort();
        Ok(saida)
    }

    pub fn criar_tabela(&self, schema: Option<&str>, esquema: Schema) -> Result<Table> {
        self.exigir_motor_padrao()?;
        validar_nome("tabela", esquema.nome())?;
        let dir = match schema {
            None => self.caminho.clone(),
            Some(s) => self.garantir_schema(s)?,
        };
        Table::criar(dir, esquema)
    }

    pub fn abrir_tabela(&self, schema: Option<&str>, nome: &str) -> Result<Table> {
        self.exigir_motor_padrao()?;
        validar_nome("tabela", nome)?;
        Table::abrir(self.diretorio(schema)?, nome)
            .map_err(|e| self.tabela_que_nao_existe(e, schema, nome))
    }

    /// Troca o erro cru de «nenhum volume de x.reg em /tmp/.../base» por «a
    /// tabela x nao existe em base» -- e SO quando a tabela nao existe mesmo.
    ///
    /// O erro cru e verdadeiro e serve a quem opera o disco, mas ele sai pelo
    /// protocolo para quem digitou `SELECT * FROM x` com o nome errado: manda
    /// procurar arquivo em vez de conferir o nome, e publica o caminho
    /// absoluto do servidor a todo cliente que erra uma letra. E a mesma
    /// correcao que a chave conferida ja pagou em `table.rs`, quando a mae
    /// nao existia.
    ///
    /// # Por que a conferencia e no caminho do ERRO, e por que ela confere
    ///
    /// O store nao sabe qual dos dez arquivos faltou: um `NaoEncontrado` ao
    /// abrir tanto pode ser a tabela inteira ausente quanto uma tabela com o
    /// `.reg` perdido -- e a segunda NAO pode virar «nao existe», porque ai o
    /// operador criaria outra por cima de uma tabela quebrada. Entao a
    /// tradução so acontece quando o diretorio nao tem arquivo nenhum com
    /// aquele nome, e a lista do diretorio so e lida depois de a abertura
    /// falhar: o laco quente, que abre a tabela a cada pedido, nao paga um
    /// `read_dir` por isso.
    fn tabela_que_nao_existe(&self, e: PhxError, schema: Option<&str>, nome: &str) -> PhxError {
        if !matches!(e, PhxError::NaoEncontrado(_)) {
            return e;
        }
        // Erro ao listar o diretorio (schema inexistente, permissao) mantem
        // o erro original: `unwrap_or(true)` diz «nao sei se existe», e na
        // duvida a frase que fica e a que nomeia o componente.
        if self.existe_tabela(schema, nome).unwrap_or(true) {
            return e;
        }
        PhxError::NaoEncontrado(format!(
            "a tabela {} nao existe em {}",
            qualificar(schema, nome),
            self.nome
        ))
    }

    /// Abre por nome qualificado: `schema.tabela` ou so `tabela`.
    pub fn abrir_qualificada(&self, qualificado: &str) -> Result<Table> {
        let (schema, nome) = separar_qualificado(qualificado);
        self.abrir_tabela(schema.as_deref(), &nome)
    }

    /// O que uma CÓPIA leva: os cinco arquivos da tabela mais o espelho
    /// `.bkp`. Nao inclui a lixeira nem os motivos -- uma copia nasce como a
    /// origem esta, e nao herda o que foi excluido dela.
    const EXTENSOES: [&'static str; 6] = ["reg", "ndx", "bin", "memo", "log", "bkp"];

    /// O que uma tabela OCUPA em disco -- tudo que o nome dela carrega.
    ///
    /// Lista separada da de cima, e a diferenca entre as duas e o ponto: o que
    /// uma copia leva e uma decisao (a lixeira da origem nao e da copia), o que
    /// um `excluir_tabela` apaga nao e -- ou apaga tudo, ou nao apagou a tabela.
    ///
    /// Ela nasceu do defeito: a lista de apagar tinha SEIS extensoes e a tabela
    /// ja tinha NOVE. O `.trash`, o `.reason` e o `.pag` entraram depois e
    /// ninguem voltou aqui — a mesma armadilha da peca nova no fim de uma lista
    /// que o `rownum` armou na tela. O estrago era duplo: recriar a tabela com
    /// o mesmo nome passava a ser IMPOSSIVEL (`Table::criar` confere as sete
    /// extensoes e recusa), e se nao fosse, a tabela nova herdaria a lixeira e
    /// os motivos de exclusao de uma tabela alheia — que e conteudo de linha,
    /// e so `administrar` pode ler.
    /// O `.lgpd` entrou nesta lista DEPOIS de o dono olhar a tela e contar dez
    /// arquivos onde a frase dizia cinco. A lista tinha nove e a tabela ja
    /// tinha dez -- exatamente o mesmo defeito de quando tinha seis e a tabela
    /// tinha nove, repetido pela mesma razao: extensao nova entra no motor e
    /// ninguem volta aqui. E este era o pior deles, porque o que ficava para
    /// tras era a trilha de dados PESSOAIS, sob um nome que nao existe mais.
    ///
    /// O `.fts` faltou aqui de novo, pedido 213: entrou no motor em 07/09/2026
    /// (pedido 200) e ninguem voltou a esta lista, exatamente a mesma
    /// armadilha -- so que desta vez o estrago era pior por ser silencioso:
    /// `excluir_tabela` e `renomear_tabela` deixavam o `.fts` para tras (orfao
    /// sob um nome que nao existe mais, no renomear; ou vazando um indice de
    /// texto de uma tabela apagada, no excluir), e `arquivos_da_tabela`
    /// mentia dizendo que a tabela nao tinha indice de texto nenhum. A prova
    /// esta em `fts_fica_orfao_se_a_lista_nao_o_conhece` (o teste que falha
    /// com "fts" fora da lista e passa com ele dentro). O literal vira
    /// `EXT_FTS` (de `crate::fts`) em vez de uma segunda string "fts": duas
    /// fontes da mesma verdade e o que fez o defeito ser possivel.
    ///
    /// A guarda agora nao confere a lista contra ela mesma (isso passaria com
    /// qualquer numero): confere o DIRETORIO depois de um `excluir_tabela`.
    const EXTENSOES_TODAS: [&'static str; 11] = [
        "reg", "ndx", "bin", "memo", "log", "bkp", "trash", "reason", "pag", "lgpd", EXT_FTS,
    ];

    /// A mesma lista de cima, exposta para quem precisa dela como REFERENCIA
    /// e nao para apagar nada -- o conferidor do pedido 213 (`phxsql-server`)
    /// compara os tres inventarios escritos a mao (Figura 1, Figura 8, a
    /// tabela-mestra do `FORMATO.md`) contra ESTA lista, para que nenhum dos
    /// tres volte a divergir do codigo calado. Publica so a leitura: quem
    /// quiser apagar ou renomear continua chamando `excluir_tabela` ou
    /// `renomear_tabela`, que sao os unicos que decidem o que fazer com cada
    /// extensao.
    pub fn extensoes_de_uma_tabela() -> &'static [&'static str] {
        &Self::EXTENSOES_TODAS
    }

    /// Apaga os arquivos de uma tabela e devolve o que apagou.
    ///
    /// Inclui o `.bkp`: deixar o espelho para tras faria a tabela "voltar"
    /// pela metade se alguem recriasse uma com o mesmo nome.
    ///
    /// Nao ha desfazer. Quem chama confere antes.
    ///
    /// # Por que recusa quando alguem aponta para ela
    ///
    /// **A regra primordial vale no nivel da TABELA, e nao so no da linha.**
    /// Medido por sonda (`--example sonda-fk-buracos`): este caminho apagava
    /// os 8 arquivos da mae e deixava a filha com a linha intacta apontando
    /// para o vazio. O `renomear_tabela` ja recusava o MESMO cenario, com o
    /// recado que diz por que -- entao o motor sabia fazer a pergunta e nao a
    /// fazia aqui.
    ///
    /// E apagar e pior que renomear: o renomear deixa a filha apontando para
    /// um nome que nao existe mais, e este deixa a filha apontando para um
    /// nome que nao existe mais **e** joga fora a linha mae, que era a unica
    /// coisa que ainda diria qual pai era aquele.
    ///
    /// Como no renomear, **nao se pula a chave com `verificar: false`**: la a
    /// pergunta e se a regra e IMPOSTA, porque o que sai e uma linha; aqui o
    /// que sai e o NOME, e uma declaracao pendurada num nome inexistente e uma
    /// mentira sobre o modelo mesmo quando ninguem a confere.
    ///
    /// # O limite, escrito em vez de escondido
    ///
    /// A busca e no diretorio do schema da tabela, e nao no database inteiro
    /// -- e o mesmo alcance que o `renomear_tabela` ja tem. Uma filha em
    /// OUTRO schema apontando para ca por nome qualificado nao e vista. Fecha-
    /// -lo pede varrer todos os schemas por `excluir_tabela`, e essa e uma
    /// decisao de custo que se toma com numero na mao, nao de passagem.
    pub fn excluir_tabela(&self, qualificado: &str) -> Result<Vec<String>> {
        self.exigir_motor_padrao()?;
        let (schema, nome) = separar_qualificado(qualificado);
        let (schema, nome) = (schema.as_deref(), nome.as_str());
        validar_nome("tabela", nome)?;
        let dir = self.diretorio(schema)?;
        if let Some(filha) = self.quem_aponta_para(&dir, nome)? {
            return Err(PhxError::Integridade(format!(
                "a tabela {qualificado} nao pode ser apagada: {filha} declara \
                 uma chave estrangeira para ela. Nunca se apaga o pai que tem \
                 filhos -- apague {filha} primeiro, ou tire a chave dela"
            )));
        }
        let mut apagados = Vec::new();
        for ext in Self::EXTENSOES_TODAS {
            // Uma tabela paginada tem varios volumes por extensao.
            for arq in std::fs::read_dir(&dir)?.flatten() {
                let f = arq.file_name();
                let f = f.to_string_lossy();
                if pertence(&f, nome, ext) {
                    std::fs::remove_file(arq.path())?;
                    apagados.push(f.to_string());
                }
            }
        }
        if apagados.is_empty() {
            return Err(PhxError::NaoEncontrado(format!(
                "tabela {qualificado} nao existe em {}",
                self.nome()
            )));
        }
        apagados.sort();
        Ok(apagados)
    }

    /// Os arquivos que esta tabela TEM em disco, com extensao e sem o nome.
    ///
    /// Existe para a tela parar de digitar a lista. A frase do cabecalho dizia
    /// «.reg + .ndx + .bin + .memo + .log» -- cinco, digitados a mao, quando a
    /// tabela ja tinha dez. O dono contou olhando a tela.
    ///
    /// Devolve o que EXISTE, e nao a lista das dez: `.trash` so nasce quando
    /// alguem exclui, `.lgpd` so quando ha dado pessoal, `.pag` so com
    /// particao. Mostrar as dez sempre seria trocar um numero errado por
    /// outro -- a presenca do arquivo e a informacao.
    pub fn arquivos_da_tabela(&self, qualificado: &str) -> Result<Vec<String>> {
        let (schema, nome) = separar_qualificado(qualificado);
        let (schema, nome) = (schema.as_deref(), nome.as_str());
        validar_nome("tabela", nome)?;
        let dir = self.diretorio(schema)?;
        let mut achados = Vec::new();
        for ext in Self::EXTENSOES_TODAS {
            for arq in std::fs::read_dir(&dir)?.flatten() {
                let f = arq.file_name();
                let f = f.to_string_lossy();
                if pertence(&f, nome, ext) {
                    achados.push(format!(".{ext}"));
                    break; // um por extensao: volume nao vira item repetido.
                }
            }
        }
        Ok(achados)
    }

    /// Renomeia uma tabela: MOVE os arquivos, nao copia.
    ///
    /// # Por que as NOVE extensoes, e nao as seis da copia
    ///
    /// A diferenca entre copiar e renomear e a mesma que ha entre copiar e
    /// apagar: uma copia nao herda a lixeira da origem (decisao), mas um
    /// renomear leva TUDO -- deixar o `.trash` e o `.reason` no nome velho
    /// orfanaria o diario de exclusoes da propria tabela que se moveu.
    ///
    /// # Por que recusa quando alguem aponta para ela
    ///
    /// A chave estrangeira guarda a mae por NOME (`tabela_ref`), nao por
    /// identificador. Renomear uma tabela referenciada deixaria a filha
    /// apontando para um nome que nao existe mais -- e deixaria CALADO, que e
    /// o estrago que a regra primordial desta casa existe para impedir. Entao
    /// a recusa acontece antes de mover o primeiro arquivo, e ela DIZ qual
    /// tabela aponta, em vez de mandar procurar.
    ///
    /// Repare a divergencia proposital com o `conferir_filhas` do `excluir`,
    /// que pula a chave com `verificar: false`: la a pergunta e se a regra e
    /// IMPOSTA, porque o que sai e uma linha. Aqui o que sai e o NOME, e uma
    /// declaracao pendurada num nome inexistente e uma mentira sobre o modelo
    /// mesmo quando ninguem a confere. Por isso aqui nao se pula nenhuma.
    ///
    /// # O meio do caminho
    ///
    /// Os arquivos sao colhidos ANTES de o primeiro se mover, e um erro no
    /// meio desfaz os que ja foram: tabela partida entre dois nomes seria pior
    /// que renomear nenhum, porque nenhum dos dois nomes abriria.
    pub fn renomear_tabela(&self, origem: &str, destino: &str) -> Result<usize> {
        let (schema_o, nome_o) = separar_qualificado(origem);
        let (schema_d, nome_d) = separar_qualificado(destino);
        let (schema_o, nome_o) = (schema_o.as_deref(), nome_o.as_str());
        let (schema_d, nome_d) = (schema_d.as_deref(), nome_d.as_str());
        validar_nome("tabela", nome_o)?;
        validar_nome("tabela de destino", nome_d)?;
        if !self.existe_tabela(schema_o, nome_o)? {
            return Err(PhxError::NaoEncontrado(format!(
                "tabela {origem} nao existe em {}",
                self.nome()
            )));
        }
        if self.existe_tabela(schema_d, nome_d)? {
            return Err(PhxError::Duplicado(format!("a tabela {destino} ja existe")));
        }
        let dir_o = self.diretorio(schema_o)?;
        let dir_d = self.diretorio(schema_d)?;
        // Nome que o catalogo NAO leria de volta como ele mesmo faz a tabela
        // sumir. `pedidos_2025.reg` e indistinguivel do volume 2025 de uma
        // tabela `pedidos` -- o renomear funcionaria, moveria tudo, e a tabela
        // nao apareceria mais na arvore. Achado pela propria prova deste
        // renomear, que escolheu `pedidos_2025` sem pensar.
        //
        // A conferencia PERGUNTA em vez de reimplementar: quem sabe a regra do
        // sufixo de volume e o `nome_da_tabela`, e uma segunda copia da regra
        // divergiria dele calada.
        if nome_da_tabela(&dir_d.join(format!("{nome_d}.{EXT_REG}"))).as_deref() != Some(nome_d) {
            return Err(PhxError::Esquema(format!(
                "{destino} nao serve como nome de tabela: o catalogo o leria \
                 como um VOLUME de outra tabela (o sufixo `_` seguido so de \
                 digitos, ou de letra da particao, e reservado para isso), e a \
                 tabela renomeada sumiria da arvore. Escolha um nome que nao \
                 termine assim"
            )));
        }
        if let Some(filha) = self.quem_aponta_para(&dir_o, nome_o)? {
            return Err(PhxError::Integridade(format!(
                "a tabela {origem} nao pode ser renomeada: {filha} declara uma \
                 chave estrangeira para ela, e a chave guarda a mae pelo NOME \
                 -- renomear deixaria {filha} apontando para uma tabela que nao \
                 existe. Tire a chave de {filha}, renomeie, e declare de novo"
            )));
        }

        // Colher primeiro, mover depois: `read_dir` enquanto se renomeia dentro
        // do mesmo diretorio pode enxergar o nome novo.
        let mut mover: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
        for ext in Self::EXTENSOES_TODAS {
            for arq in std::fs::read_dir(&dir_o)?.flatten() {
                let f = arq.file_name();
                let f = f.to_string_lossy();
                if pertence(&f, nome_o, ext) {
                    // Preserva o sufixo do volume, como a copia faz:
                    // `precos_002.reg` vira `novo_002.reg`.
                    let novo = format!("{nome_d}{}", &f[nome_o.len()..]);
                    mover.push((arq.path(), dir_d.join(novo)));
                }
            }
        }
        let mut feitos: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
        for (de, para) in &mover {
            if let Err(e) = std::fs::rename(de, para) {
                // Desfaz na ordem inversa. Se o desfazer tambem falhar nao ha o
                // que fazer alem de nao piorar -- o erro que sai e o primeiro,
                // que e o que explica o que aconteceu.
                for (d, p) in feitos.iter().rev() {
                    let _ = std::fs::rename(p, d);
                }
                return Err(PhxError::Io(e));
            }
            feitos.push((de.clone(), para.clone()));
        }
        Ok(feitos.len())
    }

    /// A primeira tabela irma que declara chave estrangeira para `nome`.
    ///
    /// Devolve so a primeira: quem le a recusa precisa de UM nome para ir
    /// consertar, e listar todas seria conselho que ninguem segue de uma vez.
    fn quem_aponta_para(&self, dir: &std::path::Path, nome: &str) -> Result<Option<String>> {
        for irma in tabelas_em(dir)? {
            if irma == nome {
                continue;
            }
            // Irma que nao abre nao tranca o renomear: o defeito dela e dela, e
            // mistura-lo aqui faria uma tabela quebrada travar o banco inteiro.
            // E o mesmo julgamento que o `conferir_filhas` ja faz.
            let Ok(reg) = crate::reg::RegFile::abrir(dir, &irma) else {
                continue;
            };
            for fk in reg.esquema().chaves_estrangeiras() {
                let alvo = fk
                    .tabela_ref
                    .rsplit_once('.')
                    .map_or(fk.tabela_ref.as_str(), |(_, t)| t);
                if alvo == nome {
                    return Ok(Some(irma));
                }
            }
        }
        Ok(None)
    }

    /// Copia uma tabela inteira para outro nome, byte a byte.
    ///
    /// Copiar os arquivos preserva a ordem de digitacao e os rowids; reinserir
    /// linha a linha nao preservaria nem um nem outro.
    pub fn duplicar_tabela(&self, origem: &str, destino: &str) -> Result<usize> {
        self.exigir_motor_padrao()?;
        let (schema_o, nome_o) = separar_qualificado(origem);
        let (schema_d, nome_d) = separar_qualificado(destino);
        let (schema_o, nome_o) = (schema_o.as_deref(), nome_o.as_str());
        let (schema_d, nome_d) = (schema_d.as_deref(), nome_d.as_str());
        validar_nome("tabela", nome_o)?;
        validar_nome("tabela de destino", nome_d)?;
        if self.existe_tabela(schema_d, nome_d)? {
            return Err(PhxError::Duplicado(format!("a tabela {destino} ja existe")));
        }
        let dir_o = self.diretorio(schema_o)?;
        let dir_d = self.diretorio(schema_d)?;
        let mut copiados = 0usize;
        for ext in Self::EXTENSOES {
            for arq in std::fs::read_dir(&dir_o)?.flatten() {
                let f = arq.file_name();
                let f = f.to_string_lossy();
                if pertence(&f, nome_o, ext) {
                    // Preserva o sufixo do volume: `precos_002.reg` vira
                    // `copia_002.reg`, nao `copia.reg`.
                    let novo = format!("{nome_d}{}", &f[nome_o.len()..]);
                    std::fs::copy(arq.path(), dir_d.join(&novo))?;
                    copiados += 1;
                }
            }
        }
        if copiados == 0 {
            return Err(PhxError::NaoEncontrado(format!(
                "tabela {origem} nao existe em {}",
                self.nome()
            )));
        }
        Ok(copiados)
    }

    /// Copia uma tabela para OUTRO database -- o "colar" da tela.
    ///
    /// O `duplicar_tabela` copia dentro do mesmo database; este atravessa. E a
    /// mesma copia byte a byte, e pela mesma razao: a copia nasce com os
    /// mesmos rowids e na mesma ordem de digitacao.
    pub fn copiar_tabela_para(
        &self,
        origem: &str,
        destino_db: &Database,
        destino: &str,
    ) -> Result<usize> {
        // Os DOIS lados: colar de OU para uma colmeia e' tao invalido quanto
        // criar tabela nela. O motor padrao le e escreve tabela relacional, e
        // nenhuma das duas pontas pode ser de outro tipo.
        self.exigir_motor_padrao()?;
        destino_db.exigir_motor_padrao()?;
        let (schema_o, nome_o) = separar_qualificado(origem);
        let (schema_d, nome_d) = separar_qualificado(destino);
        let (schema_o, nome_o) = (schema_o.as_deref(), nome_o.as_str());
        let (schema_d, nome_d) = (schema_d.as_deref(), nome_d.as_str());
        validar_nome("tabela", nome_o)?;
        validar_nome("tabela de destino", nome_d)?;
        if destino_db.existe_tabela(schema_d, nome_d)? {
            return Err(PhxError::Duplicado(format!(
                "a tabela {destino} ja existe em {}",
                destino_db.nome()
            )));
        }
        let dir_o = self.diretorio(schema_o)?;
        // Colar num schema que ainda nao existe cria a pasta -- e o que quem
        // cola espera, e o mesmo que `criar_tabela` faz.
        let dir_d = match schema_d {
            None => destino_db.caminho().to_path_buf(),
            Some(sc) => destino_db.garantir_schema(sc)?,
        };
        if dir_o == dir_d && nome_o == nome_d {
            return Err(PhxError::Duplicado(
                "origem e destino sao a mesma tabela".into(),
            ));
        }

        let mut copiados = 0usize;
        for ext in Self::EXTENSOES {
            for arq in std::fs::read_dir(&dir_o)?.flatten() {
                let f = arq.file_name();
                let f = f.to_string_lossy();
                if pertence(&f, nome_o, ext) {
                    let novo = format!("{nome_d}{}", &f[nome_o.len()..]);
                    std::fs::copy(arq.path(), dir_d.join(&novo))?;
                    copiados += 1;
                }
            }
        }
        if copiados == 0 {
            return Err(PhxError::NaoEncontrado(format!(
                "tabela {origem} nao existe em {}",
                self.nome()
            )));
        }
        Ok(copiados)
    }

    pub fn existe_tabela(&self, schema: Option<&str>, nome: &str) -> Result<bool> {
        Ok(self.tabelas(schema)?.iter().any(|t| t == nome))
    }
}

/// Quebra `schema.tabela` em `(Some(schema), tabela)`. Sem ponto, o schema e
/// `None` e a tabela esta na raiz do database.
pub fn separar_qualificado(qualificado: &str) -> (Option<String>, String) {
    match qualificado.split_once('.') {
        Some((s, t)) if !s.is_empty() && !t.is_empty() => (Some(s.to_string()), t.to_string()),
        _ => (None, qualificado.to_string()),
    }
}

/// Monta o nome qualificado a partir das partes.
pub fn qualificar(schema: Option<&str>, tabela: &str) -> String {
    match schema {
        Some(s) => format!("{s}.{tabela}"),
        None => tabela.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::ColumnType;
    use phxsql_core::value::Value;

    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo. O helper velho nao
    // chamava `create_dir_all` (quem cria e' `Instancia::nova`, mais abaixo,
    // e ela mesma chama `create_dir_all`) -- pre-criar aqui e' idempotente e
    // nao muda o teste, so garante que o `Drop` tem o que limpar mesmo se
    // `Instancia::nova` nunca chegar a rodar.
    fn dir_temp(rotulo: &str) -> crate::apoio_teste::DirTemp {
        crate::apoio_teste::DirTemp::novo(&format!("cat-{rotulo}"))
    }

    fn esquema(nome: &str) -> Schema {
        Schema::new(
            nome,
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("descricao", ColumnType::Str(40)),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap()
    }

    #[test]
    fn hierarquia_database_schema_tabela() {
        let base = dir_temp("hierarquia");
        let inst = Instancia::nova(&base).unwrap();
        let z = inst.criar_database("Z").unwrap();

        // Tabela na raiz do database.
        z.criar_tabela(None, esquema("cadastroClientes")).unwrap();
        // Tabelas em dois schemas.
        z.criar_tabela(Some("X"), esquema("pedidos")).unwrap();
        z.criar_tabela(Some("X"), esquema("itens")).unwrap();
        z.criar_tabela(Some("Y"), esquema("notas")).unwrap();

        assert_eq!(inst.databases().unwrap(), vec!["Z"]);
        assert_eq!(z.schemas().unwrap(), vec!["X", "Y"]);
        assert_eq!(z.tabelas(None).unwrap(), vec!["cadastroClientes"]);
        assert_eq!(z.tabelas(Some("X")).unwrap(), vec!["itens", "pedidos"]);
        assert_eq!(z.tabelas(Some("Y")).unwrap(), vec!["notas"]);
        assert_eq!(
            z.todas_as_tabelas().unwrap(),
            vec!["X.itens", "X.pedidos", "Y.notas", "cadastroClientes"]
        );

        // O layout em disco e o do diagrama.
        assert!(base.join("Z/cadastroClientes.reg").exists());
        assert!(base.join("Z/X/pedidos.reg").exists());
        assert!(base.join("Z/Y/notas.reg").exists());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn mesmo_nome_em_schemas_diferentes_nao_colide() {
        let base = dir_temp("homonimas");
        let inst = Instancia::nova(&base).unwrap();
        let z = inst.criar_database("Z").unwrap();

        let mut a = z.criar_tabela(Some("X"), esquema("pedidos")).unwrap();
        let mut b = z.criar_tabela(Some("Y"), esquema("pedidos")).unwrap();
        a.inserir(&[Value::Int(1), Value::Str("do X".into())])
            .unwrap();
        b.inserir(&[Value::Int(1), Value::Str("do Y".into())])
            .unwrap();
        b.inserir(&[Value::Int(2), Value::Str("so do Y".into())])
            .unwrap();

        assert_eq!(a.registros(), 1);
        assert_eq!(b.registros(), 2);

        let mut lida = z.abrir_qualificada("X.pedidos").unwrap();
        assert_eq!(lida.ler(1).unwrap().unwrap()[1], Value::Str("do X".into()));
        let mut lida = z.abrir_qualificada("Y.pedidos").unwrap();
        assert_eq!(lida.ler(1).unwrap().unwrap()[1], Value::Str("do Y".into()));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn databases_separados_nao_se_enxergam() {
        let base = dir_temp("bancos");
        let inst = Instancia::nova(&base).unwrap();
        let z = inst.criar_database("Z").unwrap();
        let w = inst.criar_database("W").unwrap();
        z.criar_tabela(None, esquema("clientes")).unwrap();
        w.criar_tabela(None, esquema("fornecedores")).unwrap();

        assert_eq!(z.tabelas(None).unwrap(), vec!["clientes"]);
        assert_eq!(w.tabelas(None).unwrap(), vec!["fornecedores"]);
        assert!(z.abrir_tabela(None, "fornecedores").is_err());
        assert_eq!(inst.databases().unwrap(), vec!["W", "Z"]);
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn database_nasce_com_o_tipo_declarado_e_le_de_volta() {
        let base = dir_temp("tipos");
        let inst = Instancia::nova(&base).unwrap();
        inst.criar_database_com_tipo("cfg", TipoDatabase::Hive)
            .unwrap();
        inst.criar_database_com_tipo("emb", TipoDatabase::Vetorial)
            .unwrap();
        inst.criar_database("rel").unwrap(); // sem tipo = padrao
                                             // Reabre: o tipo vem do marcador NO DISCO, nao da memoria de quem criou.
        assert_eq!(
            inst.abrir_database("cfg").unwrap().tipo(),
            TipoDatabase::Hive
        );
        assert_eq!(
            inst.abrir_database("emb").unwrap().tipo(),
            TipoDatabase::Vetorial
        );
        assert_eq!(
            inst.abrir_database("rel").unwrap().tipo(),
            TipoDatabase::Padrao
        );
    }

    #[test]
    fn database_sem_marcador_e_padrao_por_compatibilidade() {
        // Simula um database criado ANTES desta decisao: diretorio cru, sem o
        // _database.json. Tem de abrir como padrao, sem migracao e sem recusa.
        let base = dir_temp("antigo");
        let inst = Instancia::nova(&base).unwrap();
        std::fs::create_dir_all(base.join("legado")).unwrap();
        assert!(!base.join("legado").join(MARCA_DATABASE).exists());
        assert_eq!(
            inst.abrir_database("legado").unwrap().tipo(),
            TipoDatabase::Padrao
        );
    }

    #[test]
    fn marcador_corrompido_cai_em_padrao_nao_recusa_abrir() {
        // Prova real do ramo de defeito: tipo invalido no marcador NAO pode
        // travar a abertura de um banco que existe -- cai em padrao.
        let base = dir_temp("corrompido");
        let inst = Instancia::nova(&base).unwrap();
        inst.criar_database_com_tipo("x", TipoDatabase::Hive)
            .unwrap();
        std::fs::write(base.join("x").join(MARCA_DATABASE), "{\"tipo\":\"grafo\"}").unwrap();
        assert_eq!(
            inst.abrir_database("x").unwrap().tipo(),
            TipoDatabase::Padrao
        );
    }

    #[test]
    fn hive_recusa_operacao_de_tabela_mas_padrao_funciona() {
        // PROVA REAL nos dois sentidos. O motor padrao (tabela relacional) so
        // opera database padrao:
        // - com o portao (conserto): criar/abrir tabela numa colmeia RECUSA,
        //   "em construcao"; num padrao PASSA.
        // - sem o portao (defeito reposto): a colmeia criaria um `.reg`
        //   relacional calada -- "fingir que gravou", metade pior que nada.
        //   Este teste fica VERMELHO se alguem tirar o `exigir_motor_padrao`
        //   de `criar_tabela` (ou de `abrir_tabela`).
        let base = dir_temp("motor-em-obra");
        let inst = Instancia::nova(&base).unwrap();

        let colmeia = inst
            .criar_database_com_tipo("cfg", TipoDatabase::Hive)
            .unwrap();
        // `.err()` em vez de `unwrap_err()`: o lado Ok e' `Table`, que nao
        // implementa `Debug`, entao `unwrap_err` nem compilaria.
        let erro = colmeia
            .criar_tabela(None, esquema("clientes"))
            .err()
            .expect("criar tabela numa colmeia tinha de recusar");
        assert!(
            matches!(erro, PhxError::Esquema(ref m) if m.contains("construcao")),
            "esperava recusa de motor em construcao, veio {erro:?}"
        );
        // E de fato NADA nasceu: nenhum `.reg` relacional no diretorio da
        // colmeia. Listar continua valendo -- a colmeia existe e aparece vazia.
        assert!(colmeia.tabelas(None).unwrap().is_empty());
        // abrir tambem recusa pelo MESMO motivo, antes do «nao existe»: o
        // portao fira antes de procurar o arquivo.
        let abrir = colmeia
            .abrir_tabela(None, "clientes")
            .err()
            .expect("abrir numa colmeia tinha de recusar");
        assert!(
            matches!(abrir, PhxError::Esquema(ref m) if m.contains("construcao")),
            "abrir numa colmeia tinha de recusar por motor, veio {abrir:?}"
        );

        // O padrao, ao lado, opera como sempre -- o portao so barra o que nao
        // e padrao.
        let rel = inst.criar_database("rel").unwrap();
        rel.criar_tabela(None, esquema("clientes")).unwrap();
        assert_eq!(rel.tabelas(None).unwrap(), vec!["clientes"]);
    }

    #[test]
    fn tabela_paginada_aparece_uma_vez_so_na_listagem() {
        let base = dir_temp("paginada");
        let inst = Instancia::nova(&base).unwrap();
        let z = inst.criar_database("Z").unwrap();
        let esq = esquema("grande")
            .com_paginacao(phxsql_core::paginacao::Paginacao::nova(2, 99).unwrap())
            .unwrap();
        let mut t = z.criar_tabela(None, esq).unwrap();
        for i in 1..=7i64 {
            t.inserir(&[Value::Int(i), Value::Null]).unwrap();
        }
        // 4 volumes de .reg, mas uma unica tabela na listagem.
        assert!(base.join("Z/grande_004.reg").exists());
        assert_eq!(z.tabelas(None).unwrap(), vec!["grande"]);
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn nomes_perigosos_sao_recusados() {
        let base = dir_temp("seguranca");
        let inst = Instancia::nova(&base).unwrap();
        assert!(inst.criar_database("..").is_err());
        assert!(inst.criar_database("a/b").is_err());
        assert!(inst.criar_database("a\\b").is_err());
        assert!(inst.criar_database("").is_err());
        let z = inst.criar_database("Z").unwrap();
        assert!(z.criar_schema("../fora").is_err());
        assert!(z.abrir_tabela(Some(".."), "x").is_err());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn nome_qualificado_vai_e_volta() {
        assert_eq!(
            separar_qualificado("X.pedidos"),
            (Some("X".to_string()), "pedidos".to_string())
        );
        assert_eq!(
            separar_qualificado("cadastroClientes"),
            (None, "cadastroClientes".to_string())
        );
        assert_eq!(qualificar(Some("X"), "pedidos"), "X.pedidos");
        assert_eq!(qualificar(None, "pedidos"), "pedidos");
    }

    /// **A tabela que nao existe e nomeada, e o caminho do disco nao sai.**
    ///
    /// O erro cru -- «nenhum volume de x.reg em /tmp/.../Z» -- publicava o
    /// caminho absoluto do servidor a todo cliente que errasse uma letra no
    /// `FROM`, e mandava procurar arquivo em vez de conferir o nome. A
    /// tradução so vale para a tabela AUSENTE: uma tabela que perdeu um
    /// arquivo continua com o erro que nomeia o arquivo, porque chama-la de
    /// inexistente faria alguem criar outra por cima da quebrada.
    #[test]
    fn a_tabela_que_nao_existe_e_nomeada_sem_o_caminho_do_disco() {
        let base = dir_temp("sem-caminho");
        let inst = Instancia::nova(&base).unwrap();
        let z = inst.criar_database("Z").unwrap();
        z.criar_tabela(None, esquema("clientes")).unwrap();
        z.criar_tabela(Some("X"), esquema("pedidos")).unwrap();
        let caminho = base.display().to_string();

        // As duas fichas dizem a mesma coisa, na raiz e dentro de um schema.
        let raiz = Raiz::nova(&base).unwrap();
        for nome in ["inventada", "X.inventada"] {
            let Err(e) = z.abrir_qualificada(nome) else {
                panic!("{nome} abriu");
            };
            let t = e.to_string();
            assert!(matches!(e, PhxError::NaoEncontrado(_)), "{t}");
            assert!(
                t.contains(&format!("a tabela {nome} nao existe em Z")),
                "{t}"
            );
            assert!(!t.contains(&caminho), "vazou o caminho: {t}");
            let Err(e) = raiz.abrir_para_ler("Z", nome) else {
                panic!("{nome} abriu para ler");
            };
            let t = e.to_string();
            assert!(
                t.contains(&format!("a tabela {nome} nao existe em Z")),
                "{t}"
            );
            assert!(!t.contains(&caminho), "vazou o caminho: {t}");
        }
        // O database que nao existe tambem nao publica o diretorio da base.
        let Err(e) = inst.abrir_database("W") else {
            panic!("W abriu");
        };
        let t = e.to_string();
        assert!(t.contains("database W nao existe"), "{t}");
        assert!(!t.contains(&caminho), "vazou o caminho: {t}");

        // A tabela que PERDEU um arquivo existe -- o `.reg` esta la -- e o
        // erro continua sendo o do componente, e nao «nao existe».
        std::fs::remove_file(base.join("Z/clientes.ndx")).unwrap();
        let Err(e) = z.abrir_qualificada("clientes") else {
            panic!("a tabela sem .ndx abriu");
        };
        let t = e.to_string();
        assert!(
            !t.contains("nao existe em Z"),
            "tabela quebrada virou inexistente: {t}"
        );
    }

    #[test]
    fn nome_de_tabela_ignora_sufixo_de_volume() {
        assert_eq!(
            nome_da_tabela(Path::new("cadastroClientes.reg")).as_deref(),
            Some("cadastroClientes")
        );
        assert_eq!(
            nome_da_tabela(Path::new("cadastroClientes_007.reg")).as_deref(),
            Some("cadastroClientes")
        );
        // Sublinhado que nao e sufixo de volume fica no nome.
        assert_eq!(
            nome_da_tabela(Path::new("cadastro_clientes.reg")).as_deref(),
            Some("cadastro_clientes")
        );
        assert_eq!(nome_da_tabela(Path::new("cadastroClientes.ndx")), None);
    }
    #[test]
    fn separa_nome_ruim_de_tentativa_de_travessia() {
        // Engano de digitacao: recusado, mas nao e ataque.
        assert!(!nome_hostil("minha tabela!"));
        assert!(!nome_hostil("cadastro*"));
        assert!(!nome_hostil("aspas\"aqui"));
        assert!(!nome_hostil("cadastroClientes"));
        assert!(!nome_hostil("Comercial"));
        assert!(!nome_hostil("nota.fiscal"));

        // Sondagem: ninguem digita isso por acidente.
        assert!(nome_hostil(".."));
        assert!(nome_hostil("."));
        assert!(nome_hostil("../../etc/passwd"));
        assert!(nome_hostil("..\\..\\windows"));
        assert!(nome_hostil("/etc"));
        assert!(nome_hostil("C:\\dados"));
        assert!(nome_hostil("a/b"));
        assert!(nome_hostil("nome\u{0}nulo"));
        assert!(nome_hostil("quebra\nlinha"));
        // Sem barra, mas ainda saindo do lugar.
        assert!(nome_hostil("tabela..oculta"));
    }

    #[test]
    fn tudo_que_e_hostil_tambem_e_invalido() {
        // O contrario nao vale, e e essa a assimetria que interessa.
        for n in ["..", "/etc", "a/b", "C:\\x", "quebra\nlinha"] {
            assert!(nome_hostil(n));
            assert!(
                validar_nome("tabela", n).is_err(),
                "{n:?} deveria ser invalido"
            );
        }
    }
}

#[cfg(test)]
mod testes_gestao {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};

    #[test]
    fn pertence_nao_confunde_tabela_de_prefixo_igual() {
        // O caso que apagaria a tabela errada: `precos_historico` comeca com
        // `precos`. Sem conferir o sufixo, excluir `precos` levaria as duas.
        assert!(pertence("precos.reg", "precos", "reg"));
        assert!(pertence("precos_001.reg", "precos", "reg"));
        assert!(pertence("precos_00042.reg", "precos", "reg"));

        assert!(!pertence("precos_historico.reg", "precos", "reg"));
        assert!(!pertence("precos2.reg", "precos", "reg"));
        assert!(!pertence("precos_.reg", "precos", "reg"));
        assert!(!pertence("precos_1a.reg", "precos", "reg"));
        assert!(!pertence("precos.ndx", "precos", "reg"), "extensao errada");
        assert!(!pertence("outra.reg", "precos", "reg"));
    }

    #[test]
    fn qualificado_se_parte_em_schema_e_nome() {
        let parte = |q: &str| {
            let (e, n) = separar_qualificado(q);
            (e, n)
        };
        assert_eq!(parte("clientes"), (None, "clientes".into()));
        assert_eq!(
            parte("vendas.pedidos"),
            (Some("vendas".into()), "pedidos".into())
        );
        // Ponto solto nao vira schema vazio: o nome inteiro fica sendo a
        // tabela, e o `validar_nome` recusa depois. E o que impede um
        // ".reg" de virar caminho.
        assert_eq!(parte(".pedidos"), (None, ".pedidos".into()));
        assert_eq!(parte("vendas."), (None, "vendas.".into()));
    }

    fn esquema_simples(nome: &str) -> Schema {
        use phxsql_core::types::ColumnType;
        Schema::new(
            nome,
            vec![Column::new("id", ColumnType::Int8).obrigatoria()],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap()
    }

    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    fn base_temp(rotulo: &str) -> crate::apoio_teste::DirTemp {
        crate::apoio_teste::DirTemp::novo(&format!("cat2-{rotulo}"))
    }

    #[test]
    fn excluir_tabela_leva_os_arquivos_dela_e_so_os_dela() {
        let base = base_temp("excluir");
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("Z").unwrap();
        db.criar_tabela(None, esquema_simples("precos")).unwrap();
        db.criar_tabela(None, esquema_simples("precos_historico"))
            .unwrap();

        let apagados = db.excluir_tabela("precos").unwrap();
        assert!(!apagados.is_empty());
        assert!(
            apagados.iter().all(|a| a.starts_with("precos.")),
            "levou arquivo que nao era: {apagados:?}"
        );

        assert!(!db.existe_tabela(None, "precos").unwrap());
        assert!(
            db.existe_tabela(None, "precos_historico").unwrap(),
            "a tabela de prefixo igual foi junto"
        );
    }

    /// O defeito: a lista de extensoes do `excluir_tabela` tinha SEIS e a
    /// tabela ja tinha NOVE. O `.trash`, o `.reason` e o `.pag` ficavam para
    /// tras, e o estrago era duplo — recriar a tabela com o mesmo nome passava
    /// a ser IMPOSSIVEL, e o dado de uma tabela excluida sobrevivia num
    /// arquivo que so `administrar` deveria abrir.
    ///
    /// A prova e o CICLO: criar, excluir e criar de novo com o mesmo nome. Ele
    /// falha com a lista velha e passa com a nova, e nao depende de eu lembrar
    /// quais sao as extensoes — quem sabe isso e o `Table::criar`, que confere
    /// todas antes de escrever a primeira.
    #[test]
    fn excluir_tabela_deixa_o_nome_livre_para_a_proxima() {
        let base = base_temp("excluir-ciclo");
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("Z").unwrap();
        db.criar_tabela(None, esquema_simples("precos")).unwrap();
        let dir = base.join("Z");

        // A tabela nasce com mais que os quatro arquivos do folclore.
        let antes: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|f| f.starts_with("precos."))
            .collect();
        assert!(
            antes.iter().any(|f| f.ends_with(".trash")),
            "esperava o .trash entre os arquivos da tabela: {antes:?}"
        );

        db.excluir_tabela("precos").unwrap();
        let sobrou: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|f| f.starts_with("precos."))
            .collect();
        assert!(
            sobrou.is_empty(),
            "excluir_tabela deixou para tras: {sobrou:?}"
        );

        // E o que importa para quem usa: o nome volta a estar livre.
        db.criar_tabela(None, esquema_simples("precos"))
            .expect("recriar com o mesmo nome tem de funcionar depois de excluir");
    }

    #[test]
    fn excluir_tabela_que_nao_existe_e_erro() {
        let base = base_temp("excluir-ausente");
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("Z").unwrap();
        assert!(db.excluir_tabela("naoexiste").is_err());
    }

    #[test]
    fn duplicar_preserva_os_rowids_e_a_ordem() {
        use phxsql_core::value::Value;
        let base = base_temp("duplicar");
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("Z").unwrap();
        let mut t = db.criar_tabela(None, esquema_simples("precos")).unwrap();
        for i in 1..=5i64 {
            t.inserir(&[Value::Int(i * 10)]).unwrap();
        }
        t.excluir(3).unwrap(); // um buraco no meio
        t.sincronizar().unwrap();
        drop(t);

        let copiados = db.duplicar_tabela("precos", "copia").unwrap();
        assert!(copiados >= 5, "copiou {copiados} arquivos");

        // A copia tem os MESMOS rowids, o mesmo buraco e a mesma ordem. Uma
        // reinsercao linha a linha renumeraria tudo.
        let mut c = db.abrir_tabela(None, "copia").unwrap();
        assert_eq!(c.ler(1).unwrap().unwrap()[0], Value::Int(10));
        assert!(
            c.ler(3).unwrap().is_none(),
            "o slot excluido nao foi copiado"
        );
        assert_eq!(c.ler(5).unwrap().unwrap()[0], Value::Int(50));

        // E o original continua inteiro.
        let mut o = db.abrir_tabela(None, "precos").unwrap();
        assert_eq!(o.ler(5).unwrap().unwrap()[0], Value::Int(50));
    }

    #[test]
    fn duplicar_para_nome_que_ja_existe_e_recusado() {
        let base = base_temp("duplicar-ocupado");
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("Z").unwrap();
        db.criar_tabela(None, esquema_simples("a")).unwrap();
        db.criar_tabela(None, esquema_simples("b")).unwrap();
        assert!(
            db.duplicar_tabela("a", "b").is_err(),
            "sobrescreveu a tabela b"
        );
    }
}

#[cfg(test)]
mod testes_copia_entre_bancos {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::ColumnType;
    use phxsql_core::value::Value;

    fn esquema(nome: &str) -> Schema {
        Schema::new(
            nome,
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("texto", ColumnType::Str(20)),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria()],
        )
        .unwrap()
    }

    /// **A lista das extensoes esta completa?** -- a pergunta que ja falhou
    /// DUAS vezes aqui, e as duas com o dono olhando a TELA.
    ///
    /// A lista nasceu com seis, a tabela tinha nove, e o comentario do
    /// `EXTENSOES_TODAS` conta o estrago. O `.lgpd` entrou depois disso e
    /// ninguem voltou aqui: `excluir_tabela` deixava a trilha de dados
    /// PESSOAIS para tras, sob um nome que nao existe mais -- e uma tabela
    /// nova com o mesmo nome herdaria a trilha de uma tabela alheia, que e
    /// conteudo de linha e so `administrar` pode ler.
    ///
    /// A segunda vez foi o `.fts` (pedido 213, 07/09/2026): entrou no motor
    /// meses depois do `.lgpd` e ninguem voltou aqui de novo, a MESMA
    /// armadilha. `pedidos.fts` e criado a mao abaixo pela mesma razao do
    /// `.lgpd` -- a tabela de duas colunas deste teste nao declara indice de
    /// texto nenhum, entao o motor nunca criaria um sozinho.
    ///
    /// Este teste nao confere a LISTA: confere o DIRETORIO. Conferir a lista
    /// contra ela mesma passaria com qualquer numero.
    #[test]
    fn excluir_tabela_nao_deixa_arquivo_nenhum_para_tras() {
        use phxsql_core::value::Value;
        // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
        let base = crate::apoio_teste::DirTemp::novo("phx-ext");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();
        let mut t = db.criar_tabela(None, esquema("pedidos")).unwrap();
        t.inserir(&[Value::Int(1), Value::Str("um".into())])
            .unwrap();
        // A trilha LGPD so nasce quando o primeiro evento aparece -- por isso
        // o arquivo e criado a mao aqui, que e o que o motor faria.
        std::fs::write(base.join("loja/pedidos.lgpd"), b"PLGP").unwrap();
        // Idem para o `.fts`: so nasce se o esquema declara indice de texto,
        // e este nao declara.
        std::fs::write(base.join("loja/pedidos.fts"), b"PHXNDX\0\0").unwrap();
        drop(t);

        db.excluir_tabela("pedidos").unwrap();
        let sobrou: Vec<String> = std::fs::read_dir(base.join("loja"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|f| f.starts_with("pedidos."))
            .collect();
        assert!(
            sobrou.is_empty(),
            "excluir_tabela deixou para tras: {sobrou:?} -- a lista de extensoes \
             ficou para tras de novo"
        );
    }

    /// `arquivos_da_tabela` e o inventario que a TELA le -- se ele nao ve o
    /// `.fts`, a tela diz "sem indice de texto" para uma tabela que tem um.
    /// Com "fts" fora de `EXTENSOES_TODAS` este teste falha (o vetor nao
    /// contem ".fts"); com "fts" dentro, passa. E a prova direta de que
    /// `extensoes_de_uma_tabela()` -- o que o conferidor do pedido 213 le --
    /// bate com o que este metodo relata.
    #[test]
    fn arquivos_da_tabela_enxerga_o_fts() {
        let base = crate::apoio_teste::DirTemp::novo("phx-fts-inv");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();
        db.criar_tabela(None, esquema("pedidos")).unwrap();
        std::fs::write(base.join("loja/pedidos.fts"), b"PHXNDX\0\0").unwrap();

        let achados = db.arquivos_da_tabela("pedidos").unwrap();
        assert!(
            achados.iter().any(|e| e == ".fts"),
            "arquivos_da_tabela nao achou o .fts: {achados:?}"
        );
        assert!(
            Database::extensoes_de_uma_tabela().contains(&"fts"),
            "extensoes_de_uma_tabela() (a lista que o conferidor do pedido \
             213 le) nao tem \"fts\""
        );
    }

    /// O renomear MOVE: o nome velho some, o novo abre, e o dado e o mesmo.
    #[test]
    fn renomear_move_a_tabela_inteira() {
        // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
        let base = crate::apoio_teste::DirTemp::novo("phx-renom");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();
        let mut t = db.criar_tabela(None, esquema("pedidos")).unwrap();
        for (i, txt) in ["um", "dois", "tres"].iter().enumerate() {
            t.inserir(&[Value::Int(i as i64 + 1), Value::Str((*txt).into())])
                .unwrap();
        }
        // Um furo, para provar que o renomear nao renumera nada: ele MOVE.
        t.excluir(2).unwrap();
        // O mesmo `.fts` de mentira do teste de excluir: prova que o
        // renomear tambem nao esquecia mais dele (pedido 213).
        std::fs::write(base.join("loja/pedidos.fts"), b"PHXNDX\0\0").unwrap();
        drop(t);

        let movidos = db.renomear_tabela("pedidos", "pedidos_arquivados").unwrap();
        let listadas = db.tabelas(None).unwrap();
        assert!(
            !db.existe_tabela(None, "pedidos").unwrap(),
            "moveu {movidos} arquivo(s), mas `tabelas()` ainda lista: {listadas:?}"
        );

        // **NADA do nome velho pode sobrar.** Conferir so o `.reg` deixava
        // este teste passar com o renomear movendo as SEIS extensoes da copia
        // em vez das NOVE -- e o `.trash`, o `.reason` e o `.pag` ficariam
        // para tras, orfaos, sob um nome que nao existe mais. Foi a prova real
        // que pegou: reposto o defeito, o teste continuava verde. O `.fts`
        // (pedido 213) repetiu o mesmo defeito meses depois.
        let sobrou: Vec<String> = std::fs::read_dir(base.join("loja"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|f| f.starts_with("pedidos."))
            .collect();
        assert!(
            sobrou.is_empty(),
            "ficou para tras com o nome velho: {sobrou:?}"
        );
        assert!(db.existe_tabela(None, "pedidos_arquivados").unwrap());

        // O dado e o mesmo, com o furo no mesmo lugar -- renomear nao toca na
        // ordem de digitacao, que e pretrea.
        let mut nova = db.abrir_tabela(None, "pedidos_arquivados").unwrap();
        let rowids: Vec<u64> = nova.varrer().unwrap().into_iter().map(|(r, _)| r).collect();
        assert_eq!(rowids, vec![1, 3], "o furo tinha de continuar onde estava");
    }

    /// **A recusa que este renomear existe para ter.**
    ///
    /// A chave estrangeira guarda a mae por NOME. Renomear a mae deixaria a
    /// filha apontando para uma tabela que nao existe -- e calada. A recusa
    /// acontece antes de mover o primeiro arquivo, e diz QUEM aponta.
    #[test]
    fn renomear_recusa_a_mae_que_tem_filha_apontando() {
        use phxsql_core::schema::ForeignKey;
        // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
        let base = crate::apoio_teste::DirTemp::novo("phx-renom-fk");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();
        db.criar_tabela(None, esquema("clientes")).unwrap();

        let filha = Schema::new(
            "pedidos",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("cliente", ColumnType::Int8),
            ],
            vec![
                IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria(),
                // A chave conferida exige indice dos DOIS lados -- sem este, o
                // motor recusa a declaracao antes de este teste chegar ao ponto.
                IndexDef::new("porCliente", vec![IndexColumn::asc(1)]),
            ],
        )
        .unwrap()
        .com_chaves_estrangeiras(vec![ForeignKey::new(
            "fk_cliente",
            vec![1],
            "clientes",
            vec!["id".into()],
        )])
        .unwrap();
        db.criar_tabela(None, filha).unwrap();

        let e = db
            .renomear_tabela("clientes", "clientes_arquivados")
            .unwrap_err();
        let t = e.to_string();
        assert!(
            t.contains("pedidos"),
            "a recusa tem de DIZER quem aponta: {t}"
        );
        // E nada se moveu: recusar depois de mover metade seria pior.
        assert!(db.existe_tabela(None, "clientes").unwrap());
        assert!(!db.existe_tabela(None, "clientes_arquivados").unwrap());
    }

    /// **A regra primordial no nivel da TABELA.**
    ///
    /// Medido por sonda antes desta guarda: o `excluir_tabela` apagava os oito
    /// arquivos da mae e a filha ficava com a linha intacta apontando para o
    /// vazio. O `renomear_tabela` recusava o MESMO cenario -- o motor sabia
    /// fazer a pergunta e nao a fazia aqui.
    #[test]
    fn excluir_recusa_a_mae_que_tem_filha_apontando() {
        // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
        let base = crate::apoio_teste::DirTemp::novo("phx-drop-fk");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();
        db.criar_tabela(None, esquema("clientes")).unwrap();
        db.criar_tabela(None, filha_de_clientes()).unwrap();

        let e = db.excluir_tabela("clientes").unwrap_err();
        let t = e.to_string();
        assert!(
            matches!(e, PhxError::Integridade(_)),
            "tinha de ser integridade: {e:?}"
        );
        assert!(
            t.contains("pedidos"),
            "a recusa tem de DIZER quem aponta: {t}"
        );
        // E nenhum arquivo saiu: recusar depois de apagar metade nao e recusar.
        assert!(db.existe_tabela(None, "clientes").unwrap());
        assert!(
            !db.arquivos_da_tabela("clientes").unwrap().is_empty(),
            "a mae ficou sem arquivo nenhum"
        );
    }

    /// O conserto que a recusa manda fazer FUNCIONA -- e este teste e o que
    /// impede o de cima de "passar" com um portao que recusasse todo apagar.
    #[test]
    fn apagada_a_filha_a_mae_sai_normalmente() {
        // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
        let base = crate::apoio_teste::DirTemp::novo("phx-drop-fk-ok");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();
        db.criar_tabela(None, esquema("clientes")).unwrap();
        db.criar_tabela(None, filha_de_clientes()).unwrap();

        db.excluir_tabela("pedidos").expect("a filha sai sozinha");
        db.excluir_tabela("clientes")
            .expect("sem filha, a mae sai como sempre saiu");
        assert!(!db.existe_tabela(None, "clientes").unwrap());
    }

    /// A tabela sem ninguem apontando continua saindo -- o controle do portao.
    #[test]
    fn excluir_tabela_sem_chave_nenhuma_nao_muda_nada() {
        // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
        let base = crate::apoio_teste::DirTemp::novo("phx-drop-livre");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();
        db.criar_tabela(None, esquema("avulsa")).unwrap();
        db.excluir_tabela("avulsa")
            .expect("ninguem aponta para ela");
    }

    /// **A copia entre bancos NAO confere, e isto e decisao escrita.**
    ///
    /// Colar a filha num database onde a mae ainda nao esta e ordem legitima
    /// de trabalho -- cola-se uma, cola-se a outra --, e recusar aqui obrigaria
    /// uma ordem que a tela nao tem como impor. A copia e byte a byte: ela
    /// preserva a ordem de digitacao e os rowids, e reinserir linha a linha
    /// para conferir perderia os dois.
    ///
    /// O que a decisao custa esta MEDIDO neste teste em vez de suposto: a
    /// tabela colada nasce com orfa. O que a torna aceitavel sao as duas
    /// saidas que existem depois -- o motor RECUSA a proxima gravacao dizendo
    /// que a tabela mae nao existe naquele banco, e o verificador de
    /// consistencia acha a orfa sem que ninguem precise desconfiar dela.
    ///
    /// Trocar isto por uma recusa exige numero: quantas colagens legitimas
    /// quebrariam contra quantas orfas evitadas. Sem esse numero, a recusa
    /// seria preferencia.
    #[test]
    fn colar_a_filha_sem_a_mae_passa_e_o_verificador_acha_a_orfa() {
        // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
        let base = crate::apoio_teste::DirTemp::novo("phx-colar-fk");
        let cat = Instancia::nova(&base).unwrap();
        let origem = cat.criar_database("loja").unwrap();
        let destino = cat.criar_database("vazio").unwrap();
        origem.criar_tabela(None, esquema("clientes")).unwrap();
        origem.criar_tabela(None, filha_de_clientes()).unwrap();
        {
            let mut m = crate::table::Table::abrir(origem.caminho(), "clientes").unwrap();
            m.inserir(&[Value::Int(1), Value::Str("Ana".into())])
                .unwrap();
            m.sincronizar().unwrap();
            let mut f = crate::table::Table::abrir(origem.caminho(), "pedidos").unwrap();
            f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
            f.sincronizar().unwrap();
        }
        // A origem esta limpa.
        assert!(crate::integridade::conferir_diretorio(origem.caminho())
            .unwrap()
            .limpo());

        origem
            .copiar_tabela_para("pedidos", &destino, "pedidos")
            .expect("colar a filha sozinha passa: colar a mae depois e ordem legitima");

        // E a copia nasce com orfa -- dito por um numero, e nao por suposicao.
        let r = crate::integridade::conferir_diretorio(destino.caminho()).unwrap();
        assert_eq!(r.violacoes.len(), 1, "{:?}", r.violacoes);
        assert_eq!(
            r.violacoes[0].falha,
            crate::integridade::Falha::TabelaMaeAusente
        );

        // A outra saida: a proxima gravacao ali RECUSA, e diz o que falta.
        let mut f = crate::table::Table::abrir(destino.caminho(), "pedidos").unwrap();
        let e = f.inserir(&[Value::Int(11), Value::Int(1)]).unwrap_err();
        assert!(e.to_string().contains("nao existe neste banco"), "{e}");
    }

    /// A filha de `clientes`, com o indice dos dois lados que a chave exige.
    fn filha_de_clientes() -> Schema {
        use phxsql_core::schema::ForeignKey;
        Schema::new(
            "pedidos",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("cliente", ColumnType::Int8),
            ],
            vec![
                IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria(),
                IndexDef::new("porCliente", vec![IndexColumn::asc(1)]),
            ],
        )
        .unwrap()
        .com_chaves_estrangeiras(vec![ForeignKey::new(
            "fk_cliente",
            vec![1],
            "clientes",
            vec!["id".into()],
        )])
        .unwrap()
    }

    /// **O achado que a propria prova deste renomear trouxe.**
    ///
    /// A primeira versao do teste acima renomeava para `pedidos_2025`. Moveu
    /// os oito arquivos, devolveu sucesso -- e a tabela SUMIU da arvore, porque
    /// `nome_da_tabela` le `pedidos_2025.reg` como o volume 2025 de uma tabela
    /// `pedidos`. Renomear com sucesso e perder a tabela e o pior par possivel:
    /// nada avisa.
    ///
    /// A recusa nao reimplementa a regra do sufixo -- ela PERGUNTA ao
    /// `nome_da_tabela`, que e quem a possui. Uma segunda copia da regra
    /// divergiria dele calada, que e a mesma armadilha da receita de um numero
    /// que envelhece.
    #[test]
    fn renomear_recusa_nome_que_o_catalogo_leria_como_volume() {
        // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
        let base = crate::apoio_teste::DirTemp::novo("phx-renom-vol");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();
        db.criar_tabela(None, esquema("pedidos")).unwrap();

        // `_` + so digitos: o sufixo de volume numerico.
        let e = db.renomear_tabela("pedidos", "pedidos_2025").unwrap_err();
        assert!(e.to_string().contains("VOLUME"), "{e}");
        // A origem tem de continuar inteira: recusar depois de mover seria pior.
        assert!(db.existe_tabela(None, "pedidos").unwrap());

        // A regra e mais AMPLA do que parecia, e o teste corrigiu a suposicao:
        // `pedidos_de_2025` tambem e recusado, porque o que conta e o ultimo
        // trecho depois do `_` ser so digitos -- ele seria lido como o volume
        // 2025 de uma tabela `pedidos_de`. Nao e so o nome curto que colide.
        assert!(db.renomear_tabela("pedidos", "pedidos_de_2025").is_err());

        // E o nome que de fato NAO e ambiguo passa -- senao a guarda recusaria
        // tudo, que e o defeito irmao de uma guarda boa demais.
        assert!(db.renomear_tabela("pedidos", "pedidos_de_janeiro").is_ok());
    }

    /// Destino ocupado recusa, e a origem fica intata.
    #[test]
    fn renomear_recusa_destino_que_ja_existe() {
        // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
        let base = crate::apoio_teste::DirTemp::novo("phx-renom-ocup");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();
        db.criar_tabela(None, esquema("a")).unwrap();
        db.criar_tabela(None, esquema("b")).unwrap();
        assert!(db.renomear_tabela("a", "b").is_err());
        assert!(db.existe_tabela(None, "a").unwrap(), "a origem sumiu");
    }

    /// A copia entre databases preserva rowid e ordem de digitacao -- que e o
    /// ponto de copiar arquivo em vez de reinserir linha a linha.
    #[test]
    fn colar_em_outro_banco_preserva_rowids_e_ordem() {
        // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
        let base = crate::apoio_teste::DirTemp::novo("phx-colar");
        let cat = Instancia::nova(&base).unwrap();
        let origem = cat.criar_database("loja").unwrap();
        let destino = cat.criar_database("arquivo").unwrap();

        let mut t = origem.criar_tabela(None, esquema("pedidos")).unwrap();
        for (i, txt) in ["um", "dois", "tres"].iter().enumerate() {
            t.inserir(&[Value::Int(i as i64 + 1), Value::Str((*txt).into())])
                .unwrap();
        }
        // Um buraco no meio: o slot excluido NAO e reaproveitado, e a copia tem
        // de carregar o buraco junto -- senao os rowids andariam.
        t.excluir(2).unwrap();
        t.sincronizar().unwrap();

        let copiados = origem
            .copiar_tabela_para("pedidos", &destino, "pedidos_2026")
            .unwrap();
        assert_eq!(copiados, 5, "os cinco arquivos");

        let mut c = destino.abrir_qualificada("pedidos_2026").unwrap();
        assert_eq!(c.slots(), 3, "o slot excluido continua ocupando lugar");
        assert!(c.ler(2).unwrap().is_none(), "o excluido continua excluido");
        for (rowid, txt) in [(1u64, "um"), (3, "tres")] {
            match &c.ler(rowid).unwrap().unwrap()[1] {
                Value::Str(s) => assert_eq!(s, txt, "rowid {rowid}"),
                outro => panic!("esperava texto, veio {outro:?}"),
            }
        }
        // E a chave primaria atravessou junto.
        assert!(c.esquema().chave_primaria().is_some());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn colar_por_cima_de_tabela_existente_e_recusado() {
        // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
        let base = crate::apoio_teste::DirTemp::novo("phx-colar2");
        let cat = Instancia::nova(&base).unwrap();
        let a = cat.criar_database("a").unwrap();
        let b = cat.criar_database("b").unwrap();
        a.criar_tabela(None, esquema("t")).unwrap();
        b.criar_tabela(None, esquema("t")).unwrap();

        // Sobrescrever cinco arquivos sem aviso nao tem desfazer.
        assert!(a.copiar_tabela_para("t", &b, "t").is_err());
        // E colar em cima de si mesma tambem nao faz sentido.
        assert!(a.copiar_tabela_para("t", &a, "t").is_err());
        // Mas com outro nome, no mesmo banco, vale.
        assert!(a.copiar_tabela_para("t", &a, "t_copia").is_ok());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn colar_dentro_de_schema_que_ainda_nao_existe_cria_a_pasta() {
        // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
        let base = crate::apoio_teste::DirTemp::novo("phx-colar3");
        let cat = Instancia::nova(&base).unwrap();
        let a = cat.criar_database("a").unwrap();
        let b = cat.criar_database("b").unwrap();
        a.criar_tabela(None, esquema("t")).unwrap();

        a.copiar_tabela_para("t", &b, "historico.t").unwrap();
        assert!(b.existe_tabela(Some("historico"), "t").unwrap());
        assert!(base.join("b").join("historico").is_dir());

        let _ = std::fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod testes_do_invariante {
    use super::*;

    /// Pergunta ao compilador se um tipo e `Sync`, sem exigir que ele seja.
    ///
    /// Rust nao tem `assert!(!T: Sync)`. O truque e o de sempre: um metodo
    /// INERENTE ganha do metodo do trait quando existe, entao `eh()` responde
    /// `true` so quando a implementacao com o limite `Sync` se aplica.
    struct Pergunta<T>(PhantomData<T>);

    trait Talvez {
        fn eh(&self) -> bool {
            false
        }
    }
    impl<T> Talvez for Pergunta<T> {}
    impl<T: Sync> Pergunta<T> {
        fn eh(&self) -> bool {
            true
        }
    }

    /// A `Instancia` e `Send` e **NAO** e `Sync` -- e isso e a trava por
    /// construcao.
    ///
    /// # O que este teste impede, e por que ele parece estranho
    ///
    /// Ele nao confere comportamento: confere um LIMITE DE TIPO. Existe porque
    /// a garantia que ele guarda nao esta em nenhuma linha executavel -- esta
    /// no fato de `RwLock<T>` exigir `T: Sync` e `Mutex<T>` nao.
    ///
    /// **Prova real, nos dois sentidos:** tire o campo `_so_com_a_ficha` e este
    /// teste reprova dizendo que a `Instancia` virou `Sync`; troque o
    /// `Mutex<Instancia>` do `servidor.rs` por `RwLock<Instancia>` e o
    /// PROJETO para de compilar, que e o sentido que mais importa.
    #[test]
    fn a_instancia_e_send_e_nao_e_sync() {
        fn exige_send<T: Send>() {}
        exige_send::<Instancia>();

        assert!(
            !Pergunta::<Instancia>(PhantomData).eh(),
            "a Instancia virou Sync. Se isso foi de proposito, o `RwLock` passou \
             a compilar -- e com ele dois escritores tomam guard de LEITURA e \
             abrem dois Table sobre os mesmos arquivos. Leia o comentario do \
             campo `_so_com_a_ficha` antes de tirar esta guarda."
        );
        // O controle: um tipo que E Sync tem de responder `true`, senao o
        // truque estaria sempre dizendo `false` e este teste passaria por
        // vacuidade -- que e o defeito que esta casa ja pegou tres vezes hoje.
        assert!(
            Pergunta::<PathBuf>(PhantomData).eh(),
            "o medidor esta quebrado"
        );

        // E o custo, MEDIDO em vez de afirmado: o marcador e de tamanho zero,
        // entao a `Instancia` continua sendo exatamente o `PathBuf` que ela
        // sempre foi. Garantia que custasse memoria seria outra conversa.
        assert_eq!(
            std::mem::size_of::<Instancia>(),
            std::mem::size_of::<PathBuf>(),
            "o marcador da trava passou a ocupar espaco"
        );
    }
}
