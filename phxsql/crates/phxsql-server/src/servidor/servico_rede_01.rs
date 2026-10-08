//! A rede do fio de dados: o ouvinte e o `accept`, o aperto, o `atender`, o
//! `Remoto` e a `Janela`, o desafio e o login. O portao (`despachar` ate
//! `portoes_do_pedido`) e o `executar` ficam no `servidor.rs`.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// Dois textos de endereco apontam para o MESMO IP?
///
/// Compara como IP quando os dois analisam (`::ffff:10.0.0.2` e `10.0.0.2`
/// sao o mesmo endereco), e como texto quando um deles e um nome de host --
/// sem resolver DNS: resolver aqui seria uma ida a rede dentro de um portao,
/// e um nome que nao bate como texto simplesmente nao autoriza.
pub(super) fn mesmo_endereco_ip(a: &str, b: &str) -> bool {
    use std::net::IpAddr;
    match (a.trim().parse::<IpAddr>(), b.trim().parse::<IpAddr>()) {
        (Ok(x), Ok(y)) => x.to_canonical() == y.to_canonical(),
        _ => a.trim().eq_ignore_ascii_case(b.trim()),
    }
}

/// Uma conexao viva para outro PhxSql, do lado de ca da interface.
pub struct Remoto {
    pub destino: String,
    leitor: BufReader<TcpStream>,
    escrita: TcpStream,
    /// Em claro (como sempre foi) ou dentro do tunel. O canal e UM so para que
    /// `conversar` nao repita `if cifrado` em toda escrita e leitura -- a
    /// mesma decisao que a `replica::Cliente` tomou. Ver `docs/CIFRA-DO-FIO.md`.
    pub(super) canal: Canal,
}

impl Remoto {
    /// Abre a conexao. Nao autentica -- quem autentica e o pedido de login,
    /// que segue por aqui igual a qualquer outro.
    pub fn abrir(destino: &str, timeout_s: u64) -> Result<Remoto> {
        use std::net::ToSocketAddrs;
        let endereco = destino
            .to_socket_addrs()
            .map_err(|e| PhxError::Esquema(format!("destino {destino:?} nao resolve: {e}")))?
            .next()
            .ok_or_else(|| PhxError::Esquema(format!("destino {destino:?} sem endereco")))?;
        let fluxo =
            TcpStream::connect_timeout(&endereco, Duration::from_secs(timeout_s.min(10)))
                .map_err(|e| PhxError::Esquema(format!("nao consegui falar com {destino}: {e}")))?;
        fluxo.set_read_timeout(Some(Duration::from_secs(timeout_s)))?;
        let escrita = fluxo.try_clone()?;
        Ok(Remoto {
            destino: destino.to_string(),
            leitor: BufReader::new(fluxo),
            escrita,
            canal: Canal::Claro,
        })
    }

    /// Pede o aperto de mao e passa a falar por dentro do tunel.
    ///
    /// Igual em espirito ao `replica::Cliente::cifrar`, e pela MESMA razao de
    /// vir ANTES do login: e a prova do desafio-resposta e o token que o tunel
    /// existe para esconder, e depois do login ja seria tarde. `pino` e a
    /// chave publica que se ESPERA do destino; com pino, um destino que
    /// apresente outra chave derruba a conexao -- e assim a interface se
    /// protege de quem esta no meio. Sem pino, o tunel protege so da escuta
    /// PASSIVA, e o arranque ja avisou disso.
    ///
    /// Devolve a chave que o destino apresentou, para quem quiser anota-la.
    pub fn cifrar(&mut self, pino: Option<[u8; 32]>) -> Result<[u8; 32]> {
        let (iniciador, m1) = phxsql_core::fio::Iniciador::comecar(pino);
        let pedido = Json::objeto(vec![
            ("op", Json::texto_de("cifrar")),
            ("e", Json::texto_de(phxsql_core::base64::codificar(&m1))),
        ])
        .escrever();
        writeln!(self.escrita, "{pedido}")?;
        self.escrita.flush()?;

        // O TETO vale aqui tambem -- pedido 312, e este e o IRMAO exato do
        // `replica::Cliente::cifrar`: mesma coreografia, mesma leitura antes
        // de o tunel existir e antes de qualquer autenticacao. O canal ainda
        // e `Claro`, entao quem le e o mesmo `Canal` de sempre, so com o teto
        // do APERTO. Sem ele, quem escolhe quanta memoria esta interface
        // reserva e o servidor do outro lado -- medido no irmao: 192 MiB numa
        // linha so, em 294 ms.
        let resposta = match self
            .canal
            .ler_ate(&mut self.leitor, phxsql_core::fio::TETO_DO_APERTO)?
        {
            Recebido::Linha(l) => l,
            Recebido::Fim => {
                return Err(PhxError::Esquema(format!(
                    "{} fechou a conexao no aperto de mao",
                    self.destino
                )))
            }
        };
        let j = Json::analisar(&resposta)?;
        if !j.booleano_ou("ok", false) {
            return Err(PhxError::Autorizacao(format!(
                "{} recusou o aperto de mao: {}",
                self.destino,
                j.texto_ou("erro", "sem motivo")
            )));
        }
        let m2 = phxsql_core::base64::decodificar(
            j.campo("resultado")
                .map(|r| r.texto_ou("m2", ""))
                .unwrap_or(""),
        )?;
        let (transporte, apresentada) = iniciador.terminar(&m2)?;
        self.canal = Canal::Cifrado(Box::new(transporte));
        Ok(apresentada)
    }

    /// Manda uma linha e devolve a resposta, crua.
    ///
    /// Crua de proposito: o que o servidor remoto respondeu e o que o
    /// navegador recebe. Reescrever no meio do caminho seria mentir sobre
    /// quem respondeu o que. Em claro ou cifrado, o `Canal` cuida do sela e
    /// abre; o unico corte comum e trocar `\n`/`\r` do pedido por espaco,
    /// porque o registro do protocolo e uma linha so.
    pub fn conversar(&mut self, linha: &str) -> Result<Json> {
        let limpa = linha.replace(['\n', '\r'], " ");
        self.canal.escrever(&mut self.escrita, &limpa)?;
        match self.canal.ler(&mut self.leitor)? {
            Recebido::Linha(l) => Json::analisar(&l),
            Recebido::Fim => Err(PhxError::Esquema(format!(
                "{} fechou a conexao",
                self.destino
            ))),
        }
    }
}

/// Decide QUANDO uma gravacao vai de fato para o disco.
///
/// O `write` acontece sempre, na hora: os bytes vao para o sistema operacional
/// em toda gravacao, sem buffer nosso, entao outro processo ve o dado
/// imediatamente. O que este contador decide e o `fsync`, que e o que protege
/// de o computador perder energia antes de o sistema descarregar a pagina.
///
/// Medido: sincronizar a cada linha da 1.289 linhas/s; a cada 200, 20.000/s.
/// Eram **95% do tempo da insercao**.
pub(super) struct Janela {
    modo: Durabilidade,
    a_cada: u64,
    ms: u64,
    /// Gravacoes desde o ultimo `fsync`.
    pendentes: AtomicU64,
    /// Quando a janela corrente abriu.
    desde: Mutex<Instant>,
}

impl Janela {
    pub(super) fn nova(r: &crate::config::Recursos) -> Janela {
        Janela {
            modo: r.durabilidade,
            a_cada: r.lote_operacoes,
            ms: r.lote_milissegundos,
            pendentes: AtomicU64::new(0),
            desde: Mutex::new(Instant::now()),
        }
    }

    /// Conta mais uma gravacao e diz se e hora de sincronizar.
    ///
    /// Fecha a janela por QUANTIDADE ou por TEMPO, o que vier primeiro. So por
    /// quantidade, um servidor com pouco movimento deixaria a ultima gravacao
    /// pendurada indefinidamente; so por tempo, uma carga em massa encheria a
    /// memoria entre um relogio e outro.
    pub(super) fn hora_de_gravar(&self) -> bool {
        match self.modo {
            Durabilidade::PorOperacao => true,
            Durabilidade::Sistema => false,
            Durabilidade::PorLote => {
                let n = self.pendentes.fetch_add(1, Ordering::SeqCst) + 1;
                if n >= self.a_cada {
                    self.fechar();
                    return true;
                }
                let mut desde = match self.desde.lock() {
                    Ok(g) => g,
                    Err(e) => e.into_inner(),
                };
                if desde.elapsed().as_millis() as u64 >= self.ms {
                    self.pendentes.store(0, Ordering::SeqCst);
                    *desde = Instant::now();
                    return true;
                }
                false
            }
        }
    }

    pub(super) fn fechar(&self) {
        self.pendentes.store(0, Ordering::SeqCst);
        if let Ok(mut d) = self.desde.lock() {
            *d = Instant::now();
        }
    }

    /// Ha gravacao esperando o `fsync`?
    pub(super) fn pendente(&self) -> u64 {
        self.pendentes.load(Ordering::SeqCst)
    }
}

