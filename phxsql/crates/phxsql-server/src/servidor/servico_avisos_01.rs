//! Violacao, bloqueio e mensagens; o `Carteiro`; o relogio de gravacao e o
//! amostrador.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// O que um SPARE atende. Reserva e reserva: cliente comum nao le nem
/// escreve; o que passa e administracao, monitoramento e a propria
/// replicacao -- inclusive `posicao`/`replicar`, para o spare poder ser
/// origem de cascata e mostrar o proprio atraso.
///
/// Lista de PERMISSAO, e nao de recusa, de proposito: operacao nova nasce
/// BARRADA no spare ate alguem decidir o contrario -- o mesmo principio do
/// portao de permissao que nega operacao desconhecida.
/// O codigo do `PhxError::Io` (5001), como ele chega no `Acesso` que o
/// `anotar` recebe. E o portao do gancho da saude do disco: uma comparacao de
/// inteiro, antes de qualquer trabalho. Ha teste que o compara com
/// `PhxError::Io(..).codigo()`, para o numero nao poder derivar calado.
pub(super) const CODIGO_DE_ES: u16 = 5001;

impl Servidor {
    /// Violacao grave: bloqueia (na hora ou na enesima, conforme a politica)
    /// e avisa no log. Devolve o que aconteceu, porque a RESPOSTA depende
    /// disso: dizer "o IP foi bloqueado" quando a whitelist protegeu ou a
    /// tentativa so contou seria mentir para o cliente.
    pub(super) fn violacao_grave(
        &self,
        ip: &str,
        comando: &str,
        motivo: &str,
    ) -> crate::blacklist::Grave {
        // A trava sai de cena antes do e-mail: falar com um rele com a lista
        // negra na mao pararia toda conexao que precisasse consultar um
        // bloqueio, e o rele e quem manda no tempo dessa conversa.
        let mut resultado = {
            let Ok(mut lista) = self.lista_negra.lock() else {
                return crate::blacklist::Grave::Protegido;
            };
            lista.violacao_grave(
                ip,
                comando,
                motivo,
                &self.config.politica,
                crate::agora_ms(),
            )
        };
        if let crate::blacklist::Grave::Bloqueado(b, aviso) = &mut resultado {
            // O firewall DEPOIS de soltar a lista (pedido 638): o comando tem
            // prazo, mas ate ele voltar a lista tem de estar livre para o
            // `barrado()` de toda outra conexao. O bloqueio ja vale.
            crate::blacklist::aplicar_no_firewall(
                &self.lista_negra,
                &self.config.politica,
                b,
                aviso,
            );
            eprintln!(
                "BLOQUEADO {ip} ate {} -- {} ({})",
                b.ate(),
                b.motivo,
                b.comando
            );
            if let Some(a) = aviso {
                eprintln!("AVISO: {a}");
            }
            self.avisar_violacao_por_email(b);
        }
        resultado
    }

    /// Avisa o administrador, por e-mail, que um IP acabou de ser bloqueado.
    ///
    /// # Por que aqui, e nao em cada portao
    ///
    /// E o mesmo motivo do portao de permissao ser um so: espalhar o aviso
    /// pelos seis pontos que chamam `violacao_grave` faria o que alguem
    /// esquecesse virar a violacao silenciosa -- e ninguem descobre por
    /// leitura. Este e o unico lugar por onde um bloqueio nasce.
    ///
    /// # Pedido, nao imposto
    ///
    /// Exige `alertas.email.ligado` E `alertas.email.avisar_seguranca`, que
    /// nasce falso. Quem configurou o rele so para o disco apertado nao pode
    /// comecar a receber aviso de seguranca por causa de uma versao nova --
    /// e o mesmo desenho do `avisar_jobs`.
    ///
    /// # O silencio, e o que ele custa
    ///
    /// `Blacklist::bloquear` SUBSTITUI o bloqueio do IP a cada violacao, entao
    /// uma conexao ja aberta que insista no comando proibido geraria um e-mail
    /// por tentativa: quem ataca escolheria quantas mensagens o administrador
    /// recebe. A chave e o IP e o silencio e o `alertas.repetir_horas` do
    /// disco e dos jobs -- uma lei so para a casa inteira. O preco esta
    /// escrito: uma segunda investida do MESMO IP dentro da janela nao manda
    /// segundo e-mail; ela continua na blacklist e no `acessos.log`.
    fn avisar_violacao_por_email(&self, b: &crate::blacklist::Bloqueio) {
        let email = self.config.alertas.email.clone();
        if !email.ligado || !email.avisar_seguranca {
            return;
        }
        let agora = crate::agora_ms();
        let silencio = self.config.alertas.repetir_horas as i64 * 3_600_000;
        {
            let Ok(mut avisados) = self.avisos_de_seguranca.lock() else {
                return;
            };
            let chave = format!("grave:{}", b.ip);
            if !crate::jobs::pode_avisar(&mut avisados, &chave, agora, silencio) {
                return;
            }
        }
        let assunto = format!("PhxSql: IP {} bloqueado ({})", b.ip, b.motivo);
        let corpo = Self::texto_da_violacao(b);
        // Linha de execucao propria pelo mesmo motivo do aviso de job, com um
        // peso a mais: quem dispara aqui e o portao de politica, no caminho de
        // um pedido da rede -- um rele fora do ar seguraria a RESPOSTA de quem
        // pediu pelo `timeout_s` inteiro.
        self.telemetria.subir(
            "aviso-seguranca",
            "entrega UM e-mail de violacao grave e sai; existe em thread \
             propria porque quem dispara e o portao de politica, no caminho \
             de um pedido da rede -- e um rele fora do ar seguraria a resposta",
            "servico",
            agora,
            move |fio| {
                fio.fazendo("falando com o rele de e-mail");
                match crate::email::enviar(&email, &assunto, &corpo) {
                    Ok(r) => eprintln!("aviso de seguranca enviado: {r}"),
                    // Falhar em avisar tambem e noticia, como no disco.
                    Err(e) => eprintln!("aviso de seguranca NAO ENVIADO: {e}"),
                }
            },
        );
    }

