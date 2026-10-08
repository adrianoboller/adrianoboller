//! Configuracao, usuario e diretivas.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

impl Servidor {
    // ----------------------------------------------- a porta de dados, na tela

    /// O que a tela do Serviço precisa saber para nao mentir.
    /// A configuracao como o servidor a entende AGORA.
    ///
    /// E o `para_json` do arquivo com os tres ajustes vivos por cima: depois
    /// de uma gravacao a quente, o arquivo e a memoria concordam, mas quem le
    /// so o arquivo veria o valor de antes de a tela mexer.
    pub(super) fn configuracao_json(&self) -> Json {
        let mut j = self.config.para_json();
        j.definir("max_linhas", Json::de_u64(self.max_linhas()));
        j.definir("somente_leitura", Json::Bool(self.somente_leitura()));
        j.definir("espelho", Json::Bool(self.espelho()));
        // As cores e os limiares do painel valem A QUENTE, entao quem manda e
        // o registro da telemetria, e nao o `Config` de quando o servidor
        // subiu. Sem esta linha a tela grava a cor, o painel a obedece na
        // volta seguinte, e a propria tela de Configuracoes anuncia «gravado,
        // vale no proximo arranque» -- mentindo sobre o que ela acabou de
        // fazer. Apareceu exercitando, e nao lendo.
        j.definir("telemetria", self.telemetria.pintura().para_json());
        // O aviso do pedido 214(b), estruturado e nao em prosa: a tela monta a
        // frase pela fabrica de idiomas. Campo AUSENTE quando a lista esta
        // preenchida -- assim quem le nao precisa distinguir `false` de
        // "este servidor e velho e nao sabe responder".
        if let Some(com_imagem) = self.replicacao_aberta() {
            j.definir(
                "replicacao_aberta",
                Json::objeto(vec![
                    ("aberta", Json::Bool(true)),
                    ("com_imagem_da_linha", Json::Bool(com_imagem)),
                    (
                        "ops",
                        Json::Lista(
                            OPS_DE_REPLICACAO
                                .iter()
                                .map(|o| Json::texto_de(*o))
                                .collect(),
                        ),
                    ),
                ]),
            );
        }
        // A lista de nos do cluster tambem vale A QUENTE desde o 217, e pelo
        // mesmo motivo das cores: o `Config` e o retrato do arranque, e a tela
        // que acabou de acrescentar um no leria a lista de antes -- calada.
        // Este e o encontro dos dois pedidos: o 218 fez o bloco aparecer, e
        // sem esta linha ele apareceria dizendo o cluster de ontem.
        if let (Some(estado), Some(Json::Objeto(pares))) = (&self.cluster, j.campo("cluster")) {
            let mut cl = Json::Objeto(pares.clone());
            // O quorum tambem e A QUENTE (pedido 207): o que vale e o do cubo.
            if let Some(cubo) = &self.quorum {
                cl.definir("quorum_minimo", Json::de_u64(cubo.minimo()));
                cl.definir(
                    "quorum_prazo_ms",
                    Json::de_u64(cubo.prazo().as_millis() as u64),
                );
            }
            cl.definir(
                "nos",
                Json::Lista(
                    estado
                        .lista()
                        .iter()
                        .map(|n| {
                            Json::objeto(vec![
                                ("id", Json::texto_de(&n.id)),
                                ("endereco", Json::texto_de(&n.endereco)),
                                ("porta", Json::de_u64(n.porta as u64)),
                                // Os MESMOS campos que `Cluster::para_json`
                                // escreve para o no do arquivo: a lista viva
                                // nao pode dizer menos que a do arranque.
                                ("tem_pino", Json::Bool(!n.chave_do_fio.is_empty())),
                            ])
                        })
                        .collect(),
                ),
            );
            j.definir("cluster", cl);
        }
        // A conta de usuarios sai do cadastro VIVO. O `Config::para_json` a
        // tira da fotografia do arranque, e desde o pedido 221 o cadastro
        // muda em vida: sem esta linha, criar um usuario pela tela deixaria o
        // painel de Configuracoes dizendo o numero de antes -- e numero
        // visivel que envelhece calado e o defeito que esta casa mais paga.
        {
            let c = self.cadastro();
            j.definir(
                "usuarios",
                Json::de_u64((c.usuarios.len() + usize::from(c.root.is_some())) as u64),
            );
        }
        // O que ja esta GRAVADO e ainda nao vale: campo que so aplica no
        // proximo arranque volta aqui com o valor do arquivo, para a tela
        // mostra-lo em vez de redesenhar o valor velho calada.
        if let Some(caminho) = &self.config.caminho {
            let divergentes = crate::config::divergencias_do_arquivo(caminho, &j);
            if !divergentes.is_empty() {
                j.definir("no_arquivo", Json::Objeto(divergentes));
            }
        }
        j
    }

