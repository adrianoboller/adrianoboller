//! Sistema e disco, painel, memoria, diario, profiler e posicao.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

impl Servidor {
    // ----------------------------------------------------- a maquina embaixo

    /// Os caminhos cujo espaco em disco interessa a este servidor.
    ///
    /// O `base` sempre, porque e onde o dado mora. O destino do backup quando
    /// ha backup agendado -- e o disco que enche calado, porque ninguem olha
    /// para ele ate o dia em que o backup falha. E o que o operador acrescentar
    /// em `alertas.caminhos`.
    ///
    /// Repetido nao entra duas vezes; a mesma particao, sim: `base` e
    /// `backup.destino` podem cair na mesma montagem, e mostrar as duas linhas
    /// e o que responde "o disco do banco esta cheio?" sem obrigar ninguem a
    /// adivinhar qual montagem contem qual pasta.
    pub fn caminhos_vigiados(&self) -> Vec<PathBuf> {
        let mut v = vec![self.config.base.clone()];
        if self.config.backup.agendado {
            v.push(self.config.backup.destino.clone());
        }
        v.extend(self.config.alertas.caminhos.iter().cloned());
        v.dedup_by(|a, b| a == b);
        v
    }

    /// O retrato da maquina: CPU, memoria, discos, placas de rede e IO.
    ///
    /// Uma chamada so, pelo mesmo motivo do painel: cinco pedidos separados
    /// custariam cinco idas e voltas para mostrar um cabecalho.
    pub(super) fn op_sistema(&self) -> Json {
        let caminhos = self.caminhos_vigiados();
        let refs: Vec<&Path> = caminhos.iter().map(|p| p.as_path()).collect();
        let mut retrato = match self.monitor.lock() {
            Ok(mut m) => m.ler(&refs),
            // Trava envenenada nao pode derrubar o painel: monitor e o que
            // alguem abre JUSTAMENTE quando algo ja deu errado.
            Err(_) => crate::sistema::Monitor::novo().ler(&refs),
        };
        // O limite entra junto com a medida: sem ele a tela teria de conhecer
        // a regra do config para saber que barra pintar de vermelho, e a regra
        // acabaria escrita em dois lugares.
        if let Json::Objeto(campos) = &mut retrato {
            campos.push((
                "alertas".into(),
                Json::objeto(vec![
                    ("ligado", Json::Bool(self.config.alertas.ligado)),
                    (
                        "livre_minimo_percentual",
                        Json::texto_de(format!(
                            "{:.2}",
                            self.config.alertas.livre_minimo_percentual
                        )),
                    ),
                    (
                        "livre_minimo_mb",
                        Json::de_u64(self.config.alertas.livre_minimo_mb),
                    ),
                    ("email", Json::Bool(self.config.alertas.email.ligado)),
                ]),
            ));
            campos.push((
                "apertados".into(),
                Json::Lista(
                    self.discos_apertados()
                        .iter()
                        .map(|e| Json::texto_de(&e.caminho))
                        .collect(),
                ),
            ));
        }
        retrato
    }

    /// Quais dos caminhos vigiados estao abaixo do limite.
    ///
    /// Responde mesmo com os alertas desligados: o limite continua sendo a
    /// regra de "apertado", e o painel pinta a barra de vermelho de qualquer
    /// jeito. Desligado quer dizer "nao manda e-mail", nao "nao olha".
    fn discos_apertados(&self) -> Vec<crate::sistema::EspacoEmDisco> {
        let caminhos = self.caminhos_vigiados();
        let refs: Vec<&Path> = caminhos.iter().map(|p| p.as_path()).collect();
        crate::sistema::espaco(&refs)
            .into_iter()
            .filter(|e| {
                self.config
                    .alertas
                    .apertado(e.livre_percentual(), e.livre_kb)
            })
            .collect()
    }