    /// O corpo do e-mail da violacao grave.
    ///
    /// Leva o IP, o motivo, a OPERACAO pedida e a hora -- e nada do corpo do
    /// pedido. Senha nunca em texto puro vale aqui como em todo lugar: um
    /// `login` recusado por politica carrega a senha no proprio pedido, e
    /// colar o pedido no e-mail seria mandar a senha de alguem para a caixa
    /// do administrador.
    fn texto_da_violacao(b: &crate::blacklist::Bloqueio) -> String {
        let mut t = String::new();
        t.push_str("O PhxSql bloqueou um endereço por violação grave.\n\n");
        t.push_str(&format!("  endereço    {}\n", b.ip));
        t.push_str(&format!("  motivo      {}\n", b.motivo));
        t.push_str(&format!("  operação    {}\n", b.comando));
        t.push_str(&format!("  tentativas  {}\n", b.tentativas));
        t.push_str(&format!("  desde       {}\n", b.desde()));
        t.push_str(&format!("  até         {}\n", b.ate()));
        t.push_str(&format!(
            "  firewall    {}\n\n",
            if b.firewall {
                "regra aplicada"
            } else {
                "sem regra (não configurado, ou a regra falhou)"
            }
        ));
        t.push_str("A operação foi recusada e o endereço está na blacklist.\n");
        t.push_str("Para soltar: phxsqld --desbloquear ");
        t.push_str(&b.ip);
        t.push('\n');
        t.push_str(&format!("Servidor PhxSql {VERSAO}\n"));
        t
    }

    /// Tentativa leve: conta, e bloqueia se passar do limite na janela.
    pub(super) fn violacao_leve(&self, ip: &str, comando: &str, motivo: &str) {
        // A lista sai de cena ANTES do firewall (pedido 638): so o resultado
        // atravessa o fim do bloco.
        let bloqueou = match self.lista_negra.lock() {
            Ok(mut lista) => lista.tentativa_leve(
                ip,
                comando,
                motivo,
                &self.config.politica,
                crate::agora_ms(),
            ),
            Err(_) => None,
        };
        if let Some((mut b, mut aviso)) = bloqueou {
            crate::blacklist::aplicar_no_firewall(
                &self.lista_negra,
                &self.config.politica,
                &mut b,
                &mut aviso,
            );
            eprintln!(
                "BLOQUEADO {ip} ate {} -- {} apos {} tentativas",
                b.ate(),
                b.motivo,
                b.tentativas
            );
            if let Some(a) = aviso {
                eprintln!("AVISO: {a}");
            }
        }
    }

    pub(super) fn anotar(&self, acesso: &Acesso) {
        if let Ok(mut log) = self.log.lock() {
            if let Err(e) = log.registrar(acesso) {
                eprintln!("falha ao gravar o log de acessos: {e}");
                // O proprio log e disco. Um `acessos.log` que nao aceita
                // escrita e a mesma noticia que um `.reg` que nao aceita --
                // e chegaria calada, porque nenhum cliente recebe este erro.
                if let PhxError::Io(io) = &e {
                    self.evento_de_disco(
                        crate::saude_do_disco::classificar(io),
                        "acessos.log",
                        "",
                        "",
                        &e.to_string(),
                    );
                }
            }
        }
        // O GANCHO DA SAUDE DO DISCO (pedido 249). Este e o unico sumidouro
        // por onde toda resposta de erro passa -- porta de dados, web, REST,
        // jobs, fio --, entao o gancho mora aqui e em lugar nenhum mais: em
        // quarenta operacoes, a que alguem esquecesse seria o furo. O portao
        // e uma comparacao de inteiro e vem ANTES de qualquer trabalho: o
        // caminho de sucesso, e o de todo outro erro, nao paga nada.
        crate::aquario::alarme::conferir_o_1001(acesso.codigo);
        if acesso.codigo == CODIGO_DE_ES {
            let texto = acesso.erro.as_deref().unwrap_or("");
            crate::telemetria::sinal(crate::aquario::Alarme::ErroDeDisco, &acesso.op);
            self.evento_de_disco(
                crate::saude_do_disco::tipo_do_texto(texto),
                &acesso.op,
                &acesso.database,
                &acesso.tabela,
                texto,
            );
        }
        // O AQUARIO (pedido 707), pelo mesmo motivo do gancho de cima: este e
        // o unico sumidouro, e a base e a contagem do aquario precisam de
        // TODO pedido. Uma chamada so; o portao da telemetria esta dentro do
        // `aquario_se_ligada`, antes de qualquer trabalho.
        if let Some(aquario) = self.telemetria.aquario_se_ligada() {
            let habitual = aquario.anotar(acesso);
            if let Some(desvio) = habitual.desvio() {
                self.sinalizar_desvio(aquario, acesso, habitual, &desvio);
            }
            // A linha `estourou` do `aquario.log` (A6). Fora do
            // `Aquario::anotar` porque a falha e noticia de disco, e o
            // `evento_de_disco` e daqui -- o mesmo destino da falha do
            // `acessos.log` logo acima. O corte por duracao vem antes de
            // qualquer trabalho la dentro, inclusive antes da classificacao.
            self.no_aquario_log(
                aquario
                    .log()
                    .tarefa_terminou(acesso, || self.classe_do_fim(acesso, habitual)),
            );
        }
    }