/// Abaixo disto, comprimir SOBRA em vez de ajudar.
///
/// O envelope custa dois precos: o cabecalho/tabela do DEFLATE e o Base64 (que
/// so para caber numa linha de texto ja incha o binario em ~33%). Numa
/// resposta pequena — um `ping`, um erro, um `inserir` de uma linha — esse
/// custo fixo passa do que a compressao economiza, e o cliente receberia uma
/// linha MAIOR fingindo ser uma otimizacao. `talvez_comprimir` tambem confere
/// o tamanho final contra o original por seguranca, mas o limiar evita gastar
/// o DEFLATE (que nao e de graca) em respostas onde o resultado ja se sabe de
/// antemao.
const LIMIAR_COMPRESSAO_BYTES: usize = 256;

impl Servidor {
    /// O mesmo `escutar`, num ouvinte que QUEM CHAMA ja abriu -- pedidos
    /// 352 e 401.
    ///
    /// Existe para o caso em que o numero da porta precisa ser conhecido
    /// ANTES de o servidor existir: o par de replicacao e o cluster de dois
    /// escrevem a porta de um no config do outro. Sem isto a unica saida era
    /// sortear uma porta, solta-la e torcer para ninguem pega-la antes do
    /// `bind` -- e quando alguem pegava, o `bind` falhava, o teste engolia a
    /// falha e passava a conversar com o servidor VIZINHO (medido: «database
    /// loja ja existe» vindo do servidor de outro teste). Com o ouvinte preso
    /// desde o sorteio nao ha janela: o numero nunca volta ao sistema.
    ///
    /// O TLS e conferido antes de qualquer conexao ser aceita, como no
    /// `escutar`; o `bind` do config nao e consultado -- a porta e a do
    /// ouvinte, e e ela que `porta_dos_dados` devolve.
    pub fn escutar_em(self: &Arc<Self>, ouvinte: TcpListener) -> Result<()> {
        self.preparar_tls_dos_dados()?;
        self.atender_no_ouvinte(ouvinte)
    }

    /// Pediu TLS e nao deu: a porta NAO sobe. Cair calada para o claro seria
    /// o rebaixamento que a `docs/CIFRA-DO-FIO.md` §2 recusa. Vem ANTES do
    /// `bind` no `escutar` para a porta nunca abrir em claro nem por um
    /// instante.
    pub(super) fn preparar_tls_dos_dados(&self) -> Result<()> {
        let tls = self.identidade_http("dados", &self.config.tls, &self.config.bind)?;
        if let Ok(mut t) = self.tls_dos_dados.lock() {
            *t = tls;
        }
        Ok(())
    }