    /// **O portao das diretivas, e ele e UM so.**
    ///
    /// O portao geral do `despachar` confere o campo `"tabela"` do pedido, e
    /// nenhuma destas operacoes tem tabela -- elas caem na regra da base
    /// vazia. Isso ja exige `administrar`, mas depender disso deixaria a
    /// guarda mais importante do servidor amarrada a um detalhe de resolucao
    /// de nome de base.
    ///
    /// Ele existe como funcao, e nao copiado em cada operacao, porque a
    /// segunda copia e sempre a que envelhece: `config_gravar`,
    /// `diretiva_gravar` e o `SHOW … SETTINGS` conferem AQUI, e o dia em que
    /// a regra mudar ela muda uma vez.
    fn exigir_administrar_config(&self, sessao: &Sessao) -> Result<()> {
        if let Some(u) = &sessao.usuario {
            if !u.pode_em("", "", Atividade::Administrar) {
                return Err(PhxError::Autorizacao(format!(
                    "{} nao tem permissao de administrar: as diretivas do \
                     servidor exigem esse poder",
                    u.login
                )));
            }
        }
        Ok(())
    }

    /// Grava campos do `config.json` pedidos pela tela.
    ///
    /// # O portao e proprio, e nao pode nao ser
    ///
    /// O portao geral do `despachar` confere o campo `"tabela"` do pedido, e
    /// esta operacao nao tem tabela nenhuma -- ela cai na regra da base vazia.
    /// Isso ja exige `administrar`, mas depender disso deixaria a guarda mais
    /// importante do servidor amarrada a um detalhe de resolucao de nome de
    /// base. A conferencia aqui dentro e explicita: sem `administrar`, nao
    /// grava, viesse o pedido por onde viesse.
    ///
    /// # O que ela NAO grava
    ///
    /// O que estiver fora de [`crate::config::CAMPOS_EDITAVEIS`]: token,
    /// seguranca, cadastro de usuarios, cifra, credencial de e-mail e
    /// replicacao. Uma sessao roubada nao abre o firewall, nao esvazia a lista
    /// de comandos proibidos, nao cria supervisor e nao vira este servidor
    /// para outro source. Esses continuam sendo edicao do arquivo.
    pub(super) fn op_config_gravar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        self.exigir_administrar(sessao, "gravar a configuracao do servidor")?;
        let Some(caminho) = self.config.caminho.clone() else {
            return Err(PhxError::Esquema(
                "este servidor nao subiu de um arquivo (--config): nao ha config.json para gravar"
                    .into(),
            ));
        };

        let Some(Json::Objeto(pares)) = p.campo("campos") else {
            return Err(PhxError::Esquema(
                "informe \"campos\" como objeto: {\"campos\":{\"max_linhas\":500}}".into(),
            ));
        };
        let mudancas: Vec<(String, Json)> =
            pares.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

        // O valor ANTERIOR sai do ARQUIVO, e antes da gravacao -- ler depois
        // devolveria o valor novo, e ler da memoria viva devolveria o do
        // arranque para os campos que so valem no proximo. O diario tem de
        // dizer de que valor se saiu, e o arquivo e quem sabe.
        let antes: Vec<Json> = {
            let arvore = crate::config_phz::ler_texto(&caminho)
                .ok()
                .and_then(|t| Json::analisar(&t).ok());
            mudancas
                .iter()
                .map(|(campo, _)| {
                    arvore
                        .as_ref()
                        .and_then(|a| crate::config::valor_em(a, campo))
                        .unwrap_or(Json::Nulo)
                })
                .collect()
        };

        let novo = crate::config::Config::gravar_campos(&caminho, &mudancas)?;

        // O efeito, para os que aplicam sem reiniciar. Os globais de processo
        // (cache do .ndx, volume do diario, teto de nucleos) valem para o que
        // abrir daqui para a frente; os tres vivos valem para o proximo
        // pedido, inclusive nesta mesma conexao.
        phxsql_store::ndx::definir_cache_paginas(novo.recursos.cache_paginas);
        novo.recursos.aplicar();
        novo.lgpd.aplicar();
        // O rodizio do Profiler vale para o arquivo CORRENTE: quem esta vendo
        // o arquivo crescer na tela e abaixa o teto quer o efeito agora, e nao
        // no proximo `profiler_ligar`.
        if let Ok(mut prof) = self.profiler.lock() {
            prof.definir_rodizio(novo.profiler.teto_do_arquivo(), novo.profiler.arquivos);
            // E a lista de tabelas sigilosas pelo mesmo motivo, com um peso a
            // mais: quem acrescenta uma tabela a `cifra.tabelas` esta pedindo
            // protecao, e protecao que so vale no proximo `profiler_ligar`
            // deixa uma janela aberta justamente enquanto alguem observa.
            prof.definir_sigilosas(&novo.cifra.tabelas);
            // A raiz vem junto da lista, sempre: sao as duas metades da MESMA
            // decisao desde o pedido 356, e separa-las e o que faria uma
            // sobreviver a outra numa refacao.
            prof.definir_raiz_dos_dados(&novo.base);
        }
        // O rodizio de `acessos.log` e `diretivas.log` -- pedido 228, mesmo
        // motivo do Profiler acima: quem baixou o teto na tela quer o efeito
        // no arquivo CORRENTE, e nao no proximo arranque do servidor.
        if let Ok(mut log) = self.log.lock() {
            log.definir_rodizio(novo.acessos.teto_do_arquivo(), novo.acessos.arquivos);
        }
        self.diario
            .definir_rodizio(novo.diretivas.teto_do_arquivo(), novo.diretivas.arquivos);
        // A cor vale na resposta seguinte da telemetria -- dois segundos. Cor
        // se escolhe VENDO, e uma que so aparecesse no proximo arranque seria
        // escolhida no escuro.
        self.telemetria.definir_pintura(novo.telemetria.clone());
        self.max_linhas_vivo
            .store(novo.max_linhas, Ordering::Relaxed);
        self.somente_leitura_vivo
            .store(novo.somente_leitura, Ordering::Relaxed);
        self.espelho_vivo.store(novo.espelho, Ordering::Relaxed);
        // O quorum vale A QUENTE (pedido 207, D-quente): o commit seguinte ja
        // espera -- ou deixa de esperar. O `gravar_campos` releu o arquivo
        // pelo `validar`, entao o que chega aqui ja passou pelas recusas.
        if let (Some(cubo), Some(c)) = (&self.quorum, &novo.cluster) {
            cubo.definir(c.quorum_minimo, c.quorum_prazo_ms);
        }