    /// Um erro de E/S visto em qualquer caminho: conta na saude do disco e,
    /// se o silencio deixar, ENTREGA ao carteiro -- que acorda na hora.
    ///
    /// Nada de rede aqui, e isto e lei e nao estilo: quem chama pode estar
    /// com a trava global de dados na mao (`fecho_recusado` esta, dentro de
    /// `descarregar_sujas_com`), e um `email::enviar` ali ataria o servidor
    /// inteiro ao tempo de resposta do rele -- no pior momento, o do disco
    /// doente. A catraca `rede-ou-espera` do `mapa-da-trava.py` acusou
    /// exatamente isso na primeira versao (hoje `rede-ou-espera-2`, que
    /// conta tambem a espera por outra thread -- pedido 627).
    pub(super) fn evento_de_disco(
        &self,
        tipo: crate::saude_do_disco::Tipo,
        origem: &str,
        database: &str,
        tabela: &str,
        texto: &str,
    ) {
        let agora = crate::agora_ms();
        if let Some(evento) = self
            .saude
            .erro_de_es(agora, tipo, origem, database, tabela, texto)
        {
            self.saude.entregar(evento);
        }
    }

    /// Este IP esta barrado agora? Devolve o bloqueio, para o chamador poder
    /// formatar dois textos diferentes dele: o do LOG, sempre de fabrica, e o
    /// da RESPOSTA, que passa pela tabela de mensagens.
    ///
    /// Reaproveitado pelas duas portas: a de dados e a da interface web. Um IP
    /// bloqueado e bloqueado no servidor inteiro, nao numa porta so.
    pub(super) fn barrado(&self, ip: &str, agora: i64) -> Option<crate::blacklist::Bloqueio> {
        let (vencidos, barrou) = {
            let mut lista = self.lista_negra.lock().ok()?;
            // Outro processo pode ter mexido no arquivo (phxsqld --desbloquear).
            let _ = lista.recarregar_se_mudou();
            let vencidos = lista.limpar_vencidos(agora).unwrap_or_default();
            // Whitelist vence SEMPRE -- inclusive sobre um bloqueio gravado
            // antes de a regra entrar. E o que impede o operador de se
            // trancar fora: a regra nova vale na proxima conexao, sem esperar
            // o bloqueio vencer.
            let barrou = if lista.protegido(&self.config.politica, ip) {
                None
            } else {
                lista.bloqueado(ip, agora).cloned()
            };
            (vencidos, barrou)
        };
        // Soltar a regra de quem venceu e do firewall, que pode demorar ate o
        // prazo: fora do mutex, porque este metodo roda em TODA conexao
        // (pedido 638). Sem vencido nao custa nada.
        if !vencidos.is_empty() {
            let _ = crate::blacklist::soltar_no_firewall(&self.config.politica, &vencidos);
        }
        barrou
    }

    /// O motivo do bloqueio como o log de acessos sempre gravou. NAO passa
    /// pela tabela de mensagens: filtro de log nao quebra por troca de idioma.
    pub(super) fn motivo_de_bloqueio(b: &crate::blacklist::Bloqueio) -> String {
        format!(
            "bloqueado desde {} ate {} por {} ({})",
            b.desde(),
            b.ate(),
            b.motivo,
            b.comando
        )
    }

    /// O mesmo motivo para a RESPOSTA, resolvido pela tabela de mensagens.
    pub(super) fn recado_de_bloqueio(&self, b: &crate::blacklist::Bloqueio) -> String {
        self.msg(
            "erro.ip_bloqueado",
            &[
                ("desde", b.desde().as_str()),
                ("ate", b.ate().as_str()),
                ("motivo", b.motivo.as_str()),
                ("comando", b.comando.as_str()),
            ],
        )
    }

    // ------------------------------------------------ mensagens do servidor
    //
    // Regra de ouro: `msg` e `texto_do_erro` NUNCA sao chamados com a trava
    // de dados na mao -- a recarga do cache a toma. Os pontos de uso sao os
    // portoes e a montagem da resposta, que rodam fora dela.

    /// O texto de uma mensagem nomeada, resolvido pela tabela de mensagens.
    pub(super) fn msg(&self, nome: &str, parametros: &[(&str, &str)]) -> String {
        self.mensagens_atualizar();
        self.mensagens.texto(nome, parametros)
    }

    /// O texto humano de um erro para a RESPOSTA. O log de acessos continua
    /// gravando o `Display` de fabrica -- filtro de log nao pode quebrar por
    /// troca de idioma.
    pub(super) fn texto_do_erro(&self, e: &PhxError) -> String {
        self.mensagens_atualizar();
        self.mensagens.texto_do_erro(e)
    }

    /// O recado de uma recusa por politica, com o sufixo que diz a verdade:
    /// bloqueado, tentativa N de M, ou nada (whitelist protegeu).
    pub(super) fn recado_de_grave(
        &self,
        destino: &crate::blacklist::Grave,
        nome: &str,
        parametros: &[(&str, &str)],
    ) -> String {
        let recado = self.msg(nome, parametros);
        match destino {
            crate::blacklist::Grave::Protegido => recado,
            crate::blacklist::Grave::Contada { tentativas, limite } => self.msg(
                "erro.grave_tentativa",
                &[
                    ("recado", recado.as_str()),
                    ("n", &tentativas.to_string()),
                    ("m", &limite.to_string()),
                ],
            ),
            crate::blacklist::Grave::Bloqueado(..) => {
                self.msg("erro.grave_bloqueado", &[("recado", recado.as_str())])
            }
        }
    }

