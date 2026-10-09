//! O expurgo do diario (`.log`) -- pedido 706.
//!
//! O caixa que fica meses no ar enchia o disco com o diario: ~1,6 KB por
//! venda, sem limite (bancada do 678). Aqui moram as tres pecas do lado do
//! servidor: a confirmacao de quem puxa, a recusa dita a quem pede o que ja
//! saiu, e a passada periodica que tira os volumes fechados. O desenho e o
//! parecer do papel C (`docs/propostas/expurgo-do-diario-706.md`); o prazo e
//! decisao do dono (08/10/2026): o caixa segura no maximo 30 dias de diario
//! nao confirmado, e nunca para de vender.

use super::*;

/// Um dia em milissegundos, para o prazo do diario.
const DIA_MS: i64 = 86_400_000;

/// O relatorio de uma passada do expurgo do diario. Diz o que NAO fez:
/// tabela retida pelo consumidor que nao confirmou aparece pelo nome.
#[derive(Debug, Default)]
pub(super) struct ExpurgoDosDiarios {
    pub(super) volumes: u64,
    pub(super) eventos: u64,
    pub(super) bytes: u64,
    /// Volumes que sairam pelo PRAZO, sem confirmacao -- o central que
    /// estava fora vai receber a recusa e se refazer por copia.
    pub(super) pelo_prazo: u64,
    pub(super) retidas: Vec<String>,
    pub(super) falhas: Vec<String>,
}

impl ExpurgoDosDiarios {
    pub(super) fn resumo(&self) -> String {
        let mut s = format!(
            "expurgo do diario: {} volume(s), {} evento(s), {} bytes",
            self.volumes, self.eventos, self.bytes
        );
        if self.pelo_prazo > 0 {
            s.push_str(&format!(
                "; {} pelo PRAZO, sem confirmacao de todos os consumidores",
                self.pelo_prazo
            ));
        }
        if !self.retidas.is_empty() {
            s.push_str(&format!("; retidas: {}", self.retidas.join(", ")));
        }
        if !self.falhas.is_empty() {
            s.push_str(&format!("; FALHAS: {}", self.falhas.join("; ")));
        }
        s
    }
}

impl Servidor {
    /// Anota o que quem puxa confirmou ter: tudo antes de `desde`.
    ///
    /// O portao vem ANTES do trabalho: sem `diario.expurgo` ligado nao se
    /// monta nem a chave -- o `replicar` de todo servidor que nao expurga
    /// continua custando o que custava. Quem puxa sem estar em
    /// `diario.consumidores` nao segura nada: e o slot do PostgreSQL, que so
    /// existe declarado.
    ///
    /// # Quem confirma e a SESSAO, e nao o campo -- pedido 728
    ///
    /// Com login, o consumidor e o LOGIN da sessao, diga o pedido o que
    /// disser: qualquer sessao com `replicar` mandava `"consumidor":"central"`
    /// com a ponta no `desde`, a confirmacao (que e `max` e nao volta) soltava
    /// o que o central de verdade nao puxou, e o bidirecional -- que nao se
    /// refaz por retrato -- parava em `erro.diario_expurgado` com venda que
    /// nao chegou. Quem declara `diario.consumidores` num servidor com
    /// cadastro poe ali o LOGIN de cada replica.
    ///
    /// Sem login (servidor sem cadastro, onde o token e a identidade inteira)
    /// o nome vem de `consumidor` (a replica fiel e o espelho) ou de `para`
    /// (o bidirecional, que ja dizia quem pede): ali nao ha um segundo
    /// alguem por quem se passar.
    pub(super) fn anotar_confirmacao_do_diario(
        &self,
        p: &Json,
        sessao: &Sessao,
        chave: &str,
        desde: u64,
    ) {
        let cfg = &self.config.diario;
        if !cfg.expurgo || cfg.consumidores.is_empty() {
            return;
        }
        let quem = match (sessao.usuario.as_ref(), p.texto_ou("consumidor", "").trim()) {
            (Some(u), _) => u.login.as_str(),
            (None, "") => p.texto_ou("para", "").trim(),
            (None, q) => q,
        };
        if quem.is_empty() || !cfg.consumidores.iter().any(|c| c == quem) {
            return;
        }
        // O `duravel` (quando vem) e o que a replica JA levou ao disco; o
        // `desde` conta tambem a cauda da rodada, que uma queda de energia
        // dela leva embora. O menor dos dois e o que se confirma.
        let confirmado = match p.campo("duravel").and_then(Json::inteiro) {
            Some(d) if d >= 0 => (d as u64).min(desde),
            _ => desde,
        };
        let mut m = self
            .confirmados_do_diario
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let v = m.entry((quem.to_string(), chave.to_string())).or_insert(0);
        *v = (*v).max(confirmado);
    }

