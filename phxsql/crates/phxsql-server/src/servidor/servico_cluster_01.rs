//! O cluster: pulso, arbitro, promocao e as operacoes do cluster.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// O nome de host desta entrada da lista resolve para este IP?
///
/// Vai a rede (DNS), e por isso e a ULTIMA pergunta de
/// `sem_propagar_so_de_dentro`, nunca a primeira. Nome que nao resolve nao
/// autoriza -- e um IP literal na lista nem chega aqui.
fn endereco_resolve_para(endereco: &str, porta: u16, ip: &str) -> bool {
    use std::net::{IpAddr, ToSocketAddrs};
    let Ok(alvo) = ip.trim().parse::<IpAddr>() else {
        return false;
    };
    if endereco.trim().parse::<IpAddr>().is_ok() {
        return false;
    }
    (endereco.trim(), porta)
        .to_socket_addrs()
        .map(|mut enderecos| enderecos.any(|e| e.ip().to_canonical() == alvo.to_canonical()))
        .unwrap_or(false)
}

impl Servidor {
    // -------------------------------------------------------------- cluster

    /// Sobe as tres pecas do cluster: o pulso (uma thread por par), o arbitro
    /// e o laco que puxa do master corrente. Sem o bloco `cluster` no
    /// `config.json`, nenhuma delas existe.
    pub(super) fn subir_cluster(self: &Arc<Self>) {
        let Some(estado) = &self.cluster else { return };
        let c = &estado.config;
        eprintln!(
            "cluster: {} nos | este e {} ({}, epoca {}) | janela {}s | pulso {}s | {}",
            estado.total(),
            c.id,
            estado.papel().nome(),
            estado.epoca(),
            c.janela_s,
            c.pulso_s,
            if c.email.ligado {
                format!(
                    "avisa {} a cada {:.1} min enquanto degradado",
                    c.email.para.join(", "),
                    c.avisar_cada_min
                )
            } else {
                "sem e-mail (aviso so no log)".to_string()
            }
        );
        if estado.total() == 2 {
            // Nao e recusa: dois nos replicam e redirecionam normalmente. So a
            // PROMOCAO automatica nunca acontece, e melhor dizer no arranque
            // do que deixar descobrir na primeira queda.
            eprintln!(
                "ATENCAO: cluster de DOIS nos nunca se promove sozinho -- com o \
                 master caido o que sobra ve 1 de 2, e metade nao e maioria. \
                 Promocao automatica pede tres ou mais nos."
            );
        }
        // O estado da cifra do cluster e dito no arranque, com a mesma
        // franqueza do resto: cifrado sem pino protege so da escuta passiva, e
        // esconder isso seria vender protecao que nao existe contra quem esta
        // no meio. So os OUTROS nos entram na conta -- pulsar a si mesmo nao
        // acontece, entao o proprio pino nao muda nada.
        if c.cifra {
            let outros = estado.outros();
            let sem_pino: Vec<&str> = outros
                .iter()
                .filter(|n| n.chave_do_fio.is_empty())
                .map(|n| n.id.as_str())
                .collect();
            if sem_pino.is_empty() {
                eprintln!(
                    "cluster: trafego CIFRADO (pulso e replicacao), com pino em \
                     todos os nos"
                );
            } else {
                eprintln!(
                    "cluster: trafego cifrado, mas SEM pino em {} -- esses nos \
                     ficam protegidos so da escuta passiva, nao de quem esta no \
                     meio. Ponha `chave_do_fio` de cada um (phxsqld \
                     --chave-do-fio no no) para fechar o buraco.",
                    sem_pino.join(", ")
                );
            }
        }
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "pulso-supervisor",
            "mantem UMA thread de pulso por no do cluster, acompanhando a \
             lista VIVA: e o que faz um no acrescentado a quente comecar a ser \
             pulsado sem ninguem reiniciar nada",
            "servico",
            crate::agora_ms(),
            move |fio| {
                fio.fazendo("cuidando das threads de pulso");
                servidor.laco_do_supervisor_do_pulso();
            },
        );
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "arbitro-cluster",
            "decide quem e o master: conta os pulsos, apura a maioria e promove \
             quando o master de antes para de responder",
            "servico",
            crate::agora_ms(),
            move |fio| {
                fio.fazendo("apurando a maioria");
                servidor.laco_do_arbitro();
            },
        );
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "replica-cluster",
            "puxa do master CORRENTE do cluster, que muda a cada promocao -- e \
             por isso ela nao pode ser uma origem fixa do config.json",
            "servico",
            crate::agora_ms(),
            move |fio| {
                fio.fazendo("seguindo o master corrente");
                servidor.laco_da_replica_do_cluster();
            },
        );
    }

    /// Mantem uma thread de pulso por no da lista VIVA -- pedido 217.
    ///
    /// # Por que um supervisor, e nao um `for` no arranque
    ///
    /// O `for` de antes subia uma thread por no do `config.json` e nunca mais
    /// olhava: acrescentar um no ao cluster vivo nao subia pulso nenhum para
    /// ele, e o unico jeito de o antigo falar com o novo era reiniciar. Este
    /// laco fecha o outro lado do 217 -- a lista viva aceita o pulso do no
    /// novo, e o supervisor faz o pulso EXISTIR.
    ///
    /// Meio segundo de intervalo: o mesmo do arbitro, e um no acrescentado
    /// comeca a ser pulsado dentro dele.
    pub(super) fn laco_do_supervisor_do_pulso(self: Arc<Self>) {
        let Some(estado) = self.cluster.clone() else {
            return;
        };
        loop {
            for no in estado.outros() {
                // Quem MARCA sobe. A propria thread desmarca ao morrer, e por
                // isso nao ha um segundo registro aqui para envelhecer. «Ao
                // morrer» e o `Drop` da guarda, que nasce AQUI e viaja para
                // dentro da thread: morte normal, panico, ou a thread que nem
                // chegou a nascer -- pedido 452. Antes era uma linha antes do
                // `return`, e o panico a pulava.
                if !estado.marcar_pulso(&no.id) {
                    continue;
                }
                let guarda = estado.guarda_do_pulso(&no.id);
                let servidor = Arc::clone(&self);
                self.telemetria.subir(
                    format!("pulso-{}", no.id),
                    "manda o pulso para UM no do cluster e escuta o dele: e por \
                     este batimento que a queda do master e descoberta",
                    "servico",
                    crate::agora_ms(),
                    move |fio| {
                        fio.fazendo("pulsando");
                        servidor.laco_do_pulso(no, guarda);
                    },
                );
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    /// Pulsa UM outro no, para sempre: conecta, autentica como a replicacao,
    /// e troca `cluster_pulso` a cada intervalo. Cada lado da troca aprende o
    /// estado do outro -- o pedido leva o meu, a resposta traz o dele.
    ///
    /// Erro aqui e rotina, nao noticia: no caido e exatamente o que o mapa
    /// registra envelhecendo, e quem fala sobre isso e o arbitro, uma vez --
    /// nao esta thread, a cada pulso perdido.
    ///
    /// A `_guarda` so existe para morrer junto: e o `Drop` dela que desmarca o
    /// no, em qualquer saida desta funcao (pedido 452).
    fn laco_do_pulso(
        self: Arc<Self>,
        no: crate::config::NoCluster,
        _guarda: crate::cluster::GuardaDoPulso,
    ) {
        let Some(estado) = self.cluster.clone() else {
            return;
        };
        let intervalo = Duration::from_secs(estado.config.pulso_s);
        loop {
            #[cfg(test)]
            self.panico_no_pulso_de_teste();
            // O no saiu da lista viva, ou mudou de endereco? A thread morre e
            // o supervisor sobe outra com o endereco novo. Continuar seria
            // pulsar um no que o cluster ja nao tem -- ou, pior, o endereco
            // velho de um no que se mudou. Quem desmarca e a guarda.
            if estado.no(&no.id).as_ref() != Some(&no) {
                return;
            }
            let _ = self.pulsar(&estado, &no);
            std::thread::sleep(intervalo);
        }
    }

    /// Uma conexao de pulso: dura ate o primeiro erro, e o laco de fora
    /// reconecta. O prazo de conexao e CURTO de proposito -- um no morto nao
    /// pode segurar a conferencia dos vivos alem do proprio pulso.
    fn pulsar(
        &self,
        estado: &crate::cluster::EstadoCluster,
        no: &crate::config::NoCluster,
    ) -> Result<()> {
        let c = &estado.config;
        let prazo = Duration::from_secs(c.pulso_s.clamp(1, 2));
        // O pedido inteiro cabe num silencio (pedido 580): o no que goteja
        // sai da conta no mesmo prazo do no que calou.
        let mut cliente = crate::replica::Cliente::conectar_com_prazo(
            &no.endereco,
            no.porta,
            &c.token,
            crate::replica::prazo_do_pulso(c.pulso_s),
            prazo,
        )?;
        // O tunel ANTES do login e do primeiro pulso, de proposito: e o token
        // e a prova do desafio-resposta que ele existe para esconder, e o pulso
        // carrega o mapa do cluster (papel, epoca, posicao) que um observador
        // no meio nao precisa ler. O pino e o do no DESTINO -- known_hosts: cada
        // no confere a chave publica de quem ele alcanca. Sem esta linha o pulso
        // sairia em claro mesmo com a cifra do cluster ligada, que e o defeito
        // que a guarda `pulso-do-cluster-em-claro` repoe: cifrar so a
        // replicacao e deixar o pulso em claro e a metade que engana.
        if c.cifra {
            cliente.cifrar(no.pino_do_fio()?)?;
        }
        if !c.usuario.is_empty() {
            cliente.autenticar(&c.usuario, &c.senha_hash, "")?;
        }
        loop {
            // O IRMAO da conferencia do `laco_do_pulso`: este laco de dentro
            // dura enquanto a conexao durar, e sem a mesma pergunta aqui um no
            // removido continuaria sendo pulsado ate a conexao cair sozinha --
            // que pode nao cair nunca.
            if estado.no(&no.id).as_ref() != Some(no) {
                return Ok(());
            }
            let mut campos = vec![
                ("op", Json::texto_de("cluster_pulso")),
                ("id", Json::texto_de(&c.id)),
                ("papel", Json::texto_de(estado.papel().nome())),
                ("epoca", Json::de_u64(estado.epoca())),
                ("posicao", Json::de_u64(estado.posicao())),
                ("incompleta", Json::de_bool(estado.posicao_incompleta())),
                ("prioridade", Json::de_i64(c.prioridade)),
            ];
            // Pedido 294: o vetor por tabela, medida e nao voto -- e fora da
            // prova, pelo motivo escrito em `cluster::campos_por_tabela`.
            campos.extend(crate::cluster::campos_por_tabela(&estado.por_tabela()));
            // A prova de identidade -- pedido 278. So sai quando ha material
            // (a estatica deste no e o `chave_do_fio` do destino); sem pino do
            // outro lado o pulso sai como sempre saiu, e o outro lado nao
            // teria como conferir nada de qualquer forma.
            let transcricao = cliente.transcricao();
            if let Ok(estatica) = self.estatica_do_fio() {
                if let Some(extras) =
                    estado.campos_da_prova(&estatica, no, transcricao.as_ref().map(|t| &t[..]))
                {
                    campos.extend(extras);
                }
            }
            let r = cliente.pedir(campos)?;
            if let Some((id, pulso)) = crate::cluster::PulsoDeNo::de_json(&r) {
                // O IRMAO da conferencia do `op_cluster_pulso`: a RESPOSTA
                // tambem entra no mapa, e um `registrar` sem guarda deste lado
                // deixaria a porta aberta por onde o pedido nao passa mais.
                // Inclusive o crivo da lista: ate o pedido 441 ele morava so no
                // `op_cluster_pulso`, e uma resposta com id fantasma, sem
                // prova, rebaixava o master. Hoje ele e a primeira pergunta do
                // proprio `conferir_identidade`.
                match estado.conferir_identidade(
                    &id,
                    &pulso,
                    &r,
                    transcricao.as_ref().map(|t| &t[..]),
                    || self.estatica_do_fio(),
                ) {
                    Ok(_) => {
                        // Pedido 597: o estado que nao foi ao disco se diz --
                        // aqui nao ha resposta a quem dizer, entao e o log.
                        if let Err(e) = estado.registrar(&id, pulso) {
                            eprintln!(
                                "cluster: a epoca espelhada do pulso de {id:?} NAO foi \
                                 ao disco ({e}); um reinicio volta com a anterior"
                            );
                        }
                    }
                    Err(e) => eprintln!("cluster: resposta do pulso de {id:?} recusada: {e}"),
                }
            }
            std::thread::sleep(Duration::from_secs(c.pulso_s));
        }
    }

    /// O arbitro: a cada meio segundo olha o mapa e decide -- rebaixar,
    /// eleger, liberar ou recusar escrita, avisar. As decisoes moram em
    /// `cluster.rs`; aqui e so o relogio e as consequencias.
    fn laco_do_arbitro(self: Arc<Self>) {
        let Some(estado) = self.cluster.clone() else {
            return;
        };
        let mut ultima_conta = 0i64;
        let mut motivos_anteriores: Vec<String> = Vec::new();
        loop {
            let agora = crate::agora_ms();
            // A posicao local no ritmo do pulso, e nao do tique: ela toma a
            // trava de dados, e o dobro da frequencia nao compraria nada.
            if agora - ultima_conta >= estado.config.pulso_s as i64 * 1_000 {
                self.contar_posicao_do_cluster(&estado);
                ultima_conta = agora;
            }
            let motivos = self.rodada_do_arbitro(&estado, agora);
            // So a MUDANCA vira log: um cluster degradado continua degradado,
            // e repetir a cada tique afogaria a noticia seguinte.
            if motivos != motivos_anteriores {
                for m in &motivos {
                    eprintln!("cluster: {m}");
                }
                if motivos.is_empty() {
                    eprintln!("cluster: normalizado");
                }
                motivos_anteriores = motivos.clone();
            }
            estado.definir_degradacao(motivos);
            self.avisos_do_cluster(&estado, agora);
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    /// Rebaixa este no a replica na `epoca` e, se o papel NAO foi ao disco,
    /// devolve o motivo que o diz -- pedido 599, o mesmo motor do `registrar`
    /// consertado no 597.
    ///
    /// O `rebaixar` para de escrever ANTES de gravar (o lado seguro, que nao
    /// espera o disco), entao a memoria ja esta certa quando a gravacao falha.
    /// O que fica errado e o arranque seguinte: o `cluster.estado.json` ainda
    /// diz master, e o no volta mandando ate o pulso do master corrente o
    /// rebaixar de novo -- com escrita liberada na graca do arranque. Engolir
    /// isso (`let _ =`) era deixar a unica pista sumir; o motivo vai para a
    /// degradacao e, por ela, para o log do laco do arbitro.
    fn rebaixar_dizendo(estado: &crate::cluster::EstadoCluster, epoca: u64) -> Option<String> {
        estado.rebaixar(epoca).err().map(|e| {
            format!(
                "o rebaixamento NAO foi ao disco ({e}): um reinicio volta com o \
                 papel gravado antes, master, ate o pulso do master corrente \
                 rebaixar de novo"
            )
        })
    }

    /// Uma rodada de decisao. Devolve os motivos de degradacao ATUAIS.
    ///
    /// Os dois rebaixamentos daqui (epoca maior no ar, desempate perdido)
    /// passam por [`Self::rebaixar_dizendo`], para a falha de gravar o papel nunca
    /// ser engolida num e dita no outro.
    pub(super) fn rodada_do_arbitro(
        &self,
        estado: &crate::cluster::EstadoCluster,
        agora: i64,
    ) -> Vec<String> {
        use crate::cluster::{Candidato, PapelVivo};
        let c = &estado.config;
        let mapa = estado.mapa();
        let mut motivos = Vec::new();

        // A graca do arranque: por uma janela, "nunca deu pulso" e "ainda nao
        // deu tempo", nao degradacao -- o primeiro tique roda antes do
        // primeiro pulso, e sem isto todo cluster nasceria doente.
        let em_graca = agora - estado.nascido_ms() <= c.janela_ms();
        let mut vivos_qtd = 1usize; // eu
        for no in estado.outros() {
            match mapa.get(&no.id) {
                Some(p) if agora - p.quando_ms <= c.janela_ms() => vivos_qtd += 1,
                Some(p) => motivos.push(format!(
                    "no {} sem pulso ha {}s",
                    no.id,
                    (agora - p.quando_ms) / 1_000
                )),
                None if em_graca => {}
                None => motivos.push(format!("no {} nunca deu pulso", no.id)),
            }
        }

        match estado.papel() {
            PapelVivo::Master => {
                // Epoca maior no ar = houve eleicao sem mim. O destronado se
                // rebaixa SOZINHO -- e o que resolve "dois masters" quando o
                // antigo volta da particao ou do reinicio.
                let maior = estado.maior_epoca_vista();
                if maior > estado.epoca() {
                    eprintln!(
                        "cluster: ha epoca {maior} no ar e a minha e {} -- houve \
                         eleicao enquanto este no esteve fora; REBAIXANDO a replica",
                        estado.epoca()
                    );
                    motivos.push("este no foi rebaixado: um master de epoca maior assumiu".into());
                    motivos.extend(Self::rebaixar_dizendo(estado, maior));
                    return motivos;
                }
                // Dois masters na MESMA epoca (dois configs com papel source,
                // ou um empate de particao): perde quem a eleicao nao
                // escolheria -- a MESMA conta de `vencedor`, para os dois
                // lados decidirem igual.
                for (id, p) in &mapa {
                    if p.papel == PapelVivo::Master
                        && p.epoca == estado.epoca()
                        && agora - p.quando_ms <= c.janela_ms()
                    {
                        let eu = Candidato {
                            id: c.id.clone(),
                            posicao: estado.posicao(),
                            incompleta: estado.posicao_incompleta(),
                            prioridade: c.prioridade,
                        };
                        let ele = Candidato {
                            id: id.clone(),
                            posicao: p.posicao,
                            incompleta: p.incompleta,
                            prioridade: p.prioridade,
                        };
                        let vence = crate::cluster::vencedor(&[eu, ele], 1).map(|v| v.id.clone());
                        if vence.as_deref() != Some(c.id.as_str()) {
                            eprintln!(
                                "cluster: {id} tambem e master na epoca {} e ganha \
                                 o desempate; REBAIXANDO este no a replica",
                                estado.epoca()
                            );
                            motivos.push(format!("este no perdeu o desempate para {id}"));
                            motivos.extend(Self::rebaixar_dizendo(estado, estado.epoca()));
                            return motivos;
                        }
                    }
                }
                // Master isolado nao escreve: e o que limita o split-brain ao
                // tempo de DETECCAO -- o que entrou antes disso e a cauda que
                // se perde, e docs/CLUSTER.md diz isso sem eufemismo. Na graca
                // do arranque a escrita fica como nasceu (liberada): ainda nao
                // houve tempo de um pulso chegar, e recusar aqui seria recusar
                // todo arranque de master por uma janela.
                let tem_maioria = estado.e_maioria(vivos_qtd);
                if tem_maioria || !em_graca {
                    estado.liberar_escrita(tem_maioria);
                }
                if !tem_maioria && !em_graca {
                    motivos.push(format!(
                        "sem maioria visivel ({vivos_qtd} de {}): escrita recusada",
                        estado.total()
                    ));
                }
            }
            PapelVivo::Replica => {
                // Diario local A FRENTE do master e a cauda de um antigo
                // master: nao ha como replicar para tras. Aviso, nao conserto
                // -- apagar dado sozinho nunca.
                if let Some((id, _)) = estado.master_atual() {
                    if let Some(p) = mapa.get(&id) {
                        if agora - p.quando_ms <= c.janela_ms() && estado.posicao() > p.posicao {
                            motivos.push(format!(
                                "diario local ({}) a frente do master {id} ({}): \
                                 provavel cauda de escritas perdidas -- ressemeie \
                                 este no a partir do master",
                                estado.posicao(),
                                p.posicao
                            ));
                        }
                    }
                }
                let silencio = agora - estado.master_visto_ms();
                if silencio > c.janela_ms() {
                    let vivos = estado.vivos(agora);
                    // Pedido 313: a referencia do teto de atraso e a ultima
                    // posicao que o master publicou -- a que os clientes ja
                    // ouviram «gravei» -- ou a do vivo mais a frente, se o
                    // master nunca pulsou para ca.
                    let do_master = estado
                        .master_atual()
                        .and_then(|(id, _)| mapa.get(&id).map(|p| p.posicao))
                        .unwrap_or(0);
                    let referencia = vivos
                        .iter()
                        .map(|v| v.posicao)
                        .max()
                        .unwrap_or(0)
                        .max(do_master);
                    let eleicao = crate::cluster::eleger(
                        &vivos,
                        estado.total(),
                        referencia,
                        c.atraso_maximo_na_eleicao,
                    );
                    let eleito = match eleicao {
                        crate::cluster::Eleicao::TodosAtrasados {
                            referencia,
                            menor_atraso,
                        } => {
                            motivos.push(format!(
                                "master calado ha {}s e todos os {} vivos atras da ultima \
                                 posicao dele ({referencia}) mais do que \
                                 cluster.atraso_maximo_na_eleicao ({}): o menos atrasado \
                                 esta {menor_atraso} atras -- NAO promovo",
                                silencio / 1_000,
                                vivos.len(),
                                c.atraso_maximo_na_eleicao
                            ));
                            return motivos;
                        }
                        crate::cluster::Eleicao::SemMaioria => None,
                        crate::cluster::Eleicao::Eleito(v) => Some(v),
                    };
                    match eleito {
                        // O teste de protecao mais importante da bateria: sem
                        // maioria visivel, ficar degradado E a decisao certa.
                        None => motivos.push(format!(
                            "master calado ha {}s e sem maioria visivel ({} de {}): \
                             NAO promovo",
                            silencio / 1_000,
                            vivos.len(),
                            estado.total()
                        )),
                        Some(v) if v.id == c.id => {
                            let motivo = format!(
                                "master calado ha {}s; eleito entre {} vivos de {} \
                                 configurados",
                                silencio / 1_000,
                                vivos.len(),
                                estado.total()
                            );
                            if let Err(e) = self.promover_a_master(&motivo) {
                                motivos.push(format!("promocao falhou: {e}"));
                            }
                        }
                        Some(v) => motivos.push(format!(
                            "master calado ha {}s; aguardando {} assumir",
                            silencio / 1_000,
                            v.id
                        )),
                    }
                }
            }
        }
        motivos
    }

    /// PROMOVE este no a master do cluster: epoca nova (a maior vista + 1),
    /// papel persistido, escrita liberada, aviso agendado.
    ///
    /// E o UNICO caminho de promocao. A eleicao automatica chama daqui, e
    /// qualquer promocao MANUAL que venha a existir deve cair aqui tambem --
    /// dois caminhos de promover e a porta dos fundos classica: o que alguem
    /// esquecer de atualizar vira o furo.
    pub fn promover_a_master(&self, motivo: &str) -> Result<Json> {
        let Some(estado) = &self.cluster else {
            return Err(Self::sem_cluster());
        };
        let epoca = estado.promover(estado.maior_epoca_vista() + 1)?;
        eprintln!("cluster: PROMOVIDO a master na epoca {epoca} -- {motivo}");
        estado.anotar_promocao(format!(
            "O no {} assumiu como master do cluster, na epoca {epoca}.\n\
             Motivo: {motivo}\n\
             As replicas passam a segui-lo sozinhas; clientes que escreverem \
             num outro no recebem REDIRECIONA {}.",
            estado.config.id,
            estado
                .no(&estado.config.id)
                .map(|n| n.alvo())
                .unwrap_or_default()
        ));
        // O log de acessos guarda tambem o que o servidor decide sozinho --
        // sem isto, a unica prova da promocao seria o comportamento mudar.
        self.anotar(&Acesso {
            quando_ms: crate::agora_ms(),
            ip: "(local)".into(),
            porta_origem: 0,
            op: "cluster_promocao".into(),
            usuario: estado.config.id.clone(),
            autenticado: true,
            ok: true,
            duracao_ms: 0,
            erro: None,
            database: String::new(),
            tabela: String::new(),
            codigo: 0,
        });
        Ok(Json::objeto(vec![
            ("papel", Json::texto_de("master")),
            ("epoca", Json::de_u64(epoca)),
        ]))
    }

    /// Os e-mails do cluster: a promocao avisa UMA vez; a degradacao repete a
    /// cada `avisar_cada_min` enquanto durar. Sem e-mail configurado, nada.
    fn avisos_do_cluster(&self, estado: &crate::cluster::EstadoCluster, agora: i64) {
        if !estado.config.email.ligado {
            return;
        }
        if let Some(texto) = estado.tomar_aviso_de_promocao() {
            match crate::email::enviar(
                &estado.config.email,
                "PhxSql cluster: promocao de master",
                &texto,
            ) {
                Ok(r) => eprintln!("cluster: e-mail de promocao enviado: {r}"),
                Err(e) => eprintln!("cluster: e-mail de promocao NAO ENVIADO: {e}"),
            }
        }
        let motivos = estado.degradacao();
        if motivos.is_empty() || !estado.hora_de_avisar(agora) {
            return;
        }
        let corpo = format!(
            "O cluster PhxSql esta degradado. O no {} ve:\n\n{}\n\n\
             Este aviso repete a cada {:.1} min enquanto durar.\n\
             Servidor PhxSql {VERSAO}\nQuando: {}\n",
            estado.config.id,
            motivos
                .iter()
                .map(|m| format!("  - {m}"))
                .collect::<Vec<_>>()
                .join("\n"),
            estado.config.avisar_cada_min,
            phxsql_core::datahora::instante_iso(agora)
        );
        let assunto = format!(
            "PhxSql cluster degradado ({} motivo{})",
            motivos.len(),
            if motivos.len() == 1 { "" } else { "s" }
        );
        match crate::email::enviar(&estado.config.email, &assunto, &corpo) {
            Ok(r) => eprintln!("cluster: e-mail de degradacao enviado: {r}"),
            Err(e) => eprintln!("cluster: e-mail de degradacao NAO ENVIADO: {e}"),
        }
    }

    /// A soma dos eventos das tabelas replicadas -- a posicao que o pulso
    /// carrega e que a eleicao compara. Toma a trava de dados; por isso quem
    /// chama e o arbitro, no ritmo do pulso, e o resultado fica em cache.
    ///
    /// # Devolve a soma E se ela esta INCOMPLETA (pedido 211)
    ///
    /// Uma tabela que nao abre, ou que abre e nao conta, some da soma -- e uma
    /// posicao menor que a real faz o no se declarar mais atrasado do que e,
    /// perdendo uma eleicao que deveria vencer. Por decisao do dono (10/09/2026,
    /// saida (b)), a posicao NAO e recusada: ela sai marcada `incompleta`, e a
    /// eleicao (`cluster::vencedor`) prefere quem esta completo. Aqui a funcao
    /// so RELATA a falha; quem a publica e o pulso.
    ///
    /// E `true` sempre que faltou contar algo do que se devia: a trava nao
    /// veio, um database nao abriu, a lista de tabelas de um database falhou,
    /// uma tabela nao abriu, ou a contagem dela deu erro. Uma so basta.
    /// Conta a posicao do diario e a entrega ao estado do cluster -- a soma,
    /// a bandeira e o vetor por tabela numa chamada so. E o que o arbitro faz
    /// no ritmo do pulso; separado para a prova do pedido 294 contar sem
    /// subir a thread.
    pub(super) fn contar_posicao_do_cluster(&self, estado: &crate::cluster::EstadoCluster) {
        // Pedido 300: o master conta tudo o que tem, porque e o que ele serve;
        // a replica conta so o que o master anunciou. A replica que nunca
        // ouviu um master conta tudo E diz que pode estar errada -- nao saber
        // nao vira «sei que e tudo».
        //
        // E o master conta so o que a replica ALCANCA: a tabela que o usuario
        // do cluster nao pode `replicar` mora so nele, e nenhuma replica a
        // tera nunca -- somada, ela pos o master a frente de toda replica por
        // um dado que nao viaja. O filtro e o MESMO motor que o `posicao`
        // aplica a sessao da replica ([`replica_alcanca`]), e nao uma segunda
        // opiniao sobre o que replica. A ficha e copiada ANTES da trava de
        // dados: o cadastro tem trava propria, e as duas nunca se aninham aqui.
        let anunciadas = estado.anunciadas();
        let usuario = (!estado.config.usuario.is_empty())
            .then(|| self.cadastro().por_login(&estado.config.usuario).cloned())
            .flatten();
        let escopo = match (estado.papel(), &anunciadas) {
            (crate::cluster::PapelVivo::Master, _) => {
                EscopoDaPosicao::DoQueSeServe(usuario.as_ref())
            }
            (_, Some(a)) => EscopoDaPosicao::DoMaster(a),
            (_, None) => EscopoDaPosicao::Desconhecido,
        };
        let (p, incompleta, por_tabela) =
            self.posicao_do_diario_em(&estado.config.databases, escopo);
        estado.definir_posicao(p, incompleta, por_tabela);
    }

    /// A posicao contando tudo, sem usuario no filtro. So os
    /// testes a chamam sem escopo; o arbitro passa sempre pelo
    /// [`Self::contar_posicao_do_cluster`].
    #[cfg(test)]
    pub(super) fn posicao_do_diario(&self, so_estes: &[String]) -> (u64, bool, Vec<(String, u64)>) {
        self.posicao_do_diario_em(so_estes, EscopoDaPosicao::DoQueSeServe(None))
    }

    /// A soma dos eventos das tabelas replicadas -- a posicao que o pulso
    /// carrega e que a eleicao compara --, a bandeira de incompleta (pedido
    /// 211) e, da MESMA passada, a contagem de cada tabela (pedido 294).
    ///
    /// O vetor e medida para o painel, nunca criterio: a decisao do dono de
    /// 17/09/2026 mantem a soma no `vencedor`, porque comparar vetores exige
    /// uma ordem total que dois nos podem enxergar diferente. Sair da mesma
    /// passada e o que impede a soma publicada e o vetor publicado de
    /// contarem coisas diferentes.
    fn posicao_do_diario_em(
        &self,
        so_estes: &[String],
        escopo: EscopoDaPosicao<'_>,
    ) -> (u64, bool, Vec<(String, u64)>) {
        let Ok(trava) = self.travar_dados() else {
            // Sem a trava nao ha o que somar: posicao zero, e incompleta,
            // porque nao se contou nada do que se devia contar.
            return (0, true, Vec::new());
        };
        let bases = if so_estes.is_empty() {
            trava.databases().unwrap_or_default()
        } else {
            so_estes.to_vec()
        };
        let mut total = 0u64;
        let mut incompleta = matches!(escopo, EscopoDaPosicao::Desconhecido);
        let mut por_tabela = Vec::new();
        for b in bases {
            // O alcance deste database: `None` = toda tabela dele.
            let alcance = match escopo {
                EscopoDaPosicao::Desconhecido | EscopoDaPosicao::DoQueSeServe(_) => None,
                EscopoDaPosicao::DoMaster(a) => {
                    if !a.databases.contains(&b) {
                        // O master nao anuncia este database: ele so mora
                        // aqui, e nao e posicao de nada que o cluster replica.
                        continue;
                    }
                    match a.tabelas.get(&b) {
                        Some(ts) => Some(ts),
                        None => {
                            // Anunciado e ainda nao perguntado: conta o que ha
                            // e diz que nao sabe se tudo e replicado.
                            incompleta = true;
                            None
                        }
                    }
                }
            };
            let Ok(db) = trava.abrir_database(&b) else {
                incompleta = true;
                continue;
            };
            let Ok(tabelas) = db.todas_as_tabelas() else {
                incompleta = true;
                continue;
            };
            for t in tabelas {
                if alcance.is_some_and(|ts| !ts.contains(&t)) {
                    continue;
                }
                if let EscopoDaPosicao::DoQueSeServe(u) = escopo {
                    if !replica_alcanca(u, &b, &t) {
                        continue;
                    }
                }
                match db.abrir_qualificada(&t) {
                    Ok(mut tab) => match tab.eventos() {
                        Ok(n) => {
                            total += n;
                            por_tabela.push((format!("{b}/{t}"), n));
                        }
                        Err(_) => incompleta = true,
                    },
                    Err(_) => incompleta = true,
                }
            }
        }
        (total, incompleta, por_tabela)
    }

    /// O laco que puxa do master CORRENTE -- e a unica diferenca para o laco
    /// da replicacao comum: a origem sai do mapa do cluster a cada rodada, em
    /// vez de sair fixa do `config.json`. Promovido, ele para de puxar
    /// sozinho, porque o papel vivo e conferido a cada volta.
    pub(super) fn laco_da_replica_do_cluster(self: Arc<Self>) {
        let Some(estado) = self.cluster.clone() else {
            return;
        };
        let espera = Duration::from_secs(estado.config.pulso_s);
        // O master que recusou a credencial do cluster, enquanto for ele.
        let mut recusado_por: Option<String> = None;
        let mut ritmo = crate::replica::Ritmo::novo(espera);
        // De qual master e o recuo corrente (ver o `ritmo.sucesso` abaixo).
        let mut recuo_de: Option<String> = None;
        // O canal aberto do quorum (pedido 207) e as confirmacoes que ainda
        // nao chegaram ao master -- vivem entre as voltas, porque um pull no
        // meio nao pode perder o que ja foi aplicado e gravado.
        let mut canal_do_quorum: Option<(String, crate::replica::Cliente)> = None;
        let mut confirmar: Vec<crate::quorum::Exigencia> = Vec::new();
        loop {
            let c = &estado.config;
            if estado.papel() == crate::cluster::PapelVivo::Master {
                std::thread::sleep(espera);
                continue;
            }
            let alvo = estado
                .master_atual()
                .filter(|(id, _)| id != &c.id)
                .and_then(|(id, _)| estado.no(&id));
            let Some(no) = alvo else {
                std::thread::sleep(espera);
                continue;
            };
            let origem = origem_do_master(c, &no);
            // O IRMAO do laco da replica, e a mesma licao do pedido 203: a
            // credencial que o master corrente recusou nao muda por insistir,
            // e cada insistencia conta contra este IP no bloqueio de la. Aqui
            // nao se estaciona para sempre, porque o master MUDA -- a eleicao
            // pode entregar outro, e esse pode aceitar. Entao a recusa fica
            // anotada por master: enquanto o corrente for o que recusou, nada
            // de tentar; mudou o master, ou alguem religou (`replicacao_ligar`
            // com a origem `cluster:<id>`), tenta uma vez.
            if recusado_por.as_deref() == Some(no.id.as_str()) && !self.tomar_religar(&origem.nome)
            {
                std::thread::sleep(espera);
                continue;
            }
            // O recuo e do PAR: master novo e conversa nova, e comeca na base.
            // Sem isto o no que sofreu com o master caido esperaria o recuo
            // herdado para falar com o eleito, que esta de pe.
            if recuo_de.as_deref() != Some(no.id.as_str()) {
                ritmo.sucesso();
                recuo_de = Some(no.id.clone());
            }
            let retomando = recusado_por.take().is_some();
            match self.rodada_classificada(&origem) {
                Ok(n) => {
                    ritmo.sucesso();
                    if retomando {
                        self.anotar_estado(&origem.nome, |e| {
                            e.parada.clear();
                            e.ultimo_erro.clear();
                        });
                    }
                    if n == 0 {
                        // Nada novo. Com o quorum ligado, a espera vira o
                        // canal aberto (pedido 207): a replica fica numa
                        // conversa com o master e so volta ao pull quando
                        // ficou para tras. Sem ele, o sono de sempre.
                        if self.quorum.as_ref().is_some_and(|q| q.minimo() > 0) {
                            self.esperar_pelo_master(
                                &estado,
                                &origem,
                                &no.id,
                                espera,
                                &mut canal_do_quorum,
                                &mut confirmar,
                            );
                        } else {
                            std::thread::sleep(espera);
                        }
                    } else {
                        eprintln!("cluster: {n} evento(s) aplicado(s) do master {}", no.id);
                    }
                }
                Err((crate::replica::Falha::CredencialRecusada, e)) => {
                    eprintln!(
                        "cluster: o master {} recusou a credencial ({e}); parei de tentar \
                         nele -- corrija cluster.usuario/senha_hash e religue \
                         (replicacao_ligar, origem {:?}), ou espere outro master",
                        no.id, origem.nome
                    );
                    let texto = e.to_string();
                    self.anotar_estado(&origem.nome, |est| {
                        est.parada = "credencial_recusada".to_string();
                        est.ultimo_erro = texto;
                    });
                    recusado_por = Some(no.id.clone());
                }
                // Pedido 587: a MESMA decisao do laco comum (`replica::Ritmo`,
                // pedido 585) -- rede e limite recuam base × 2^n ate 60 s, o
                // resto espera o pulso de sempre. Antes era `sleep(espera)` para
                // tudo, e o par que goteja prendia a thread um total inteiro a
                // cada pulso. O sono acorda se o master mudar ou este no for
                // promovido: o recuo nao atrasa seguir o eleito.
                Err((falha, e)) => {
                    eprintln!("cluster: replicacao do master {}: {e}", no.id);
                    let master = no.id.clone();
                    self.apos_a_falha_vigiando(&origem.nome, &mut ritmo, falha, &|| {
                        estado.papel() != crate::cluster::PapelVivo::Master
                            && estado.master_atual().map(|(id, _)| id) == Some(master.clone())
                    });
                }
            }
        }
    }

    pub(super) fn sem_cluster() -> PhxError {
        PhxError::Esquema(
            "este servidor nao esta em cluster: nao ha o bloco \"cluster\" no \
             config.json"
                .into(),
        )
    }

    /// `cluster_pulso`: registra o pulso de OUTRO no e devolve o proprio --
    /// uma troca, e cada lado sai sabendo do outro.
    pub(super) fn op_cluster_pulso(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let Some(estado) = &self.cluster else {
            return Err(Self::sem_cluster());
        };
        let Some((id, pulso)) = crate::cluster::PulsoDeNo::de_json(p) else {
            return Err(PhxError::Esquema(
                "informe \"id\" e \"papel\" (master ou replica)".into(),
            ));
        };
        // Quem MANDOU este pulso e mesmo o no que ele diz ser, e e um dos
        // OUTROS nos desta lista? Pedidos 278 e 441. A conferencia vem antes
        // do `registrar` porque e o `registrar` que move a epoca, renova "vi o
        // master agora" e decide a eleicao.
        //
        // O crivo da lista e do «e ESTE servidor» mora no MOTOR desde o 441:
        // aqui ele so existia neste chamador, e o irmao (a RESPOSTA, no laco
        // do pulso) nao o herdou. O que ficou aqui e a APRESENTACAO da recusa,
        // que o motor nao conhece -- UMA frase para os dois casos, pela
        // fabrica de idiomas (revisao SEC de 17/09/2026, A11: duas frases
        // faziam do pulso um oraculo de ids); o id duplicado dito no log
        // DESTE processo, com o IP, e nao no fio; e a tentativa leve so com
        // `seguranca.contar_pulso_desconhecido` ligado, porque o no que entra
        // a quente pulsa os antigos antes de ser acrescentado -- ver o
        // comentario do campo em `blacklist::Politica`.
        let identidade = match estado.conferir_identidade(
            &id,
            &pulso,
            p,
            sessao.transcricao_do_fio.as_ref().map(|t| &t[..]),
            || self.estatica_do_fio(),
        ) {
            Ok(identidade) => identidade,
            Err(crate::cluster::RecusaDoPulso::ForaDaLista { e_este_no }) => {
                if e_este_no {
                    // Pelo silencio do motor, e nao um `eprintln!` por pulso: e
                    // quem MANDA que escolhe quando esta linha sai, e o stderr
                    // e o journal (pedido 445, SEC B2 -- o irmao das recusas
                    // do `cluster.rs`). O id aqui e o DESTE no: chave unica.
                    estado.diagnosticar_contido("este_no", &id, || {
                        format!(
                            "cluster: pulso com o id DESTE servidor ({id}) vindo de {:?} -- \
                             dois nos com o mesmo id no ar?",
                            sessao.ip
                        )
                    });
                }
                if self.config.politica.contar_pulso_desconhecido && !sessao.ip.is_empty() {
                    self.violacao_leve(
                        &sessao.ip,
                        "cluster_pulso",
                        "pulso com id que nao e um no deste cluster",
                    );
                }
                return Err(PhxError::Autorizacao(
                    self.msg("erro.pulso_de_no_desconhecido", &[("id", &id)]),
                ));
            }
            Err(crate::cluster::RecusaDoPulso::Outra(e)) => return Err(e),
        };
        // Pedido 597: era um `let _ =` dentro do `registrar`, e o pulso
        // respondia como se a epoca espelhada estivesse no disco.
        let nao_gravou = estado.registrar(&id, pulso).err();
        let mut resposta = vec![
            ("id", Json::texto_de(&estado.config.id)),
            ("papel", Json::texto_de(estado.papel().nome())),
            ("epoca", Json::de_u64(estado.epoca())),
            ("posicao", Json::de_u64(estado.posicao())),
            ("incompleta", Json::de_bool(estado.posicao_incompleta())),
            ("prioridade", Json::de_i64(estado.config.prioridade)),
        ];
        // O IRMAO do pedido: a resposta leva o vetor deste no, senao so um
        // lado do par enxergaria a assimetria (pedido 294).
        resposta.extend(crate::cluster::campos_por_tabela(&estado.por_tabela()));
        // A resposta tambem prova quem a manda, pelo mesmo motivo: ela entra
        // no mapa do outro lado exatamente como o pedido entra neste. Mas SO
        // quando o pedido provou (435 reaberto, SEC A2): assinar a resposta a
        // um pulso sem prova publicava «o remetente alegado tem pino aqui» --
        // o porque, e por que isso nao tira nada de par legitimo nenhum, esta
        // em `EstadoCluster::campos_da_resposta`.
        if let (Ok(estatica), Some(no)) = (self.estatica_do_fio(), estado.no(&id)) {
            if let Some(extras) = estado.campos_da_resposta(
                identidade,
                &estatica,
                &no,
                sessao.transcricao_do_fio.as_ref().map(|t| &t[..]),
            ) {
                resposta.extend(extras);
            }
        }
        // Fora da prova, de proposito: o aviso e sobre o DISCO deste no, e
        // nao muda nada do que o pulso afirma sobre papel e epoca.
        if let Some(e) = nao_gravou {
            eprintln!("cluster: a epoca espelhada do pulso de {id:?} NAO foi ao disco ({e})");
            resposta.push((
                "aviso",
                Json::texto_de(format!(
                    "a epoca espelhada NAO foi ao disco ({e}); um reinicio deste no \
                     volta com a anterior ate o proximo pulso do master"
                )),
            ));
        }
        Ok(Json::objeto(resposta))
    }

    /// `cluster_no_acrescentar`: escalonar o cluster A QUENTE -- pedido 217.
    ///
    /// # O que estava quebrado, e o que o conserto mudou
    ///
    /// A `bancada/cluster/escalonar.py` mediu: um no novo com a lista de
    /// quatro na propria configuracao ficava isolado a janela inteira, porque
    /// o pulso de um id fora da lista dos ANTIGOS era recusado na hora
    /// (`ACESSO_NEGADO`) e nenhum antigo tinha por que falar com ele. O unico
    /// caminho que funcionava era editar `cluster.nos` dos tres antigos e
    /// reinicia-los -- **0,367 s de master fora do ar**, sem eleicao.
    ///
    /// A recusa do pulso continua, e continuar e a decisao certa: aceitar um
    /// id desconhecido deixaria qualquer credencial de replicacao inflar o
    /// denominador da maioria com nos fantasmas e travar toda promocao. O que
    /// muda e a LISTA -- ela deixou de ser o retrato do arranque.
    ///
    /// # Tres coisas, nesta ordem, e por que a ordem importa
    ///
    /// 1. **a lista viva**, para o pulso do no novo ser aceito AGORA;
    /// 2. **o `config.json`**, para o no acrescentado nao sumir calado no
    ///    proximo arranque -- perder um no assim e pior que nao acrescentar,
    ///    porque o cluster segue com um denominador menor do que o operador
    ///    acredita;
    /// 3. **a propagacao**, uma vez por no, com a mesma credencial do pulso.
    ///
    /// Gravar antes de aplicar deixaria o arquivo prometendo o que a memoria
    /// ainda nao faz; propagar antes de aplicar mandaria os outros aceitarem
    /// um no que este aqui ainda recusa.
    ///
    /// # A propagacao FALA quando falha
    ///
    /// A resposta traz um veredito por no. No que nao aceitou aparece com o
    /// erro, e nao sumindo da lista: metade do cluster escalonada em silencio
    /// e exatamente o estado em que uma eleicao conta votos diferentes em cada
    /// lado. A tela mostra esse veredito.
    ///
    /// Consequencia aceita e escrita: **a propagacao autentica com
    /// `cluster.usuario`**, entao esse usuario precisa poder `administrar`.
    /// Sem isso a ordem local vale e a propagacao volta recusada, nomeando o
    /// no -- que e melhor que dar o poder de mexer na maioria a quem so tem
    /// credencial de replica.
    pub(super) fn op_cluster_no_acrescentar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let estado = self.cluster_para_escalonar(sessao)?;
        self.sem_propagar_so_de_dentro(&estado, p, sessao)?;
        let id = p.texto_ou("id", "").trim().to_string();
        if id.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"id\": o nome pelo qual os outros nos vao chamar este".into(),
            ));
        }
        let endereco = p.texto_ou("endereco", "").trim().to_string();
        if endereco.is_empty() {
            return Err(PhxError::Esquema(format!(
                "informe \"endereco\": por onde os outros nos alcancam {id:?}"
            )));
        }
        // O pino do no novo entra junto -- senao um no acrescentado a quente
        // num cluster cifrado seria pulsado e replicado SEM ancora (so escuta
        // passiva), enquanto os do arquivo tem pino: metade do cluster com
        // ancora e metade sem. Vazio = sem pino, como o no do arquivo sem
        // `chave_do_fio`. O `Config::validar` da gravacao recusa pino torto.
        let no = crate::config::NoCluster {
            id: id.clone(),
            endereco,
            porta: p
                .inteiro_ou("porta", crate::config::PORTA_PADRAO as i64)
                .clamp(1, 65_535) as u16,
            chave_do_fio: p.texto_ou("chave_do_fio", "").trim().to_string(),
        };
        let mudou = estado.acrescentar(no.clone());
        match self.gravar_a_lista_do_cluster(&estado) {
            Ok(()) => {}
            Err(e) => {
                // Desfaz o que acabou de entrar: memoria que diverge do
                // arquivo e a mentira que o proximo arranque conta.
                if mudou {
                    estado.remover(&no.id);
                }
                return Err(e);
            }
        }
        if mudou {
            eprintln!(
                "cluster: no {} ({}) ACRESCENTADO a quente -- a maioria passa a ser \
                 contada sobre {} nos",
                no.id,
                no.alvo(),
                estado.total()
            );
        }
        Ok(Json::objeto(vec![
            ("acrescentado", Json::Bool(mudou)),
            ("id", Json::texto_de(&no.id)),
            ("nos", Json::de_u64(estado.total() as u64)),
            (
                "propagado",
                self.propagar_no_cluster(
                    &estado,
                    p,
                    vec![
                        ("op", Json::texto_de("cluster_no_acrescentar")),
                        ("id", Json::texto_de(&no.id)),
                        ("endereco", Json::texto_de(&no.endereco)),
                        ("porta", Json::de_u64(no.porta as u64)),
                        // O pino viaja para os outros nos aprenderem a ancora
                        // do no novo -- so o pino (chave publica), nunca a
                        // privada de ninguem. Quando o cluster esta cifrado, a
                        // propria propagacao ja vai por dentro do tunel.
                        ("chave_do_fio", Json::texto_de(&no.chave_do_fio)),
                        ("propagar", Json::Bool(false)),
                    ],
                ),
            ),
        ]))
    }

    /// `cluster_no_remover`: tira um no do cluster vivo -- o outro lado do 217.
    ///
    /// Duas recusas, e as duas sao decisao:
    ///
    /// - **este no nao se remove**: um servidor fora da propria lista nao
    ///   passa mais no `Cluster::validar` e nao subiria de novo;
    /// - **o MASTER nao se remove**: tirar da lista quem esta escrevendo
    ///   deixaria o cluster sem para onde redirecionar, e o master continuaria
    ///   aceitando escrita que ninguem mais conta. Rebaixe primeiro.
    pub(super) fn op_cluster_no_remover(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let estado = self.cluster_para_escalonar(sessao)?;
        self.sem_propagar_so_de_dentro(&estado, p, sessao)?;
        let id = p.texto_ou("id", "").trim().to_string();
        if id.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"id\": o no a tirar da lista".into(),
            ));
        }
        if id == estado.config.id {
            return Err(PhxError::Esquema(format!(
                "{id:?} e ESTE servidor: um no fora da propria lista nao volta a \
                 subir. Remova-o a partir de outro no do cluster."
            )));
        }
        if estado.master_atual().is_some_and(|(m, _)| m == id) {
            return Err(PhxError::Esquema(format!(
                "{id:?} e o MASTER corrente: remove-lo deixaria o cluster sem para \
                 onde redirecionar a escrita. Espere a promocao de outro no."
            )));
        }
        let saiu = estado.remover(&id);
        if let Err(e) = self.gravar_a_lista_do_cluster(&estado) {
            if saiu {
                // Volta como estava: a lista viva nao pode ficar menor que o
                // arquivo, senao a maioria de agora e a do proximo arranque
                // divergem.
                if let Some(no) = self
                    .config
                    .cluster
                    .as_ref()
                    .and_then(|c| c.no(&id).cloned())
                {
                    estado.acrescentar(no);
                }
            }
            return Err(e);
        }
        if saiu {
            eprintln!(
                "cluster: no {id} REMOVIDO a quente -- a maioria passa a ser contada \
                 sobre {} nos",
                estado.total()
            );
        }
        Ok(Json::objeto(vec![
            ("removido", Json::Bool(saiu)),
            ("id", Json::texto_de(&id)),
            ("nos", Json::de_u64(estado.total() as u64)),
            (
                "propagado",
                self.propagar_no_cluster(
                    &estado,
                    p,
                    vec![
                        ("op", Json::texto_de("cluster_no_remover")),
                        ("id", Json::texto_de(&id)),
                        ("propagar", Json::Bool(false)),
                    ],
                ),
            ),
        ]))
    }

    /// O portao das duas operacoes de escalonamento.
    ///
    /// A conferencia e PROPRIA, e pelo mesmo motivo do `config_gravar`: estas
    /// operacoes nao tem `"tabela"`, entao caem na regra da base vazia do
    /// portao geral. Aquela regra ja exige `administrar` -- mas amarrar a
    /// guarda que decide quem vota numa eleicao a um detalhe de resolucao de
    /// nome de base e deixar a guarda mais importante do cluster depender de
    /// coisa nenhuma.
    fn cluster_para_escalonar(
        &self,
        sessao: &Sessao,
    ) -> Result<Arc<crate::cluster::EstadoCluster>> {
        if let Some(u) = &sessao.usuario {
            if !u.pode_em("", "", Atividade::Administrar) {
                return Err(PhxError::Autorizacao(format!(
                    "{} nao tem permissao de administrar: mexer na lista de nos do \
                     cluster muda quantos votos fazem uma maioria",
                    u.login
                )));
            }
        }
        self.cluster.clone().ok_or_else(Self::sem_cluster)
    }

    /// `"propagar": false` so vale para a ordem que o PROPRIO cluster
    /// propaga -- revisao SEC de 17/09/2026, A4.
    ///
    /// O campo existe para impedir a tempestade (a ordem propagada nao se
    /// propaga de novo), e vinha do pedido sem ninguem perguntar de quem.
    /// De um cliente, dois `cluster_no_remover` sem propagar deixavam o master
    /// sozinho na propria lista -- `1 * 2 > 1`, maioria -- enquanto os outros
    /// dois, ainda com tres na lista e sem ouvir mais o master, promoviam
    /// outro: dois masters gravaveis, sem particao de rede nenhuma. E o
    /// espelho, `cluster_no_acrescentar` com ids fantasmas, recusava toda
    /// escrita. Nenhuma das duas e falha: e uma lista que os outros nunca
    /// confirmaram.
    ///
    /// Ordem INTERNA e a que chega com a credencial do cluster (quando o
    /// cluster tem usuario) e de um endereco da lista viva; sem IP e caminho
    /// de dentro do processo (job, rotina, teste), que nunca teve de quem
    /// vir. O portao mora aqui, UM so, e as duas ops o chamam -- espalha-lo
    /// deixaria uma delas de fora no dia em que alguem esquecesse.
    fn sem_propagar_so_de_dentro(
        &self,
        estado: &crate::cluster::EstadoCluster,
        p: &Json,
        sessao: &Sessao,
    ) -> Result<()> {
        if p.booleano_ou("propagar", true) || sessao.ip.is_empty() {
            return Ok(());
        }
        let c = &estado.config;
        let credencial_do_cluster = sessao_e_do_cluster(c, sessao);
        // Primeiro sem rede: IP contra IP. So quando nao bate e que o nome de
        // host da lista e resolvido -- `endereco` aceita nome («host ou IP»),
        // e a propagacao de um cluster configurado por nome chega de um IP;
        // recusa-la seria a guarda quebrando o cluster que ja rodava. A
        // resolucao fica atras das duas perguntas baratas de proposito: so
        // paga quem chegou com `propagar:false`, com IP, e sem casar por texto.
        let lista = estado.lista();
        let de_um_no = credencial_do_cluster
            && (lista
                .iter()
                .any(|n| mesmo_endereco_ip(&n.endereco, &sessao.ip))
                || lista
                    .iter()
                    .any(|n| endereco_resolve_para(&n.endereco, n.porta, &sessao.ip)));
        if de_um_no {
            return Ok(());
        }
        Err(PhxError::Autorizacao(
            self.msg("erro.escalonar_sem_propagar", &[]),
        ))
    }

    /// Grava a lista VIVA no `config.json` deste no.
    ///
    /// Servidor sem arquivo (`--config`) nao e erro: a lista viva ja mudou e o
    /// cluster ja funciona. O que ele perde e a sobrevivencia ao reinicio, e a
    /// resposta diz isso em vez de fingir que gravou.
    fn gravar_a_lista_do_cluster(&self, estado: &crate::cluster::EstadoCluster) -> Result<()> {
        let Some(caminho) = &self.config.caminho else {
            return Ok(());
        };
        crate::config::Config::gravar_nos_do_cluster(caminho, &estado.lista())?;
        Ok(())
    }

    /// Manda a MESMA ordem aos outros nos, uma vez cada, e conta o que cada um
    /// respondeu.
    ///
    /// `"propagar": false` no pedido que chega e o que impede a tempestade: a
    /// ordem propagada nunca se propaga de novo.
    fn propagar_no_cluster(
        &self,
        estado: &crate::cluster::EstadoCluster,
        pedido: &Json,
        corpo: Vec<(&'static str, Json)>,
    ) -> Json {
        if !pedido.booleano_ou("propagar", true) {
            return Json::Nulo;
        }
        let c = &estado.config;
        // O MESMO prazo do pulso (pedido 580): chama as mesmas funcoes na
        // mesma ordem, e o irmao sem total seria a thread de quem escalona
        // presa ao no que goteja.
        let espera = crate::replica::prazo_do_pulso(c.pulso_s);
        let prazo = Duration::from_secs(c.pulso_s.clamp(1, 3));
        let mut vereditos = Vec::new();
        for no in estado.outros() {
            let veredito = (|| -> Result<Json> {
                let mut cliente = crate::replica::Cliente::conectar_com_prazo(
                    &no.endereco,
                    no.porta,
                    &c.token,
                    espera,
                    prazo,
                )?;
                // A propagacao e trafego do cluster como o pulso: com a cifra
                // ligada ela tambem vai por dentro do tunel, senao um no com
                // `cifra_fio.exigir` recusaria a ordem de escalonamento em
                // claro. O pino e o do no de destino.
                if c.cifra {
                    cliente.cifrar(no.pino_do_fio()?)?;
                }
                if !c.usuario.is_empty() {
                    cliente.autenticar(&c.usuario, &c.senha_hash, "")?;
                }
                cliente.pedir(corpo.clone())
            })();
            vereditos.push((
                no.id.clone(),
                match veredito {
                    Ok(_) => Json::texto_de("ok"),
                    // O erro INTEIRO, e nao "falhou": quem escalona precisa
                    // saber se o no esta fora do ar ou se recusou por
                    // permissao -- os dois se consertam de jeitos diferentes.
                    Err(e) => Json::texto_de(format!("{e}")),
                },
            ));
        }
        Json::Objeto(vereditos)
    }

    /// `cluster_estado`: quem e o master, a epoca e o mapa dos nos --
    /// respondida igual em QUALQUER no. E o endereco unico do cluster pelo
    /// protocolo: o cliente valida com um endereco qualquer e e apontado ao
    /// certo. VIP de rede e infraestrutura, nao banco.
    ///
    /// # Duas metades, por permissao (revisao SEC de 17/09/2026, A6)
    ///
    /// `ler` recebe o que um cliente precisa para achar quem manda: `master`
    /// (id e endereco), `escrita_liberada`, epoca e papel deste no. A lista
    /// `nos[]` -- endereco, epoca, posicao e idade do pulso de CADA no -- so
    /// vai para quem `administra`, pelo mesmo criterio do `sistema`: nome de
    /// placa de rede e ponto de montagem descrevem a infraestrutura, e nao o
    /// dado; quem so le uma tabela nao ganha nada com o mapa, e o atacante
    /// ganha o alvo do pulso forjado. O campo fica AUSENTE (nao vazio) para
    /// quem nao pode: lista vazia diria «nao ha nos», que e mentira.
    pub(super) fn op_cluster_estado(&self, sessao: &Sessao) -> Result<Json> {
        let Some(estado) = &self.cluster else {
            return Err(Self::sem_cluster());
        };
        let agora = crate::agora_ms();
        let c = &estado.config;
        let administra = sessao
            .usuario
            .as_ref()
            .is_none_or(|u| u.pode_em("", "", Atividade::Administrar));
        let mapa = if administra {
            estado.mapa()
        } else {
            HashMap::new()
        };
        let lista = if administra {
            estado.lista()
        } else {
            Vec::new()
        };
        // Pedido 294: o vetor de cada no e em que tabelas ele esta atras do
        // mais adiantado. Calculado sobre a MESMA lista que vira `nos`, para o
        // `atras_em` de um no nunca comparar com quem a tela nao mostra.
        let meu_vetor = estado.por_tabela();
        let vetores: Vec<Option<&[(String, u64)]>> = lista
            .iter()
            .map(|n| {
                if n.id == c.id {
                    Some(&meu_vetor[..])
                } else {
                    mapa.get(&n.id).and_then(|p| p.por_tabela.as_deref())
                }
            })
            .collect();
        let atras = crate::cluster::atras_em(&vetores);
        let nos: Vec<Json> = lista
            .iter()
            .zip(vetores.iter().zip(atras))
            .map(|(n, (vetor, atras))| {
                let (papel, epoca, posicao, incompleta, idade_ms) = if n.id == c.id {
                    (
                        Some(estado.papel().nome()),
                        estado.epoca(),
                        estado.posicao(),
                        estado.posicao_incompleta(),
                        0i64,
                    )
                } else {
                    match mapa.get(&n.id) {
                        Some(p) => (
                            Some(p.papel.nome()),
                            p.epoca,
                            p.posicao,
                            p.incompleta,
                            agora - p.quando_ms,
                        ),
                        None => (None, 0, 0, false, -1),
                    }
                };
                Json::objeto(vec![
                    ("id", Json::texto_de(&n.id)),
                    ("endereco", Json::texto_de(n.alvo())),
                    ("este", Json::Bool(n.id == c.id)),
                    (
                        "papel",
                        match papel {
                            Some(p) => Json::texto_de(p),
                            None => Json::Nulo,
                        },
                    ),
                    ("epoca", Json::de_u64(epoca)),
                    ("posicao", Json::de_u64(posicao)),
                    // Pedido 211: a posicao saiu incompleta (tabela nao abriu).
                    // A tela mostra isso ao lado da posicao -- a verdade visivel.
                    ("posicao_incompleta", Json::de_bool(incompleta)),
                    // Pedido 294: MEDIDA ao lado da soma, nunca voto. Nulo =
                    // o no nao mandou vetor (versao anterior), e nao «zero».
                    (
                        "por_tabela",
                        match vetor {
                            Some(v) => Json::Objeto(
                                v.iter()
                                    .map(|(t, n)| (t.clone(), Json::de_u64(*n)))
                                    .collect(),
                            ),
                            None => Json::Nulo,
                        },
                    ),
                    (
                        "atras_em",
                        match atras {
                            Some(l) => Json::Lista(l.iter().map(Json::texto_de).collect()),
                            None => Json::Nulo,
                        },
                    ),
                    // -1 = nunca deu pulso; 0 = este proprio no.
                    ("ultimo_pulso_ms", Json::de_i64(idade_ms)),
                    (
                        "vivo",
                        Json::Bool(n.id == c.id || (0..=c.janela_ms()).contains(&idade_ms)),
                    ),
                ])
            })
            .collect();
        let mut resposta = vec![
            ("id", Json::texto_de(&c.id)),
            ("papel", Json::texto_de(estado.papel().nome())),
            ("epoca", Json::de_u64(estado.epoca())),
            (
                "master",
                match estado.master_atual().and_then(|(id, _)| estado.no(&id)) {
                    Some(n) => Json::objeto(vec![
                        ("id", Json::texto_de(&n.id)),
                        ("endereco", Json::texto_de(n.alvo())),
                    ]),
                    None => Json::Nulo,
                },
            ),
            ("escrita_liberada", Json::Bool(estado.escrita_liberada())),
            (
                "degradado",
                Json::Lista(estado.degradacao().iter().map(Json::texto_de).collect()),
            ),
            ("janela_inatividade_s", Json::de_u64(c.janela_s)),
        ];
        if administra {
            resposta.push(("nos", Json::Lista(nos)));
        }
        Ok(Json::objeto(resposta))
    }

    /// Confere o numero de origem de um par contra TODOS os ja vistos, e
    /// anota o novo no disco -- pedido 329.
    ///
    /// UM portao para os dois sentidos: a rodada que PUXA confere a origem
    /// (o numero que o `posicao` dela diz), e o `replicar` que SERVE confere
    /// quem pede (o numero do `para`). E o proprio numero entra antes de
    /// qualquer outro, para que «o outro tem o MEU numero» seja so um caso da
    /// mesma conta -- era a conferencia do par, e ela nao via dois caixas com
    /// o mesmo numero entre si.
    ///
    /// # O disco ANTES de aceitar (parecer do DBA de 01/10/2026, §3)
    ///
    /// O par novo so e aceito depois de o registro estar no disco, pela troca
    /// duravel: aceitar antes e cair perderia o registro e deixaria outro id
    /// ganhar o mesmo numero -- e a ambiguidade iria para dentro dos `.log`,
    /// onde nao se desfaz. Por isso a conta e feita numa COPIA, e a memoria so
    /// recebe o par novo depois de a gravacao voltar `Ok`: com a memoria
    /// atualizada antes, um disco que recusasse deixaria a proxima chamada
    /// achar o par «ja conhecido» e aceita-lo sem nunca ter gravado.
    pub(super) fn conferir_numero_do_par(&self, numero: u16, id: &str) -> Result<()> {
        let rep = &self.config.replicacao;
        let mut guarda = self.numeros_bidi.tomar("numeros_bidi")?;
        let vistos = match &*guarda {
            Ok(v) => v,
            Err(motivo) => return Err(PhxError::Esquema(motivo.clone())),
        };
        let mut copia = vistos.clone();
        let mut novo = false;
        let meu = rep.numero();
        if meu != 0 {
            novo |= bidirecional::conferir_numero(&mut copia, meu, &rep.id_servidor)
                .map_err(PhxError::Esquema)?;
        }
        novo |= bidirecional::conferir_numero(&mut copia, numero, id).map_err(PhxError::Esquema)?;
        if novo {
            // Raro: so na primeira vez de cada par.
            bidirecional::gravar_numeros(
                &self.config.base.join("replicacao-numeros.json"),
                &copia,
            )?;
            *guarda = Ok(copia);
        }
        Ok(())
    }
}