    /// Rele a tabela de mensagens quando o `mtime` do `.reg` diz que alguem
    /// gravou nela -- e so entao. Pega QUALQUER caminho de escrita (grade,
    /// protocolo, SQL, ate outro processo), sem espalhar avisos por quarenta
    /// operacoes: e o mesmo desenho do `recarregar_se_mudou` da blacklist.
    pub(super) fn mensagens_atualizar(&self) {
        if !self.mensagens.precisa_recarregar() {
            return;
        }
        let Ok(dados) = self.travar_dados() else {
            return;
        };
        let linhas = Self::ler_tabela_de_mensagens(&dados);
        self.mensagens.carregar(linhas);
    }

    /// Le a tabela inteira para o cache. Tabela ausente = mapa vazio, que na
    /// resolucao significa "textos de fabrica" -- o comportamento de sempre.
    fn ler_tabela_de_mensagens(dados: &Instancia) -> HashMap<String, [String; 6]> {
        let mut mapa = HashMap::new();
        let Ok(db) = dados.abrir_database(crate::mensagens::DATABASE) else {
            return mapa;
        };
        let Ok(mut t) = db.abrir_qualificada(crate::mensagens::TABELA) else {
            return mapa;
        };
        let Some(col_nome) = posicao_da_coluna(t.esquema(), "TextName") else {
            return mapa;
        };
        let idiomas: Vec<Option<usize>> = crate::mensagens::IDIOMAS
            .iter()
            .map(|n| posicao_da_coluna(t.esquema(), n))
            .collect();
        let total = t.registros();
        let Ok((rowids, _)) = t.pagina_por_posicao(0, total, Visao::Ativas) else {
            return mapa;
        };
        for rowid in rowids {
            let Ok(Some(linha)) = t.ler(rowid) else {
                continue;
            };
            let Some(nome) = linha.get(col_nome).and_then(Value::como_str) else {
                continue;
            };
            let nome = nome.trim().to_string();
            if nome.is_empty() {
                continue;
            }
            let mut textos: [String; 6] = Default::default();
            for (i, pos) in idiomas.iter().enumerate() {
                if let Some(p) = pos {
                    if let Some(s) = linha.get(*p).and_then(Value::como_str) {
                        textos[i] = s.trim().to_string();
                    }
                }
            }
            mapa.insert(nome, textos);
        }
        mapa
    }

    /// Cria `phxsys.mensagens` se falta e grava as mensagens de fabrica que
    /// ainda nao existem la. Idempotente por `TextName`: linha presente nao e
    /// tocada -- semear de novo NUNCA desfaz a traducao de alguem.
    ///
    /// Devolve (criou database, criou tabela, semeadas, ja existiam).
    pub(super) fn semear_mensagens(&self) -> Result<(bool, bool, u64, u64)> {
        let dados = self.travar_dados()?;
        let mut criou_db = false;
        let db = match dados.abrir_database(crate::mensagens::DATABASE) {
            Ok(db) => db,
            Err(_) => {
                criou_db = true;
                dados.criar_database(crate::mensagens::DATABASE)?
            }
        };
        let mut criou_tabela = false;
        if !db.existe_tabela(None, crate::mensagens::TABELA)? {
            // `id` e `TextName` sao os FIXOS da programacao: chave primaria e
            // indice unico. As seis colunas de idioma sao texto comum -- e a
            // grade do Centro de Controle ja sabe editar texto comum.
            let mut colunas = vec![
                Column::new("id", ColumnType::Uuid).obrigatoria(),
                Column::new("TextName", ColumnType::Str(80)).obrigatoria(),
            ];
            for idioma in crate::mensagens::IDIOMAS {
                colunas.push(Column::new(
                    idioma,
                    ColumnType::Str(crate::mensagens::LARGURA_DO_TEXTO as u16),
                ));
            }
            let indices = vec![
                IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria(),
                IndexDef::new("porTextName", vec![IndexColumn::asc(1)]).unico(),
            ];
            db.criar_tabela(
                None,
                Schema::new(crate::mensagens::TABELA, colunas, indices)?,
            )?;
            criou_tabela = true;
        }

        let mut t = db.abrir_qualificada(crate::mensagens::TABELA)?;
        let Some(col_nome) = posicao_da_coluna(t.esquema(), "TextName") else {
            return Err(PhxError::Esquema(format!(
                "a tabela {}.{} existe mas nao tem a coluna TextName",
                crate::mensagens::DATABASE,
                crate::mensagens::TABELA
            )));
        };
        // O que ja esta la fica exatamente como esta.
        let mut existentes = std::collections::HashSet::new();
        let total = t.registros();
        // A visao e TODAS de proposito: um TextName marcado como excluido
        // ainda ocupa o indice unico, e semear por cima daria chave duplicada.
        if let Ok((rowids, _)) = t.pagina_por_posicao(0, total, Visao::Todas) {
            for rowid in rowids {
                if let Ok(Some(linha)) = t.ler(rowid) {
                    if let Some(nome) = linha.get(col_nome).and_then(Value::como_str) {
                        existentes.insert(nome.trim().to_string());
                    }
                }
            }
        }

        let mut semeadas = 0u64;
        for m in crate::mensagens::FABRICA {
            if existentes.contains(m.nome) {
                continue;
            }
            // A linha entra pelo MESMO caminho do `inserir` da rede
            // (`json_para_linha`): e ele que completa as colunas de sistema.
            // Um segundo caminho de montar linha seria o que diverge um dia.
            let mut objeto = vec![
                (
                    "id".to_string(),
                    Json::texto_de(phxsql_core::uuid::Uuid::v7().to_string()),
                ),
                ("TextName".to_string(), Json::texto_de(m.nome)),
            ];
            for (i, idioma) in crate::mensagens::IDIOMAS.iter().enumerate() {
                // Celula vazia fica NULL: e o degrau que cai para o
                // portugues, e e o que a tela mostra como "sem traducao".
                if !m.textos[i].is_empty() {
                    objeto.push((idioma.to_string(), Json::texto_de(m.textos[i])));
                }
            }
            let linha = json_para_linha(&Json::Objeto(objeto), t.esquema())?;
            t.inserir(&linha)?;
            semeadas += 1;
        }
        if semeadas > 0 {
            t.sincronizar()?;
        }
        drop(dados);
        // O cache rele na proxima resolucao -- e o "aplica sem reiniciar".
        self.mensagens.invalidar();
        Ok((criou_db, criou_tabela, semeadas, existentes.len() as u64))
    }

