//! Conjunto de volumes de um arquivo paginado.
//!
//! Uma tabela grande se parte em `Tabela_001.reg`, `Tabela_002.reg`, ... Este
//! modulo esconde essa divisao: quem chama pede "leia tantos bytes no offset X
//! do volume N" e nao precisa saber quantos arquivos existem nem quais estao
//! abertos.
//!
//! # Abertura preguicosa
//!
//! Uma tabela de 999 volumes nao pode manter 999 descritores de arquivo
//! abertos. Os volumes sao abertos sob demanda e mantidos num cache LRU; o
//! menos usado e fechado quando o teto e atingido. E o mesmo *lazy open* que o
//! `FileManager` do Clarion(R) faz.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::paginacao::Paginacao;

/// Quantos volumes ficam abertos ao mesmo tempo.
pub const LIMITE_ABERTOS_PADRAO: usize = 64;

pub struct Volumes {
    diretorio: PathBuf,
    nome: String,
    ext: &'static str,
    paginacao: Paginacao,
    abertos: HashMap<u32, File>,
    ordem: VecDeque<u32>,
    limite: usize,
    /// Espelho do conjunto, com outra extensao. Toda escrita vai para os dois.
    ///
    /// A leitura NAO consulta o espelho: quem le vai ao arquivo principal, e
    /// so quem descobre corrupcao pede a segunda chance de proposito. Ler
    /// sempre dos dois dobraria o custo de toda leitura para pagar por um caso
    /// que quase nunca acontece.
    espelho: Option<Box<Volumes>>,
    /// Quantas vezes `sincronizar` chamou o disco neste conjunto.
    ///
    /// # Por que ele existe, e por que e um `u64` comum
    ///
    /// Porque «este caminho espera o disco» e uma AFIRMACAO, e afirmacao sem
    /// medida e a mesma familia do numero digitado a mao. E a unica forma de
    /// um teste provar que a exclusao continua sincronizando por operacao
    /// quando ninguem pediu o contrario -- o efeito do `fsync` so aparece numa
    /// queda de energia, que teste nenhum provoca.
    ///
    /// Nao e atomico e nao e global de proposito: e por conjunto, entao nao ha
    /// disputa, nao ha contador do processo para dois testes paralelos
    /// atrapalharem, e o custo e um `+= 1` ao lado de uma chamada de sistema
    /// que custa quatro ordens de grandeza mais.
    sincronizacoes: u64,
    /// A senha da ULTIMA sincronizacao deste conjunto, tirada de [`SENHA`].
    ///
    /// Contar nao diz ordem, e a ordem e o que `Table::sincronizar` decide:
    /// o `.trash` fecha antes do `.reg`, porque a copia de recuperacao tem de
    /// estar no disco antes da liberacao contra a qual ela protege. Sem esta
    /// senha a ordem estaria escrita no comentario e em lugar nenhum mais --
    /// e comentario nao reprova ninguem.
    selo: u64,
    /// Os volumes desta FAMILIA de arquivos escritos e ainda nao levados ao
    /// disco, compartilhados por todas as instancias do processo. Ver
    /// [`ESCRITAS_PENDENTES`].
    pendentes: Pendentes,
    /// Quantos `fsync` de verdade este conjunto ja mandou.
    ///
    /// # Por que ele nao e' o `sincronizacoes`
    ///
    /// Porque o outro conta a CHAMADA e este conta o ARQUIVO, e a diferenca
    /// entre os dois e' exatamente o defeito que esta rodada consertou:
    /// `sincronizar()` incrementava `sincronizacoes`, devolvia `Ok(())` e nao
    /// tocava disco nenhum quando `abertos` estava vazio. Um teste que
    /// conferisse o contador antigo -- ou o selo -- passaria com o defeito de
    /// pe, porque os dois medem a INTENCAO. Este mede o fato, e e' o que
    /// permite provar a durabilidade sem `strace`.
    sincronizados: u64,
}

/// A senha que ordena as sincronizacoes do processo inteiro.
///
/// Um `fetch_add` sem disputa custa 13,2 ns, medidos nesta casa, ao lado de um
/// `fsync` que custa dezenas de microssegundos: quatro ordens de grandeza.
static SENHA: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Os volumes escritos e ainda nao sincronizados, por familia de arquivos.
type Pendentes = Arc<Mutex<BTreeSet<u32>>>;