/// O que a posicao do diario soma -- pedido 300.
#[derive(Clone, Copy)]
enum EscopoDaPosicao<'a> {
    /// O master: o que ele tem E serve -- toda tabela que o usuario do
    /// cluster pode `replicar` ([`replica_alcanca`]). `None` = cluster sem
    /// usuario (a replica entra pelo token e alcanca tudo), ou usuario que o
    /// cadastro nao tem -- e ai nenhuma replica entra, e contar tudo e o
    /// comportamento de antes.
    DoQueSeServe(Option<&'a Usuario>),
    /// So o que o master anunciou: a replica que ja o ouviu.
    DoMaster(&'a crate::cluster::Anunciadas),
    /// A replica que nunca ouviu um master: conta tudo e sai INCOMPLETA,
    /// porque o numero pode carregar tabela que so mora aqui.
    Desconhecido,
}

/// A sessao entrou com a credencial do cluster (`cluster.usuario`)?
///
/// Cluster sem usuario declarado = sim (o comportamento de sempre: entra pelo
/// token). E o MESMO predicado que o crivo do `propagar` usa, escrito uma vez
/// para o quorum nao ter uma segunda opiniao sobre quem e «o no» (649).
pub(super) fn sessao_e_do_cluster(c: &crate::config::Cluster, sessao: &Sessao) -> bool {
    c.usuario.is_empty() || sessao.login() == c.usuario
}

/// A `Origem` com que a replicacao do cluster puxa do master CORRENTE.
///
/// Fora do laco, e pura, para o teste conferir o que a leitura nao pega: que a
/// cifra do cluster e o pino do master ATRAVESSAM ate a origem. Cifrar o pulso
/// e esquecer aqui deixaria a replicacao do cluster em claro -- a outra metade
/// do defeito `pulso-do-cluster-em-claro`, e a guarda
/// `replicacao-do-cluster-em-claro` repoe justamente esta linha.
///
/// O pino e o do no de DESTINO (o master de quem se puxa), como o pino do pulso
/// -- known_hosts, cada no confere a chave de quem alcanca.
pub(super) fn origem_do_master(
    c: &crate::config::Cluster,
    no: &crate::config::NoCluster,
) -> crate::config::Origem {
    crate::config::Origem {
        nome: format!("{PREFIXO_DA_ORIGEM_DO_CLUSTER}{}", no.id),
        host: no.endereco.clone(),
        porta: no.porta,
        token: c.token.clone(),
        databases: c.databases.clone(),
        reconectar_em: c.pulso_s,
        usuario: c.usuario.clone(),
        senha_hash: c.senha_hash.clone(),
        senha: String::new(),
        // A origem do cluster e sempre STREAMING: quem marca o ritmo e o pulso.
        // Agendar aqui atrasaria a deteccao de master novo.
        cada_minutos: 0,
        hora: String::new(),
        // A replicacao do cluster viaja pela MESMA cifra do pulso -- ligar
        // `cluster.cifra` protege o trafego INTEIRO do cluster, nunca so metade.
        cifra: c.cifra,
        chave_do_fio: no.chave_do_fio.clone(),
        espelho: false,
    }
}