    /// Sobe o relogio do backup agendado, se ligado.
    ///
    /// Confere de minuto em minuto em vez de dormir ate a hora certa: dormir
    /// horas seguidas e frageil -- a maquina suspende, o relogio anda, e o
    /// backup nao acontece sem ninguem notar.
    /// O relogio que fecha a janela de durabilidade quando ninguem grava.
    ///
    /// Sem ele, a gravacao em lote so sincronizaria na PROXIMA gravacao -- e um
    /// servidor que recebe a ultima venda do dia as 18h e fica quieto deixaria
    /// essa venda sem `fsync` a noite inteira. O relogio acorda a cada janela e
    /// descarrega o que ficou.
    ///
    /// Em `por_operacao` e em `sistema` ele nao tem o que fazer: um sincroniza
    /// sempre, o outro nunca.
    pub(super) fn ligar_relogio_de_gravacao(self: &Arc<Self>) {
        if self.config.recursos.durabilidade != Durabilidade::PorLote {
            return;
        }
        let ms = self.config.recursos.lote_milissegundos.max(20);
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "relogio-gravacao",
            "fecha a janela de durabilidade quando ninguem grava: sem ela, a \
             ultima venda do dia ficaria sem `fsync` a noite inteira",
            "servico",
            crate::agora_ms(),
            move |fio| loop {
                std::thread::sleep(Duration::from_millis(ms));
                if servidor.janela.pendente() > 0 {
                    fio.fazendo("descarregando o que ficou pendente");
                    servidor.janela.fechar();
                    servidor.descarregar_sujas();
                } else {
                    fio.fazendo(&format!("nada pendente; acorda a cada {ms} ms"));
                }
            },
        );
    }

    /// Sobe o amostrador das series da telemetria.
    ///
    /// # Por que UMA thread, e nao a conta na hora de perguntar
    ///
    /// Taxa exige dois instantes. Se cada aba aberta calculasse a propria,
    /// cada uma teria a sua base de comparacao e duas telas mostrariam
    /// numeros diferentes do mesmo servidor -- e a primeira pergunta de cada
    /// aba nao teria taxa nenhuma. Uma amostragem so, de segundo em segundo,
    /// da a mesma serie para todo mundo.
    ///
    /// A thread sobe SEMPRE, mesmo com a telemetria desligada, e e barata
    /// justamente por isso: desligada, `amostrar` devolve no portao antes de
    /// ler qualquer `/proc`. Subir a thread junto com o interruptor exigiria
    /// que ligar a telemetria criasse thread -- e ai o custo de ligar
    /// dependeria de quantas vezes alguem ligou e desligou.
    pub(super) fn subir_amostrador(self: &Arc<Self>) {
        if !self.telemetria.marcar_amostrador() {
            return;
        }
        // O «no ar» do retrato sai junto com a thread -- o irmao do pedido
        // 452, o mesmo desenho do `RelogioNoAr`.
        let no_ar = AmostradorNoAr(Arc::clone(&self.telemetria));
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "amostrador",
            "tira, de segundo em segundo, a amostra das series do painel de \
             telemetria: esperas, vazao, CPU do processo, leitura e escrita \
             fisicas e acertos do cache do .ndx",
            "servico",
            crate::agora_ms(),
            move |fio| loop {
                // A guarda viaja capturada pelo `move` (ver `RelogioNoAr`).
                let _no_ar = &no_ar;
                #[cfg(test)]
                if servidor
                    .panico_no_amostrador_de_teste
                    .swap(false, Ordering::SeqCst)
                {
                    panic!("panico de teste no amostrador (irmao do pedido 452)");
                }
                let comeco = Instant::now();
                if servidor.telemetria.ligada() {
                    // A CPU da MAQUINA sai do mesmo monitor que o painel de
                    // sistema ja usa, e nao de uma segunda leitura do
                    // `/proc/stat`: duas leituras com bases diferentes dariam
                    // dois percentuais para o mesmo instante.
                    let maquina = crate::sistema::jiffies_da_maquina();
                    let agora = crate::agora_ms();
                    servidor.telemetria.amostrar(maquina, agora);
                    // A poda anda junto com a amostra porque as duas sao do
                    // mesmo relogio: uma aba fechada para de pedir, e um
                    // minuto depois a bolha dela sai.
                    servidor.telemetria.podar(agora, 60_000);
                    // A contagem do aquario vira o minuto no mesmo relogio, e
                    // so com a telemetria ligada: minuto desligado fica
                    // ausente (`null`), nunca zero. A linha do minuto vai ao
                    // `aquario.log` la dentro.
                    servidor.virar_a_contagem(agora);
                    fio.fazendo("amostra tirada");
                } else {
                    fio.fazendo("telemetria desligada: nao amostra");
                }
                // Desconta o que a amostra custou, para o periodo ser o
                // periodo e nao "o periodo mais o trabalho". Sem isso a serie
                // andaria mais devagar do que ela mesma diz que anda.
                let gasto = comeco.elapsed();
                let periodo = Duration::from_millis(crate::telemetria::PERIODO_DA_AMOSTRA_MS);
                std::thread::sleep(periodo.saturating_sub(gasto));
            },
        );
    }
}

