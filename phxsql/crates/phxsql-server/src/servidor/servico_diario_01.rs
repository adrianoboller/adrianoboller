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
    /// O nome vem de `consumidor` (a replica fiel e o espelho) ou de `para`
    /// (o bidirecional, que ja dizia quem pede).
    pub(super) fn anotar_confirmacao_do_diario(&self, p: &Json, chave: &str, desde: u64) {
        let cfg = &self.config.diario;
        if !cfg.expurgo || cfg.consumidores.is_empty() {
            return;
        }
        let quem = match p.texto_ou("consumidor", "").trim() {
            "" => p.texto_ou("para", "").trim(),
            q => q,
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

/// Quantas vezes o retrato tenta achar o database sem tabela suja antes de
/// desistir. A janela de durabilidade fecha em centenas de milissegundos;
/// cinquenta tentativas so falham num database que nao para de escrever.
const TENTATIVAS_DO_RETRATO: u32 = 50;

/// O prefixo dos arquivos de retrato na raiz de dados. Arquivo, e nao pasta:
/// toda pasta da raiz e um database.
const PREFIXO_SERVIDO: &str = ".retrato-servido-";
const PREFIXO_RECEBIDO: &str = ".retrato-recebido-";

/// O retrato que esta origem serve -- pedido 706.
pub(super) struct RetratoServido {
    id: u64,
    /// `(tabela, arquivo, copia, bytes)`.
    arquivos: Vec<(String, String, PathBuf, u64)>,
}

/// Apaga da raiz os arquivos de retrato com `prefixo` -- o servido que
/// acabou, ou o que uma queda deixou para tras.
fn apagar_retratos(raiz: &Path, prefixo: &str) {
    let Ok(entradas) = std::fs::read_dir(raiz) else {
        return;
    };
    for e in entradas.flatten() {
        if e.file_name().to_string_lossy().starts_with(prefixo) {
            let _ = std::fs::remove_file(e.path());
        }
    }
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
    pub(super) fn op_retrato_da_replica(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        if p.booleano_ou("soltar", false) {
            self.soltar_retrato_servido();
            return Ok(Json::objeto(vec![("soltou", Json::Bool(true))]));
        }
        if p.campo("indice").is_some() {
            return self.pedaco_do_retrato(p);
        }
        self.tirar_retrato(p, sessao)
    }

    fn soltar_retrato_servido(&self) {
        let mut r = self
            .retrato_servido
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *r = None;
        apagar_retratos(&self.config.base, PREFIXO_SERVIDO);
    }

    fn tirar_retrato(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "").trim().to_string();
        if database.is_empty() {
            return Err(PhxError::Esquema("informe \"database\"".into()));
        }
        self.soltar_retrato_servido();
        // O retrato so vale com os cabecalhos escritos: a tabela suja da
        // janela de durabilidade tem o cabecalho do `.log` e o do `.reg` atras
        // dos dados. Descarrega fora desta tomada (a descarga toma a trava e
        // faz o `fsync` ela mesma), e confere de novo com a trava na mao.
        let prefixo_db = format!("{}/", database.to_lowercase());
        let mut tentativa = 0;
        let dados = loop {
            self.descarregar_sujas();
            let dados = self.travar_dados()?;
            let suja = self.sujas.lock().map_or(true, |s| {
                s.iter().any(|k| k.to_lowercase().starts_with(&prefixo_db))
            });
            if !suja {
                break dados;
            }
            drop(dados);
            tentativa += 1;
            if tentativa >= TENTATIVAS_DO_RETRATO {
                return Err(PhxError::EmCarga(format!(
                    "o database {database} nao parou de escrever em {TENTATIVAS_DO_RETRATO} \
                     tentativas: o retrato fica para a rodada seguinte"
                )));
            }
        };
        let db = dados.abrir_database(&database)?;
        let id = crate::agora_ms().max(1) as u64;
        let mut arquivos = Vec::new();
        let mut eventos = Vec::new();
        for (k, nome) in db.todas_as_tabelas()?.into_iter().enumerate() {
            if !replica_alcanca(sessao.usuario.as_ref(), &database, &nome) {
                continue;
            }
            {
                let mut t = db.abrir_qualificada(&nome)?;
                // O mesmo portao do `replicar` (pedido 342): coluna marcada
                // nao atravessa fio em claro, nem pelo retrato.
                if t.tem_dado_pessoal() && !self.fio_cifrado(sessao) {
                    return Err(PhxError::Autorizacao(
                        self.msg("erro.replicar_marcada_exige_cifra", &[("tabela", &nome)]),
                    ));
                }
                eventos.push((nome.clone(), Json::de_u64(t.eventos()?)));
            }
            let prefixo = format!("{PREFIXO_SERVIDO}{id}-{k}-");
            for (arquivo, copia, bytes) in db.retratar_tabela(&nome, &self.config.base, &prefixo)? {
                arquivos.push((nome.clone(), arquivo, copia, bytes));
            }
        }
        drop(dados);
        let lista = arquivos
            .iter()
            .map(|(t, a, _, b)| {
                Json::objeto(vec![
                    ("tabela", Json::texto_de(t)),
                    ("arquivo", Json::texto_de(a)),
                    ("bytes", Json::de_u64(*b)),
                ])
            })
            .collect();
        *self
            .retrato_servido
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(RetratoServido { id, arquivos });
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(&database)),
            ("id", Json::de_u64(id)),
            ("arquivos", Json::Lista(lista)),
            ("eventos", Json::Objeto(eventos)),
        ]))
    }

    fn pedaco_do_retrato(&self, p: &Json) -> Result<Json> {
        let id = p.inteiro_ou("id", 0).max(0) as u64;
        let indice = p.inteiro_ou("indice", -1);
        let offset = p.inteiro_ou("offset", 0).max(0) as u64;
        let max = match p.inteiro_ou("max", 0) {
            n if n <= 0 => PEDACO_DO_RETRATO,
            n => (n as u64).min(PEDACO_DO_RETRATO),
        };
        let caminho = {
            let r = self
                .retrato_servido
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(r) = r.as_ref().filter(|r| r.id == id) else {
                return Err(PhxError::NaoEncontrado(format!(
                    "nao ha retrato {id} servido aqui: peca um novo"
                )));
            };
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
        let mut tabelas: Vec<String> = recebidos.iter().map(|(t, _, _)| t.clone()).collect();
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