/// O registro de escritas pendentes DO PROCESSO, e nao de uma instancia.
///
/// # Por que ele existe: quem escreve e quem sincroniza sao objetos diferentes
///
/// `sincronizar` percorria `self.abertos`, e isso e' uma promessa de
/// durabilidade medida pelo CACHE de descritores desta instancia. As duas
/// coisas nao sao a mesma, e a diferenca custava dado:
///
/// - o fecho da janela de durabilidade (`descarregar_sujas_com`, no servidor)
///   REABRE a tabela so para sincronizar. Quem escreveu foi outra `Table`, ja
///   morta; a instancia que sincroniza tem `abertos` vazio para o `.reg`,
///   porque `RegFile::abrir` le o cabecalho com um `std::fs::File` direto --
///   fora deste cache. O laco rodava zero vezes e `sincronizar()` devolvia
///   `Ok(())` tendo sincronizado NADA;
/// - o mesmo vale para o `.reg` de um volume no MEIO de uma tabela paginada,
///   que um `atualizar` suja e que a reabertura nunca abre;
/// - e vale ate' sem reabertura nenhuma: `abertos` e' um LRU de
///   [`LIMITE_ABERTOS_PADRAO`] entradas, entao quem escreve no volume 1 e
///   depois em outros 64 perde o descritor do primeiro por despejo, e com ele
///   perderia o `fsync`.
///
/// Nada disso aparece numa queda de PROCESSO -- pagina suja no cache do nucleo
/// sobrevive a `SIGKILL` --, so numa queda de ENERGIA. Por isso o defeito
/// atravessou a bateria inteira sem uma falha.
///
/// # A regra que decide a forma: a marca SOMA fsync, nunca subtrai
///
/// Este registro e' consultado para acrescentar `fsync`, jamais para pular
/// um. A assimetria e' a coisa mais importante deste modulo: uma marca
/// esquecida no caminho de escrita faz `sincronizar` cair no comportamento
/// ANTIGO (sincroniza o que esta aberto) -- custa velocidade. Uma marca
/// esquecida num registro usado para PULAR `fsync` custaria o dado, calada, e
/// so numa queda de energia. Mesmo registro, lido nos dois sentidos, com
/// modos de falha opostos. Ver `docs/FORMATO.md`.
///
/// # A chave, e a grafia que dividia a familia
///
/// A chave e' a familia (`diretorio/nome.ext`), e nao o caminho de cada
/// volume: e' o que permite a instancia B achar o que a instancia A escreveu.
///
/// Este comentario dizia, ate' 05/09/2026, que «duas grafias diferentes do
/// mesmo diretorio dariam duas familias, e ai a degradacao e a mesma de acima,
/// para o comportamento antigo, nunca para menos que ele». **As duas metades
/// estavam erradas, e a sonda `--example sonda-do-volume-do-meio` mediu as
/// duas.**
///
/// 1. **Nem toda grafia divide.** `PathBuf` compara por `components()`, e ali
///    o `CurDir` e a barra final somem: `/tmp/x`, `/tmp/./x` e `/tmp/x/` sao
///    a MESMA chave, medido. A grafia que divide de verdade e' a **relativa
///    contra a absoluta** -- `dados/loja` e `/srv/dados/loja`.
/// 2. **A degradacao nao e' benigna: ela perde dado.** O comportamento antigo
///    e' `abrir_para_sincronizar(1)` mais a fronteira de escrita, e a fase 3
///    da sonda mediu que ele NAO alcanca o volume do meio. Com a familia
///    partida, a fase 5 mediu o fecho levando ao disco `001` e `005` e
///    deixando para tras o volume 2, que era o sujo.
///
/// Por isso a chave sai de [`familia`], que absolutiza o caminho por via
/// **lexica** antes de montar o nome. O que ele custa esta medido em
/// `docs/DESEMPENHO.md`; o que ele NAO resolve esta escrito na propria funcao.
static ESCRITAS_PENDENTES: Mutex<BTreeMap<PathBuf, Pendentes>> = Mutex::new(BTreeMap::new());

/// A chave da familia: `diretorio/nome.ext`, com o diretorio em caminho
/// ABSOLUTO lexico.
///
/// # Por que aqui, e nao em quem chama
///
/// Este e' o UNICO lugar do motor onde a chave da familia nasce. Espalhar a
/// resolucao pelos chamadores e' a receita da porta dos fundos: o que alguem
/// esquecer volta a dividir a familia, e nenhum teste acusa -- o defeito so'
/// aparece numa queda de energia. Quem chama pode resolver antes, e
/// `Table::abrir` resolve, mas por VELOCIDADE e nao por correcao.
///
/// # Por que lexico, e nao `canonicalize`
///
/// `canonicalize` toca o disco, resolve *symlink* e **falha quando o caminho
/// ainda nao existe** -- o que quebraria `Table::criar`, que monta o conjunto
/// antes de o primeiro arquivo nascer. Aqui e' so' conta de componentes.
///
/// # O que ele NAO resolve, e e' decisao
///
/// * `..` no meio: nao se remove (medido -- `/tmp/y/../x` continua diferente
///   de `/tmp/x`). Nao alcanca o servidor, porque `validar_nome` recusa `..`
///   em database, schema e tabela; alcanca quem passa a RAIZ, e a raiz vem do
///   `config.json` ou do chamador C da FFI;
/// * dois *symlinks* para o mesmo diretorio: so' `canonicalize` os junta, e o
///   preco dela esta acima.
///
/// Nos dois casos que sobram a degradacao e a de sempre -- volta ao
/// comportamento antigo --, e ela **nao e' benigna**: ver o registro acima.
fn familia(diretorio: &Path, nome: &str, ext: &str) -> PathBuf {
    let arquivo = format!("{nome}.{ext}");
    match absoluto_lexico(diretorio) {
        Some(a) => a.join(arquivo),
        None => diretorio.join(arquivo),
    }
}