/// O `fsync` recusado DERRUBA o processo -- pedido 509.
///
/// # Por que cair, e nao ficar de pe recusando
///
/// Depois de um `fsync` recusado o nucleo pode ter descartado as paginas, e o
/// proximo responde Ok sem elas (medido pelo papel C: o fecho 2 deu Ok e o
/// `.log` perdeu 5.000 de 5.000 eventos). Os quatro motores nunca tratam a
/// repeticao como sucesso (10 x 0); no MEIO, os tres servidores caem --
/// PostgreSQL PANIC, MariaDB e MySQL `ib::fatal`, 4 + 3 + 2 = 9 -- e so a
/// biblioteca (SQLite, 1) fica recusando. Este e o caso dos tres. E ficar de
/// pe custaria mais aqui que la: a janela de durabilidade responde Ok a cada
/// gravacao ANTES do `fsync`, entao o servidor continuaria confirmando
/// escrita sobre um disco que ja se sabe que mente. E a H5 do pedido 451 pelo
/// outro gatilho: estado que nao se pode afirmar vira queda.
///
/// # Por que no INSTANTE da recusa
///
/// O gancho roda dentro do `sync_all` do motor, com a trava de dados na mao de
/// quem sincroniza. Cair na volta -- no `descarregar_sujas_com`, digamos --
/// deixaria o `Drop` da tabela reaberta gravar o cabecalho do `.ndx` limpo por
/// cima das paginas perdidas, que e a outra metade do que o papel C mediu. O
/// `abort` nao roda `Drop` nenhum, e nao drena marca nenhuma: as marcas dos
/// `COMMIT` que esperavam o fecho ficam no disco. Isso NAO basta para o dado
/// voltar: medido pelo papel C (parecer 509+512, P1, 3/3), o arranque no MESMO
/// boot le do cache do nucleo o que o disco perdeu, o `sincronizar` responde
/// Ok e a marca sairia -- o 509 continua aberto nessa metade. O que parar
/// compra e nao confirmar mais nada sobre este disco neste processo.
/// O carteiro dos avisos de saude: e-mail, SMS pelo gateway e o gancho do
/// operador (pedido 249).
///
/// # Por que um tipo fora do `Servidor`
///
/// Pedido 573: o arranque que acha a sentinela do 509 recusa subir ANTES de o
/// `Servidor` existir, e o operador tem de ser avisado por ali tambem.
/// Escrever um segundo envio para o arranque seria a decisao «como se avisa»
/// escrita duas vezes -- a que envelhecesse mandaria e-mail sem SMS, ou sem o
/// gancho. Entao o motor e este, com o que ele de fato le (a `Config` e a
/// `SaudeDoDisco`), e o `Servidor` o empresta.
pub(crate) struct Carteiro<'a> {
    pub config: &'a Config,
    pub saude: &'a crate::saude_do_disco::SaudeDoDisco,
}