        // Quem mexeu, e no que. Um campo que muda o comportamento do servidor
        // inteiro nao pode mudar sem deixar rastro.
        let quem = match &sessao.usuario {
            Some(u) => u.login.clone(),
            None => "(token de servico)".to_string(),
        };
        let lista: Vec<String> = mudancas.iter().map(|(c, _)| c.clone()).collect();
        eprintln!(
            "config.json gravado por {quem}: {} | arquivo {}",
            lista.join(", "),
            caminho.display()
        );
        // E o DIARIO, com os nove campos. Ele entra AQUI, e nao no
        // `diretiva_gravar`: `ALTER SERVER SET` desemboca nesta funcao, entao
        // registrar la deixaria de fora tudo o que a tela de Configuracoes
        // grava -- que e por onde a maioria das mudancas passa hoje.
        for ((campo, depois), antes) in mudancas.iter().zip(antes.iter()) {
            self.anotar_no_diario(
                sessao,
                "",
                campo,
                antes.clone(),
                depois.clone(),
                p.texto_ou("motivo", ""),
            );
        }

        // O que ficou gravado e ainda NAO vale: a tela mostra isto ao lado do
        // campo, em vez de prometer efeito que so vem no proximo arranque.
        let esperando: Vec<Json> = mudancas
            .iter()
            .filter(|(c, _)| matches!(crate::config::campo_editavel(c), Some((_, false))))
            .map(|(c, _)| Json::texto_de(c))
            .collect();

