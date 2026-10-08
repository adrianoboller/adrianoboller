//! O DBlink.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

impl Servidor {
    // ------------------------------------------------------------- o DbLink

    /// As ligacoes cadastradas. A senha nunca vem junto.
    pub(super) fn op_dblink(&self) -> Result<Json> {
        let r = self.dblink.tomar("dblink")?;
        // A lista vazia de um cadastro trancado seria mentira: a tela diria
        // «nenhuma ligacao» com o arquivo cheio delas (pedido 466).
        r.exigir_legivel()?;
        Ok(Json::objeto(vec![
            ("arquivo", Json::texto_de(r.caminho.display().to_string())),
            // Se o cadastro e cifrado e de onde a chave vem -- nunca a chave.
            ("cifra_do_cadastro", r.estado_da_cifra()),
            (
                "ligacoes",
                Json::Lista(r.ligacoes.iter().map(Definicao::para_json).collect()),
            ),
            (
                "motores",
                Json::Lista(
                    [Motor::MySql, Motor::Postgres, Motor::Phx]
                        .iter()
                        .map(|m| {
                            Json::objeto(vec![
                                ("nome", Json::texto_de(m.nome())),
                                ("porta", Json::de_u64(m.porta_padrao() as u64)),
                                ("conecta", Json::Bool(m.conecta())),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    /// Cria ou substitui uma ligacao.
    ///
    /// Sem o campo `senha`, a senha que ja estava fica -- desde que o DESTINO
    /// seja o mesmo. Isso e o que faz a tela de edicao funcionar: ela nunca
    /// RECEBE a senha, entao nao teria como devolve-la, e sem esta regra editar
    /// o timeout apagaria a credencial. Trocar host, porta, motor, usuario ou
    /// pino sem mandar a credencial RECUSA (pedido 470); a recusa mora nas
    /// funcoes de heranca, e nao aqui, para valer em todo caminho que herda.
    pub(super) fn op_dblink_salvar(&self, p: &Json) -> Result<Json> {
        let mut r = self.dblink.tomar("dblink")?;
        let mut d = Definicao::de_json(p)?;
        if let Ok(antiga) = r.achar(&d.nome) {
            // O pino vem PRIMEIRO porque e parte do destino: a heranca da
            // senha e do token compara o pino, e pino ausente no pedido (a
            // tela nunca o manda) nao pode contar como pino trocado. O motivo
            // da heranca dele esta mais abaixo, junto da conferencia do motor.
            if p.campo("chave_do_fio").is_none() {
                d = d.com_o_pino_de(antiga);
            }
            if p.campo("pino_tls").is_none() {
                d = d.com_o_pino_tls_de(antiga);
            }
            if p.campo("senha").is_none() && d.senha_env.is_empty() {
                d = d.com_a_senha_de(antiga)?;
            }
            // O token tem a CONDICAO DELE, e nao a da senha: quem troca so a
            // senha de uma ligacao `phxsql` nao pode perder a chave da porta
            // da rede -- e a tela nunca o recebe de volta ("(oculto)").
            if p.campo("token_remoto").is_none() && d.token_env.is_empty() {
                d = d.com_o_token_de(antiga)?;
            }
            // A tela salva sem mandar as sincronias; um salvar comum nao pode
            // apagar o que o assistente montou.
            if p.campo("sincronias").is_none() {
                d = d.com_as_sincronias_de(antiga);
            }
            // O teto de bytes (pedido 546) pela mesma regra: a tela nao o
            // manda, e quem o subiu no arquivo para uma sincronia grande nao
            // pode ve-lo voltar ao de fabrica numa troca de porta.
            if p.campo("max_mib").is_none() {
                d = d.com_o_teto_de_bytes_de(antiga);
            }
            // A CIFRA tem a condicao dela, e nao a do pino: herda-se a DECISAO
            // escrita (o `Option`), para que um salvar que nao fala de cifra
            // nao vire «ninguem decidiu» -- e para que quem escreveu
            // `"cifra": false` nao seja religado por uma troca de porta.
            if p.campo("cifra").is_none() {
                d = d.com_a_cifra_de(antiga);
            }
            // O PINO (herdado no topo deste bloco) tem a condicao DELE, e foi o
            // lugar que faltava: a tela nunca recebe o pino de volta
            // (`para_json` so da `tem_pino`, porque a lista de quem tem pino e
            // mapa para atacante), entao sem aquela heranca TODO salvar pela
            // tela apagaria o pino -- e a ligacao continuaria anunciando
            // «cifrada», com o tunel sem ancora e o painel identico.
            // Rebaixamento silencioso e o pior dos dois.
            //
            // Campo AUSENTE herda; campo presente e vazio APAGA, que e decisao
            // escrita -- e e por isso que as condicoes nao se juntam: uma so
            // decidindo por todas apagaria o campo que ela nao olhou, que e o
            // estrago que estas funcoes existem para impedir.
            //
            // A heranca monta uma definicao que NINGUEM declarou, e por isso a
            // recusa por motor volta a ser feita aqui: trocar o motor de
            // `phxsql` para `mysql` sem mandar `cifra` passaria pelo `de_json`
            // (o pedido nao traz o campo) e herdaria uma cifra que aquele fio
            // nao sabe fazer. Nao e portao espalhado -- e a MESMA conferencia,
            // no unico outro ponto em que uma `Definicao` nasce sem passar
            // inteira pelo `de_json`.
            d.conferir_cifra_do_motor()?;
        }
        let ficha = d.para_json();
        let gravacao = r.salvar(d)?;
        Ok(Json::objeto(vec![
            ("gravado", Json::Bool(true)),
            ("ligacao", ficha),
            // A migracao para o cadastro cifrado e DITA na resposta, alem do
            // log: quantas ligacoes sairam do texto puro nesta gravacao.
            ("cadastro", gravacao.para_json()),
        ]))
    }

    pub(super) fn op_dblink_excluir(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("nome", "").to_string();
        let mut r = self.dblink.tomar("dblink")?;
        let gravacao = r.excluir(&nome)?;
        Ok(Json::objeto(vec![
            ("excluido", Json::texto_de(nome)),
            ("restam", Json::de_u64(r.ligacoes.len() as u64)),
            ("cadastro", gravacao.para_json()),
        ]))
    }

    /// Abre a ligacao pedida e devolve a conexao pronta.
    ///
    /// A definicao e COPIADA antes de conectar, e a trava do cadastro sai da
    /// mao: conectar leva ida e volta de rede, e segurar a trava enquanto isso
    /// travaria a tela de cadastro de todo mundo por causa de um host que nao
    /// responde.
    pub(super) fn ligar(&self, p: &Json) -> Result<(Definicao, crate::dblink::Conexao)> {
        let d = {
            let r = self.dblink.tomar("dblink")?;
            r.achar(p.texto_ou("dblink", p.texto_ou("nome", "")))?
                .clone()
        };
        // `abrir` escolhe a conexao pelo motor da definicao -- e por aqui que
        // o PostgreSQL(R) entra sem que nenhuma operacao precise saber dele.
        let c = d.abrir()?;
        Ok((d, c))
    }

    pub(super) fn op_dblink_testar(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::testar(&d, c)
    }

    /// As bases do outro servidor.
    pub(super) fn op_dblink_bancos(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::bancos(&d, c)
    }

    /// As tabelas de uma base do outro servidor, com tamanho e comentario.
    pub(super) fn op_dblink_tabelas(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::tabelas(&d, c, p)
    }

    /// A estrutura de uma tabela do outro servidor.
    pub(super) fn op_dblink_estrutura(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::estrutura(&d, c, p)
    }

    /// O conteudo de uma tabela do outro servidor, para a grade.
    pub(super) fn op_dblink_ler(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::ler(&d, c, p)
    }

    /// Uma instrucao escrita a mao contra o outro servidor.
    ///
    /// Duas travas antes de mandar, e as duas precisam ceder: a ligacao tem de
    /// nao ser somente-leitura E este servidor tambem nao. Um espelho nao vira
    /// caminho de escrita para o banco do outro so porque a ligacao permitia.
    pub(super) fn op_dblink_consultar(&self, p: &Json) -> Result<Json> {
        let sql = p.texto_ou("sql", "").trim().to_string();
        if sql.is_empty() {
            return Err(PhxError::Esquema("dblink_consultar sem \"sql\"".into()));
        }
        // As duas travas vem ANTES de conectar: recusar depois de abrir a
        // conexao gasta uma ida a rede para dizer nao. A da ligacao precisa da
        // definicao, entao ela e achada primeiro, sem conectar.
        let d = {
            let r = self.dblink.tomar("dblink")?;
            r.achar(p.texto_ou("dblink", p.texto_ou("nome", "")))?
                .clone()
        };
        if !crate::dblink::so_consulta(&sql) {
            if d.somente_leitura {
                return Err(PhxError::Autorizacao(format!(
                    "a ligacao {:?} esta em somente leitura e a instrucao nao e consulta",
                    d.nome
                )));
            }
            if self.somente_leitura() {
                return Err(PhxError::Autorizacao(
                    "este servidor esta em somente leitura: nao escreve nem pelo dblink".into(),
                ));
            }
        }
        let limite = p
            .inteiro_ou("limite", d.max_linhas as i64)
            .clamp(1, d.max_linhas as i64) as u64;
        let c = d.abrir()?;
        crate::dblink::operacoes::consultar(&d, c, &sql, limite)
    }

    /// `dblink_ligar`: o assistente liga tabelas primas — cria a tabela local
    /// espelhando a remota e registra a sincronia na definicao da ligacao.
    pub(super) fn op_dblink_ligar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        use crate::dblink::sincronia;
        let pedidos = p
            .campo("tabelas")
            .and_then(Json::lista)
            .ok_or_else(|| PhxError::Esquema("informe \"tabelas\" como lista".into()))?;
        if pedidos.is_empty() {
            return Err(PhxError::Esquema("a lista \"tabelas\" veio vazia".into()));
        }

        let (mut d, mut c) = self.ligar(p)?;
        d.exigir_catalogo_em_sql("dblink_ligar")?;
        // Pedido 545: a REDE inteira primeiro, SEM a trava de dados. Antes a
        // trava global era tomada aqui em cima e o `LIMIT 0` de cada tabela
        // ia ao fio com ela na mao: um par que gotejasse um byte abaixo do
        // prazo por leitura prendia todo pedido de todo cliente do banco. Os
        // metadados remotos nao dependem de nada local, entao busca-los antes
        // nao muda o que se decide depois -- e, se uma tabela da lista falhar
        // no fio, nenhuma chega a ser criada, em vez de metade.
        let mut buscadas = Vec::with_capacity(pedidos.len());
        for t in pedidos {
            let sinc = sincronia::Sincronia::de_json(t)?;
            crate::dblink::nome_seguro(&sinc.remota)?;
            if sinc.local_database.trim().is_empty() {
                return Err(PhxError::Esquema(format!(
                    "sincronia de {:?} sem \"local_database\"",
                    sinc.remota
                )));
            }
            // O portao por tabela, no alvo LOCAL: esta operacao nao tem o
            // campo "tabela" que o portao comum le -- o mesmo furo do juntar.
            if let Some(u) = &sessao.usuario {
                if !u.pode_em(&sinc.local_database, &sinc.local_tabela, Atividade::Criar) {
                    return Err(PhxError::Autorizacao(format!(
                        "sem direito de criar {}.{}",
                        sinc.local_database, sinc.local_tabela
                    )));
                }
            }
            // So os metadados, no dialeto de la (pedido 583): o `LIMIT 0` no
            // MySQL(R), o catalogo no PostgreSQL(R), que e o unico dos dois
            // que diz qual coluna e a chave.
            let colunas = sincronia::colunas_do_espelho(d.motor, &mut c, &sinc.remota)?;
            let (esquema, chave) =
                sincronia::esquema_local_de(d.motor, &sinc.local_tabela, &colunas)?;
            buscadas.push((sinc, esquema, chave));
        }
        c.encerrar();

        // Pedido 609: a copia `d` foi lida ANTES da rede, e o fio pode ter
        // levado minutos (10 s x 60 de fabrica). Conferida aqui, antes de
        // criar tabela local, a ligacao excluida ou trocada no meio nao deixa
        // nem o espelho para tras; a conferencia que DECIDE e a de baixo, sob
        // a mesma trava da gravacao -- esta so poupa o trabalho perdido.
        self.dblink
            .tomar("dblink")?
            .conferir_que_nao_mudou(&d, "dblink_ligar")?;

        // So agora a trava, e so para o que e local: criar a tabela que falta.
        // Nenhuma ida ao fio acontece daqui ate solta-la.
        let dados = self.travar_dados()?;
        let mut ligadas = Vec::new();
        // Pedido 589: o que nasce aqui vai ao disco depois de a trava soltar,
        // e antes da resposta -- o mesmo desenho do `criar_tabela` da rede.
        let mut pendente = PorSincronizar::default();
        for (mut sinc, esquema, chave) in buscadas {
            let (db, do_database) =
                dados.garantir_database_adiando_o_fsync(&sinc.local_database)?;
            pendente.juntar(do_database);
            let criada = match db.abrir_qualificada(&sinc.local_tabela) {
                Ok(existente) => {
                    // Tabela ja existente serve, desde que a chave case; o
                    // resto o mapa por nome confere a cada rodada.
                    drop(existente);
                    false
                }
                Err(_) => {
                    let (t, da_tabela) = db.criar_tabela_adiando_o_fsync(None, esquema)?;
                    drop(t);
                    pendente.juntar(da_tabela);
                    true
                }
            };
            sinc.chave = chave;
            d.sincronias.retain(|x| {
                !(x.remota.eq_ignore_ascii_case(&sinc.remota)
                    && x.local_database.eq_ignore_ascii_case(&sinc.local_database)
                    && x.local_tabela.eq_ignore_ascii_case(&sinc.local_tabela))
            });
            ligadas.push(Json::objeto(vec![
                ("remota", Json::texto_de(&sinc.remota)),
                ("local_database", Json::texto_de(&sinc.local_database)),
                ("local_tabela", Json::texto_de(&sinc.local_tabela)),
                ("chave", Json::texto_de(&sinc.chave)),
                ("sentido", Json::texto_de(sinc.sentido.nome())),
                ("dono", Json::texto_de(sinc.dono.nome())),
                ("tabela_criada", Json::Bool(criada)),
            ]));
            d.sincronias.push(sinc);
        }
        drop(dados);
        pendente.levar_ao_disco()?;
        let mut r = self.dblink.tomar("dblink")?;
        // A conferencia que decide: na MESMA trava do `salvar`, entao entre
        // ela e a gravacao ninguem mexe. Sem ela, o `salvar` (que substitui
        // pelo nome) ressuscitava a ligacao excluida com a senha antiga e
        // desfazia a troca de senha, host, pino ou `somente_leitura` feita
        // enquanto o fio falava. A tabela local que ja nasceu fica: e um
        // espelho vazio, e o proximo `dblink_ligar` a reaproveita.
        r.conferir_que_nao_mudou(&d, "dblink_ligar")?;
        let gravacao = r.salvar(d)?;
        Ok(Json::objeto(vec![
            ("ligadas", Json::Lista(ligadas)),
            ("cadastro", gravacao.para_json()),
        ]))
    }

    /// `dblink_sincronizar`: uma rodada de convergencia das tabelas ligadas.
    ///
    /// E a operacao que o job agenda. Exclusao nao viaja, o conflito e por
    /// linha e quem vence e o dono -- o porque de cada limite esta no modulo
    /// `dblink::sincronia`.
    pub(super) fn op_dblink_sincronizar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        use crate::dblink::sincronia::{self, Sentido};
        use std::collections::HashMap;

        let so = p.texto_ou("tabela", "").trim().to_string();
        let (d, mut c) = self.ligar(p)?;
        d.exigir_catalogo_em_sql("dblink_sincronizar")?;
        if d.sincronias.is_empty() {
            return Err(PhxError::Esquema(format!(
                "a ligacao {:?} nao tem tabela ligada: rode o assistente do DbLink",
                d.nome
            )));
        }

        let mut relatorio = Vec::new();
        for sinc in &d.sincronias {
            if !so.is_empty()
                && !so.eq_ignore_ascii_case(&sinc.remota)
                && !so.eq_ignore_ascii_case(&sinc.local_tabela)
            {
                continue;
            }
            // Os portoes locais desta sincronia, conforme o que ela faz.
            if let Some(u) = &sessao.usuario {
                if !u.pode_em(&sinc.local_database, &sinc.local_tabela, Atividade::Ler) {
                    return Err(PhxError::Autorizacao(format!(
                        "sem direito de ler {}.{}",
                        sinc.local_database, sinc.local_tabela
                    )));
                }
                if sinc.sentido != Sentido::Empurrar
                    && !(u.pode_em(&sinc.local_database, &sinc.local_tabela, Atividade::Inserir)
                        && u.pode_em(&sinc.local_database, &sinc.local_tabela, Atividade::Alterar))
                {
                    return Err(PhxError::Autorizacao(format!(
                        "sem direito de gravar em {}.{}",
                        sinc.local_database, sinc.local_tabela
                    )));
                }
            }
            if sinc.sentido != Sentido::Puxar && d.somente_leitura {
                return Err(PhxError::Autorizacao(format!(
                    "a ligacao {:?} esta em somente leitura e a sincronia de {:?} \
                     empurra: tire o somente_leitura ou mude o sentido para puxar",
                    d.nome, sinc.remota
                )));
            }

            // O lado de la, inteiro -- com uma linha de sobra para saber se o
            // teto cortou. Sincronizar metade e fingir que acabou seria pior
            // que recusar.
            //
            // E vem ANTES da trava de dados (pedido 545). Com a trava na mao,
            // um par que gotejasse abaixo do prazo por leitura prendia todo
            // pedido de todo cliente. Ler antes e seguro porque a trava nunca
            // cobriu o lado de la: o remoto podia mudar entre esta leitura e a
            // gravacao local tambem quando ela era feita sob a trava. Do lado
            // de ca nada e decidido aqui -- esquema, mapa, chave, indice, linhas
            // locais e o plano saem todos sob a trava, do mesmo retrato em que
            // a gravacao acontece.
            //
            // Duas idas, as duas no dialeto de la (pedidos 583 e 584): as
            // colunas primeiro, para a leitura pedir o binario em hexadecimal
            // -- o protocolo de texto entrega o BLOB cru, e o leitor o guarda
            // como `String`.
            let teto = d.max_linhas;
            let meta = c.consultar(&d.motor.sql_metadados(&sinc.remota)?, 1)?;
            let r = c.consultar(&d.motor.sql_leitura(&sinc.remota, &meta.colunas)?, teto + 1)?;
            if r.truncado || r.linhas.len() as u64 > teto {
                return Err(PhxError::LimiteExcedido(format!(
                    "a tabela remota {:?} passa das {} linhas da ligacao: suba o \
                     max_linhas ou sincronize por outra estrategia",
                    sinc.remota, teto
                )));
            }

            // A trava por SINCRONIA, e so para o trecho local: cada tabela
            // ligada solta o banco antes de voltar ao fio pela seguinte.
            let mut dados = self.travar_dados()?;
            let db = dados.abrir_database(&sinc.local_database)?;
            let mut t = db.abrir_qualificada(&sinc.local_tabela).map_err(|_| {
                PhxError::NaoEncontrado(format!(
                    "a tabela local {}.{} nao existe: rode o assistente do DbLink",
                    sinc.local_database, sinc.local_tabela
                ))
            })?;
            t.definir_usuario(sessao.id());
            let esquema = t.esquema().clone();

            let negocio = sincronia::posicoes_de_negocio(&esquema);
            let mapa = sincronia::mapa_de_colunas(&esquema, &r.colunas)?;
            let chave_biz = negocio
                .iter()
                .position(|p| esquema.colunas()[*p].nome.eq_ignore_ascii_case(&sinc.chave))
                .ok_or_else(|| {
                    PhxError::Esquema(format!(
                        "a chave {:?} sumiu da tabela local {}.{}",
                        sinc.chave, sinc.local_database, sinc.local_tabela
                    ))
                })?;
            let indice_da_chave = esquema
                .indices()
                .iter()
                .find(|i| {
                    i.unico && i.colunas.len() == 1 && i.colunas[0].coluna == negocio[chave_biz]
                })
                .map(|i| i.nome.clone())
                .ok_or_else(|| {
                    PhxError::Esquema(format!(
                        "{}.{} nao tem indice UNICO na chave {:?} -- e ele que \
                         faz o upsert sem varrer",
                        sinc.local_database, sinc.local_tabela, sinc.chave
                    ))
                })?;

            let mut remotas = HashMap::new();
            for lr in &r.linhas {
                let lv = sincronia::linha_remota_para_negocio(&esquema, &negocio, &mapa, lr)?;
                remotas.insert(sincronia::chave_canonica(&lv[chave_biz]), lv);
            }
            let mut locais = HashMap::new();
            for (_rowid, linha) in t.varrer()? {
                let lv: Vec<_> = negocio.iter().map(|p| linha[*p].clone()).collect();
                locais.insert(sincronia::chave_canonica(&lv[chave_biz]), lv);
            }

            let plano = sincronia::plano(sinc.sentido, sinc.dono, &remotas, &locais);
            // A linha que ja existe grava pela porta da alteracao solta: com
            // filha na chave que muda, a marca da cascata vai antes (pedido
            // 540). A cascata que quebrou e a recuperacao completou vai para o
            // relatorio, em vez de sumir.
            let ped = pedido_da_tabela(&sinc.local_database, &sinc.local_tabela);
            let mut avisos_da_cascata: Vec<Json> = Vec::new();
            let mut alterar = |t: &mut Table, rowid: u64, nova: &[Value]| {
                let feita = self.alterar_solto(&mut dados, t, &ped, sessao, rowid, nova)?;
                if let Some(aviso) = feita.aviso {
                    avisos_da_cascata.push(Json::texto_de(aviso));
                }
                Ok(())
            };
            let (inseridas, alteradas) = sincronia::aplicar_para_ca(
                &mut t,
                &indice_da_chave,
                chave_biz,
                &plano.para_ca,
                &mut alterar,
            )?;
            t.sincronizar()?;

            let colunas_sql: Vec<(String, phxsql_core::types::ColumnType)> = negocio
                .iter()
                .map(|p| (esquema.colunas()[*p].nome.clone(), esquema.colunas()[*p].ty))
                .collect();
            let empurrao = sincronia::sql_do_empurrao(
                d.motor,
                &sinc.remota,
                &sinc.chave,
                &colunas_sql,
                &plano.para_la,
                500,
            )?;
            // O punho ANTES da trava: o `Drop` do `Table` ainda grava no disco,
            // e gravar depois de soltar seria escrever sem a trava.
            drop(t);
            drop(dados);

            // O empurrao volta ao fio SEM a trava (pedido 545). As linhas dele
            // sao o retrato local lido sob a trava acima; se alguem alterar a
            // linha aqui depois de soltarmos, o `plano` -- que so compara
            // valores, sem memoria da rodada anterior -- ve a diferenca na
            // proxima rodada e a empurra de novo. E o mesmo que acontecia com
            // uma escrita que chegasse um instante depois do fim da rodada
            // antiga: soltar antes nao cria caso novo, so tira o banco inteiro
            // da fila de um par lento.
            let mut empurradas = 0u64;
            for sql in empurrao {
                let r = c.consultar(&sql, 1)?;
                empurradas += r.afetadas;
            }

            let mut ficha = vec![
                ("remota", Json::texto_de(&sinc.remota)),
                (
                    "local",
                    Json::texto_de(format!("{}.{}", sinc.local_database, sinc.local_tabela)),
                ),
                ("sentido", Json::texto_de(sinc.sentido.nome())),
                ("puxadas_novas", Json::de_u64(inseridas)),
                ("puxadas_alteradas", Json::de_u64(alteradas)),
                ("empurradas", Json::de_u64(plano.para_la.len() as u64)),
                ("linhas_afetadas_la", Json::de_u64(empurradas)),
                ("iguais", Json::de_u64(plano.iguais)),
                ("conflitos", Json::de_u64(plano.conflitos)),
            ];
            // So quando houve: a ficha de sempre nao ganha campo vazio.
            if !avisos_da_cascata.is_empty() {
                ficha.push(("avisos", Json::Lista(avisos_da_cascata)));
            }
            relatorio.push(Json::objeto(ficha));
        }
        c.encerrar();
        if relatorio.is_empty() {
            return Err(PhxError::NaoEncontrado(format!(
                "nenhuma sincronia casa com {so:?} na ligacao {:?}",
                d.nome
            )));
        }
        Ok(Json::objeto(vec![(
            "sincronizadas",
            Json::Lista(relatorio),
        )]))
    }
}
