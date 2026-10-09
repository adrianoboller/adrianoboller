//! Administracao: acessos, bloqueios, mensagens, idiomas, catalogo de
//! sistema, sessoes e telemetria.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// Quantas tabelas o fecho da janela sincroniza AO MESMO TEMPO.
///
/// O fecho e' 93-96% `fsync` (medido, `--example o-comboio-por-dentro`), e num
/// `ext4` os `fsync` de arquivos diferentes se juntam no mesmo diario: por isso
/// sincronizar as K tabelas sujas ao mesmo tempo custa muito menos que K vezes
/// uma. Medido em `--example o-comboio-em-paralelo`: 1,62x em K=4 e 2,52x em
/// K=16.
///
/// # Por que ele existe, e nao um fio por tabela suja
///
/// Porque K nao tem teto: uma base com dezenas de tabelas ativas deixaria
/// dezenas de tabelas sujas na mesma janela, e a resposta a um pico de escrita
/// nao pode ser um pico de fios. Acima deste numero o fecho vai em pedacos --
/// serializa entre pedacos, que e' o comportamento de hoje, e o unico preco e'
/// nao juntar TODOS os `fsync` num diario so.
///
/// O numero e' a largura em que o ganho medido ja tinha crescido de 1,00x
/// (K=1) a 2,52x sem sinal de dobra. Nao e' catraca: catraca so' desce, e este
/// numero nao mede promessa nenhuma -- e' um teto de recurso, e quem o mudar
/// mede de novo com o exemplo.
pub(super) const FIOS_DO_FECHO: usize = 16;

/// Texto de expressao no JSON do esquema: o texto, ou nulo quando nao ha.
/// O material EM DISCO de uma tabela, como texto de resposta.
///
/// `cifra.ligada` (op `config`) e do PROCESSO; isto e do ARQUIVO. Divergem em
/// toda tabela nascida antes de o cofre ser ligado -- e nas que nasceram em
/// claro pelo defeito do pedido 210 (unicas colunas marcadas externas). Sem
/// este campo, «cifra ligada» era meia-verdade por tabela, e ninguem tinha
/// como perguntar ao servidor qual tabela protege o que declarou.
pub(super) fn material_da_tabela(t: &Table) -> &'static str {
    if t.cifrada() {
        "cifrado"
    } else {
        "em_claro"
    }
}

impl Servidor {
    // ------------------------------------------------------------ operacoes

