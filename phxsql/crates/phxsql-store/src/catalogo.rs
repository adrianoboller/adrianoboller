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
//! As duas marcas que existem (`_database.json`, o tipo, e
//! `_formato-volumes.json`, o separador de volume do pedido 508 -- ver
//! [`crate::separador`]) acrescentam informacao sem mudar essa descoberta.
//!
//! O nome qualificado de uma tabela e `schema.tabela`, ou so `tabela` quando
//! ela esta na raiz. E o mesmo formato que o catalogo do FraseSQL espera.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_core::paginacao::{separar_volume, SEPARADOR_DE_VOLUME};
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
///
/// Devolve o descritor que escreveu, com o caminho (pedido 589): perder o
/// marcador numa queda nao quebra a abertura -- ausencia e Padrao --, mas
/// faz uma colmeia voltar como database padrao, calada. Entao ele vai ao
/// disco com a criacao, e no descritor que o escreveu (pedido 552).
fn escrever_marca(diretorio: &Path, tipo: TipoDatabase) -> Result<(std::fs::File, PathBuf)> {
    use std::io::Write;
    let j = Json::objeto(vec![
        ("tipo", Json::texto_de(tipo.como_texto())),
        ("versao", Json::de_i64(1)),
    ]);
    let caminho = diretorio.join(MARCA_DATABASE);
    let mut arquivo = crate::util::recriar_do_banco(&caminho, false)?;
    arquivo.write_all(j.escrever_identado().as_bytes())?;
    Ok((arquivo, caminho))
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

/// O arquivo `precos#002.reg` pertence a tabela `precos` na extensao `reg`?
///
/// Ou e o nome exato, ou e o nome que o motor unico do pedido 508
/// ([`separar_volume`]) le como volume DESTA tabela. Sem a conferencia do
/// sufixo, `precos#historico.reg` seria dado como volume de `precos` e a
/// exclusao levaria o arquivo junto; e desde o 508 `precos_2024.reg` e outra
/// tabela, que nenhum sufixo de `precos` alcanca.
fn pertence(arquivo: &str, tabela: &str, ext: &str) -> bool {
    let Some(sem_ext) = arquivo.strip_suffix(&format!(".{ext}")) else {
        return false;
    };
    sem_ext == tabela || separar_volume(sem_ext).is_some_and(|(t, _)| t == tabela)
}

/// O [`pertence`], mais o `*.novo` que uma reescrita deixou ao lado do arquivo
/// (pedido 618): `clientes.fts.novo`, `clientes.reg.novo`,
/// `clientes#002.reg.novo`, `clientes.pag.novo`.
///
/// Existe para quem leva a tabela INTEIRA -- o `excluir_tabela` e o
/// `renomear_tabela`. Um `*.novo` interrompido e copia de conteudo da tabela
/// (o `.reg` inteiro, ou o vocabulario da coluna indexada, que pode ser
/// pessoal): apagar a tabela e deixa-lo e dado pessoal sem dono sob um nome
/// que nao existe mais; renomear e deixa-lo e orfanar a peca que a abertura
/// do `.reg` usaria para terminar uma troca decidida.
///
/// A copia e o inventario da tela continuam no [`pertence`]: copiar um
/// arquivo pela metade nao e copiar a tabela, e um `.novo` nao e extensao.
/// A copia so PERGUNTA por ele (pedido 624): havendo `*.novo` do `.reg`, a
/// troca decidida se termina antes de copiar -- ver
/// `RegFile::terminar_troca_antes_de_copiar`.
fn pertence_ou_sobra(arquivo: &str, tabela: &str, ext: &str) -> bool {
    pertence(arquivo, tabela, ext)
        || arquivo
            .strip_suffix(".novo")
            .is_some_and(|sem| pertence(sem, tabela, ext))
}

/// O nome nao e um engano de digitacao: e uma tentativa de sair do diretorio.
///
/// Separa as duas coisas de proposito. `"minha tabela!"` e um nome ruim --
/// alguem errou. `"../../etc/passwd"` nao e nome nenhum: ninguem digita isso
/// por acidente. Quem chama precisa poder tratar os dois casos de forma
/// diferente, e e por isso que esta funcao existe separada de
/// [`validar_nome`].
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
///
/// # O `#`, desde o pedido 508
///
/// O `#` e o separador de volume no nome do arquivo
/// ([`SEPARADOR_DE_VOLUME`]): `clientes#001.reg` e o volume 1 de `clientes`.
/// Nome que o contivesse voltaria a ser ambiguo, que e exatamente o que a
/// troca do `_` pelo `#` existe para acabar -- entao ele sai do nome, e a
/// recusa diz por que. Vale tambem para database e schema, de proposito: uma
/// regra so para os tres nomes, que sao todos nomes de diretorio ou arquivo
/// do mesmo motor, em vez de um caractere que entra num e nao no outro.
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
    if nome.contains(SEPARADOR_DE_VOLUME) {
        return Err(PhxError::Esquema(format!(
            "{rotulo} {nome:?} tem {SEPARADOR_DE_VOLUME:?}, e o \
             {SEPARADOR_DE_VOLUME} e reservado: no nome do arquivo ele separa a \
             tabela do volume (clientes{SEPARADOR_DE_VOLUME}001.reg e o volume 1 \
             de clientes), e um nome com ele se confundiria com o volume de \
             outra tabela. Escolha um nome sem {SEPARADOR_DE_VOLUME}"
        )));
    }
    Ok(())
}

/// Recusa o nome de tabela que o catalogo NAO leria de volta como ele mesmo.
///
/// E a pergunta das QUATRO portas que dao nome a uma tabela -- `criar_tabela`,
/// `duplicar_tabela`, `copiar_tabela_para` e `renomear_tabela` --, e mora
/// numa funcao so porque a lei manda que funcao e comando venham do mesmo
/// motor: a porta que ficasse com uma copia da regra seria a que diverge.
///
/// # O que sobrou dela depois do pedido 508
///
/// Ate o 508 o separador de volume era o `_`, e `pedidos_2025` se escrevia
/// igual ao volume 2025 de `pedidos`: a tabela nascia e sumia da arvore, e o
/// expurgo de `x` apagava a trilha de `x_001` (pedidos 368, 506). A recusa do
/// sufixo `_` morava aqui. Com o `#` como separador e o `#` fora do nome
/// ([`validar_nome`]), `vendas_2024` e nome de tabela como qualquer outro --
/// os quatro motores o aceitam -- e a recusa sai.
///
/// O que fica e o ponto, pelo pedido 507: `a.b` na raiz se escreve igual ao
/// nome QUALIFICADO schema `a` tabela `b`, e `abrir_qualificada("a.b")` nunca
/// achava a tabela de volta. E fica a pergunta ESTRUTURAL, que custa um
/// `rsplit` e nenhum disco: o nome, escrito como arquivo, volta pelo
/// [`nome_da_tabela`] como ele mesmo? Hoje ela nao recusa nada que o
/// `validar_nome` deixe passar -- e existe para que o dia em que alguem mudar
/// o separador sem mudar a recusa seja o dia em que esta funcao reclama, e
/// nao o dia em que uma tabela some.
fn exigir_nome_que_volta(dir: &Path, nome: &str) -> Result<()> {
    if separar_qualificado(nome).0.is_some() {
        return Err(PhxError::Esquema(format!(
            "{nome} nao serve como nome de tabela: o ponto e o separador do \
             nome QUALIFICADO (schema.tabela) -- abrir {nome:?} por esse \
             caminho o leria como outro schema e outra tabela, nunca esta. \
             Escolha um nome sem ponto"
        )));
    }
    if nome_da_tabela(&dir.join(format!("{nome}.{EXT_REG}"))).as_deref() != Some(nome) {
        return Err(PhxError::Esquema(format!(
            "{nome} nao serve como nome de tabela: o catalogo o leria como um \
             VOLUME de outra tabela, e a tabela sumiria da arvore. Escolha um \
             nome sem {SEPARADOR_DE_VOLUME}"
        )));
    }
    Ok(())
}

/// Extrai o nome da tabela de um arquivo `.reg`, tirando o sufixo de volume.
///
/// `cadastroClientes.reg` e `cadastroClientes#007.reg` devolvem os dois
/// `cadastroClientes`; `vendas_2024.reg` devolve `vendas_2024`.
///
/// Sem disco nenhum, desde o pedido 508: com o `_` como separador, o sufixo de
/// LETRA so contava como balde quando o volume 1 (`_A`) estava do lado, e a
/// resposta mudava sozinha no instante em que o arquivo nascia (pedido 506).
/// Com um separador que nome de tabela nao aceita, o nome responde sozinho.
fn nome_da_tabela(caminho: &Path) -> Option<String> {
    if caminho.extension().and_then(|s| s.to_str()) != Some(EXT_REG) {
        return None;
    }
    let base = caminho.file_stem()?.to_str()?;
    Some(separar_volume(base).map_or(base, |(t, _)| t).to_string())
}

/// Os nomes de tabela que moram num diretorio, resolvendo o sufixo de
/// volume das paginadas. Visivel ao `table` porque a busca reversa da
/// integridade referencial precisa perguntar "quem mais mora aqui?" --
/// e reescrever a varredura la seria a segunda copia de uma regra sutil.
///
/// # A ordem dos filtros, e por que ela e' medida
///
/// Uma tabela tem OITO arquivos e so' um deles e' `.reg`. Perguntar ao nucleo
/// "isto e' arquivo?" antes de olhar a extensao paga um `statx` por arquivo
/// para jogar sete fora -- e esta varredura roda a cada exclusao, pela regra
/// primordial da integridade. O `d_type` que o `getdents64` ja' trouxe
/// responde a mesma pergunta sem chamada nenhuma.
///
/// Medido em 16/09/2026, `--example custo-do-excluir`, N = 200.000: a
/// varredura de um diretorio de uma tabela sai de **9,05 para 4,43 us**
/// (mediana de 2 e de 6 corridas), e os `statx` por exclusao de **9 para 1**.
/// Com 30 irmas no diretorio, 309 `statx` por exclusao viram 61. Ver
/// `DESEMPENHO.md` §24.6.
///
/// O elo do symlink fica: `file_type()` nao segue o elo e `is_file()` segue,
/// entao um `.reg` alcancado por elo sumiria da lista -- e tabela que some da
/// lista e' tabela que ninguem pergunta se tem filha. Elo e' raro; a volta ao
/// `is_file()` acontece so' nele.
pub(crate) fn tabelas_em(diretorio: &Path) -> Result<Vec<String>> {
    if !diretorio.is_dir() {
        return Ok(Vec::new());
    }
    let mut nomes: Vec<String> = std::fs::read_dir(diretorio)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            Path::new(&e.file_name())
                .extension()
                .and_then(|s| s.to_str())
                == Some(EXT_REG)
        })
        .filter(|e| match e.file_type() {
            Ok(t) if t.is_symlink() => e.path().is_file(),
            Ok(t) => t.is_file(),
            Err(_) => e.path().is_file(),
        })
        .filter_map(|e| nome_da_tabela(&e.path()))
        .collect();
    nomes.sort();
    nomes.dedup();
    Ok(nomes)
}

pub(crate) fn subdiretorios(diretorio: &Path) -> Result<Vec<String>> {
    if !diretorio.is_dir() {
        return Ok(Vec::new());
    }
    let mut nomes: Vec<String> = std::fs::read_dir(diretorio)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        // A arvore do retrato da replica (pedido 729) e pasta na raiz, e
        // toda pasta da raiz e um database: sem esta poda ela apareceria em
        // `bancos`, na replicacao e no cluster como um database a mais.
        .filter(|n| !n.starts_with(PREFIXO_DO_RETRATO_DA_REPLICA))
        .collect();
    nomes.sort();
    Ok(nomes)
}

/// O que o diario de TODA tabela aberta para escrita carrega -- decisao do
/// servidor, tomada uma vez no arranque (pedidos 564 e 416).
///
/// # Por que mora aqui, e nao em quem abre a tabela
///
/// Porque quem abre a tabela sao muitos, e a decisao e uma so. Ate o pedido
/// 564 ela estava escrita em CINCO lugares do servidor (a porta, a replica, o
/// bidirecional, o PITR e o DbLink), cada um chamando `ligar_imagem_*` na
/// tabela que acabou de abrir -- e a recuperacao do arranque foi o sexto que
/// esqueceu: o `COMMIT` completado ia para o diario sem imagem, e a replica
/// parava em «veio sem imagem». Os tres maduros convergem nisto: a politica do
/// log de replicacao e do SERVIDOR (`wal_level` no PostgreSQL,
/// `binlog_row_image` no MySQL e no MariaDB), aplicada no ponto unico de
/// escrita do log, e nao de quem abriu a tabela.
///
/// Entao ela entra na [`Raiz`] (ou na [`Instancia`] do embutido), o
/// [`Database`] a herda, e [`Database::abrir_tabela`] e
/// [`Database::criar_tabela`] a aplicam. Quem abre por eles -- a porta, a
/// replica, a recuperacao, o PITR, a marca do embutido -- ja sai com ela, sem
/// lembrar de nada. O padrao e tudo desligado: o embutido e o servidor
/// isolado nao pagam pela imagem.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PoliticaDoDiario {
    /// Ver [`Table::ligar_imagem_no_diario`].
    pub imagem_no_diario: bool,
    /// Ver [`Table::ligar_imagem_na_exclusao`]. So vale com a de cima.
    pub imagem_na_exclusao: bool,
}

impl PoliticaDoDiario {
    /// A politica de um servidor que replica com `imagem_da_linha`: as duas
    /// ligadas juntas, ou as duas desligadas. A exclusao leva a imagem de
    /// antes nos quatro motores (PG com a chave ou a linha, MySQL e MariaDB
    /// `binlog_row_image=full`, SQLite session) -- pedido 416.
    pub fn com_imagem(ligada: bool) -> PoliticaDoDiario {
        PoliticaDoDiario {
            imagem_no_diario: ligada,
            imagem_na_exclusao: ligada,
        }
    }

    /// Aplica a politica numa tabela recem-aberta. UM lugar so.
    fn aplicar(self, t: &mut Table) {
        t.ligar_imagem_no_diario(self.imagem_no_diario);
        t.ligar_imagem_na_exclusao(self.imagem_na_exclusao);
    }
}