/// O caminho em forma ABSOLUTA lexica, ou `None` quando ele ja serve.
///
/// # Por que escrita a mao, e nao `std::path::absolute`
///
/// Porque ela e' estavel a partir do Rust **1.79** e este projeto declara
/// **1.75** no `Cargo.toml` -- e MSRV e' promessa de compatibilidade, nao
/// detalhe de compilacao. Subi-la para poupar cinco linhas trocaria uma
/// promessa por conveniencia, e trocaria calada.
///
/// A divergencia contra a receita da `std`, e a restricao que a causou: a
/// `absolute` tambem tira os componentes `.` e as barras repetidas, e aqui
/// isso **nao faz falta** -- `PathBuf` compara por `components()`, onde o
/// `CurDir` e a barra final ja somem (medido: `/tmp/./x`, `/tmp/x/` e
/// `/tmp/x` sao a mesma chave). O que sobra e' o que importa: a relativa
/// vira absoluta.
///
/// O caminho JA absoluto devolve `None` e nao paga nada -- so' o relativo
/// paga o `getcwd`, 395 ns medidos.
pub(crate) fn absoluto_lexico(diretorio: &Path) -> Option<PathBuf> {
    if diretorio.is_absolute() {
        return None;
    }
    // No Windows ha duas formas que nao sao absolutas e tambem NAO se
    // resolvem juntando o diretorio de trabalho: a que traz prefixo de disco
    // sem raiz (`C:pasta`, relativa ao diretorio corrente DAQUELE disco) e a
    // que traz raiz sem prefixo (`\pasta`). Juntar o `getcwd` nelas montaria
    // um caminho que nao existe. No Unix nenhuma das duas ocorre -- `has_root`
    // ali e' o proprio `is_absolute` --, entao este ramo nao custa nada.
    if diretorio.has_root()
        || matches!(
            diretorio.components().next(),
            Some(std::path::Component::Prefix(_))
        )
    {
        return None;
    }
    // Caminho vazio e `getcwd` que falha caem no mesmo lugar: o cru, que e' o
    // comportamento antigo -- e' o que a assimetria desta marca tolera.
    std::env::current_dir().ok().map(|c| c.join(diretorio))
}

/// O conjunto pendente desta familia, criado na primeira vez que alguem pede.
///
/// Resolvido UMA vez, na construcao do `Volumes`, e guardado num `Arc`: o
/// caminho de escrita nao pode pagar uma busca por `PathBuf` a cada linha
/// gravada. O que sobra no laco quente e' um `lock` sem disputa (13,2 ns
/// medidos nesta casa) e a insercao de um `u32`.
fn pendentes_da_familia(familia: PathBuf) -> Pendentes {
    let mut reg = trava(&ESCRITAS_PENDENTES);
    reg.entry(familia).or_default().clone()
}

/// Toma a trava ignorando envenenamento.
///
/// Envenenar acontece quando uma thread entra em panico segurando a trava, e o
/// que ela protege aqui e' um conjunto de `u32`: nao ha estado meio-escrito que
/// possa enganar ninguem. Propagar o envenenamento tiraria do ar o registro que
/// decide o que vai ao disco -- perder durabilidade por causa do panico de
/// outra thread seria trocar um defeito por um pior.
fn trava<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match m.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    }
}

impl Volumes {
    pub fn novo(
        diretorio: impl AsRef<Path>,
        nome: impl Into<String>,
        ext: &'static str,
        paginacao: Paginacao,
    ) -> Volumes {
        let diretorio = diretorio.as_ref().to_path_buf();
        let nome = nome.into();
        let pendentes = pendentes_da_familia(familia(&diretorio, &nome, ext));
        Volumes {
            diretorio,
            nome,
            ext,
            paginacao,
            abertos: HashMap::new(),
            ordem: VecDeque::new(),
            limite: LIMITE_ABERTOS_PADRAO,
            espelho: None,
            sincronizacoes: 0,
            selo: 0,
            pendentes,
            sincronizados: 0,
        }
    }