    /// O menor `desde` confirmado entre TODOS os consumidores declarados, ou
    /// `None` quando algum ainda nao confirmou nada desta tabela -- e ai so o
    /// prazo tira volume.
    fn confirmado_por_todos(&self, chave: &str) -> Option<u64> {
        let cfg = &self.config.diario;
        if cfg.consumidores.is_empty() {
            return None;
        }
        let m = self
            .confirmados_do_diario
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cfg.consumidores
            .iter()
            .map(|c| m.get(&(c.clone(), chave.to_string())).copied())
            .try_fold(u64::MAX, |menor, v| v.map(|v| menor.min(v)))
    }

    /// A recusa DITA a quem pede o diario abaixo do que ainda existe.
    ///
    /// So e perguntada quando a leitura ja falhou: no caminho feliz o
    /// `replicar` nao paga a conta da base. Devolve `None` quando a falha e
    /// outra, e ai o erro original segue.
    pub(super) fn recusa_do_diario_expurgado(
        &self,
        t: &mut Table,
        p: &Json,
        desde: u64,
    ) -> Option<PhxError> {
        let base = t.base_do_diario().ok()?;
        (desde < base).then(|| {
            PhxError::NaoEncontrado(self.msg(
                "erro.diario_expurgado",
                &[
                    ("tabela", p.texto_ou("tabela", "")),
                    ("desde", &desde.to_string()),
                    ("base", &base.to_string()),
                ],
            ))
        })
    }

    /// O expurgo do diario de UMA tabela, nas fases do da trilha (368): o
    /// plano e a base com a trava, o `fsync` da base sem ela, os volumes
    /// saindo com ela de novo, e o `fsync` da pasta sem ela.
    fn expurgar_diario_da_tabela(
        &self,
        database: &str,
        tabela: &str,
        limite_ms: Option<i64>,
    ) -> Result<phxsql_store::log::PlanoDoExpurgo> {
        let confirmado = self.confirmado_por_todos(&Self::chave_do_diario(database, tabela));
        let p = Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("tabela", Json::texto_de(tabela)),
        ]);
        let sessao = Sessao::default();
        // Fase 1, COM a trava: decide e grava a base em quem fica. Marca de
        // transacao de pe guarda posicoes do diario: com ela, nada sai.
        let plano = {
            let dados = self.travar_dados()?;
            let pasta = dados.abrir_database(database)?.diretorio(None)?;
            let mut t = self.abrir_travada(&dados, &p, &sessao)?;
            let com_marca = [pasta.as_path(), t.diretorio()].iter().any(|d| {
                !phxsql_store::marca::marcas_em(d).is_empty()
                    || !phxsql_store::marca::marcas_do_bidi_em(d).is_empty()
            });
            if com_marca {
                return t.planejar_expurgo_do_diario(None, None);
            }
            let plano = t.planejar_expurgo_do_diario(confirmado, limite_ms)?;
            if plano.vazio() {
                return Ok(plano);
            }
            t.gravar_bases_do_expurgo_do_diario(&plano)?;
            plano
        };
        // Fase 2, SEM a trava: a base vai ao disco antes de qualquer unlink.
        plano.levar_bases_ao_disco()?;
        // Fase 3, COM a trava: os volumes saem, conferidos de novo.
        let (r, pendente) = {
            let dados = self.travar_dados()?;
            let mut t = self.abrir_travada(&dados, &p, &sessao)?;
            t.concluir_expurgo_do_diario_adiando_o_fsync(&plano)
        };
        // Fase 4, SEM a trava: o `fsync` da pasta, tambem no erro (598).
        pendente.levar_ao_disco()?;
        r.map(|()| plano)
    }

    /// Uma passada do expurgo do diario por todas as tabelas. `agora` vem de
    /// fora para o teste viver trinta dias num segundo.
    pub(super) fn expurgar_diarios(&self, agora: i64) -> Result<ExpurgoDosDiarios> {
        let cfg = &self.config.diario;
        let mut r = ExpurgoDosDiarios::default();
        if !cfg.expurgo {
            return Ok(r);
        }
        let limite = if cfg.prazo_s > 0 {
            Some(agora - (cfg.prazo_s.min(i64::MAX as u64 / 1000) as i64) * 1000)
        } else {
            (cfg.prazo_dias > 0).then(|| agora - i64::from(cfg.prazo_dias) * DIA_MS)
        };
        let _um_por_vez = self.expurgo_do_diario.tomar("expurgo_do_diario")?;
        let tabelas: Vec<(String, String)> = {
            let dados = self.travar_dados()?;
            let mut v = Vec::new();
            for db in dados.databases()? {
                match dados.abrir_database(&db).and_then(|b| b.todas_as_tabelas()) {
                    Ok(lista) => v.extend(lista.into_iter().map(|t| (db.clone(), t))),
                    Err(e) => r.falhas.push(format!("{db}: {e}")),
                }
            }
            v
        };
        for (db, tab) in tabelas {
            match self.expurgar_diario_da_tabela(&db, &tab, limite) {
                Ok(plano) => {
                    r.volumes += plano.volumes.len() as u64;
                    r.eventos += plano.eventos;
                    r.bytes += plano.bytes;
                    r.pelo_prazo += plano
                        .volumes
                        .iter()
                        .filter(|(_, m)| *m == phxsql_store::log::MotivoDoExpurgo::Prazo)
                        .count() as u64;
                    if plano.parada == phxsql_store::log::ParadaDoExpurgo::Retido {
                        r.retidas.push(format!("{db}.{tab}"));
                    }
                }
                Err(e) => r.falhas.push(format!("{db}.{tab}: {e}")),
            }
        }
        Ok(r)
    }

    /// Sobe a passada do expurgo do diario, se `diario.expurgo` pedir.
    ///
    /// O portao vem ANTES do trabalho: desligado, nao ha thread nenhuma.
    pub(super) fn subir_expurgo_do_diario(self: &Arc<Self>) {
        let cfg = &self.config.diario;
        if !cfg.expurgo {
            return;
        }
        let passada = Duration::from_secs(cfg.passada_s.max(1));
        eprintln!(
            "expurgo do diario: a cada {} s saem os volumes fechados do .log que \
             {} confirmaram{}; o ativo e o ultimo fechado nunca",
            passada.as_secs(),
            if cfg.consumidores.is_empty() {
                "nenhum consumidor declarado".to_string()
            } else {
                cfg.consumidores.join(", ")
            },
            if cfg.prazo_s > 0 {
                format!(
                    ", e os de mais de {} s mesmo sem confirmacao (prazo de ENSAIO)",
                    cfg.prazo_s
                )
            } else if cfg.prazo_dias > 0 {
                format!(
                    ", e os de mais de {} dia(s) mesmo sem confirmacao",
                    cfg.prazo_dias
                )
            } else {
                String::new()
            }
        );
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "expurgo-do-diario",
            "tira do .log os volumes fechados que todos os consumidores \
             declarados ja puxaram, ou que passaram do prazo -- nunca o ativo \
             nem o ultimo fechado (pedido 706)",
            "servico",
            crate::agora_ms(),
            move |fio| loop {
                fio.fazendo("esperando a passada");
                std::thread::sleep(passada);
                fio.fazendo("expurgando o diario");
                match servidor.expurgar_diarios(crate::agora_ms()) {
                    Ok(r) if r.volumes > 0 || !r.falhas.is_empty() => {
                        eprintln!("{}", r.resumo())
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("expurgo do diario FALHOU: {e}"),
                }
            },
        );
    }
}