/// A raiz que contem varios databases.
pub struct Instancia {
    base: PathBuf,
    /// Ver [`PoliticaDoDiario`]. Copiada para cada [`Database`] aberto aqui.
    politica: PoliticaDoDiario,
    /// As travas de instancia que esta raiz FIXA (pedido 635): uma por pasta
    /// em que gravou, ate o dono morrer. Do servidor, vem da [`Raiz`] -- a
    /// `Instancia` dele nasce e morre a cada operacao, e a trava solta entre
    /// dois pedidos deixaria a CLI gravar com o `phxsqld` de pe.
    fixadas: Arc<crate::trava_de_instancia::Fixadas>,
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
    /// Abre (criando se preciso) a raiz de dados -- 0700 quando nasce aqui,
    /// pelo motor da permissao (pedido 542).
    ///
    /// E aqui, na abertura da RAIZ, que o disco do binario anterior ao pedido
    /// 508 troca o separador de volume (ver [`crate::separador`]): uma vez,
    /// antes da primeira operacao, e fora de qualquer trava. O database que
    /// recusar a migracao nao impede a raiz de abrir -- ele recusa na propria
    /// abertura, com o motivo.
    pub fn nova(base: impl AsRef<Path>) -> Result<Instancia> {
        let base = base.as_ref().to_path_buf();
        crate::util::criar_diretorio_do_banco(&base)?;
        let _ = crate::separador::migrar_base(&base);
        Ok(Instancia {
            base,
            politica: PoliticaDoDiario::default(),
            fixadas: Arc::default(),
            _so_com_a_ficha: PhantomData,
        })
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    /// Troca a politica do diario desta instancia -- o embutido que replica.
    /// O servidor passa pela [`Raiz::definir_politica_do_diario`].
    pub fn com_politica_do_diario(mut self, politica: PoliticaDoDiario) -> Instancia {
        self.politica = politica;
        self
    }

    pub fn politica_do_diario(&self) -> PoliticaDoDiario {
        self.politica
    }

    /// Cria um database do tipo **padrao** -- o caminho que sempre existiu.
    pub fn criar_database(&self, nome: &str) -> Result<Database> {
        self.criar_database_com_tipo(nome, TipoDatabase::Padrao)
    }

    /// Cria um database de um TIPO dado (padrao/hive/vetorial) e grava o
    /// marcador. O tipo nasce com o database e nao muda depois: um database e'
    /// de um tipo so, como uma tabela nasce com um esquema.
    ///
    /// Responde depois do `fsync` (pedido 589). Quem segura a trava global usa
    /// [`Self::criar_database_com_tipo_adiando_o_fsync`] e leva ao disco
    /// depois de solta-la -- a mesma criacao, so que adiando.
    pub fn criar_database_com_tipo(&self, nome: &str, tipo: TipoDatabase) -> Result<Database> {
        let (db, pendente) = self.criar_database_com_tipo_adiando_o_fsync(nome, tipo)?;
        pendente.levar_ao_disco()?;
        Ok(db)
    }

    /// A criacao do [`Self::criar_database_com_tipo`], devolvendo o `fsync`
    /// por fazer: o do marcador, o da pasta dele e o da entrada dela na base.
    pub fn criar_database_com_tipo_adiando_o_fsync(
        &self,
        nome: &str,
        tipo: TipoDatabase,
    ) -> Result<(Database, PorSincronizar)> {
        validar_nome("database", nome)?;
        let caminho = self.base.join(nome);
        if caminho.exists() {
            return Err(PhxError::Esquema(format!("database {nome} ja existe")));
        }
        // Pedido 605: reservado ANTES de a pasta existir, para nenhum terceiro
        // gravar dentro dela antes de a entrada dela estar no disco.
        let reserva = crate::nascendo::reservar(&self.base, nome)?;
        crate::util::criar_diretorio_do_banco(&caminho)?;
        let mut pendente = PorSincronizar::entrada_nova(&caminho);
        pendente.arquivos.push(escrever_marca(&caminho, tipo)?);
        pendente.reservas.push(reserva);
        crate::separador::marcar_novo(&caminho)?;
        Ok((
            Database {
                nome: nome.to_string(),
                caminho,
                tipo,
                politica: self.politica,
                fixadas: Some(Arc::clone(&self.fixadas)),
            },
            pendente,
        ))
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
        // O diretorio tem de estar no formato de nome do pedido 508 antes de
        // o catalogo listar qualquer coisa nele. Depois da primeira resposta
        // o custo e um `HashSet` em RAM -- ver `separador::ja_migrados`.
        crate::separador::exigir_migrado(&caminho)?;
        let tipo = ler_marca(&caminho);
        Ok(Database {
            nome: nome.to_string(),
            caminho,
            tipo,
            politica: self.politica,
            fixadas: Some(Arc::clone(&self.fixadas)),
        })
    }

    /// Cria o database se ainda nao existir.
    pub fn garantir_database(&self, nome: &str) -> Result<Database> {
        let (db, pendente) = self.garantir_database_adiando_o_fsync(nome)?;
        pendente.levar_ao_disco()?;
        Ok(db)
    }

    /// O [`Self::garantir_database`] com o `fsync` por fazer -- vazio quando o
    /// database ja existia (pedido 589).
    pub fn garantir_database_adiando_o_fsync(
        &self,
        nome: &str,
    ) -> Result<(Database, PorSincronizar)> {
        match self.abrir_database(nome) {
            Ok(d) => Ok((d, PorSincronizar::default())),
            Err(_) => self.criar_database_com_tipo_adiando_o_fsync(nome, TipoDatabase::Padrao),
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
    /// Ver [`PoliticaDoDiario`]. Vai para cada [`Instancia`] que a ficha
    /// exclusiva entrega -- e por ela, para cada tabela aberta para escrita.
    politica: PoliticaDoDiario,
    /// As travas de instancia do servidor (pedido 635), vivas enquanto a
    /// `Raiz` viver. Ver [`Instancia`].
    fixadas: Arc<crate::trava_de_instancia::Fixadas>,
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
    /// Abre (criando se preciso) a raiz de dados -- 0700 quando nasce aqui,
    /// pelo motor da permissao (pedido 542). A que ja existia fica como
    /// estava; o alerta dela e do arranque (`permissao::permissao_larga`).
    pub fn nova(base: impl AsRef<Path>) -> Result<Raiz> {
        let base = base.as_ref().to_path_buf();
        crate::util::criar_diretorio_do_banco(&base)?;
        Ok(Raiz {
            base,
            politica: PoliticaDoDiario::default(),
            fixadas: Arc::default(),
        })
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    /// A politica do diario do servidor, definida UMA vez, antes da primeira
    /// abertura -- inclusive antes da recuperacao do arranque, que e quem a
    /// esquecia (pedido 564).
    pub fn definir_politica_do_diario(&mut self, politica: PoliticaDoDiario) {
        self.politica = politica;
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
                politica: self.politica,
                fixadas: Arc::clone(&self.fixadas),
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
        self.abrir_para_ler_com(database, qualificado, Table::abrir_para_ler)
    }

    /// O [`Raiz::abrir_para_ler`] de quem le so o DIARIO -- pedido 330: a cauda
    /// do `.log` alem do cabecalho entra pela memoria em vez de recusar (ver
    /// [`Table::abrir_para_ler_o_diario`]). O caminho, os nomes e as recusas
    /// sao os MESMOS, pelo mesmo corpo.
    pub fn abrir_diario_para_ler(&self, database: &str, qualificado: &str) -> Result<Aberta> {
        self.abrir_para_ler_com(database, qualificado, Table::abrir_para_ler_o_diario)
    }

    /// Os databases da raiz -- so olha o diretorio, nao abre nada. Existe para
    /// o vigia de previsao (pedido 496, F6), que roda sob a ficha
    /// compartilhada e nao pode pedir a exclusiva so para fazer uma lista.
    pub fn databases(&self) -> Result<Vec<String>> {
        subdiretorios(&self.base)
    }

    /// Toda tabela de um database, qualificada, para quem so LE. `None` quando
    /// o database ainda nao tem a marca do formato de nome (pedido 508): ali
    /// listar ja exige escrever, e quem chama deixa a vez para a ficha
    /// exclusiva -- a mesma regra do [`Raiz::abrir_para_ler`].
    pub fn tabelas_para_ler(&self, database: &str) -> Result<Option<Vec<String>>> {
        validar_nome("database", database)?;
        if crate::separador::precisa_migrar(&self.base.join(database)) {
            return Ok(None);
        }
        self.so_para_achar_o_caminho()
            .abrir_database(database)?
            .todas_as_tabelas()
            .map(Some)
    }

    /// A `Instancia` que serve so para resolver nomes e caminhos. A politica
    /// nao importa: o que sai dela e leitura.
    fn so_para_achar_o_caminho(&self) -> Instancia {
        Instancia {
            base: self.base.clone(),
            politica: self.politica,
            fixadas: Arc::clone(&self.fixadas),
            _so_com_a_ficha: PhantomData,
        }
    }

    fn abrir_para_ler_com(
        &self,
        database: &str,
        qualificado: &str,
        abrir: fn(PathBuf, &str) -> Result<SemEscrever>,
    ) -> Result<Aberta> {
        let so_para_achar_o_caminho = self.so_para_achar_o_caminho();
        // O database que ainda nao tem a marca do formato de nome (pedido
        // 508) ganha a marca -- ou recusa -- na abertura; e marca e escrita.
        validar_nome("database", database)?;
        if crate::separador::precisa_migrar(&self.base.join(database)) {
            return Ok(Aberta::PrecisaDaFichaExclusiva(
                "o database ainda nao tem a marca do formato de nome (pedido 508)",
            ));
        }
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
        let aberta =
            abrir(dir, &nome).map_err(|e| db.tabela_que_nao_existe(e, schema.as_deref(), &nome));
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
    /// Herdada da [`Instancia`] que abriu este database. Ver
    /// [`PoliticaDoDiario`].
    politica: PoliticaDoDiario,
    /// As travas fixadas da [`Instancia`] que abriu este database (pedido
    /// 635). `None` no palco da restauracao: ele e de um dono so e sai do
    /// lugar por `rename`, e trava fixada ali impediria o `rename` no Windows.
    fixadas: Option<Arc<crate::trava_de_instancia::Fixadas>>,
}

impl Database {
    /// A politica do diario que este database aplica a toda tabela que abre.
    pub fn politica_do_diario(&self) -> PoliticaDoDiario {
        self.politica
    }

    /// [`Database::no_diretorio`] com a politica do diario de quem pede --
    /// a pasta de um schema, na recuperacao, e o diretorio do `reindex` do
    /// CLI (pedido 601).
    pub(crate) fn no_diretorio_com_politica(
        caminho: &Path,
        politica: PoliticaDoDiario,
    ) -> Database {
        let mut db = Database::no_diretorio(caminho);
        db.politica = politica;
        db
    }

    /// O database num diretorio que NAO mora debaixo de uma raiz de dados: o
    /// palco da restauracao, cujo nome comeca por ponto e nao passaria no
    /// `validar_nome`. So o crate usa, e so para o que a restauracao faz no
    /// palco antes de ele virar database.
    pub(crate) fn no_diretorio(caminho: &Path) -> Database {
        Database {
            nome: caminho
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            caminho: caminho.to_path_buf(),
            tipo: ler_marca(caminho),
            // O palco da restauracao nao e servidor de ninguem: quem reaplica
            // nele e o proprio `restaurar`, que nao replica dali.
            politica: PoliticaDoDiario::default(),
            fixadas: None,
        }
    }

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

    /// Cria um schema -- uma pasta dentro do database. Responde depois do
    /// `fsync` do database, que e onde mora a entrada da pasta nova (pedido
    /// 589); quem segura a trava usa [`Self::criar_schema_adiando_o_fsync`].
    pub fn criar_schema(&self, nome: &str) -> Result<PathBuf> {
        let (caminho, pendente) = self.criar_schema_adiando_o_fsync(nome)?;
        pendente.levar_ao_disco()?;
        Ok(caminho)
    }

    /// A criacao do [`Self::criar_schema`], devolvendo o `fsync` por fazer.
    pub fn criar_schema_adiando_o_fsync(&self, nome: &str) -> Result<(PathBuf, PorSincronizar)> {
        validar_nome("schema", nome)?;
        let caminho = self.caminho.join(nome);
        if caminho.exists() {
            return Err(PhxError::Esquema(format!(
                "schema {nome} ja existe em {}",
                self.nome
            )));
        }
        self.garantir_schema_adiando_o_fsync(nome)
    }

    /// Cria a pasta do schema se ainda nao existir, e responde depois do
    /// `fsync` do database quando ela nasceu aqui (pedido 589).
    pub fn garantir_schema(&self, nome: &str) -> Result<PathBuf> {
        let (caminho, pendente) = self.garantir_schema_adiando_o_fsync(nome)?;
        pendente.levar_ao_disco()?;
        Ok(caminho)
    }

    /// O [`Self::garantir_schema`] com o `fsync` por fazer. E o laco UNICO
    /// dos tres caminhos que criam pasta de schema -- `criar_schema`,
    /// `criar_tabela` e o colar --: o 589 nasceu porque o colar (586) ganhou o
    /// `fsync` do database e os outros dois, que chamavam o mesmo
    /// `garantir_schema`, ficaram sem.
    ///
    /// A marca de formato da pasta nova (`separador::marcar_novo`) fica fora
    /// do `fsync` de proposito: perde-la so refaz uma varredura que nao
    /// renomeia nada (ver o comentario dela).
    pub fn garantir_schema_adiando_o_fsync(&self, nome: &str) -> Result<(PathBuf, PorSincronizar)> {
        validar_nome("schema", nome)?;
        let caminho = self.caminho.join(nome);
        // Pedido 605: a pasta que OUTRA operacao ainda leva ao disco nao serve
        // de morada antes de chegar la -- nem a do database acima dela.
        crate::nascendo::esperar(&self.caminho, nome)?;
        if caminho.is_dir() {
            return Ok((caminho, PorSincronizar::default()));
        }
        let reserva = crate::nascendo::reservar(&self.caminho, nome)?;
        crate::util::criar_diretorio_do_banco(&caminho)?;
        crate::separador::marcar_novo(&caminho)?;
        let mut pendente = PorSincronizar::entrada_nova(&caminho);
        pendente.reservas.push(reserva);
        Ok((caminho, pendente))
    }

    pub fn schemas(&self) -> Result<Vec<String>> {
        subdiretorios(&self.caminho)
    }

    // ------------------------------------------------- sequencias nomeadas
    //
    // O `.seq` do pedido 229 (`crate::sequencia`). Moram na pasta do
    // database ou do schema, ao lado das tabelas, e dividem o espaco de
    // nomes com elas -- os dois motores que tem `CREATE SEQUENCE` (PostgreSQL
    // e MariaDB) fazem assim. A escrita passa pela trava de instancia
    // (pedido 635) como toda escrita de pasta do banco.

    /// Cria a sequencia nomeada, com o `fsync` por fazer (pedido 589: o
    /// `fsync` do arquivo e da pasta ficam para fora da trava global).
    pub fn criar_sequencia_adiando_o_fsync(
        &self,
        schema: Option<&str>,
        nome: &str,
        definicao: crate::sequencia::Definicao,
    ) -> Result<(crate::sequencia::Sequencia, PorSincronizar)> {
        self.exigir_motor_padrao()?;
        // O schema tem de EXISTIR -- `CREATE SEQUENCE filial.nf` sem o schema
        // `filial` e erro no PostgreSQL e no MariaDB, que convergem. E nao
        // passar pelo `garantir_schema` mantem esta secao fora da espera do
        // `nascendo` (a catraca `rede-ou-espera-2`): criar sequencia nao
        // precisa esperar pasta nenhuma nascer.
        let dir = self.diretorio(schema)?;
        if !dir.is_dir() {
            return Err(PhxError::NaoEncontrado(format!(
                "schema {} nao existe em {}: crie-o antes da sequencia",
                schema.unwrap_or(""),
                self.nome
            )));
        }
        let mut pendente = PorSincronizar::default();
        if tabelas_em(&dir)?.iter().any(|t| t == nome) {
            return Err(PhxError::Duplicado(format!(
                "ja existe uma tabela chamada {nome}: tabela e sequencia dividem o \
                 mesmo espaco de nomes"
            )));
        }
        let _trava = self.tomar_a_trava(&dir)?;
        let (s, arquivo) =
            crate::sequencia::Sequencia::criar_adiando_o_fsync(&dir, nome, definicao)?;
        pendente.arquivos.push((arquivo, s.caminho().to_path_buf()));
        Ok((s, pendente))
    }

    /// [`Self::criar_sequencia_adiando_o_fsync`] respondendo depois do disco.
    pub fn criar_sequencia(
        &self,
        schema: Option<&str>,
        nome: &str,
        definicao: crate::sequencia::Definicao,
    ) -> Result<crate::sequencia::Sequencia> {
        let (s, pendente) = self.criar_sequencia_adiando_o_fsync(schema, nome, definicao)?;
        pendente.levar_ao_disco()?;
        Ok(s)
    }

    /// Abre a sequencia nomeada. Quem vai pedir numero segura a trava de
    /// instancia pela [`Posse`] devolvida -- o `proximo` grava.
    pub fn abrir_sequencia(
        &self,
        schema: Option<&str>,
        nome: &str,
    ) -> Result<(
        crate::sequencia::Sequencia,
        Option<crate::trava_de_instancia::Posse>,
    )> {
        self.exigir_motor_padrao()?;
        let dir = self.diretorio(schema)?;
        let posse = self.tomar_a_trava(&dir)?;
        Ok((crate::sequencia::Sequencia::abrir(&dir, nome)?, posse))
    }

    /// Apaga a sequencia nomeada, com o `fsync` da pasta por fazer.
    pub fn excluir_sequencia_adiando_o_fsync(
        &self,
        schema: Option<&str>,
        nome: &str,
    ) -> Result<PorSincronizar> {
        self.exigir_motor_padrao()?;
        let dir = self.diretorio(schema)?;
        let _trava = self.tomar_a_trava(&dir)?;
        let saiu = crate::sequencia::Sequencia::excluir(&dir, nome)?;
        Ok(PorSincronizar::entradas_que_sairam(vec![saiu]))
    }

    /// [`Self::excluir_sequencia_adiando_o_fsync`] respondendo depois do disco.
    pub fn excluir_sequencia(&self, schema: Option<&str>, nome: &str) -> Result<()> {
        self.excluir_sequencia_adiando_o_fsync(schema, nome)?
            .levar_ao_disco()?;
        Ok(())
    }

    /// As sequencias nomeadas de um schema, ou da raiz quando `None`.
    pub fn sequencias_nomeadas(&self, schema: Option<&str>) -> Result<Vec<String>> {
        crate::sequencia::listar(&self.diretorio(schema)?)
    }

    /// Toda sequencia nomeada do database, qualificada quando mora num schema.
    pub fn todas_as_sequencias_nomeadas(&self) -> Result<Vec<String>> {
        let mut saida = self.sequencias_nomeadas(None)?;
        for s in self.schemas()? {
            for n in self.sequencias_nomeadas(Some(&s))? {
                saida.push(format!("{s}.{n}"));
            }
        }
        saida.sort();
        Ok(saida)
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

    /// Reconstroi o `.ndx` -- e o `.fts` -- de toda tabela deste database que
    /// abre marcada, e sincroniza o que reconstruiu. Pedido 522.
    ///
    /// Devolve quantas reconstruiu e, por tabela, o que NAO reconstruiu -- e a
    /// linha de cada uma diz qual e por que: tabela que continua recusando nao
    /// pode sumir de relatorio nenhum.
    ///
    /// # Quem chama, e por que o mesmo motor serve aos dois
    ///
    /// O arranque do servidor, para as tabelas que o processo anterior deixou
    /// com o byte 52 em 1 (o `fechar` nao baixa mais a marca -- so o
    /// `sincronizar`, com `fsync`); e a restauracao, porque o backup copia o
    /// `.ndx` como o nucleo o tinha, e o de toda tabela escrita desde o ultimo
    /// fecho da janela sai marcado. A pergunta e a mesma -- «esta arvore abre
    /// marcada?» -- e a resposta mora aqui uma vez so.
    ///
    /// # Por que o cabecalho antes da tabela
    ///
    /// Abrir a tabela sao dez arquivos, e quase toda tabela esta limpa:
    /// [`crate::ndx::marcado_no_arquivo`] le 128 bytes. O cabecalho que nem se
    /// le conta como marcado, e a abertura da tabela diz o motivo.
    ///
    /// # Por que `sincronizar` depois do `reindexar`
    ///
    /// Reconstruido e so atestado neste processo, o indice abriria marcado de
    /// novo na queda seguinte antes do primeiro fecho da janela.
    pub fn reconstruir_indices_marcados(&self) -> (usize, Vec<String>) {
        let mut feitas = 0usize;
        let mut pendentes = Vec::new();
        if self.tipo != TipoDatabase::Padrao {
            return (feitas, pendentes);
        }
        let tabelas = match self.todas_as_tabelas() {
            Ok(t) => t,
            Err(e) => {
                pendentes.push(format!("{}: nao listou as tabelas ({e})", self.nome));
                return (feitas, pendentes);
            }
        };
        for qualificada in tabelas {
            let (schema, tabela) = separar_qualificado(&qualificada);
            let Ok(dir) = self.diretorio(schema.as_deref()) else {
                continue;
            };
            let marcado = |ext: &str| {
                let c = dir.join(format!("{tabela}.{ext}"));
                c.exists() && crate::ndx::marcado_no_arquivo(&c).unwrap_or(true)
            };
            if !marcado("ndx") && !marcado(EXT_FTS) {
                continue;
            }
            let feito = self.abrir_qualificada(&qualificada).and_then(|mut t| {
                if !t.indice_precisa_reconstruir() {
                    return Ok(false);
                }
                t.reindexar()?;
                t.sincronizar()?;
                Ok(true)
            });
            match feito {
                Ok(true) => feitas += 1,
                Ok(false) => {}
                Err(e) => pendentes.push(format!("{}/{qualificada}: {e}", self.nome)),
            }
        }
        (feitas, pendentes)
    }

    /// As tabelas deste database cuja arvore guarda coluna marcada EM CLARO
    /// com o cofre ligado -- condicao A do papel C sobre o pedido 339. Uma
    /// linha por tabela, nomeando-a e dizendo o comando que a sela.
    ///
    /// AVISA e nao converte: guarda nova entra pedida, e refazer sozinho no
    /// arranque seria impor uma varredura de tabela inteira a quem nao pediu.
    /// Os casos que chegam aqui sao tres -- tabela de antes do conserto,
    /// coluna indexada marcada depois, e a queda entre os dois `rename` da
    /// FASE B do `Criptografar` --, e o remedio dos tres e o `reindexar`.
    ///
    /// # Por que o cabecalho antes do esquema
    ///
    /// Cofre desligado nao pergunta nada. Ligado, o `.ndx` versao 2 ja esta
    /// selado e sai com 128 bytes lidos; so o versao 1 abre o `.reg` -- sem
    /// escrever -- para saber se a arvore tem coluna marcada.
    pub fn indices_em_claro_sobre_coluna_marcada(&self) -> Vec<String> {
        let mut achados = Vec::new();
        if self.tipo != TipoDatabase::Padrao || !crate::cofre::ligado() {
            return achados;
        }
        let Ok(tabelas) = self.todas_as_tabelas() else {
            return achados;
        };
        for qualificada in tabelas {
            let (schema, tabela) = separar_qualificado(&qualificada);
            let Ok(dir) = self.diretorio(schema.as_deref()) else {
                continue;
            };
            let ndx = dir.join(format!("{tabela}.ndx"));
            if crate::ndx::versao_no_arquivo(&ndx).ok() != Some(1) {
                continue;
            }
            let Ok(Some(reg)) = crate::reg::RegFile::abrir_sem_escrever(&dir, &tabela) else {
                continue;
            };
            if crate::ndx::indice_sobre_coluna_marcada(reg.esquema()) {
                achados.push(format!(
                    "{db}/{qualificada}: o indice sobre coluna marcada esta EM CLARO no .ndx \
                     com o cofre ligado -- sele com \
                     {{\"op\":\"reindexar\",\"database\":\"{db}\",\"tabela\":\"{qualificada}\"}}",
                    db = self.nome
                ));
            }
        }
        achados
    }

    /// Cria uma tabela, e responde depois de os arquivos dela, a pasta e --
    /// se a pasta do schema nasceu aqui -- o database estarem no disco (pedido
    /// 589). Quem segura a trava global usa
    /// [`Self::criar_tabela_adiando_o_fsync`] e leva ao disco depois.
    pub fn criar_tabela(&self, schema: Option<&str>, esquema: Schema) -> Result<Table> {
        let (t, pendente) = self.criar_tabela_adiando_o_fsync(schema, esquema)?;
        pendente.levar_ao_disco()?;
        Ok(t)
    }

    /// A criacao do [`Self::criar_tabela`], devolvendo o `fsync` por fazer:
    /// os descritores que o `Table::criar` abriu (duplicados -- ver
    /// `Table::descritores_da_criacao`), a pasta onde eles nasceram e, se o
    /// schema nasceu junto, a entrada dele no database.
    pub fn criar_tabela_adiando_o_fsync(
        &self,
        schema: Option<&str>,
        esquema: Schema,
    ) -> Result<(Table, PorSincronizar)> {
        self.exigir_motor_padrao()?;
        validar_nome("tabela", esquema.nome())?;
        let (dir, mut pendente) = match schema {
            None => (self.caminho.clone(), PorSincronizar::default()),
            Some(s) => self.garantir_schema_adiando_o_fsync(s)?,
        };
        exigir_nome_que_volta(&dir, esquema.nome())?;
        // Tabela e sequencia nomeada dividem o espaco de nomes, como no
        // PostgreSQL e no MariaDB (la a sequencia E uma relacao). Os dois
        // arquivos coexistiriam no disco; o que nao coexiste e o nome na
        // cabeca de quem le `nf` num erro.
        if crate::sequencia::existe(&dir, esquema.nome()) {
            return Err(PhxError::Duplicado(format!(
                "ja existe uma sequencia chamada {}: tabela e sequencia dividem o \
                 mesmo espaco de nomes",
                esquema.nome()
            )));
        }
        // Pedido 605: o nome entra RESERVADO antes do primeiro arquivo, e sai
        // so quando o `levar_ao_disco` terminar -- ate la, quem abre espera
        // (`crate::nascendo`). A pasta acima que ainda nasce por outra
        // operacao tambem segura: morar nela antes do `fsync` dela e o mesmo
        // defeito.
        crate::nascendo::esperar(&dir, esquema.nome())?;
        pendente
            .reservas
            .push(crate::nascendo::reservar(&dir, esquema.nome())?);
        let mut t = Table::criar(dir, esquema)?;
        self.politica.aplicar(&mut t);
        self.fixar_a_trava(&t);
        pendente.arquivos.extend(t.descritores_da_criacao()?);
        Ok((t, pendente))
    }

    pub fn abrir_tabela(&self, schema: Option<&str>, nome: &str) -> Result<Table> {
        self.exigir_motor_padrao()?;
        validar_nome("tabela", nome)?;
        let mut t = Table::abrir(self.diretorio(schema)?, nome)
            .map_err(|e| self.tabela_que_nao_existe(e, schema, nome))?;
        self.politica.aplicar(&mut t);
        self.fixar_a_trava(&t);
        Ok(t)
    }

    /// Fixa na [`Instancia`] a trava de instancia da pasta desta tabela
    /// (pedido 635) -- ao lado da politica, pelo mesmo motivo: e a decisao
    /// da raiz aplicada a TODA tabela que ela abre para gravar. O custo
    /// depois do primeiro pedido e um mapa e uma comparacao de ponteiro.
    fn fixar_a_trava(&self, t: &Table) {
        if let (Some(f), Some(p)) = (&self.fixadas, t.trava_de_instancia()) {
            f.fixar(p);
        }
    }

    /// Toma a trava de instancia da pasta para quem mexe nos arquivos SEM
    /// abrir a tabela (`excluir_tabela`, `renomear_tabela`) -- os dois do
    /// inventario do congelamento, que pelo mesmo motivo perguntam a ele.
    fn tomar_a_trava(&self, dir: &Path) -> Result<Option<crate::trava_de_instancia::Posse>> {
        let posse = crate::trava_de_instancia::tomar(dir)?;
        if let (Some(f), Some(p)) = (&self.fixadas, &posse) {
            f.fixar(p);
        }
        Ok(posse)
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

    /// [`Table::realinhar_sequencia`] pelo nome qualificado, com as mesmas
    /// conferencias do [`Database::abrir_tabela`] (motor, nome, «nao existe»)
    /// -- a porta do pedido 290 para a tabela que a faixa recusa abrir.
    pub fn realinhar_sequencia(&self, qualificado: &str) -> Result<(u64, u64, u64)> {
        self.exigir_motor_padrao()?;
        let (schema, nome) = separar_qualificado(qualificado);
        validar_nome("tabela", &nome)?;
        Table::realinhar_sequencia(self.diretorio(schema.as_deref())?, &nome)
            .map_err(|e| self.tabela_que_nao_existe(e, schema.as_deref(), &nome))
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
    ///
    /// # Responde depois do `fsync` da pasta (pedido 591)
    ///
    /// O `unlink` so vale no disco depois do `fsync` do diretorio. Sem ele,
    /// uma queda devolvia a tabela que o cliente ouviu «excluida» -- inteira,
    /// ou pela metade (o `.reg` sem o `.ndx`), e a regra primordial tinha
    /// conferido um estado que a queda desfaz. Quem segura a trava global usa
    /// [`Self::excluir_tabela_adiando_o_fsync`] e leva ao disco depois.
    pub fn excluir_tabela(&self, qualificado: &str) -> Result<Vec<String>> {
        let (apagados, pendente) = self.excluir_tabela_adiando_o_fsync(qualificado);
        // Mesmo no erro (pedido 595): o que ja saiu saiu, e sem o `fsync` da
        // pasta volta numa queda -- a tabela pela metade que o 591 fechou no
        // caminho feliz. A recusa do disco, se vier, fala mais alto.
        pendente.levar_ao_disco()?;
        apagados
    }

    /// O [`Self::excluir_tabela`] sem o `fsync` da pasta: ele volta em
    /// [`PorSincronizar`], para o servidor soltar a trava global antes de
    /// esperar o disco (a catraca `alcancam-fsync-2`, como no 589).
    ///
    /// O [`PorSincronizar`] volta FORA do `Result`, e e a razao do formato:
    /// um `remove_file` que falha no terceiro de cinco arquivos deixa dois
    /// nomes ja apagados, e um `?` aqui os esquecia sem `fsync` (pedido 595).
    /// Quem chama leva ao disco nos dois casos.
    pub fn excluir_tabela_adiando_o_fsync(
        &self,
        qualificado: &str,
    ) -> (Result<Vec<String>>, PorSincronizar) {
        let mut sairam = Vec::new();
        let apagados = self.excluir_tabela_juntando(qualificado, &mut sairam);
        (apagados, PorSincronizar::entradas_que_sairam(sairam))
    }

    /// O corpo do [`Self::excluir_tabela_adiando_o_fsync`]: cada nome apagado
    /// entra em `sairam` NO INSTANTE do `unlink`, antes de qualquer `?` que
    /// venha depois.
    fn excluir_tabela_juntando(
        &self,
        qualificado: &str,
        sairam: &mut Vec<PathBuf>,
    ) -> Result<Vec<String>> {
        self.exigir_motor_padrao()?;
        let (schema, nome) = separar_qualificado(qualificado);
        let (schema, nome) = (schema.as_deref(), nome.as_str());
        validar_nome("tabela", nome)?;
        let dir = self.diretorio(schema)?;
        // Apagar mexe nos arquivos SEM abrir a tabela, entao nao passa pelo
        // portao do `Table::abrir_com` -- e no meio de uma reescrita apagaria o
        // volume que a FASE B vai trocar (pedido 428). Pergunta ao mesmo
        // registro, pela mesma funcao.
        crate::congelamento::conferir(&dir, nome)?;
        // E pelo mesmo motivo a trava de instancia (pedido 635): apagar e
        // gravar, e o outro processo que serve a pasta nao ve.
        let _trava = self.tomar_a_trava(&dir)?;
        if let Some(filha) = self.quem_aponta_para(&dir, nome)? {
            return Err(PhxError::Integridade(format!(
                "a tabela {qualificado} nao pode ser apagada: {filha} declara \
                 uma chave estrangeira para ela. Nunca se apaga o pai que tem \
                 filhos -- apague {filha} primeiro, ou tire a chave dela"
            )));
        }
        // Os caminhos inteiros vao para `sairam`, para o `fsync` da pasta;
        // `apagados` e so o nome, que e o que a resposta mostra.
        let mut apagados = Vec::new();
        for ext in Self::EXTENSOES_TODAS {
            // Uma tabela paginada tem varios volumes por extensao.
            for arq in std::fs::read_dir(&dir)?.flatten() {
                let f = arq.file_name();
                let f = f.to_string_lossy();
                if pertence_ou_sobra(&f, nome, ext) {
                    std::fs::remove_file(arq.path())?;
                    apagados.push(f.to_string());
                    sairam.push(arq.path());
                }
            }
        }
        if apagados.is_empty() {
            return Err(PhxError::NaoEncontrado(format!(
                "tabela {qualificado} nao existe em {}",
                self.nome()
            )));
        }
        // O que ela devia ao disco foi apagado junto (536).
        crate::volume::mudar_pendentes_de_nome(&dir, nome, None);
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
        // O irmao do `excluir_tabela`: mover os arquivos de uma tabela em
        // reescrita deixaria a FASE B sem o volume que ela troca (pedido 428).
        crate::congelamento::conferir(&dir_o, nome_o)?;
        // A trava de instancia das DUAS pastas (pedido 635): mover tira de
        // uma e poe na outra.
        let _trava = (self.tomar_a_trava(&dir_o)?, self.tomar_a_trava(&dir_d)?);
        // Nome que o catalogo NAO leria de volta como ele mesmo faz a tabela
        // sumir -- achado pela propria prova deste renomear, que escolheu
        // `pedidos_2025` sem pensar. A pergunta e a mesma das outras tres
        // portas, e por isso mora numa funcao so.
        exigir_nome_que_volta(&dir_d, nome_d)?;
        if let Some(filha) = self.quem_aponta_para(&dir_o, nome_o)? {
            return Err(PhxError::Integridade(format!(
                "a tabela {origem} nao pode ser renomeada: {filha} declara uma \
                 chave estrangeira para ela, e a chave guarda a mae pelo NOME \
                 -- renomear deixaria {filha} apontando para uma tabela que nao \
                 existe. Tire a chave de {filha}, renomeie, e declare de novo"
            )));
        }
        // A AUTO-REFERENCIA guarda a mae pelo nome do mesmo jeito, e o
        // `quem_aponta_para` a pula -- certo para o `excluir_tabela`, que leva a
        // chave junto com a tabela. Aqui ela ficaria apontando para o nome
        // velho, deixaria de se reconhecer, e o chefe com subordinado passaria
        // a sair orfao calado: o irmao do pedido 491 no catalogo.
        if let Ok(reg) = crate::reg::RegFile::abrir(&dir_o, nome_o) {
            if let Some(fk) = reg
                .esquema()
                .chaves_estrangeiras()
                .iter()
                .find(|fk| crate::table::nome_simples(&fk.tabela_ref) == nome_o)
            {
                return Err(PhxError::Integridade(format!(
                    "a tabela {origem} nao pode ser renomeada: a chave {:?} dela \
                     aponta para ela mesma, e a chave guarda a mae pelo NOME -- \
                     renomear a deixaria apontando para uma tabela que nao existe. \
                     Tire a chave, renomeie, e declare de novo",
                    fk.nome
                )));
            }
        }

        // Colher primeiro, mover depois: `read_dir` enquanto se renomeia dentro
        // do mesmo diretorio pode enxergar o nome novo.
        let mut mover: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
        for ext in Self::EXTENSOES_TODAS {
            for arq in std::fs::read_dir(&dir_o)?.flatten() {
                let f = arq.file_name();
                let f = f.to_string_lossy();
                if pertence_ou_sobra(&f, nome_o, ext) {
                    // Preserva o sufixo do volume, como a copia faz:
                    // `precos#002.reg` vira `novo#002.reg`.
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
        // Pedido 467: os nomes novos so valem depois do `fsync` dos dois
        // diretorios. Um so, no fim: todos os arquivos da tabela moram no
        // mesmo diretorio de origem e vao para o mesmo de destino.
        if let Some((de, para)) = mover.first() {
            crate::sincronia::sincronizar_os_diretorios(de, para, true)?;
        }
        // O atestado do pedido 522 vai junto: e o mesmo inode, no caminho
        // novo. Sem isto, a tabela escrita desde o ultimo fecho da janela
        // abria no nome novo recusando tudo. Ver `ndx::levar_atestado`.
        for (de, para) in &feitos {
            crate::ndx::levar_atestado(de, para, true);
        }
        // E o registro do que ainda deve ao disco, pelo mesmo motivo (536).
        crate::volume::mudar_pendentes_de_nome(&dir_o, nome_o, Some((&dir_d, nome_d)));
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
            // Irma que nao abre RECUSA o apagar e o renomear (pedido 631): ela
            // pode ser a filha, e pula-la era responder «ninguem aponta» por
            // ela. A decisao e a do `crate::irmas::abrir_irma`, a mesma da
            // exclusao de linha.
            let reg = crate::irmas::abrir_irma(dir, &irma, nome)?;
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
    ///
    /// Responde depois do `fsync` da copia (pedido 586). Quem segura a trava
    /// global e nao quer pagar o `fsync` sob ela usa
    /// [`Self::duplicar_tabela_adiando_o_fsync`] e leva ao disco depois de
    /// solta-la -- as duas sao a MESMA copia, esta so nao adia.
    pub fn duplicar_tabela(&self, origem: &str, destino: &str) -> Result<usize> {
        self.duplicar_tabela_adiando_o_fsync(origem, destino)?
            .levar_ao_disco()
    }

    /// A copia do [`Self::duplicar_tabela`], devolvendo o `fsync` por fazer.
    pub fn duplicar_tabela_adiando_o_fsync(
        &self,
        origem: &str,
        destino: &str,
    ) -> Result<PorSincronizar> {
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
        exigir_nome_que_volta(&dir_d, nome_d)?;
        let copia = Self::copiar_os_arquivos(&dir_o, nome_o, &dir_d, nome_d)?;
        if copia.arquivos.is_empty() {
            return Err(PhxError::NaoEncontrado(format!(
                "tabela {origem} nao existe em {}",
                self.nome()
            )));
        }
        Ok(copia)
    }

    /// Copia uma tabela para OUTRO database -- o "colar" da tela.
    ///
    /// O `duplicar_tabela` copia dentro do mesmo database; este atravessa. E a
    /// mesma copia byte a byte, e pela mesma razao: a copia nasce com os
    /// mesmos rowids e na mesma ordem de digitacao. E responde depois do
    /// `fsync`, pelo mesmo motivo do irmao (pedido 586).
    pub fn copiar_tabela_para(
        &self,
        origem: &str,
        destino_db: &Database,
        destino: &str,
    ) -> Result<usize> {
        self.copiar_tabela_para_adiando_o_fsync(origem, destino_db, destino)?
            .levar_ao_disco()
    }

    /// A copia do [`Self::copiar_tabela_para`], devolvendo o `fsync` por fazer.
    pub fn copiar_tabela_para_adiando_o_fsync(
        &self,
        origem: &str,
        destino_db: &Database,
        destino: &str,
    ) -> Result<PorSincronizar> {
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
        // cola espera, e o mesmo que `criar_tabela` faz. A pasta que nasce
        // aqui e entrada do database: o `fsync` dele vai junto da copia, senao
        // a copia sincronizada moraria numa pasta que a queda pode levar.
        let (dir_d, mut pendente) = match schema_d {
            None => (
                destino_db.caminho().to_path_buf(),
                PorSincronizar::default(),
            ),
            Some(sc) => destino_db.garantir_schema_adiando_o_fsync(sc)?,
        };
        if dir_o == dir_d && nome_o == nome_d {
            return Err(PhxError::Duplicado(
                "origem e destino sao a mesma tabela".into(),
            ));
        }
        exigir_nome_que_volta(&dir_d, nome_d)?;
        let copia = Self::copiar_os_arquivos(&dir_o, nome_o, &dir_d, nome_d)?;
        if copia.arquivos.is_empty() {
            return Err(PhxError::NaoEncontrado(format!(
                "tabela {origem} nao existe em {}",
                self.nome()
            )));
        }
        pendente.juntar(copia);
        Ok(pendente)
    }

    /// O laco das duas copias -- um so, para o `fsync` de uma nao faltar na
    /// outra, que e exatamente como o 586 nasceu (irmas do 582).
    fn copiar_os_arquivos(
        dir_o: &Path,
        nome_o: &str,
        dir_d: &Path,
        nome_d: &str,
    ) -> Result<PorSincronizar> {
        // Pedido 624: a troca DECIDIDA e nao terminada se termina antes da
        // copia, pela mesma decisao da abertura -- senao a copia levaria o
        // volume 1 numa largura e o resto na outra, sem o `*.novo` que
        // terminaria. O portao vem antes do trabalho: sem `*.novo` do `.reg`
        // ao lado, nao se abre nada e a copia e a de sempre.
        let ha_novo_do_reg = std::fs::read_dir(dir_o)?.flatten().any(|a| {
            let f = a.file_name();
            let f = f.to_string_lossy();
            pertence_ou_sobra(&f, nome_o, EXT_REG) && !pertence(&f, nome_o, EXT_REG)
        });
        if ha_novo_do_reg {
            crate::reg::RegFile::terminar_troca_antes_de_copiar(dir_o, nome_o)?;
        }
        // Pedido 605: a copia e uma tabela que NASCE, e o irmao do
        // `criar_tabela` -- a mesma reserva, antes do primeiro arquivo.
        crate::nascendo::esperar(dir_d, nome_d)?;
        let reservas = vec![crate::nascendo::reservar(dir_d, nome_d)?];
        let mut arquivos = Vec::new();
        // Copia e historia NOVA (pedido 601): UMA linhagem para todos os
        // volumes da copia e o espelho deles, que carregam o mesmo bloco.
        let linhagem = phxsql_core::uuid::Uuid::v7();
        for ext in Self::EXTENSOES {
            for arq in std::fs::read_dir(dir_o)?.flatten() {
                let f = arq.file_name();
                let f = f.to_string_lossy();
                if pertence(&f, nome_o, ext) {
                    // Preserva o sufixo do volume: `precos#002.reg` vira
                    // `copia#002.reg`, nao `copia.reg`.
                    let novo = dir_d.join(format!("{nome_d}{}", &f[nome_o.len()..]));
                    // Pelo motor da permissao (pedido 542): a copia nasce
                    // 0600, sem herdar o modo da origem.
                    let mut escrito = crate::util::copiar_do_banco(&arq.path(), &novo)?;
                    if ext == EXT_REG || ext == phxsql_core::EXT_BKP {
                        crate::reg::cunhar_linhagem_na_copia(&arq.path(), &mut escrito, linhagem)?;
                    }
                    // O atestado do pedido 522 vale para a copia: ela saiu do
                    // nucleo junto do `.reg` dela. Ver `ndx::levar_atestado`.
                    crate::ndx::levar_atestado(&arq.path(), &novo, false);
                    arquivos.push((escrito, novo));
                }
            }
        }
        Ok(PorSincronizar {
            arquivos,
            reservas,
            ..PorSincronizar::default()
        })
    }

    pub fn existe_tabela(&self, schema: Option<&str>, nome: &str) -> Result<bool> {
        Ok(self.tabelas(schema)?.iter().any(|t| t == nome))
    }

    /// O que o retrato de uma replica leva de cada tabela -- pedido 706.
    ///
    /// As extensoes da COPIA menos o espelho `.bkp`: o espelho e decisao do
    /// servidor que recebe (`espelho` no config dele), e nao da origem.
    ///
    /// A lixeira (`.trash`) e os motivos (`.reason`) VAO, desde o pedido 736:
    /// a replica refeita pode ser promovida pelo cluster, e uma primaria que
    /// nao restaura nem explica nada anterior ao retrato perdeu o que a origem
    /// guardava. O `.fts`, o `.pag` e o `.bkp` nao vao: os tres se refazem na
    /// abertura e nenhum e fonte de verdade.
    const EXTENSOES_DO_RETRATO: [&'static str; 7] =
        ["reg", "ndx", "bin", "memo", "log", "trash", "reason"];

    /// O que o retrato NAO apaga na replica -- pedido 736. O `.lgpd` e a
    /// trilha de quem leu dado pessoal NESTE servidor: o retrato nao a traz e
    /// nao a substitui, e apaga-la seria destruir prova sem ninguem pedir.
    const EXTENSOES_QUE_O_RETRATO_PRESERVA: [&'static str; 1] = ["lgpd"];

    /// Deixa uma tabela pronta para o retrato de uma replica -- pedido 706:
    /// a troca de volume do `.reg` que ficou por terminar termina aqui (e
    /// escrita, entao quem chama segura a ficha exclusiva). A copia em si e a
    /// do backup, em duas passadas (pedido 729): [`crate::backup::copiar_fase_1_das_tabelas`].
    ///
    /// # Por que a copia e fiel, e nao a de sempre
    ///
    /// O [`Self::copiar_tabela_para`] cunha uma LINHAGEM nova (pedido 601):
    /// a copia e outra historia. Aqui e o contrario -- a replica refeita
    /// continua a MESMA historia da origem, e o `aplicar` dela recusaria os
    /// eventos seguintes se a linhagem mudasse. Entao os bytes vao como
    /// estao, o `.log` inclusive: e a base dele (bytes 104..112) que faz a
    /// posicao da replica refeita ser a da origem, sem conta nenhuma.
    pub fn preparar_para_o_retrato(&self, qualificado: &str) -> Result<()> {
        if self.ha_troca_por_terminar(qualificado)? {
            let (schema, nome) = separar_qualificado(qualificado);
            let dir = self.diretorio(schema.as_deref())?;
            crate::reg::RegFile::terminar_troca_antes_de_copiar(&dir, &nome)?;
        }
        Ok(())
    }

    /// O que o retrato de uma replica le de cada tabela (pedido 729) -- se
    /// ela tem coluna marcada e quantos eventos o diario dela tem --, pela
    /// abertura de LEITURA ([`Table::abrir_para_ler_o_diario`]). `None` quando
    /// abrir exigiria escrever: quem chama decide se cura.
    ///
    /// # Por que a leitura, se quem chama segura a ficha exclusiva
    ///
    /// Porque a abertura gravavel escreve mesmo na tabela sa -- a marca do
    /// `.ndx` (pedido 522) --, e o retrato pergunta isto logo antes do acerto
    /// da fase 2: o `.ndx` «recem-mudado» seria recopiado inteiro com a
    /// escrita parada. Medido num database de 99 MiB: 93-107 ms de escrita
    /// parada com a abertura gravavel, 0 ms com esta.
    pub fn ler_para_o_retrato(&self, qualificado: &str) -> Result<Option<(bool, u64)>> {
        self.exigir_motor_padrao()?;
        let (schema, nome) = separar_qualificado(qualificado);
        validar_nome("tabela", &nome)?;
        let dir = self.diretorio(schema.as_deref())?;
        Ok(match Table::abrir_para_ler_o_diario(&dir, &nome)? {
            SemEscrever::Aberta(mut t) => Some((t.tem_dado_pessoal(), t.eventos()?)),
            SemEscrever::PrecisaEscrever(_) => None,
        })
    }

    /// Ha `*.novo` do `.reg` desta tabela ao lado -- uma troca de volume por
    /// terminar? So le a pasta: e a pergunta que a fase 2 do retrato faz com
    /// a ficha de LEITURA, para nao copiar o `.reg` de antes da troca.
    pub fn ha_troca_por_terminar(&self, qualificado: &str) -> Result<bool> {
        self.exigir_motor_padrao()?;
        let (schema, nome) = separar_qualificado(qualificado);
        validar_nome("tabela", &nome)?;
        let dir = self.diretorio(schema.as_deref())?;
        Ok(std::fs::read_dir(&dir)?.flatten().any(|a| {
            let f = a.file_name();
            let f = f.to_string_lossy();
            pertence_ou_sobra(&f, &nome, EXT_REG) && !pertence(&f, &nome, EXT_REG)
        }))
    }

    /// Troca os arquivos de uma tabela pelos de um retrato -- pedido 706, o
    /// lado da replica.
    ///
    /// Todo arquivo da tabela sai (inclusive a lixeira, os motivos e a
    /// trilha, que eram desta replica e ja nao contam a historia do retrato)
    /// e cada `(arquivo, copia)` entra pelo `rename`. O nome de cada um tem de
    /// ser DESTA tabela e de uma extensao do retrato: o nome vem do fio, e um
    /// `../` ou o arquivo de outra tabela nao pode entrar por aqui.
    ///
    /// Os dados das copias ja foram ao disco por quem as escreveu; o `fsync`
    /// da pasta volta em [`PorSincronizar`], para depois de soltar a trava.
    pub fn trocar_pelo_retrato(
        &self,
        qualificado: &str,
        novos: &[(String, PathBuf)],
    ) -> Result<PorSincronizar> {
        self.exigir_motor_padrao()?;
        let (schema, nome) = separar_qualificado(qualificado);
        validar_nome("tabela", &nome)?;
        for (arquivo, _) in novos {
            let valido = !arquivo.contains(['/', '\\'])
                && Self::EXTENSOES_DO_RETRATO
                    .iter()
                    .any(|ext| pertence(arquivo, &nome, ext));
            if !valido {
                return Err(PhxError::Esquema(format!(
                    "o retrato da tabela {qualificado} traz o arquivo {arquivo:?}, que \
                     nao e dela (pedido 706)"
                )));
            }
        }
        // O schema vem do fio tambem, e passa pelo MESMO `diretorio` que toda
        // operacao de catalogo usa -- pedido 726. Antes ele ia direto ao
        // `join`, e `Path::join` com caminho absoluto SUBSTITUI a base: a
        // origem que mandasse `/srv/phxsql/base/central.produtos` apagava e
        // reescrevia o `produtos` de OUTRO database desta replica.
        let dir = self.diretorio(schema.as_deref())?;
        if schema.is_some() {
            crate::util::criar_diretorio_do_banco(&dir)?;
        }
        let mut mexidos = Vec::new();
        for ext in Self::EXTENSOES_TODAS {
            if Self::EXTENSOES_QUE_O_RETRATO_PRESERVA.contains(&ext) {
                continue;
            }
            for arq in std::fs::read_dir(&dir)?.flatten() {
                let f = arq.file_name();
                if pertence_ou_sobra(&f.to_string_lossy(), &nome, ext) {
                    std::fs::remove_file(arq.path())?;
                    mexidos.push(arq.path());
                }
            }
        }
        crate::volume::mudar_pendentes_de_nome(&dir, &nome, None);
        for (arquivo, copia) in novos {
            let para = dir.join(arquivo);
            std::fs::rename(copia, &para)?;
            mexidos.push(para);
        }
        Ok(PorSincronizar::entradas_que_sairam(mexidos))
    }
}

/// A tabela (qualificada) cujo arquivo de retrato e `rel` -- o caminho
/// relativo a pasta do database, com barra normal: `t.reg`, `t#001.log`,
/// `sc/t.ndx`. `None` para o que o retrato nao leva (a trilha `.lgpd`, o
/// `.fts`, o `*.novo`, a sequencia). E o filtro das duas passadas do retrato
/// (pedido 729), e mora aqui porque a lista de extensoes e daqui.
pub fn tabela_do_arquivo_do_retrato(rel: &str) -> Option<String> {
    let (schema, arquivo) = match rel.split_once('/') {
        Some((s, a)) if !a.contains('/') => (Some(s), a),
        Some(_) => return None,
        None => (None, rel),
    };
    let (sem_ext, ext) = arquivo.rsplit_once('.')?;
    if !Database::EXTENSOES_DO_RETRATO.contains(&ext) {
        return None;
    }
    let nome = separar_volume(sem_ext).map_or(sem_ext, |(t, _)| t);
    validar_nome("tabela", nome).ok()?;
    if let Some(s) = schema {
        validar_nome("schema", s).ok()?;
    }
    Some(qualificar(schema, nome))
}

/// O prefixo de TODO arquivo de retrato da replica na raiz de dados (pedido
/// 706): o servido e o recebido comecam por ele. Mora aqui, e nao no
/// servidor, porque o backup precisa pula-lo (pedido 731) e o nome tem de
/// ser UM so para os dois.
pub const PREFIXO_DO_RETRATO_DA_REPLICA: &str = ".retrato-";

/// Grava um pedaco de um arquivo de retrato recebido pelo fio (pedido 706),
/// no `offset` dele, pelo motor da permissao: o retrato e dado da tabela e
/// nasce 0600 como ela. O primeiro pedaco (`offset` zero) recria o arquivo.
pub fn gravar_pedaco_do_retrato(caminho: &Path, offset: u64, bytes: &[u8]) -> Result<()> {
    use std::io::{Seek, SeekFrom, Write};
    let mut f = crate::util::opcoes_do_banco()
        .write(true)
        .create(true)
        .open(caminho)?;
    if offset == 0 {
        f.set_len(0)?;
    }
    f.seek(SeekFrom::Start(offset))?;
    f.write_all(bytes)?;
    Ok(())
}

/// Leva um arquivo de retrato inteiro ao disco, ANTES de ele virar arquivo de
/// tabela pelo `rename` -- e fora da trava global.
pub fn sincronizar_arquivo_do_retrato(caminho: &Path) -> Result<()> {
    let f = std::fs::File::open(caminho)?;
    crate::sincronia::sync_all(&f, caminho)
}

/// O que uma criacao ou copia de catalogo ja fez nos nomes novos e ainda deve
/// ao disco -- pedidos 586 (a copia de tabela) e 589 (criar database, schema
/// e tabela).
///
/// # Por que o `fsync` sai da operacao
///
/// No servidor essas operacoes rodam com a trava global na mao, e a catraca
/// `alcancam-fsync-2` do `mapa-da-trava.py` proibe `fsync` novo ali: cada um
/// e a proxima conexao esperando o disco. Criar e copiar precisam da trava (e
/// ela que impede a origem de mudar no meio, e dois criadores de colidirem no
/// mesmo nome); sincronizar nao precisa -- o `fsync` e do inode e vale por
/// qualquer descritor que o escreveu, e so este tem a garantia do
/// *fsyncgate* (pedido 552). Entao a operacao devolve os descritores
/// abertos, e quem chama solta a trava e leva ao disco antes de responder.
///
/// A janela entre soltar e sincronizar e a de qualquer escrita sem `fsync`
/// para QUEM CRIOU: ninguem ouviu «ok» por ela ainda. Para um TERCEIRO ela
/// nao era: a tabela ja era visivel, e o «ok» dele saia antes do `fsync` da
/// pasta (pedido 605, medido). Por isso o nome que nasce vai RESERVADO aqui
/// dentro ([`crate::nascendo`]) e so se publica no fim do
/// [`Self::levar_ao_disco`].
///
/// # Um tipo so para as quatro operacoes
///
/// O 589 e o 586 de novo: o colar ganhou o `fsync` do database quando criava
/// a pasta do schema, e o `criar_schema` e o `criar_tabela`, que chamavam o
/// MESMO `garantir_schema`, ficaram sem. Aqui a decisao «o que deve o disco,
/// e em que ordem» mora uma vez; quem cria so diz o que criou.
#[must_use = "a criacao so vale depois de levar_ao_disco"]
#[derive(Default)]
pub struct PorSincronizar {
    /// Os arquivos novos, cada um no descritor que o escreveu.
    arquivos: Vec<(std::fs::File, PathBuf)>,
    /// As pastas que nasceram: a entrada de cada uma e dado da pasta MAE.
    entradas_novas: Vec<PathBuf>,
    /// Os nomes que SAIRAM (pedido 591): o `unlink` tambem e dado da pasta,
    /// e sem o `fsync` dela a tabela excluida volta numa queda, inteira ou
    /// pela metade -- depois de o cliente ouvir «excluida».
    entradas_que_sairam: Vec<PathBuf>,
    /// Os nomes que NASCEM aqui, reservados ate o fim do [`Self::levar_ao_disco`]
    /// -- pedido 605. Saem no `Drop`: no caminho feliz depois do ultimo
    /// `fsync`, no de erro junto com o resto.
    reservas: Vec<crate::nascendo::Reserva>,
}

impl PorSincronizar {
    /// Uma pasta que acabou de nascer, e nada mais por enquanto.
    fn entrada_nova(pasta: &Path) -> PorSincronizar {
        PorSincronizar {
            entradas_novas: vec![pasta.to_path_buf()],
            ..PorSincronizar::default()
        }
    }

    /// Os nomes que uma exclusao apagou -- pedido 591. Quem apaga numa pasta
    /// so precisa dar UM nome dela: o `fsync` e da pasta, e o
    /// [`Self::levar_ao_disco`] faz um por pasta, por mais nomes que saiam.
    ///
    /// Publico desde o 595: o cadastro do servidor (`gatilhos.json`,
    /// `procedimentos.json`, `visoes.json`) apaga o arquivo quando a ultima
    /// rotina sai, e o `fsync` da pasta dele e este mesmo -- nao um segundo.
    pub fn entradas_que_sairam(nomes: Vec<PathBuf>) -> PorSincronizar {
        PorSincronizar {
            entradas_que_sairam: nomes,
            ..PorSincronizar::default()
        }
    }

    /// Soma o que outra etapa da mesma operacao ficou devendo -- o schema que
    /// o colar criou, e a copia que foi morar nele.
    pub fn juntar(&mut self, outra: PorSincronizar) {
        self.arquivos.extend(outra.arquivos);
        self.entradas_novas.extend(outra.entradas_novas);
        self.entradas_que_sairam.extend(outra.entradas_que_sairam);
        self.reservas.extend(outra.reservas);
    }

    /// O `fsync` de cada arquivo NO DESCRITOR QUE O ESCREVEU, e depois o de
    /// cada pasta que ganhou entrada -- a ordem do `durable_rename` do
    /// PostgreSQL: a entrada so vale depois de o conteudo para o qual ela
    /// aponta estar no disco. As pastas vao da mais funda para a mais rasa:
    /// a do schema (onde a tabela nasceu) antes do database (onde o schema
    /// nasceu), antes da base (onde o database nasceu). Pelo motor
    /// [`crate::sincronia`], com o gancho do processo: e dado do banco.
    ///
    /// Devolve quantos arquivos foram levados.
    pub fn levar_ao_disco(self) -> Result<usize> {
        for (arquivo, caminho) in &self.arquivos {
            crate::sincronia::sync_all(arquivo, caminho)?;
        }
        // Cada pasta vai pelo nome de uma entrada dela: e pelo pai do caminho
        // que a `sincronia` marca a recusa. Uma por pasta, por mais arquivos
        // que tenham nascido nela.
        let mut pastas: std::collections::BTreeMap<PathBuf, &Path> =
            std::collections::BTreeMap::new();
        for (_, caminho) in &self.arquivos {
            pastas.entry(pai_de(caminho)).or_insert(caminho);
        }
        for entrada in &self.entradas_novas {
            pastas.entry(pai_de(entrada)).or_insert(entrada);
        }
        // O nome que saiu nao existe mais, mas a pasta dele sim, e e por ela
        // que a `sincronia` abre e marca -- o mesmo calculo das outras duas.
        for entrada in &self.entradas_que_sairam {
            pastas.entry(pai_de(entrada)).or_insert(entrada);
        }
        let mut ordem: Vec<(PathBuf, &Path)> = pastas.into_iter().collect();
        ordem.sort_by_key(|(pasta, _)| std::cmp::Reverse(pasta.components().count()));
        for (_, entrada) in ordem {
            crate::sincronia::sincronizar_os_diretorios(entrada, entrada, true)?;
        }
        let levados = self.arquivos.len();
        // A PUBLICACAO do pedido 605, e so aqui: o ultimo `fsync` de pasta
        // voltou, e a reserva sai -- quem esperava pelo nome acorda. Escrita
        // e nao deixada ao fim do escopo, para ninguem mover um `fsync` para
        // depois dela sem ver.
        drop(self);
        Ok(levados)
    }
}

/// A pasta de um caminho, como a `sincronia` a calcula: vazio vira `.`.
fn pai_de(caminho: &Path) -> PathBuf {
    match caminho.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// O que o DISCO responde sobre «o `.reg` desta tabela esta cifrado?».
///
/// Quatro estados, e nao um `bool` nem um `Option<bool>`, porque quem pergunta
/// decide COISAS DIFERENTES em cada um -- e as duas ausencias de resposta nao
/// se parecem: «nao ha arquivo nenhum com esse nome» e «ha um `.reg` ali e nao
/// consegui ler o cabecalho dele». Empacotar as duas num `None` obrigaria quem
/// chama a escolher o mesmo destino para as duas, e uma delas e a tabela que
/// talvez esteja cifrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegNoDisco {
    /// Nao ha volume de `.reg` com esse nome: a tabela nao existe (ainda), o
    /// database nao existe, ou o nome nem e nome de tabela.
    SemVolume,
    /// Achei o volume, e o cabecalho dele diz CIFRADO.
    Cifrado,
    /// Achei o volume, e o cabecalho dele diz EM CLARO.
    EmClaro,
    /// Achei o volume e NAO consegui ler o cabecalho: truncado, corrompido, ou
    /// de uma versao que este binario nao conhece.
    Ilegivel,
}

/// O `.reg` desta tabela esta CIFRADO, segundo o DISCO -- sem abrir a tabela.
///
/// # Por que aqui, e nao em quem pergunta
///
/// Porque quem pergunta e o **Profiler** (pedido 356), e ele so tem os nomes
/// que vieram no pedido -- ou seja, de FORA. Compor `base/database/tabela.reg`
/// la em cima seria uma segunda copia da regra de caminho desta casa, e seria
/// uma copia sem `validar_nome`: um `"tabela": "../../etc/passwd"` faria o
/// servidor ir perguntar ao disco por um arquivo fora da base. A resolucao de
/// caminho mora num lugar so, e e aqui.
pub fn reg_cifrado(base: &Path, database: &str, qualificado: &str) -> RegNoDisco {
    let Some(dir) = diretorio_da_tabela(base, database, qualificado) else {
        return RegNoDisco::SemVolume;
    };
    let (_, nome) = separar_qualificado(qualificado);
    let Some(volume) = crate::reg::primeiro_volume(&dir, &nome) else {
        return RegNoDisco::SemVolume;
    };
    match crate::reg::cifrado_no_volume(&volume) {
        Some(true) => RegNoDisco::Cifrado,
        Some(false) => RegNoDisco::EmClaro,
        None => RegNoDisco::Ilegivel,
    }
}

/// O diretorio em que a tabela mora, com todo nome conferido antes de virar
/// caminho. `None` para nome que nao passa no [`validar_nome`].
/// A espera do pedido 605 pelos NOMES do pedido, para quem ainda nao abriu
/// nada -- o servidor, antes de tomar a trava global (ver `crate::nascendo`).
/// Sem prazo, porque quem espera aqui nao segura ninguem; chamar com a trava
/// na mao seria o pedido 629 de volta.
///
/// Pela mesma resolucao de caminho do [`reg_cifrado`], e pelo mesmo motivo:
/// compor `base/database/tabela` no servidor seria uma segunda copia da regra
/// de caminho, sem o `validar_nome`. Nome invalido nao espera: a recusa dele
/// vem depois, de quem abre. Sem `qualificado`, espera so pelo database.
pub fn esperar_pelo_nome(base: &Path, database: &str, qualificado: &str) {
    if crate::nascendo::quantas() == 0 || database.is_empty() {
        return;
    }
    if qualificado.is_empty() {
        if validar_nome("database", database).is_ok() {
            crate::nascendo::esperar_fora_da_trava(base, database);
        }
        return;
    }
    if let Some(dir) = diretorio_da_tabela(base, database, qualificado) {
        let (_, nome) = separar_qualificado(qualificado);
        crate::nascendo::esperar_fora_da_trava(&dir, &nome);
    }
}

fn diretorio_da_tabela(base: &Path, database: &str, qualificado: &str) -> Option<PathBuf> {
    validar_nome("database", database).ok()?;
    let (schema, nome) = separar_qualificado(qualificado);
    validar_nome("tabela", &nome).ok()?;
    let mut dir = base.join(database);
    if let Some(s) = schema {
        validar_nome("schema", &s).ok()?;
        dir = dir.join(s);
    }
    Some(dir)
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
        assert!(base.join("Z/grande#004.reg").exists());
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
            nome_da_tabela(Path::new("cadastroClientes#007.reg")).as_deref(),
            Some("cadastroClientes")
        );
        assert_eq!(
            nome_da_tabela(Path::new("cadastroClientes#Outros.reg")).as_deref(),
            Some("cadastroClientes")
        );
        // Sublinhado nunca e sufixo de volume desde o pedido 508: fica no
        // nome, com digitos ou com letra de balde depois dele.
        assert_eq!(
            nome_da_tabela(Path::new("cadastro_clientes.reg")).as_deref(),
            Some("cadastro_clientes")
        );
        assert_eq!(
            nome_da_tabela(Path::new("vendas_2024.reg")).as_deref(),
            Some("vendas_2024")
        );
        assert_eq!(
            nome_da_tabela(Path::new("dados_X.reg")).as_deref(),
            Some("dados_X")
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
        assert!(pertence("precos#001.reg", "precos", "reg"));
        assert!(pertence("precos#00042.reg", "precos", "reg"));
        assert!(pertence("precos#A.reg", "precos", "reg"));

        assert!(!pertence("precos_historico.reg", "precos", "reg"));
        assert!(!pertence("precos#historico.reg", "precos", "reg"));
        assert!(!pertence("precos2.reg", "precos", "reg"));
        assert!(!pertence("precos#.reg", "precos", "reg"));
        assert!(!pertence("precos#1a.reg", "precos", "reg"));
        // Pedido 508: `precos_2024` e OUTRA tabela, e excluir `precos` nao a
        // leva -- com o separador `_` levava.
        assert!(!pertence("precos_2024.reg", "precos", "reg"));
        assert!(!pertence("precos_A.reg", "precos", "reg"));
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

    /// O retrato de `a.t` (pedido 706) para a pasta `destino`, no formato que
    /// a replica recebe: `(arquivo, copia)`.
    fn retrato_de_t(base: &Path, a: &Database) -> Vec<(String, PathBuf)> {
        a.preparar_para_o_retrato("t").unwrap();
        let destino = base.join(".retrato-servido-teste");
        let mut fase = crate::backup::copiar_fase_1_das_tabelas(
            &base.join("a"),
            &destino,
            ["t".to_string()].into_iter().collect(),
        )
        .unwrap();
        crate::backup::acertar_fase_2(&mut fase, &std::collections::BTreeMap::new()).unwrap();
        crate::backup::copias_por_caminho(&fase)
            .into_iter()
            .map(|(rel, copia, _)| (rel, copia))
            .collect()
    }

    /// Pedido 729: o filtro das duas passadas do retrato leva o que a troca
    /// aceita, e deixa de fora a trilha, o `.fts` e o `*.novo`.
    #[test]
    fn o_filtro_do_retrato_leva_so_os_arquivos_da_tabela() {
        let t = tabela_do_arquivo_do_retrato;
        assert_eq!(t("t.reg").as_deref(), Some("t"));
        assert_eq!(t("t#001.log").as_deref(), Some("t"));
        assert_eq!(t("sc/t.ndx").as_deref(), Some("sc.t"));
        assert_eq!(t("t.trash").as_deref(), Some("t"));
        assert_eq!(t("t.lgpd"), None);
        assert_eq!(t("t.fts"), None);
        assert_eq!(t("t.reg.novo"), None);
        assert_eq!(t("a/b/t.reg"), None);
    }

    /// Pedido 736 (R1 do parecer C): a troca pelo retrato NAO apaga o
    /// `.lgpd` da replica -- a trilha de quem leu dado pessoal AQUI, que o
    /// retrato nao traz --, e a lixeira e os motivos da origem VEM junto.
    ///
    /// Defeito reposto (o `.lgpd` na varredura de apagar, e `trash`/`reason`
    /// fora do retrato): a trilha some e a replica fica sem lixeira.
    #[test]
    fn o_retrato_preserva_a_trilha_lgpd_e_traz_lixeira_e_motivos() {
        let base = base_temp("retrato-736");
        let inst = Instancia::nova(&base).unwrap();
        let a = inst.criar_database("a").unwrap();
        a.criar_tabela(None, esquema_simples("t")).unwrap();
        std::fs::write(base.join("a/t.trash"), b"LIXEIRA-DA-ORIGEM").unwrap();
        std::fs::write(base.join("a/t.reason"), b"MOTIVO-DA-ORIGEM").unwrap();
        let b = inst.criar_database("b").unwrap();
        b.criar_tabela(None, esquema_simples("t")).unwrap();
        std::fs::write(base.join("b/t.lgpd"), b"TRILHA-DA-REPLICA").unwrap();

        let novos = retrato_de_t(&base, &a);
        b.trocar_pelo_retrato("t", &novos)
            .unwrap()
            .levar_ao_disco()
            .unwrap();
        assert_eq!(
            std::fs::read(base.join("b/t.lgpd")).unwrap_or_default(),
            b"TRILHA-DA-REPLICA",
            "a troca pelo retrato apagou a trilha LGPD da replica"
        );
        assert_eq!(
            std::fs::read(base.join("b/t.trash")).unwrap_or_default(),
            b"LIXEIRA-DA-ORIGEM",
            "a lixeira da origem nao veio no retrato"
        );
        assert_eq!(
            std::fs::read(base.join("b/t.reason")).unwrap_or_default(),
            b"MOTIVO-DA-ORIGEM",
            "os motivos da origem nao vieram no retrato"
        );
    }

    /// Pedido 726 (A1 da revisao SEC): o schema do nome que veio do fio passa
    /// pelo `validar_nome`. Com caminho absoluto, `Path::join` SUBSTITUI a
    /// base -- e a troca apagava e reescrevia a tabela de outro database.
    ///
    /// Defeito reposto (`self.caminho().join(sc)`): `b/t.reg` vira a copia
    /// de `a`, e o teste cai.
    #[test]
    fn o_retrato_com_schema_absoluto_nao_sai_do_database() {
        let base = base_temp("retrato-726");
        let inst = Instancia::nova(&base).unwrap();
        let a = inst.criar_database("a").unwrap();
        a.criar_tabela(None, esquema_simples("t")).unwrap();
        let b = inst.criar_database("b").unwrap();
        b.criar_tabela(None, esquema_simples("t")).unwrap();
        let c = inst.criar_database("c").unwrap();
        // Uma linha so na origem: a copia tem de ser DIFERENTE do que esta em
        // `b`, senao a troca indevida passaria despercebida.
        let mut t = a.abrir_qualificada("t").unwrap();
        t.inserir(&[phxsql_core::value::Value::Int(7)]).unwrap();
        t.sincronizar().unwrap();
        drop(t);
        let antes = std::fs::read(base.join("b/t.reg")).unwrap();
        let novos = retrato_de_t(&base, &a);
        let alvo = format!("{}.t", base.join("b").display());
        assert!(!base.join("b").display().to_string().contains('.'));
        let Err(e) = c.trocar_pelo_retrato(&alvo, &novos) else {
            panic!("o schema absoluto foi aceito");
        };
        assert!(matches!(e, PhxError::Esquema(_)), "{e}");
        assert_eq!(
            std::fs::read(base.join("b/t.reg")).unwrap(),
            antes,
            "a troca escreveu no database vizinho"
        );
    }

    /// Pedido 229: a sequencia nomeada mora na pasta do database (ou do
    /// schema), o `fsync` dela sai pelo `PorSincronizar` como o da tabela,
    /// e o nome e dividido com a tabela nos dois sentidos.
    #[test]
    fn sequencia_nomeada_mora_ao_lado_das_tabelas_e_divide_o_nome() {
        use crate::sequencia::Definicao;
        let base = base_temp("seq-nomeada");
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("banco").unwrap();
        db.criar_tabela(None, esquema_simples("precos")).unwrap();

        let mut s = db
            .criar_sequencia(None, "nf", Definicao::default())
            .unwrap();
        assert!(base.join("banco/nf.seq").is_file());
        assert_eq!(s.proximo().unwrap(), 1);
        drop(s);
        let (mut s, _posse) = db.abrir_sequencia(None, "nf").unwrap();
        assert_eq!(s.proximo().unwrap(), 2);

        // No schema, pela mesma porta -- e o schema tem de existir antes,
        // como no PostgreSQL e no MariaDB.
        assert!(matches!(
            db.criar_sequencia(Some("filial"), "nf", Definicao::default())
                .unwrap_err(),
            PhxError::NaoEncontrado(_)
        ));
        db.criar_schema("filial").unwrap();
        db.criar_sequencia(Some("filial"), "nf", Definicao::default())
            .unwrap();
        assert!(base.join("banco/filial/nf.seq").is_file());
        assert_eq!(
            db.todas_as_sequencias_nomeadas().unwrap(),
            vec!["filial.nf", "nf"]
        );

        // O nome e um so: tabela nao nasce com nome de sequencia, nem o
        // contrario. E a tabela `precos` continua sendo so tabela.
        let e = match db.criar_tabela(None, esquema_simples("nf")) {
            Ok(_) => panic!("a tabela nasceu com o nome da sequencia"),
            Err(e) => e,
        };
        assert!(matches!(e, PhxError::Duplicado(_)), "{e}");
        let e = db
            .criar_sequencia(None, "precos", Definicao::default())
            .unwrap_err();
        assert!(matches!(e, PhxError::Duplicado(_)), "{e}");
        assert_eq!(db.tabelas(None).unwrap(), vec!["precos"]);

        // Apagar a tabela homonima de outro schema nao leva a sequencia, e
        // apagar a sequencia nao leva tabela nenhuma.
        db.excluir_sequencia(None, "nf").unwrap();
        assert!(!base.join("banco/nf.seq").exists());
        assert_eq!(db.tabelas(None).unwrap(), vec!["precos"]);
        assert!(matches!(
            db.abrir_sequencia(None, "nf").unwrap_err(),
            PhxError::NaoEncontrado(_)
        ));
    }

    /// Pedido 428: apagar e renomear mexem nos arquivos SEM abrir a tabela, e
    /// por isso nao passavam pelo portao do congelamento. Com a tabela em
    /// reescrita, os dois tem de recusar com `EmMigracao` -- e soltar quando
    /// a reescrita acaba (o comportamento velho).
    #[test]
    fn apagar_e_renomear_respeitam_o_congelamento() {
        let base = base_temp("congelada-apagar-renomear");
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("banco").unwrap();
        db.criar_tabela(None, esquema_simples("precos")).unwrap();
        let dir = db.diretorio(None).unwrap();
        {
            // Congelada pela OUTRA grafia: a chave nao distingue caixa.
            let _posse = crate::congelamento::congelar(&dir, "Precos", "teste").unwrap();
            let e = db.excluir_tabela("precos").unwrap_err();
            assert!(matches!(e, PhxError::EmMigracao(_)), "excluir: {e:?}");
            let e = db.renomear_tabela("precos", "tarifas").unwrap_err();
            assert!(matches!(e, PhxError::EmMigracao(_)), "renomear: {e:?}");
            assert!(db.existe_tabela(None, "precos").unwrap(), "a tabela saiu");
        }
        assert!(db.renomear_tabela("precos", "tarifas").unwrap() > 0);
        assert!(!db.excluir_tabela("tarifas").unwrap().is_empty());
    }

    /// O comportamento VELHO da varredura, travado antes de ela ficar barata.
    ///
    /// `tabelas_em` deixou de perguntar ao nucleo "isto e' arquivo?" em cada
    /// uma das oito extensoes -- olha a extensao primeiro e usa o `d_type` que
    /// o `getdents64` ja trouxe. As duas pontas que a troca podia quebrar
    /// calada estao aqui:
    ///
    /// - **o elo simbolico**: `file_type()` nao segue o elo e `is_file()`
    ///   segue. Sem a volta ao `is_file()`, uma tabela alcancada por elo
    ///   sumiria da lista -- e tabela que some da lista e' tabela que a busca
    ///   reversa nao pergunta se tem filha, que e' a petrea da integridade
    ///   perdida em silencio. **Prova real**: tirar o ramo `is_symlink` faz
    ///   este teste falhar.
    /// - **o diretorio com nome de tabela**: `dados.reg/` nao e' tabela, e o
    ///   `d_type` tem de recusa-lo como o `is_file()` recusava.
    #[test]
    fn a_varredura_barata_ve_o_mesmo_que_a_cara() {
        let base = base_temp("varredura-barata");
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("banco").unwrap();
        db.criar_tabela(None, esquema_simples("precos")).unwrap();
        let dir = base.join("banco");

        // Um diretorio com nome de tabela: nunca foi tabela, continua nao
        // sendo.
        std::fs::create_dir_all(dir.join("pasta.reg")).unwrap();

        // Uma tabela alcancada por ELO: o arquivo mora fora e o `.reg` daqui
        // e' um elo para o de la'.
        let fora = base.join("fora-do-banco");
        std::fs::create_dir_all(&fora).unwrap();
        std::fs::copy(dir.join("precos.reg"), fora.join("espelho.reg")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(fora.join("espelho.reg"), dir.join("espelho.reg")).unwrap();

        let achadas = tabelas_em(&dir).unwrap();
        assert!(
            achadas.contains(&"precos".to_string()),
            "a tabela normal sumiu da varredura: {achadas:?}"
        );
        assert!(
            !achadas.contains(&"pasta".to_string()),
            "um DIRETORIO chamado pasta.reg entrou como tabela: {achadas:?}"
        );
        #[cfg(unix)]
        assert!(
            achadas.contains(&"espelho".to_string()),
            "a tabela alcancada por elo simbolico sumiu da varredura: {achadas:?}"
        );
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

    /// **Pedido 601, obrigacao 1 do DBA: copia e historia NOVA.** Paginada
    /// (tres volumes) e espelhada, para provar que TODO volume e o espelho
    /// de cada um saem com a MESMA linhagem nova -- e que a copia reabre,
    /// com os rowids de sempre.
    ///
    /// **Defeito reposto**: tirar o `cunhar_linhagem_na_copia` do laco da
    /// copia -- a copia sai com a linhagem da origem e o `assert_ne` cai.
    #[test]
    fn a_copia_de_tabela_nasce_com_linhagem_nova_em_todo_volume() {
        use phxsql_core::paginacao::Paginacao;
        use phxsql_core::value::Value;
        let base = base_temp("linhagem-da-copia");
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("Z").unwrap();
        let outro = inst.criar_database("W").unwrap();
        let e = esquema_simples("precos")
            .com_paginacao(Paginacao::nova(2, 10).unwrap())
            .unwrap();
        let mut t = crate::table::Table::criar_espelhada(db.caminho(), e).unwrap();
        for i in 1..=5i64 {
            t.inserir(&[Value::Int(i * 10)]).unwrap();
        }
        t.sincronizar().unwrap();
        let da_origem = t.esquema().linhagem().expect("tabela nova sem linhagem");
        drop(t);

        db.duplicar_tabela("precos", "copia").unwrap();
        db.copiar_tabela_para("precos", &outro, "colada").unwrap();
        let mut vistas = Vec::new();
        for (d, nome) in [(&db, "copia"), (&outro, "colada")] {
            let mut volumes = 0;
            for arq in std::fs::read_dir(d.caminho()).unwrap().flatten() {
                let f = arq.file_name().to_string_lossy().to_string();
                if f.starts_with(nome) && (f.ends_with(".reg") || f.ends_with(".bkp")) {
                    let b = std::fs::read(arq.path()).unwrap();
                    let cab = if u16::from_le_bytes([b[8], b[9]]) == 5 {
                        192
                    } else {
                        128
                    };
                    let len = u32::from_le_bytes(b[52..56].try_into().unwrap()) as usize;
                    let e = Schema::desserializar(&b[cab..cab + len]).unwrap();
                    vistas.push((nome, f, e.linhagem().unwrap()));
                    volumes += 1;
                }
            }
            assert!(
                volumes >= 6,
                "{nome}: {volumes} volume(s) -- a premissa e 3 + 3 espelhos"
            );
            let mut c = d.abrir_tabela(None, nome).unwrap();
            assert_eq!(c.ler(5).unwrap().unwrap()[0], Value::Int(50));
        }
        let copia = vistas.iter().find(|v| v.0 == "copia").unwrap().2;
        let colada = vistas.iter().find(|v| v.0 == "colada").unwrap().2;
        assert_ne!(copia, da_origem, "a copia levou a historia da origem");
        assert_ne!(colada, da_origem, "a colada levou a historia da origem");
        assert_ne!(copia, colada);
        for (nome, f, l) in &vistas {
            let esperada = if *nome == "copia" { copia } else { colada };
            assert_eq!(
                *l, esperada,
                "{f}: volume com linhagem diferente dos irmaos"
            );
        }
        // E a origem nao mudou.
        let o = db.abrir_tabela(None, "precos").unwrap();
        assert_eq!(o.esquema().linhagem(), Some(da_origem));
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

    /// Pedido 467: o renomear so responde depois do `fsync` do diretorio --
    /// sem ele, uma queda logo depois podia voltar com o nome velho depois de
    /// a resposta ter dito que a tabela mudou de nome.
    #[test]
    fn renomear_so_responde_depois_do_fsync_do_diretorio() {
        let base = crate::apoio_teste::DirTemp::novo("phx-renom-fsync");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();
        let mut t = db.criar_tabela(None, esquema("pedidos")).unwrap();
        t.inserir(&[Value::Int(1), Value::Str("um".into())])
            .unwrap();
        t.sincronizar().unwrap();
        drop(t);
        let dir = base.join("loja");
        crate::sincronia::falha_de_teste::armar(
            &dir,
            crate::sincronia::falha_de_teste::Onde::Fsync,
            1,
        );
        let r = db.renomear_tabela("pedidos", "pedidos_novo");
        crate::sincronia::falha_de_teste::desarmar(&dir);
        assert!(r.is_err(), "o renomear respondeu sem o fsync do diretorio");
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

    /// **O irmao do pedido 491 no catalogo:** a auto-referencia tambem guarda
    /// a mae pelo NOME. O `quem_aponta_para` pula a propria tabela -- certo no
    /// `excluir_tabela`, que leva a chave junto --, e o renomear deixava
    /// `equipe.chefe_id -> funcionarios.id`: a chave para de reconhecer a
    /// propria tabela, e o chefe com subordinado passava a sair, orfao calado.
    ///
    /// # Prova real
    ///
    /// Sem a recusa, o renomear responde `Ok` e o `excluir_de_vez` do chefe na
    /// tabela renomeada tambem -- o vermelho medido antes do conserto.
    #[test]
    fn renomear_recusa_a_tabela_que_aponta_para_si_mesma() {
        use phxsql_core::schema::ForeignKey;
        let base = crate::apoio_teste::DirTemp::novo("phx-renom-auto");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();
        let e = Schema::new(
            "funcionarios",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("chefe_id", ColumnType::Int8),
            ],
            vec![
                IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria(),
                IndexDef::new("porChefe", vec![IndexColumn::asc(1)]),
            ],
        )
        .unwrap()
        .com_chaves_estrangeiras(vec![ForeignKey::new(
            "fk_chefe",
            vec![1],
            "funcionarios",
            vec!["id".into()],
        )])
        .unwrap();
        db.criar_tabela(None, e).unwrap();

        let r = db.renomear_tabela("funcionarios", "equipe");
        let depois = if r.is_ok() {
            // O dano, se passou: a regra do pai que tem filhos morreu nela.
            let mut t = crate::table::Table::abrir(db.diretorio(None).unwrap(), "equipe").unwrap();
            let chefe = t.inserir(&[Value::Int(1), Value::Null]).unwrap();
            t.inserir(&[Value::Int(2), Value::Int(1)]).ok();
            format!("excluir o chefe: {:?}", t.excluir_de_vez(chefe, "x"))
        } else {
            String::new()
        };
        let e = r.expect_err(&format!(
            "a tabela que aponta para si mesma foi renomeada -- {depois}"
        ));
        let t = e.to_string();
        assert!(
            t.contains("fk_chefe"),
            "a recusa tem de nomear a chave: {t}"
        );
        assert!(db.existe_tabela(None, "funcionarios").unwrap());
        assert!(!db.existe_tabela(None, "equipe").unwrap());
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

    /// **O achado que a propria prova deste renomear trouxe** -- e o que o
    /// pedido 508 fez com ele.
    ///
    /// A primeira versao do teste acima renomeava para `pedidos_2025`. Moveu
    /// os oito arquivos, devolveu sucesso -- e a tabela SUMIU da arvore,
    /// porque com o separador `_` o `nome_da_tabela` lia `pedidos_2025.reg`
    /// como o volume 2025 de `pedidos`. Ate o 508 a saida foi recusar o nome.
    /// Com o separador `#`, `pedidos_2025` e nome como outro qualquer: o
    /// renomear passa e a tabela VOLTA pelo nome novo. O que recusa agora e o
    /// `#`, e a recusa diz por que.
    ///
    /// **Defeito reposto** (`SEPARADOR_DE_VOLUME` de volta a `_`): a tabela
    /// renomeada some da listagem e o `existe_tabela` cai.
    #[test]
    fn renomear_para_nome_com_sublinhado_e_digitos_volta_como_ele_mesmo() {
        // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
        let base = crate::apoio_teste::DirTemp::novo("phx-renom-vol");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();
        db.criar_tabela(None, esquema("pedidos")).unwrap();

        db.renomear_tabela("pedidos", "pedidos_2025").unwrap();
        assert!(db.existe_tabela(None, "pedidos_2025").unwrap());
        assert!(!db.existe_tabela(None, "pedidos").unwrap());
        assert!(db.abrir_tabela(None, "pedidos_2025").is_ok());

        // O `#` e o separador: recusa, e a origem continua inteira.
        let e = db
            .renomear_tabela("pedidos_2025", "pedidos#2026")
            .unwrap_err();
        assert!(e.to_string().contains("separa a tabela do volume"), "{e}");
        assert!(db.existe_tabela(None, "pedidos_2025").unwrap());
    }

    /// Os arquivos do diretorio cujo nome comeca por `prefixo` -- pelo sistema
    /// de arquivos, e nao pelo catalogo, que e quem esta sendo provado.
    fn arquivos_com(dir: &Path, prefixo: &str) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(prefixo))
            .collect();
        v.sort();
        v
    }

    /// Linhas de uma tabela, na ordem de digitacao, so a coluna `id`.
    fn ids(db: &Database, nome: &str) -> Vec<i64> {
        let mut t = db.abrir_qualificada(nome).unwrap();
        t.varrer()
            .unwrap()
            .into_iter()
            .map(|(_, l)| match l[0] {
                Value::Int(i) => i,
                ref v => panic!("id inesperado {v:?}"),
            })
            .collect()
    }

    /// **Pedido 508: `vendas_2024` nasce, e convive com `vendas` paginada no
    /// mesmo diretorio sem se confundir.**
    ///
    /// E o caso que os quatro motores aceitam e que o separador `_` impedia:
    /// `vendas_2024.reg` se escrevia igual ao volume 2024 de `vendas`. Aqui
    /// `vendas` tem volumes de 4 digitos -- o `vendas#2024` existiria se ela
    /// crescesse ate la -- e a outra tabela chama `vendas_2024`. As duas
    /// aparecem na listagem, cada uma le as proprias linhas, e o
    /// `excluir_tabela("vendas")` leva os arquivos de `vendas` e SO os dela --
    /// que e o estrago que o 368 e o 506 mediram com o separador antigo.
    ///
    /// **Defeito reposto** (`SEPARADOR_DE_VOLUME` de volta a `_` no
    /// `paginacao.rs`): `vendas_2024` e recusada na criacao, ou some da
    /// listagem como volume de `vendas`, e o teste cai.
    #[test]
    fn vendas_2024_nasce_e_convive_com_vendas_paginada() {
        let base = crate::apoio_teste::DirTemp::novo("phx-508-convive");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();
        let paginada = esquema("vendas")
            .com_paginacao(
                phxsql_core::paginacao::Paginacao::nova(2, 999)
                    .unwrap()
                    .com_digitos(4)
                    .unwrap(),
            )
            .unwrap();
        let mut v = db.criar_tabela(None, paginada).unwrap();
        for i in 1..=5i64 {
            v.inserir(&[Value::Int(i), Value::Null]).unwrap();
        }
        drop(v);
        let mut outra = db.criar_tabela(None, esquema("vendas_2024")).unwrap();
        for i in 100..=102i64 {
            outra.inserir(&[Value::Int(i), Value::Null]).unwrap();
        }
        drop(outra);
        // A letra do balde tambem deixou de ser sufixo reservado.
        db.criar_tabela(None, esquema("vendas_A")).unwrap();

        assert!(base.join("loja/vendas#0003.reg").exists());
        assert_eq!(
            db.tabelas(None).unwrap(),
            vec!["vendas", "vendas_2024", "vendas_A"]
        );
        assert_eq!(ids(&db, "vendas"), vec![1, 2, 3, 4, 5]);
        assert_eq!(ids(&db, "vendas_2024"), vec![100, 101, 102]);

        db.excluir_tabela("vendas").unwrap();
        assert!(arquivos_com(db.caminho(), "vendas#").is_empty());
        assert_eq!(db.tabelas(None).unwrap(), vec!["vendas_2024", "vendas_A"]);
        assert_eq!(ids(&db, "vendas_2024"), vec![100, 101, 102]);
    }

    /// **O `#` e recusado nas quatro portas que dao nome a uma tabela**, antes
    /// de qualquer arquivo nascer, e com o motivo na mensagem. E o outro lado
    /// da troca: o separador so e inequivoco porque nome nenhum o contem.
    ///
    /// **Defeito reposto** (tirar a conferencia do `#` do `validar_nome`):
    /// `x#001` nasce -- e o catalogo o lista como o volume 1 de `x` -- e o
    /// `unwrap_err` cai.
    #[test]
    fn o_separador_de_volume_e_recusado_nas_quatro_portas() {
        let base = crate::apoio_teste::DirTemp::novo("phx-508-portas");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();
        let outro = cat.criar_database("arquivo").unwrap();

        let Err(e) = db.criar_tabela(None, esquema("x#001")) else {
            panic!("x#001 nasceu: o catalogo a leria como o volume 1 de x");
        };
        assert!(e.to_string().contains("separa a tabela do volume"), "{e}");
        assert!(arquivos_com(db.caminho(), "x#").is_empty());
        assert!(db.criar_tabela(Some("arq"), esquema("x#A")).is_err());

        db.criar_tabela(None, esquema("x_001")).unwrap();
        assert!(db.existe_tabela(None, "x_001").unwrap());
        assert!(db.duplicar_tabela("x_001", "x#002").is_err());
        assert!(db.copiar_tabela_para("x_001", &outro, "x#003").is_err());
        assert!(db.renomear_tabela("x_001", "x#004").is_err());
        assert!(arquivos_com(db.caminho(), "x#").is_empty());
        assert!(arquivos_com(outro.caminho(), "x#").is_empty());

        // E o nome com `_` passa nas portas que copiam -- a guarda boa demais
        // e o defeito irmao da fraca demais.
        assert!(db.duplicar_tabela("x_001", "x_002").is_ok());
        assert!(db.copiar_tabela_para("x_001", &outro, "x_003").is_ok());
        assert_eq!(db.tabelas(None).unwrap(), vec!["x_001", "x_002"]);
        assert_eq!(outro.tabelas(None).unwrap(), vec!["x_003"]);
    }

    /// **Pedido 507**: o ponto no nome recusa na declaracao, pela mesma
    /// funcao que `abrir_qualificada` usa para separar schema de tabela.
    ///
    /// `a.b` na raiz nascia e aparecia em `todas_as_tabelas`, mas
    /// `abrir_qualificada("a.b")` a lia como schema `a` tabela `b` -- que nao
    /// existe -- e nunca a achava de volta.
    ///
    /// **Defeito reposto** (tirar a conferencia do ponto): `a.b` nasce e o
    /// `unwrap_err` abaixo cai.
    #[test]
    fn criar_recusa_ponto_no_nome_por_colidir_com_o_qualificado() {
        let base = crate::apoio_teste::DirTemp::novo("phx-criar-ponto");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("loja").unwrap();

        let Err(e) = db.criar_tabela(None, esquema("a.b")) else {
            panic!("a.b nasceu: abrir_qualificada nunca a acha de volta");
        };
        assert!(e.to_string().contains("ponto"), "{e}");
        assert!(arquivos_com(db.caminho(), "a.b").is_empty());

        // Nome sem ponto, do mesmo formato, continua nascendo.
        assert!(db.criar_tabela(None, esquema("ab")).is_ok());
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

        // O destino era `pedidos_2026` -- o nome do volume 2026 de uma tabela
        // `pedidos`: a copia nascia e a arvore do destino nao a mostrava. A
        // recusa que o pedido 368 estendeu a esta porta o pegou (B1 do papel
        // C); o nome agora e um que o catalogo le de volta.
        let copiados = origem
            .copiar_tabela_para("pedidos", &destino, "pedidos_arquivados")
            .unwrap();
        assert_eq!(copiados, 5, "os cinco arquivos");
        assert!(destino.existe_tabela(None, "pedidos_arquivados").unwrap());

        let mut c = destino.abrir_qualificada("pedidos_arquivados").unwrap();
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

    /// **Pedido 605, a GARANTIA de dentro**: a tabela criada pela porta que
    /// adia o `fsync` nao abre para OUTRA thread ate o `levar_ao_disco`
    /// terminar -- e abre logo depois. E o `Table::abrir_com` que espera, e
    /// por isso vale para toda porta de abertura (cascata, chave, juncao, SQL),
    /// e nao so para o campo `tabela` que o servidor le. A copia, irma da
    /// criacao, entra na mesma prova.
    #[test]
    fn a_tabela_que_nasce_so_abre_para_outro_depois_do_fsync() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        let base = crate::apoio_teste::DirTemp::novo("phx-nascendo-605");
        let cat = Instancia::nova(&base).unwrap();
        let db = cat.criar_database("b").unwrap();
        db.criar_tabela(None, esquema("origem")).unwrap();
        let abrir_noutra = |nome: &'static str| {
            let voltou = Arc::new(AtomicBool::new(false));
            let v = Arc::clone(&voltou);
            let raiz = base.to_path_buf();
            let h = std::thread::spawn(move || {
                let r = Instancia::nova(&raiz)
                    .unwrap()
                    .abrir_database("b")
                    .unwrap()
                    .abrir_tabela(None, nome)
                    .map(|_| ());
                v.store(true, Ordering::SeqCst);
                r
            });
            (voltou, h)
        };
        let criada = db
            .criar_tabela_adiando_o_fsync(None, esquema("nova"))
            .unwrap()
            .1;
        let copiada = db
            .duplicar_tabela_adiando_o_fsync("origem", "copia")
            .unwrap();
        for (nome, pendente) in [("nova", criada), ("copia", copiada)] {
            let (voltou, h) = abrir_noutra(nome);
            std::thread::sleep(std::time::Duration::from_millis(200));
            assert!(
                !voltou.load(Ordering::SeqCst),
                "outra thread abriu {nome} antes do `fsync` da pasta"
            );
            pendente.levar_ao_disco().unwrap();
            h.join()
                .unwrap()
                .unwrap_or_else(|e| panic!("{nome}: depois de publicar, abrir falhou: {e}"));
        }
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

    /// O filho que o `strace` observa: as duas copias, pelo caminho do
    /// servidor -- copiar com a trava, levar ao disco depois.
    #[test]
    #[ignore = "roda so dentro de as_copias_de_tabela_vao_ao_disco"]
    fn filho_das_copias_de_tabela() {
        let base = PathBuf::from(std::env::var("PHX_586_DIR").unwrap());
        let inst = Instancia::nova(&base).unwrap();
        let a = inst.abrir_database("a").unwrap();
        let b = inst.abrir_database("b").unwrap();
        a.duplicar_tabela_adiando_o_fsync("t", "copia")
            .unwrap()
            .levar_ao_disco()
            .unwrap();
        a.copiar_tabela_para_adiando_o_fsync("t", &b, "novo.t")
            .unwrap()
            .levar_ao_disco()
            .unwrap();
    }

    /// **Pedido 586, contra o sistema operacional: a copia de tabela responde
    /// depois de estar no disco.**
    ///
    /// Pelo `strace -y`: cada arquivo da copia recebe `fsync` no descritor
    /// que o criou, antes do `close` dele; a pasta de destino recebe `fsync`
    /// depois do ultimo arquivo criado nela; e a pasta de schema que o colar
    /// criou tem a entrada dela sincronizada no database (`fsync` de `b`).
    ///
    /// **Nao medido:** a queda em si (pede derrubar a maquina). O que se prova
    /// e o DESCRITOR e a ORDEM das chamadas, que e o que o conserto muda.
    #[test]
    fn as_copias_de_tabela_vao_ao_disco() {
        if std::process::Command::new("strace")
            .arg("-V")
            .output()
            .is_err()
        {
            eprintln!("sem strace nesta maquina: a prova do 586 NAO MEDIDA");
            return;
        }
        let t = crate::apoio_teste::DirTemp::novo("cat-586-strace");
        let base = std::fs::canonicalize(&t.0).unwrap();
        {
            let inst = Instancia::nova(&base).unwrap();
            let a = inst.criar_database("a").unwrap();
            inst.criar_database("b").unwrap();
            let mut tab = a.criar_tabela(None, esquema("t")).unwrap();
            tab.inserir(&[Value::Int(1), Value::Str("um".into())])
                .unwrap();
            tab.sincronizar().unwrap();
        }
        let traco = base.join("traco.txt");
        let saida = std::process::Command::new("strace")
            .args([
                "-f",
                "-y",
                "-e",
                "trace=openat,close,fsync,mkdir,mkdirat",
                "-o",
            ])
            .arg(&traco)
            .arg(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "catalogo::testes_copia_entre_bancos::filho_das_copias_de_tabela",
            ])
            .env("PHX_586_DIR", &base)
            .output()
            .unwrap();
        assert!(
            saida.status.success(),
            "{}",
            String::from_utf8_lossy(&saida.stderr)
        );
        let texto = std::fs::read_to_string(&traco).unwrap();
        let linhas: Vec<&str> = texto.lines().filter(|l| l.contains(" = ")).collect();
        let ok = |l: &str| l.trim_end().ends_with("= 0");
        let txt = |p: &Path| p.display().to_string();
        let fsync_da_pasta = |p: &Path, desde: usize| {
            linhas[desde..]
                .iter()
                .any(|l| l.contains("fsync(") && l.contains(&format!("<{}>)", txt(p))) && ok(l))
        };
        let mut erros = Vec::new();
        for pasta in [base.join("a"), base.join("b/novo")] {
            let prefixo = if pasta.ends_with("novo") {
                "t."
            } else {
                "copia."
            };
            // Os arquivos que a copia CRIOU nesta pasta, com o descritor.
            let criados: Vec<(usize, String, String)> = linhas
                .iter()
                .enumerate()
                .filter_map(|(i, l)| {
                    let caminho = l.split('"').nth(1)?;
                    let p = Path::new(caminho);
                    let nome = p.file_name()?.to_string_lossy().to_string();
                    (l.contains("openat(")
                        && l.contains("O_CREAT")
                        && p.parent().is_some_and(|d| d == pasta)
                        && nome.starts_with(prefixo))
                    .then(|| {
                        // Com `-y` o retorno vem decorado (`= 5</caminho>`):
                        // so os digitos sao o descritor.
                        let fd: String = l
                            .rsplit("= ")
                            .next()?
                            .chars()
                            .take_while(char::is_ascii_digit)
                            .collect();
                        Some((i, caminho.to_string(), fd))
                    })
                    .flatten()
                })
                .collect();
            if criados.is_empty() {
                erros.push(format!("a premissa: nada nasceu em {}", txt(&pasta)));
                continue;
            }
            for (i, caminho, fd) in &criados {
                let fechou = linhas[i + 1..]
                    .iter()
                    .position(|l| l.contains(&format!("close({fd}<")))
                    .map_or(linhas.len(), |j| i + 1 + j);
                let sincronizou = linhas[i + 1..fechou]
                    .iter()
                    .any(|l| l.contains(&format!("fsync({fd}<{caminho}>)")) && ok(l));
                if !sincronizou {
                    erros.push(format!(
                        "{caminho}: sem fsync no descritor {fd} antes do close"
                    ));
                }
            }
            let ultimo = criados.last().map_or(0, |c| c.0);
            if !fsync_da_pasta(&pasta, ultimo + 1) {
                erros.push(format!(
                    "{}: sem fsync da pasta depois do ultimo arquivo da copia",
                    txt(&pasta)
                ));
            }
        }
        // A pasta do schema nasceu no colar: a entrada dela e dado de `b`.
        let nasceu = linhas
            .iter()
            .position(|l| {
                l.contains("mkdir") && l.contains(&format!("\"{}\"", txt(&base.join("b/novo"))))
            })
            .unwrap_or_else(|| panic!("a premissa: o colar criou b/novo:\n{texto}"));
        if !fsync_da_pasta(&base.join("b"), nasceu + 1) {
            erros.push("b: sem fsync do database depois de a pasta do schema nascer".into());
        }
        assert!(erros.is_empty(), "{erros:#?}\n{texto}");
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
        // entao a `Instancia` continua sendo exatamente os campos de dado que
        // ela carrega -- o `PathBuf` e, desde o pedido 564, a politica do
        // diario, e desde o pedido 635 o ponteiro das travas de instancia
        // fixadas. Garantia que custasse memoria seria outra conversa.
        assert_eq!(
            std::mem::size_of::<Instancia>(),
            std::mem::size_of::<(
                PathBuf,
                PoliticaDoDiario,
                Arc<crate::trava_de_instancia::Fixadas>,
            )>(),
            "o marcador da trava passou a ocupar espaco"
        );
    }
}

/// Pedido 589: criar database, schema e tabela responde depois de estar no
/// disco -- provado contra o sistema operacional, pelo `strace -y`.
#[cfg(test)]
mod testes_criar_vai_ao_disco {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::ColumnType;

    fn esquema(nome: &str) -> Schema {
        Schema::new(
            nome,
            vec![Column::new("id", ColumnType::Int8).obrigatoria()],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria()],
        )
        .unwrap()
    }

    /// Um `openat` de um nome que nao existe: a marca que separa, no traco,
    /// o que cada operacao fez. Sem ela o `fsync` do database que a tabela
    /// da raiz paga no fim passaria pelo que o `criar_schema` ficou devendo.
    fn passo(base: &Path, n: u32) {
        let _ = std::fs::File::open(base.join(format!("passo-{n}")));
    }

    /// O filho que o `strace` observa: as quatro criacoes, cada uma separada
    /// da seguinte por um [`passo`].
    #[test]
    #[ignore = "roda so dentro de criar_database_schema_e_tabela_vao_ao_disco"]
    fn filho_das_criacoes() {
        let base = PathBuf::from(std::env::var("PHX_589_DIR").unwrap());
        let inst = Instancia::nova(&base).unwrap();
        passo(&base, 0);
        let d = inst.criar_database("d").unwrap();
        passo(&base, 1);
        d.criar_schema("s").unwrap();
        passo(&base, 2);
        // Schema que ainda nao existe: e o `garantir_schema` do 589.
        let t1 = d.criar_tabela(Some("n"), esquema("t1")).unwrap();
        passo(&base, 3);
        let t2 = d.criar_tabela(None, esquema("t2")).unwrap();
        passo(&base, 4);
        drop((t1, t2));
    }

    /// **Pedido 589, contra o sistema operacional.**
    ///
    /// Em cada operacao, antes da seguinte comecar: todo arquivo que ela
    /// criou recebe `fsync` (o `.pag` fica de fora -- e derivado, e nasce por
    /// `rename` justamente para dispensa-lo); a pasta onde eles nasceram
    /// recebe `fsync` depois do ultimo; e a pasta que nasceu tem a entrada
    /// dela sincronizada na mae -- a base para o database, o database para o
    /// schema.
    ///
    /// **Nao medido:** a queda em si (pede derrubar a maquina). O que se prova
    /// e o DESCRITOR e a ORDEM das chamadas, que e o que o conserto muda.
    #[test]
    fn criar_database_schema_e_tabela_vao_ao_disco() {
        if std::process::Command::new("strace")
            .arg("-V")
            .output()
            .is_err()
        {
            eprintln!("sem strace nesta maquina: a prova do 589 NAO MEDIDA");
            return;
        }
        let t = crate::apoio_teste::DirTemp::novo("cat-589-strace");
        let base = std::fs::canonicalize(&t.0).unwrap();
        let traco = t.0.join("traco.txt");
        let saida = std::process::Command::new("strace")
            .args([
                "-f",
                "-y",
                "-e",
                "trace=openat,close,fsync,mkdir,mkdirat",
                "-o",
            ])
            .arg(&traco)
            .arg(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "catalogo::testes_criar_vai_ao_disco::filho_das_criacoes",
            ])
            .env("PHX_589_DIR", &base)
            .output()
            .unwrap();
        assert!(
            saida.status.success(),
            "{}",
            String::from_utf8_lossy(&saida.stderr)
        );
        let texto = std::fs::read_to_string(&traco).unwrap();
        let linhas: Vec<&str> = texto.lines().filter(|l| l.contains(" = ")).collect();
        let txt = |p: &Path| p.display().to_string();
        let marca = |n: u32| {
            let alvo = format!("\"{}\"", txt(&base.join(format!("passo-{n}"))));
            linhas
                .iter()
                .position(|l| l.contains("openat(") && l.contains(&alvo))
                .unwrap_or_else(|| panic!("a premissa: o passo {n} no traco:\n{texto}"))
        };
        let ok = |l: &str| l.trim_end().ends_with("= 0");
        let fsync_de = |p: &Path, janela: &[&str]| {
            janela
                .iter()
                .any(|l| l.contains("fsync(") && l.contains(&format!("<{}>)", txt(p))) && ok(l))
        };
        let mut erros = Vec::new();
        // (passo, pasta onde nascem arquivos e o prefixo deles, pastas que nascem)
        type Caso<'a> = (u32, Option<(PathBuf, &'a str)>, Vec<PathBuf>);
        let d = base.join("d");
        let casos: [Caso; 4] = [
            (1, Some((d.clone(), "_database.json")), vec![d.clone()]),
            (2, None, vec![d.join("s")]),
            (3, Some((d.join("n"), "t1.")), vec![d.join("n")]),
            (4, Some((d.clone(), "t2.")), vec![]),
        ];
        for (n, arquivos, pastas_novas) in casos {
            let janela = &linhas[marca(n - 1) + 1..marca(n)];
            if let Some((pasta, prefixo)) = arquivos {
                let criados: Vec<(usize, String)> = janela
                    .iter()
                    .enumerate()
                    .filter_map(|(i, l)| {
                        let caminho = l.split('"').nth(1)?;
                        let p = Path::new(caminho);
                        let nome = p.file_name()?.to_string_lossy().to_string();
                        (l.contains("openat(")
                            && l.contains("O_CREAT")
                            && p.parent().is_some_and(|x| x == pasta)
                            && nome.starts_with(prefixo)
                            && !nome.contains(".pag"))
                        .then(|| (i, caminho.to_string()))
                    })
                    .collect();
                if criados.is_empty() {
                    erros.push(format!(
                        "passo {n}: a premissa: nada nasceu em {}",
                        txt(&pasta)
                    ));
                    continue;
                }
                for (i, caminho) in &criados {
                    if !fsync_de(Path::new(caminho), &janela[i + 1..]) {
                        erros.push(format!("passo {n}: {caminho} sem fsync antes da resposta"));
                    }
                }
                let ultimo = criados.last().map_or(0, |c| c.0);
                if !fsync_de(&pasta, &janela[ultimo + 1..]) {
                    erros.push(format!(
                        "passo {n}: {} sem fsync depois do ultimo arquivo criado nela",
                        txt(&pasta)
                    ));
                }
            }
            for nova in pastas_novas {
                let alvo = format!("\"{}\"", txt(&nova));
                let Some(nasceu) = janela
                    .iter()
                    .position(|l| l.contains("mkdir") && l.contains(&alvo) && ok(l))
                else {
                    erros.push(format!("passo {n}: a premissa: {} nao nasceu", txt(&nova)));
                    continue;
                };
                let mae = nova.parent().unwrap();
                if !fsync_de(mae, &janela[nasceu + 1..]) {
                    erros.push(format!(
                        "passo {n}: {} nasceu e {} ficou sem fsync",
                        txt(&nova),
                        txt(mae)
                    ));
                }
            }
        }
        assert!(erros.is_empty(), "{erros:#?}\n{texto}");
    }
}

#[cfg(test)]
mod testes_excluir_vai_ao_disco {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::{ColumnType, DadoPessoal};
    use phxsql_core::value::Value;

    fn esquema(nome: &str) -> Schema {
        Schema::new(
            nome,
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("email", ColumnType::Str(40)).com_dado_pessoal(DadoPessoal::Pessoal),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
                .unico()
                .primaria()],
        )
        .unwrap()
    }

    /// A marca que separa, no traco, o que cada operacao fez: um `openat` de
    /// um nome que nao existe (a mesma do 589).
    fn passo(base: &Path, n: u32) {
        let _ = std::fs::File::open(base.join(format!("passo-{n}")));
    }

    /// O filho que o `strace` observa: excluir a tabela, esvaziar a lixeira e
    /// expurgar a trilha, cada um entre dois [`passo`]s.
    #[test]
    #[ignore = "roda so dentro de excluir_esvaziar_e_expurgar_vao_ao_disco"]
    fn filho_das_exclusoes() {
        let base = PathBuf::from(std::env::var("PHX_591_DIR").unwrap());
        let inst = Instancia::nova(&base).unwrap();
        let d = inst.criar_database("d").unwrap();
        drop(d.criar_tabela(None, esquema("sai")).unwrap());
        let mut t = d.criar_tabela(None, esquema("fica")).unwrap();
        t.definir_usuario(7);
        // Tres volumes fechados da trilha, e duas linhas na lixeira. Laco com
        // teto fixo: 3 x 3 alteracoes.
        t.inserir(&[Value::Int(1), Value::Str("a@x.com".into())])
            .unwrap();
        for v in 0..3u32 {
            for i in 0..3u32 {
                let email = format!("a{v}{i}@x.com");
                t.atualizar(1, &[Value::Int(1), Value::Str(email)]).unwrap();
            }
            assert!(t.fechar_volume_da_trilha(true, 0).unwrap().is_some());
        }
        t.inserir(&[Value::Int(2), Value::Str("b@x.com".into())])
            .unwrap();
        t.inserir(&[Value::Int(3), Value::Str("c@x.com".into())])
            .unwrap();
        t.excluir_de_vez(2, "a").unwrap();
        t.excluir_de_vez(3, "b").unwrap();
        t.sincronizar().unwrap();
        passo(&base, 0);
        d.excluir_tabela("sai").unwrap();
        passo(&base, 1);
        assert_eq!(t.esvaziar_lixeira("limpeza").unwrap(), 2);
        passo(&base, 2);
        let limite = phxsql_core::datahora::ms_de_instante_iso("2099-01-01").unwrap();
        t.expurgar_trilha(limite, "prazo vencido").unwrap();
        passo(&base, 3);
    }

    /// **Pedido 591, contra o sistema operacional.**
    ///
    /// Em cada operacao, antes da seguinte comecar: os nomes que ela apagou
    /// (`unlink` bem-sucedido) e a pasta deles recebe `fsync` DEPOIS do
    /// ultimo -- sem ele a entrada apagada volta numa queda, e a tabela
    /// «excluida», o `.trash` «esvaziado» ou o volume «expurgado» voltam com
    /// ela.
    ///
    /// **Nao medido:** a queda em si (pede derrubar a maquina). O que se prova
    /// e o DESCRITOR e a ORDEM das chamadas, que e o que o conserto muda.
    #[test]
    fn excluir_esvaziar_e_expurgar_vao_ao_disco() {
        if std::process::Command::new("strace")
            .arg("-V")
            .output()
            .is_err()
        {
            eprintln!("sem strace nesta maquina: a prova do 591 NAO MEDIDA");
            return;
        }
        let t = crate::apoio_teste::DirTemp::novo("cat-591-strace");
        let base = std::fs::canonicalize(&t.0).unwrap();
        let traco = t.0.join("traco.txt");
        let saida = std::process::Command::new("strace")
            .args([
                "-f",
                "-y",
                "-e",
                "trace=openat,close,fsync,unlink,unlinkat",
                "-o",
            ])
            .arg(&traco)
            .arg(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "catalogo::testes_excluir_vai_ao_disco::filho_das_exclusoes",
            ])
            .env("PHX_591_DIR", &base)
            .output()
            .unwrap();
        assert!(
            saida.status.success(),
            "{}",
            String::from_utf8_lossy(&saida.stderr)
        );
        let texto = std::fs::read_to_string(&traco).unwrap();
        let linhas: Vec<&str> = texto.lines().filter(|l| l.contains(" = ")).collect();
        let txt = |p: &Path| p.display().to_string();
        let marca = |n: u32| {
            let alvo = format!("\"{}\"", txt(&base.join(format!("passo-{n}"))));
            linhas
                .iter()
                .position(|l| l.contains("openat(") && l.contains(&alvo))
                .unwrap_or_else(|| panic!("a premissa: o passo {n} no traco:\n{texto}"))
        };
        let ok = |l: &str| l.trim_end().ends_with("= 0");
        let pasta = base.join("d");
        let fsync_da_pasta = format!("<{}>)", txt(&pasta));
        let mut erros = Vec::new();
        // (passo, prefixo dos nomes que saem)
        for (n, prefixo) in [(1, "sai."), (2, "fica"), (3, "fica")] {
            let janela = &linhas[marca(n - 1) + 1..marca(n)];
            let apagados: Vec<usize> = janela
                .iter()
                .enumerate()
                .filter(|(_, l)| {
                    l.contains("unlink")
                        && ok(l)
                        && l.split('"').nth(1).is_some_and(|c| {
                            let p = Path::new(c);
                            p.parent().is_some_and(|x| x == pasta)
                                && p.file_name()
                                    .is_some_and(|f| f.to_string_lossy().starts_with(prefixo))
                        })
                })
                .map(|(i, _)| i)
                .collect();
            let Some(ultimo) = apagados.last() else {
                erros.push(format!(
                    "passo {n}: a premissa: nada saiu de {}",
                    txt(&pasta)
                ));
                continue;
            };
            let sincronizou = janela[ultimo + 1..]
                .iter()
                .any(|l| l.contains("fsync(") && l.contains(&fsync_da_pasta) && ok(l));
            if !sincronizou {
                erros.push(format!(
                    "passo {n}: {} nomes sairam de {} e a pasta ficou sem fsync",
                    apagados.len(),
                    txt(&pasta)
                ));
            }
        }
        assert!(erros.is_empty(), "{erros:#?}\n{texto}");
    }

    /// O filho do erro no meio: a tabela `sai` com um DIRETORIO no lugar do
    /// `.bkp` -- o `remove_file` dele falha (EISDIR) depois de o `.reg` e o
    /// `.ndx` ja terem saido.
    #[test]
    #[ignore = "roda so dentro de o_erro_no_meio_leva_ao_disco_o_que_saiu"]
    fn filho_do_erro_no_meio() {
        let base = PathBuf::from(std::env::var("PHX_595_DIR").unwrap());
        let inst = Instancia::nova(&base).unwrap();
        let d = inst.criar_database("d").unwrap();
        drop(d.criar_tabela(None, esquema("sai")).unwrap());
        let tranca = base.join("d").join("sai.bkp");
        std::fs::create_dir(&tranca).unwrap();
        std::fs::write(tranca.join("dentro"), b"x").unwrap();
        passo(&base, 0);
        assert!(
            d.excluir_tabela("sai").is_err(),
            "a premissa: o erro no meio"
        );
        passo(&base, 1);
    }

    /// **Pedido 595, o menor: o erro no meio da exclusao.** Os nomes que ja
    /// sairam antes do erro devem o `fsync` da pasta tanto quanto no caminho
    /// feliz -- com o `?` que havia, eles ficavam sem, e a tabela voltava
    /// pela metade numa queda.
    #[test]
    fn o_erro_no_meio_leva_ao_disco_o_que_saiu() {
        if std::process::Command::new("strace")
            .arg("-V")
            .output()
            .is_err()
        {
            eprintln!("sem strace nesta maquina: a prova do 595 NAO MEDIDA");
            return;
        }
        let t = crate::apoio_teste::DirTemp::novo("cat-595-strace");
        let base = std::fs::canonicalize(&t.0).unwrap();
        let traco = t.0.join("traco.txt");
        let saida = std::process::Command::new("strace")
            .args(["-f", "-y", "-e", "trace=openat,fsync,unlink,unlinkat", "-o"])
            .arg(&traco)
            .arg(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "catalogo::testes_excluir_vai_ao_disco::filho_do_erro_no_meio",
            ])
            .env("PHX_595_DIR", &base)
            .output()
            .unwrap();
        assert!(
            saida.status.success(),
            "{}",
            String::from_utf8_lossy(&saida.stderr)
        );
        let texto = std::fs::read_to_string(&traco).unwrap();
        let linhas: Vec<&str> = texto.lines().filter(|l| l.contains(" = ")).collect();
        let marca = |n: u32| {
            let alvo = format!("\"{}\"", base.join(format!("passo-{n}")).display());
            linhas
                .iter()
                .position(|l| l.contains("openat(") && l.contains(&alvo))
                .unwrap_or_else(|| panic!("a premissa: o passo {n} no traco:\n{texto}"))
        };
        let janela = &linhas[marca(0) + 1..marca(1)];
        let pasta = base.join("d");
        let reg = format!("\"{}\"", pasta.join("sai.reg").display());
        let saiu = janela
            .iter()
            .position(|l| l.contains("unlink") && l.contains(&reg) && l.trim_end().ends_with("= 0"))
            .unwrap_or_else(|| panic!("a premissa: o .reg saiu antes do erro\n{texto}"));
        let fsync_da_pasta = format!("<{}>)", pasta.display());
        assert!(
            janela[saiu + 1..].iter().any(|l| l.contains("fsync(")
                && l.contains(&fsync_da_pasta)
                && l.trim_end().ends_with("= 0")),
            "o erro no meio deixou os nomes que sairam sem fsync da pasta\n{texto}"
        );
    }

    /// Os volumes de `nome` com a extensao `ext` na pasta, em ordem de nome.
    fn volumes_de(pasta: &Path, nome: &str, ext: &str) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = std::fs::read_dir(pasta)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name().is_some_and(|f| {
                    let f = f.to_string_lossy();
                    f.starts_with(nome) && f.ends_with(ext)
                })
            })
            .collect();
        v.sort();
        v
    }

    /// O filho do 598: o esvaziar da lixeira e o expurgo da trilha, cada um
    /// com o `unlink` do SEGUNDO volume recusado (a arma
    /// `Onde::RemocaoDeVolume`) depois de o primeiro ja ter saido.
    #[test]
    #[ignore = "roda so dentro de o_erro_no_meio_do_esvaziar_e_do_expurgo_leva_ao_disco"]
    fn filho_do_erro_no_meio_598() {
        use crate::sincronia::falha_de_teste::{armar, desarmar, Onde};
        let base = PathBuf::from(std::env::var("PHX_598_DIR").unwrap());
        let pasta = base.join("d");
        let inst = Instancia::nova(&base).unwrap();
        let d = inst.criar_database("d").unwrap();
        // Um byte por volume: cada linha descartada abre um volume do
        // `.trash`, e duas bastam para haver um «segundo».
        let pag = phxsql_core::paginacao::Paginacao::nova(1_000, 9)
            .unwrap()
            .com_bytes_por_arquivo(1)
            .unwrap();
        let mut lixo = d
            .criar_tabela(None, esquema("lixo").com_paginacao(pag).unwrap())
            .unwrap();
        for i in 1..=2i64 {
            let r = lixo
                .inserir(&[Value::Int(i), Value::Str(format!("l{i}@x.com"))])
                .unwrap();
            lixo.excluir_de_vez(r, "m").unwrap();
        }
        lixo.sincronizar().unwrap();
        let trash = volumes_de(&pasta, "lixo", ".trash");
        assert!(
            trash.len() >= 2,
            "a premissa: dois volumes do .trash: {trash:?}"
        );

        // A trilha de `fica`: tres volumes fechados, como no filho do 591.
        let mut t = d.criar_tabela(None, esquema("fica")).unwrap();
        t.definir_usuario(7);
        t.inserir(&[Value::Int(1), Value::Str("a@x.com".into())])
            .unwrap();
        for v in 0..3u32 {
            for i in 0..3u32 {
                let email = format!("a{v}{i}@x.com");
                t.atualizar(1, &[Value::Int(1), Value::Str(email)]).unwrap();
            }
            assert!(t.fechar_volume_da_trilha(true, 0).unwrap().is_some());
        }
        t.sincronizar().unwrap();
        let ativo = pasta.join("fica.lgpd");
        let fechados: Vec<PathBuf> = volumes_de(&pasta, "fica", ".lgpd")
            .into_iter()
            .filter(|p| *p != ativo)
            .collect();
        assert!(
            fechados.len() >= 2,
            "a premissa: volumes fechados: {fechados:?}"
        );

        passo(&base, 0);
        armar(&trash[1], Onde::RemocaoDeVolume, 1);
        let r = lixo.esvaziar_lixeira("limpeza");
        desarmar(&trash[1]);
        assert!(r.is_err(), "a premissa: o erro no meio do esvaziar");
        passo(&base, 1);
        let limite = phxsql_core::datahora::ms_de_instante_iso("2099-01-01").unwrap();
        armar(&fechados[1], Onde::RemocaoDeVolume, 1);
        let r = t.expurgar_trilha(limite, "prazo vencido");
        desarmar(&fechados[1]);
        assert!(r.is_err(), "a premissa: o erro no meio do expurgo");
        passo(&base, 2);
    }

    /// **Pedido 598, contra o sistema operacional: o erro no meio do esvaziar
    /// e do expurgo.** O mesmo defeito que o 595 fechou no `excluir_tabela`:
    /// o `?` das duas `_adiando_o_fsync` descartava o pendente, e o volume que
    /// ja tinha saido ficava so no cache do nucleo -- a lixeira «esvaziada» ou
    /// o volume da trilha «expurgado» voltavam numa queda.
    #[test]
    fn o_erro_no_meio_do_esvaziar_e_do_expurgo_leva_ao_disco() {
        if std::process::Command::new("strace")
            .arg("-V")
            .output()
            .is_err()
        {
            eprintln!("sem strace nesta maquina: a prova do 598 NAO MEDIDA");
            return;
        }
        let t = crate::apoio_teste::DirTemp::novo("cat-598-strace");
        let base = std::fs::canonicalize(&t.0).unwrap();
        let traco = t.0.join("traco.txt");
        let saida = std::process::Command::new("strace")
            .args(["-f", "-y", "-e", "trace=openat,fsync,unlink,unlinkat", "-o"])
            .arg(&traco)
            .arg(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "catalogo::testes_excluir_vai_ao_disco::filho_do_erro_no_meio_598",
            ])
            .env("PHX_598_DIR", &base)
            .output()
            .unwrap();
        assert!(
            saida.status.success(),
            "{}",
            String::from_utf8_lossy(&saida.stderr)
        );
        let texto = std::fs::read_to_string(&traco).unwrap();
        let linhas: Vec<&str> = texto.lines().filter(|l| l.contains(" = ")).collect();
        let marca = |n: u32| {
            let alvo = format!("\"{}\"", base.join(format!("passo-{n}")).display());
            linhas
                .iter()
                .position(|l| l.contains("openat(") && l.contains(&alvo))
                .unwrap_or_else(|| panic!("a premissa: o passo {n} no traco:\n{texto}"))
        };
        let pasta = base.join("d");
        let fsync_da_pasta = format!("<{}>)", pasta.display());
        let mut erros = Vec::new();
        for (n, nome, ext) in [(1, "lixo", ".trash"), (2, "fica", ".lgpd")] {
            let janela = &linhas[marca(n - 1) + 1..marca(n)];
            let Some(saiu) = janela.iter().rposition(|l| {
                l.contains("unlink")
                    && l.trim_end().ends_with("= 0")
                    && l.split('"').nth(1).is_some_and(|c| {
                        let p = Path::new(c);
                        p.parent().is_some_and(|x| x == pasta)
                            && p.file_name().is_some_and(|f| {
                                let f = f.to_string_lossy();
                                f.starts_with(nome) && f.ends_with(ext)
                            })
                    })
            }) else {
                erros.push(format!("passo {n}: a premissa: nenhum {nome}*{ext} saiu"));
                continue;
            };
            let sincronizou = janela[saiu + 1..].iter().any(|l| {
                l.contains("fsync(") && l.contains(&fsync_da_pasta) && l.trim_end().ends_with("= 0")
            });
            if !sincronizou {
                erros.push(format!(
                    "passo {n}: o erro no meio deixou {nome}*{ext} que saiu sem fsync da pasta"
                ));
            }
        }
        assert!(erros.is_empty(), "{erros:#?}\n{texto}");
    }
}