    /// Liga o espelho: um conjunto irmao com outra extensao.
    ///
    /// # O que o espelho protege, e o que nao protege
    ///
    /// Protege contra o dado ficar RUIM: escrita pela metade, bit virado,
    /// defeito nosso que estraga um slot. Nesses casos o outro lado ainda tem
    /// a versao boa, e o `reparar` a devolve.
    ///
    /// NAO protege contra o disco morrer. Os dois arquivos estao no mesmo
    /// disco, no mesmo diretorio. Quem precisa sobreviver a perda do disco
    /// precisa de backup em outro lugar -- e por isso o backup existe
    /// separado, e nao e a mesma coisa.
    pub fn com_espelho(mut self, ext: &'static str) -> Volumes {
        let mut e = Volumes::novo(&self.diretorio, self.nome.clone(), ext, self.paginacao);
        e.limite = self.limite;
        self.espelho = Some(Box::new(e));
        self
    }

    pub fn tem_espelho(&self) -> bool {
        self.espelho.is_some()
    }

    /// Le do ESPELHO, quando o principal falhou. A segunda chance.
    pub fn ler_do_espelho(&mut self, volume: u32, offset: u64, buf: &mut [u8]) -> Result<()> {
        match &mut self.espelho {
            None => Err(PhxError::NaoEncontrado(
                "esta tabela nao tem espelho".into(),
            )),
            Some(e) => e.ler(volume, offset, buf),
        }
    }

    /// Escreve SO no principal, sem tocar no espelho.
    ///
    /// Existe para o reparo e para o teste que precisa estragar um lado so.
    /// Fora disso ninguem deve usar: escrita que nao vai aos dois lugares e
    /// exatamente o que o espelho existe para evitar.
    pub fn escrever_so_no_principal(&mut self, volume: u32, offset: u64, buf: &[u8]) -> Result<()> {
        let f = self.arquivo(volume, true)?;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(buf)?;
        self.marcar_escrito(volume);
        Ok(())
    }

    /// Tamanho do volume no espelho. Zero quando ele ainda nao existe.
    pub fn tamanho_do_espelho(&mut self, volume: u32) -> Result<u64> {
        match &mut self.espelho {
            None => Ok(0),
            Some(e) => {
                if e.existe(volume) {
                    e.tamanho(volume)
                } else {
                    Ok(0)
                }
            }
        }
    }

    /// Copia um trecho do principal para o espelho, para o reparo inverso.
    pub fn escrever_no_espelho(&mut self, volume: u32, offset: u64, buf: &[u8]) -> Result<()> {
        match &mut self.espelho {
            None => Err(PhxError::NaoEncontrado(
                "esta tabela nao tem espelho".into(),
            )),
            Some(e) => {
                e.garantir(volume)?;
                e.escrever(volume, offset, buf)
            }
        }
    }

    /// Caminho de um volume no espelho, quando o espelho esta ligado.
    pub fn caminho_do_espelho(&self, volume: u32) -> Option<PathBuf> {
        self.espelho.as_ref().map(|e| e.caminho(volume))
    }

    /// Caminho de um volume. Sem paginacao o sufixo e vazio.
    pub fn caminho(&self, volume: u32) -> PathBuf {
        self.diretorio.join(format!(
            "{}{}.{}",
            self.nome,
            self.paginacao.sufixo(volume),
            self.ext
        ))
    }

    pub fn existe(&self, volume: u32) -> bool {
        self.caminho(volume).exists()
    }

    pub fn paginacao(&self) -> Paginacao {
        self.paginacao
    }

    pub fn nome(&self) -> &str {
        &self.nome
    }

    pub fn diretorio(&self) -> &Path {
        &self.diretorio
    }

    /// Volumes que existem em disco, em ordem crescente.
    pub fn existentes(&self) -> Vec<u32> {
        if !self.paginacao.ligada() {
            return if self.existe(1) { vec![1] } else { vec![] };
        }
        (1..=self.paginacao.max_arquivos)
            .filter(|v| self.existe(*v))
            .collect()
    }

    fn registrar_uso(&mut self, volume: u32) {
        if let Some(pos) = self.ordem.iter().position(|v| *v == volume) {
            self.ordem.remove(pos);
        }
        self.ordem.push_back(volume);
    }

    /// Anota que este volume foi escrito e ainda nao foi ao disco.
    ///
    /// Chamado de TODO caminho de escrita deste modulo, e de nenhum caminho de
    /// leitura -- e' a distincao que `abertos` nao faz e que o registro
    /// existe para fazer. Ver [`ESCRITAS_PENDENTES`].
    fn marcar_escrito(&mut self, volume: u32) {
        trava(&self.pendentes).insert(volume);
    }

    fn fechar_menos_usado(&mut self) {
        while self.abertos.len() >= self.limite {
            match self.ordem.pop_front() {
                Some(v) => {
                    self.abertos.remove(&v);
                }
                None => break,
            }
        }
    }

