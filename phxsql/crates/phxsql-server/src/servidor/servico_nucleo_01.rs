//! O nucleo do `Servidor`: o arranque (`novo`), o papel, as portas, as
//! aberturas travadas (`abrir_travada*`) e a `TomarTrava`. As tomadas da trava
//! global (`travar_dados` e `travar_dados_para_ler`) ficam no `servidor.rs`, cada
//! uma dentro da funcao que lhe da nome: e o que `so_um_lugar_toma_a_trava` exige.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// Esta escrita da lista e da tabela `tabela` do `database`? UM criterio, para
/// a leitura da transacao (`sobreposicao`) e a linha que o `empilhar` ve
/// (`linha_na_transacao`) nunca discordarem sobre de quem e cada escrita.
fn escrita_da_tabela(e: &crate::transacao::Escrita, database: &str, tabela: &str) -> bool {
    e.database.eq_ignore_ascii_case(database) && e.tabela.eq_ignore_ascii_case(tabela)
}

/// O que a transacao DESTA sessao ja pediu em cada tabela, montado so quando o
/// plano do `ao_alterar` do `empilhar` pergunta -- pedido 515.
///
/// # Por que preguicoso
///
/// O `empilhar` abre a tabela sem sobreposicao nenhuma, e a razao e medida
/// (`abrir_travada_sem_sobrepor`): montar o mapa da transacao custava
/// O(pendentes) por instrucao sob a trava global. O plano so abre FILHA quando
/// a chave referenciada mudou e ha irma apontando para ela -- o portao
/// `alguma_coluna_indexada_mudou` sai antes --, entao e so ali que o prefixo
/// da filha e montado, uma vez por filha e por instrucao. A alteracao que nao
/// cascateia continua sem pagar nada.
///
/// A montagem e a da leitura da transacao (`abrir_travada`, que dobra a lista
/// pela `sobreposicao`): o plano ve a filha como o `varrer` desta sessao a
/// veria, e nao uma terceira versao dela.
struct PrefixoDaSessao<'a> {
    servidor: &'a Servidor,
    trava: &'a Instancia,
    database: &'a str,
    /// O schema da MAE: a chave estrangeira nao atravessa diretorio, entao a
    /// filha mora nele.
    schema: Option<String>,
    sessao: &'a Sessao,
    montados: std::cell::RefCell<HashMap<String, Option<Arc<phxsql_store::table::Sobreposicao>>>>,
}

impl phxsql_store::table::MaesEmProgresso for PrefixoDaSessao<'_> {
    /// Nenhum handle emprestado: o plano so PERGUNTA pelo prefixo, e a
    /// conferencia de chave do `empilhar` ja le o disco por conta propria.
    fn mae(&mut self, _tabela_ref: &str) -> Option<&mut Table> {
        None
    }

    fn prefixo(&self, tabela: &str) -> Option<Arc<phxsql_store::table::Sobreposicao>> {
        let chave = tabela.to_ascii_lowercase();
        if let Some(s) = self.montados.borrow().get(&chave) {
            return s.clone();
        }
        let qualificada = phxsql_store::catalogo::qualificar(self.schema.as_deref(), tabela);
        let ped = pedido_da_tabela(self.database, &qualificada);
        // Tabela que nao abre nao tem prefixo: o plano abre a mesma filha logo
        // depois, e e ele quem diz o erro dela.
        let s = self
            .servidor
            .abrir_travada(self.trava, &ped, self.sessao)
            .ok()
            .and_then(|t| t.sobreposicao().cloned());
        self.montados.borrow_mut().insert(chave, s.clone());
        s
    }
}

/// A escrita da transacao na lingua do `store`. UM tradutor, para a leitura
/// da transacao e a pre-conferencia do COMMIT dobrarem a lista do mesmo jeito.
pub(crate) fn pendente_de(e: &crate::transacao::Escrita) -> phxsql_store::table::Pendente<'_> {
    use crate::transacao::Acao;
    use phxsql_store::table::Pendente;
    match e.acao {
        Acao::Inserir => Pendente::Insercao(&e.linha),
        Acao::Atualizar => Pendente::Alteracao(&e.linha, &e.linha_antiga),
        Acao::ExcluirSuave => Pendente::Marca(true),
        Acao::Restaurar => Pendente::Marca(false),
        Acao::ExcluirDeVez => Pendente::Exclusao,
    }
}