/// O maior pedaco de um arquivo de retrato numa resposta: 8 MiB, que em
/// hexadecimal sao 16 MiB -- folga larga sob o teto de 128 MiB do registro do
/// fio, e um pedido por pedaco nao segura a origem.
const PEDACO_DO_RETRATO: u64 = 8 * 1024 * 1024;

/// Quantos retratos esta origem serve ao mesmo tempo -- pedido 729. Cada um e
/// uma copia inteira de um database na raiz: sem teto, um laco de pedidos de
/// quem tem `replicar` enchia o disco (e, ate a copia em duas passadas,
/// congelava as escritas a copia inteira). Quatro cobrem as replicas atrasadas de um
/// central com folga; a quinta ouve a recusa e tenta na rodada seguinte.
pub(super) const TETO_DE_RETRATOS: usize = 4;

/// O que o retrato deixa livre no disco DEPOIS da copia -- pedido 729. A
/// copia que levasse o disco da origem a zero pararia a venda do caixa, que e
/// o contrario do que o expurgo existe para garantir.
const FOLGA_DE_DISCO_DO_RETRATO: u64 = 256 * 1024 * 1024;

/// O prefixo dos arquivos de retrato na raiz de dados. Arquivo, e nao pasta:
/// toda pasta da raiz e um database. Os dois comecam pelo prefixo comum do
/// motor, que o backup pula (pedido 731).
const PREFIXO_SERVIDO: &str = ".retrato-servido-";
const PREFIXO_RECEBIDO: &str = ".retrato-recebido-";

/// Os dois prefixos comecam pelo do motor, que o backup pula: conferido pelo
/// compilador, para o nome daqui nao divergir do de la calado.
const _: () = {
    const fn comeca_com(s: &str, p: &str) -> bool {
        let (s, p) = (s.as_bytes(), p.as_bytes());
        if p.len() > s.len() {
            return false;
        }
        let mut i = 0;
        while i < p.len() {
            if s[i] != p[i] {
                return false;
            }
            i += 1;
        }
        true
    }
    let comum = phxsql_store::catalogo::PREFIXO_DO_RETRATO_DA_REPLICA;
    assert!(comeca_com(PREFIXO_SERVIDO, comum) && comeca_com(PREFIXO_RECEBIDO, comum));
};