    fn arquivo(&mut self, volume: u32, criar: bool) -> Result<&mut File> {
        if !self.abertos.contains_key(&volume) {
            let caminho = self.caminho(volume);
            if !criar && !caminho.exists() {
                return Err(PhxError::NaoEncontrado(format!(
                    "volume {volume} nao existe: {}",
                    caminho.display()
                )));
            }
            self.fechar_menos_usado();
            let f = OpenOptions::new()
                .read(true)
                .write(true)
                .create(criar)
                .open(&caminho)?;
            self.abertos.insert(volume, f);
        }
        self.registrar_uso(volume);
        Ok(self.abertos.get_mut(&volume).expect("acabou de ser aberto"))
    }

    /// Poe este volume no cache de descritores, para que `sincronizar` o
    /// alcance -- e nao faz nada quando o arquivo ainda nao existe.
    ///
    /// # Por que ela e' PUBLICA, e por que o `.reg` precisa dela
    ///
    /// Seis dos oito componentes de uma tabela leem o proprio cabecalho pelo
    /// `Volumes` ao abrir (`l.cab(1)?; l.cab(volume_atual)?;` na lixeira, e o
    /// igual no `.bin`, `.memo`, `.log` e `.reason`), entao os volumes que eles
    /// podem dever ao disco ja estao em `abertos` quando `sincronizar` chega.
    /// O `.reg` e' o unico que nao faz isso: `RegFile::abrir` le o cabecalho
    /// com um `std::fs::File` direto -- e tem de ler, porque a largura do
    /// sufixo de volume mora DENTRO do cabecalho e o conjunto nao pode ser
    /// montado antes de le-lo. O ovo e a galinha sao reais; o efeito colateral
    /// de deixar o `.reg` fora do cache nao era intencional.
    ///
    /// Isto e' o irmao de disco do registro de [`ESCRITAS_PENDENTES`], e os
    /// dois cobrem buracos diferentes: o registro alcanca qualquer volume, mas
    /// so' dentro do PROCESSO que escreveu; este metodo atravessa processo,
    /// mas so' alcanca o volume que quem chama souber nomear.
    pub fn abrir_para_sincronizar(&mut self, volume: u32) -> Result<()> {
        if !self.existe(volume) {
            return Ok(());
        }
        self.arquivo(volume, false)?;
        Ok(())
    }