/// As colunas de `base.tabela` como estao no disco -- `None` quando a base,
/// o schema ou a tabela ainda nao existem.
///
/// E o resolvedor que `Cadastro::conferir_colunas` recebe, nos dois lugares
/// que a chamam: o arranque e a porta das tres operacoes de cadastro. Abre a
/// tabela pela ficha EXCLUSIVA de proposito: a conferencia acontece uma vez
/// por carga de cadastro, sobre as poucas tabelas que o cadastro cita, e a
/// exclusiva e a unica que termina uma troca de volume interrompida em vez de
/// devolver «precisa da ficha exclusiva» -- e no arranque ainda nao ha com
/// quem disputar.
pub(super) fn colunas_em_disco(
    dados: &Instancia,
    base: &str,
    tabela: &str,
) -> Result<Option<Vec<String>>> {
    let db = match dados.abrir_database(base) {
        Ok(db) => db,
        Err(PhxError::NaoEncontrado(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    let (schema, nome) = phxsql_store::catalogo::separar_qualificado(tabela);
    if !db.diretorio(schema.as_deref())?.is_dir() || !db.existe_tabela(schema.as_deref(), &nome)? {
        return Ok(None);
    }
    let t = db.abrir_tabela(schema.as_deref(), &nome)?;
    Ok(Some(
        t.esquema()
            .colunas()
            .iter()
            .map(|c| c.nome.clone())
            .collect(),
    ))
}

impl Servidor {
    pub fn novo(config: Config) -> Result<Arc<Servidor>> {
        // O lancador do `df` nasce ANTES da primeira trava de instancia, que a
        // recuperacao abaixo ja toma: filho criado depois herdaria uma copia
        // dela e, orfao de um `SIGKILL`, a seguraria (pedido 758).
        #[cfg(unix)]
        crate::sistema::Lancador::preparar();
        // O irmao do gancho e do firewall (pedido 759), pelo mesmo motivo. So
        // quando um deles esta ligado: desligado, nenhum processo a mais.
        #[cfg(unix)]
        if config.alertas.gancho.ligado
            || config.politica.firewall.as_ref().is_some_and(|f| f.ligado)
        {
            crate::gancho::lancador::preparar();
        }
        // Antes de tudo, e antes da recuperacao abaixo -- que tambem
        // sincroniza: o `fsync` recusado derruba o processo (pedido 509).
        phxsql_store::sincronia::ao_recusar(fsync_recusado_derruba_o_processo);
        // E o arranque que vem DEPOIS de uma queda dessas, no mesmo boot, nao
        // sobe (pedido 509, saida (a), decisao do dono em 30/09/2026).
        // Pedido 573: o operador e AVISADO antes da recusa, pelo carteiro do
        // 249 -- o `fsync` recusado derrubou o processo e o carteiro morreu
        // junto, entao quem diz e o arranque seguinte.
        let fsync_de_boot_anterior = conferir_sentinela_509(&config.base, |caminho, erro| {
            avisar_o_arranque_recusado(&config, caminho, erro)
        })?;
        registrar_base_da_sentinela(&config.base);
        // `recursos.cache_paginas` estava no config.json e na documentacao
        // desde a 0.13.0 -- e nao era lido por ninguem, porque o cache nao
        // existia. Agora existe, e o campo passa a valer. Tem de ser aqui,
        // ANTES de a primeira tabela abrir: o teto vale para o que abrir
        // daqui para a frente.
        phxsql_store::ndx::definir_cache_paginas(config.recursos.cache_paginas);
        // A faixa da `Sequence` deste no (pedido 290), e pelo mesmo motivo de
        // morar aqui: ANTES de a primeira tabela abrir -- a recuperacao logo
        // abaixo ja abre --, porque a abertura confere a faixa gravada contra
        // esta. E DEFINE sempre, inclusive o zero de quem nao escreveu o
        // campo: herdar a faixa de um arranque anterior no mesmo processo
        // poria este servidor na faixa de outro, calado.
        phxsql_store::no::definir_inicio_da_sequencia(config.replicacao.inicio_da_sequencia);
        // Copiados ANTES de o `config` entrar no struct, que o consome.
        let (max_linhas, somente_leitura, espelho) =
            (config.max_linhas, config.somente_leitura, config.espelho);
        let mut raiz = Raiz::nova(&config.base)?;
        // A imagem no diario e decisao do servidor, e entra AQUI, antes da
        // recuperacao logo abaixo -- que era quem a esquecia (pedido 564).
        raiz.definir_politica_do_diario(politica_do_diario(&config));
        // Pedido 731: a copia de retrato que uma vida anterior deixou na raiz
        // sai aqui, antes de a porta abrir -- e antes da migracao do separador
        // logo abaixo, que senao renomearia volumes dentro da arvore de uma
        // copia que vai para o lixo (pedido 729: o retrato servido e pasta).
        // Um segundo processo na MESMA base apagaria o retrato vivo do
        // primeiro -- mas dois processos numa base
        // ja sao o erro que a trava de instancia de cada database recusa
        // (pedido 635), e o retrato do primeiro so serve a um database dele.
        servico_diario_01::limpar_retratos_no_arranque(&config.base);
        // O separador de volume do binario anterior (`_`) vira `#` AQUI, uma
        // vez, antes da primeira operacao e fora de qualquer trava: a abertura
        // de database, que roda dentro das secoes, so confere a marca (pedido
        // 508, `separador::exigir_migrado`). Database que recusa a migracao
        // nao impede a subida; ele recusa na abertura, com o motivo.
        let (migrados, recusados) = phxsql_store::separador::migrar_base(&config.base);
        if migrados > 0 {
            eprintln!(
                "pedido 508: {migrados} arquivo(s) de volume renomeados do separador `_` para `#`"
            );
        }
        for r in recusados {
            eprintln!("AVISO: a migracao do separador de volume (pedido 508) recusou {r}");
        }
        // A base que JA EXISTIA com permissao larga (pedido 542): o banco
        // cria tudo 0600/0700 desde entao, mas nao aperta o que achou -- a
        // regua do J nao recusa (PG 4 contra 6) e apertar calado tiraria o
        // acesso de quem le por grupo. Sobra dizer, uma vez, no arranque.
        if let Some(aviso) = phxsql_store::permissao::permissao_larga(&config.base) {
            eprintln!("AVISO: {aviso}");
        }
        // A RECUPERACAO, e ela vem antes de tudo o mais.
        //
        // Um `transacao_<id>.tx` orfao quer dizer uma coisa so: alguem morreu
        // no meio de um `COMMIT`. Completa-lo aqui, com o servidor ainda
        // fechado para o mundo, e o que faz a resposta a pergunta do contrato
        // ser inequivoca -- «depois de reiniciar, o banco consegue determinar
        // se esta transacao foi COMMITTED ou ABORTED?». Deixar isso para o
        // primeiro pedido seria responder «depende de quem chegar primeiro».
        //
        // Silencio quando nao ha marca nenhuma, que e o arranque de sempre: um
        // bloco de relatorio dizendo zero em toda subida treina quem opera a
        // nao ler o relatorio.
        let recuperacao = crate::transacao::recuperar(&raiz.exclusiva());
        if recuperacao.houve() || recuperacao.impede_subir() {
            eprintln!("{}", recuperacao.texto(&config.base));
        }
        if recuperacao.impede_subir() {
            return Err(PhxError::Io(std::io::Error::other(format!(
                "o servidor NAO subiu: {} marca(s) de commit nao se leram por erro \
                 de E/S, e nenhuma foi apagada -- {}",
                recuperacao.sem_leitura.len(),
                recuperacao.sem_leitura.join("; ")
            ))));
        }
        // O cadastro contra o esquema -- pedido 235 -- com as tabelas em
        // disco e ANTES de a porta abrir. `Config::ler` carregou o cadastro
        // sem tabela nenhuma, porque o `--usuarios` le o mesmo arquivo sem
        // subir servidor; aqui a raiz existe e ja passou pela recuperacao, e
        // a regra de coluna que cita coluna que a tabela nao tem recusa o
        // arranque nomeando-a. A que cita tabela que ainda nao existe entra
        // com aviso, e o aviso vai para a lista do cadastro (a mesma que o
        // `main` imprime) e para o log daqui, porque o `main` ja imprimiu a
        // lista dele antes de chegar aqui. Ver `Cadastro::conferir_colunas`.
        let mut config = config;
        // Pedido 714: o contador nasce ACIMA de toda marca que ficou no disco
        // -- a retida pela recuperacao, de qualquer familia --, e nao so do
        // relogio, que pode ter recuado desde a vida que a gravou.
        let semente_das_marcas = crate::transacao::semente_do_contador(&raiz.exclusiva());
        {
            let dados = raiz.exclusiva();
            let avisos = config
                .cadastro
                .conferir_colunas(&mut |base, tabela| colunas_em_disco(&dados, base, tabela))?;
            for aviso in &avisos {
                eprintln!("AVISO: {aviso}");
            }
            config.cadastro.avisos.extend(avisos);
        }
        let mut log = LogAcessos::abrir(&config.log_acessos)?;
        // O rodizio do `acessos.log` -- pedido 228, mesma logica do
        // Profiler (`crate::rodizio`). Zero/ausente e o padrao, e continua
        // sem girar, como sempre.
        log.definir_rodizio(config.acessos.teto_do_arquivo(), config.acessos.arquivos);
        let lista_negra = Blacklist::abrir(&config.blacklist)?;
        // A memoria de IPs (pedido 765, P6), ao lado do `acessos.log`. A
        // semente le o `acessos.log` so no arranque em que ela ainda nao
        // existe. Falhar AVISA e sobe com a memoria vazia e sem arquivo: ela
        // e aviso e guarda, e um banco que nao sobe por causa dela trocaria
        // o dado pelo aviso.
        let ips_vistos = {
            let caminho = config
                .log_acessos
                .with_file_name(crate::ips_vistos::NOME_DO_ARQUIVO);
            let do_log = config.log_acessos.clone();
            crate::ips_vistos::IpsVistos::abrir(&caminho, || {
                LogAcessos::ler(&do_log).unwrap_or_default()
            })
            .unwrap_or_else(|e| {
                eprintln!(
                    "AVISO: a memoria de IPs ({}) nao abriu: {e}",
                    caminho.display()
                );
                crate::ips_vistos::IpsVistos::default()
            })
        };
        // O perfil habitual (pedido 765, P7), ao lado da memoria de IPs e
        // pelo mesmo motivo de nao derrubar o arranque: e aviso.
        let perfis = {
            let caminho = config
                .log_acessos
                .with_file_name(crate::perfis::NOME_DO_ARQUIVO);
            crate::perfis::Perfis::abrir(&caminho, crate::agora_ms()).unwrap_or_else(|e| {
                eprintln!(
                    "AVISO: o perfil habitual ({}) nao abriu: {e}",
                    caminho.display()
                );
                crate::perfis::Perfis::default()
            })
        };
        // Com a chave mestra do cadastro (pedido 372). A chave que falta, ou
        // a errada, NAO recusa aqui: tranca so as ligacoes cifradas, e o aviso
        // ja saiu pela lista do `Config::ler`. E o arquivo torto, o formato
        // mais novo que este binario e o arquivo que nao se le tambem nao
        // recusam mais (pedido 466): TRANCAM o DbLink inteiro, e o motor sobe
        // -- o DbLink e acessorio. O aviso e o do mesmo `avisos()`.
        let dblink =
            crate::dblink::Registro::abrir_ou_trancar(&config.dblink, &config.cifra_do_dblink);
        // O irmao do DbLink no pedido 466: a MESMA sequencia de aberturas, e o
        // `jobs.json` torto derrubava o motor pelo mesmo `?`. Trancado, o
        // motor sobe e as operacoes de job recusam dizendo o motivo.
        let mut jobs = crate::jobs::Registro::abrir_ou_trancar(&config.jobs);
        if let Some(motivo) = jobs.trancado() {
            eprintln!("AVISO: {motivo}");
        }
        // A corrida que derrubou o processo anterior (pedido 502): vira
        // FALHOU no historico e conta como a ultima, e o relogio nao a roda de
        // novo logo na partida. O e-mail sai depois, no `subir_jobs`: aqui o
        // servidor ainda nao existe para subir a thread do aviso.
        let interrompidas = jobs.fechar_interrompidas(crate::agora_ms());
        for c in &interrompidas {
            eprintln!("AVISO: job {}: {}", c.job, c.detalhe);
        }
        let cluster = config.cluster.clone().map(|c| {
            Arc::new(crate::cluster::EstadoCluster::novo(
                c,
                &config.base,
                config.replicacao.papel,
            ))
        });
        let quorum = config.cluster.as_ref().map(|c| {
            Arc::new(crate::quorum::Cubo::novo(
                c.quorum_minimo,
                c.quorum_prazo_ms,
                Duration::from_secs(c.pulso_s),
            ))
        });
        let rotinas = crate::rotinas::Rotinas::carregar(&config.base)?;
        let ha_gatilhos = AtomicBool::new(rotinas.ha_gatilhos());
        let visoes = crate::visoes::Visoes::carregar(&config.base)?;
        let ha_visoes = AtomicBool::new(visoes.ha_visoes());
        let mensagens = Mensagens::nova(&config.idioma, &config.base);
        let papel = config.replicacao.papel;
        let posicoes_bidi =
            bidirecional::ler_posicoes(&config.base.join("replicacao-posicoes.json"));
        let numeros_bidi = bidirecional::ler_numeros(&config.base.join("replicacao-numeros.json"));
        // Copiado ANTES de o `config` entrar no struct, que o consome -- pela
        // mesma razao que `max_linhas` e os outros dois acima.
        let cadastro_de_arranque = config.cadastro.clone();
        let proibidos_por_base = config.politica.proibidos_por_base.clone();
        let log_diretivas = config.log_acessos.clone();
        // Os tetos de thread nascem aqui, uma vez: o semaforo carrega o
        // numero, e o laco de aceitacao so pede vaga. `conexoes_max` ja
        // chega >= 1 do `Config`; o da web aceita zero, que e «sem teto».
        let permissoes_de_dados = Semaforo::novo(config.conexoes_max);
        // A saude do disco nasce com o caminho do `base`, que e o disco que
        // interessa: e nele que todo `.reg` e todo `.ndx` moram.
        // O segredo do sal falso, uma vez por subida. Em cluster, o que veio do
        // ARQUIVO LOCAL da um sal falso diferente em cada no, e perguntar o
        // mesmo login a dois nos separa quem existe: isso se diz alto.
        let (segredo_do_desafio, do_arquivo_local, avisos) =
            config.desafio.segredo(config.caminho.as_deref())?;
        for a in avisos {
            eprintln!("AVISO: {a}");
        }
        if do_arquivo_local && config.cluster.is_some() {
            eprintln!(
                "AVISO: desafio: este no esta em cluster e o segredo do sal falso veio \
                 do arquivo local -- cada no tera o seu, e o mesmo login perguntado a \
                 dois nos separa quem existe. Declare `desafio.segredo` (ou \
                 `desafio.segredo_env`) IGUAL em todos os nos"
            );
        }
        let saude = Arc::new(crate::saude_do_disco::SaudeDoDisco::nova(
            config.alertas.disco.clone(),
            &config.base,
        ));
        // Pedido 255: o indice reconstruido no arranque tambem sai pelo
        // carteiro, e nao so no log -- quem opera le e-mail, nao `stderr`.
        let evento_de_indice = crate::saude_do_disco::evento_do_arranque(
            crate::agora_ms(),
            recuperacao.indices_reconstruidos,
            &recuperacao.indices_pendentes,
        );
        // O alarme do indice atrasado (pedido 769) sai da MESMA decisao do
        // aviso: o texto do evento, ou nada. Duas condicoes escritas em dois
        // lugares avisariam por e-mail o que o aquario nao mostra.
        let indice_atrasado = evento_de_indice.as_ref().map(|e| e.texto.clone());
        if let Some(evento) = evento_de_indice {
            saude.entregar(evento);
        }
        // Pedido 339, condicao A do papel C: a arvore em claro sobre coluna
        // marcada se AVISA, e nao se converte sozinha.
        let em_claro = crate::saude_do_disco::evento_dos_indices_em_claro(
            crate::agora_ms(),
            &recuperacao.indices_em_claro,
        );
        if let Some(evento) = em_claro {
            saude.entregar(evento);
        }
        // A camada de ocorrencias (495, F2) sobre o correio da saude: o
        // carteiro e um so, e a fila tambem.
        let ocorrencias = Arc::new(crate::ocorrencias::Ocorrencias::nova(Arc::clone(
            saude.correio(),
        )));
        let permissoes_http = match config.recursos.conexoes_web_max {
            0 => Semaforo::sem_teto(),
            teto => Semaforo::novo(teto),
        };
        let servidor = Arc::new(Servidor {
            cluster,
            quorum,
            mensagens,
            papel_vivo: AtomicU8::new(papel_para_u8(papel)),
            somente_leitura_vivo: AtomicBool::new(somente_leitura),
            ha_proibidos_por_base: AtomicBool::new(!proibidos_por_base.is_empty()),
            proibidos_por_base: Mutex::new(proibidos_por_base),
            diario: crate::diretivas::Diario::ao_lado_de(&log_diretivas),
            senhas_de_execucao: crate::senha_de_execucao::Cofre::ao_lado_de(&log_diretivas),
            estado_replicacao: Mutex::new(HashMap::new()),
            dono_do_database: Mutex::new(HashMap::new()),
            ha_varias_origens: AtomicBool::new(false),
            toques_bidi: Mutex::new(HashMap::new()),
            orfas_na_replica: Mutex::new(HashMap::new()),
            escritas_locais_na_replica: Mutex::new(HashMap::new()),
            posicoes_bidi: Mutex::new(posicoes_bidi),
            numeros_bidi: Mutex::new(numeros_bidi),
            ledger_marcado_recebido: AtomicU64::new(0),
            aviso_do_noise: crate::fio_dados::AvisoDoNoise::default(),
            janela: Janela::nova(&config.recursos),
            sujas: Mutex::new(std::collections::HashSet::new()),
            trava_das_sequencias: Mutex::new(()),
            config,
            dados: RwLock::new(raiz),
            retrato: crate::retrato::PortaoDoRetrato::default(),
            panicos_na_trava: AtomicU64::new(0),
            reparos_da_trava: AtomicU64::new(0),
            log: Mutex::new(log),
            lista_negra: Mutex::new(lista_negra),
            ips_vistos: Mutex::new(ips_vistos),
            perfis: Mutex::new(perfis),
            sessoes: Mutex::new(http::Sessoes::default()),
            residentes: Mutex::new(HashMap::new()),
            remotos: Mutex::new(HashMap::new()),
            ligacoes: Mutex::new(crate::ligacoes::Ligacoes::default()),
            desde_ms: crate::agora_ms(),
            monitor: Mutex::new(crate::sistema::Monitor::novo()),
            dblink: Mutex::new(dblink),
            jobs: Mutex::new(jobs),
            relogio_de_jobs: AtomicBool::new(false),
            jobs_rodando: Mutex::new(Vec::new()),
            interrompidas_a_avisar: Mutex::new(interrompidas),
            avisos_de_jobs: Mutex::new(HashMap::new()),
            expurgo_da_trilha: Mutex::new(()),
            expurgo_do_diario: Mutex::new(()),
            confirmados_do_diario: Mutex::new(HashMap::new()),
            retrato_servido: Mutex::new(Vec::new()),
            avisos_de_seguranca: Mutex::new(HashMap::new()),
            porta_no_ar: AtomicBool::new(false),
            parar_de_aceitar: AtomicBool::new(false),
            proximo_ouvinte: Mutex::new(None),
            endereco_dos_dados: Mutex::new(None),
            endereco_web: Mutex::new(None),
            endereco_rest: Mutex::new(None),
            endereco_swagger: Mutex::new(None),
            avisados: Mutex::new(HashMap::new()),
            previsor: Mutex::new(crate::previsao::Previsor::default()),
            backup_marcas: crate::previsao::MarcasDoBackup::default(),
            saude,
            ocorrencias,
            segredo_do_desafio,
            permissoes_de_dados,
            permissoes_http,
            http_cheia_ate_ms: AtomicU64::new(0),
            #[cfg(test)]
            panicos_de_teste: AtomicUsize::new(0),
            #[cfg(test)]
            aceitas_de_teste: AtomicUsize::new(0),
            #[cfg(test)]
            sem_vaga_de_teste: AtomicUsize::new(0),
            #[cfg(test)]
            passada_quebra_na_escrita: AtomicUsize::new(0),
            #[cfg(test)]
            passada_quebra_congelando: Mutex::new(None),
            #[cfg(test)]
            passada_quebra_congela: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            na_janela_sem_trava: Mutex::new(None),
            #[cfg(test)]
            na_janela_da_criacao: Mutex::new(None),
            #[cfg(test)]
            panico_de_teste_na_op: Mutex::new(None),
            #[cfg(test)]
            reparo_falha_de_teste: AtomicBool::new(false),
            #[cfg(test)]
            panicos_no_pulso_de_teste: std::sync::atomic::AtomicU32::new(0),
            #[cfg(test)]
            panicos_no_pulso_dados: std::sync::atomic::AtomicU32::new(0),
            #[cfg(test)]
            panico_no_relogio_de_jobs_de_teste: AtomicBool::new(false),
            #[cfg(test)]
            panico_no_amostrador_de_teste: AtomicBool::new(false),
            #[cfg(test)]
            reparo_panica_de_teste: AtomicBool::new(false),
            #[cfg(test)]
            marca_ilegivel_de_teste: AtomicBool::new(false),
            #[cfg(test)]
            panico_no_fecho_de_teste: Mutex::new(None),
            #[cfg(test)]
            fecho_falha_de_teste: Mutex::new(None),
            #[cfg(test)]
            pre_conferencia_desligada: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            solto_sem_trava_de_linha: std::sync::atomic::AtomicBool::new(false),
            cargas: Mutex::new(crate::carga::Cargas::default()),
            marcas_pendentes: Mutex::new(Vec::new()),
            // Pedido 458: as duas travas da vida de uma transacao nao tem
            // disco atras, e um panico com uma delas na mao nao pode matar
            // toda transacao de toda conexao ate reiniciar. O registro das
            // transacoes passa pelo saneamento (as ativas vao para
            // ABORT_ONLY); o das travas nao precisa, porque cada entrada e
            // de uma transacao e sai com ela.
            travas: crate::pulso::TravaDaGuarda::nova(
                "das travas de transacao",
                crate::travas::Travas::default(),
            ),
            transacoes: crate::pulso::TravaDaGuarda::nova_saneada(
                "das transacoes abertas",
                crate::transacao::Transacoes::nova(semente_das_marcas),
                crate::transacao::Transacoes::abortar_abertas,
            ),
            transacoes_abertas: AtomicUsize::new(0),
            marcas_do_diario: Mutex::new(HashMap::new()),
            continuidade_da_replica: Mutex::new(HashMap::new()),
            profiler: Mutex::new(crate::profiler::Profiler::default()),
            profiler_ligado: AtomicBool::new(false),
            rotinas: Mutex::new(rotinas),
            ha_gatilhos,
            visoes: Mutex::new(visoes),
            cadastro_no_disco: Mutex::new(()),
            ha_visoes,
            max_linhas_vivo: AtomicU64::new(max_linhas),
            espelho_vivo: AtomicBool::new(espelho),
            estatica_do_fio: Mutex::new(None),
            tls_dos_dados: Mutex::new(None),
            telemetria: Arc::new(crate::telemetria::Telemetria::default()),
            cadastro_vivo: RwLock::new(cadastro_de_arranque),
            cadastro_geracao: AtomicU64::new(0),
        });
        // **O leitor do bloco `telemetria`.** Sem esta linha o `config.json`
        // teria quatro cores e dois limiares que ninguem le -- exatamente o
        // `cache_paginas` que passou tres versoes prometendo um cache que nao
        // existia. O teste que cai com ela removida e
        // `cor_do_config_chega_no_retrato`.
        servidor
            .telemetria
            .definir_pintura(servidor.config.telemetria.clone());
        // O leitor do bloco `aquario` (pedido 783), pelo mesmo motivo: perfil
        // gravado que ninguem le seria configuracao que mente.
        servidor
            .telemetria
            .definir_perfil_do_aquario(&servidor.config.aquario);
        // O `aquario.log` (pedido 707, A6), ao lado do `acessos.log`: grava
        // sem ninguem perguntar, entao abre aqui e nao na primeira consulta.
        // Falhar AVISA e sobe -- o aquario e acessorio, e a consulta devolve o
        // motivo a quem olhar a tela.
        let caminho_do_aquario = servidor
            .config
            .log_acessos
            .with_file_name(crate::aquario::log::NOME_DO_ARQUIVO);
        if let Err(e) = servidor
            .telemetria
            .aquario()
            .log()
            .abrir(&caminho_do_aquario)
        {
            eprintln!(
                "AVISO: o aquario.log nao abriu ({}): {e}",
                caminho_do_aquario.display()
            );
        }
        // O `ocorrencias.log` (pedido 495, F2), ao lado do `acessos.log`, e a
        // camada amarrada a telemetria (o alarme da TAREFA vai ao servidor
        // dela) e ao processo (o que nenhuma tarefa carrega). Falhar AVISA e
        // sobe, como o aquario: a ocorrencia continua contada e entregue, so
        // nao chega ao disco -- e a falha de abrir fica para a consulta.
        let caminho_das_ocorrencias = servidor
            .config
            .log_acessos
            .with_file_name(crate::ocorrencias::NOME_DO_ARQUIVO);
        if let Err(e) = servidor.ocorrencias.abrir(&caminho_das_ocorrencias) {
            eprintln!(
                "AVISO: o ocorrencias.log nao abriu ({}): {e}",
                caminho_das_ocorrencias.display()
            );
        }
        servidor
            .telemetria
            .definir_ocorrencias(&servidor.ocorrencias);
        crate::ocorrencias::instalar(&servidor.ocorrencias);
        // Os alarmes do arranque (pedido 769) saem SO agora, com a camada de
        // ocorrencias deste servidor instalada: o fato aconteceu antes (na
        // sentinela, na recuperacao), mas sinalizado la a ocorrencia iria a
        // camada de outro servidor do processo, ou a nenhuma.
        servidor.sinalizar_o_arranque(
            fsync_de_boot_anterior.as_deref(),
            &recuperacao,
            indice_atrasado.as_deref(),
        );
        // As horas fechadas da contagem do aquario (pedido 707, A8), ao lado
        // do `acessos.log`, fora do rodizio.
        servidor.telemetria.aquario().contagem().definir_arquivo(
            crate::aquario::contagem::arquivo_ao_lado_de(&servidor.config.log_acessos),
        );
        // E so agora, com os dois arquivos definidos, a hora corrente se
        // refaz do que o processo anterior deixou no `aquario.log`.
        servidor.retomar_a_contagem(crate::agora_ms());
        // O rodizio do `diretivas.log` -- pedido 228, mesmo motivo do
        // `acessos.log` acima. `Diario::definir_rodizio` toma `&self` (o
        // diario nao guarda descritor entre chamadas), entao isto pode
        // esperar o `Arc<Servidor>` existir em vez de brigar com o `config`
        // sendo movido para dentro do struct.
        servidor.diario.definir_rodizio(
            servidor.config.diretivas.teto_do_arquivo(),
            servidor.config.diretivas.arquivos,
        );
        // Pedido 698: o grupo do bidirecional que a queda partiu se completa
        // AQUI, antes de a porta abrir -- o irmao do `recuperar` la de cima,
        // que precisa do servidor de pe (o mapa de toques e dele).
        servidor.completar_marcas_do_bidi();
        // Pedido 766, P11: o firewall do SO reconciliado com a lista -- o
        // bloqueio que o reboot tirou do conjunto volta com o prazo que
        // falta, e a regra que ficou de um processo morto sai. Numa thread
        // propria e so com o `listar` configurado: os comandos tem prazo, e
        // a porta nao espera por eles (o servidor ja barra pela lista).
        if servidor
            .config
            .politica
            .firewall
            .as_ref()
            .is_some_and(|f| f.ligado && !f.listar.is_empty())
        {
            let s = Arc::clone(&servidor);
            servidor.telemetria.subir(
                "reconciliar-firewall",
                "reconcilia UMA vez o firewall do SO com a lista de bloqueio, no \
                 arranque, e sai; em thread propria porque cada comando tem o \
                 prazo do firewall e a porta nao espera por eles",
                "servico",
                crate::agora_ms(),
                move |fio| {
                    fio.fazendo("reconciliando o firewall do SO com a lista");
                    let _ = s.reconciliar_o_firewall();
                },
            );
        }
        // Quem configurou "idioma" pediu o recurso: a tabela de mensagens e
        // semeada no arranque se ainda nao existe. Sem o campo, nada e criado
        // -- guarda nova entra pedida, nao imposta.
        // A tabela da protecao (765/767, P12) e COMPLETADA no arranque de
        // quem ja tem o database de sistema: a op nova da fabrica ganha a
        // linha dela no upgrade. Quem nunca pediu `phxsys` nao o ve nascer
        // -- a tabela ausente ja vale `proteger` em tudo, e e o
        // `protecao_semear` que a cria. A replica e o bidirecional nao
        // semeiam: a linha deles chega pela replicacao da origem, e uma
        // semeadura local daria a mesma `op` com outro `id` dos dois lados.
        let semeia_protecao = servidor.config.protecao.ligada
            && !servidor.config.somente_leitura
            && matches!(servidor.papel_atual(), Papel::Isolado | Papel::Source);
        let semeia_mensagens = !servidor.config.idioma.is_empty();
        let (mensagens, protecao) = if semeia_mensagens || semeia_protecao {
            servidor.semear_o_sistema(semeia_mensagens, semeia_protecao, false)
        } else {
            (Ok((false, false, 0, 0)), Ok(0))
        };
        match protecao {
            Ok(n) if n > 0 => eprintln!(
                "protecao: tabela {}.{} semeada ({n} comandos em proteger)",
                crate::protecao::DATABASE,
                crate::protecao::TABELA
            ),
            Ok(_) => {}
            Err(e) => eprintln!("AVISO: nao consegui semear a tabela de protecao: {e}"),
        }
        if semeia_mensagens {
            match mensagens {
                Ok((db_novo, tab_nova, semeadas, _)) if tab_nova || semeadas > 0 => eprintln!(
                    "mensagens: tabela {}.{} semeada ({} mensagens{})",
                    crate::mensagens::DATABASE,
                    crate::mensagens::TABELA,
                    semeadas,
                    if db_novo { ", database criado" } else { "" }
                ),
                Ok(_) => {}
                Err(e) => eprintln!("AVISO: nao consegui semear a tabela de mensagens: {e}"),
            }
        }
        Ok(servidor)
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// O cadastro VIVO. Sempre por aqui -- nunca `self.config.cadastro`.
    ///
    /// A trava envenenada nao derruba a leitura, e a escolha e deliberada: a
    /// escrita e uma atribuicao unica (`*guarda = novo`), entao nao existe
    /// «cadastro pela metade» para se herdar de uma thread que morreu. Negar
    /// a leitura aqui deixaria o servidor sem autenticar ninguem por causa de
    /// um panico em outro lugar.
    pub(super) fn cadastro(&self) -> std::sync::RwLockReadGuard<'_, crate::usuarios::Cadastro> {
        self.cadastro_vivo
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// O papel VIVO deste processo -- o do `config.json`, ate uma promocao.
    /// A replicacao deste servidor esta ABERTA a quem tiver o token?
    ///
    /// `replicacao.replicas_autorizadas` vazia -- que e o padrao de fabrica e
    /// o que todo `config.json` de hoje tem -- libera as tres operacoes de
    /// replicacao para qualquer IP. Isso continua assim de proposito (*guarda
    /// nova entra pedida, nao imposta*: passar a recusar quebraria toda
    /// replicacao montada sem a lista, que e a maioria). O que muda e o
    /// SILENCIO: o servidor passa a dizer no arranque e na resposta de
    /// `config` que a porta esta aberta.
    ///
    /// # Por que o aviso NAO depende do papel
    ///
    /// Porque as tres ops respondem em qualquer papel -- medido pela bateria
    /// `bancada/seguranca/porta.py`, caso 4d-i: um servidor de FABRICA (papel
    /// isolado, imagem desligada) devolveu o diario a quem so tinha o token.
    /// Condicionar o aviso ao papel `source` calaria justamente o servidor que
    /// ninguem configurou para replicar e que replica assim mesmo.
    ///
    /// O `bool` de dentro diz ate onde vai a exposicao: com
    /// `imagem_da_linha` ligada o `replicar` entrega a LINHA INTEIRA em
    /// hexadecimal (248 caracteres na medicao); sem ela, entrega o metadado do
    /// diario -- e o `aplicar` grava do mesmo jeito nos dois casos.
    pub fn replicacao_aberta(&self) -> Option<bool> {
        if self.config.replicacao.replicas_autorizadas.is_empty() {
            Some(self.config.replicacao.imagem_da_linha)
        } else {
            None
        }
    }

    pub fn papel_atual(&self) -> Papel {
        u8_para_papel(self.papel_vivo.load(Ordering::Relaxed))
    }

    /// A origem que traz `base` por replicacao e a declarou ESPELHO -- pedido
    /// 677, «um escritor por database».
    ///
    /// QUAIS databases continua saindo de `replicacao.origens[].databases`, o
    /// mesmo que o laco le para saber o que puxar e a guarda do 406 le para
    /// saber de quem e cada nome: uma segunda lista («bases so leitura") seria
    /// a mesma pergunta em dois lugares, e o dia em que divergissem a replica
    /// puxaria o que a aplicacao tambem escreve. O `espelho` nao e lista, e o
    /// interruptor POR ORIGEM que diz se a lista dela tranca a escrita local.
    ///
    /// E a guarda entrando pedida, nao imposta: ligada por omissao ela
    /// recusou de um dia para o outro arranjo que ja escrevia num database
    /// recebido (`tests/trava-atras-da-rede.rs`). Sem `"espelho": true` nada
    /// muda; lista vazia («o que vier») nao reivindica nome nenhum mesmo com
    /// ele, e segue com o aviso do arranque e a conta do pedido 300 (4). O multi fica
    /// fora (existe para ser escrito, e casa pela chave), e o cluster tambem
    /// (quem recusa ali e o papel vivo dele, no mesmo portao, antes).
    pub(super) fn origem_que_traz(&self, base: &str) -> Option<&crate::config::Origem> {
        let base = base.trim();
        if base.is_empty() || self.cluster.is_some() {
            return None;
        }
        let papel = self.papel_atual();
        if !papel.puxa_de_origem() || papel == Papel::Multi {
            return None;
        }
        self.config
            .replicacao
            .origens
            .iter()
            .find(|o| o.espelho && o.databases.iter().any(|d| d == base))
    }

    /// O primeiro endereco de origem, para o erro de recusa apontar o lugar
    /// certo em vez de so dizer "nao".
    pub(super) fn primario(&self) -> String {
        match self.config.replicacao.origens.first() {
            Some(o) => format!("{}:{} ({})", o.host, o.porta, o.nome),
            None => "(nenhuma origem configurada)".into(),
        }
    }

    /// Promove ESTE servidor a primario: o laco de replica para, a escrita
    /// abre, e o papel vira `source`.
    ///
    /// # Coesa, com `motivo`, e chamavel por dentro
    ///
    /// A op `spare_promover` e uma casca fina sobre esta funcao -- ela so
    /// escolhe o texto do `motivo`. O parametro existe para que TODA promocao
    /// diga por que aconteceu: «pedido manual» e uma coisa, «o primario nao
    /// respondeu» e outra, e quem le o log depois precisa distinguir.
    ///
    /// A assinatura e a mesma que a promocao AUTOMATICA usa do outro lado
    /// (`promover_a_master(&self, motivo: &str) -> Result<Json>`), de
    /// proposito: o degrau manual e o mesmo degrau, e as duas se reconciliam
    /// numa funcao so em vez de virarem dois caminhos ate o mesmo estado --
    /// que e sempre o par em que um esquece uma conferencia.
    ///
    /// # O que ela NAO faz
    ///
    /// Nao reescreve o `config.json` -- o arquivo e do administrador, com os
    /// comentarios dele, e um servidor que edita a propria configuracao
    /// perderia os dois. A resposta avisa o que ajustar para o proximo
    /// arranque; ate la, o processo vive promovido.
    pub fn promover_para_primario(&self, motivo: &str) -> Result<Json> {
        let de = self.papel_atual();
        if !de.puxa_de_origem() {
            return Err(PhxError::Esquema(format!(
                "o papel atual e {}, e promover so faz sentido num servidor \
                 que replica de uma origem (spare, replica, read_replica)",
                de.nome()
            )));
        }
        self.papel_vivo
            .store(papel_para_u8(Papel::Source), Ordering::SeqCst);
        self.somente_leitura_vivo.store(false, Ordering::SeqCst);
        eprintln!(
            "PROMOVIDO ({motivo}): papel {} virou source; os lacos de replica \
             param na proxima rodada e a escrita esta aberta",
            de.nome()
        );
        Ok(Json::objeto(vec![
            ("papel_anterior", Json::texto_de(de.nome())),
            ("papel", Json::texto_de("source")),
            ("motivo", Json::texto_de(motivo)),
            ("somente_leitura", Json::Bool(false)),
            (
                "aviso",
                Json::texto_de(
                    "promocao vale para este processo: ajuste replicacao.papel \
                     para \"source\" (e somente_leitura para false) no \
                     config.json antes do proximo arranque",
                ),
            ),
            // Desde o c-pleno do pedido 229 o contador de cada Sequence viaja no
            // `posicao` e a replica o adota a cada rodada, ANTES dos eventos:
            // a replica atrasada de vazao nao reemite mais. O que sobra e o
            // atraso de REDE -- o que o master emitiu depois da ultima rodada
            // que chegou aqui --, que nenhum protocolo assincrono recupera.
            (
                "aviso_sequencia",
                Json::texto_de(
                    "o contador de cada Sequence e o do ultimo `posicao` que o \
                     master respondeu a esta replica; numeros que ele emitiu \
                     depois disso (rede cortada antes da promocao) PODEM ser \
                     reemitidos. Rode `reparar` nas tabelas com Sequence para \
                     garantir que o contador nao ficou atras do dado local -- \
                     ver docs/AUTONUMBER.md",
                ),
            ),
        ]))
    }

    /// Conta a escrita LOCAL que uma replica fiel aceitou -- pedido 300 (4).
    ///
    /// # Contar, e nao recusar
    ///
    /// Os tres maduros recusam escrita na replica (`hot_standby` do
    /// PostgreSQL, `read_only` do MySQL e da MariaDB), e aqui a recusa ja
    /// existe: e o `somente_leitura`, que este servidor anuncia faltando no
    /// arranque. Ela continua PEDIDA, e nao imposta (a lei da guarda nova):
    /// quem escreve numa replica hoje nao para de um dia para o outro. O que
    /// faltava era o efeito nao ser calado nem mal contado -- a escrita local
    /// toma o lugar do evento seguinte do source, a conferencia de
    /// continuidade para a tabela, e a recusa culpava o source («apagada e
    /// recriada») pelo que foi escrito AQUI.
    ///
    /// O portao vem antes do trabalho: fora de uma replica fiel, uma
    /// comparacao de papel e volta.
    ///
    /// # Conta em `executar_e_contar_escrita_local`, e nao no portao 2b (pedido 630)
    ///
    /// (02/10/2026: a conta agora entra ANTES da escrita e SAI se ela falhar --
    /// a janela entre a escrita e a conta fazia a recusa nomear as duas causas
    /// em vez da local. O resto desta nota descreve a razao de nao ser o
    /// portao 2b, e continua valendo: o que sai e o que falhou.)
    ///
    /// Ate 01/10/2026 a conta ficava no portao 2b, antes do portao 3 de
    /// permissao e antes de a tabela abrir, com o nome do pedido sem validar.
    /// Dois estragos (revisao SEC, M2): quem so le mandava `inserir` em laco
    /// com nomes aleatorios, todos recusados, e cada nome virava uma chave
    /// deste mapa -- memoria sem teto; e um pedido RECUSADO numa tabela real
    /// fazia o `por_que_nao_continua` culpar a escrita local que nao houve.
    ///
    /// Agora quem chama e o `executar_e_contar_escrita_local`, so com a
    /// operacao respondida `Ok`: passou por todos os portoes, a tabela abriu
    /// -- e o nome e o de uma tabela que existe, o que poe o teto do mapa no
    /// numero de tabelas. O erro que sobra e para o lado certo: a escrita que
    /// falhou no meio depois de gravar alguma coisa nao conta, e sem a conta
    /// a recusa da continuidade NOMEIA as duas causas em vez de escolher uma
    /// (`por_que_nao_continua`). Contar a mais escolheria a errada.
    pub(super) fn anotar_escrita_local(&self, pedido: &Json) -> Option<String> {
        let papel = self.papel_atual();
        if !papel.puxa_de_origem() || papel == Papel::Multi {
            return None;
        }
        let (Some(db), Some(tab)) = (
            pedido.campo("database").and_then(Json::texto),
            pedido.campo("tabela").and_then(Json::texto),
        ) else {
            return None;
        };
        let chave = Self::chave_do_diario(db, tab);
        let mut m = self.escritas_locais_na_replica.lock().ok()?;
        *m.entry(chave.clone()).or_default() += 1;
        Some(chave)
    }

    /// Soma o que um handle de lote contou de orfas -- pedido 300 §2.7.
    pub(super) fn anotar_orfas(&self, chave_tab: &str, (orfas, sem_conferir): (u64, u64)) {
        if orfas + sem_conferir == 0 {
            return;
        }
        if let Ok(mut m) = self.orfas_na_replica.lock() {
            let c = m.entry(chave_tab.to_string()).or_default();
            c.0 += orfas;
            c.1 += sem_conferir;
        }
    }

    /// Anota algo no estado de uma origem, para `replicacao_estado`.
    pub(super) fn anotar_estado(&self, origem: &str, f: impl FnOnce(&mut EstadoOrigem)) {
        if let Ok(mut e) = self.estado_replicacao.lock() {
            f(e.entry(origem.to_string()).or_default());
        }
    }

    /// A replicacao desta tabela nesta origem esta parada?
    ///
    /// Uma comparacao num mapa pequeno, sob o mesmo mutex do
    /// `replicacao_estado` -- e o portao que vem ANTES do trabalho do
    /// alcance. Ver [`bidirecional::ParadaDaTabela`].
    pub(super) fn esta_parada(&self, origem: &str, chave_tab: &str) -> bool {
        self.estado_replicacao
            .lock()
            .ok()
            .and_then(|e| e.get(origem).map(|o| o.paradas.contains_key(chave_tab)))
            .unwrap_or(false)
    }

    /// Marca a parada de UMA tabela num par. Grita UMA vez, e nao a cada
    /// rodada: a rodada seguinte nem chega aqui, porque o portao a tira antes.
    pub(super) fn parar_o_par(
        &self,
        origem: &str,
        chave_tab: &str,
        chave_pos: &str,
        posicao: u64,
        motivo: &str,
        detalhe: String,
    ) {
        eprintln!(
            "replicacao [{origem}]: {chave_tab} PAROU na posicao {posicao} -- {detalhe}. \
             Resolva e solte o par com \
             {{\"op\":\"replicacao_pular\",\"origem\":\"{origem}\",...}}"
        );
        self.anotar_estado(origem, |e| {
            e.paradas.insert(
                chave_tab.to_string(),
                bidirecional::ParadaDaTabela {
                    motivo: motivo.to_string(),
                    posicao,
                    chave_da_posicao: chave_pos.to_string(),
                    detalhe,
                    em_ms: crate::agora_ms(),
                },
            );
        });
    }

    /// O teto de linhas por resposta que vale AGORA.
    pub fn max_linhas(&self) -> u64 {
        self.max_linhas_vivo.load(Ordering::Relaxed)
    }

    /// O servidor esta recusando escrita AGORA?
    pub fn somente_leitura(&self) -> bool {
        self.somente_leitura_vivo.load(Ordering::Relaxed)
    }

    /// Toda tabela aberta daqui para a frente ganha `.bkp`?
    pub fn espelho(&self) -> bool {
        self.espelho_vivo.load(Ordering::Relaxed)
    }

    /// O registro da telemetria, para quem precisa anotar alguma coisa nele.
    pub fn telemetria(&self) -> &Arc<crate::telemetria::Telemetria> {
        &self.telemetria
    }

    /// A porta de dados que este processo REALMENTE abriu -- pedido 401.
    ///
    /// `None` antes do `bind` (a thread do `escutar` ainda nao chegou la) ou
    /// com a porta parada por `servico_parar`. Quem pede `bind: "…:0"` no
    /// `config.json` e precisa saber que numero o sistema deu usa isto, em
    /// vez de reservar um numero por fora e torcer para ninguem mais pega-lo
    /// antes do `bind` de verdade.
    pub fn porta_dos_dados(&self) -> Option<u16> {
        self.endereco_dos_dados
            .lock()
            .ok()
            .and_then(|e| *e)
            .map(|e| e.port())
    }

    /// A porta da interface web REALMENTE aberta -- mesmo motivo da de cima.
    pub fn porta_web(&self) -> Option<u16> {
        self.endereco_web
            .lock()
            .ok()
            .and_then(|e| *e)
            .map(|e| e.port())
    }

    /// A porta do webservice REST REALMENTE aberta -- mesmo motivo.
    pub fn porta_rest(&self) -> Option<u16> {
        self.endereco_rest
            .lock()
            .ok()
            .and_then(|e| *e)
            .map(|e| e.port())
    }

    /// A porta do explorador da API REALMENTE aberta -- mesmo motivo.
    pub fn porta_swagger(&self) -> Option<u16> {
        self.endereco_swagger
            .lock()
            .ok()
            .and_then(|e| *e)
            .map(|e| e.port())
    }

    /// Sobe o servidor e atende ate o processo ser encerrado.
    pub fn escutar(self: &Arc<Self>) -> Result<()> {
        let endereco = self.config.endereco()?;
        self.preparar_tls_dos_dados()?;
        let ouvinte = TcpListener::bind(endereco)
            .map_err(|e| PhxError::Esquema(format!("nao consegui escutar em {endereco}: {e}")))?;
        self.atender_no_ouvinte(ouvinte)
    }

    // ------------------------------------------------------------ ajudantes

    /// Abre a tabela DENTRO de uma trava que quem chamou ja tomou.
    ///
    /// # Por que a trava vem de fora
    ///
    /// Abrir uma tabela LE o cabecalho, e o cabecalho traz `slot_count` e
    /// `proxima_sequencia` -- os dois contadores que decidem onde a proxima
    /// linha vai. Se a trava for tomada e solta aqui, duas operacoes
    /// simultaneas abrem a tabela, cada uma guarda `slot_count = N`, e as duas
    /// gravam no rowid N+1: **uma sobrescreve a outra, em silencio**.
    ///
    /// Era exatamente o que acontecia. A trava tem de cobrir abrir E gravar,
    /// como um bloco so -- por isso ela entra por parametro, e nao e tomada
    /// aqui dentro.
    pub(super) fn abrir_travada(
        &self,
        _dados: &Instancia,
        p: &Json,
        sessao: &Sessao,
    ) -> Result<Table> {
        self.abrir_travada_com(_dados, p, sessao, true)
    }

    /// Abre a tabela travada enxergando SO O DISCO -- sem montar a
    /// sobreposicao da transacao.
    ///
    /// # Por que e uma PORTA, e nao um `ver_so_o_disco()` logo depois
    ///
    /// Porque montar para jogar fora custa, e custa DENTRO da trava de dados:
    /// `sobreposicao` percorre o conjunto de escrita inteiro da transacao e
    /// monta um mapa a cada abertura -- O(pendentes) por operacao, O(n²) por
    /// transacao. O caminho que EMPILHA abria a tabela pela porta de sempre e
    /// chamava `ver_so_o_disco()` na linha seguinte: o mapa nascia e morria
    /// sem ninguem consultar.
    ///
    /// Medido (`--example reparticao-do-gatilho`, sonda do `empilhar`, mesma
    /// tabela e transacoes da maior para a menor): **60 us/op com 100
    /// pendentes e 625,62 us/op com 1.600** -- o piso da secao crescendo com
    /// a lista, sob a trava, para construir o que a linha de baixo apagava.
    ///
    /// # O motivo de o caminho do `empilhar` nao querer a sobreposicao
    ///
    /// Ele ja sabe o que esta pendente por conta propria, e com mensagem
    /// melhor: `chave_ja_empilhada` diz «esta seria a linha 3 da lista», e a
    /// conferencia contra o disco diria so «o indice unico ja tem essa
    /// chave». Ligada ali, a sobreposicao faria a conferencia de disco achar
    /// a linha pendente primeiro e responder com a frase que nao ajuda
    /// ninguem. E dispensa registrada, e nao esquecimento -- e e ela que esta
    /// porta carrega.
    pub(super) fn abrir_travada_sem_sobrepor(
        &self,
        _dados: &Instancia,
        p: &Json,
        sessao: &Sessao,
    ) -> Result<Table> {
        self.abrir_travada_com(_dados, p, sessao, false)
    }

    /// O corpo das duas portas. UMA implementacao, pelo mesmo motivo de o
    /// portao de permissao ser um so: o usuario, a origem, o espelho e a
    /// imagem no diario sao definidos aqui, e uma segunda copia seria a que
    /// esquece um deles.
    fn abrir_travada_com(
        &self,
        _dados: &Instancia,
        p: &Json,
        sessao: &Sessao,
        sobrepor: bool,
    ) -> Result<Table> {
        let database = p.texto_ou("database", "");
        let tabela = p.texto_ou("tabela", "");
        if database.is_empty() || tabela.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"database\" e \"tabela\"".into(),
            ));
        }
        let mut t = _dados.abrir_database(database)?.abrir_qualificada(tabela)?;
        // O espelho e decisao do servidor, nao da tabela: ligar no config.json
        // vale para tudo que este servidor abrir daqui para a frente.
        if self.espelho() && !t.tem_espelho() {
            t.espelhar()?;
        }
        // Quem alterar assina o evento no .log da tabela.
        t.definir_usuario(sessao.id());
        // E, na trilha de dado pessoal, assina tambem de ONDE. Aqui e o unico
        // lugar em que a tabela e aberta para o cliente, entao e o unico ponto
        // que precisa saber disso -- pelo mesmo motivo de o usuario ser
        // definido aqui e nao em cada operacao.
        t.definir_origem(&sessao.ip);
        // A imagem da linha no diario NAO se liga aqui: ela ja veio da
        // politica do diario, herdada do `Database` (pedido 564). Ligar aqui
        // so a da linha era o que deixava a exclusao fisica do multi sem
        // imagem, e o par parado.
        // E o READ-YOUR-OWN-WRITES. Aqui, e nao em cada operacao de leitura,
        // pelo mesmo motivo do portao de permissao ser um so: espalhado por
        // vinte operacoes, a que alguem esquecer mostra o disco enquanto as
        // outras mostram a transacao -- e quem olha a tela nao tem como saber
        // qual das duas esta certa.
        //
        // O `sobrepor` FALSO nao e um esquecimento: e a dispensa do caminho
        // que empilha, escrita em `abrir_travada_sem_sobrepor`. O portao vem
        // ANTES do trabalho, e nao depois -- montar o mapa para a linha
        // seguinte apaga-lo custava O(pendentes) sob a trava.
        if sobrepor {
            self.sobreposicao(database, tabela, sessao, &mut |rowid, p| {
                // O erro da previsao (um CHECK que a linha completada viola)
                // nao impede a LEITURA: a linha crua entra no lugar, e quem
                // recusa e a pre-conferencia do COMMIT.
                let _ = t.sobrepor_mais(rowid, p);
            });
        }
        Ok(t)
    }

    /// Abre a tabela para LER, dentro da ficha compartilhada que quem chamou
    /// ja tomou -- e devolve `None` quando esta tabela pede a ficha exclusiva.
    ///
    /// # As tres recusas, e as tres sao ESCRITA no caminho de leitura
    ///
    /// 1. abrir a tabela escreveria (`.trash` ou `.reason` que ainda nao
    ///    existem, `.log` por curar, troca de volume interrompida no `.reg`);
    /// 2. `recursos.espelho` esta ligado e a tabela ainda nao tem `.bkp` --
    ///    abrir CRIA o espelho, e criar arquivo e escrever;
    /// 3. a tabela tem coluna de dado pessoal -- toda leitura dela grava um
    ///    registro na trilha, e trilha que perde registro em silencio e pior
    ///    que trilha nenhuma.
    ///
    /// Nenhuma das tres e erro: quem chama solta esta ficha, toma a exclusiva
    /// e refaz o trabalho por la, exatamente como antes. **Por isso o
    /// comportamento visto de fora nao muda para tabela nenhuma** -- que e o
    /// que `sem_a_ficha_compartilhada_nada_muda` trava.
    ///
    /// O `usuario` e a `origem` NAO sao definidos aqui, e nao e esquecimento:
    /// os dois so servem para assinar o que se GRAVA (o `.log`) e o que se
    /// registra na trilha -- e a trilha e justamente a recusa (3).
    pub(super) fn abrir_para_ler_travada(
        &self,
        raiz: &Raiz,
        p: &Json,
        sessao: &Sessao,
    ) -> Result<Option<TabelaLeitura>> {
        let database = p.texto_ou("database", "");
        let tabela = p.texto_ou("tabela", "");
        if database.is_empty() || tabela.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"database\" e \"tabela\"".into(),
            ));
        }
        let Aberta::Pronta(mut t) = raiz.abrir_para_ler(database, tabela)? else {
            return Ok(None);
        };
        if (self.espelho() && !t.tem_espelho()) || t.tem_dado_pessoal() {
            return Ok(None);
        }
        self.sobreposicao(database, tabela, sessao, &mut |rowid, p| {
            let _ = t.sobrepor_mais(rowid, p);
        });
        Ok(Some(t))
    }

    /// O que a transacao DESTA conexao ja pediu nesta tabela e ainda nao gravou.
    ///
    /// # O portao vem antes do trabalho
    ///
    /// Primeira linha: um `load` atomico. Servidor sem transacao aberta --
    /// que e o caso comum -- nao toma mutex, nao percorre lista e nao aloca
    /// nada. E a licao do Profiler, que cobrava 7% da carga fazendo o trabalho
    /// antes de perguntar se estava ligado.
    ///
    /// # Por que a dobra acontece AQUI e nao no `transacao.rs`
    ///
    /// Porque marcar uma linha exige saber onde mora a coluna de sistema, e
    /// completar uma linha exige a tabela inteira -- o DEFAULT, a `Sequence`,
    /// a calculada (achado A3 da revisao do DBA ao 448). Quem dobra e a
    /// PROPRIA tabela aberta (`sobrepor_mais`), pelas funcoes que gravam; esta
    /// funcao so entrega as escritas, na ordem, a quem dobra -- e assim ela
    /// serve as DUAS fichas, a exclusiva (`Table`) e a compartilhada
    /// (`TabelaLeitura`). Uma segunda copia para a segunda ficha divergiria, e
    /// a divergencia apareceria como a mesma consulta enxergando a transacao
    /// num caminho e o disco no outro.
    fn sobreposicao(
        &self,
        database: &str,
        tabela: &str,
        sessao: &Sessao,
        dobrar: &mut dyn FnMut(u64, phxsql_store::table::Pendente<'_>),
    ) {
        if self.transacoes_abertas.load(Ordering::Relaxed) == 0 || sessao.ligacao == 0 {
            return;
        }
        let reg = self.transacoes.travar();
        let Some(tx) = reg.de(sessao.ligacao) else {
            return;
        };
        // A ORDEM MANDA, e a dobra e por isso -- ver `Sobreposicao::empilhar`,
        // que e a MESMA que a pre-conferencia do COMMIT usa (pedido 448).
        for e in &tx.escritas {
            if !escrita_da_tabela(e, database, tabela) {
                continue;
            }
            dobrar(e.rowid, pendente_de(e));
        }
    }

    /// A linha `rowid` como a transacao DESTA sessao a ve: o disco, com as
    /// escritas que a propria lista ja pediu NESSA linha por cima.
    ///
    /// Pedidos 492 e 515. O `empilhar` le o disco de proposito (a dispensa do
    /// `abrir_travada_sem_sobrepor`), e continua lendo para o que e do disco:
    /// a existencia da linha e a `linha_antiga` da marca. Mas a marca de
    /// excluida que a alteracao herda, a linha sobre a qual o upsert mescla o
    /// SET e a linha de onde o plano da cascata parte sao da TRANSACAO: lidas
    /// do disco, `[excluir suave M, atualizar M]` ressuscitava M so dentro de
    /// transacao, e o elo sobrescrevia o que a lista ja tinha escrito na
    /// filha.
    ///
    /// O custo e uma volta na lista -- a mesma do `nasceu_aqui`, que este
    /// caminho ja paga -- e a dobra so das escritas DESTA linha, pela funcao
    /// da leitura (`Table::ler_do_disco_com`), e nao a sobreposicao inteira
    /// que a porta sem sobrepor existe para nao montar.
    pub(super) fn linha_na_transacao(
        &self,
        t: &mut Table,
        database: &str,
        tabela: &str,
        rowid: u64,
        sessao: &Sessao,
    ) -> Result<Option<Vec<Value>>> {
        let desta_linha: Vec<crate::transacao::Escrita> = {
            let reg = self.transacoes.travar();
            let tx = reg.de(sessao.ligacao).ok_or_else(sem_transacao)?;
            tx.escritas
                .iter()
                .filter(|e| e.rowid == rowid && escrita_da_tabela(e, database, tabela))
                .cloned()
                .collect()
        };
        let pendentes: Vec<phxsql_store::table::Pendente<'_>> =
            desta_linha.iter().map(pendente_de).collect();
        t.ler_do_disco_com(rowid, &pendentes)
    }

    /// O plano da cascata de uma alteracao que a transacao esta EMPILHANDO,
    /// contra o que ela ve (pedido 515): a mae parte de `antes` -- a linha
    /// como a transacao a ve --, e cada filha que o plano abre enxerga o que a
    /// lista ja pediu nela. Os tres pontos que planejam no `empilhar` (a
    /// alteracao, o upsert que virou alteracao e a fase 3 da cascata) passam
    /// por aqui, e nao cada um pelo seu caminho.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn planejar_cascata_empilhada(
        &self,
        trava: &Instancia,
        t: &mut Table,
        database: &str,
        tabela: &str,
        antes: &[Value],
        crua: &[Value],
        sessao: &Sessao,
    ) -> Result<Vec<phxsql_store::table::EscritaDaCascata>> {
        let prefixo = PrefixoDaSessao {
            servidor: self,
            trava,
            database,
            schema: phxsql_store::catalogo::separar_qualificado(tabela).0,
            sessao,
            montados: std::cell::RefCell::new(HashMap::new()),
        };
        let plano = t.planejar_cascata_da_alteracao(antes, crua, Some(&prefixo))?;
        // Pedido 496, F8: o irmao do `alterar_solto` -- empilhar nao grava,
        // e o COMMIT e a primeira escrita.
        // O irmao da recusa do `alterar_solto` (765/767): a mesma pergunta,
        // antes do COMMIT, que e a primeira escrita.
        if let Some((filha, linhas, vivas)) =
            crate::plano_largo::observar_a_cascata(database, &plano)
        {
            self.protecao_do_plano(
                "cascata",
                database,
                &filha,
                (linhas, vivas),
                sessao,
                Some(trava),
            )?;
        }
        Ok(plano)
    }

    /// Anota na trilha que uma operacao LEU dado pessoal.
    ///
    /// # O `if` vem antes do `format!`
    ///
    /// O criterio chega como fecho e nao como texto de proposito: numa tabela
    /// sem coluna marcada -- a maioria -- montar a frase seria alocar uma
    /// `String` por leitura para jogar fora, que e exatamente a conta que o
    /// Profiler desligado cobrava. Aqui o portao pergunta primeiro.
    ///
    /// # E por que o erro sobe, em vez de ser engolido
    ///
    /// Uma trilha que perde registro em silencio e pior que trilha nenhuma:
    /// ela PARECE completa, e quem audita seis meses depois conclui que
    /// ninguem leu aquela ficha. Se o disco nao aceita o registro, a leitura
    /// que ficaria sem rastro nao acontece.
    pub(super) fn trilhar_acesso(
        t: &mut Table,
        rowid: u64,
        linhas: u64,
        criterio: impl FnOnce() -> String,
    ) -> Result<()> {
        if !t.tem_dado_pessoal() || linhas == 0 {
            return Ok(());
        }
        t.registrar_acesso(rowid, &criterio(), linhas)
    }

    pub(super) fn rowid(&self, p: &Json) -> Result<u64> {
        p.campo("rowid")
            .and_then(Json::inteiro)
            .filter(|n| *n > 0)
            .map(|n| n as u64)
            .ok_or_else(|| PhxError::Esquema("informe \"rowid\" maior que zero".into()))
    }

    pub(super) fn limite(&self, p: &Json) -> u64 {
        let teto = self.max_linhas();
        let pedido = p.inteiro_ou("max", teto as i64).max(0) as u64;
        if pedido == 0 {
            teto
        } else {
            pedido.min(teto)
        }
    }

    /// Os tres alarmes de servidor que so o ARRANQUE ve (pedido 769), cada
    /// um pelo produtor unico, e so quando o fato aconteceu: o arranque de
    /// sempre nao sinaliza nada, como nao avisa nada.
    ///
    /// - `FsyncRecusadoAntes`: a sentinela do 509 de um boot anterior (ao
    ///   vivo o processo cai, e quem conta e o arranque seguinte);
    /// - `MarcaNaoResolvida`: marca que a recuperacao deixou no disco sem
    ///   completar -- operacao impossivel ou cifrada sem a chave. A que nem
    ///   se leu (`sem_leitura`) impede a subida e nao chega aqui;
    /// - `IndiceAtrasado`: o texto do `evento_do_arranque`, a mesma decisao
    ///   do aviso por e-mail.
    pub(super) fn sinalizar_o_arranque(
        &self,
        fsync_de_boot_anterior: Option<&str>,
        recuperacao: &crate::transacao::Relatorio,
        indice_atrasado: Option<&str>,
    ) {
        if let Some(sentinela) = fsync_de_boot_anterior {
            crate::telemetria::sinal(crate::aquario::Alarme::FsyncRecusadoAntes, sentinela);
        }
        for marca in recuperacao.impossiveis.iter().chain(&recuperacao.paradas) {
            crate::telemetria::sinal(crate::aquario::Alarme::MarcaNaoResolvida, marca);
        }
        if let Some(texto) = indice_atrasado {
            crate::telemetria::sinal(crate::aquario::Alarme::IndiceAtrasado, texto);
        }
    }
}