        Ok(Json::objeto(vec![
            ("gravado", Json::Bool(true)),
            ("arquivo", Json::texto_de(caminho.display().to_string())),
            (
                "campos",
                Json::Lista(lista.iter().map(Json::texto_de).collect()),
            ),
            ("exigem_reinicio", Json::Lista(esperando)),
            // A configuracao inteira de volta, ja sem segredo nenhum dentro:
            // a tela redesenha do que o servidor entendeu, e nao do que ela
            // achava que tinha mandado.
            ("config", self.configuracao_json()),
        ]))
    }

    // ------------------------------------------------- o cadastro de usuarios

    /// `usuario_criar`, `usuario_alterar` e `usuario_excluir` -- as tres pela
    /// mesma porta, porque as tres fazem a MESMA coisa: ler o `config.json`,
    /// mexer na lista `usuarios`, gravar atomicamente e trocar o cadastro
    /// vivo.
    ///
    /// # A senha
    ///
    /// Chega em claro no pedido (o fio ja e cifrado -- `docs/CIFRA-DO-FIO.md`)
    /// e vira hash PBKDF2 dentro de `usuarios::aplicar_na_arvore`, ANTES de
    /// tocar em qualquer estrutura que se serialize. A resposta e a FICHA, que
    /// nunca traz senha nem hash; o `acessos.log` guarda a operacao e o login
    /// de quem pediu, nunca o corpo; e o Profiler tapa `senha` por analise da
    /// arvore, em qualquer profundidade.
    ///
    /// # Aplicacao a quente
    ///
    /// O cadastro vivo e trocado e a geracao anda. O proximo `login` ja aceita
    /// quem acabou de nascer, e quem foi excluido perde a ficha no proximo
    /// pedido da propria conexao -- ver `refrescar_a_sessao`.
    pub(super) fn op_usuario(
        &self,
        acao: crate::usuarios::Acao,
        p: &Json,
        sessao: &Sessao,
    ) -> Result<Json> {
        self.exigir_administrar(sessao, "mexer no cadastro de usuarios")?;
        let Some(caminho) = self.config.caminho.clone() else {
            return Err(PhxError::Esquema(
                "este servidor nao subiu de um arquivo (--config): nao ha config.json \
                 onde gravar o cadastro"
                    .into(),
            ));
        };

        // A ficha de quem pede sai da SESSAO, e nao do cadastro: e ela que as
        // duas guardas de si-mesmo consultam, e a sessao ja foi conferida
        // contra o cadastro vivo no `refrescar_a_sessao`.
        let quem = sessao.usuario.clone();
        let mut login = String::new();
        let mut avisos = Vec::new();
        let novo = crate::config::Config::gravar_a_secao(&caminho, "usuarios", |arvore| {
            login = crate::usuarios::aplicar_na_arvore(arvore, acao, p, quem.as_ref())?;
            // O cadastro que VAI ao disco, conferido contra o esquema antes
            // de ir -- pedido 235, pela MESMA passada do arranque. Dentro do
            // fecho porque a recusa tem de vir antes da gravacao: gravar e
            // depois recusar deixaria no arquivo a regra que o proximo
            // arranque recusaria, e o servidor nao subiria mais. O `de_json`
            // e relido aqui (o `aplicar_na_arvore` ja o leu para as guardas
            // de si-mesmo, e devolve so o login): e a analise de um arquivo
            // de poucos KiB, uma vez por operacao de cadastro. A trava de
            // dados e tomada so pelo tempo de abrir as tabelas citadas.
            let cadastro = crate::usuarios::Cadastro::de_json(arvore)?;
            let dados = self.travar_dados()?;
            avisos = cadastro
                .conferir_colunas(&mut |base, tabela| colunas_em_disco(&dados, base, tabela))?;
            Ok(())
        })?;

        // O cadastro vivo, e a geracao logo depois: a ordem importa. Quem ler
        // a geracao nova tem de achar o cadastro novo -- na ordem inversa,
        // uma conexao releria a ficha do cadastro VELHO e marcaria a geracao
        // nova, e ficaria com a ficha velha ate a mudanca seguinte.
        {
            let mut vivo = self
                .cadastro_vivo
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *vivo = novo.cadastro.clone();
        }
        self.cadastro_geracao.fetch_add(1, Ordering::Release);

        // Quem mexeu, e em quem. O cadastro nao muda sem deixar rastro, pelo
        // mesmo motivo do `config.json`.
        let autor = match &sessao.usuario {
            Some(u) => u.login.clone(),
            None => "(token de servico)".to_string(),
        };
        eprintln!(
            "cadastro gravado por {autor}: {} {login} | arquivo {}",
            match acao {
                crate::usuarios::Acao::Criar => "criou",
                crate::usuarios::Acao::Alterar => "alterou",
                crate::usuarios::Acao::Excluir => "excluiu",
            },
            caminho.display()
        );

        let mut pares = vec![
            ("gravado", Json::Bool(true)),
            ("login", Json::texto_de(&login)),
            (
                "acao",
                Json::texto_de(match acao {
                    crate::usuarios::Acao::Criar => "criado",
                    crate::usuarios::Acao::Alterar => "alterado",
                    crate::usuarios::Acao::Excluir => "excluido",
                }),
            ),
            ("arquivo", Json::texto_de(caminho.display().to_string())),
            (
                "aviso",
                Json::texto_de(
                    "vale agora, sem reiniciar: o proximo login ja enxerga o cadastro novo",
                ),
            ),
        ];
        // A ficha de volta, para a tela redesenhar do que o SERVIDOR entendeu
        // e nao do que ela achava que tinha mandado. Nunca traz o hash -- ha
        // teste que falha se trouxer.
        if let Some(u) = novo.cadastro.por_login(&login) {
            pares.push(("usuario", u.ficha()));
        }
        // A regra de coluna sobre tabela que ainda nao existe: aceita, e o
        // aviso vai a quem pediu -- na resposta, porque a lista de avisos do
        // cadastro so e impressa no arranque, e quem chamou a op nao esta
        // olhando o log. So quando ha, para a resposta de sempre nao mudar.
        if !avisos.is_empty() {
            for aviso in &avisos {
                eprintln!("AVISO: {aviso}");
            }
            pares.push((
                "avisos",
                Json::Lista(avisos.iter().map(Json::texto_de).collect()),
            ));
        }
        Ok(Json::objeto(pares))
    }

    // ------------------------------------------------------ as DIRETIVAS

    /// Uma linha no diario administrativo. Falha em silencio de proposito.
    ///
    /// # Por que o diario nao pode derrubar a gravacao
    ///
    /// A alternativa seria recusar a mudanca quando o diario nao grava -- e ai
    /// um disco cheio, ou um diretorio sem permissao, tirariam do
    /// administrador justamente o poder de consertar o servidor. A mudanca ja
    /// aconteceu e ja esta no `config.json`; o que se perde e a linha, e a
    /// perda vai para o erro padrao, que e onde o resto das queixas do
    /// arranque ja mora.
    fn anotar_no_diario(
        &self,
        sessao: &Sessao,
        banco: &str,
        recurso: &str,
        valor_anterior: Json,
        valor_novo: Json,
        motivo: &str,
    ) {
        let a = crate::diretivas::Alteracao {
            quando_ms: crate::agora_ms(),
            servidor: self.config.bind.clone(),
            banco: banco.to_string(),
            recurso: recurso.to_string(),
            valor_anterior,
            valor_novo,
            usuario: match &sessao.usuario {
                Some(u) => u.login.clone(),
                None => "(token de servico)".to_string(),
            },
            ip_origem: sessao.ip.clone(),
            motivo: motivo.to_string(),
        };
        if let Err(e) = self.diario.registrar(&a) {
            eprintln!(
                "nao consegui anotar em {}: {e}",
                self.diario.caminho().display()
            );
        }
    }

    /// `SHOW … SETTINGS`: o que esta valendo, por escopo.
    pub(super) fn op_diretivas(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        self.exigir_administrar_config(sessao)?;
        let escopo = p.texto_ou("escopo", "servidor").trim().to_lowercase();
        let mut r = match escopo.as_str() {
            "servidor" | "server" => self.diretivas_do_servidor(),
            "database" | "banco" | "schema" => self.diretivas_da_base(p, sessao)?,
            "tabela" | "table" => self.diretivas_da_tabela(p, sessao)?,
            "conexao" | "connection" => self.diretivas_da_conexao(sessao),
            outro => {
                return Err(PhxError::Esquema(format!(
                    "escopo {outro:?} nao existe: use servidor, database, tabela ou conexao"
                )))
            }
        };
        // O diario junto, e isso e decisao: a pergunta que uma pessoa faz ao
        // ver uma diretiva estranha e «quem mexeu nisso?», e diario que so se
        // le por outro comando e diario que ninguem le.
        let quantas = p.inteiro_ou("diario", 0).clamp(0, 500) as usize;
        if quantas > 0 {
            let linhas: Vec<Json> = self
                .diario
                .ultimas(quantas)
                .iter()
                .map(crate::diretivas::Alteracao::para_json)
                .collect();
            r.definir("diario", Json::Lista(linhas));
        }
        Ok(r)
    }

    /// Os campos do `config.json` que se gravam, com valor, tipo e alcance.
    ///
    /// A lista sai de `CAMPOS_EDITAVEIS`, que e a MESMA que a tela usa e a
    /// mesma que o `config_gravar` confere. Uma segunda lista aqui divergiria
    /// no primeiro campo que alguem acrescentasse de um lado so -- e a que
    /// envelhece e sempre a que ninguem compila contra a outra.
    fn diretivas_do_servidor(&self) -> Json {
        let vivo = self.configuracao_json();
        // O que ja esta GRAVADO e ainda nao vale.
        //
        // Achado exercitando, e nao lendo: `ALTER SERVER SET timeout_s = 45`
        // gravava 45 e o `SHOW` seguinte respondia 30, calado -- porque
        // `configuracao_json` devolve o valor VIVO. E o mesmo defeito que a
        // tela de Configuracoes ja pagou uma vez ("quem acabou de digitar 90
        // via 45 de novo"), reaparecido pela porta nova. Aqui os dois valores
        // aparecem lado a lado, e a resposta deixa de mentir sobre o que
        // acabou de acontecer.
        let no_arquivo: Vec<(String, Json)> = vivo
            .campo("no_arquivo")
            .and_then(|j| match j {
                Json::Objeto(pares) => Some(pares.clone()),
                _ => None,
            })
            .unwrap_or_default();
        let campos: Vec<Json> = crate::config::CAMPOS_EDITAVEIS
            .iter()
            .map(|(campo, tipo, quente)| {
                let sigiloso = crate::diretivas::campo_sigiloso(campo);
                let valor = match crate::config::valor_em(&vivo, campo) {
                    // Segredo nunca sai daqui, e a regra e por NOME de campo:
                    // lista de segredos envelhece calada.
                    _ if sigiloso => Json::texto_de(crate::diretivas::OCULTO),
                    Some(v) => v,
                    None => Json::Nulo,
                };
                let gravado = no_arquivo.iter().find(|(c, _)| c == campo).map(|(_, v)| {
                    if sigiloso {
                        Json::texto_de(crate::diretivas::OCULTO)
                    } else {
                        v.clone()
                    }
                });
                let mut pares = vec![
                    ("recurso", Json::texto_de(*campo)),
                    ("valor", valor),
                    ("tipo", Json::texto_de(tipo.nome())),
                    (
                        "aplica",
                        Json::texto_de(if *quente {
                            "a quente"
                        } else {
                            "exige reinicio"
                        }),
                    ),
                    ("a_quente", Json::Bool(*quente)),
                    ("escopo", Json::texto_de("servidor")),
                ];
                if let Some(g) = gravado {
                    pares.push(("no_arquivo", g));
                    pares.push(("esperando_reinicio", Json::Bool(true)));
                }
                Json::objeto(pares)
            })
            .collect();
        Json::objeto(vec![
            ("escopo", Json::texto_de("servidor")),
            ("servidor", Json::texto_de(&self.config.bind)),
            (
                "arquivo",
                match &self.config.caminho {
                    Some(c) => Json::texto_de(c.display().to_string()),
                    None => Json::Nulo,
                },
            ),
            ("versao", Json::texto_de(env!("CARGO_PKG_VERSION"))),
            ("configuraveis", Json::de_u64(campos.len() as u64)),
            ("recursos", Json::Lista(campos)),
        ])
    }

    /// O que vale NESTE banco.
    fn diretivas_da_base(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let base = p.texto_ou("database", "").trim().to_string();
        if base.is_empty() {
            return Err(PhxError::Esquema(
                "informe o banco: SHOW DATABASE <banco> SETTINGS".into(),
            ));
        }
        // A base tem de existir: dizer as diretivas de um banco que nao ha
        // seria responder sobre nada com cara de resposta.
        let tabelas = {
            let trava = self.travar_dados()?;
            trava.abrir_database(&base)?.tabelas(None)?.len()
        };
        let (gatilhos, procedimentos) = {
            let r = self.rotinas.tomar("rotinas")?;
            (
                r.gatilhos_do_db(&base).len(),
                r.procedimentos_do_db(&base).len(),
            )
        };
        let daqui: Vec<Json> = self
            .proibidos_por_base
            .lock()
            .map(|l| {
                l.iter()
                    .filter(|(b, _)| *b == base)
                    .map(|(_, c)| Json::texto_de(c))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Json::objeto(vec![
            ("escopo", Json::texto_de("database")),
            ("servidor", Json::texto_de(&self.config.bind)),
            ("banco", Json::texto_de(&base)),
            ("tabelas", Json::de_u64(tabelas as u64)),
            ("gatilhos", Json::de_u64(gatilhos as u64)),
            ("procedimentos", Json::de_u64(procedimentos as u64)),
            // As transacoes NAO se ligam e desligam por banco: elas existem
            // por conexao, sempre, e quem nao abre uma nao paga nada. Ver
            // docs/DIRETIVAS.md.
            (
                "transacoes",
                Json::texto_de("sempre disponiveis, por conexao"),
            ),
            (
                "journal",
                Json::texto_de("sempre ligado, por tabela (.log)"),
            ),
            (
                "base_proibida",
                Json::Bool(self.config.politica.base_proibida(&base)),
            ),
            (
                "comandos_proibidos_globais",
                Json::Lista(
                    self.config
                        .politica
                        .comandos_proibidos
                        .iter()
                        .map(Json::texto_de)
                        .collect(),
                ),
            ),
            ("comandos_proibidos_da_base", Json::Lista(daqui)),
            (
                "somente_leitura",
                Json::Bool(self.somente_leitura_vivo.load(Ordering::Relaxed)),
            ),
            ("_sessao", Json::texto_de(sessao.login())),
        ]))
    }

    /// O que a TABELA ja declara -- e as tres diretivas do HFSQL lidas nela.
    ///
    /// Tudo aqui e leitura da declaracao, e nao um bloco de configuracao
    /// paralelo: `duplicate_check` e o `unico` do indice, a integridade
    /// referencial e o `verificar` da chave, e o journal e o `.log` que toda
    /// tabela tem. Guardar uma copia disso num `diretivas.json` criaria uma
    /// segunda verdade ao lado do esquema -- e a segunda e sempre a que
    /// diverge.
    fn diretivas_da_tabela(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let base = p.texto_ou("database", "").trim().to_string();
        let tabela = p.texto_ou("tabela", "").trim().to_string();
        if tabela.is_empty() {
            return Err(PhxError::Esquema(
                "informe a tabela: SHOW TABLE <tabela> SETTINGS".into(),
            ));
        }
        let ped = Json::objeto(vec![
            ("database", Json::texto_de(&base)),
            ("tabela", Json::texto_de(&tabela)),
        ]);
        let esquema = self.executar_derivado("esquema", &ped, sessao)?;
        let indices = esquema.campo("indices").cloned().unwrap_or(Json::Nulo);
        let fks = esquema
            .campo("chaves_estrangeiras")
            .cloned()
            .unwrap_or(Json::Nulo);
        let duplicidade: Vec<Json> = indices
            .lista()
            .unwrap_or_default()
            .iter()
            .map(|i| {
                Json::objeto(vec![
                    ("indice", Json::texto_de(i.texto_ou("nome", ""))),
                    // `duplicate_check` do HFSQL e o inverso do `unico`: la se
                    // liga a CONFERENCIA de duplicidade, aqui se declara que a
                    // chave e unica. Mesma garantia, nome pelo avesso.
                    ("duplicate_check", Json::Bool(i.booleano_ou("unico", false))),
                    ("unico", Json::Bool(i.booleano_ou("unico", false))),
                ])
            })
            .collect();
        let integridade: Vec<Json> = fks
            .lista()
            .unwrap_or_default()
            .iter()
            .map(|f| {
                Json::objeto(vec![
                    ("chave", Json::texto_de(f.texto_ou("nome", ""))),
                    ("tabela_ref", Json::texto_de(f.texto_ou("tabela_ref", ""))),
                    (
                        "referential_integrity",
                        Json::Bool(f.booleano_ou("verificar", false)),
                    ),
                    ("ao_excluir", Json::texto_de(f.texto_ou("ao_excluir", ""))),
                    ("ao_alterar", Json::texto_de(f.texto_ou("ao_alterar", ""))),
                ])
            })
            .collect();
        let gatilhos = {
            let r = self.rotinas.tomar("rotinas")?;
            r.gatilhos_do_db(&base)
                .iter()
                .filter(|g| g.tabela == tabela)
                .count()
        };
        Ok(Json::objeto(vec![
            ("escopo", Json::texto_de("tabela")),
            ("servidor", Json::texto_de(&self.config.bind)),
            ("banco", Json::texto_de(&base)),
            ("tabela", Json::texto_de(&tabela)),
            ("duplicidade", Json::Lista(duplicidade)),
            ("integridade_referencial", Json::Lista(integridade)),
            ("triggers", Json::de_u64(gatilhos as u64)),
            ("journal", Json::Bool(true)),
            (
                "motivo_obrigatorio",
                Json::Bool(esquema.booleano_ou("motivo_obrigatorio", false)),
            ),
            (
                "gravavel_por_diretiva",
                // Dispensa registrada, dita na propria resposta: ver
                // `docs/DIRETIVAS.md` §4.
                Json::Lista(Vec::new()),
            ),
        ]))
    }

    /// O que e verdade DESTA conexao.
    fn diretivas_da_conexao(&self, sessao: &Sessao) -> Json {
        Json::objeto(vec![
            ("escopo", Json::texto_de("conexao")),
            ("servidor", Json::texto_de(&self.config.bind)),
            ("usuario", Json::texto_de(sessao.login())),
            ("ip_origem", Json::texto_de(&sessao.ip)),
            ("ligacao", Json::de_u64(sessao.ligacao)),
            // A cifra do fio existe e se negocia no APERTO DE MAO, nao por
            // diretiva -- `docs/CIFRA-DO-FIO.md`. Aqui se diz o estado.
            //
            // Por onde esta conexao entrou. Sem isto, `encryption_exigida`
            // seria um `false` que quem le nao sabe interpretar -- «e porque
            // nao exigem, ou porque esta porta nao tem fio?». E e o campo que
            // faz as tres portas do `Entrada` serem LIDAS, e nao so
            // modeladas: variante que ninguem distingue e chave morta.
            (
                "via",
                Json::texto_de(match sessao.entrada {
                    Entrada::Dados => "dados",
                    Entrada::Http => "http",
                    Entrada::SemFio => "interna",
                }),
            ),
            // `encryption` e a CAPACIDADE do servidor (ele atende o aperto), e
            // continua sendo: e o que a bancada de diretivas pergunta.
            ("encryption", Json::Bool(self.config.cifra_fio.ligada)),
            // Este e o que o pedido 370 consertou: a verdade DESTA conexao, e
            // nao um campo do `config.json` publicado para quem quer que
            // pergunte. Ate 18/09/2026 a conexao HTTP em claro perguntava e
            // recebia `true`.
            (
                "encryption_neste_canal",
                Json::Bool(sessao.transcricao_do_fio.is_some()),
            ),
            (
                "encryption_exigida",
                // «o texto claro seria recusado NESTA conexao?» -- e so a
                // porta de dados pode responder sim. Numa porta HTTP com a
                // exigencia ligada, quem esta do outro lado ou foi recusado
                // (e nao esta aqui perguntando) ou declarou o proxy: e ai
                // quem cifra e o TLS de fora, que este servidor NAO tem como
                // conferir -- e protecao que nao se confere nao se anuncia.
                Json::Bool(self.config.cifra_fio.exigir && sessao.entrada == Entrada::Dados),
            ),
            // A compressao existe, mas nao e um estado DESTA conexao: e
            // pedida por PEDIDO (`"aceita_compressao":true`), nunca por
            // aperto nem por diretiva -- entao o que se relata aqui e a
            // CAPACIDADE do servidor, e nao "esta conexao esta comprimindo
            // agora" (ela pode estar, no proximo pedido, ou nao).
            ("compression", Json::Bool(true)),
            // E nunca dentro do tunel: comprimir antes de cifrar vaza
            // tamanho (estilo CRIME/BREACH). Decisao de seguranca registrada
            // em docs/CIFRA-DO-FIO.md, nao revisitada por esta diretiva.
            ("compression_no_tunel", Json::Bool(false)),
            (
                "max_linhas",
                Json::de_u64(self.max_linhas_vivo.load(Ordering::Relaxed)),
            ),
            ("timeout_s", Json::de_u64(self.config.timeout_s)),
            (
                "somente_leitura",
                Json::Bool(self.somente_leitura_vivo.load(Ordering::Relaxed)),
            ),
            ("gravavel_por_diretiva", Json::Lista(Vec::new())),
        ])
    }

    /// `ALTER … SET`: muda uma diretiva, por escopo.
    ///
    /// # O escopo de SERVIDOR nao tem caminho proprio
    ///
    /// Ele monta o pedido do `config_gravar` e chama a MESMA funcao. Mesmo
    /// portao, mesma conferencia de tipo, mesma gravacao atomica, mesma
    /// aplicacao a quente, mesmo diario. Um segundo caminho de gravacao ao
    /// lado daquele seria a porta dos fundos que a lei desta casa manda
    /// procurar: *portao de permissao e UM so*.
    pub(super) fn op_diretiva_gravar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let escopo = p.texto_ou("escopo", "servidor").trim().to_lowercase();
        let campo = p.texto_ou("campo", "").trim().to_string();
        if campo.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"campo\": ALTER <escopo> SET <campo> = <valor>".into(),
            ));
        }
        let valor = p.campo("valor").cloned().unwrap_or(Json::Nulo);
        match escopo.as_str() {
            "servidor" | "server" => {
                let pedido = Json::objeto(vec![
                    ("campos", Json::Objeto(vec![(campo, valor)])),
                    ("motivo", Json::texto_de(p.texto_ou("motivo", ""))),
                ]);
                self.op_config_gravar(&pedido, sessao)
            }
            "database" | "banco" | "schema" => self.diretiva_da_base_gravar(p, sessao),
            "tabela" | "table" => Err(PhxError::Esquema(format!(
                "ALTER TABLE nao grava {campo:?}, e nenhuma outra: o que a tabela \
                 tem se declara. `duplicate_check` e o `unico` do indice \
                 (criar_tabela); `referential_integrity` e o `verificar` da chave \
                 (declarar_fk, e ela NASCE conferida). SHOW TABLE <t> SETTINGS \
                 mostra as duas. O motivo esta em docs/DIRETIVAS.md"
            ))),
            "conexao" | "connection" => Err(PhxError::Esquema(format!(
                "ALTER CONNECTION nao grava {campo:?}, e nenhuma outra: nao ha \
                 ajuste por conexao neste servidor. A cifra do fio se negocia no \
                 aperto de mao (op `cifrar`); a compressao do fio se pede por \
                 PEDIDO (\"aceita_compressao\", so no caminho claro) -- nenhuma \
                 das duas e diretiva, entao ALTER CONNECTION para qualquer uma \
                 delas nao existe. SHOW CONNECTION SETTINGS diz o estado desta"
            ))),
            outro => Err(PhxError::Esquema(format!(
                "escopo {outro:?} nao existe: use servidor, database, tabela ou conexao"
            ))),
        }
    }

    /// A primeira diretiva POR BANCO de verdade: `comandos_proibidos`.
    ///
    /// # Ela so acrescenta, e isso e a guarda e nao a falta dela
    ///
    /// O global continua valendo em todos os bancos; o do banco APERTA e nunca
    /// afrouxa. Retirar continua sendo edicao do `config.json` -- e a mesma
    /// razao pela qual `seguranca.*` ficou fora do `CAMPOS_EDITAVEIS`: uma
    /// sessao roubada nao esvazia a lista de comandos proibidos.
    fn diretiva_da_base_gravar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        self.exigir_administrar_config(sessao)?;
        let campo = p.texto_ou("campo", "").trim().to_string();
        if campo != "comandos_proibidos" {
            return Err(PhxError::Esquema(format!(
                "ALTER DATABASE nao grava {campo:?}: a unica diretiva por banco \
                 hoje e \"comandos_proibidos\". Transacoes, journal e integridade \
                 referencial nao se ligam por banco aqui -- SHOW DATABASE <b> \
                 SETTINGS diz o que cada uma e, e docs/DIRETIVAS.md diz por que"
            )));
        }
        let base = p.texto_ou("database", "").trim().to_string();
        let Some(caminho) = self.config.caminho.clone() else {
            return Err(PhxError::Esquema(
                "este servidor nao subiu de um arquivo (--config): nao ha config.json para gravar"
                    .into(),
            ));
        };
        // Um nome so, ou uma lista deles.
        let comandos: Vec<String> = match p.campo("valor") {
            Some(Json::Lista(l)) => l
                .iter()
                .filter_map(Json::texto)
                .map(str::to_string)
                .collect(),
            Some(Json::Texto(t)) => vec![t.clone()],
            _ => {
                return Err(PhxError::Esquema(
                    "informe os comandos: ALTER DATABASE <b> SET comandos_proibidos = (a, b)"
                        .into(),
                ))
            }
        };
        // Proibir uma operacao que nao existe e engano de digitacao, e ele
        // custa caro: a lista fica com uma linha que nunca casa e quem a
        // escreveu acha que fechou a porta. O catalogo e quem sabe os nomes.
        for c in &comandos {
            let alvo = c.trim().to_lowercase();
            if !crate::catalogo::OPERACOES
                .iter()
                .any(|o| o.nome == alvo || o.apelidos.contains(&alvo.as_str()))
            {
                return Err(PhxError::Esquema(format!(
                    "{c:?} nao e uma operacao deste servidor: proibir um nome que \
                     nao existe deixa a lista com uma guarda que nunca fecha. \
                     A op `catalogo` lista as que ha"
                )));
            }
        }

        let antes: Vec<Json> = self
            .proibidos_por_base
            .lock()
            .map(|l| {
                l.iter()
                    .filter(|(b, _)| *b == base)
                    .map(|(_, c)| Json::texto_de(c))
                    .collect()
            })
            .unwrap_or_default();

        let (novo, entraram, existiam) =
            crate::config::Config::acrescentar_proibidos_da_base(&caminho, &base, &comandos)?;

        // A quente: aperto que so valesse no proximo arranque deixaria aberta
        // justamente a janela em que alguem esta fechando a porta.
        let vivos = novo.politica.proibidos_por_base.clone();
        if let Ok(mut l) = self.proibidos_por_base.lock() {
            *l = vivos.clone();
        }
        self.ha_proibidos_por_base
            .store(!vivos.is_empty(), Ordering::Relaxed);

        let depois: Vec<Json> = vivos
            .iter()
            .filter(|(b, _)| *b == base)
            .map(|(_, c)| Json::texto_de(c))
            .collect();
        self.anotar_no_diario(
            sessao,
            &base,
            "comandos_proibidos",
            Json::Lista(antes),
            Json::Lista(depois.clone()),
            p.texto_ou("motivo", ""),
        );

        Ok(Json::objeto(vec![
            ("gravado", Json::Bool(true)),
            ("escopo", Json::texto_de("database")),
            ("banco", Json::texto_de(&base)),
            ("recurso", Json::texto_de("comandos_proibidos")),
            (
                "acrescentados",
                Json::Lista(entraram.iter().map(Json::texto_de).collect()),
            ),
            (
                "ja_existiam",
                Json::Lista(existiam.iter().map(Json::texto_de).collect()),
            ),
            ("comandos_proibidos_da_base", Json::Lista(depois)),
            ("aplica", Json::texto_de("a quente")),
            // Dito na resposta, e nao so no manual: quem esperava que o `SET`
            // substituisse a lista precisa descobrir agora, e nao no dia em
            // que uma proibicao que ele achou ter tirado continuar valendo.
            (
                "aviso",
                Json::texto_de(
                    "esta diretiva so ACRESCENTA; para retirar, edite \
                     seguranca.comandos_proibidos no config.json",
                ),
            ),
            ("arquivo", Json::texto_de(caminho.display().to_string())),
        ]))
    }
}