    /// Cria o volume zerado. Falha se ja existir.
    pub fn criar(&mut self, volume: u32) -> Result<()> {
        let caminho = self.caminho(volume);
        if caminho.exists() {
            return Err(PhxError::Esquema(format!(
                "{} ja existe",
                caminho.display()
            )));
        }
        self.fechar_menos_usado();
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&caminho)?;
        self.abertos.insert(volume, f);
        self.registrar_uso(volume);
        // Volume que nasce ainda vazio ja conta como escrito: o inode e' novo,
        // e quem o criou vai gravar nele em seguida.
        self.marcar_escrito(volume);
        Ok(())
    }

    /// Cria o volume se ainda nao existir. Devolve `true` se criou agora.
    pub fn garantir(&mut self, volume: u32) -> Result<bool> {
        if self.existe(volume) {
            self.arquivo(volume, false)?;
            Ok(false)
        } else {
            self.criar(volume)?;
            Ok(true)
        }
    }

    pub fn ler(&mut self, volume: u32, offset: u64, buf: &mut [u8]) -> Result<()> {
        let f = self.arquivo(volume, false)?;
        f.seek(SeekFrom::Start(offset))?;
        f.read_exact(buf)?;
        Ok(())
    }

    pub fn escrever(&mut self, volume: u32, offset: u64, buf: &[u8]) -> Result<()> {
        let f = self.arquivo(volume, true)?;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(buf)?;
        self.marcar_escrito(volume);
        // O espelho recebe a mesma coisa, no mesmo lugar. Falhar aqui NAO
        // desfaz a escrita boa: o principal ja tem o dado, e um espelho
        // defasado e melhor do que uma gravacao recusada.
        if let Some(e) = &mut self.espelho {
            e.garantir(volume)?;
            let g = e.arquivo(volume, true)?;
            g.seek(SeekFrom::Start(offset))?;
            g.write_all(buf)?;
            e.marcar_escrito(volume);
        }
        Ok(())
    }

    /// Escreve dois blocos seguidos (cabecalho e conteudo) sem reposicionar.
    pub fn escrever_par(
        &mut self,
        volume: u32,
        offset: u64,
        cabecalho: &[u8],
        conteudo: &[u8],
    ) -> Result<()> {
        let f = self.arquivo(volume, true)?;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(cabecalho)?;
        f.write_all(conteudo)?;
        self.marcar_escrito(volume);
        if let Some(e) = &mut self.espelho {
            e.garantir(volume)?;
            let g = e.arquivo(volume, true)?;
            g.seek(SeekFrom::Start(offset))?;
            g.write_all(cabecalho)?;
            g.write_all(conteudo)?;
            e.marcar_escrito(volume);
        }
        Ok(())
    }

    pub fn tamanho(&mut self, volume: u32) -> Result<u64> {
        let f = self.arquivo(volume, false)?;
        Ok(f.metadata()?.len())
    }

    pub fn definir_tamanho(&mut self, volume: u32, tamanho: u64) -> Result<()> {
        let f = self.arquivo(volume, true)?;
        f.set_len(tamanho)?;
        self.marcar_escrito(volume);
        if let Some(e) = &mut self.espelho {
            e.garantir(volume)?;
            e.arquivo(volume, true)?.set_len(tamanho)?;
            e.marcar_escrito(volume);
        }
        Ok(())
    }

    /// Leva ao disco tudo o que este conjunto de arquivos deve.
    ///
    /// Sao DUAS listas, e a segunda e' a que faz a promessa valer:
    ///
    /// 1. os descritores que esta instancia tem abertos -- o comportamento de
    ///    sempre, e o unico que existia;
    /// 2. os volumes marcados como escritos e ainda nao sincronizados NO
    ///    PROCESSO, inclusive por uma instancia que ja morreu. E' o caso do
    ///    fecho da janela de durabilidade, que reabre a tabela so para
    ///    sincroniza-la, e o do volume despejado do LRU. Ver
    ///    [`ESCRITAS_PENDENTES`].
    ///
    /// A marca so' sai depois que o disco confirmou: se qualquer `fsync`
    /// falhar, a lista inteira volta ao registro. Uma sincronizacao repetida
    /// custa tempo; uma marca perdida custaria o dado, e o `descarregar_sujas`
    /// do servidor conta justamente com poder tentar de novo.
    pub fn sincronizar(&mut self) -> Result<()> {
        self.sincronizacoes += 1;
        self.selo = SENHA.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let pendentes: BTreeSet<u32> = std::mem::take(&mut trava(&self.pendentes));
        match self.sincronizar_listas(&pendentes) {
            Ok(()) => {}
            Err(e) => {
                trava(&self.pendentes).extend(pendentes);
                return Err(e);
            }
        }
        if let Some(e) = &mut self.espelho {
            e.sincronizar()?;
        }
        Ok(())
    }

    /// As duas listas de `sincronizar`, sem o espelho e sem o registro.
    ///
    /// A lista sai INTEIRA antes do primeiro `fsync`, e isso nao e' estilo. Um
    /// laco que sincroniza e ABRE ao mesmo tempo faz o LRU despejar quem acabou
    /// de ser sincronizado, e a volta do laco o sincroniza de novo: com o cache
    /// em 4 e oito volumes sujos saiam **12** `fsync` para oito arquivos.
    fn sincronizar_listas(&mut self, pendentes: &BTreeSet<u32>) -> Result<()> {
        let mut alvos: BTreeSet<u32> = self.abertos.keys().copied().collect();
        for volume in pendentes {
            // O volume pode ter sumido entre a escrita e agora -- `apagar_tudo`
            // no reindex, um `rename` que trocou o arquivo inteiro. Arquivo que
            // nao existe nao tem pagina suja a levar.
            if !alvos.contains(volume) && self.existe(*volume) {
                alvos.insert(*volume);
            }
        }
        for volume in alvos {
            // Quem ja esta em `abertos` volta pelo mesmo descritor, inclusive
            // se o arquivo tiver sido apagado debaixo dele: `arquivo` so' pede
            // o disco quando o volume nao esta no cache.
            let f = self.arquivo(volume, false)?;
            f.flush()?;
            f.sync_all()?;
            self.sincronizados += 1;
        }
        Ok(())
    }

    /// Quantas vezes este conjunto pediu o disco. Ver o campo.
    pub fn sincronizacoes(&self) -> u64 {
        self.sincronizacoes
    }

    /// Quantos arquivos este conjunto ja mandou ao disco de verdade. Ver o
    /// campo -- ele conta o ARQUIVO, e o `sincronizacoes` conta a CHAMADA.
    pub fn sincronizados(&self) -> u64 {
        self.sincronizados
    }

    /// A senha da ultima sincronizacao. Zero = nunca sincronizou. Ver o campo.
    pub fn selo(&self) -> u64 {
        self.selo
    }

    /// Fecha todos os descritores. Usado antes de apagar arquivos.
    pub fn fechar_todos(&mut self) {
        self.abertos.clear();
        self.ordem.clear();
        if let Some(e) = &mut self.espelho {
            e.fechar_todos();
        }
    }

    /// Apaga todos os volumes do conjunto. Usado pelo reindex, que recria o
    /// `.ndx` do zero.
    pub fn apagar_tudo(&mut self) -> Result<()> {
        let volumes = self.existentes();
        self.fechar_todos();
        for v in volumes {
            std::fs::remove_file(self.caminho(v))?;
        }
        // As marcas de escrita morrem com os arquivos: nao ha pagina suja a
        // levar para um inode que deixou de existir. `sincronizar_listas`
        // tambem pula o que sumiu, mas limpar aqui evita o registro crescer
        // com volume que nunca mais vai voltar.
        trava(&self.pendentes).clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    fn dir_temp(rotulo: &str) -> crate::apoio_teste::DirTemp {
        crate::apoio_teste::DirTemp::novo(&format!("vol-{rotulo}"))
    }

    /// A PREMISSA da chave da familia, em forma de teste.
    ///
    /// O comentario do `ESCRITAS_PENDENTES` afirmava que «duas grafias
    /// diferentes do mesmo diretorio dariam duas familias», e a afirmacao
    /// estava errada em metade dos casos. Premissa que so' vive num
    /// comentario envelhece calada -- esta trava as quatro que foram medidas.
    #[test]
    fn a_chave_da_familia_junta_as_grafias_que_o_pathbuf_ja_junta() {
        let chave = |d: &str| familia(Path::new(d), "clientes", "reg");
        // O que o proprio `PathBuf` ja junta, por `components()`.
        assert_eq!(chave("/tmp/x"), chave("/tmp/./x"));
        assert_eq!(chave("/tmp/x"), chave("/tmp/x/"));
        // O que a resolucao junta, e era o defeito.
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(chave(cwd.join("dados").to_str().unwrap()), chave("dados"));
        // E o que ela NAO junta, de proposito: `..` fica, e `canonicalize`
        // seria o unico jeito -- ao preco de tocar o disco e de falhar em
        // caminho que ainda nao existe, que quebraria `Table::criar`.
        assert_ne!(chave("/tmp/x"), chave("/tmp/y/../x"));
    }

    #[test]
    fn sem_paginacao_usa_arquivo_unico_sem_sufixo() {
        let d = dir_temp("unico");
        let v = Volumes::novo(&d, "cadastroClientes", "reg", Paginacao::DESLIGADA);
        assert_eq!(
            v.caminho(1).file_name().unwrap().to_string_lossy(),
            "cadastroClientes.reg"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn com_paginacao_usa_sufixo_numerado() {
        let d = dir_temp("paginado");
        let p = Paginacao::nova(1_000, 999).unwrap();
        let v = Volumes::novo(&d, "cadastroClientes", "reg", p);
        assert_eq!(
            v.caminho(1).file_name().unwrap().to_string_lossy(),
            "cadastroClientes_001.reg"
        );
        assert_eq!(
            v.caminho(42).file_name().unwrap().to_string_lossy(),
            "cadastroClientes_042.reg"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// A prova do registro de escritas pendentes, no caso que motivou tudo:
    /// quem escreveu ja morreu, e quem sincroniza nunca tocou o arquivo.
    ///
    /// **Prova real:** apague a chamada a `marcar_escrito` do `escrever` e este
    /// teste falha com `sincronizados = 0` -- que era exatamente o que
    /// `sincronizar()` fazia enquanto devolvia `Ok(())`.
    #[test]
    fn o_fecho_alcanca_o_que_outra_instancia_escreveu() {
        let d = dir_temp("pendente-de-outro");
        {
            let mut a = Volumes::novo(&d, "t", "reg", Paginacao::DESLIGADA);
            a.criar(1).unwrap();
            // Sincroniza AQUI: sem isto a marca que o `criar` deixou faria o
            // teste passar mesmo com a marca do `escrever` apagada -- e teste
            // que passa por engano e' pior que teste que falta.
            a.sincronizar().unwrap();
            a.escrever(1, 0, b"dado que ainda nao foi ao disco")
                .unwrap();
            // Morre sem sincronizar: e' o `gravar_de_verdade` quando a janela
            // de durabilidade ainda nao fechou.
        }
        let mut b = Volumes::novo(&d, "t", "reg", Paginacao::DESLIGADA);
        assert_eq!(
            b.sincronizados(),
            0,
            "a instancia nova nao tocou nada ainda"
        );
        b.sincronizar().unwrap();
        assert!(
            b.sincronizados() >= 1,
            "o fecho da janela nao mandou nenhum arquivo ao disco: quem escreveu              foi outra instancia, e `abertos` desta esta vazio"
        );
        // E a marca sai depois de gastar: sincronizar de novo, sem escrita no
        // meio, nao repete o `fsync` do que ja foi.
        let gastos = b.sincronizados();
        b.sincronizar().unwrap();
        assert_eq!(
            b.sincronizados(),
            gastos + 1,
            "o segundo fecho devia gastar so' o descritor que o primeiro deixou              aberto, e nao repetir a lista de pendentes"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// O volume do MEIO de uma tabela paginada -- o que nem a reabertura abre,
    /// porque nao e' o volume 1 nem a fronteira de escrita.
    #[test]
    fn o_fecho_alcanca_volume_do_meio_de_tabela_paginada() {
        let d = dir_temp("pendente-do-meio");
        let p = Paginacao::nova(10, 99).unwrap();
        {
            let mut a = Volumes::novo(&d, "t", "reg", p);
            for v in 1..=5 {
                a.criar(v).unwrap();
            }
            a.sincronizar().unwrap();
            // So' o volume 3, que e' o caso de um `atualizar` no meio.
            a.escrever(3, 0, b"linha alterada no meio").unwrap();
        }
        let mut b = Volumes::novo(&d, "t", "reg", p);
        b.sincronizar().unwrap();
        assert_eq!(
            b.sincronizados(),
            1,
            "devia mandar ao disco exatamente o volume 3 -- nem menos (o dado              se perde numa queda de energia) nem mais (fsync em volume limpo              custa 52 us medidos)"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// O terceiro buraco do mesmo defeito: o LRU despeja o descritor de quem
    /// escreveu, e o `fsync` ia embora junto.
    #[test]
    fn o_despejo_do_lru_nao_leva_o_fsync_junto() {
        let d = dir_temp("despejo-lru");
        let p = Paginacao::nova(10, 999).unwrap();
        let mut v = Volumes::novo(&d, "t", "reg", p);
        v.limite = 4; // o mesmo mecanismo, numa escala que cabe num teste
        for n in 1..=8 {
            v.criar(n).unwrap();
        }
        // O mesmo cuidado do teste irmao: a marca do `criar` sai da frente
        // antes de a do `escrever` ser cobrada.
        v.sincronizar().unwrap();
        let ate_aqui = v.sincronizados();
        for n in 1..=8 {
            v.escrever(n, 0, b"escrita que ainda nao foi ao disco")
                .unwrap();
        }
        assert!(
            !v.abertos.contains_key(&1),
            "o teste so' prova algo se o volume 1 tiver mesmo sido despejado"
        );
        v.sincronizar().unwrap();
        assert_eq!(
            v.sincronizados() - ate_aqui,
            8,
            "os oito volumes foram escritos e nenhum pode ficar sem `fsync` --              quatro estao no cache e quatro sairam dele"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn le_e_escreve_em_volumes_diferentes() {
        let d = dir_temp("rw");
        let p = Paginacao::nova(10, 99).unwrap();
        let mut v = Volumes::novo(&d, "t", "reg", p);

        v.escrever(1, 0, b"volume um").unwrap();
        v.escrever(3, 0, b"volume tres").unwrap();

        let mut buf = vec![0u8; 9];
        v.ler(1, 0, &mut buf).unwrap();
        assert_eq!(&buf, b"volume um");

        let mut buf = vec![0u8; 11];
        v.ler(3, 0, &mut buf).unwrap();
        assert_eq!(&buf, b"volume tres");

        assert_eq!(v.existentes(), vec![1, 3]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn cache_lru_nao_estoura_o_limite_de_descritores() {
        let d = dir_temp("lru");
        let p = Paginacao::nova(10, 999).unwrap();
        let mut v = Volumes::novo(&d, "t", "reg", p);
        v.limite = 4;

        for vol in 1..=20u32 {
            v.escrever(vol, 0, format!("vol {vol}").as_bytes()).unwrap();
            assert!(
                v.abertos.len() <= 4,
                "abriu {} descritores, limite e 4",
                v.abertos.len()
            );
        }
        // Todos continuam legiveis depois de terem sido fechados e reabertos.
        for vol in 1..=20u32 {
            let esperado = format!("vol {vol}");
            let mut buf = vec![0u8; esperado.len()];
            v.ler(vol, 0, &mut buf).unwrap();
            assert_eq!(String::from_utf8(buf).unwrap(), esperado);
        }
        assert_eq!(v.existentes().len(), 20);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn volume_inexistente_e_erro_na_leitura() {
        let d = dir_temp("faltando");
        let mut v = Volumes::novo(&d, "t", "reg", Paginacao::nova(10, 99).unwrap());
        let mut buf = [0u8; 4];
        assert!(v.ler(7, 0, &mut buf).is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn apagar_tudo_limpa_o_conjunto() {
        let d = dir_temp("apagar");
        let mut v = Volumes::novo(&d, "t", "ndx", Paginacao::nova(10, 99).unwrap());
        for vol in 1..=5u32 {
            v.escrever(vol, 0, b"x").unwrap();
        }
        assert_eq!(v.existentes().len(), 5);
        v.apagar_tudo().unwrap();
        assert!(v.existentes().is_empty());
        std::fs::remove_dir_all(&d).unwrap();
    }
}