    /// Confere o espaco de tempos em tempos e avisa quando aperta.
    ///
    /// Thread propria porque a conferencia chama o `df` e, no caso do aviso,
    /// abre uma conexao TCP com o rele de e-mail -- nenhuma das duas coisas
    /// pode acontecer no caminho de uma consulta.
    ///
    /// Sobe SEMPRE (pedido 496, F5): a previsao de esgotamento precisa da
    /// serie, e serie que so comeca no dia em que alguem liga o alerta nao
    /// preve nada nesse dia. `alertas.ligado` decide so o aviso de disco
    /// apertado; a previsao vai ao sedimento do aquario de qualquer jeito.
    pub(super) fn ligar_vigia_de_disco(self: &Arc<Self>) {
        let a = &self.config.alertas;
        if a.ligado {
            eprintln!(
                "vigia de disco: a cada {} min | aperta abaixo de {:.0}% livre ou {} MB | {}",
                a.checar_minutos,
                a.livre_minimo_percentual,
                a.livre_minimo_mb,
                if a.email.ligado {
                    format!("avisa {}", a.email.para.join(", "))
                } else {
                    "so no painel (e-mail desligado)".to_string()
                }
            );
        }
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "vigia-disco",
            "chama o `df` de tempos em tempos, amostra disco, memoria e \
             descritores para a previsao de esgotamento, e avisa quando o \
             espaco aperta; thread propria porque ela roda um programa do \
             sistema e pode abrir uma conexao com o rele de e-mail -- nenhuma \
             das duas coisas cabe no caminho de uma consulta",
            "servico",
            crate::agora_ms(),
            move |fio| {
                let intervalo = Duration::from_secs(servidor.config.alertas.checar_minutos * 60);
                loop {
                    fio.fazendo("amostrando para a previsao de esgotamento");
                    servidor.amostrar_e_prever(crate::agora_ms());
                    fio.fazendo("contando o que falta ate o teto duro");
                    servidor.amostrar_e_contar(crate::agora_ms());
                    if servidor.config.alertas.ligado {
                        fio.fazendo("conferindo o espaco em disco");
                        servidor.conferir_disco();
                    }
                    fio.fazendo(&format!(
                        "dormindo {} min ate a proxima conferencia",
                        servidor.config.alertas.checar_minutos
                    ));
                    std::thread::sleep(intervalo);
                }
            },
        );
    }

    /// O que o previsor le nesta rodada: o espaco livre de cada MONTAGEM
    /// vigiada (o `base` e o backup na mesma particao sao um disco so, e
    /// duas series dele dariam dois avisos de uma noticia), a memoria e os
    /// descritores.
    pub(super) fn leituras_do_previsor(&self) -> Vec<crate::previsao::Leitura> {
        let caminhos = self.caminhos_vigiados();
        let refs: Vec<&Path> = caminhos.iter().map(|p| p.as_path()).collect();
        let mut v: Vec<crate::previsao::Leitura> = Vec::new();
        for e in crate::sistema::espaco(&refs) {
            let recurso = format!("disco:{}", e.montagem);
            if v.iter().any(|l| l.recurso == recurso) {
                continue;
            }
            v.push(crate::previsao::Leitura {
                recurso,
                resta: e.livre_kb,
                piso: self.config.alertas.piso_kb(e.utilizavel_kb()),
            });
        }
        v.extend(crate::previsao::leituras_da_maquina());
        v
    }

    /// Uma rodada do previsor (pedido 496, F5): amostra e preve.
    pub(super) fn amostrar_e_prever(&self, agora_ms: i64) -> Vec<crate::previsao::Aviso> {
        let leituras = self.leituras_do_previsor();
        self.prever(agora_ms, &leituras)
    }

    /// Guarda as leituras, e toda previsao dentro dos degraus vira alarme de
    /// servidor pelo produtor UNICO (`telemetria::sinal`, A3) -- o sedimento
    /// do aquario. A cada rodada, e nao so na mudanca: o `visto_ms` da pedra
    /// e o que diz a tela que a previsao CONTINUA de pe. O `stderr` so ouve a
    /// mudanca de degrau.
    pub(super) fn prever(
        &self,
        agora_ms: i64,
        leituras: &[crate::previsao::Leitura],
    ) -> Vec<crate::previsao::Aviso> {
        let avisos = self.com_o_previsor(|p| p.rodada(agora_ms, leituras));
        for a in &avisos {
            let dados = format!(
                "{} esgota em {:.2} h ({:.0}/h, r2 {:.2})",
                a.recurso, a.previsao.horas, a.previsao.por_hora, a.previsao.r2
            );
            crate::telemetria::sinal(a.degrau.alarme(), &dados);
            if a.mudou {
                eprintln!("PREVISAO DE ESGOTAMENTO ({:?}): {dados}", a.degrau);
            }
        }
        avisos
    }

    /// O previsor, com o veneno recuperado: a serie pode estar pela metade,
    /// mas continua sendo serie -- calar o previsor no dia em que algo ja deu
    /// errado e o pior momento. UMA porta para a F5 e a F6.
    fn com_o_previsor<R>(&self, f: impl FnOnce(&mut crate::previsao::Previsor) -> R) -> R {
        match self.previsor.lock() {
            Ok(mut p) => f(&mut p),
            Err(e) => f(&mut e.into_inner()),
        }
    }

    /// Uma rodada da contagem regressiva (pedido 496, F6): le o que tem teto
    /// duro e avisa o que esta perto dele.
    pub(super) fn amostrar_e_contar(&self, agora_ms: i64) -> Vec<crate::previsao::AvisoDaContagem> {
        let leituras = self.contagens_do_servidor(agora_ms);
        self.contar_regressivo(agora_ms, &leituras)
    }

    /// Guarda as contagens e leva toda uma dentro dos degraus ao sedimento
    /// pelo produtor UNICO. O `Alarme` e o da F5 (`Esgotamento*`): a pergunta
    /// e a mesma («quanto falta ate o teto?»), e o recurso vai no `dados`.
    pub(super) fn contar_regressivo(
        &self,
        agora_ms: i64,
        leituras: &[crate::previsao::Contagem],
    ) -> Vec<crate::previsao::AvisoDaContagem> {
        let avisos = self.com_o_previsor(|p| p.contar(agora_ms, leituras));
        for a in &avisos {
            let dados = format!(
                "{} a {:.1}% do teto ({}/{}){}",
                a.recurso,
                a.percentual(),
                a.usado,
                a.teto,
                match &a.previsao {
                    Some(p) => format!(", cheio em {:.1} d", p.horas / 24.0),
                    None => String::new(),
                }
            );
            crate::telemetria::sinal(a.degrau.alarme(), &dados);
            if a.mudou {
                eprintln!("CONTAGEM REGRESSIVA ({:?}): {dados}", a.degrau);
            }
        }
        avisos
    }

    /// Tudo o que tem teto duro neste servidor, nesta rodada.
    fn contagens_do_servidor(&self, agora_ms: i64) -> Vec<crate::previsao::Contagem> {
        let mut v = self.contagens_dos_dados();
        v.extend(self.contagens_do_backup(agora_ms));
        v
    }

    /// C7 e C3: a tabela paginada que enche e o diario que bate no teto de
    /// volumes. Sob a ficha COMPARTILHADA e por tabelas abertas SO para ler:
    /// o vigia nao pode parar a escrita de ninguem a cada 15 min. Tabela que
    /// quer a ficha exclusiva para abrir fica sem amostra nesta rodada --
    /// amostra que falta nao e amostra que zerou.
    fn contagens_dos_dados(&self) -> Vec<crate::previsao::Contagem> {
        use phxsql_core::paginacao::ModoParticao;
        let mut v = Vec::new();
        let Ok(trava) = self.travar_dados_para_ler() else {
            return v;
        };
        for db in trava.databases().unwrap_or_default() {
            let Ok(Some(tabelas)) = trava.tabelas_para_ler(&db) else {
                continue;
            };
            for t in tabelas {
                let Ok(Aberta::Pronta(l)) = trava.abrir_diario_para_ler(&db, &t) else {
                    continue;
                };
                let pag = l.esquema().paginacao();
                if !pag.ligada() {
                    continue;
                }
                let (reg, _, _, log) = l.volumes_por_arquivo();
                match pag.modo {
                    // O endereco e uma divisao: o rowid so existe ate a
                    // capacidade, e `slots` e a marca d'agua que nunca recua.
                    ModoParticao::PorQuantidade => v.push(crate::previsao::Contagem {
                        recurso: format!("tabela:{db}.{t}"),
                        usado: l.slots(),
                        teto: pag.capacidade(),
                        tendencia: true,
                    }),
                    // Aqui o corte e o calendario: o que acaba e a conta de
                    // volumes, e e ela que o `.reg` confere ao virar de volume.
                    ModoParticao::PorPeriodo { .. } => v.push(crate::previsao::Contagem {
                        recurso: format!("tabela:{db}.{t}"),
                        usado: reg.len() as u64,
                        teto: pag.max_arquivos as u64,
                        tendencia: true,
                    }),
                    // 37 baldes fixos: o teto e por letra, e nao ha uma conta
                    // unica de «quanto falta» para a tabela inteira.
                    ModoParticao::PorLetra { .. } => {}
                }
                // O `.log` rola por tamanho em todos os modos (a particao
                // por letra volta ao sufixo numerico: `para_externos`), e o
                // numero do volume nunca recua -- o ultimo e o quanto ja se
                // gastou do teto.
                if let Some(&ultimo) = log.last() {
                    v.push(crate::previsao::Contagem {
                        recurso: format!("diario:{db}.{t}"),
                        usado: ultimo as u64,
                        teto: pag.para_externos().max_arquivos as u64,
                        tendencia: true,
                    });
                }
            }
        }
        v
    }

    /// C6 e C5: a idade do ultimo backup bom e quanto ele levou, do agendado
    /// e dos jobs cujo `op` e `backup`.
    ///
    /// Sem backup nenhum declarado, nada: «guarda nova entra pedida» -- um
    /// servidor de desenvolvimento nao tem periodo contra o qual comparar.
    fn contagens_do_backup(&self, agora_ms: i64) -> Vec<crate::previsao::Contagem> {
        use crate::previsao::{contagem_da_idade, contagem_da_janela};
        const HORA_MS: u64 = 3_600_000;
        const DIA_MS: u64 = 24 * HORA_MS;
        let mut v = Vec::new();
        let desde = self.backup_marcas.primeira_olhada(agora_ms);
        let b = &self.config.backup;
        if b.agendado {
            let periodo = if b.hora.is_empty() {
                b.cada_horas.saturating_mul(HORA_MS)
            } else {
                DIA_MS
            };
            let ultimo = self
                .backup_marcas
                .ultimo_ok(|| ultimo_toque_do_destino(&b.destino));
            v.push(contagem_da_idade(
                "backup:idade".into(),
                agora_ms,
                ultimo,
                desde,
                periodo,
            ));
            let duracao = self.backup_marcas.ultima_duracao_ms();
            if duracao > 0 {
                v.push(contagem_da_janela("backup:janela".into(), duracao, periodo));
            }
        }
        let jobs = self
            .jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for j in jobs.jobs.iter().filter(|j| j.op() == "backup") {
            let periodo = match j.agenda {
                crate::jobs::Agenda::Cada { minutos } => minutos.saturating_mul(60_000),
                crate::jobs::Agenda::Diaria { .. } => DIA_MS,
            };
            // So a ultima corrida e conhecida: se ela falhou, nao ha «ultimo
            // bom» a citar e a conta parte de `desde` -- a falha ja avisa
            // (510), o que falta aqui e a passagem do tempo sem sucesso.
            let boa = jobs
                .ultima_corrida_de(&j.nome)
                .filter(|c| c.ok && !c.em_curso);
            v.push(contagem_da_idade(
                format!("backup:idade:{}", j.nome),
                agora_ms,
                boa.map_or(0, |c| c.quando_ms),
                desde,
                periodo,
            ));
            if let Some(c) = boa.filter(|c| c.duracao_ms > 0) {
                v.push(contagem_da_janela(
                    format!("backup:janela:{}", j.nome),
                    c.duracao_ms as u64,
                    periodo,
                ));
            }
        }
        v
    }

    /// Uma rodada do vigia: olha os discos, avisa o que estiver apertado.
    ///
    /// O silencio entre dois avisos do mesmo caminho e por caminho, e nao
    /// global: dois discos apertando no mesmo dia sao duas noticias, nao uma.
    fn conferir_disco(&self) {
        let apertados = self.discos_apertados();
        if apertados.is_empty() {
            // Aliviou: esquece o que ja foi avisado, para que a proxima vez
            // avise de novo em vez de ficar calado pelas horas do silencio.
            if let Ok(mut a) = self.avisados.lock() {
                a.clear();
            }
            return;
        }
        let agora = crate::agora_ms();
        let silencio = self.config.alertas.repetir_horas as i64 * 3_600_000;
        let novos: Vec<&crate::sistema::EspacoEmDisco> = {
            let Ok(mut vistos) = self.avisados.lock() else {
                return;
            };
            // O silencio e o do `jobs::pode_avisar`, e nao uma copia dele em
            // linha (pedido 496, parecer do DBA §5: tres escritas da mesma
            // decisao; esta era a primeira).
            apertados
                .iter()
                .filter(|e| crate::jobs::pode_avisar(&mut vistos, &e.caminho, agora, silencio))
                .collect()
        };
        if novos.is_empty() {
            return;
        }
        for e in &novos {
            eprintln!(
                "DISCO APERTADO: {} ({}) -- {:.1}% livre, {} MB de espaco",
                e.caminho,
                e.montagem,
                e.livre_percentual(),
                e.livre_kb / 1_024
            );
        }
        if !self.config.alertas.email.ligado {
            return;
        }
        let assunto = format!(
            "{} ({} {})",
            self.config.alertas.email.assunto,
            novos.len(),
            if novos.len() == 1 {
                "caminho"
            } else {
                "caminhos"
            }
        );
        let corpo = Self::texto_do_alerta(&novos, agora);
        match crate::email::enviar(&self.config.alertas.email, &assunto, &corpo) {
            Ok(r) => eprintln!("alerta de disco enviado: {r}"),
            // Falhar em avisar tambem e noticia -- e ela nao pode sumir junto
            // com o aviso que nao saiu.
            Err(e) => eprintln!("alerta de disco NAO ENVIADO: {e}"),
        }
    }

    fn texto_do_alerta(discos: &[&crate::sistema::EspacoEmDisco], agora: i64) -> String {
        let mut t = String::new();
        t.push_str("O PhxSql esta com pouco espaco em disco.\n\n");
        for e in discos {
            // O "de" e o ALCANCAVEL, e nao o tamanho do disco: o percentual ao
            // lado ja e sobre ele, e misturar as duas bases daria uma conta que
            // nao fecha para quem le ("45% de 258 GB nao dao 17 GB"). A reserva
            // do sistema de arquivos aparece a parte, quando existe.
            t.push_str(&format!(
                "  {}\n    montagem  {} ({})\n    livre     {} MB de {} MB ({:.1}%)\n",
                e.caminho,
                e.montagem,
                e.dispositivo,
                e.livre_kb / 1_024,
                e.utilizavel_kb() / 1_024,
                e.livre_percentual()
            ));
            if e.reservado_kb() > 0 {
                t.push_str(&format!(
                    "    reserva   {} MB do sistema de arquivos, fora do alcance\n",
                    e.reservado_kb() / 1_024
                ));
            }
            t.push('\n');
        }
        t.push_str(&format!(
            "Servidor PhxSql {VERSAO}\nQuando: {}\n",
            phxsql_core::datahora::instante_iso(agora)
        ));
        t
    }

    // ---------------------------------------------------- a saude do disco

    /// Sobe a thread da saude do disco (pedido 249): a sonda canario E o
    /// carteiro dos avisos. Thread propria pelo mesmo motivo do vigia de
    /// espaco: ela faz `fsync` e fala com o rele, e nenhuma das duas coisas
    /// cabe no caminho de uma consulta -- nem debaixo da trava de dados.
    ///
    /// Sobe SEMPRE desde a F2 do pedido 495: alem da sonda e dos avisos, ela
    /// e o carteiro das ocorrencias, e o `ocorrencias.log` nao depende de
    /// e-mail nem de sonda. Sem ela a fila enche ate o teto e o arquivo nunca
    /// recebe nada -- o registro de seguranca calado justo no servidor que
    /// desligou o resto.
    pub(super) fn ligar_sonda_de_disco(self: &Arc<Self>) {
        let d = &self.config.alertas.disco;
        eprintln!(
            "sonda de disco: {} a cada {} s | silencio {} min por tipo | lento acima de {} ms | {}",
            self.config
                .base
                .join(crate::saude_do_disco::CANARIO)
                .display(),
            d.checar_segundos,
            d.repetir_minutos,
            d.lento_ms,
            match (
                self.config.alertas.email.ligado,
                self.config.alertas.sms.ligado,
                self.config.alertas.gancho.ligado,
            ) {
                (true, true, true) => "avisa por e-mail, SMS e gancho do operador",
                (true, false, true) => "avisa por e-mail e gancho do operador",
                (true, true, false) => "avisa por e-mail e SMS",
                (true, false, false) => "avisa por e-mail",
                (false, _, true) => "avisa pelo gancho do operador (e-mail desligado)",
                _ => "so no painel (e-mail desligado)",
            }
        );
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "sonda-disco",
            "escreve, sincroniza, rele e apaga o canario `.saude-do-disco` no \
             base, de tempos em tempos, e e o CARTEIRO dos avisos de saude: \
             dorme na fila, acorda na hora em que um evento entra e fala com \
             o rele fora de qualquer trava; thread propria porque `fsync` num \
             disco doente e rele fora do ar levam segundos -- nada disso cabe \
             no caminho de uma consulta, nem debaixo da trava de dados",
            "servico",
            crate::agora_ms(),
            move |fio| {
                let intervalo = Duration::from_secs(servidor.saude.checar_segundos());
                let sonda_ligada = servidor.saude.ligada();
                // A primeira sonda e ja: o painel nao pode esperar o
                // intervalo inteiro para dizer alguma coisa.
                let mut proxima = Instant::now();
                loop {
                    if sonda_ligada && Instant::now() >= proxima {
                        fio.fazendo("escrevendo, sincronizando e relendo o canario");
                        if let Some(evento) = servidor.saude.sondar(crate::agora_ms()) {
                            servidor.avisar_saude_do_disco(&fio, evento);
                        }
                        proxima = Instant::now() + intervalo;
                    }
                    // Sem sonda, o prazo e so o ritmo de reconferir o laco.
                    let ate = if sonda_ligada {
                        proxima.saturating_duration_since(Instant::now())
                    } else {
                        Duration::from_secs(60)
                    };
                    fio.fazendo("esperando um evento de saude, ou a hora da proxima sonda");
                    // O carteiro de verdade tira TODA carta: a da saude vira
                    // aviso, a ocorrencia vira linha no `ocorrencias.log`.
                    for carta in servidor.saude.correio().retirar(ate, |_| true) {
                        match carta {
                            crate::ocorrencias::Carta::Saude(evento) => {
                                servidor.avisar_saude_do_disco(&fio, evento)
                            }
                            crate::ocorrencias::Carta::Ocorrencia(o) => {
                                fio.fazendo("gravando uma ocorrencia");
                                servidor.gravar_ocorrencia(&o);
                            }
                        }
                    }
                }
            },
        );
    }

    /// A ocorrencia no `ocorrencias.log`. A falha de gravar e noticia de
    /// disco, e vai ao mesmo destino da falha do `aquario.log`.
    ///
    /// E o e-mail (F9) sai DAQUI, e nao do `sinal`: o produtor pode estar com
    /// a trava de dados na mao, e o carteiro nunca esta. O e-mail vem depois
    /// da gravacao -- o arquivo e o registro; o e-mail e o recado.
    pub(super) fn gravar_ocorrencia(&self, o: &crate::ocorrencias::Ocorrencia) {
        let gravou = self.ocorrencias.gravar(o);
        self.avisar_ocorrencia_por_email(o);
        if let Err(PhxError::Io(io)) = gravou {
            self.evento_de_disco(
                crate::saude_do_disco::classificar(&io),
                crate::ocorrencias::NOME_DO_ARQUIVO,
                "",
                "",
                &io.to_string(),
            );
        }
    }

    /// O aviso de um evento de saude que passou pelo silencio -- pelo
    /// [`Carteiro`], o motor UNICO que o arranque recusado tambem usa (573).
    fn avisar_saude_do_disco(
        &self,
        fio: &crate::telemetria::Fio,
        evento: crate::saude_do_disco::Evento,
    ) {
        Carteiro {
            config: &self.config,
            saude: &self.saude,
        }
        .avisar_saude_do_disco(fio, evento);
    }

    /// O SMS: UMA linha, ate 160 caracteres, sem caminho e sem segredo. SMS
    /// atravessa a operadora em claro, e o nome de usuario dentro de um
    /// caminho (`/home/fulano/dados`) e mais do que a operadora precisa saber.
    pub(super) fn texto_do_sms_de_saude(e: &crate::saude_do_disco::Evento) -> String {
        let quando = phxsql_core::datahora::instante_iso(e.quando_ms);
        // `2026-09-16T05:40:12.123` -> `05:40 UTC`.
        let hora = quando.get(11..16).unwrap_or("").to_string();
        let linha = if e.tipo == crate::saude_do_disco::Tipo::Backup {
            format!(
                "PhxSql {}: o backup agendado FALHOU {hora} UTC",
                crate::email::nome_da_maquina()
            )
        } else if e.tipo == crate::saude_do_disco::Tipo::Arranque {
            format!(
                "PhxSql {}: subiu depois de uma queda e reconstruiu indices {hora} UTC",
                crate::email::nome_da_maquina()
            )
        } else {
            format!(
                "PhxSql {}: disco {} em {}{} {hora} UTC",
                crate::email::nome_da_maquina(),
                Carteiro::tipo_de_saude_legivel(e.tipo),
                e.origem,
                Carteiro::alvo_do_evento(e)
            )
        };
        // O ajudante UNICO (pedido 643): `database`/`tabela` vem do pedido de
        // um usuario, e CR/LF nao eram os unicos controles perigosos. Vale
        // para o SMS por e-mail e para o stdin do gancho, que dividem esta
        // linha.
        crate::gancho::linha_limpa(&linha, 160)
    }

    /// A op `saude_disco`, so leitura. O texto do ultimo erro e o alvo dele
    /// (base/tabela) so saem para quem administra: o texto pode carregar
    /// caminho de disco, e o nome de uma base que a sessao nao pode abrir
    /// nao e dela.
    pub(super) fn op_saude_disco(&self, sessao: &Sessao) -> Json {
        self.saude.para_json(self.administra(sessao))
    }

    /// A sessao tem o poder de administrar? Sem cadastro (so o token), tem.
    fn administra(&self, sessao: &Sessao) -> bool {
        match &sessao.usuario {
            None => true,
            Some(u) => u.pode_em("", "", Atividade::Administrar),
        }
    }

    // -------------------------------------------------------------- o painel

    /// Tudo que o painel mostra, numa chamada so.
    ///
    /// Poderia ser dez chamadas do navegador, e o painel ficaria dez vezes
    /// mais lento por causa da ida e volta. Agregar aqui tambem deixa a conta
    /// do que o usuario PODE VER acontecer de um lado so: o painel nunca
    /// mostra numero de base que quem esta olhando nao poderia abrir.
    pub(super) fn op_painel(&self, sessao: &Sessao) -> Result<Json> {
        let agora = crate::agora_ms();

        // ---------------------------------------------------------- bancos
        let (mut bancos, mut tabelas_total, mut registros_total, mut bytes_total) =
            (Vec::new(), 0u64, 0u64, 0u64);
        let mut maiores: Vec<(String, u64, u64)> = Vec::new();
        {
            let dados = self.travar_dados()?;
            for nome in dados.databases()? {
                // O painel so conta o que quem esta olhando poderia abrir.
                if let Some(u) = &sessao.usuario {
                    if !u.pode(&nome, Atividade::Ler) {
                        continue;
                    }
                }
                let db = dados.abrir_database(&nome)?;
                // E so o que quem olha poderia abrir, tabela a tabela: o
                // total do painel nao pode contar linha de tabela negada.
                let lista: Vec<String> = db
                    .todas_as_tabelas()?
                    .into_iter()
                    .filter(|t| self.pode_ver_tabela(sessao, &nome, t))
                    .collect();
                let schemas = db.schemas()?.len() as u64;
                let mut registros_db = 0u64;
                for t in &lista {
                    if let Ok(tab) = db.abrir_qualificada(t) {
                        let regs = tab.registros();
                        registros_db += regs;
                        // O caminho de cada volume vem do motor que o abre
                        // (pedido 508). A copia que morava aqui punha o
                        // volume 1 sem sufixo e os outros em `_NNN` de tres
                        // digitos: numa tabela paginada o volume 1 nunca era
                        // somado, e numa de 4 digitos ou por letra nenhum.
                        let bytes: u64 = tab
                            .caminhos_do_reg()
                            .iter()
                            .map(|c| std::fs::metadata(c).map(|m| m.len()).unwrap_or(0))
                            .sum();
                        bytes_total += bytes;
                        maiores.push((format!("{nome}/{t}"), regs, bytes));
                    }
                }
                tabelas_total += lista.len() as u64;
                registros_total += registros_db;
                bancos.push(Json::objeto(vec![
                    ("nome", Json::texto_de(&nome)),
                    ("tabelas", Json::de_u64(lista.len() as u64)),
                    ("schemas", Json::de_u64(schemas)),
                    ("registros", Json::de_u64(registros_db)),
                ]));
            }
        }
        // As dez maiores, por registro. Mais que isso vira lista, nao grafico.
        maiores.sort_by(|a, b| b.1.cmp(&a.1));
        maiores.truncate(10);

        // --------------------------------------------------------- acessos
        //
        // Uma passada so sobre o log, alimentando todas as contagens de uma
        // vez. Ler o arquivo cinco vezes para responder cinco perguntas seria
        // o painel ficando lento com o log crescendo.
        let acessos = LogAcessos::ler(&self.config.log_acessos).unwrap_or_default();
        let dia_ms = 86_400_000i64;
        let desde = agora - dia_ms;
        let mut por_hora = [0u64; 24];
        let mut recusadas_por_hora = [0u64; 24];
        let mut por_op: HashMap<String, (u64, u64)> = HashMap::new();
        let mut por_usuario: HashMap<String, u64> = HashMap::new();
        let (mut ok, mut falhas, mut soma_ms) = (0u64, 0u64, 0u64);
        for a in &acessos {
            if a.ok {
                ok += 1;
            } else {
                falhas += 1;
            }
            soma_ms += a.duracao_ms;
            let e = por_op.entry(a.op.clone()).or_insert((0, 0));
            if a.ok {
                e.0 += 1;
            } else {
                e.1 += 1;
            }
            if !a.usuario.is_empty() {
                *por_usuario.entry(a.usuario.clone()).or_insert(0) += 1;
            }
            if a.quando_ms >= desde {
                // Balde por hora, contando de tras para frente a partir de
                // agora: o balde 23 e a hora corrente.
                let atras = ((agora - a.quando_ms) / 3_600_000) as usize;
                if atras < 24 {
                    let i = 23 - atras;
                    por_hora[i] += 1;
                    if !a.ok {
                        recusadas_por_hora[i] += 1;
                    }
                }
            }
        }
        let mut ops: Vec<(String, u64, u64)> =
            por_op.into_iter().map(|(k, (a, b))| (k, a, b)).collect();
        ops.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)));
        ops.truncate(12);
        let mut usuarios_ativos: Vec<(String, u64)> = por_usuario.into_iter().collect();
        usuarios_ativos.sort_by(|a, b| b.1.cmp(&a.1));
        usuarios_ativos.truncate(8);

        let ips = LogAcessos::resumo_por_ip(&self.config.log_acessos).unwrap_or_default();
        let mut top_ips: Vec<&crate::acesso::ResumoIp> = ips.iter().collect();
        top_ips.sort_by(|a, b| b.acessos.cmp(&a.acessos));
        top_ips.truncate(8);

        // -------------------------------------------------------- usuarios
        let cadastro = self.cadastro();
        let mut por_nivel: HashMap<&'static str, u64> = HashMap::new();
        for u in cadastro.root.iter().chain(cadastro.usuarios.iter()) {
            *por_nivel.entry(u.nivel.nome()).or_insert(0) += 1;
        }
        let ordem_nivel = ["admin", "dono", "operador", "leitor", "nenhum"];

        // --------------------------------------------------------- estado
        let bloqueios = self
            .lista_negra
            .lock()
            .map(|l| l.ativos(agora).len() as u64)
            .unwrap_or(0);
        let (residentes, bytes_ram) = self
            .residentes
            .lock()
            .map(|r| {
                (
                    r.len() as u64,
                    r.values().map(|m| m.bytes() as u64).sum::<u64>(),
                )
            })
            .unwrap_or((0, 0));
        let sessoes_web = self.sessoes.lock().map(|s| s.quantas() as u64).unwrap_or(0);

        Ok(Json::objeto(vec![
            (
                "quando",
                Json::texto_de(phxsql_core::datahora::instante_iso(agora)),
            ),
            ("versao", Json::texto_de(VERSAO)),
            ("papel", Json::texto_de(self.config.replicacao.papel.nome())),
            (
                "resumo",
                Json::objeto(vec![
                    ("bancos", Json::de_u64(bancos.len() as u64)),
                    ("tabelas", Json::de_u64(tabelas_total)),
                    ("registros", Json::de_u64(registros_total)),
                    ("bytes_reg", Json::de_u64(bytes_total)),
                    (
                        "usuarios",
                        Json::de_u64(
                            (cadastro.usuarios.len() + usize::from(cadastro.root.is_some())) as u64,
                        ),
                    ),
                    (
                        "conexoes",
                        Json::de_u64(self.permissoes_de_dados.em_uso() as u64),
                    ),
                    ("sessoes_web", Json::de_u64(sessoes_web)),
                    ("bloqueios", Json::de_u64(bloqueios)),
                    ("tabelas_em_ram", Json::de_u64(residentes)),
                    ("bytes_em_ram", Json::de_u64(bytes_ram)),
                    ("acessos", Json::de_u64(ok + falhas)),
                    ("acessos_ok", Json::de_u64(ok)),
                    ("acessos_recusados", Json::de_u64(falhas)),
                    (
                        "ms_medio",
                        Json::de_u64(if ok + falhas > 0 {
                            soma_ms / (ok + falhas)
                        } else {
                            0
                        }),
                    ),
                    ("espelho", Json::Bool(self.espelho())),
                    ("somente_leitura", Json::Bool(self.somente_leitura())),
                ]),
            ),
            ("bancos", Json::Lista(bancos)),
            // A saude do disco (pedido 249), ao lado do espaco que o
            // `sistema` ja traz. Reduzido para quem nao administra.
            (
                "saude_do_disco",
                self.saude.para_json(self.administra(sessao)),
            ),
            (
                "maiores_tabelas",
                Json::Lista(
                    maiores
                        .iter()
                        .map(|(n, r, b)| {
                            Json::objeto(vec![
                                ("tabela", Json::texto_de(n)),
                                ("registros", Json::de_u64(*r)),
                                ("bytes", Json::de_u64(*b)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "por_hora",
                Json::Lista(por_hora.iter().map(|n| Json::de_u64(*n)).collect()),
            ),
            (
                "recusadas_por_hora",
                Json::Lista(
                    recusadas_por_hora
                        .iter()
                        .map(|n| Json::de_u64(*n))
                        .collect(),
                ),
            ),
            (
                "por_operacao",
                Json::Lista(
                    ops.iter()
                        .map(|(o, a, r)| {
                            Json::objeto(vec![
                                ("op", Json::texto_de(o)),
                                ("ok", Json::de_u64(*a)),
                                ("recusados", Json::de_u64(*r)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "por_nivel",
                Json::Lista(
                    ordem_nivel
                        .iter()
                        .filter_map(|n| {
                            por_nivel.get(n).map(|q| {
                                Json::objeto(vec![
                                    ("nivel", Json::texto_de(*n)),
                                    ("quantos", Json::de_u64(*q)),
                                ])
                            })
                        })
                        .collect(),
                ),
            ),
            (
                "usuarios_ativos",
                Json::Lista(
                    usuarios_ativos
                        .iter()
                        .map(|(u, q)| {
                            Json::objeto(vec![
                                ("usuario", Json::texto_de(u)),
                                ("acessos", Json::de_u64(*q)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "top_ips",
                Json::Lista(
                    top_ips
                        .iter()
                        .map(|r| {
                            Json::objeto(vec![
                                ("ip", Json::texto_de(&r.ip)),
                                ("acessos", Json::de_u64(r.acessos)),
                                ("recusados", Json::de_u64(r.recusados)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    // ------------------------------------------------------ tabela em memoria

    /// Chave de residencia. Inclui o database porque duas bases podem ter
    /// tabela de mesmo nome -- e teriam, se ninguem cuidasse disso.
    pub(super) fn chave_residente(p: &Json) -> String {
        format!(
            "{}/{}",
            p.texto_ou("database", ""),
            p.texto_ou("tabela", "")
        )
    }

    /// Mexe na copia residente, se a tabela deste pedido estiver carregada.
    pub(super) fn residente_mut(&self, p: &Json, f: impl FnOnce(&mut TabelaMemoria)) {
        if let Ok(mut r) = self.residentes.lock() {
            if let Some(m) = r.get_mut(&Self::chave_residente(p)) {
                f(m);
            }
        }
    }

    /// Le a tabela inteira para a RAM.
    pub(super) fn op_memoria_carregar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let esquema = t.esquema().clone();

        // As colunas com mapa de igualdade. Sem pedido, mapeia as que ja sao
        // primeira coluna de algum indice: quem indexou no disco costuma
        // filtrar pelo mesmo campo na memoria.
        let mapear: Vec<usize> = match p.campo("mapear").and_then(Json::lista) {
            Some(l) => l
                .iter()
                .map(|j| coluna_de(j, &esquema))
                .collect::<Result<Vec<usize>>>()?,
            None => {
                let mut v: Vec<usize> = esquema
                    .indices()
                    .iter()
                    .filter_map(|i| i.colunas.first().map(|c| c.coluna))
                    .collect();
                v.sort_unstable();
                v.dedup();
                v
            }
        };

        let inicio = Instant::now();
        // A trava de dados JA esta tomada, no topo desta funcao.
        //
        // Aqui havia uma segunda tomada da MESMA trava, e `Mutex` da `std` nao
        // e reentrante: a operacao travava a si mesma, e com ela o servidor
        // inteiro -- toda operacao de dados passa por esta trava. Ninguem
        // percebeu porque nao havia teste que chamasse `memoria_carregar`, e
        // pela tela a chamada simplesmente nunca voltava.
        //
        // Achado pelo teste do teto de `memoria_max_mb`: o primeiro que
        // precisou carregar uma tabela residente de verdade.
        let m = TabelaMemoria::carregar(&mut t, &mapear, crate::agora_ms())?;
        let ficha = ficha_residente(&Self::chave_residente(p), &m);
        let ms = inicio.elapsed().as_millis() as u64;
        let chave = Self::chave_residente(p);
        let mut residentes = self.residentes.tomar("residentes")?;

        // O TETO de memoria das tabelas residentes.
        //
        // `recursos.memoria_max_mb` estava no config.json, no MANUAL e na tela
        // desde a 0.13.0 e NENHUMA linha o lia -- a mesma armadilha do
        // `cache_paginas` sem cache. Aqui e o unico lugar onde a memoria
        // residente cresce, entao e aqui que o teto vale.
        //
        // Zero continua sendo SEM TETO, que e o padrao e o comportamento de
        // sempre: quem nunca preencheu o campo nao ve diferenca nenhuma.
        //
        // A conta troca o que a chave ja ocupava pelo tamanho novo, senao
        // recarregar a MESMA tabela contaria duas vezes e recusaria sozinha.
        let teto = self.config.recursos.memoria_max_mb;
        let ja: u64 = residentes
            .iter()
            .filter(|(k, _)| *k != &chave)
            .map(|(_, r)| r.bytes() as u64)
            .sum();
        let depois = ja + m.bytes() as u64;
        if !cabe_na_memoria(depois, teto) {
            return Err(PhxError::Esquema(format!(
                "carregar {chave} passaria do teto de memoria residente: \
                 {} MB depois de carregar, contra {teto} MB em \
                 recursos.memoria_max_mb. Libere uma tabela com \
                 memoria_liberar, ou suba o teto",
                depois.div_ceil(1024 * 1024)
            )));
        }
        residentes.insert(chave, m);
        drop(residentes);

        let mut campos = ficha;
        campos.push(("carregou_em_ms", Json::de_u64(ms)));
        Ok(Json::objeto(campos))
    }

    pub(super) fn op_memoria_liberar(&self, p: &Json) -> Result<Json> {
        let chave = Self::chave_residente(p);
        let saiu = self
            .residentes
            .tomar("residentes")?
            .remove(&chave)
            .is_some();
        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(&chave)),
            ("estava_carregada", Json::Bool(saiu)),
        ]))
    }

    /// O que esta residente agora.
    pub(super) fn op_memoria(&self) -> Result<Json> {
        let r = self.residentes.tomar("residentes")?;
        let mut chaves: Vec<&String> = r.keys().collect();
        chaves.sort();
        let agora = crate::agora_ms();
        Ok(Json::objeto(vec![
            ("tabelas", Json::de_u64(r.len() as u64)),
            (
                "bytes",
                Json::de_u64(r.values().map(|m| m.bytes() as u64).sum()),
            ),
            (
                "residentes",
                Json::Lista(
                    chaves
                        .into_iter()
                        .map(|c| {
                            let m = &r[c];
                            let mut f = ficha_residente(c, m);
                            f.push((
                                "carregada_ha_s",
                                Json::de_u64(((agora - m.carregada_ms()) / 1000).max(0) as u64),
                            ));
                            Json::objeto(f)
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    /// `SelectMemory`: a consulta que nao toca em disco.
    ///
    /// Recusa em vez de adivinhar quando a tabela nao esta carregada. Carregar
    /// uma tabela grande sem ninguem ter pedido seria a operacao rapida virando
    /// a operacao lenta, calada, na hora errada.
    pub(super) fn op_selecionar_memoria(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let chave = Self::chave_residente(p);
        let r = self.residentes.tomar("residentes")?;
        let m = r.get(&chave).ok_or_else(|| {
            PhxError::NaoEncontrado(format!(
                "{chave} nao esta em memoria; carregue antes com {{\"op\":\"memoria_carregar\",\"database\":...,\"tabela\":...}}"
            ))
        })?;
        let esquema = m.esquema();

        // O poder vale igual na memoria e no disco. O portao ja passou pelo
        // despachar; isto e o cinto: quem chegar aqui por outro caminho para.
        if let Some(u) = &sessao.usuario {
            let (base, tabela) = (p.texto_ou("database", ""), p.texto_ou("tabela", ""));
            if !u.pode_em(base, tabela, Atividade::Ler) {
                return Err(PhxError::Autorizacao(format!(
                    "{} nao tem permissao de ler em {base}.{tabela}",
                    u.login
                )));
            }
        }

        let onde = filtros_do_pedido(p, esquema)?;
        let expressao = expressao_do_pedido(p, esquema)?;

        let mut ordenar = Vec::new();
        if let Some(l) = p.campo("ordenar").and_then(Json::lista) {
            for o in l {
                ordenar.push(Ordem {
                    coluna: coluna_de(
                        o.campo("coluna")
                            .ok_or_else(|| PhxError::Esquema("ordem sem \"coluna\"".into()))?,
                        esquema,
                    )?,
                    desc: o.booleano_ou("desc", false),
                });
            }
        }

        let colunas = match p.campo("colunas").and_then(Json::lista) {
            Some(l) => l
                .iter()
                .map(|j| coluna_de(j, esquema))
                .collect::<Result<Vec<usize>>>()?,
            None => Vec::new(),
        };

        let consulta = Consulta {
            onde,
            expressao,
            ordenar,
            colunas,
            pular: p.inteiro_ou("pular", 0).max(0) as u64,
            max: self.limite(p),
        };

        let inicio = Instant::now();
        let saida = m.selecionar(&consulta)?;
        let us = inicio.elapsed().as_micros() as u64;

        // A projecao muda as colunas, entao os nomes vem com o resultado --
        // senao quem le nao sabe qual campo e qual.
        let indices: Vec<usize> = if consulta.colunas.is_empty() {
            (0..esquema.colunas().len()).collect()
        } else {
            consulta.colunas.clone()
        };
        let nomes: Vec<String> = indices
            .iter()
            .map(|i| esquema.colunas()[*i].nome.clone())
            .collect();
        let tipos: Vec<phxsql_core::types::ColumnType> =
            indices.iter().map(|i| esquema.colunas()[*i].ty).collect();

        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(&chave)),
            (
                "colunas",
                Json::Lista(nomes.iter().map(Json::texto_de).collect()),
            ),
            ("achadas", Json::de_u64(saida.achadas)),
            ("devolvidas", Json::de_u64(saida.linhas.len() as u64)),
            ("examinadas", Json::de_u64(saida.examinadas)),
            (
                "por_mapa",
                match &saida.por_mapa {
                    Some(c) => Json::texto_de(c),
                    None => Json::Nulo,
                },
            ),
            ("us", Json::de_u64(us)),
            (
                "linhas",
                Json::Lista(
                    saida
                        .linhas
                        .iter()
                        .map(|(rowid, l)| {
                            let mut campos = vec![("rowid", Json::de_u64(*rowid))];
                            for ((n, v), ty) in nomes.iter().zip(l.iter()).zip(tipos.iter()) {
                                campos.push((n.as_str(), crate::valores::valor_para_json(v, ty)));
                            }
                            Json::objeto(campos)
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    pub(super) fn op_diario(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let max = self.limite(p) as usize;
        let rowid = p.campo("rowid").and_then(Json::inteiro).map(|n| n as u64);
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let (total, eventos) = match rowid {
            Some(r) => {
                let h = t.historico(r)?;
                (h.len() as u64, h)
            }
            None => {
                // So a CAUDA. A resposta sempre foi "os `max` mais recentes",
                // e para chega-la o diario inteiro era lido para a memoria e
                // descartado ate sobrar `max` -- com a trava global na mao.
                // Irmao do A2 da revisao SEC de 17/09/2026: a resposta e a
                // mesma byte a byte, o que muda e o que se aloca.
                let total = t.eventos()?;
                // A cauda nunca comeca antes do que o expurgo deixou (706).
                let de = total.saturating_sub(max as u64).max(t.base_do_diario()?);
                (total, t.diario(de, max as u64)?)
            }
        };
        let recentes: Vec<Json> = eventos
            .iter()
            .rev()
            .take(max)
            .rev()
            .map(|e| {
                Json::objeto(vec![
                    ("quando", Json::texto_de(e.instante_iso())),
                    ("carimbo_ms", Json::Numero(e.carimbo as f64)),
                    ("operacao", Json::texto_de(e.operacao.nome())),
                    ("rowid", Json::de_u64(e.rowid)),
                    ("versao", Json::de_u64(e.versao)),
                    ("usuario", Json::de_u64(e.usuario as u64)),
                ])
            })
            .collect();
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(total)),
            ("eventos", Json::Lista(recentes)),
        ]))
    }

    // ------------------------------------------------------------- profiler

    /// **E administrador DESTE servidor?** -- a pergunta que o portao geral
    /// nao consegue fazer sobre as quatro operacoes do profiler.
    ///
    /// # Por que ele existe, se o `da_operacao` ja pede `Administrar`
    ///
    /// Pela mesma razao do `portao_da_telemetria`, e o custo aqui e maior.
    /// Nenhum pedido do profiler tem campo `"database"`, entao o portao 3 do
    /// `despachar` pergunta «este usuario pode administrar a base VAZIA?» --
    /// e quem tem `bases: {"*": {administrar: true}}` responde sim sem ser
    /// administrador de nada.
    ///
    /// Provado por soquete antes de virar codigo, num servidor com tres
    /// usuarios: o `curioso` -- nivel leitor, `ler` so em `loja`,
    /// `administrar` na regra `"*"` -- levou **acesso negado** ao pedir
    /// `ler` em `folha.salarios`, ligou o profiler no pedido seguinte, e leu
    /// no anel o texto inteiro do `inserir` que o `adm` fez naquela mesma
    /// tabela, valor incluido. A telemetria mostra o NOME da tabela; o
    /// profiler mostra a LINHA.
    ///
    /// E ha o segundo poder, que nem a telemetria tem: `profiler_ligar`
    /// escolhe um caminho no disco e o servidor cria e escreve nele. Nao e
    /// direito de leitor mandar o servidor abrir arquivo.
    ///
    /// Sem cadastro de usuarios, quem entrou pelo token de servico continua
    /// podendo -- e assim que toda operacao de administracao ja funciona, e
    /// apertar isso aqui tiraria um direito que ninguem pediu para tirar.
    fn portao_do_profiler(&self, sessao: &Sessao) -> Result<()> {
        match &sessao.usuario {
            None => Ok(()),
            Some(u) if u.e_admin() => Ok(()),
            Some(u) => Err(PhxError::Autorizacao(format!(
                "{} nao e administrador deste servidor; o profiler mostra o \
                 texto dos pedidos de todo mundo, inclusive das tabelas que \
                 este login nao pode ler",
                u.login
            ))),
        }
    }

    /// `profiler_ligar`: comeca a observar o que chega pela porta.
    ///
    /// **So administrador**, e a razao esta no que ele mostra: o texto dos
    /// pedidos de todo mundo, com os dados que estao sendo gravados dentro.
    /// Quem pode ler uma tabela nao ganha por isto o direito de ver o que os
    /// outros escrevem nela.
    pub(super) fn op_profiler_ligar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        self.portao_do_profiler(sessao)?;
        let filtro = crate::profiler::Filtro {
            database: p.texto_ou("database", "").trim().to_string(),
            usuario: p.texto_ou("usuario", "").trim().to_string(),
            op: p.texto_ou("operacao", "").trim().to_string(),
            so_escrita: p.booleano_ou("so_escrita", false),
        };
        let arquivo = p.texto_ou("arquivo", "").to_string();
        let teto = p.inteiro_ou("guardar", 500).max(0) as usize;
        let agora = crate::agora_ms();
        let mut prof = self.profiler.tomar("profiler")?;
        // ANTES do `ligar`: e ele quem le o tamanho do arquivo que ja existe
        // para saber quanto falta para o primeiro rodizio.
        prof.definir_rodizio(
            self.config.profiler.teto_do_arquivo(),
            self.config.profiler.arquivos,
        );
        // A lista de tabelas declaradas em `cifra.tabelas` entra ANTES do
        // `ligar`, e nao depois: entre um e outro cabe um pedido, e um pedido
        // e o bastante para o `perfil.txt` receber em claro o payload que a
        // lista existe para manter fora dele.
        prof.definir_sigilosas(&self.config.cifra.tabelas);
        // E a raiz de dados no mesmo lugar e pelo mesmo motivo (pedido 356):
        // a lista e a INTENCAO do dono, e quem sabe se o `.reg` esta cifrado e
        // o disco. Sem a raiz o Profiler volta a decidir por um campo so, e
        // volta calado -- a tabela cifrada que ninguem declarou e o caso
        // comum, porque `cifra.tabelas` nasce vazia.
        prof.definir_raiz_dos_dados(&self.config.base);
        prof.ligar(filtro, &arquivo, teto, agora)?;
        // Dentro da trava, e DEPOIS de `ligar` ter dado certo: um espelho que
        // sobe antes faria o caminho quente pagar por um profiler que nao ligou.
        self.profiler_ligado.store(true, Ordering::Relaxed);
        Ok(Json::objeto(vec![
            ("ligado", Json::Bool(true)),
            ("guardar", Json::de_u64(prof.teto() as u64)),
            (
                "arquivo",
                Json::texto_de(prof.caminho().display().to_string()),
            ),
            // O teto vai na resposta de LIGAR porque e onde alguem escolhe o
            // caminho do arquivo: e a hora de saber quanto ele pode comer.
            (
                "teto_do_arquivo_bytes",
                Json::de_u64(prof.teto_do_arquivo()),
            ),
            ("arquivos_guardados", Json::de_u64(prof.manter() as u64)),
            ("teto_em_disco_bytes", Json::de_u64(prof.teto_em_disco())),
            // A lista sai NAS DUAS respostas -- a de ligar e a que a tela
            // pergunta a cada segundo. Na de ligar porque e a hora em que
            // alguem escolhe o arquivo; na outra porque `cifra.tabelas` vale a
            // quente, e uma tela que so soubesse do que estava valendo no
            // `ligar` mostraria a lista de antes da mudanca.
            (
                "tabelas_sem_texto",
                Json::Lista(prof.sigilosas().iter().map(Json::texto_de).collect()),
            ),
            (
                "desde",
                Json::texto_de(phxsql_core::datahora::instante_iso(agora)),
            ),
        ]))
    }

    pub(super) fn op_profiler_desligar(&self, sessao: &Sessao) -> Result<Json> {
        self.portao_do_profiler(sessao)?;
        let mut prof = self.profiler.tomar("profiler")?;
        let n = prof.observados();
        prof.desligar(crate::agora_ms());
        self.profiler_ligado.store(false, Ordering::Relaxed);
        Ok(Json::objeto(vec![
            ("ligado", Json::Bool(false)),
            ("observados", Json::de_u64(n)),
        ]))
    }

    pub(super) fn op_profiler_limpar(&self, sessao: &Sessao) -> Result<Json> {
        self.portao_do_profiler(sessao)?;
        let mut prof = self.profiler.tomar("profiler")?;
        prof.limpar();
        Ok(Json::objeto(vec![("limpo", Json::Bool(true))]))
    }

    /// `profiler`: o que foi observado, do mais recente para o mais antigo.
    pub(super) fn op_profiler(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        self.portao_do_profiler(sessao)?;
        let max = p.inteiro_ou("max", 200).max(0) as usize;
        // `desde_serial` deixa a tela pedir so o que ainda nao viu, em vez de
        // rebaixar o anel inteiro a cada atualizacao.
        let desde = p.inteiro_ou("desde_serial", 0).max(0) as u64;
        let prof = self.profiler.tomar("profiler")?;
        let f = prof.filtro();

        let eventos: Vec<Json> = prof
            .eventos(max.clamp(1, 5_000))
            .into_iter()
            .filter(|e| e.serial > desde)
            .map(|e| {
                Json::objeto(vec![
                    ("serial", Json::de_u64(e.serial)),
                    (
                        "quando",
                        Json::texto_de(phxsql_core::datahora::instante_iso(e.quando_ms)),
                    ),
                    ("ip", Json::texto_de(e.ip)),
                    ("usuario", Json::texto_de(e.usuario)),
                    ("op", Json::texto_de(e.op)),
                    ("database", Json::texto_de(e.database)),
                    ("tabela", Json::texto_de(e.tabela)),
                    ("bytes", Json::de_u64(e.bytes as u64)),
                    // Ja vem redigido: os campos de senha viraram *** antes de
                    // encostar no anel.
                    ("pedido", Json::texto_de(e.pedido)),
                    (
                        "ms",
                        match e.duracao_ms {
                            Some(ms) => Json::de_u64(ms),
                            None => Json::Nulo,
                        },
                    ),
                    (
                        "ok",
                        match e.ok {
                            Some(v) => Json::Bool(v),
                            None => Json::Nulo,
                        },
                    ),
                    ("erro", Json::texto_de(e.erro)),
                ])
            })
            .collect();

        Ok(Json::objeto(vec![
            ("ligado", Json::Bool(prof.ligado())),
            (
                "arquivo",
                Json::texto_de(prof.caminho().display().to_string()),
            ),
            ("observados", Json::de_u64(prof.observados())),
            ("esquecidos", Json::de_u64(prof.esquecidos())),
            // O tamanho do arquivo e as linhas que ele RECUSOU. Sem os dois, a
            // tela dizia «gravando em ...» com a particao cheia -- medido: 400
            // pedidos, 223 linhas no arquivo, nenhum aviso.
            ("gravados_bytes", Json::de_u64(prof.gravados())),
            ("falhas_de_escrita", Json::de_u64(prof.falhas_de_escrita())),
            // O rodizio, pelo mesmo motivo das duas de cima: sem estes tres, a
            // tela diria «gravando em perfil.txt» sem contar que o arquivo ja
            // virou quatro vezes e o comeco da sessao nao esta mais la.
            ("rodizios", Json::de_u64(prof.rodizios())),
            ("falhas_de_rodizio", Json::de_u64(prof.falhas_de_rodizio())),
            (
                "teto_do_arquivo_bytes",
                Json::de_u64(prof.teto_do_arquivo()),
            ),
            ("arquivos_guardados", Json::de_u64(prof.manter() as u64)),
            ("teto_em_disco_bytes", Json::de_u64(prof.teto_em_disco())),
            // A lista sai NAS DUAS respostas -- a de ligar e a que a tela
            // pergunta a cada segundo. Na de ligar porque e a hora em que
            // alguem escolhe o arquivo; na outra porque `cifra.tabelas` vale a
            // quente, e uma tela que so soubesse do que estava valendo no
            // `ligar` mostraria a lista de antes da mudanca.
            (
                "tabelas_sem_texto",
                Json::Lista(prof.sigilosas().iter().map(Json::texto_de).collect()),
            ),
            ("guardar", Json::de_u64(prof.teto() as u64)),
            (
                "desde",
                Json::texto_de(if prof.ligado() {
                    phxsql_core::datahora::instante_iso(prof.ligado_em_ms())
                } else {
                    String::new()
                }),
            ),
            (
                "filtro",
                Json::objeto(vec![
                    ("database", Json::texto_de(&f.database)),
                    ("usuario", Json::texto_de(&f.usuario)),
                    ("operacao", Json::texto_de(&f.op)),
                    ("so_escrita", Json::Bool(f.so_escrita)),
                ]),
            ),
            ("eventos", Json::Lista(eventos)),
        ]))
    }

    // ----------------------------------------------------------- replicacao

    /// `posicao`: quantos eventos cada tabela do database ja tem.
    ///
    /// E o equivalente do `SHOW MASTER STATUS`, e o que a replica compara com
    /// a propria posicao para saber o que falta. Sai do cabecalho de cada
    /// volume do `.log`, sem ler evento nenhum.
    ///
    /// Por que POR TABELA e nao por servidor: o PhxSql ainda nao tem transacao
    /// entre tabelas, entao nao existe ordem global a preservar -- e um numero
    /// por tabela deixa as tabelas replicarem em paralelo.
    pub(super) fn op_posicao(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "").to_string();
        if database.is_empty() {
            return Err(PhxError::Esquema("informe \"database\"".into()));
        }
        let _trava = self.travar_dados()?;
        let db = _trava.abrir_database(&database)?;
        let com_esquema = p.booleano_ou("com_esquema", false);
        let mut posicoes = Vec::new();
        for nome in db.todas_as_tabelas()? {
            // `posicao` tambem varre a base inteira sem campo `tabela`, e o
            // portao geral so ve o de cima -- a mesma familia do `juntar`, do
            // `unir`, do `pivotar` e do `sequencias`. Aqui a conferencia e de
            // `replicar`, e nao de `ler`, porque e o direito que o portao
            // aplicou a operacao: quem nao pode replicar a folha nao pode
            // saber por `posicao` quantos eventos ela tem, nem pedir o
            // esquema cru dela com `com_esquema`.
            //
            // Sem regra de tabela nada muda: a replica de sempre tem
            // `replicar` na base e nenhuma regra por tabela, e continua vendo
            // todas -- e uma sessao interna (sem usuario) tambem.
            if !replica_alcanca(sessao.usuario.as_ref(), &database, &nome) {
                continue;
            }
            let mut t = db.abrir_qualificada(&nome)?;
            let identidade: Option<Vec<String>> =
                bidirecional::chave_unica(t.esquema()).map(|(_, cols)| {
                    cols.iter()
                        .map(|c| t.esquema().colunas()[*c].nome.clone())
                        .collect()
                });
            // A base do diario (pedido 706): o primeiro evento que ainda
            // existe aqui. A replica atras dela se refaz por retrato.
            let (eventos, base) = t.eventos_e_base()?;
            let mut campos = vec![
                ("eventos".to_string(), Json::de_u64(eventos)),
                ("base".to_string(), Json::de_u64(base)),
                ("registros".to_string(), Json::de_u64(t.registros())),
                // A chave unica, se houver: e a identidade que o modo
                // bidirecional exige, e e aqui que um assistente descobre
                // ANTES de configurar que a tabela nao tem uma. `chave` segue
                // com o NOME quando e uma coluna so -- quem le hoje continua
                // lendo igual -- e a composta junta os nomes por virgula;
                // `chave_colunas` e a lista, que e a forma sem ambiguidade
                // (pedido 331).
                (
                    "chave".to_string(),
                    match &identidade {
                        Some(nomes) => Json::texto_de(nomes.join(",")),
                        None => Json::Nulo,
                    },
                ),
                (
                    "chave_colunas".to_string(),
                    match &identidade {
                        Some(nomes) => Json::Lista(nomes.iter().map(Json::texto_de).collect()),
                        None => Json::Nulo,
                    },
                ),
            ];
            // O contador da `Sequence` (pedido 229, c-pleno): a replica o adota
            // antes de puxar evento nenhum, e e isso que impede a promocao de
            // uma replica atrasada de reemitir numero que este master ja
            // entregou. So a tabela com `Sequence` ja usada paga o campo.
            if t.esquema().coluna_sequencia().is_some() && t.sequencia_atual() > 0 {
                campos.push((
                    "proxima_sequencia".to_string(),
                    crate::replica::proxima_sequencia_para_o_fio(t.sequencia_atual()),
                ));
            }
            if com_esquema {
                // O bloco de esquema CRU, do jeito que mora no `.reg`. A
                // replica desserializa o mesmo bloco e cria a tabela dela --
                // sem remontar coluna por coluna a partir de JSON, que e onde
                // um tipo ou uma escala se perderiam sem ninguem notar.
                campos.push((
                    "esquema".to_string(),
                    Json::texto_de(bytes_para_hex(&t.esquema().serializar())),
                ));
            }
            posicoes.push((nome, Json::Objeto(campos)));
        }
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("papel", Json::texto_de(self.papel_atual().nome())),
            // A identidade deste servidor: o bidirecional confere aqui a
            // colisao de hash antes de confiar na supressao de origem.
            (
                "id_servidor",
                Json::texto_de(&self.config.replicacao.id_servidor),
            ),
            // O numero de origem EFETIVO (pedido 329): o atribuido, senao o
            // hash. Mandar o efetivo, e nao o configurado, poupa quem le de
            // repetir a regra do «senao» -- e uma segunda copia dela
            // divergiria no dia em que a primeira mudasse.
            (
                "numero_servidor",
                Json::de_u64(self.config.replicacao.numero() as u64),
            ),
            // Sem a imagem ligada o diario existe mas nao replica, e a replica
            // precisa saber disso ANTES de puxar mil eventos inaplicaveis.
            (
                "imagem_da_linha",
                Json::Bool(self.config.replicacao.imagem_da_linha),
            ),
            // O diario daqui grava o id de transacao e o `replicar` o manda
            // (pedido 676). A replica anterior ignora o campo e aplica
            // evento a evento, como sempre; a replica nova que nao o ve sabe
            // que a origem e velha e diz isso.
            ("tx_no_diario", Json::Bool(true)),
            ("tabelas", Json::Objeto(posicoes)),
            ("usuario", Json::de_u64(sessao.id() as u64)),
        ]))
    }
}

/// Quando alguem tocou pela ultima vez no destino do backup (ms desde 1970),
/// ou zero se nao ha nada la. A lapide da corrida em curso nao conta: ela
/// prova que o backup COMECOU, e a idade pergunta pelo que TERMINOU. Serve de
/// semente depois de um reinicio, quando a memoria do servidor nao sabe de
/// backup nenhum mas o disco sabe.
fn ultimo_toque_do_destino(destino: &Path) -> i64 {
    let Ok(dir) = std::fs::read_dir(destino) else {
        return 0;
    };
    dir.flatten()
        .filter(|e| e.file_name() != LAPIDE_DO_BACKUP)
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .max()
        .unwrap_or(0)
}

/// Uma coluna, pelo nome ou pelo numero. Aceitar os dois e o que deixa a
/// consulta legivel a mao e barata pela interface.
pub(super) fn coluna_de(j: &Json, esquema: &phxsql_core::schema::Schema) -> Result<usize> {
    if let Some(n) = j.inteiro() {
        let i = n as usize;
        if n < 0 || i >= esquema.colunas().len() {
            return Err(PhxError::Esquema(format!("coluna {n} nao existe")));
        }
        return Ok(i);
    }
    let nome = j.texto().unwrap_or("");
    esquema
        .colunas()
        .iter()
        .position(|c| c.nome == nome)
        .ok_or_else(|| PhxError::Esquema(format!("coluna {nome:?} nao existe")))
}

/// O total residente cabe no teto de `recursos.memoria_max_mb`?
///
/// Funcao a parte para a decisao poder ser provada sem montar uma tabela de
/// megabytes: o teto e em MB, e o menor que existe e 1 MB -- um teste que
/// dependesse do volume precisaria inserir milhares de linhas para dizer o que
/// esta conta diz em uma.
///
/// Zero e SEM TETO, e e o padrao: quem nunca preencheu o campo nao ve
/// diferenca nenhuma.
pub(super) fn cabe_na_memoria(bytes_depois: u64, teto_mb: u64) -> bool {
    teto_mb == 0 || bytes_depois <= teto_mb * 1024 * 1024
}

fn ficha_residente(chave: &str, m: &TabelaMemoria) -> Vec<(&'static str, Json)> {
    let nomes: Vec<Json> = m
        .colunas_mapeadas()
        .iter()
        .map(|i| Json::texto_de(&m.esquema().colunas()[*i].nome))
        .collect();
    vec![
        ("tabela", Json::texto_de(chave)),
        ("linhas", Json::de_u64(m.vivos())),
        ("slots", Json::de_u64(m.slots())),
        ("bytes", Json::de_u64(m.bytes() as u64)),
        ("mapas", Json::Lista(nomes)),
    ]
}