/// O papel num byte, para o `AtomicU8` do papel vivo.
///
/// O par de funcoes mora junto para nao divergir; um valor desconhecido na
/// volta cai em `Isolado`, que e o papel que nao promete nada.
/// A politica do diario deste servidor, decidida UMA vez a partir do config
/// (pedidos 564 e 416). Ver `phxsql_store::catalogo::PoliticaDoDiario`.
///
/// O multi entra ligado mesmo que alguem o monte sem `imagem_da_linha` (o
/// `validar` do config ja recusa, mas um `Config` montado em codigo nao passa
/// por ele): no multi a chave mora na imagem, e sem ela o par para.
pub(super) fn politica_do_diario(config: &Config) -> phxsql_store::catalogo::PoliticaDoDiario {
    phxsql_store::catalogo::PoliticaDoDiario::com_imagem(
        config.replicacao.imagem_da_linha || config.replicacao.papel == Papel::Multi,
    )
}

fn papel_para_u8(p: Papel) -> u8 {
    match p {
        Papel::Isolado => 0,
        Papel::Source => 1,
        Papel::Replica => 2,
        Papel::ReadReplica => 3,
        Papel::Spare => 4,
        Papel::Multi => 5,
    }
}

fn u8_para_papel(v: u8) -> Papel {
    match v {
        1 => Papel::Source,
        2 => Papel::Replica,
        3 => Papel::ReadReplica,
        4 => Papel::Spare,
        5 => Papel::Multi,
        _ => Papel::Isolado,
    }
}