/// O retrato que esta origem serve -- pedido 706.
///
/// Amarrado a CONEXAO que o tirou, ao login dela e ao database -- pedido 727.
/// Ate ali ele guardava so o `id` (o `agora_ms()`, que se adivinha varrendo a
/// janela) e os arquivos: o portao de permissao conferia o database do
/// PEDIDO, e quem tinha `replicar` em `loja` lia o `.reg` cru de `rh` pedindo
/// o pedaco com o id do outro, em claro, e o soltava.
pub(super) struct RetratoServido {
    id: u64,
    database: String,
    ligacao: u64,
    login: String,
    /// Alguma tabela do retrato tem coluna marcada: o pedaco exige o fio
    /// cifrado, como o `replicar` (pedido 342).
    exige_cifra: bool,
    /// `(tabela, arquivo, copia, bytes)`.
    arquivos: Vec<(String, String, PathBuf, u64)>,
}

impl RetratoServido {
    /// O nome da PASTA da copia na raiz (pedido 729): a arvore da copia em
    /// duas passadas do backup. O `subdiretorios` do catalogo a pula, senao
    /// ela seria um database a mais.
    fn prefixo(&self) -> String {
        format!("{PREFIXO_SERVIDO}{}", self.id)
    }
}

/// Apaga da raiz o que comeca com `prefixo` -- o retrato servido que acabou
/// (uma pasta, desde o 729), os arquivos recebidos, ou o que uma queda
/// deixou para tras.
fn apagar_retratos(raiz: &Path, prefixo: &str) {
    let Ok(entradas) = std::fs::read_dir(raiz) else {
        return;
    };
    for e in entradas.flatten() {
        if e.file_name().to_string_lossy().starts_with(prefixo) {
            // `symlink_metadata`: um link com o nome do retrato sai como link,
            // e nunca leva junto a pasta para onde aponta.
            match std::fs::symlink_metadata(e.path()) {
                Ok(m) if m.is_dir() => {
                    let _ = std::fs::remove_dir_all(e.path());
                }
                Ok(_) => {
                    let _ = std::fs::remove_file(e.path());
                }
                Err(_) => {}
            }
        }
    }
}

/// O arranque apaga toda copia de retrato que uma vida anterior deixou --
/// pedido 731. Servida ou recebida, ela e uma copia inteira de tabelas com
/// coluna marcada FORA do ciclo da tabela: o esquecimento e a exclusao feitos
/// depois nao a alcancam. Nenhum retrato sobrevive ao processo que o tirou
/// (a conexao que o amarrava morreu junto), entao nao ha o que preservar.
pub(super) fn limpar_retratos_no_arranque(raiz: &Path) {
    apagar_retratos(raiz, PREFIXO_SERVIDO);
    apagar_retratos(raiz, PREFIXO_RECEBIDO);
}

/// Os bytes dos arquivos de um database, para o teto de disco do retrato.
/// Conta tudo, e nao so o que a replica alcanca: a estimativa por cima e a
/// que erra do lado de nao encher o disco.
fn bytes_do_database(dir: &Path) -> u64 {
    let mut total = 0;
    let mut pilha = vec![dir.to_path_buf()];
    while let Some(d) = pilha.pop() {
        let Ok(entradas) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entradas.flatten() {
            match e.metadata() {
                Ok(m) if m.is_dir() => pilha.push(e.path()),
                Ok(m) => total += m.len(),
                Err(_) => {}
            }
        }
    }
    total
}

/// Um id que nao se adivinha -- pedido 727. 53 bits, porque o id viaja como
/// numero do JSON (um `f64`): acima disso a replica pediria outro id. A
/// amarracao a conexao e a guarda; o sorteio so tira o oraculo.
fn sortear_id_do_retrato() -> u64 {
    let b = phxsql_core::senha::bytes_aleatorios(8);
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[..8]);
    (u64::from_le_bytes(v) & ((1u64 << 53) - 1)).max(1)
}

impl Servidor {
    /// `retrato_da_replica`: o retrato fiel de um database, para a replica que
    /// ficou atras do que o diario daqui ainda guarda -- pedido 706, o item
    /// (d) do parecer do papel C (`pg_basebackup`, `mariabackup` + GTID: os
    /// tres maduros refazem a replica atrasada por retrato com posicao).
    ///
    /// Tres formas, numa operacao so: sem `id`, tira o retrato e devolve a
    /// lista de arquivos; com `id` e `indice`, devolve um pedaco de um deles;
    /// com `soltar`, apaga o retrato. A posicao nao viaja: ela esta DENTRO do
    /// `.log` copiado, na base do primeiro volume (bytes 104..112).
    ///
    /// So pela porta de DADOS (pedido 727): o retrato vive amarrado a
    /// conexao, e some quando ela cai (731). A porta web nao tem conexao para
    /// amarrar -- um retrato tirado por ela ficaria na raiz sem dono.
    pub(super) fn op_retrato_da_replica(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        if sessao.ligacao == 0 {
            return Err(PhxError::Autorizacao(
                self.msg("erro.retrato_so_pela_porta_de_dados", &[]),
            ));
        }
        if p.booleano_ou("soltar", false) {
            return self.soltar_pelo_pedido(p, sessao);
        }
        if p.campo("indice").is_some() {
            return self.pedaco_do_retrato(p, sessao);
        }
        self.tirar_retrato(p, sessao)
    }