impl Carteiro<'_> {
    /// O aviso de um evento de saude que passou pelo silencio. Sempre escreve
    /// no erro padrao; e-mail e SMS saem se o rele estiver ligado.
    ///
    /// SO a thread `sonda-disco` chama isto, fora de qualquer trava do
    /// servidor. Quem registra o evento (o `anotar`, o fecho) entrega a fila
    /// e volta -- ver `evento_de_disco`.
    pub(crate) fn avisar_saude_do_disco(
        &self,
        fio: &crate::telemetria::Fio,
        evento: crate::saude_do_disco::Evento,
    ) {
        // O backup agendado pega carona no carteiro (pedido 510), e o texto e
        // dele: «detectou um problema no disco onde o banco grava» seria
        // mentira sobre um destino sem permissao.
        let backup = evento.tipo == crate::saude_do_disco::Tipo::Backup;
        // O do arranque (255) tambem nao e do disco: texto proprio, e fora da
        // conta dos avisos de saude, como o do backup.
        let arranque = evento.tipo == crate::saude_do_disco::Tipo::Arranque;
        let fora_do_disco = backup || arranque;
        if !fora_do_disco {
            eprintln!(
                "SAUDE DO DISCO ({}): {} em {}{} -- {}",
                evento.tipo.nome(),
                Carteiro::tipo_de_saude_legivel(evento.tipo),
                evento.origem,
                Carteiro::alvo_do_evento(&evento),
                evento.texto
            );
        }
        // Lentidao e aviso de painel, nunca de canal.
        if !evento.tipo.e_erro() {
            return;
        }
        // Os dois meios sao independentes: o gancho do operador sai com o
        // e-mail desligado (e o e-mail sem gancho), e o erro de um nunca
        // impede o outro. O e-mail e o SMS-por-gateway vao primeiro, porque
        // sao o comportamento que ja existia; o gancho vem depois, com prazo.
        self.avisar_por_email_e_sms(fio, &evento, fora_do_disco);
        self.avisar_pelo_gancho(fio, &evento);
    }

    /// O e-mail e o SMS por e-mail-para-SMS da operadora -- o aviso de
    /// sempre, so movido para ca para o gancho nao herdar os `return` dele.
    fn avisar_por_email_e_sms(
        &self,
        fio: &crate::telemetria::Fio,
        evento: &crate::saude_do_disco::Evento,
        fora_do_disco: bool,
    ) {
        let backup = evento.tipo == crate::saude_do_disco::Tipo::Backup;
        let arranque = evento.tipo == crate::saude_do_disco::Tipo::Arranque;
        let email = self.config.alertas.email.clone();
        // O SMS ja nasce validado como «so com e-mail ligado» (config.rs).
        if !email.ligado {
            return;
        }
        let sms = self.config.alertas.sms.clone();
        let assunto = if backup {
            "PhxSql: o backup agendado FALHOU".to_string()
        } else if arranque {
            "PhxSql: o arranque reconstruiu indices depois de uma queda".to_string()
        } else {
            format!(
                "PhxSql: saude do disco -- {} ({})",
                Carteiro::tipo_de_saude_legivel(evento.tipo),
                evento.origem
            )
        };
        let corpo = if backup {
            self.texto_do_aviso_de_backup(evento)
        } else if arranque {
            self.texto_do_aviso_do_arranque(evento)
        } else {
            self.texto_do_aviso_de_saude(evento)
        };
        let linha_sms = Servidor::texto_do_sms_de_saude(evento);
        // O painel conta os avisos DA SAUDE DO DISCO (a carta diz «o disco
        // onde o banco grava»): o do backup sai pelo mesmo carteiro e fica
        // fora da conta dela, com o log proprio.
        let de_que = if backup {
            "do backup agendado"
        } else if arranque {
            "do arranque"
        } else {
            "de saude do disco"
        };
        fio.fazendo("falando com o rele de e-mail");
        let r = crate::email::enviar(&email, &assunto, &corpo);
        match &r {
            Ok(r) => eprintln!("aviso {de_que} enviado: {r}"),
            // Falhar em avisar tambem e noticia, como no disco cheio.
            Err(e) => eprintln!("aviso {de_que} NAO ENVIADO: {e}"),
        }
        if !fora_do_disco {
            self.saude.anotar_aviso(
                "email",
                r.map(|_| ()).map_err(|e| e.to_string()),
                crate::agora_ms(),
            );
        }
        if !sms.ligado {
            return;
        }
        fio.fazendo("mandando o SMS pelo gateway da operadora");
        // O MESMO rele, com os destinatarios trocados por `numero@gateway`:
        // e o que a operadora entrega como texto.
        let mut por_sms = email.clone();
        por_sms.para = sms.enderecos();
        let r = crate::email::enviar(&por_sms, "PhxSql", &linha_sms);
        match &r {
            Ok(r) => eprintln!("SMS {de_que} enviado: {r}"),
            Err(e) => eprintln!("SMS {de_que} NAO ENVIADO: {e}"),
        }
        if !fora_do_disco {
            self.saude.anotar_aviso(
                "sms",
                r.map(|_| ()).map_err(|e| e.to_string()),
                crate::agora_ms(),
            );
        }
    }

    /// O gancho externo do operador (pedido 249). O ERRO dele e so log e
    /// `avisos.ultima_falha`: o aviso que nao saiu por aqui nao pode
    /// derrubar o carteiro nem esconder o que saiu pelos outros meios.
    ///
    /// Roda na thread `sonda-disco`, fora de qualquer trava, e o portao
    /// (`ligado`) vem ANTES de montar qualquer texto: desligado custa um
    /// `if`. O texto e o MESMO do SMS por e-mail -- uma linha, ate 160
    /// caracteres, sem caminho --, e e tudo o que o programa recebe do
    /// evento; o pedido que provocou o erro nunca chega aqui.
    fn avisar_pelo_gancho(
        &self,
        fio: &crate::telemetria::Fio,
        evento: &crate::saude_do_disco::Evento,
    ) {
        let g = &self.config.alertas.gancho;
        if !g.ligado {
            return;
        }
        fio.fazendo("chamando o gancho do operador");
        let quando = phxsql_core::datahora::instante_iso(evento.quando_ms);
        let linha = Servidor::texto_do_sms_de_saude(evento);
        let chamada = crate::gancho::Chamada {
            tipo: evento.tipo.nome(),
            origem: &evento.origem,
            quando: &quando,
            linha: &linha,
        };
        let r = crate::gancho::executar(g, &chamada, &self.saude.gancho_em_voo);
        match &r {
            Ok(()) => eprintln!("gancho do operador executado ({})", evento.tipo.nome()),
            Err(e) => eprintln!("gancho do operador NAO EXECUTADO: {e}"),
        }
        self.saude.anotar_aviso("gancho", r, crate::agora_ms());
    }

    pub(super) fn tipo_de_saude_legivel(tipo: crate::saude_do_disco::Tipo) -> &'static str {
        use crate::saude_do_disco::Tipo;
        match tipo {
            Tipo::SoLeitura => "montagem so-leitura (EROFS)",
            Tipo::SemEspaco => "sem espaco (ENOSPC)",
            Tipo::EntradaSaida => "erro de E/S",
            Tipo::Conferencia => "o dado voltou diferente do escrito",
            Tipo::Lento => "disco lento",
            Tipo::Backup => "o backup agendado falhou",
            Tipo::Arranque => "o arranque reconstruiu indices",
        }
    }

    /// O corpo do e-mail do arranque que reconstruiu indices -- pedido 255.
    /// Diz o que aconteceu, que o dado nao se perdeu por isso, e o que fazer
    /// com a tabela que ficou pendente.
    fn texto_do_aviso_do_arranque(&self, e: &crate::saude_do_disco::Evento) -> String {
        format!(
            "O PhxSql subiu depois de uma queda, e o arranque reconstruiu indices que \
             ela deixou para tras.\n\n\x20 servidor   {}\n  base       {}\n  quando     \
             {}\n  o que      {}\n\n\
             O indice se reconstroi a partir do arquivo de dados: nenhuma linha se \
             perdeu por isso. Se ha tabela pendente, ela recusa ate alguem rodar \
             `reindexar` nela. Queda sem aviso se investiga: energia, SIGKILL, falta \
             de memoria.\n\
             Servidor PhxSql {VERSAO}\n",
            crate::email::nome_da_maquina(),
            self.config.base.display(),
            phxsql_core::datahora::instante_iso(e.quando_ms),
            e.texto
        )
    }

    /// O corpo do e-mail do backup agendado que falhou -- pedido 510. Leva o
    /// destino e o erro, e o que acontece depois: sem isto, quem le nao sabe
    /// se o servidor tenta de novo daqui a um minuto ou amanha.
    fn texto_do_aviso_de_backup(&self, e: &crate::saude_do_disco::Evento) -> String {
        let b = &self.config.backup;
        format!(
            "O backup agendado do PhxSql FALHOU -- a copia desta rodada NAO existe.\n\n\
             \x20 servidor   {}\n  base       {}\n  destino    {}\n  quando     {}\n  \
             erro       {}\n\n\
             O proximo backup roda na proxima hora da agenda ({}). Enquanto ele \
             continuar falhando, o proximo aviso sai em ate {} min; o primeiro que der \
             certo zera o silencio.\n\
             Servidor PhxSql {VERSAO}\n",
            crate::email::nome_da_maquina(),
            self.config.base.display(),
            b.destino.display(),
            phxsql_core::datahora::instante_iso(e.quando_ms),
            e.texto,
            if b.hora.is_empty() {
                format!("a cada {} h", b.cada_horas)
            } else {
                format!("todo dia as {}", b.hora)
            },
            self.config.alertas.disco.repetir_minutos
        )
    }

    /// ` (base/tabela)` quando o evento nomeia uma; vazio quando nao.
    pub(super) fn alvo_do_evento(e: &crate::saude_do_disco::Evento) -> String {
        match (e.database.is_empty(), e.tabela.is_empty()) {
            (true, true) => String::new(),
            (false, true) => format!(" ({})", e.database),
            _ => format!(" ({}/{})", e.database, e.tabela),
        }
    }

    /// O corpo do e-mail. Leva o servidor, o `base`, o tipo, a origem, a hora
    /// e o texto do erro -- e NUNCA o pedido que o provocou: a resposta de um
    /// `inserir` recusado por E/S nao carrega a linha, e o e-mail tambem nao.
    fn texto_do_aviso_de_saude(&self, e: &crate::saude_do_disco::Evento) -> String {
        let mut t = String::new();
        t.push_str("O PhxSql detectou um problema no disco onde o banco grava.\n\n");
        t.push_str(&format!(
            "  servidor   {}\n  base       {}\n  tipo       {}\n  origem     {}{}\n  quando     {}\n  erro       {}\n\n",
            crate::email::nome_da_maquina(),
            self.config.base.display(),
            Carteiro::tipo_de_saude_legivel(e.tipo),
            e.origem,
            Carteiro::alvo_do_evento(e),
            phxsql_core::datahora::instante_iso(e.quando_ms),
            e.texto
        ));
        t.push_str(&format!(
            "  erros de E/S desde o arranque: {}\n",
            self.saude.erros_es()
        ));
        match self.saude.ultima_sonda() {
            Some(s) => t.push_str(&format!(
                "  ultima sonda: {} ({} ms, {})\n\n",
                match &s.falha {
                    None => "passou".to_string(),
                    Some((_, texto)) => format!("falhou -- {texto}"),
                },
                s.duracao_us / 1_000,
                phxsql_core::datahora::instante_iso(s.medido_em_ms)
            )),
            None => t.push_str("  ultima sonda: ainda nao rodou\n\n"),
        }
        t.push_str(&format!(
            "Enquanto o problema continuar, o proximo aviso deste tipo sai em ate {} min.\n\
             Servidor PhxSql {VERSAO}\n",
            self.config.alertas.disco.repetir_minutos
        ));
        t
    }
}