    /// O corpo comum do `escutar` e do `escutar_em`: um so, para os dois
    /// arranques nunca divergirem no que sobem junto com a porta.
    pub(super) fn atender_no_ouvinte(self: &Arc<Self>, ouvinte: TcpListener) -> Result<()> {
        let endereco = ouvinte
            .local_addr()
            .map_err(|e| PhxError::Esquema(format!("o ouvinte entregue nao tem endereco: {e}")))?;
        anunciar(&format!(
            "PhxSql {VERSAO} escutando em {endereco} | base {} | papel {}",
            self.config.base.display(),
            self.config.replicacao.papel.nome()
        ));
        eprintln!("log de acessos: {}", self.config.log_acessos.display());
        if self.config.replicacao.papel != crate::config::Papel::Isolado {
            let portas = self.config.replicacao.portas();
            eprintln!(
                "replicacao: papel {} | {}",
                self.config.replicacao.papel.nome(),
                if portas.is_empty() {
                    "envio e retorno pela porta de dados".to_string()
                } else {
                    portas
                        .iter()
                        .map(|(k, v)| format!("{k} {v}"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                }
            );
            if self.config.replicacao.papel == crate::config::Papel::Source
                && !self.config.replicacao.imagem_da_linha
            {
                eprintln!(
                    "ATENCAO: source com replicacao.imagem_da_linha DESLIGADA. O \
                     diario grava que a linha mudou, nao grava para que, e as \
                     replicas nao terao o que aplicar."
                );
            }
        }

        // A replicacao aberta a quem tiver o token -- ver `replicacao_aberta`.
        // FORA do `if papel != Isolado` de proposito: e justamente o servidor
        // que ninguem declarou como source que serve o diario sem saber.
        if let Some(com_imagem) = self.replicacao_aberta() {
            eprintln!(
                "ATENCAO: replicacao.replicas_autorizadas esta VAZIA -- \
                 `posicao`, `replicar` e `aplicar` atendem QUALQUER endereco \
                 que tenha o token{}. Para fechar: liste os IPs das replicas \
                 em replicacao.replicas_autorizadas.",
                if com_imagem {
                    ", e com replicacao.imagem_da_linha ligada o `replicar` \
                     entrega a LINHA INTEIRA"
                } else {
                    ""
                }
            );
        }
        // Pedido 284: a lista compara o par do soquete, e atras do proxy o par
        // e o proxy. O portao 2a-bis recusa a replicacao nessas portas; dizer
        // no arranque poupa quem apontou a replica para o proxy de descobrir
        // pela recusa.
        let lista_preenchida = !self.config.replicacao.replicas_autorizadas.is_empty();
        for (ligada, proxy, secao) in [
            (
                self.config.web.ligado,
                self.config.web.atras_de_proxy,
                "web",
            ),
            (
                self.config.rest.ligado,
                self.config.rest.atras_de_proxy,
                "rest",
            ),
        ] {
            if lista_preenchida && ligada && proxy {
                eprintln!(
                    "replicacao: a secao {secao} declara atras_de_proxy, entao o IP \
                     que ela ve e o do proxy e replicacao.replicas_autorizadas nao \
                     decide ali -- `posicao`, `replicar`, `aplicar` e \
                     `cluster_pulso` sao recusados por essa porta. A replica \
                     entra pela porta de dados."
                );
            }
        }

        self.subir_web();
        self.subir_rest();
        self.subir_swagger();
        self.subir_replicacao();
        self.subir_cluster();
        self.subir_backup_agendado();
        self.subir_jobs();
        self.subir_retencao_da_trilha();
        self.ligar_relogio_de_gravacao();
        self.ligar_vigia_de_disco();
        self.ligar_sonda_de_disco();
        self.ligar_vigia_de_jobs();
        self.subir_amostrador();
        // A thread PRINCIPAL tambem entra no registro. Ela nao e criada por
        // ninguem -- e o processo --, e por isso era a unica que faltaria numa
        // lista montada so pelos `spawn`. Um inventario com um buraco e um
        // inventario em que nao se confia.
        let principal = self.telemetria.registrar_fio(
            "aceitador-dados",
            "a thread principal: fica no `accept` da porta de dados e entrega \
             cada conexao nova a uma thread de atendimento",
            "servico",
            crate::agora_ms(),
        );
        principal.fazendo("aceitando conexoes");

        self.anotar_porta_no_ar(&ouvinte);
        let mut atual = Some(ouvinte);
        loop {
            match atual.take() {
                Some(o) => {
                    self.aceitar_ate_mandarem_parar(&o);
                    // A porta so e SOLTA aqui, depois do laco sair -- e o
                    // ouvinte novo, quando ha, ja esta preso desde antes de
                    // qualquer coisa parar. Ver `op_servico_subir`.
                    drop(o);
                    self.porta_no_ar.store(false, Ordering::SeqCst);
                    eprintln!("porta de dados PARADA (a interface web continua no ar)");
                }
                // Parada: a linha de execucao fica aqui, de olho no pedido de
                // subir de novo. Um quarto de segundo de espera so acontece
                // enquanto o servico esta parado, que e o caso raro.
                None => std::thread::sleep(Duration::from_millis(250)),
            }
            if let Ok(mut p) = self.proximo_ouvinte.lock() {
                if let Some(novo) = p.take() {
                    self.anotar_porta_no_ar(&novo);
                    atual = Some(novo);
                }
            }
        }
    }

    /// Aceita conexoes ate alguem pedir para parar.
    ///
    /// # Como o `accept` acorda
    ///
    /// Ele bloqueia, e nao ha como interromper um `accept` bloqueado sem
    /// mexer no laco. As duas saidas eram: pesquisar de tempos em tempos com
    /// o soquete em modo nao bloqueante, ou ACORDAR o laco com uma conexao.
    ///
    /// A pesquisa foi descartada por medicao de custo, e nao por gosto: um
    /// intervalo de 100 ms poe ate 100 ms de espera em TODA conexao nova, o
    /// tempo inteiro, para servir um pedido de parada que acontece uma vez por
    /// mes. O despertador custa zero enquanto ninguem para: quem pede a parada
    /// levanta o sinalizador e conecta no proprio endereco, o `accept`
    /// devolve, e a primeira coisa do laco e olhar o sinalizador.
    ///
    /// A conexao do despertador nao vira sessao: o laco sai antes de atender.
    pub(super) fn aceitar_ate_mandarem_parar(self: &Arc<Self>, ouvinte: &TcpListener) {
        loop {
            let conexao = ouvinte.accept();
            // ANTES de atender: se o pedido de parada chegou junto com uma
            // conexao de verdade, quem mandou parar ganha -- e a conexao
            // recusada volta a existir quando a porta subir de novo.
            if self.parar_de_aceitar.swap(false, Ordering::SeqCst) {
                return;
            }
            match conexao {
                Ok((fluxo, _)) => {
                    // Sem isto, o Nagle segura a resposta ate 40 ms esperando
                    // mais bytes para encher um pacote -- e nunca vem mais,
                    // porque a resposta acabou. Medido: a pagina de uma tabela
                    // de 20.000 linhas levava 1 ms de servidor e 44 ms de
                    // relogio, e 43 deles eram esta linha faltando.
                    //
                    // O protocolo aqui e pedido-resposta curto, que e o caso
                    // exato em que o Nagle atrapalha em vez de ajudar.
                    let _ = fluxo.set_nodelay(true);
                    let par = fluxo.peer_addr().ok();
                    #[cfg(test)]
                    self.aceitas_de_teste.fetch_add(1, Ordering::SeqCst);
                    // Uma vaga AGORA, ou recusa: e o `max_connections` do
                    // PostgreSQL («too many clients already»), do MySQL e do
                    // MariaDB («Too many connections»), e tres motores
                    // maduros convergindo e aceite automatico. A espera com
                    // prazo fica para as portas HTTP, onde o molde e outro.
                    let Some(permissao) = self.permissoes_de_dados.tentar() else {
                        #[cfg(test)]
                        self.sem_vaga_de_teste.fetch_add(1, Ordering::SeqCst);
                        // Recusa sem derrubar o servico, e deixa registro.
                        if let Some(p) = par {
                            self.anotar(&Acesso {
                                quando_ms: crate::agora_ms(),
                                ip: p.ip().to_string(),
                                porta_origem: p.port(),
                                op: "conexao".into(),
                                usuario: String::new(),
                                autenticado: false,
                                ok: false,
                                duracao_ms: 0,
                                erro: Some("limite de conexoes atingido".into()),
                                database: String::new(),
                                tabela: String::new(),
                                codigo: 0,
                            });
                        }
                        continue;
                    };
                    let servidor = Arc::clone(self);
                    let endereco = par.unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)));
                    self.telemetria.subir(
                        format!("dados-{}", endereco.port()),
                        "atende UMA conexao da porta de dados, do login ate o fim: \
                         le uma linha, despacha o pedido e responde, em laco",
                        "atendimento",
                        crate::agora_ms(),
                        move |fio| {
                            // A vaga mora na thread da conexao e morre com
                            // ela -- pelo fim do `atender` ou por um panico
                            // dentro dele. Nao ha `fetch_sub` para pular.
                            let _vaga = permissao;
                            fio.fazendo(&format!("conexao de {endereco}"));
                            servidor.atender(fluxo, endereco);
                        },
                    );
                }
                Err(e) => eprintln!("conexao recusada pelo sistema: {e}"),
            }
        }
    }

    /// A porta de dados que o servidor escuta AGORA, e nao a do arquivo.
    ///
    /// Um lugar so para o `/saude` e o `ping`: a tela dizia «porta 5000» de
    /// cabeca (pedido 645) porque so o formulario de entrada sabia a real.
    pub(super) fn porta_dados_agora(&self) -> u16 {
        self.endereco_dos_dados
            .lock()
            .ok()
            .and_then(|e| *e)
            .map(|e| e.port())
            .or_else(|| self.config.endereco().ok().map(|e| e.port()))
            .unwrap_or(0)
    }

    /// Guarda o endereco em que a porta de dados esta escutando AGORA.
    ///
    /// Ele pode diferir do `bind` do `config.json` depois de uma troca pela
    /// tela -- e a tela mostra os dois lado a lado justamente por isso.
    /// Configuracao que nao e lida mente; endereco corrente que finge ser o
    /// configurado mente do mesmo jeito.
    pub(super) fn anotar_porta_no_ar(&self, ouvinte: &TcpListener) {
        if let Ok(e) = ouvinte.local_addr() {
            if let Ok(mut atual) = self.endereco_dos_dados.lock() {
                *atual = Some(e);
            }
            anunciar(&format!("porta de dados escutando em {e}"));
        }
        self.porta_no_ar.store(true, Ordering::SeqCst);
    }

    /// O fio da conexao da porta de dados: claro, ou TLS quando a porta tem
    /// identidade e o PRIMEIRO byte e o de um registro de aperto (`0x16`).
    ///
    /// Na mesma porta, e decidido pelo cliente -- como o PostgreSQL
    /// (`SSLRequest`) e o MySQL (a flag no aperto), que negociam o TLS na
    /// porta de sempre. Um pedido JSON comeca por `{`, e nunca por `0x16`,
    /// entao a decisao nao adivinha nada. `None` = a conexao acabou aqui.
    fn abrir_fio_de_dados(
        &self,
        fluxo: TcpStream,
        par: SocketAddr,
    ) -> Option<(crate::fio_dados::Leitura, crate::fio_dados::Escrita)> {
        let id = self.tls_dos_dados.lock().ok().and_then(|t| t.clone());
        let Some(id) = id else {
            return crate::fio_dados::claro(fluxo).ok();
        };
        let mut primeiro = [0u8; 1];
        match fluxo.peek(&mut primeiro) {
            Ok(1) if primeiro[0] == 0x16 => {}
            Ok(1) => return crate::fio_dados::claro(fluxo).ok(),
            _ => return None,
        }
        match phxsql_core::tls::aceitar(fluxo, &id, &[]) {
            Ok(t) => Some(crate::fio_dados::tls(t)),
            Err(e) => {
                self.anotar_aperto_tls_recusado(par, "dados", &e);
                None
            }
        }
    }

    pub(super) fn atender(&self, fluxo: TcpStream, par: SocketAddr) {
        // So nos testes: a prova real da permissao RAII (pedido 248) precisa
        // de um panico DE VERDADE dentro da thread da conexao. Simula-lo por
        // fora provaria outra coisa. Compilado fora do binario de producao.
        #[cfg(test)]
        if self.panicos_de_teste.load(Ordering::SeqCst) > 0 {
            self.panicos_de_teste.fetch_sub(1, Ordering::SeqCst);
            panic!("panico de teste dentro do atender (pedido 248)");
        }
        let ip = par.ip().to_string();
        let porta = par.port();
        let _ = fluxo.set_read_timeout(Some(Duration::from_secs(self.config.timeout_s)));

        // O soquete vai para o registro para que `encerrar_sessao` consiga
        // fecha-lo de fora: a thread desta conexao passa a vida parada dentro
        // de um `read_line`, e so um `shutdown` a acorda. Clonado ANTES do
        // aperto TLS, que toma posse do soquete.
        let para_fechar = fluxo.try_clone().ok().map(Arc::new);
        let Some((leitura, mut saida)) = self.abrir_fio_de_dados(fluxo, par) else {
            return;
        };

        // Antes de qualquer coisa: quem esta na lista de bloqueio nao entra.
        let agora = crate::agora_ms();
        if let Some(b) = self.barrado(&ip, agora) {
            self.anotar(&Acesso {
                quando_ms: agora,
                ip: ip.clone(),
                porta_origem: porta,
                op: "conexao".into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some(Self::motivo_de_bloqueio(&b)),
                database: String::new(),
                tabela: String::new(),
                codigo: 0,
            });
            let _ = writeln!(
                saida,
                "{}",
                self.resposta_erro(
                    "conexao",
                    &PhxError::Autorizacao(self.recado_de_bloqueio(&b)),
                    0
                )
                .escrever()
            );
            return;
        }

        let permitido = self.config.ip_permitido(&ip);
        let mut leitor = BufReader::new(leitura);

        if !permitido {
            self.violacao_leve(&ip, "conexao", "ip fora da lista de permitidos");
            self.anotar(&Acesso {
                quando_ms: crate::agora_ms(),
                ip,
                porta_origem: porta,
                op: "conexao".into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some("ip fora da lista de permitidos".into()),
                database: String::new(),
                tabela: String::new(),
                codigo: 0,
            });
            let _ = writeln!(
                saida,
                "{}",
                self.resposta_erro(
                    "conexao",
                    &PhxError::Autorizacao(self.msg("erro.ip_nao_autorizado", &[])),
                    0
                )
                .escrever()
            );
            return;
        }

        let (id_ligacao, morrer) = match self.ligacoes.lock() {
            Ok(mut l) => l.entrar(&ip, porta, crate::agora_ms(), para_fechar),
            Err(_) => (0, Arc::new(std::sync::atomic::AtomicBool::new(false))),
        };
        let mut sessao = Sessao {
            ligacao: id_ligacao,
            ip: ip.clone(),
            entrada: Entrada::Dados,
            fio_tls: saida.cifrado(),
            ..Sessao::default()
        };
        // Sai do registro por qualquer caminho -- inclusive os `return` do
        // meio do laco. Sem isto, uma conexao caida ficaria na lista para
        // sempre, e a lista que existe para dizer a verdade passaria a mentir.
        // A chave da bolha e da CONEXAO, e nao do pedido: uma bolha que
        // trocasse de identificador a cada pedido seria redesenhada duas
        // vezes por segundo e ninguem conseguiria clicar nela.
        let chave_da_atividade = format!("dados:{id_ligacao}");
        let _saida_do_registro = AoSair(|| {
            if let Ok(mut l) = self.ligacoes.lock() {
                l.sair(id_ligacao);
            }
            self.telemetria.sair(&chave_da_atividade);
            // A PRIMEIRA rede de protecao da reserva de carga: a conexao caiu,
            // a tabela solta. Sem isto, um cliente morto no meio de uma carga
            // deixaria a tabela reservada ate o prazo vencer -- e o prazo e
            // medido em dezenas de minutos, de proposito.
            self.soltar_cargas_da_ligacao(id_ligacao);
            // E a PRIMEIRA rede da transacao, pelo mesmo motivo e no mesmo
            // lugar: a conexao caiu, a transacao e desfeita. Nada foi gravado,
            // entao desfazer e jogar a lista fora -- zero bytes de trabalho.
            self.soltar_transacao_da_ligacao(id_ligacao);
        });

        // O canal comeca EM CLARO, sempre. E o comportamento de hoje, e ele so
        // muda se o cliente pedir o aperto -- cliente que nunca ouviu falar
        // disto nunca pede, e para ele nada mudou.
        let mut canal = Canal::Claro;
        loop {
            // QUANTO ESTE LADO RESERVA ANTES DE SABER QUEM FALA -- pedido 434.
            //
            // O teto vem do MOTOR, e a pergunta que ele responde muda com a
            // sessao: enquanto ninguem provou quem e, reservar 128 MiB e
            // deixar quem ainda nao e ninguem escolher a memoria deste lado.
            //
            // E ela e feita QUANDO A LINHA PASSA do teto de todos, e nao antes
            // de a leitura bloquear -- pedido 442. A conexao passa a vida
            // parada aqui, e o cadastro muda enquanto ela espera: decidido
            // antes, o teto de um usuario EXCLUIDO continuava o de quem ele
            // era (medido: 1 MiB com `ok:true` depois da exclusao). A ficha
            // se refresca na hora da pergunta, pela mesma funcao que o
            // `despachar` usa, e a linha pequena -- quase todas -- nem
            // pergunta.
            // O TLS da porta (pedido 572) e cifra de verdade, e conta como tal
            // para o teto de quem ainda nao provou quem e.
            let cifrado = canal.cifrado() || saida.cifrado();
            let mut teto = TETO_DO_APERTO;
            // A linha mora DENTRO da volta, e nao fora do laco -- pedido 442,
            // a metade da memoria. Declarada fora, a `String` velha so morria
            // quando a leitura seguinte DEVOLVIA, e a conexao ociosa segurava
            // a linha anterior inteira: medido, VmRSS de 13.640 kB para
            // 80.252 kB parada depois de um ping de 64 MiB. Aqui ela cai no
            // fim de cada volta, antes de a conexao voltar a esperar.
            let lida = canal.ler_decidindo(&mut leitor, TETO_DO_APERTO, &mut || {
                self.refrescar_a_sessao(&mut sessao);
                teto = self.teto_da_linha(&sessao, cifrado);
                teto
            });
            let linha = match lida {
                Ok(Recebido::Linha(l)) => l,
                // Fim limpo: EOF em claro, ou a despedida dentro do tunel.
                Ok(Recebido::Fim) => return,
                Err(e) => {
                    // A linha ACIMA DO TETO deixava de existir: a conexao caia
                    // sem resposta, sem linha no `acessos.log` e sem violacao
                    // -- medido pela bateria `bancada/seguranca/porta.py`
                    // (caso 4b): 134.218.794 bytes derrubaram a conexao em
                    // 0,43 s e o log ganhou ZERO linhas. Memoria protegida,
                    // visibilidade nenhuma: quem opera nao tinha como saber
                    // que alguem tentou.
                    let acima_do_teto = matches!(e, PhxError::LimiteExcedido(_));
                    if acima_do_teto {
                        // Drenar ANTES de responder, e nao depois: fechar um
                        // soquete com dado por ler no buffer de recepcao manda
                        // RST, e o RST joga fora a resposta que o cliente ainda
                        // nao leu. Responder primeiro e fechar em cima seria
                        // "responder" no codigo e continuar invisivel no fio --
                        // que e o defeito que este conserto existe para matar.
                        // A DRENAGEM continua indo ate o teto do REGISTRO,
                        // mesmo quando o teto da leitura foi o pequeno: ela
                        // nao guarda nada (le e descarta com buffer fixo), e
                        // e ela que faz a recusa CHEGAR. Encurta-la junto
                        // devolveria o RST que engole a resposta -- o defeito
                        // do pedido 216, de volta pela porta de quem ainda nao
                        // se identificou, que e justamente quem mais precisa
                        // ouvir o motivo.
                        let sobrou = descartar_ate_a_quebra(&mut leitor, TETO_DO_REGISTRO);
                        let resposta = self.resposta_erro("fio", &e, 0);
                        let _ = canal.escrever(&mut saida, &resposta.escrever());
                        self.anotar(&Acesso {
                            quando_ms: crate::agora_ms(),
                            ip: ip.clone(),
                            porta_origem: porta,
                            op: "fio".into(),
                            usuario: sessao.login().to_string(),
                            autenticado: sessao.usuario.is_some(),
                            ok: false,
                            duracao_ms: 0,
                            // O TAMANHO entra no log, e nao so o teto: quem
                            // investiga precisa distinguir "passou um byte" de
                            // "mandaram meio giga". O numero e o que este lado
                            // LEU (o teto mais um) somado ao que se drenou
                            // depois -- e o texto diz isso, porque o que o
                            // outro lado ainda tinha na mao ninguem mediu.
                            erro: Some(format!(
                                "{e}; lidos {} bytes desta linha (teto {teto})",
                                teto + 1 + sobrou
                            )),
                            database: String::new(),
                            tabela: String::new(),
                            codigo: e.codigo(),
                        });
                        // PEDIDA, NAO IMPOSTA. Mandar uma linha grande demais
                        // e engano de cliente com a mesma cara de ataque, e
                        // bloquear de fabrica trancaria para fora quem so
                        // configurou um lote alto -- o estrago do pedido 203.
                        // Ligado, conta pela politica leve que ja existe.
                        if self.config.politica.contar_linha_acima_do_teto {
                            self.violacao_leve(&ip, "fio", "linha acima do teto");
                        }
                        return;
                    }
                    // Aqui "nao deu erro" NAO pode virar "deu certo": dentro do
                    // tunel, um EOF sem despedida e um fio cortado, e ele vai
                    // para o log como erro em vez de sumir como fim de sessao.
                    if canal.cifrado() {
                        self.anotar(&Acesso {
                            quando_ms: crate::agora_ms(),
                            ip: ip.clone(),
                            porta_origem: porta,
                            op: "fio".into(),
                            usuario: sessao.login().to_string(),
                            autenticado: sessao.usuario.is_some(),
                            ok: false,
                            duracao_ms: 0,
                            erro: Some(e.to_string()),
                            database: String::new(),
                            tabela: String::new(),
                            codigo: 0,
                        });
                    }
                    return;
                }
            };
            // Conferido AQUI, e nao so no `shutdown`: se o pedido chegou junto
            // com o encerramento, quem mandou encerrar ganha.
            if morrer.load(Ordering::SeqCst) {
                return;
            }
            if linha.trim().is_empty() {
                continue;
            }

            // Os dois portoes do fio, e nenhum deles chega ao `despachar`.
            //
            // O `cifrar` e atendido ANTES do portao do token de proposito: o
            // token e justamente uma das coisas que o tunel existe para
            // esconder, e exigi-lo em claro para abrir o tunel esvaziaria
            // metade do ganho. Ele nao concede nada -- quem o completa continua
            // passando por token, login e permissao, agora por dentro.
            if !canal.cifrado() {
                if let Some(pedido) = Servidor::pedido_de_aperto(&linha) {
                    if !self.responder_aperto(&pedido, &mut canal, &mut saida, &ip, porta) {
                        return;
                    }
                    // A transcricao vira propriedade da conexao: e o que o
                    // `login` amarra a credencial quando o cliente pede
                    // `amarrar_canal`. Ver `docs/CIFRA-DO-FIO.md` §10.
                    sessao.transcricao_do_fio = canal.transcricao();
                    continue;
                }
                if self.config.cifra_fio.exigir && !saida.cifrado() {
                    self.recusar_texto_claro(&mut saida, &ip, porta);
                    return;
                }
            }

            let inicio = Instant::now();
            let quando_ms = crate::agora_ms();
            // A atividade e aberta a cada pedido, e nao uma vez por conexao:
            // quem liga a telemetria no meio do expediente precisa ver as
            // conexoes que JA estavam abertas. `entrar` devolve a mesma
            // atividade quando a chave ja existe, entao a bolha nao pisca.
            let atividade =
                self.telemetria
                    .entrar(&chave_da_atividade, "dados", &ip, id_ligacao, quando_ms);
            // Amarra a atividade a ESTA thread: e por ela que os lacos longos
            // acham a marca de encerramento, la no fundo, sem carregar a
            // telemetria por parametro em dezenas de assinaturas.
            let _amarrada = crate::telemetria::amarrar(atividade.clone());
            {
                let alvo = objeto_do_pedido(&linha, &Ok(Json::Nulo));
                let nome_op = Json::analisar(&linha)
                    .ok()
                    .map(|j| j.texto_ou("op", "?").to_string())
                    .unwrap_or_else(|| "?".into());
                if let Some(a) = &atividade {
                    a.comecou_pedido(
                        &nome_op,
                        sessao.login(),
                        &alvo.database,
                        &alvo.tabela,
                        quando_ms,
                    );
                }
                if let Ok(mut l) = self.ligacoes.lock() {
                    l.comecou(
                        id_ligacao,
                        &nome_op,
                        sessao.login(),
                        &alvo.database,
                        &alvo.tabela,
                        quando_ms,
                    );
                }
            }
            // O PROFILER olha AQUI: o pedido chegou pelo soquete e nada foi
            // gravado ainda. Se a operacao travar, ele ja apareceu na tela
            // como «em curso» -- que e justamente o pedido que se quer achar.
            // Desligado, nao custa NADA: nem parse, nem alocacao, nem trava.
            let marca = if self.profiler_ligado.load(Ordering::Relaxed) {
                let alvo = objeto_do_pedido(&linha, &Ok(Json::Nulo));
                let nome_op = Json::analisar(&linha)
                    .ok()
                    .map(|j| j.texto_ou("op", "?").to_string())
                    .unwrap_or_else(|| "?".into());
                self.profiler.lock().ok().and_then(|mut p| {
                    p.chegou(
                        &linha,
                        &nome_op,
                        sessao.login(),
                        &alvo.database,
                        &alvo.tabela,
                        &ip,
                        quando_ms,
                    )
                })
            } else {
                None
            };

            let (op, autenticado, resultado) = self.despachar(&linha, &mut sessao, &ip);
            let duracao = inicio.elapsed().as_millis() as u64;
            if let Some(serial) = marca {
                if let Ok(mut p) = self.profiler.lock() {
                    p.terminou(
                        serial,
                        duracao,
                        resultado.is_ok(),
                        &resultado
                            .as_ref()
                            .err()
                            .map(|e| e.to_string())
                            .unwrap_or_default(),
                    );
                }
            }
            // O login so se sabe DEPOIS: o pedido que autentica e o proprio
            // `login`, e antes dele a sessao ainda esta anonima.
            if let Ok(mut l) = self.ligacoes.lock() {
                l.terminou(id_ligacao, sessao.login());
            }
            if let Some(a) = &atividade {
                a.terminou_pedido(sessao.login());
            }
            self.telemetria
                .contar_pedido(OPS_ESCRITA.contains(&op.as_str()), resultado.is_ok());

            let resposta = match &resultado {
                Ok(valor) => Json::objeto(vec![
                    ("ok", Json::Bool(true)),
                    ("op", Json::texto_de(&op)),
                    ("resultado", valor.clone()),
                    ("ms", Json::de_u64(duracao)),
                ]),
                Err(e) => self.resposta_erro(&op, e, duracao),
            };

            self.anotar(&Acesso {
                quando_ms,
                ip: ip.clone(),
                porta_origem: porta,
                op: op.clone(),
                usuario: sessao.login().to_string(),
                autenticado,
                ok: resultado.is_ok(),
                duracao_ms: duracao,
                erro: resultado.as_ref().err().map(|e| e.to_string()),
                // O objeto do pedido, para o log poder somar por tabela.
                ..objeto_do_pedido(&linha, &resultado)
            });

            let texto_da_resposta = self.talvez_comprimir(&resposta.escrever(), &linha, &canal);
            if canal.escrever(&mut saida, &texto_da_resposta).is_err() {
                return;
            }
        }
    }

    /// A linha e o pedido de aperto? Devolve o pedido ja analisado.
    ///
    /// ANALISA em vez de recortar, pela mesma regra que o Profiler pagou: quem
    /// procura `"op":"cifrar"` com um `find` depende de o cliente ter escrito
    /// o JSON de um jeito, e erra nos dois sentidos.
    fn pedido_de_aperto(linha: &str) -> Option<Json> {
        let p = Json::analisar(linha).ok()?;
        if p.texto_ou("op", "").trim() == "cifrar" {
            Some(p)
        } else {
            None
        }
    }

    /// Comprime a resposta quando o pedido pediu, valeu a pena e o canal
    /// deixa -- e devolve a linha de sempre em qualquer outro caso.
    ///
    /// # Pedida, nao imposta
    ///
    /// A mesma regra da versao otimista (`conferir_versao_pedida`, logo
    /// abaixo): quem manda `"aceita_compressao":true` no PEDIDO ganha a
    /// resposta comprimida a partir dali; quem nunca ouviu falar disto
    /// continua recebendo a linha de sempre, byte a byte. E por pedido, e nao
    /// por conexao ou por um `op` de aperto -- do jeito que o `Accept-Encoding`
    /// do HTTP funciona: cada lado escolhe a cada troca, sem guardar estado
    /// novo na `Sessao` nem exigir uma segunda mensagem so para negociar.
    ///
    /// # Por que nunca dentro do tunel cifrado
    ///
    /// Comprimir e depois cifrar a MESMA resposta (compress-then-encrypt)
    /// vaza tamanho: se um atacante consegue influenciar parte do conteudo
    /// (um campo de busca ecoado na resposta, por exemplo) e observa o
    /// tamanho da linha cifrada, o tamanho encolhe quando o trecho dele
    /// repete um segredo que tambem esta na resposta -- e o estilo do ataque
    /// CRIME/BREACH contra TLS. Decidir SE vale mitigar isso aqui dentro (por
    /// exemplo com padding) e escolha de seguranca, e fica fora desta frente
    /// -- ver `docs/CIFRA-DO-FIO.md`. A regra desta funcao e simples e nao
    /// negocia: canal cifrado nunca comprime, ponto -- ainda que o pedido
    /// tenha marcado `aceita_compressao`.
    ///
    /// # O enquadramento
    ///
    /// O DEFLATE nao produz texto (pode conter qualquer byte, inclusive um
    /// `\n` no meio), e o canal em claro le por LINHA -- por isso o Base64,
    /// exatamente como a cifra do fio ja faz em `Transporte::selar`. A marca
    /// de "isto esta comprimido" e o proprio envelope: em vez do `{"ok":...}`
    /// de sempre, a linha vira `{"cz":"<base64 do deflate>"}` -- um cliente
    /// que decodifica sabe que "cz" e o unico campo que uma resposta normal
    /// nunca tem, decodifica o Base64, descomprime e analisa o JSON de dentro
    /// como se tivesse chegado direto.
    fn talvez_comprimir(&self, texto: &str, pedido_bruto: &str, canal: &Canal) -> String {
        // Regra 1, sem excecao: dentro do tunel nao se comprime.
        if canal.cifrado() {
            return texto.to_string();
        }
        // Regra 2: abaixo do limiar, o envelope so pesaria mais.
        if texto.len() < LIMIAR_COMPRESSAO_BYTES {
            return texto.to_string();
        }
        // Regra 3: pedida, nao imposta -- analisa o PEDIDO original, nunca a
        // resposta, pela mesma razao do `pedido_de_aperto` acima: um `find`
        // por `"aceita_compressao"` dependeria de o cliente ter escrito o
        // campo naquela posicao exata, e erraria nos dois sentidos.
        let pediu = Json::analisar(pedido_bruto)
            .map(|p| p.booleano_ou("aceita_compressao", false))
            .unwrap_or(false);
        if !pediu {
            return texto.to_string();
        }
        let comprimido = phxsql_core::zip::deflate(texto.as_bytes());
        let em_base64 = phxsql_core::base64::codificar(&comprimido);
        let envelope = Json::objeto(vec![("cz", Json::texto_de(em_base64))]).escrever();
        // Confere o resultado FINAL contra o original, e nao so contra o
        // limiar de entrada: o limiar e uma estimativa, o tamanho do
        // envelope e um fato. Um JSON de alta entropia (textos ja
        // aleatorios, por exemplo) pode nao comprimir o bastante para pagar
        // o Base64 -- e mandar a linha de sempre nesse caso e estritamente
        // melhor que mandar uma "otimizacao" que pesa mais.
        if envelope.len() < texto.len() {
            envelope
        } else {
            texto.to_string()
        }
    }

    /// A privada estatica deste servidor, criada na primeira vez que se pede.
    pub(super) fn estatica_do_fio(&self) -> Result<[u8; 32]> {
        let mut guarda = self
            .estatica_do_fio
            .lock()
            .map_err(|_| PhxError::Corrompido("trava da chave do fio envenenada".into()))?;
        if let Some(k) = *guarda {
            return Ok(k);
        }
        let (k, avisos) = self
            .config
            .cifra_fio
            .estatica(self.config.caminho.as_deref())?;
        for a in avisos {
            eprintln!("aviso: {a}");
        }
        *guarda = Some(k);
        Ok(k)
    }

    /// Responde o aperto e troca o canal. `false` = feche a conexao.
    ///
    /// Aperto que falha FECHA a conexao em vez de continuar em claro: o
    /// cliente pediu tunel, e devolver-lhe uma sessao em claro depois de o
    /// aperto ter falhado seria exatamente o rebaixamento silencioso que a
    /// secao 2 do `docs/CIFRA-DO-FIO.md` recusa.
    fn responder_aperto(
        &self,
        pedido: &Json,
        canal: &mut Canal,
        saida: &mut crate::fio_dados::Escrita,
        ip: &str,
        porta: u16,
    ) -> bool {
        let inicio = Instant::now();
        let quando_ms = crate::agora_ms();
        let feito = self.aperto(pedido);
        let duracao = inicio.elapsed().as_millis() as u64;

        let resposta = match &feito {
            Ok((_, m2)) => Json::objeto(vec![
                ("ok", Json::Bool(true)),
                ("op", Json::texto_de("cifrar")),
                (
                    "resultado",
                    Json::objeto(vec![("m2", Json::texto_de(m2.clone()))]),
                ),
                ("ms", Json::de_u64(duracao)),
            ]),
            Err(e) => self.resposta_erro("cifrar", e, duracao),
        };
        self.anotar(&Acesso {
            quando_ms,
            ip: ip.to_string(),
            porta_origem: porta,
            op: "cifrar".into(),
            usuario: String::new(),
            autenticado: false,
            ok: feito.is_ok(),
            duracao_ms: duracao,
            erro: feito.as_ref().err().map(|e| e.to_string()),
            database: String::new(),
            tabela: String::new(),
            codigo: 0,
        });
        // A resposta 2 vai EM CLARO -- ela e o aperto, nao o conteudo dele.
        if writeln!(saida, "{}", resposta.escrever()).is_err() {
            return false;
        }
        let _ = saida.flush();
        match feito {
            Ok((transporte, _)) => {
                *canal = Canal::Cifrado(Box::new(transporte));
                // Pedido 652 (decisao do dono, 01/10/2026): na 0.19 o Noise
                // ainda entra, mas quem chega por ele e registrado. E este o
                // ponto UNICO onde o servidor aceita um aperto Noise; o TLS
                // decide-se antes, no primeiro byte (`abrir_fio_de_dados`), e
                // nunca passa por aqui. Noise DENTRO de um TLS ja e o canal
                // novo, e nao se avisa.
                if !saida.cifrado() {
                    self.avisar_que_o_noise_acaba(ip);
                }
                true
            }
            Err(_) => false,
        }
    }

    /// Uma linha de log por PAR que ainda chegou por Noise (a porta e efemera,
    /// entao o par e o endereco). O iniciador e o no da lista do cluster que
    /// tem esse endereco, ou `cliente`: no aperto NX quem inicia ainda nao se
    /// identificou, e o login so existe depois dele.
    fn avisar_que_o_noise_acaba(&self, ip: &str) {
        if !self.aviso_do_noise.avisar(ip, crate::agora_ms()) {
            return;
        }
        let iniciador = self
            .cluster
            .as_ref()
            .and_then(|c| {
                c.lista()
                    .into_iter()
                    .find(|n| mesmo_endereco_ip(&n.endereco, ip))
                    .map(|n| format!("no {}", n.id))
            })
            .unwrap_or_else(|| "cliente".to_string());
        eprintln!("{}", crate::fio_dados::linha_do_noise(ip, &iniciador));
    }

    /// O aperto em si: mensagem 1 em Base64 entra, mensagem 2 em Base64 sai.
    fn aperto(&self, pedido: &Json) -> Result<(phxsql_core::fio::Transporte, String)> {
        if !self.config.cifra_fio.ligada {
            return Err(PhxError::Autorizacao(
                self.msg("erro.cifra_do_fio_desligada", &[]),
            ));
        }
        let m1 = phxsql_core::base64::decodificar(pedido.texto_ou("e", ""))?;
        let (transporte, m2) = phxsql_core::fio::responder(&self.estatica_do_fio()?, &m1)?;
        Ok((transporte, phxsql_core::base64::codificar(&m2)))
    }

    /// A recusa de quem falou em claro com `cifra_fio.exigir` ligado.
    ///
    /// Sai como uma linha JSON comum, em claro, com erro nomeado: cliente velho
    /// recebe algo que ele SABE exibir, em vez de um silencio ou de uma
    /// conexao que morre sem motivo. O estrago de ligar isto por engano tem de
    /// ser visivel no primeiro pedido.
    fn recusar_texto_claro(&self, saida: &mut crate::fio_dados::Escrita, ip: &str, porta: u16) {
        let erro = PhxError::Autorizacao(self.msg("erro.cifra_do_fio_exigida", &[]));
        self.anotar(&Acesso {
            quando_ms: crate::agora_ms(),
            ip: ip.to_string(),
            porta_origem: porta,
            op: "cifrar".into(),
            usuario: String::new(),
            autenticado: false,
            ok: false,
            duracao_ms: 0,
            erro: Some(erro.to_string()),
            database: String::new(),
            tabela: String::new(),
            codigo: 0,
        });
        let _ = writeln!(
            saida,
            "{}",
            self.resposta_erro("cifrar", &erro, 0).escrever()
        );
        let _ = saida.flush();
    }

    /// Esta sessao ainda nao provou quem e, num servidor que EXIGE prova.
    ///
    /// UMA funcao para os dois lugares que fazem a mesma pergunta -- o portao
    /// do login, no `despachar`, e o teto da linha, no laco da conexao. Duas
    /// copias da mesma condicao divergiriam no dia em que uma das duas
    /// ganhasse um caso novo, e a que ficasse para tras seria a porta dos
    /// fundos: uma sessao tratada como anonima por um lado e como identificada
    /// pelo outro.
    ///
    /// O `cadastro().vazio()` nao e detalhe: servidor sem usuario nenhum nao
    /// tem credencial a esperar, e quem chega nele ja pode tudo. Chamar de
    /// anonima uma sessao ali seria tratar o cliente legitimo como suspeito.
    pub(super) fn ainda_anonima(&self, sessao: &Sessao) -> bool {
        // O barato primeiro: sessao com usuario nem toca a trava do cadastro.
        sessao.usuario.is_none() && !self.cadastro().vazio()
    }

    /// Quanto este lado reserva numa linha da porta de dados -- pedido 434.
    ///
    /// # A pergunta e «quanto eu reservo antes de saber quem e?»
    ///
    /// E antes do login a resposta nao pode ser 128 MiB. Com
    /// `conexoes_max` nascendo em 64, sessenta e quatro soquetes mandando
    /// bytes sem `\n` reservam 8 GiB antes de qualquer credencial existir --
    /// e nenhum deles precisou de senha para isso.
    ///
    /// # Por que o teto pequeno e o do APERTO, e nao um valor novo
    ///
    /// Porque e a mesma pergunta, ja respondida: o `TETO_DO_APERTO` existe
    /// desde o pedido 312 para a linha lida antes de o outro lado se
    /// identificar. Uma constante nova ao lado dela seria a mesma decisao
    /// escrita duas vezes, e um dia as duas divergiriam.
    ///
    /// # Por que 64 KiB nao recusa nenhum cliente legitimo
    ///
    /// Porque anonimo, com cadastro, so ha DEZESSEIS operacoes legais -- as
    /// que `Atividade::da_operacao` devolve `None`; toda outra cai no «faca
    /// login». Seis de sessao (`ping`, `login`, `desafio`, `quem_sou`, `sair`,
    /// `catalogo`) e dez de controle de transacao (`begin` e os apelidos,
    /// `commit`, `rollback`, os de `savepoint`, `transacao`). Esta linha
    /// dizia «seis» (revisao SEC, B5); a lista viva e o teste
    /// `usuarios::tests::as_operacoes_anonimas_sao_estas_dezesseis`, tirada do
    /// catalogo. Nenhuma das dez carrega dado -- no maximo um nome de
    /// savepoint --, e a maior das dezesseis continua sendo um `login` com
    /// token e prova, que nao chega a mil bytes. Sessenta e quatro vezes de
    /// folga.
    ///
    /// # Os dois escapes, e por que cada um existe
    ///
    /// * **sessao identificada**: teto do registro, como sempre foi. Quem
    ///   provou quem e tem direito ao lote de 128 MiB.
    /// * **servidor sem cadastro**: teto do registro tambem. Ali nao ha
    ///   credencial a esperar, e apertar quebraria o `inserir` em lote de todo
    ///   cliente que nunca criou usuario -- proteger o dado de ninguem ao
    ///   preco de derrubar quem ja trabalha e o estrago do pedido 203, nao uma
    ///   protecao.
    ///
    /// E o escape do cadastro vazio NAO vale quando a cifra e exigida e o
    /// tunel ainda nao existe: ali a unica linha que o servidor aceitaria e o
    /// proprio aperto de mao, e toda outra cai no `recusar_texto_claro` --
    /// reservar 128 MiB para recusar em seguida seria reservar por nada.
    ///
    /// # QUANDO se pergunta -- pedido 442
    ///
    /// So quando a linha ja passou do `TETO_DO_APERTO`, e com a ficha
    /// refrescada naquele instante (`Canal::ler_decidindo`, no laco do
    /// `atender`). Perguntada antes de a leitura bloquear, a resposta valia
    /// pela vida inteira da espera -- e o usuario excluido enquanto a conexao
    /// dele esperava continuava com os 128 MiB de quem ele era. Os dois
    /// lugares que fazem esta pergunta (`ainda_anonima`, aqui e no portao do
    /// login) passam a faze-la sobre a MESMA ficha: a do cadastro vivo.
    fn teto_da_linha(&self, sessao: &Sessao, cifrado: bool) -> u64 {
        if sessao.usuario.is_some() {
            return TETO_DO_REGISTRO;
        }
        if self.config.cifra_fio.exigir && !cifrado {
            return TETO_DO_APERTO;
        }
        if self.ainda_anonima(sessao) {
            return TETO_DO_APERTO;
        }
        TETO_DO_REGISTRO
    }

    /// A recusa do portao 3, num lugar so.
    ///
    /// Quem confere direito fora do portao -- o `SCOPE` do `begin`, pedido
    /// 607 -- recusa com ESTA frase, e nao com uma propria: duas redacoes da
    /// mesma recusa divergiriam no dia em que uma ganhasse traducao, e a
    /// diferenca entre elas diria a quem pergunta por qual caminho caiu.
    pub(super) fn recusa_sem_direito(
        &self,
        usuario: &Usuario,
        atividade: Atividade,
        base: &str,
        tabela: &str,
    ) -> PhxError {
        PhxError::Autorizacao(self.msg(
            "erro.sem_direito",
            &[
                ("login", usuario.login.as_str()),
                ("atividade", atividade.nome()),
                (
                    "alvo",
                    &match (base.is_empty(), tabela.is_empty()) {
                        (true, _) => "(sem base)".to_string(),
                        (false, true) => base.to_string(),
                        (false, false) => format!("{base}.{tabela}"),
                    },
                ),
            ],
        ))
    }

    /// Abre um desafio: devolve sal, iteracoes e um nonce de uso unico.
    ///
    /// Usuario que nao existe recebe um desafio de aparencia normal, com sal
    /// derivado do proprio login e as iteracoes de um usuario novo. Quem NAO
    /// tem o token nao distingue; quem tem, distingue pelo sal -- ver o
    /// comentario do sal falso abaixo, que diz por que e o que falta.
    pub(super) fn op_desafio(&self, p: &Json, sessao: &mut Sessao) -> Result<Json> {
        let login = p
            .texto_ou("usuario", p.texto_ou("login", ""))
            .trim()
            .to_string();
        if login.is_empty() {
            return Err(PhxError::Esquema("informe \"usuario\"".into()));
        }
        // Os dois caminhos fazem as MESMAS contas, na mesma ordem -- o irmao
        // do pedido 520 que mora aqui. Antes, so quem nao existe pagava o
        // HMAC do sal falso, e so quem existe destrinchava um hash: medido em
        // debug pela porta de dados, n = 2.000 intercalados em duas rodadas e
        // login do mesmo tamanho, +9 us na mediana para quem nao existe, ~8%
        // do pedido; depois, 0,0-0,2 us -- com TRES usuarios. Com cadastro
        // grande o relogio volta pela varredura linear do `por_login` (+363 us
        // com 20.000, medido pelo SEC; `docs/SEGURANCA.md` §26.2), que este
        // conserto nao alcanca. Agora os dois pagam o HMAC e os dois
        // destrincham um hash -- o de verdade ou o `senha::hash_de_fachada`,
        // que e o mesmo usuario de mentira do login e ja carrega as
        // `ITERACOES_PADRAO`.
        let cadastro = self.cadastro();
        let achado = cadastro.por_login(&login);
        // Sal falso, estavel por login. A chave do HMAC era o TOKEN, que todo
        // cliente tem: quem o tinha recalculava e comparava, e sabia quem nao
        // existe num pedido so (6 de 6 na frente do 520, `SEGURANCA.md`
        // §26.6). Desde o pedido 528 a chave e o segredo do servidor, que o
        // token nao abre -- ver `config::Desafio`.
        let falso = phxsql_core::hash::hmac_sha256(&self.segredo_do_desafio, login.as_bytes());
        let guardado = achado.map_or(phxsql_core::senha::hash_de_fachada(), |u| {
            u.senha_hash.as_str()
        });
        let (sal, iteracoes) = phxsql_core::senha::sal_e_iteracoes(guardado)?;
        let sal: &[u8] = if achado.is_some() { &sal } else { &falso[..16] };
        let sal_hex = phxsql_core::hash::para_hex(sal);

        let nonce = phxsql_core::desafio::nonce();
        sessao.desafio = Some((
            login,
            nonce.clone(),
            crate::agora_ms() + phxsql_core::desafio::VALIDADE_MS,
        ));
        Ok(Json::objeto(vec![
            ("sal", Json::texto_de(sal_hex)),
            ("iteracoes", Json::de_u64(iteracoes as u64)),
            ("nonce", Json::texto_de(nonce)),
            (
                "validade_ms",
                Json::de_i64(phxsql_core::desafio::VALIDADE_MS),
            ),
        ]))
    }

    /// Confere a credencial e guarda a identidade na conexao.
    ///
    /// Aceita tres formas, da mais segura para a menos:
    ///
    /// 1. `prova` + `nonce_cliente` -- desafio-resposta. A senha nao sai da
    ///    maquina do cliente.
    /// 2. `senha_b64` -- Base64. Some do grep e do olho, mas quem captura o
    ///    pacote decodifica: NAO e cifra.
    /// 3. `senha` -- texto puro.
    pub(super) fn op_login(&self, p: &Json, sessao: &mut Sessao) -> Result<Json> {
        let login = match p.campo("usuario_b64").and_then(Json::texto) {
            Some(b) => phxsql_core::base64::decodificar_texto(b)?,
            None => p.texto_ou("usuario", p.texto_ou("login", "")).to_string(),
        };
        let login = login.trim().to_string();
        if login.is_empty() {
            return Err(PhxError::Esquema("informe \"usuario\" e \"senha\"".into()));
        }

        // Todo caminho de erro devolve a MESMA mensagem, para nao dizer se o
        // que falhou foi o login, a senha ou o desafio.
        let recusa = || PhxError::Autorizacao(self.msg("erro.credencial_invalida", &[]));

        // Antes do cadastro e do PBKDF2: e politica do fio, nao «senha
        // errada», e por isso nao diz nada sobre o usuario (pedido 667).
        self.conferir_o_fio_da_senha(p, sessao)?;

        // Amarracao da credencial ao canal (channel binding), o gap da §10 da
        // `docs/CIFRA-DO-FIO.md`. Quem pede `amarrar_canal` prende a prova a
        // transcricao DESTE tunel; o servidor a confere contra a SUA, e as
        // duas so coincidem se nao ha ninguem no meio que tenha terminado o
        // tunel. A transcricao usada e a da conexao (`sessao`), NUNCA uma que
        // venha no pedido -- deixar o cliente escolher a transcricao seria
        // devolver ao atacante exatamente o que a amarracao tira dele.
        //
        // Sem o campo, `canal_ref` e `None` e a prova e a de sempre: a porta
        // web (HTTP, sem tunel) e o cliente velho nao mudam -- pedida, nao
        // imposta. Com o campo mas sem tunel, a recusa manda abrir o aperto,
        // em vez de amarrar a credencial a coisa nenhuma.
        let amarrar = p
            .campo("amarrar_canal")
            .and_then(Json::booleano)
            .unwrap_or(false);

        // EXIGIR a amarracao ao canal, o degrau seguinte da §10: fecha o gap de
        // a amarracao ser so PEDIDA. Um atacante ativo que terminou o tunel do
        // cliente pode cortar `amarrar_canal` antes de reencaminhar -- a mesma
        // aritmetica do rebaixamento do `exigir`. Contra ele so vale o servidor
        // exigir, e essa e decisao de quem implanta (`cifra_fio.exigir_amarra`).
        //
        // So morde quando HA tunel: em claro nao ha transcricao a que amarrar,
        // e a conexao em claro segue exatamente como antes. Nasce desligada --
        // com `exigir_amarra` off, o comportamento e byte a byte o de hoje
        // (guarda nova entra pedida, nao imposta). A recusa e NOMEADA e vem
        // ANTES de olhar a credencial: e politica, nao "senha errada", entao
        // nada vaza sobre o usuario.
        if self.config.cifra_fio.exigir_amarra && sessao.transcricao_do_fio.is_some() && !amarrar {
            return Err(PhxError::Autorizacao(self.msg("erro.amarra_exigida", &[])));
        }

        let canal_amarrado: Option<[u8; 32]> = if amarrar {
            match sessao.transcricao_do_fio {
                Some(t) => Some(t),
                None => {
                    return Err(PhxError::Autorizacao(
                        self.msg("erro.amarra_sem_tunel", &[]),
                    ))
                }
            }
        } else {
            None
        };
        let canal_ref = canal_amarrado.as_ref().map(|t| &t[..]);

        let mut nonces: Option<(String, String)> = None;
        let autenticado = if let Some(prova) = p.campo("prova").and_then(Json::texto) {
            // (1) desafio-resposta
            let (usuario_desafio, nonce, expira) = sessao.desafio.take().ok_or_else(|| {
                PhxError::Autorizacao("peca um desafio antes de mandar a prova".into())
            })?;
            if crate::agora_ms() > expira {
                return Err(PhxError::Autorizacao(
                    "o desafio expirou; peca outro".into(),
                ));
            }
            if usuario_desafio != login {
                return Err(recusa());
            }
            let nonce_cliente = p.texto_ou("nonce_cliente", "");
            nonces = Some((nonce.clone(), nonce_cliente.to_string()));
            // O irmao do pedido 520 neste ramo: quem nao existe e o inativo
            // saiam sem conferir prova nenhuma, e quem existe conferia. Aqui
            // nao ha PBKDF2 (o cliente ja derivou), e a diferenca era de
            // microssegundos -- 24 us em debug, ~15% do pedido --, mas era a
            // mesma pergunta respondida pelo relogio. Os tres agora fazem as
            // mesmas contas, na mesma ordem, pelo mesmo usuario de mentira do
            // `Cadastro::autenticar`.
            let cadastro = self.cadastro();
            let achado = cadastro.por_login(&login);
            let guardado = achado.map_or(phxsql_core::senha::hash_de_fachada(), |u| {
                u.senha_hash.as_str()
            });
            let dk = phxsql_core::senha::derivado_do_hash(guardado)?;
            let confere = phxsql_core::desafio::conferir_prova(
                &dk,
                &nonce,
                nonce_cliente,
                &login,
                canal_ref,
                prova,
            );
            achado.filter(|u| confere && u.ativo).cloned()
        } else {
            // (2) Base64 ou (3) texto puro
            let clara = match p.campo("senha_b64").and_then(Json::texto) {
                Some(b) => phxsql_core::base64::decodificar_texto(b)?,
                None => p.texto_ou("senha", "").to_string(),
            };
            // O teto ANTES do cadastro e do PBKDF2 (pedido 521). Antes de
            // olhar o login, para a recusa nao depender de quem existe; e
            // nomeada, porque o teto e publico e nao ha o que esconder nele.
            // A porta web aceita 4 MiB de corpo antes da credencial: sem isto,
            // era ali que uma senha grande virava horas de conta.
            phxsql_core::senha::caber_no_teto(&clara)?;
            let cadastro = self.cadastro();
            cadastro.autenticar(&login, &clara).cloned()
        };

        // Segundo fator: quem tem chave publica no config.json tambem assina.
        //
        // A mensagem assinada e a MESMA do desafio-resposta -- os dois nonces
        // e o login --, entao a assinatura tambem vale uma vez so. Nao ha
        // atalho: sem desafio aberto nao ha o que assinar.
        if let Some(u) = &autenticado {
            if let Some(publica) = &u.chave_publica {
                let (nonce, nonce_cliente) =
                    match &nonces {
                        Some(par) => par.clone(),
                        None => return Err(PhxError::Autorizacao(
                            "este usuario exige chave: peca um desafio e mande a prova assinada"
                                .into(),
                        )),
                    };
                let hex = p.texto_ou("assinatura", "");
                let assinatura = phxsql_core::ed25519::assinatura_de_hex(hex).ok_or_else(|| {
                    PhxError::Autorizacao(
                        "este usuario exige \"assinatura\" com 128 hexadecimais".into(),
                    )
                })?;
                let mensagem = phxsql_core::desafio::mensagem_assinada(
                    &nonce,
                    &nonce_cliente,
                    &login,
                    canal_ref,
                );
                if !phxsql_core::ed25519::conferir(publica, &mensagem, &assinatura) {
                    return Err(recusa());
                }
            }
        }

        match autenticado {
            Some(u) => {
                // O TETO de usuarios SIMULTANEOS, aplicado no unico ponto por
                // onde alguem passa a existir para o servidor.
                //
                // `recursos.usuarios_max` estava no config.json, no MANUAL e
                // na tela e NENHUMA linha o lia -- a mesma armadilha do
                // `cache_paginas` sem cache. Zero continua sendo SEM TETO, que
                // e o padrao: quem nunca preencheu o campo nao ve diferenca.
                //
                // Conta LOGIN, e nao conexao: a mesma pessoa em tres abas
                // continua sendo uma, que e o que uma licenca por posto quer
                // contar. E quem JA esta dentro entra de novo sem gastar vaga.
                self.recusar_se_lotou(&u.login)?;
                let ficha = u.ficha();
                // A geracao do cadastro em que esta ficha foi tirada. Ver
                // `refrescar_a_sessao`.
                sessao.geracao_do_cadastro = self.cadastro_geracao.load(Ordering::Relaxed);
                sessao.usuario = Some(u);
                Ok(ficha)
            }
            None => {
                sessao.usuario = None;
                Err(recusa())
            }
        }
    }

    /// Este login cabe no teto de `recursos.usuarios_max`?
    ///
    /// Conta os logins distintos das conexoes vivas e das sessoes do
    /// navegador. Quem ja esta dentro nunca e barrado -- reconectar nao pode
    /// custar uma vaga que a propria pessoa ja ocupa.
    pub(super) fn recusar_se_lotou(&self, login: &str) -> Result<()> {
        let teto = self.config.recursos.usuarios_max;
        if teto == 0 {
            return Ok(());
        }
        let mut vivos: Vec<String> = match self.ligacoes.lock() {
            Ok(l) => l
                .todas()
                .into_iter()
                .map(|x| x.usuario)
                .filter(|u| !u.is_empty())
                .collect(),
            Err(_) => Vec::new(),
        };
        if let Ok(s) = self.sessoes.lock() {
            vivos.extend(s.logins_vivos(crate::agora_ms()));
        }
        vivos.sort();
        vivos.dedup();
        if vivos.iter().any(|u| u == login) || vivos.len() < teto {
            return Ok(());
        }
        Err(PhxError::Autorizacao(format!(
            "o servidor esta com {} usuario(s) diferentes conectados, que e o \
             teto de recursos.usuarios_max. Espere alguem sair, ou suba o teto",
            vivos.len()
        )))
    }
}