    /// Solta o retrato DESTA conexao. Com `id` de outra, recusa e conta
    /// violacao -- pedido 727: soltar o retrato alheio era o jeito de a
    /// replica legitima nunca terminar.
    fn soltar_pelo_pedido(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        if let Some(id) = p.campo("id").and_then(Json::inteiro) {
            self.retrato_desta_sessao(id.max(0) as u64, p, sessao)?;
        }
        self.soltar_retrato_da_ligacao(sessao.ligacao);
        Ok(Json::objeto(vec![("soltou", Json::Bool(true))]))
    }

    /// Solta o retrato que a conexao `ligacao` tirou, se ha -- no `soltar`
    /// dela e quando ela cai (pedido 731: a replica que morre no meio nao
    /// deixa copia de dado pessoal na raiz).
    pub(super) fn soltar_retrato_da_ligacao(&self, ligacao: u64) {
        let saiu: Vec<RetratoServido> = {
            let mut r = self
                .retrato_servido
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let (sai, fica) = std::mem::take(&mut *r)
                .into_iter()
                .partition(|x| x.ligacao == ligacao);
            *r = fica;
            sai
        };
        for r in saiu {
            apagar_retratos(&self.config.base, &r.prefixo());
        }
    }

    /// O retrato `id`, se e DESTA conexao, deste login e do database do
    /// pedido -- o `database` que o portao de permissao conferiu. Qualquer
    /// outra coisa e a mesma recusa, contada como violacao: o «nao ha» e o
    /// «nao e seu» respondendo diferente seriam o oraculo do pedido 727.
    fn retrato_desta_sessao(&self, id: u64, p: &Json, sessao: &Sessao) -> Result<()> {
        let database = p.texto_ou("database", "").trim();
        let r = self
            .retrato_servido
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let meu = r.iter().any(|x| {
            x.id == id
                && x.ligacao == sessao.ligacao
                && x.login == sessao.login()
                && x.database.eq_ignore_ascii_case(database)
        });
        drop(r);
        if meu {
            return Ok(());
        }
        if !sessao.ip.is_empty() {
            self.violacao_leve(&sessao.ip, "retrato_da_replica", "retrato de outra sessao");
        }
        Err(PhxError::Autorizacao(
            self.msg("erro.retrato_alheio", &[("id", &id.to_string())]),
        ))
    }