    pub(super) fn op_acessos(&self, p: &Json) -> Result<Json> {
        let max = self.limite(p) as usize;
        let todos = LogAcessos::ler(&self.config.log_acessos)?;
        let total = todos.len();
        let recentes: Vec<Json> = todos
            .iter()
            .rev()
            .take(max)
            .map(|a| a.para_json())
            .collect();
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(total as u64)),
            ("acessos", Json::Lista(recentes)),
        ]))
    }

    pub(super) fn op_ips(&self) -> Result<Json> {
        let resumo = LogAcessos::resumo_por_ip(&self.config.log_acessos)?;
        Ok(Json::Lista(
            resumo
                .iter()
                .map(|r| {
                    Json::objeto(vec![
                        ("ip", Json::texto_de(&r.ip)),
                        ("acessos", Json::de_u64(r.acessos)),
                        ("recusados", Json::de_u64(r.recusados)),
                        ("primeiro", Json::texto_de(r.primeiro())),
                        ("ultimo", Json::texto_de(r.ultimo())),
                    ])
                })
                .collect(),
        ))
    }

    pub(super) fn op_bloqueios(&self) -> Result<Json> {
        let lista = self.lista_negra.tomar("lista_negra")?;
        let agora = crate::agora_ms();
        let p = &self.config.politica;
        Ok(Json::objeto(vec![
            (
                "arquivo",
                Json::texto_de(lista.caminho().display().to_string()),
            ),
            (
                "ativos",
                Json::Lista(
                    lista
                        .ativos(agora)
                        .into_iter()
                        .map(|b| b.para_json())
                        .collect(),
                ),
            ),
            // As duas whitelists SEPARADAS, porque so uma delas se edita pela
            // tela: a do config e de quem administra o arquivo.
            (
                "whitelist_config",
                Json::Lista(p.whitelist.iter().map(Json::texto_de).collect()),
            ),
            (
                "whitelist",
                Json::Lista(lista.whitelist().iter().map(Json::texto_de).collect()),
            ),
            // A politica em vigor, para a tela dizer a verdade sobre o que
            // esta valendo -- e nao o que alguem lembra de ter configurado.
            (
                "politica",
                Json::objeto(vec![
                    (
                        "comandos_proibidos",
                        Json::Lista(p.comandos_proibidos.iter().map(Json::texto_de).collect()),
                    ),
                    (
                        "bases_proibidas",
                        Json::Lista(p.bases_proibidas.iter().map(Json::texto_de).collect()),
                    ),
                    (
                        "tentativas_para_bloqueio",
                        Json::de_u64(p.tentativas_para_bloqueio as u64),
                    ),
                    (
                        "tentativas_ate_bloquear",
                        Json::de_u64(p.tentativas_ate_bloquear as u64),
                    ),
                    ("janela_minutos", Json::de_u64(p.janela_minutos)),
                    ("bloqueio_minutos", Json::de_u64(p.bloqueio_minutos)),
                    (
                        "firewall",
                        Json::Bool(p.firewall.as_ref().map(|f| f.ligado).unwrap_or(false)),
                    ),
                ]),
            ),
        ]))
    }

    pub(super) fn op_desbloquear(&self, p: &Json) -> Result<Json> {
        let ip = p.texto_ou("ip", "").trim().to_string();
        if ip.is_empty() {
            return Err(PhxError::Esquema("informe \"ip\"".into()));
        }
        let tinha = {
            let mut lista = self.lista_negra.tomar("lista_negra")?;
            lista.desbloquear(&ip)?
        };
        // A regra de firewall sai FORA do mutex (pedido 638). Se ela falhar o
        // IP ja saiu da lista, como sempre foi: o erro sobe para quem pediu.
        if tinha {
            crate::blacklist::soltar_no_firewall(&self.config.politica, std::slice::from_ref(&ip))?;
        }
        Ok(Json::objeto(vec![
            ("ip", Json::texto_de(&ip)),
            ("estava_bloqueado", Json::Bool(tinha)),
        ]))
    }

    /// `bloqueios_exportar`: a blacklist ativa no formato que um firewall de
    /// verdade consome. O servidor nao roda como root e nao mexe em iptables
    /// sozinho -- entrega o texto para quem tem o privilegio aplicar.
    pub(super) fn op_bloqueios_exportar(&self, p: &Json) -> Result<Json> {
        let formato = p.texto_ou("formato", "texto").trim().to_string();
        let lista = self.lista_negra.tomar("lista_negra")?;
        let agora = crate::agora_ms();
        let texto = lista.exportar(&formato, agora)?;
        Ok(Json::objeto(vec![
            ("formato", Json::texto_de(&formato)),
            ("ips", Json::de_u64(lista.ativos(agora).len() as u64)),
            ("texto", Json::texto_de(texto)),
        ]))
    }

    /// `whitelist_salvar`: substitui a whitelist EDITAVEL (a do arquivo da
    /// blacklist). A do `config.json` nao se mexe por aqui -- config muda com
    /// o arquivo, pelo mesmo motivo de sempre.
    pub(super) fn op_whitelist_salvar(&self, p: &Json) -> Result<Json> {
        let regras = p.textos("whitelist");
        let mut lista = self.lista_negra.tomar("lista_negra")?;
        lista.definir_whitelist(regras)?;
        Ok(Json::objeto(vec![(
            "whitelist",
            Json::Lista(lista.whitelist().iter().map(Json::texto_de).collect()),
        )]))
    }

    /// `mensagens`: o estado da tabela de mensagens, para a tela dizer a
    /// verdade -- qual idioma vale, se a tabela existe, o que falta semear.
    pub(super) fn op_mensagens(&self) -> Result<Json> {
        self.mensagens_atualizar();
        let (existe, linhas) = {
            let dados = self.travar_dados()?;
            match dados.abrir_database(crate::mensagens::DATABASE) {
                Err(_) => (false, 0u64),
                Ok(db) => match db.existe_tabela(None, crate::mensagens::TABELA)? {
                    false => (false, 0),
                    true => (
                        true,
                        db.abrir_qualificada(crate::mensagens::TABELA)
                            .map(|t| t.registros())
                            .unwrap_or(0),
                    ),
                },
            }
        };
        let conhecidas = crate::mensagens::FABRICA.len() as u64;
        Ok(Json::objeto(vec![
            ("idioma", Json::texto_de(self.mensagens.idioma())),
            ("database", Json::texto_de(crate::mensagens::DATABASE)),
            ("tabela", Json::texto_de(crate::mensagens::TABELA)),
            ("existe", Json::Bool(existe)),
            ("linhas", Json::de_u64(linhas)),
            ("de_fabrica", Json::de_u64(conhecidas)),
            (
                "idiomas",
                Json::Lista(
                    crate::mensagens::IDIOMAS
                        .iter()
                        .map(|i| Json::texto_de(*i))
                        .collect(),
                ),
            ),
        ]))
    }

    /// `mensagens_semear`: cria e completa a tabela de mensagens. Idempotente
    /// -- linha existente nunca e tocada, entao semear de novo e sempre
    /// seguro, inclusive depois de alguem traduzir metade.
    pub(super) fn op_mensagens_semear(&self) -> Result<Json> {
        let (criou_db, criou_tabela, semeadas, existiam) = self.semear_mensagens()?;
        Ok(Json::objeto(vec![
            ("criou_database", Json::Bool(criou_db)),
            ("criou_tabela", Json::Bool(criou_tabela)),
            ("semeadas", Json::de_u64(semeadas)),
            ("ja_existiam", Json::de_u64(existiam)),
        ]))
    }

    /// O portao PROPRIO das operacoes de idioma.
    ///
    /// As cinco nao tem campo `"tabela"` no pedido -- e o portao geral confere
    /// justamente esse campo. Sem esta conferencia elas cairiam na regra da
    /// base vazia e ninguem descobriria por leitura, que e exatamente o furo
    /// que `juntar` e `unir` ja tiveram. Aqui a tabela e fixa, entao o portao
    /// proprio e curto: ela e sempre `phxsys.mensagens`.
    fn poder_nos_idiomas(&self, sessao: &Sessao, atividade: Atividade) -> Result<()> {
        let Some(usuario) = sessao.usuario.as_ref() else {
            // Sem usuario e o token de servico, que ja passou pelo portao 1.
            return Ok(());
        };
        if usuario.pode_em(idiomas::DATABASE, idiomas::TABELA, atividade) {
            return Ok(());
        }
        Err(PhxError::Autorizacao(format!(
            "{} nao tem permissao de {} em {}.{}",
            usuario.login,
            atividade.nome(),
            idiomas::DATABASE,
            idiomas::TABELA
        )))
    }

    /// `idiomas`: o estado da tabela de textos, no idioma perguntado.
    pub(super) fn op_idiomas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        self.poder_nos_idiomas(sessao, Atividade::Ler)?;
        let idioma = idiomas::indice_do_idioma(p.texto_ou("idioma", ""));
        let dados = self.travar_dados()?;
        Ok(idiomas::estado(&dados, idioma))
    }

    /// `idiomas_carga`: semeia os textos de tela que faltam.
    ///
    /// Idempotente por `TextName`: linha que existe NAO e tocada. E por isso
    /// que esta operacao pode ser chamada a vontade sem desfazer traducao de
    /// ninguem -- quem sobrescreve e a `idiomas_padrao`, e ela pergunta antes.
    pub(super) fn op_idiomas_carga(&self, _p: &Json, sessao: &Sessao) -> Result<Json> {
        self.poder_nos_idiomas(sessao, Atividade::Administrar)?;
        let dados = self.travar_dados()?;
        Ok(idiomas::carga(&dados, idiomas::Sobrescrever::Nenhum)?.para_json())
    }

    /// `idiomas_padrao`: devolve os textos de FABRICA por cima do que estiver
    /// gravado -- so o idioma pedido, ou os seis.
    ///
    /// Esta e a que apaga trabalho, entao o escopo e explicito: sem `"idioma"`
    /// e sem `"tudo": true` ela recusa, em vez de escolher sozinha qual dos
    /// dois estragos fazer.
    pub(super) fn op_idiomas_padrao(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        self.poder_nos_idiomas(sessao, Atividade::Administrar)?;
        let tudo = p.booleano_ou("tudo", false);
        let pedido = p.texto_ou("idioma", "").trim().to_string();
        let modo = match (tudo, pedido.is_empty()) {
            (true, _) => idiomas::Sobrescrever::Tudo,
            (false, false) => {
                if !idiomas::IDIOMAS.contains(&pedido.as_str()) {
                    return Err(PhxError::Esquema(format!(
                        "idioma {pedido:?} nao existe: use um de {:?}",
                        idiomas::IDIOMAS
                    )));
                }
                idiomas::Sobrescrever::So(idiomas::indice_do_idioma(&pedido))
            }
            (false, true) => {
                return Err(PhxError::Esquema(
                    "diga o que sobrescrever: \"idioma\" para um so, ou \"tudo\":true \
                     para os seis"
                        .into(),
                ))
            }
        };
        let dados = self.travar_dados()?;
        let r = idiomas::carga(&dados, modo)?;
        let mut j = r.para_json();
        if let Json::Objeto(campos) = &mut j {
            campos.push((
                "sobrescreveu".to_string(),
                Json::texto_de(if tudo { "todos os idiomas" } else { &pedido }),
            ));
        }
        Ok(j)
    }

    /// `idiomas_exportar`: a tabela inteira em JSON, para guardar FORA do
    /// banco. Leva todos os `TextName`, e nao so os de tela -- backup que
    /// deixa metade para tras nao e backup.
    pub(super) fn op_idiomas_exportar(&self, sessao: &Sessao) -> Result<Json> {
        self.poder_nos_idiomas(sessao, Atividade::Ler)?;
        let dados = self.travar_dados()?;
        idiomas::exportar(&dados)
    }

    /// `idiomas_importar`: devolve o backup para a tabela.
    pub(super) fn op_idiomas_importar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        self.poder_nos_idiomas(sessao, Atividade::Administrar)?;
        let backup = match p.campo("backup") {
            Some(b) => b,
            None => return Err(PhxError::Esquema("informe \"backup\"".into())),
        };
        let dados = self.travar_dados()?;
        Ok(idiomas::importar(&dados, backup)?.para_json())
    }

    pub(super) fn op_bancos(&self) -> Result<Json> {
        let dados = self.travar_dados()?;
        Ok(Json::Lista(
            dados.databases()?.into_iter().map(Json::texto_de).collect(),
        ))
    }

    /// `tabelas`: as tabelas da base, **so as que quem pediu pode ler**.
    ///
    /// Filtrar aqui nao e enfeite. Sem isto, quem perdeu o direito a `folha`
    /// continuaria vendo o nome dela na arvore e so descobriria a recusa ao
    /// clicar -- e o nome de uma tabela ja conta parte da historia. A arvore
    /// mostra o que da para abrir.
    pub(super) fn op_tabelas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let nome = p.texto_ou("database", "");
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(nome)?;
        let todas = db.todas_as_tabelas()?;
        let visiveis: Vec<Json> = todas
            .into_iter()
            .filter(|t| self.pode_ver_tabela(sessao, nome, t))
            .map(Json::texto_de)
            .collect();
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(nome)),
            (
                "schemas",
                Json::Lista(db.schemas()?.into_iter().map(Json::texto_de).collect()),
            ),
            ("tabelas", Json::Lista(visiveis)),
        ]))
    }

    /// Quem esta na sessao pode LER esta tabela desta base?
    ///
    /// Sem sessao -- servidor sem cadastro -- e sim: o portao de usuario nao
    /// existe naquele modo, e inventar um aqui negaria tudo.
    pub(super) fn pode_ver_tabela(&self, sessao: &Sessao, database: &str, tabela: &str) -> bool {
        match &sessao.usuario {
            None => true,
            Some(u) => u.pode_em(database, tabela, Atividade::Ler),
        }
    }

    pub(super) fn op_criar_database(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("database", "");
        // `tipo` ausente/vazio -> padrao, por compatibilidade: todo cliente
        // escrito antes dos tres tipos manda so `database`, e continua criando
        // padrao como sempre. Tipo desconhecido e' ERRO (nao palpite) -- a
        // recusa vem de `de_texto`, na DECLARACAO, antes de criar o diretorio.
        let tipo = phxsql_core::TipoDatabase::de_texto(p.texto_ou("tipo", ""))?;
        let dados = self.travar_dados()?;
        let (db, pendente) = dados.criar_database_com_tipo_adiando_o_fsync(nome, tipo)?;
        // Pedido 589: criar precisa da trava (dois criadores no mesmo nome);
        // o `fsync` do marcador e da entrada na base nao, e sob ela seria
        // secao nova na catraca `alcancam-fsync-2`. Solta, sincroniza, e so
        // entao responde -- o mesmo desenho do colar (586).
        drop(dados);
        pendente.levar_ao_disco()?;
        // O marcador ja foi gravado. Se o motor do tipo ainda nao existe, o
        // database nasce valido e com o tipo reservado, mas quem tentar operar
        // tabela nele recebe «motor em construcao» -- a nota avisa de antemao,
        // em vez de deixar o cliente descobrir no primeiro `criar_tabela`.
        let mut campos = vec![
            ("database", Json::texto_de(db.nome())),
            ("tipo", Json::texto_de(tipo.como_texto())),
            ("motor_pronto", Json::de_bool(tipo.motor_pronto())),
            (
                "caminho",
                Json::texto_de(db.caminho().display().to_string()),
            ),
        ];
        if !tipo.motor_pronto() {
            campos.push((
                "nota",
                Json::texto_de(format!(
                    "database do tipo {} criado; o motor dele esta em construcao, \
                     operacoes de tabela ainda nao funcionam",
                    tipo.como_texto()
                )),
            ));
        }
        Ok(Json::objeto(campos))
    }

    /// Copia uma tabela para outro database -- o "colar" da tela.
    ///
    /// O `duplicar_tabela` copia dentro do mesmo database; este atravessa. Sao
    /// duas operacoes e nao uma porque a permissao e a mesma mas o alcance
    /// nao: colar num database em que o usuario nao pode criar tem de recusar
    /// no database de DESTINO, e nao no de origem.
    pub(super) fn op_copiar_tabela(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let origem_db = p.texto_ou("database", "");
        let tabela = p.texto_ou("tabela", "");
        let destino_db = match p.texto_ou("destino_database", "").trim() {
            "" => origem_db,
            outro => outro,
        };
        let destino = match p.texto_ou("destino", "").trim() {
            "" => tabela,
            outro => outro,
        };
        // O portao geral confere a permissao contra o database do campo
        // `database`, que aqui e a ORIGEM. O destino precisa da sua propria
        // conferencia: sem esta linha, quem pode ler um database e nao pode
        // criar no outro conseguiria escrever onde nao devia.
        if let Some(u) = &sessao.usuario {
            if !u.pode_em(destino_db, destino, Atividade::Criar) {
                return Err(PhxError::Autorizacao(format!(
                    "sem permissao de criar em {destino_db}.{destino}"
                )));
            }
        }

        let dados = self.travar_dados()?;
        let origem = dados.abrir_database(origem_db)?;
        let alvo = dados.abrir_database(destino_db)?;
        let copia = origem.copiar_tabela_para_adiando_o_fsync(tabela, &alvo, destino)?;
        // Pedido 586: a copia precisa da trava (a origem nao muda no meio); o
        // `fsync` dela nao, e sob a trava seria secao nova na catraca
        // `alcancam-fsync-2`. Solta, sincroniza, e so entao responde.
        drop(dados);
        let copiados = copia.levar_ao_disco()?;
        Ok(Json::objeto(vec![
            ("origem_database", Json::texto_de(origem_db)),
            ("origem", Json::texto_de(tabela)),
            ("destino_database", Json::texto_de(destino_db)),
            ("destino", Json::texto_de(destino)),
            ("arquivos", Json::de_u64(copiados as u64)),
        ]))
    }

    /// `SysTables`: o catalogo de tabelas como se fosse uma tabela.
    ///
    /// Uma linha por tabela do database, com o que ela pesa. E o mesmo que a
    /// tela de gestao mostra, mas em forma de dado -- para quem quer consultar
    /// o catalogo em vez de olhar para ele.
    pub(super) fn op_sistabelas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        let mut linhas = Vec::new();
        for nome in db.todas_as_tabelas()? {
            // O catalogo e a mesma lista da arvore por outra porta: se ele nao
            // filtrasse, bastaria pedir `sistabelas` para saber tudo sobre a
            // tabela que a arvore esconde -- nome, colunas, quantas linhas.
            if !self.pode_ver_tabela(sessao, database, &nome) {
                continue;
            }
            let t = match db.abrir_qualificada(&nome) {
                Ok(t) => t,
                // Uma tabela ilegivel nao pode derrubar o catalogo inteiro: ela
                // vira uma linha que diz que esta ilegivel, que e exatamente a
                // informacao que alguem foi procurar ali.
                Err(e) => {
                    linhas.push(Json::objeto(vec![
                        ("tabela", Json::texto_de(&nome)),
                        ("erro", Json::texto_de(e.to_string())),
                    ]));
                    continue;
                }
            };
            let e = t.esquema().clone();
            let pag = e.paginacao();
            linhas.push(Json::objeto(vec![
                ("tabela", Json::texto_de(&nome)),
                (
                    "schema",
                    match nome.split_once('.') {
                        Some((sc, _)) => Json::texto_de(sc),
                        None => Json::texto_de(""),
                    },
                ),
                ("registros", Json::de_u64(t.registros())),
                ("slots", Json::de_u64(t.slots())),
                ("colunas", Json::de_u64(e.colunas().len() as u64)),
                ("indices", Json::de_u64(e.indices().len() as u64)),
                (
                    "chave_primaria",
                    match e.chave_primaria() {
                        None => Json::Nulo,
                        Some(k) => Json::texto_de(&k.nome),
                    },
                ),
                (
                    "chaves_estrangeiras",
                    Json::de_u64(e.chaves_estrangeiras().len() as u64),
                ),
                ("bytes_por_linha", Json::de_u64(e.payload_len() as u64)),
                ("paginada", Json::Bool(pag.ligada())),
                (
                    "particao",
                    Json::texto_de(match pag.modo.periodo() {
                        None if pag.ligada() => "quantidade".to_string(),
                        None => "".to_string(),
                        Some(p) => p.nome().to_string(),
                    }),
                ),
                ("volumes", Json::de_u64(t.fronteiras().len() as u64)),
            ]));
        }
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("total", Json::de_u64(linhas.len() as u64)),
            ("tabelas", Json::Lista(linhas)),
        ]))
    }

    /// `SysColumns`: uma linha por coluna de todas as tabelas do database.
    ///
    /// Aceita `tabela` para filtrar. E aqui que os metadados novos aparecem
    /// juntos -- id, caption, descricao, mascara e o papel nas chaves --, que e
    /// o que um dicionario de dados precisa mostrar.
    pub(super) fn op_siscolunas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let so_esta = p.texto_ou("tabela", "").trim().to_string();
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;
        let mut linhas = Vec::new();
        for nome in db.todas_as_tabelas()? {
            if !so_esta.is_empty() && so_esta != nome {
                continue;
            }
            if !self.pode_ver_tabela(sessao, database, &nome) {
                continue;
            }
            let Ok(t) = db.abrir_qualificada(&nome) else {
                continue;
            };
            let e = t.esquema();
            for (i, c) in e.colunas().iter().enumerate() {
                let papel = e.papel_da_coluna(i);
                linhas.push(Json::objeto(vec![
                    ("tabela", Json::texto_de(&nome)),
                    ("posicao", Json::de_u64(i as u64 + 1)),
                    ("id", Json::texto_de(c.id.to_string())),
                    ("nome", Json::texto_de(&c.nome)),
                    ("caption", Json::texto_de(&c.caption)),
                    ("descricao", Json::texto_de(&c.descricao)),
                    ("mascara", Json::texto_de(&c.mascara)),
                    ("dado_pessoal", Json::texto_de(c.dado_pessoal.nome())),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("tamanho", Json::de_u64(largura_do_tipo(&c.ty))),
                    ("obrigatoria", Json::Bool(!c.nullable)),
                    ("primaria", Json::Bool(papel.primaria)),
                    ("estrangeira", Json::Bool(papel.estrangeira)),
                    (
                        "composta",
                        Json::Bool(papel.primaria_composta || papel.estrangeira_composta),
                    ),
                    (
                        "nos_indices",
                        Json::Lista(papel.indices.iter().map(Json::texto_de).collect()),
                    ),
                ]));
            }
        }
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("total", Json::de_u64(linhas.len() as u64)),
            ("colunas", Json::Lista(linhas)),
        ]))
    }

    /// Onde estao os dados pessoais desta base (LGPD / GDPR).
    ///
    /// ```json
    /// { "op": "dados_pessoais", "database": "loja" }
    /// ```
    ///
    /// # O portao, que aqui e o assunto e nao o detalhe
    ///
    /// Esta operacao **nao tem campo `tabela`**: ela varre a base inteira. O
    /// portao geral do `despachar` confere o campo `"tabela"` do pedido, e um
    /// pedido sem ele cai na regra da BASE -- que e exatamente o furo ja
    /// documentado no `juntar` e no `unir`.
    ///
    /// Por isso a conferencia por tabela vem aqui dentro, com o mesmo
    /// `pode_ver_tabela` do `tabelas` e do `siscolunas`: quem nao pode ler a
    /// tabela nao descobre por este relatorio que ela existe, nem que ela
    /// guarda CPF. Um relatorio de conformidade que vaza o mapa do dado
    /// sensivel para quem nao pode le-lo e a pior versao possivel desta
    /// funcionalidade.
    ///
    /// # O que ele NAO faz
    ///
    /// Nao adivinha. Coluna chamada `cpf` sem marca nao entra no relatorio, e
    /// isso e de proposito: um mapa de conformidade deduzido por nome de campo
    /// da a quem le a sensacao de estar coberto sem estar. O relatorio conta
    /// quantas colunas ficaram SEM classificacao, que e o numero que diz o
    /// tamanho do trabalho que falta.
    pub(super) fn op_dados_pessoais(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "");
        let so_esta = p.texto_ou("tabela", "").trim().to_string();
        let dados = self.travar_dados()?;
        let db = dados.abrir_database(database)?;

        let mut achados = Vec::new();
        let mut tabelas_com = 0u64;
        let mut colunas_vistas = 0u64;
        let mut pessoais = 0u64;
        let mut sensiveis = 0u64;
        let mut sem_esquema = Vec::new();

        for nome in db.todas_as_tabelas()? {
            if !so_esta.is_empty() && so_esta != nome {
                continue;
            }
            if !self.pode_ver_tabela(sessao, database, &nome) {
                continue;
            }
            let Ok(t) = db.abrir_qualificada(&nome) else {
                // Tabela que nao abre nao vira erro do relatorio inteiro: ela
                // vira uma linha dizendo que nao foi conferida. Um relatorio
                // de conformidade que para no primeiro arquivo quebrado nao
                // audita nada.
                sem_esquema.push(Json::texto_de(&nome));
                continue;
            };
            let e = t.esquema();
            let mut da_tabela = Vec::new();
            for (i, c) in e.colunas().iter().enumerate() {
                if phxsql_core::schema::e_coluna_de_sistema(&c.nome) {
                    continue;
                }
                colunas_vistas += 1;
                if !c.dado_pessoal.e_pessoal() {
                    continue;
                }
                match c.dado_pessoal {
                    DadoPessoal::Sensivel => sensiveis += 1,
                    _ => pessoais += 1,
                }
                da_tabela.push(Json::objeto(vec![
                    ("posicao", Json::de_u64(i as u64 + 1)),
                    ("coluna", Json::texto_de(&c.nome)),
                    ("rotulo", Json::texto_de(c.rotulo())),
                    ("descricao", Json::texto_de(&c.descricao)),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("grau", Json::texto_de(c.dado_pessoal.nome())),
                    // Coluna pessoal que tambem e chave aparece em indice, e
                    // indice e o caminho por onde o dado sai sem ninguem ler a
                    // linha. Quem audita precisa ver isso junto.
                    (
                        "nos_indices",
                        Json::Lista(
                            e.papel_da_coluna(i)
                                .indices
                                .iter()
                                .map(Json::texto_de)
                                .collect(),
                        ),
                    ),
                ]));
            }
            if !da_tabela.is_empty() {
                tabelas_com += 1;
                achados.push(Json::objeto(vec![
                    ("tabela", Json::texto_de(&nome)),
                    ("registros", Json::de_u64(t.registros())),
                    // Um relatorio de conformidade que lista a coluna
                    // sensivel e nao diz se ela esta protegida no disco
                    // conta metade da historia. Ver `material_da_tabela`.
                    ("material", Json::texto_de(material_da_tabela(&t))),
                    ("total", Json::de_u64(da_tabela.len() as u64)),
                    ("colunas", Json::Lista(da_tabela)),
                ]));
            }
        }

        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("tabelas_com_dado_pessoal", Json::de_u64(tabelas_com)),
            ("colunas_pessoais", Json::de_u64(pessoais)),
            ("colunas_sensiveis", Json::de_u64(sensiveis)),
            // O numero que conta a HISTORIA: quanto ainda nao foi classificado.
            // Sem ele, uma base sem marca nenhuma parece uma base sem dado
            // pessoal -- e sao coisas muito diferentes.
            (
                "colunas_sem_classificacao",
                Json::de_u64(colunas_vistas.saturating_sub(pessoais + sensiveis)),
            ),
            ("colunas_conferidas", Json::de_u64(colunas_vistas)),
            ("tabelas_que_nao_abriram", Json::Lista(sem_esquema)),
            ("achados", Json::Lista(achados)),
        ]))
    }

    /// Quem esta falando com o servidor agora.
    ///
    /// E o `SHOW PROCESSLIST`: sem ele, quando uma consulta prende a trava de
    /// dados nao havia como saber QUEM esta segurando -- so que estava lento.
    pub(super) fn op_sessoes(&self) -> Result<Json> {
        let agora = crate::agora_ms();
        let l = self.ligacoes.tomar("ligacoes")?;
        let todas = l.todas();
        // A mais demorada primeiro: quando algo trava, e ela que interessa.
        let mais_longa = todas
            .iter()
            .filter(|x| x.op_desde_ms > 0)
            .map(|x| agora - x.op_desde_ms)
            .max()
            .unwrap_or(0);
        // As sessoes do navegador entram na MESMA lista. Quem pergunta "quem
        // esta conectado?" quer os dois -- e uma lista que so mostra a porta de
        // dados nao mostra quem esta olhando a propria tela.
        let web: Vec<Json> = self
            .sessoes
            .lock()
            .map(|s| {
                s.listar(agora)
                    .into_iter()
                    .map(|(id, login, desde, expira)| {
                        Json::objeto(vec![
                            ("id", Json::texto_de(&id)),
                            ("origem", Json::texto_de("web")),
                            (
                                "usuario",
                                match login.is_empty() {
                                    true => Json::Nulo,
                                    false => Json::texto_de(login),
                                },
                            ),
                            (
                                "desde",
                                Json::texto_de(phxsql_core::datahora::instante_iso(desde)),
                            ),
                            (
                                "aberta_s",
                                Json::de_u64(((agora - desde) / 1_000).max(0) as u64),
                            ),
                            (
                                "expira_em_s",
                                Json::de_u64(((expira - agora) / 1_000).max(0) as u64),
                            ),
                        ])
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(Json::objeto(vec![
            ("quantas", Json::de_u64(todas.len() as u64)),
            (
                "executando",
                Json::de_u64(todas.iter().filter(|x| !x.op.is_empty()).count() as u64),
            ),
            ("mais_longa_ms", Json::de_u64(mais_longa.max(0) as u64)),
            (
                "sessoes",
                Json::Lista(
                    todas
                        .iter()
                        .map(|x| {
                            let mut j = x.para_json(agora);
                            if let Json::Objeto(campos) = &mut j {
                                campos.push(("origem".into(), Json::texto_de("dados")));
                            }
                            j
                        })
                        .collect(),
                ),
            ),
            ("web", Json::Lista(web.clone())),
            ("sessoes_web", Json::de_u64(web.len() as u64)),
        ]))
    }

    // ------------------------------------------------------- telemetria

    /// O portao PROPRIO das operacoes de telemetria.
    ///
    /// # Por que ele existe, se ja ha o portao do `despachar`
    ///
    /// E a licao do `juntar`/`unir`, com o sinal trocado. La, o portao geral
    /// olhava o campo `"tabela"` e duas operacoes nao o tinham -- entao elas
    /// escapavam. Aqui a telemetria tambem nao tem `"tabela"` NEM
    /// `"database"`: o portao geral so consegue perguntar «este usuario pode
    /// administrar a base vazia?», e a resposta disso cai na regra `"*"` ou no
    /// nivel. Um usuario com `bases: {"*": {administrar: true}}` e nivel de
    /// leitor passaria.
    ///
    /// E a telemetria mostra, de todo mundo: o login, o IP, a operacao e a
    /// TABELA em que ela mexe. Quem ve isso ve o movimento de bases sobre as
    /// quais nao tem direito nenhum. Entao esta operacao pergunta o que o
    /// portao geral nao consegue perguntar: **e administrador deste servidor?**
    ///
    /// Sem cadastro de usuarios, quem entrou pelo token de servico continua
    /// podendo -- e assim que toda operacao de administracao ja funciona, e
    /// apertar isso aqui tiraria um direito que ninguem pediu para tirar.
    pub(super) fn portao_da_telemetria(&self, sessao: &Sessao) -> Result<()> {
        match &sessao.usuario {
            None => Ok(()),
            Some(u) if u.e_admin() => Ok(()),
            Some(u) => Err(PhxError::Autorizacao(format!(
                "{} nao e administrador deste servidor; a telemetria mostra o \
                 login, o IP e a tabela de todas as atividades",
                u.login
            ))),
        }
    }

    /// O painel de telemetria: as series do topo e as atividades vivas.
    ///
    /// Uma chamada so, como o `painel` e o `sistema`, e pelo mesmo motivo: a
    /// tela se atualiza sozinha, e tres idas e voltas por volta seriam tres
    /// vezes o custo para desenhar uma coisa so.
    pub(super) fn op_telemetria(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        self.portao_da_telemetria(sessao)?;
        let amostras = p.inteiro_ou("amostras", 120).clamp(1, 200) as usize;
        let agora = crate::agora_ms();
        let mut retrato = self.telemetria.para_json(agora, amostras);
        if let Json::Objeto(campos) = &mut retrato {
            let (acertos, faltas, gravacoes) = phxsql_store::ndx::contadores_de_cache();
            campos.push((
                "cache_ndx".into(),
                Json::objeto(vec![
                    (
                        "paginas_teto",
                        Json::de_u64(phxsql_store::ndx::cache_paginas() as u64),
                    ),
                    ("acertos", Json::de_u64(acertos)),
                    ("faltas", Json::de_u64(faltas)),
                    ("gravacoes", Json::de_u64(gravacoes)),
                    (
                        "acerto_percentual",
                        Json::texto_de(format!(
                            "{:.2}",
                            match acertos + faltas {
                                0 => 0.0,
                                t => acertos as f64 / t as f64 * 100.0,
                            }
                        )),
                    ),
                ]),
            ));
            campos.push(("tetos".into(), self.tetos_das_threads()));
            campos.push((
                "servidor".into(),
                Json::objeto(vec![
                    ("phxsql", Json::texto_de(VERSAO)),
                    (
                        "conexoes",
                        Json::de_u64(self.permissoes_de_dados.em_uso() as u64),
                    ),
                    (
                        "conexoes_max",
                        Json::de_u64(self.config.conexoes_max as u64),
                    ),
                    (
                        "no_ar_s",
                        Json::de_u64(((agora - self.desde_ms) / 1_000).max(0) as u64),
                    ),
                    ("gravacoes_pendentes", Json::de_u64(self.janela.pendente())),
                ]),
            ));
        }
        Ok(retrato)
    }

    /// A ocupacao VIVA de cada teto de threads, para o monitor (pedido 248).
    ///
    /// Os dois semaforos trazem `em_uso`, `teto` e `esperando` lidos agora;
    /// o fecho da janela e a varredura trazem so o teto, com `vivo: false`,
    /// porque as threads deles morrem no fim do `scope` e ninguem as conta
    /// no meio -- dizer zero ali seria inventar. `teto` nulo e «sem teto».
    pub(super) fn tetos_das_threads(&self) -> Json {
        let vivo = |familia: &str, s: &Semaforo| {
            Json::objeto(vec![
                ("familia", Json::texto_de(familia)),
                ("vivo", Json::Bool(true)),
                ("em_uso", Json::de_u64(s.em_uso() as u64)),
                (
                    "teto",
                    if s.limitado() {
                        Json::de_u64(s.teto() as u64)
                    } else {
                        Json::Nulo
                    },
                ),
                ("esperando", Json::de_u64(s.esperando() as u64)),
            ])
        };
        let fixo = |familia: &str, teto: usize| {
            Json::objeto(vec![
                ("familia", Json::texto_de(familia)),
                ("vivo", Json::Bool(false)),
                ("teto", Json::de_u64(teto as u64)),
            ])
        };
        Json::Lista(vec![
            vivo("dados", &self.permissoes_de_dados),
            vivo("http", &self.permissoes_http),
            fixo("fecho", FIOS_DO_FECHO),
            fixo("varredura", phxsql_core::paralelo::nucleos()),
        ])
    }

    /// Liga a coleta. Desligada ela custa um `load(Relaxed)` por ponto.
    pub(super) fn op_telemetria_ligar(&self, sessao: &Sessao) -> Result<Json> {
        self.portao_da_telemetria(sessao)?;
        let agora = crate::agora_ms();
        self.telemetria.ligar(agora);
        Ok(Json::objeto(vec![
            ("ligada", Json::Bool(true)),
            (
                "periodo_ms",
                Json::de_u64(crate::telemetria::PERIODO_DA_AMOSTRA_MS),
            ),
            (
                "aviso",
                Json::texto_de(
                    "a serie comeca vazia e a primeira amostra nao tem taxa: taxa \
                     so existe entre dois instantes",
                ),
            ),
        ]))
    }

    /// Desliga a coleta e joga a serie fora.
    pub(super) fn op_telemetria_desligar(&self, sessao: &Sessao) -> Result<Json> {
        self.portao_da_telemetria(sessao)?;
        self.telemetria.desligar();
        Ok(Json::objeto(vec![
            ("ligada", Json::Bool(false)),
            (
                "aviso",
                Json::texto_de(
                    "a serie foi descartada: um grafico com buraco no meio mente \
                     sobre o que aconteceu ali",
                ),
            ),
        ]))
    }

    /// Encerra a operacao em curso de UMA atividade -- o cancelamento
    /// cooperativo.
    ///
    /// # O que ele promete, e o que ele NAO promete
    ///
    /// Ele marca. A marca so e olhada em ponto seguro, e por isso a resposta
    /// diz em qual dos tres casos o pedido caiu:
    ///
    /// * `encerrando` -- a operacao esta numa fase cancelavel e vai abortar na
    ///   proxima unidade de trabalho;
    /// * `nao_cancelavel` -- ela esta dentro do ponto critico e vai TERMINAR;
    ///   a marca fica posta, mirando esta mesma operacao, e vale se ela ainda
    ///   entrar numa fase cancelavel antes do fim;
    /// * `ociosa` -- nao havia nada em curso.
    ///
    /// Prometer um `KILL` instantaneo seria mentir, e a mentira apareceria no
    /// pior momento: com o operador olhando para uma tela que diz «encerrada»
    /// enquanto a tabela continua sendo escrita.
    pub(super) fn op_telemetria_encerrar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        self.portao_da_telemetria(sessao)?;
        let id = p.texto_ou("id", "").trim().to_string();
        if id.is_empty() {
            return Err(PhxError::Esquema(
                "encerrar sem \"id\": ele vem da lista de `telemetria`, na forma \
                 dados:17 ou web:a1b2c3d4"
                    .into(),
            ));
        }
        let quem = match &sessao.usuario {
            Some(u) => u.login.clone(),
            None => "token de servico".to_string(),
        };
        let atividade = self.telemetria.atividade(&id).ok_or_else(|| {
            PhxError::NaoEncontrado(format!(
                "nao ha atividade {id:?}; a lista esta em `telemetria`"
            ))
        })?;
        // Encerrar a propria atividade seria pedir para a tela se matar no
        // meio de perguntar -- e o pedido morreria antes de responder o que
        // aconteceu.
        if let Some(minha) = crate::telemetria::corrente() {
            if minha.chave == id {
                return Err(PhxError::Esquema(
                    "esta e a sua propria atividade: encerra-la mataria o pedido \
                     que esta perguntando"
                        .into(),
                ));
            }
        }
        let agora = crate::agora_ms();
        let desfecho = atividade.encerrar(&quem);
        let (estado, op_alvo, aviso) = match &desfecho {
            crate::telemetria::Encerramento::Ociosa => (
                "ociosa",
                String::new(),
                "nao havia operacao em curso. Para derrubar a CONEXAO inteira, \
                 use `encerrar_sessao` -- ele fecha o soquete."
                    .to_string(),
            ),
            crate::telemetria::Encerramento::Marcada { op, fase } => (
                "encerrando",
                op.clone(),
                format!(
                    "a operacao esta em fase cancelavel ({}) e aborta na proxima \
                     unidade de trabalho. O que ja foi gravado continua gravado e \
                     o arquivo fica integro.",
                    if fase.is_empty() { "sem nome" } else { fase }
                ),
            ),
            crate::telemetria::Encerramento::Posta { op } => (
                "marcada",
                op.clone(),
                "esta operacao TEM ponto de cancelamento, mas nao esta nele \
                 agora -- tipicamente porque esta na fila da trava de dados. A \
                 marca fica posta e vale para o primeiro ponto seguro que \
                 vier; se ela ja tiver passado do ultimo, a operacao termina \
                 normalmente."
                    .to_string(),
            ),
            crate::telemetria::Encerramento::FaseNaoCancelavel { op } => (
                "nao_cancelavel",
                op.clone(),
                "esta operacao esta DENTRO do ponto critico e vai terminar: \
                 abandonar uma gravacao entre o slot e o indice deixaria a \
                 tabela mentindo. A marca fica posta e vale se ela ainda entrar \
                 numa fase cancelavel antes do fim."
                    .to_string(),
            ),
        };
        self.telemetria.contar_encerramento();
        // Vai para o log de acessos, e nao so para a resposta: derrubar o
        // trabalho de outra pessoa e um ato de administracao, e ato de
        // administracao tem de deixar rastro de quem fez o que e quando.
        self.anotar(&Acesso {
            quando_ms: agora,
            ip: String::new(),
            porta_origem: 0,
            op: "telemetria_encerrar".into(),
            usuario: quem.clone(),
            autenticado: true,
            ok: true,
            duracao_ms: 0,
            erro: Some(format!(
                "encerrar {id} ({}) -> {estado}",
                if op_alvo.is_empty() {
                    "sem operacao"
                } else {
                    &op_alvo
                }
            )),
            database: String::new(),
            tabela: String::new(),
            codigo: 0,
            ..Acesso::default()
        });
        Ok(Json::objeto(vec![
            ("id", Json::texto_de(&id)),
            ("estado", Json::texto_de(estado)),
            (
                "op",
                match op_alvo.is_empty() {
                    true => Json::Nulo,
                    false => Json::texto_de(&op_alvo),
                },
            ),
            ("quem", Json::texto_de(&quem)),
            (
                "quando",
                Json::texto_de(phxsql_core::datahora::instante_iso(agora)),
            ),
            ("aviso", Json::texto_de(aviso)),
        ]))
    }

    /// Derruba uma conexao pelo numero.
    ///
    /// E o `KILL` -- e o que ele alcanca esta dito na resposta, em vez de
    /// prometer mais do que faz: fecha o soquete, o que e imediato para a
    /// conexao parada esperando pedido. Uma operacao que ja entrou na trava de
    /// dados termina assim mesmo; o que muda e que o resultado nao vai para
    /// lugar nenhum e a conexao nao volta.
    pub(super) fn op_encerrar_sessao(&self, p: &Json) -> Result<Json> {
        // Quem e o alvo -- sessao do navegador ou conexao da porta de dados --
        // e dito PELO PEDIDO (`"tipo": "web"|"conexao"`), nunca adivinhado pela
        // forma do texto: o id web tem 8 digitos hex e 2,3% deles ((10/16)^8)
        // saem so com algarismos, indistinguiveis de um numero de conexao
        // (pedido 644). Sem o campo, so o que nao tem ambiguidade segue como
        // antes (numero JSON = conexao; texto com letra = web, que conexao
        // nunca tem); texto so de algarismos e RECUSADO dizendo por que --
        // guarda nova entra pedida, mas o palpite que derrubava a conexao
        // errada nao se mantem.
        let id_texto = p.campo("id").and_then(Json::texto);
        let web = match p.campo("tipo").and_then(Json::texto) {
            Some("web") => true,
            Some("conexao") => false,
            Some(outro) => {
                return Err(PhxError::Esquema(format!(
                    "encerrar_sessao: \"tipo\" e \"web\" ou \"conexao\", nao {}",
                    phxsql_core::error::citar(outro)
                )))
            }
            None => match id_texto {
                Some(t) if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) => {
                    return Err(PhxError::Esquema(
                        "encerrar_sessao: id so de algarismos e ambiguo (sessao web ou \
                         numero de conexao); mande \"tipo\": \"web\" ou \"conexao\""
                            .into(),
                    ))
                }
                Some(_) => true,
                None => false,
            },
        };
        if web {
            // Prefixo vazio casaria qualquer sessao: recusa antes.
            let Some(texto) = id_texto.filter(|t| !t.is_empty()) else {
                return Err(PhxError::Esquema(
                    "encerrar_sessao tipo web sem \"id\" de texto: o id vem da operacao `sessoes`"
                        .into(),
                ));
            };
            {
                let mut s = self.sessoes.tomar("sessoes")?;
                if !s.encerrar_por_prefixo(texto) {
                    // Pelo `citar`, como o instante e a duracao: o id vem do
                    // fio e nao tinha teto (parecer SEC do 497, P1).
                    return Err(PhxError::NaoEncontrado(format!(
                        "nao ha sessao web {}; a lista esta em `sessoes`",
                        phxsql_core::error::citar(texto)
                    )));
                }
                return Ok(Json::objeto(vec![
                    ("encerrada", Json::texto_de(texto)),
                    ("origem", Json::texto_de("web")),
                    ("estava", Json::texto_de("aberta")),
                    (
                        "aviso",
                        Json::texto_de(
                            "a sessao do navegador foi invalidada: o proximo clique cai no login",
                        ),
                    ),
                ]));
            }
        }
        let id = match (id_texto, p.campo("id")) {
            // tipo "conexao" com o numero escrito como texto.
            (Some(t), _) => t.parse::<i64>().unwrap_or(0),
            _ => p.inteiro_ou("id", 0),
        };
        if id <= 0 {
            return Err(PhxError::Esquema(
                "encerrar_sessao sem \"id\": o numero vem da operacao `sessoes`".into(),
            ));
        }
        let id = id as u64;
        let agora = crate::agora_ms();
        let mut l = self.ligacoes.tomar("ligacoes")?;
        let antes = l.todas().into_iter().find(|x| x.id == id);
        if !l.encerrar(id) {
            return Err(PhxError::NaoEncontrado(format!(
                "nao ha conexao {id}; a lista esta em `sessoes`"
            )));
        }
        let executando = antes.as_ref().map(|x| !x.op.is_empty()).unwrap_or(false);
        Ok(Json::objeto(vec![
            ("encerrada", Json::de_u64(id)),
            (
                "estava",
                Json::texto_de(if executando {
                    "executando"
                } else {
                    "esperando"
                }),
            ),
            (
                "op",
                match antes.as_ref().map(|x| x.op.clone()).unwrap_or_default() {
                    o if o.is_empty() => Json::Nulo,
                    o => Json::texto_de(o),
                },
            ),
            // Dito na resposta, e nao so na documentacao: quem manda encerrar
            // precisa saber se ja acabou ou se ainda vai acabar.
            (
                "aviso",
                Json::texto_de(if executando {
                    "a operacao em curso termina antes de a conexao fechar: nao ha como \
                     abandonar uma varredura no meio sem arriscar deixar a tabela aberta \
                     pela metade. O resultado nao vai para lugar nenhum"
                } else {
                    "a conexao estava esperando pedido e foi fechada na hora"
                }),
            ),
            (
                "quando",
                Json::texto_de(phxsql_core::datahora::instante_iso(agora)),
            ),
        ]))
    }
}