/// Desliga o `relogio_de_jobs` quando a thread do relogio sai -- o irmao do
/// pedido 452.
///
/// O relogio marcava «estou no ar» ao subir e nunca desmarcava: se ele morresse
/// (um panico fora da corrida, que agora roda na filha), o `relogio_no_ar`
/// continuava verdadeiro, o vigia nunca via job PARADO e a tela dizia
/// «agendado» para um job que ninguem mais ia rodar. E a mesma forma do
/// `pulsando` do cluster: marca de vida sem `Drop`. Medido com o defeito
/// reposto: o `relogio_no_ar` seguiu verdadeiro 3 s depois da morte da thread.
pub(super) struct RelogioNoAr(pub(super) Arc<Servidor>);

impl Drop for RelogioNoAr {
    fn drop(&mut self) {
        self.0.relogio_de_jobs.store(false, Ordering::SeqCst);
    }
}

/// Desliga o `amostrador_no_ar` quando a thread do amostrador sai -- o irmao do
/// pedido 452, pelo mesmo motivo do [`RelogioNoAr`]: o retrato da telemetria
/// dizia `amostrador: true` de uma thread morta.
struct AmostradorNoAr(Arc<crate::telemetria::Telemetria>);

impl Drop for AmostradorNoAr {
    fn drop(&mut self) {
        self.0.desmarcar_amostrador();
    }
}