/// Joga fora o que sobrou de uma linha grande demais, ate a quebra ou ate o
/// teto -- e devolve quantos bytes descartou.
///
/// # Por que descartar em vez de fechar direto
///
/// Fechar um soquete com dado por ler no buffer de recepcao faz o nucleo
/// mandar RST, e o RST descarta o que o cliente ainda nao leu -- inclusive a
/// resposta de erro que se acabou de escrever. Sem esta drenagem o conserto do
/// pedido 216 "responderia" no codigo e continuaria invisivel no fio.
///
/// # Por que nao um `read_until`
///
/// Porque ele acumula num `Vec`, e acumular ate 128 MiB e exatamente a memoria
/// que o teto existe para nao reservar. Aqui o buffer e o do proprio
/// `BufReader`, e o laco so anda o cursor.
pub(super) fn descartar_ate_a_quebra<L: BufRead>(leitor: &mut L, teto: u64) -> u64 {
    let mut descartados = 0u64;
    while descartados < teto {
        let (achou, quantos) = match leitor.fill_buf() {
            Ok([]) | Err(_) => return descartados,
            Ok(bloco) => match bloco.iter().position(|c| *c == b'\n') {
                Some(i) => (true, i + 1),
                None => (false, bloco.len()),
            },
        };
        leitor.consume(quantos);
        descartados += quantos as u64;
        if achou {
            break;
        }
    }
    descartados
}