/// Tomar uma trava do servidor que RECUSA quando esta suja -- pedido 458.
///
/// # Por que recusar aqui, e recuperar nas transacoes
///
/// Atras destas travas ha estado que o panico pode ter deixado pela metade
/// e que ninguem sabe sanear as cegas: a lista de rotinas e visoes que se
/// grava junto do disco, o registro de jobs do `jobs.json`, as ligacoes de
/// DbLink. As duas da vida de uma transacao (`transacoes` e `travas`) sao
/// `TravaDaGuarda`, porque atras delas nao ha disco e o saneamento e
/// conhecido (`Transacoes::abortar_abertas`).
///
/// Um motor so para tomar, e nao 85 `map_err` com a mesma frase: a que
/// alguem esquecesse de nomear seria a que ninguem acha no log.
pub(super) trait TomarTrava<T> {
    fn tomar(&self, nome: &'static str) -> Result<std::sync::MutexGuard<'_, T>>;
}

impl<T> TomarTrava<T> for Mutex<T> {
    fn tomar(&self, nome: &'static str) -> Result<std::sync::MutexGuard<'_, T>> {
        self.lock().map_err(|_| trava_envenenada(nome))
    }
}

/// As raizes das instancias deste processo: o gancho e um `fn` sem captura, e
/// precisa saber onde gravar.
pub(super) static BASES_DA_SENTINELA: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

fn registrar_base_da_sentinela(base: &Path) {
    let mut b = BASES_DA_SENTINELA
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !b.iter().any(|x| x == base) {
        b.push(base.to_path_buf());
    }
}

/// O arranque: com a sentinela no disco, sobe so se o boot mudou.
///
/// `avisar(caminho, erro)` roda UMA vez, so no caminho que recusa, e ANTES de
/// devolver a recusa (pedido 573): e o unico ponto que sabe que o servidor
/// nao vai subir.
///
/// Devolve o texto da sentinela de um boot ANTERIOR que ela achou e apagou --
/// o fato do alarme `FsyncRecusadoAntes` (pedido 769) --, ou `None` no
/// arranque de sempre.
pub(super) fn conferir_sentinela_509(
    base: &Path,
    avisar: impl FnOnce(&str, &str),
) -> Result<Option<String>> {
    let arquivo = base.join(SENTINELA_509);
    let Ok(texto) = std::fs::read_to_string(&arquivo) else {
        return Ok(None);
    };
    let gravado = texto
        .lines()
        .find_map(|l| l.strip_prefix("boot_id="))
        .unwrap_or("")
        .trim()
        .to_string();
    let agora = boot_id();
    if !gravado.is_empty() && agora.as_deref().is_some_and(|b| b != gravado) {
        // Outro boot: o cache que mentia se foi, e o que esta no disco e a
        // verdade -- a recuperacao e o `reindexar` do 522 cuidam dela.
        let _ = std::fs::remove_file(&arquivo);
        eprintln!(
            "aviso: {} registrava um fsync recusado num boot ANTERIOR; o cache \
             daquele boot se foi, e o arranque segue pela recuperacao (pedido 509)",
            arquivo.display()
        );
        return Ok(Some(texto));
    }
    let caminho = texto
        .lines()
        .find_map(|l| l.strip_prefix("caminho="))
        .unwrap_or("?");
    let erro = texto
        .lines()
        .find_map(|l| l.strip_prefix("erro="))
        .unwrap_or("?");
    avisar(caminho, erro);
    Err(PhxError::Io(std::io::Error::other(format!(
        "o servidor NAO sobe: o fsync de {caminho} foi recusado neste MESMO boot \
         ({}). O cache do nucleo pode estar devolvendo o que o disco perdeu, e a \
         recuperacao daria por gravado o que nao esta. Remonte o volume ou \
         reinicie a maquina e suba de novo{} (pedido 509)",
        arquivo.display(),
        if agora.is_none() {
            format!(
                "; este sistema nao informa o boot, entao, DEPOIS de reiniciar, \
                 apague {}",
                arquivo.display()
            )
        } else {
            String::new()
        }
    ))))
}

/// O aviso do arranque que a sentinela do 509 recusou (pedido 573).
///
/// Sai pelo MESMO [`Carteiro`] da saude do disco (e-mail, SMS pelo gateway,
/// gancho do operador), de forma sincrona: nao ha thread `sonda-disco` ainda,
/// nem trava nenhuma na mao, e o processo vai sair logo depois -- enfileirar
/// seria perder o aviso. O tipo e o de E/S: foi um `fsync` recusado, e e o
/// que o operador ja filtra. Sem e-mail e sem gancho, so a linha do erro
/// padrao -- a mesma de todo evento de saude.
fn avisar_o_arranque_recusado(config: &Config, caminho: &str, erro: &str) {
    let saude =
        crate::saude_do_disco::SaudeDoDisco::nova(config.alertas.disco.clone(), &config.base);
    let evento = crate::saude_do_disco::Evento {
        quando_ms: crate::agora_ms(),
        tipo: crate::saude_do_disco::Tipo::EntradaSaida,
        origem: "arranque".into(),
        database: String::new(),
        tabela: String::new(),
        texto: format!(
            "o fsync de {caminho} foi recusado ({erro}) neste MESMO boot, e o \
             servidor NAO sobe ate o volume ser remontado ou a maquina \
             reiniciada (pedido 509)"
        ),
    };
    let fio = crate::telemetria::Fio::avulso(
        "arranque",
        "avisa pelo carteiro que o arranque foi recusado pela sentinela do 509",
    );
    Carteiro {
        config,
        saude: &saude,
    }
    .avisar_saude_do_disco(&fio, evento);
}