    fn tirar_retrato(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "").trim().to_string();
        if database.is_empty() {
            return Err(PhxError::Esquema("informe \"database\"".into()));
        }
        // O teto vem ANTES da copia (pedido 729): a recusa custa uma trava de
        // `Mutex`, e a copia que ela evita custa o database inteiro com a
        // trava global na mao. Um retrato por conexao: o novo pedido da mesma
        // conexao troca o dela, e nao conta duas vezes.
        self.soltar_retrato_da_ligacao(sessao.ligacao);
        let vivos = self
            .retrato_servido
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len();
        if vivos >= TETO_DE_RETRATOS {
            return Err(PhxError::EmCarga(self.msg(
                "erro.retratos_no_teto",
                &[("teto", &TETO_DE_RETRATOS.to_string())],
            )));
        }
        // O teto de disco (pedido 729), pelo mesmo `df` do backup. Sem medida
        // (fora do Linux) nao recusa: o backup tambem nao.
        let precisa = bytes_do_database(&self.config.base.join(&database));
        if let Some(livre) = self.espaco_livre_em(&self.config.base) {
            if livre < precisa.saturating_add(FOLGA_DE_DISCO_DO_RETRATO) {
                return Err(PhxError::EmCarga(self.msg(
                    "erro.retrato_sem_disco",
                    &[
                        ("database", &database),
                        ("precisa", &precisa.to_string()),
                        ("livre", &livre.to_string()),
                    ],
                )));
            }
        }
        // Que tabelas o retrato leva: as que a replica alcanca (o mesmo
        // `replica_alcanca` do `posicao`). Com a ficha exclusiva, curta -- a
        // lista, a troca de volume por terminar e a abertura GRAVAVEL de cada
        // uma, que cura o que a abertura de leitura da fase 2 recusaria.
        // Nenhum byte copiado. A cura vem ANTES da fase 1 de proposito:
        // feita so na fase 2, ela mudava os arquivos que a fase 1 ja tinha
        // copiado, e a fase 2 os recopiava inteiros com o portao fechado --
        // medido, 1.666 ms de escrita parada num database de 99 MiB. E so
        // quando a leitura recusaria: a abertura gravavel escreve a marca do
        // `.ndx` mesmo na tabela sa, e a fase 2 recopiava o arquivo
        // «recem-mudado» -- medido, 93-107 ms parada contra 0.
        let tabelas: std::collections::BTreeSet<String> = {
            let dados = self.travar_dados()?;
            let db = dados.abrir_database(&database)?;
            let mut t = std::collections::BTreeSet::new();
            for nome in db.todas_as_tabelas()? {
                if replica_alcanca(sessao.usuario.as_ref(), &database, &nome) {
                    db.preparar_para_o_retrato(&nome)?;
                    if db.ler_para_o_retrato(&nome)?.is_none() {
                        drop(db.abrir_qualificada(&nome)?);
                    }
                    t.insert(nome);
                }
            }
            t
        };
        let mut retrato = RetratoServido {
            id: sortear_id_do_retrato(),
            database: database.clone(),
            ligacao: sessao.ligacao,
            login: sessao.login().to_string(),
            exige_cifra: false,
            arquivos: Vec::new(),
        };
        let pasta = self.config.base.join(retrato.prefixo());
        let origem = self.config.base.join(&database);
        // A copia em DUAS PASSADAS, pelo motor do backup (pedido 729): a fase
        // 1 copia sem trava nenhuma, com a escrita andando; a fase 2, com a
        // ficha exclusiva, descarrega as sujas e recopia so o que mudou. Antes a copia inteira corria com a ficha exclusiva --
        // 52-65 ms por 99 MiB medidos, ~0,6 s por GiB de escrita parada.
        let copiado = self.copiar_o_retrato(
            "retrato_da_replica",
            &origem,
            true,
            || {
                let fase = phxsql_store::backup::copiar_fase_1_das_tabelas(
                    &origem,
                    &pasta,
                    tabelas.clone(),
                )?;
                // So em debug: a pausa da prova do 729, DENTRO da copia --
                // onde o escritor esperava a copia inteira e agora nao espera.
                #[cfg(debug_assertions)]
                if let Some(ms) = gancho_de_teste("PHXSQL_TESTE_PAUSA_NA_COPIA_DO_RETRATO_MS") {
                    std::thread::sleep(Duration::from_millis(ms));
                }
                Ok(fase)
            },
            // A fase 2, com a ficha exclusiva: descarrega as sujas (o retrato
            // so vale com os cabecalhos do `.log` e do `.reg` escritos),
            // le o portao do 342 e os eventos de cada tabela, e acerta a copia
            // -- `stat` de tudo, recopia do que mudou desde a fase 1.
            |mut fase, eventos, ficha| {
                let FichaDaFase2::Exclusiva(dados) = ficha else {
                    unreachable!("o retrato pede a ficha exclusiva")
                };
                self.descarregar_sujas_com(dados);
                let db = dados.abrir_database(&database)?;
                let mut lidas = Vec::new();
                for nome in &tabelas {
                    db.preparar_para_o_retrato(nome)?;
                    // Pela LEITURA: a gravavel escreve a marca do `.ndx`, e o
                    // acerto logo abaixo o recopiaria inteiro. So a tabela que
                    // pede cura (rara: a cura ja correu antes da fase 1) paga a
                    // gravavel aqui.
                    let (pessoal, n) = match db.ler_para_o_retrato(nome)? {
                        Some(lido) => lido,
                        None => {
                            let mut t = db.abrir_qualificada(nome)?;
                            (t.tem_dado_pessoal(), t.eventos()?)
                        }
                    };
                    // O mesmo portao do `replicar` (pedido 342): coluna marcada
                    // nao atravessa fio em claro, nem pelo retrato. Lido aqui,
                    // com a escrita parada: a marcacao feita durante a fase 1
                    // tambem conta.
                    if pessoal && !self.fio_cifrado(sessao) {
                        return Err(PhxError::Autorizacao(
                            self.msg("erro.replicar_marcada_exige_cifra", &[("tabela", nome)]),
                        ));
                    }
                    lidas.push((nome.clone(), pessoal, n));
                }
                phxsql_store::backup::acertar_fase_2(&mut fase, eventos)?;
                Ok((fase, lidas))
            },
        );
        let ((fase, lidas), com_o_portao_fechado) = match copiado {
            Ok(c) => c,
            Err(e) => {
                // A copia que falha no meio nao fica na raiz: o retrato ainda
                // nao esta na lista, e ninguem mais o apagaria.
                apagar_retratos(&self.config.base, &retrato.prefixo());
                return Err(e);
            }
        };
        retrato.exige_cifra = lidas.iter().any(|(_, p, _)| *p);
        let mut arquivos: Vec<(String, String, PathBuf, u64)> =
            phxsql_store::backup::copias_por_caminho(&fase)
                .into_iter()
                .filter_map(|(rel, copia, bytes)| {
                    let tabela = phxsql_store::catalogo::tabela_do_arquivo_do_retrato(&rel)?;
                    let arquivo = rel.rsplit('/').next().unwrap_or(&rel).to_string();
                    Some((tabela, arquivo, copia, bytes))
                })
                .collect();
        drop(fase);
        // Por tabela, e nao pelo caminho: a replica troca uma tabela por vez
        // e junta os arquivos dela pelos vizinhos da lista (`t#001.log`,
        // `t-a.reg` e `t.log` ficariam separados na ordem do caminho).
        arquivos.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        retrato.arquivos = arquivos;
        let eventos: Vec<(String, Json)> = lidas
            .iter()
            .map(|(nome, _, n)| (nome.clone(), Json::de_u64(*n)))
            .collect();
        let lista = retrato
            .arquivos
            .iter()
            .map(|(t, a, _, b)| {
                Json::objeto(vec![
                    ("tabela", Json::texto_de(t)),
                    ("arquivo", Json::texto_de(a)),
                    ("bytes", Json::de_u64(*b)),
                ])
            })
            .collect();
        let id = retrato.id;
        self.retrato_servido
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(retrato);
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(&database)),
            ("id", Json::de_u64(id)),
            ("arquivos", Json::Lista(lista)),
            ("eventos", Json::Objeto(eventos)),
            // Quanto a escrita esperou por este retrato (pedido 729): o tempo
            // com o portao fechado, a fase 2. Antes era a copia inteira.
            (
                "escrita_parada_ms",
                Json::de_u64(com_o_portao_fechado.as_millis() as u64),
            ),
        ]))
    }

    fn pedaco_do_retrato(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let id = p.inteiro_ou("id", 0).max(0) as u64;
        let indice = p.inteiro_ou("indice", -1);
        let offset = p.inteiro_ou("offset", 0).max(0) as u64;
        let max = match p.inteiro_ou("max", 0) {
            n if n <= 0 => PEDACO_DO_RETRATO,
            n => (n as u64).min(PEDACO_DO_RETRATO),
        };
        self.retrato_desta_sessao(id, p, sessao)?;
        let caminho = {
            let r = self
                .retrato_servido
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(r) = r.iter().find(|r| r.id == id && r.ligacao == sessao.ligacao) else {
                return Err(PhxError::NaoEncontrado(format!(
                    "nao ha retrato {id} servido aqui: peca um novo"
                )));
            };
            // O portao do 342 tambem no pedaco (pedido 727): a conexao pode
            // ter tirado o retrato cifrada, mas cada pedaco e um pedido.
            if r.exige_cifra && !self.fio_cifrado(sessao) {
                let tabela = r.arquivos.first().map(|a| a.0.as_str()).unwrap_or("");
                return Err(PhxError::Autorizacao(
                    self.msg("erro.replicar_marcada_exige_cifra", &[("tabela", tabela)]),
                ));
            }
            let Some((_, _, c, _)) = usize::try_from(indice).ok().and_then(|i| r.arquivos.get(i))
            else {
                return Err(PhxError::NaoEncontrado(format!(
                    "o retrato {id} nao tem o arquivo {indice}"
                )));
            };
            c.clone()
        };
        use std::io::{Read as _, Seek as _, SeekFrom};
        let mut f = std::fs::File::open(&caminho)?;
        f.seek(SeekFrom::Start(offset))?;
        let mut buf = Vec::new();
        f.take(max).read_to_end(&mut buf)?;
        Ok(Json::objeto(vec![
            ("id", Json::de_u64(id)),
            ("offset", Json::de_u64(offset)),
            ("bytes", Json::texto_de(bytes_para_hex(&buf))),
        ]))
    }

    /// A replica ficou atras do que a origem ainda guarda em alguma tabela
    /// deste database? E a pergunta do pedido 706, feita pela BASE que o
    /// `posicao` da origem anuncia -- e nao pela frase da recusa, que se
    /// resolve por chave e nunca por texto.
    pub(super) fn precisa_de_retrato(
        &self,
        database: &str,
        bases: &[(String, u64)],
    ) -> Result<bool> {
        for (tabela, base) in bases {
            if *base > 0 && self.posicao_local(database, tabela)? < *base {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Refaz um database desta replica pelo retrato da origem -- pedido 706.
    ///
    /// A rede vem FORA da trava: cada arquivo chega em pedacos para um
    /// arquivo da raiz, vai ao disco, e so entao, numa tomada so, os arquivos
    /// das tabelas do retrato trocam pelos recebidos. Uma tomada para o
    /// database inteiro, e nao uma por tabela: o retrato foi tirado numa
    /// tomada so la, entre dois commits, e a venda com os itens chega inteira.
    pub(super) fn refazer_por_retrato(
        &self,
        cliente: &mut crate::replica::Cliente,
        database: &str,
        origem: &str,
    ) -> Result<()> {
        let raiz = self.config.base.clone();
        apagar_retratos(&raiz, PREFIXO_RECEBIDO);
        let r = cliente.pedir(vec![
            ("op", Json::texto_de("retrato_da_replica")),
            ("database", Json::texto_de(database)),
        ])?;
        let id = r.inteiro_ou("id", 0);
        let lista = r
            .campo("arquivos")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .to_vec();
        let mut recebidos: Vec<(String, String, PathBuf)> = Vec::new();
        let mut total = 0u64;
        for (i, a) in lista.iter().enumerate() {
            let bytes = a.inteiro_ou("bytes", 0).max(0) as u64;
            let caminho = raiz.join(format!("{PREFIXO_RECEBIDO}{i}"));
            phxsql_store::catalogo::gravar_pedaco_do_retrato(&caminho, 0, &[])?;
            let mut offset = 0u64;
            while offset < bytes {
                let pedaco = cliente.pedir(vec![
                    ("op", Json::texto_de("retrato_da_replica")),
                    ("database", Json::texto_de(database)),
                    ("id", Json::Numero(id as f64)),
                    ("indice", Json::de_u64(i as u64)),
                    ("offset", Json::de_u64(offset)),
                    ("max", Json::de_u64(PEDACO_DO_RETRATO)),
                ])?;
                let b = hex_para_bytes(pedaco.texto_ou("bytes", ""))?;
                if b.is_empty() {
                    return Err(PhxError::Corrompido(format!(
                        "o retrato de {database} acabou no byte {offset} de {bytes} do \
                         arquivo {}",
                        a.texto_ou("arquivo", "")
                    )));
                }
                phxsql_store::catalogo::gravar_pedaco_do_retrato(&caminho, offset, &b)?;
                offset += b.len() as u64;
            }
            phxsql_store::catalogo::sincronizar_arquivo_do_retrato(&caminho)?;
            total += bytes;
            recebidos.push((
                a.texto_ou("tabela", "").to_string(),
                a.texto_ou("arquivo", "").to_string(),
                caminho,
            ));
        }
        let _ = cliente.pedir(vec![
            ("op", Json::texto_de("retrato_da_replica")),
            ("database", Json::texto_de(database)),
            ("soltar", Json::Bool(true)),
        ]);
        // Ordenada antes do `dedup`: com a tabela repetida fora de ordem na
        // lista, a segunda troca apagaria o que a primeira acabou de por.
        let mut tabelas: Vec<String> = recebidos.iter().map(|(t, _, _)| t.clone()).collect();
        tabelas.sort();
        tabelas.dedup();
        let pendente = {
            let dados = self.travar_dados()?;
            // O database que ainda nao existe nasce aqui, e o `fsync` da
            // criacao vai junto do resto, depois de soltar a trava.
            let mut pendente = PorSincronizar::default();
            let db = match dados.abrir_database(database) {
                Ok(db) => db,
                Err(_) => {
                    let (db, criado) = dados.criar_database_com_tipo_adiando_o_fsync(
                        database,
                        phxsql_core::TipoDatabase::Padrao,
                    )?;
                    pendente.juntar(criado);
                    db
                }
            };
            for tabela in &tabelas {
                let novos: Vec<(String, PathBuf)> = recebidos
                    .iter()
                    .filter(|(t, _, _)| t == tabela)
                    .map(|(_, a, c)| (a.clone(), c.clone()))
                    .collect();
                pendente.juntar(db.trocar_pelo_retrato(tabela, &novos)?);
                let chave = Self::chave_do_diario(database, tabela);
                if let Ok(mut m) = self.marcas_do_diario.lock() {
                    m.remove(&chave);
                }
                if let Ok(mut c) = self.continuidade_da_replica.lock() {
                    c.remove(&chave);
                }
                self.anotar_estado(origem, |e| {
                    e.recusas.remove(&chave);
                });
            }
            // Pedido 737: a tabela que chegou pelo retrato nao passou pelo
            // `aplicar_evento`, e a contagem de orfas do 300 §2.7 nao a viu. O
            // retrato e filtrado pelo alcance da replica na origem, entao a
            // filha cuja mae ficou de fora chega orfa -- e e contada aqui,
            // DEPOIS de todas as trocas (a mae pode vir no mesmo retrato, mais
            // adiante na lista). O numero da tabela trocada e o do disco dela
            // agora: o que se contou antes era de outra copia.
            for tabela in &tabelas {
                let chave = format!("{database}/{tabela}");
                if let Ok(mut m) = self.orfas_na_replica.lock() {
                    m.remove(&chave);
                }
                let contadas = db.abrir_qualificada(tabela)?.orfas_no_disco()?;
                self.anotar_orfas(&chave, contadas);
            }
            // A copia residente era da tabela de antes do retrato.
            if let Ok(mut r) = self.residentes.lock() {
                let prefixo = format!("{}/", database.to_lowercase());
                r.retain(|k, _| !k.to_lowercase().starts_with(&prefixo));
            }
            pendente
        };
        pendente.levar_ao_disco()?;
        eprintln!(
            "replicacao [{origem}]: {database} refeito por retrato da origem ({} tabela(s), \
             {} arquivo(s), {total} bytes): o diario que esta replica precisava saiu pelo \
             expurgo la (pedido 706)",
            tabelas.len(),
            recebidos.len()
        );
        Ok(())
    }
}
