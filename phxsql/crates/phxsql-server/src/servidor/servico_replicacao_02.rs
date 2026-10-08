//! Replicacao, segunda parte: as operacoes da replica (`replicar`,
//! `aplicar`, estado, ligar, pular) e a linhagem.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// A chave da fabrica para uma falha de REDE da sonda `replicacao_testar`,
/// pelo tipo do erro e nunca pela frase do sistema operacional.
pub(super) fn chave_da_falha_de_rede(tipo: std::io::ErrorKind) -> &'static str {
    use std::io::ErrorKind::*;
    match tipo {
        ConnectionRefused => "erro.sonda_recusada",
        TimedOut | WouldBlock => "erro.sonda_prazo",
        // Sem rota, host inalcancavel, nome que nao resolve, endereco que nao
        // existe: para quem sonda, e tudo «nao ha caminho ate la».
        HostUnreachable | NetworkUnreachable | NetworkDown | AddrNotAvailable | NotFound
        | Unsupported | InvalidInput => "erro.sonda_sem_rota",
        // Aceitou e derrubou: reset, EOF, cano quebrado -- ou qualquer outro
        // tipo que o sistema invente. Nunca o texto dele.
        _ => "erro.sonda_caiu",
    }
}

/// Quantos bytes de IMAGEM cabem numa resposta de `replicar`.
///
/// # Por que um teto em bytes, e nao so em eventos
///
/// O `max` do pedido conta EVENTOS, e evento nao tem tamanho fixo: 500 linhas
/// de 60 bytes sao 30 KiB, e 500 linhas com um memo de 200 KiB sao 100 MiB --
/// que, em hexadecimal, viram 200 MiB de texto montados de uma vez dos dois
/// lados. Contar so eventos e deixar o tamanho da resposta nas maos de quem
/// escreveu a linha mais gorda.
///
/// Um lote curto nao perde nada: `ate` e `fim` saem do que foi realmente
/// lido, entao a replica so pergunta de novo. E o lote nunca sai VAZIO por
/// causa do teto -- o primeiro evento entra sempre, custe o que custar, senao
/// uma linha maior que o teto pararia a replicacao para sempre em vez de
/// atrasa-la.
pub(super) const TETO_DO_LOTE_SERVIDO: usize = 16 * 1024 * 1024;

/// O `max` de um `replicar`, ja saneado: ausente, zero ou negativo vale o
/// padrao; acima do teto vale o teto. E a mesma forma do `limite` das
/// leituras de cliente, e por isso mora ao lado dos tetos que a explicam.
pub(crate) fn lote_de_replicacao(p: &Json) -> u64 {
    match p.inteiro_ou("max", 0) {
        n if n <= 0 => LOTE_PADRAO_DE_REPLICACAO,
        n => (n as u64).min(TETO_DE_EVENTOS_POR_LOTE),
    }
}

impl Servidor {
    /// `replicar`: os eventos a partir da posicao `desde`, com a imagem.
    ///
    /// A imagem vai em hexadecimal porque o transporte e JSON e JSON nao tem
    /// bytes. Dobra o tamanho -- e a alternativa seria acrescentar um formato
    /// binario ao protocolo, que e uma decisao maior do que esta.
    /// O fio desta sessao vai cifrado?
    ///
    /// # Por que a resposta depende da porta de entrada
    ///
    /// Porque cada porta tem uma cifra diferente, e duas delas nem sao deste
    /// servidor:
    ///
    /// - **dados**: e a unica onde mora o aperto de mao da §7. `None` na
    ///   transcricao quer dizer conexao em claro, e e o padrao.
    /// - **HTTP**: nunca ha tunel aqui -- o navegador fala TLS ou fala claro,
    ///   e o TLS e do proxy reverso. A unica garantia conferivel e
    ///   INDIRETA e vale so com `cifra_fio.exigir` ligado: com ela, o
    ///   `portao_de_rede_http` **ja recusou** tudo que nao veio por porta com
    ///   `atras_de_proxy` declarado, entao chegar ate aqui e a prova de que a
    ///   declaracao existe. Com `exigir` desligado nao ha prova nenhuma, e
    ///   supor o proxy seria a familia do `recursos.cache_paginas`: campo que
    ///   anuncia protecao maior que a prestada.
    /// - **sem fio**: job agendado, rotina interna, ponte MCP pelo cano do
    ///   processo. Nao ha fio por onde vazar, e responder "em claro" faria a
    ///   rotina interna recusar a si mesma.
    pub(super) fn fio_cifrado(&self, sessao: &Sessao) -> bool {
        match sessao.entrada {
            Entrada::Dados => sessao.transcricao_do_fio.is_some() || sessao.fio_tls,
            Entrada::Http => sessao.fio_tls || self.config.cifra_fio.exigir,
            Entrada::SemFio => true,
        }
    }

    /// A SENHA pode atravessar o fio desta sessao? (pedido 667)
    ///
    /// So quando o pedido traz `senha`/`senha_b64` -- o desafio-resposta
    /// nao leva a senha e passa sempre. Passa quando o fio e cifrado (a
    /// MESMA pergunta do dado pessoal, pelo mesmo [`Self::fio_cifrado`]:
    /// duas ideias de «cifrado» divergiriam no primeiro conserto), quando a
    /// origem e o loopback (a senha nao sai da maquina), ou com o escape
    /// escrito `cifra_fio.senha_em_claro_pela_rede`.
    ///
    /// UM portao para os dois caminhos que recebem a senha: o `op_login`
    /// (local, qualquer porta) e o login da web que vai para OUTRO servidor
    /// -- este nao passa pelo `op_login` daqui, e a senha ja atravessou o
    /// fio ate aqui do mesmo jeito.
    pub(super) fn conferir_o_fio_da_senha(&self, p: &Json, sessao: &Sessao) -> Result<()> {
        if p.campo("senha").is_none() && p.campo("senha_b64").is_none() {
            return Ok(());
        }
        let loopback = sessao
            .ip
            .trim()
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.to_canonical().is_loopback());
        if self.config.cifra_fio.senha_em_claro_pela_rede || loopback || self.fio_cifrado(sessao) {
            return Ok(());
        }
        Err(PhxError::Autorizacao(
            self.msg("erro.senha_em_claro_pela_rede", &[]),
        ))
    }

    /// Os eventos do diario de `t` a partir de `desde`, prontos para o fio --
    /// e quantos foram LIDOS (a posicao anda por todos, inclusive os que a
    /// supressao de origem tira da lista).
    ///
    /// UM motor para os dois caminhos que entregam o diario: o `replicar` de
    /// sempre (a replica puxa) e o lote do quorum (pedido 207, o commit
    /// materializa com a trava na mao e a replica leva pelo
    /// `replicar_aguardar`). Duas copias da marca do diario, do teto de bytes
    /// e da `imagem_para_o_fio` divergiriam no primeiro conserto -- a doenca
    /// do pedido 434. Os portoes que dependem de QUEM pede (a cifra do 342, o
    /// alcance do usuario) ficam com cada chamador, que e quem sabe.
    pub(super) fn eventos_para_o_fio(
        &self,
        t: &mut Table,
        chave: &str,
        desde: u64,
        max: u64,
        hash_para: Option<u16>,
    ) -> Result<(Vec<Json>, u64)> {
        // A dica de onde a leitura anterior desta tabela parou. Sem ela, o
        // `desde` faz o diario ser varrido desde o comeco a cada lote -- ver
        // `marcas_do_diario`. A maior que ainda cabe: a marca so serve para
        // uma posicao depois dela.
        t.definir_marca_do_diario(self.marca_do_diario_para(chave, desde));
        // O corte por BYTES acontece DENTRO da leitura, no store, que e quem
        // sabe o tamanho antes de alocar -- ver `TETO_DO_LOTE_SERVIDO`. Ate
        // 17/09/2026 ele era um `truncate` aqui, depois de o lote inteiro ja
        // estar na memoria com a trava na mao: o teto cortava a resposta e
        // nao a leitura, que e a licao do Profiler aplicada a teto em vez de
        // a interruptor (revisao SEC, A2).
        let eventos = t.diario_com_imagem_ate(desde, max, TETO_DO_LOTE_SERVIDO)?;
        if let Some(nova) = t.marca_do_diario() {
            self.guardar_marca_do_diario(chave.to_string(), desde, nova);
        }
        // A posicao anda por TODOS os lidos, inclusive os que a supressao de
        // origem vai tirar da lista: suprimir e nao mandar de volta, e nao
        // fingir que o evento nao existe -- a contagem do diario e uma so.
        let lidos = eventos.len() as u64;

        // `para` diz QUEM pede. Eventos cuja origem e o proprio destino nao
        // viajam: e a alteracao que ele mesmo mandou, e devolve-la fecharia o
        // laco infinito do bidirecional. A origem zero (escrita local) sai
        // traduzida para o hash DESTE servidor, para o outro lado guardar de
        // quem veio sem tabela de traducao nenhuma.
        let meu_hash = self.config.replicacao.numero();

        let mut lista: Vec<Json> = Vec::with_capacity(eventos.len());
        for (i, (e, imagem)) in eventos.into_iter().enumerate() {
            let origem = if e.origem == 0 { meu_hash } else { e.origem };
            if hash_para.is_some_and(|h| origem != 0 && origem == h) {
                continue;
            }
            // O externo marcado sai do diario SELADO com a chave deste
            // arquivo, e a replica nao tem como abri-lo: o sal e por arquivo
            // (pedido 344). Abre-se AQUI, na resposta, e so nela -- o portao
            // de cima ja garantiu o fio cifrado para tabela com coluna
            // marcada, e o `.log` continua selado. Tabela sem coluna externa
            // marcada devolve a mesma imagem, sem custo de cifra.
            // A recusa do evento pre-344 (pedido 603) diz QUAL evento: e a
            // posicao que o operador pula com `replicacao_pular`.
            let imagem = t.imagem_para_o_fio(&imagem).map_err(|e| {
                PhxError::Corrompido(format!("evento {} do diario: {e}", desde + i as u64))
            })?;
            let mut evento = Json::objeto(vec![
                ("operacao", Json::texto_de(e.operacao.nome())),
                // ONDE este evento mora no diario DAQUI. So o source sabe
                // dizer: quem puxa nao consegue contar, porque a supressao
                // logo acima tira eventos da lista e a posicao anda por
                // cima deles. E a posicao que `replicacao_pular` recebe --
                // o nosso equivalente do LSN do `SKIP` do PostgreSQL.
                ("posicao", Json::de_u64(desde + i as u64)),
                ("rowid", Json::de_u64(e.rowid)),
                ("versao", Json::de_u64(e.versao)),
                ("carimbo_ms", Json::Numero(e.carimbo as f64)),
                ("usuario", Json::de_u64(e.usuario as u64)),
                ("origem", Json::de_u64(origem as u64)),
                ("imagem", Json::texto_de(bytes_para_hex(&imagem))),
            ]);
            // O id de transacao (pedido 676), so quando ha: o evento de um
            // volume velho do `.log` vai sem o campo, que e «sem id».
            if e.tx != 0 {
                if let Json::Objeto(campos) = &mut evento {
                    campos.push(("tx".to_string(), crate::replica::tx_para_o_fio(e.tx)));
                }
            }
            lista.push(evento);
        }

        Ok((lista, lidos))
    }

    pub(super) fn op_replicar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let desde = p.inteiro_ou("desde", 0).max(0) as u64;
        let max = lote_de_replicacao(p);
        // `para` diz QUEM pede, e `para_numero` o numero de origem dele (pedido
        // 329). Quem pede sem o numero e de antes do campo: o numero dele e o
        // hash do id, que e o que ele de fato grava.
        //
        // A conferencia vem ANTES da trava e do diario: um par recusado nao
        // custa leitura nenhuma, e o grito sai no primeiro pedido.
        let hash_para = {
            let para = p.texto_ou("para", "").trim();
            if para.is_empty() {
                None
            } else {
                let atribuido = p.inteiro_ou("para_numero", 0).clamp(0, u16::MAX as i64) as u16;
                let numero = bidirecional::numero_do_servidor(atribuido, para);
                self.conferir_numero_do_par(numero, para)?;
                Some(numero)
            }
        };
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;

        // **A cifra do fio e EXIGIDA para tabela com coluna marcada** (pedido
        // 342, decisao do dono). O que este portao conserta e uma assimetria,
        // e nao uma falta: a coluna EXTERNA marcada ja viajava selada nesta
        // mesma imagem -- deliberadamente, com o comentario em
        // `conteudo_externo` dizendo por que --, e a INLINE marcada viajava em
        // claro ao lado dela, pelo mesmo cano e no mesmo evento. As duas
        // metades nunca foram desenhadas juntas.
        //
        // A outra saida -- a imagem levar a faixa marcada SELADA -- esta
        // bloqueada com numero: a replica nao tem chave compativel, porque o
        // sal e por arquivo (pedido 344, medido com a MESMA senha nos dois
        // lados). Entao o que fecha o furo e o CANAL, e nao a imagem -- e
        // desde o 344 a metade EXTERNA tambem passou a viajar aberta por este
        // mesmo canal (`imagem_para_o_fio`, logo abaixo), porque selada ela
        // nao replicava.
        //
        // **E esta guarda e IMPOSTA, nao pedida -- e isso e escolha.** A lei
        // da casa manda guarda nova nascer pedida, e o motivo dela e nao
        // quebrar cliente que ja funciona. Aqui quebrar e o ponto: quem
        // replica coluna marcada em claro hoje esta vazando dado pessoal no
        // fio, e um interruptor para continuar vazando seria a permissao
        // escrita de vazar. O ALCANCE e o que segura a lei: tabela sem coluna
        // marcada nao e tocada por linha nenhuma deste portao, e continua
        // replicando exatamente como antes.
        if t.tem_dado_pessoal() && !self.fio_cifrado(sessao) {
            return Err(PhxError::Autorizacao(self.msg(
                "erro.replicar_marcada_exige_cifra",
                &[("tabela", p.texto_ou("tabela", ""))],
            )));
        }
        let total = t.eventos()?;
        let chave = Self::chave_do_diario(p.texto_ou("database", ""), p.texto_ou("tabela", ""));
        let (lista, lidos) = self.eventos_para_o_fio(&mut t, &chave, desde, max, hash_para)?;

        // A trilha de dado pessoal, UM registro por lote (revisao SEC de
        // 17/09/2026, A8): a imagem viaja com o valor da coluna marcada
        // dentro, e ate aqui `replicar` era o unico caminho que entregava
        // todas as linhas de uma vez sem deixar rastro -- «quem viu o
        // prontuario do fulano?» nao tinha resposta para a replicacao. O
        // criterio guarda a faixa pedida e `linhas` conta os eventos que de
        // fato sairam (os suprimidos pela origem nao expuseram nada). Tabela
        // sem coluna marcada nao paga nem o `format!`: o portao esta dentro
        // de `trilhar_acesso`, antes do fecho.
        Self::trilhar_acesso(&mut t, 0, lista.len() as u64, || {
            format!("replicar desde={desde} ate={}", desde + lidos)
        })?;

        Ok(Json::objeto(vec![
            ("desde", Json::de_u64(desde)),
            ("ate", Json::de_u64(desde + lidos)),
            ("total", Json::de_u64(total)),
            // `fim` verdadeiro quer dizer "por enquanto acabou": a replica
            // espera e pergunta de novo, em vez de girar em falso.
            ("fim", Json::Bool(desde + lidos >= total)),
            // A historia desta tabela (pedido 601): quem empurra estes eventos
            // pelo `aplicar` a leva junto, e o destino de outra historia recusa.
            (
                "linhagem",
                match t.esquema().linhagem() {
                    Some(l) => Json::texto_de(l.to_string()),
                    None => Json::Nulo,
                },
            ),
            ("eventos", Json::Lista(lista)),
        ]))
    }

    /// Quantos eventos o diario LOCAL desta tabela tem -- zero se ela nao
    /// existe aqui.
    pub(super) fn posicao_local(&self, database: &str, tabela: &str) -> Result<u64> {
        let trava = self.travar_dados()?;
        let Ok(db) = trava.abrir_database(database) else {
            return Ok(0);
        };
        match db.abrir_qualificada(tabela) {
            Ok(mut t) => t.eventos(),
            Err(_) => Ok(0),
        }
    }

    /// `aplicar`: grava na tabela LOCAL os eventos que vieram do source.
    ///
    /// Para no primeiro erro e devolve onde parou. Seguir depois de um erro
    /// espalharia a divergencia -- e o rowid que nao bate ja e o sinal de que
    /// a replica divergiu.
    pub(super) fn op_aplicar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let eventos = p
            .campo("eventos")
            .and_then(Json::lista)
            .ok_or_else(|| PhxError::Esquema("informe \"eventos\" como lista".into()))?
            .to_vec();
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;

        // Pedido 601: `linhagem` (opcional) e a historia da tabela de onde os
        // eventos sairam -- o `replicar` a entrega. Vindo, e diferente da
        // daqui, nada se aplica: rowid e carimbo de duas historias coincidem
        // por acaso de arranque, e a exclusao de B apagaria a linha de A com
        // `ok`. Sem o campo, como sempre foi (guarda nova entra pedida).
        let recusa_da_linhagem = match p.texto_ou("linhagem", "").trim() {
            "" => None,
            texto => {
                let dela = phxsql_core::uuid::Uuid::de_texto(texto)?;
                t.esquema()
                    .conferir_linhagem(Some(dela), "aplicar")
                    .err()
                    .map(|e| e.to_string())
            }
        };

        let mut aplicados = 0u64;
        let mut erro = None;
        for e in eventos.iter() {
            let operacao = match e.texto_ou("operacao", "") {
                "inclusao" => Operacao::Inclusao,
                "alteracao" => Operacao::Alteracao,
                "exclusao" => Operacao::Exclusao,
                outro => {
                    erro = Some(format!("operacao desconhecida no evento: {outro:?}"));
                    break;
                }
            };
            let rowid = e.inteiro_ou("rowid", 0).max(0) as u64;
            if let Some(motivo) = &recusa_da_linhagem {
                erro = Some(motivo.clone());
                break;
            }
            let imagem = match hex_para_bytes(e.texto_ou("imagem", "")) {
                Ok(b) => b,
                Err(x) => {
                    erro = Some(x.to_string());
                    break;
                }
            };
            match t.aplicar_evento(operacao, rowid, &imagem) {
                Ok(_) => aplicados += 1,
                Err(x) => {
                    erro = Some(x.to_string());
                    break;
                }
            }
        }
        self.gravar_de_verdade(&_trava, &mut t, p)?;

        Ok(Json::objeto(vec![
            ("recebidos", Json::de_u64(eventos.len() as u64)),
            ("aplicados", Json::de_u64(aplicados)),
            ("posicao", Json::de_u64(t.eventos()?)),
            (
                "erro",
                match erro {
                    Some(e) => Json::texto_de(e),
                    None => Json::Nulo,
                },
            ),
        ]))
    }

    /// Prova a ligacao com o outro servidor, e diz o que ele serve.
    ///
    /// # Por que ela existe, e por que no SERVIDOR
    ///
    /// Um assistente de replicacao so pode prometer o que conseguiu provar. O
    /// passo final dele precisa responder tres perguntas antes de alguem
    /// gravar configuracao nenhuma: **eu alcanco aquele servidor?**, **a
    /// credencial de replicacao entra?**, e **as tabelas de la servem para o
    /// modo escolhido?**. Fazer isso do navegador esbarraria na porta 5000 do
    /// outro lado; feito aqui, e a MESMA conexao e a MESMA autenticacao que o
    /// laco de replicacao usa depois -- e a prova vale, em vez de parecer.
    ///
    /// # A credencial
    ///
    /// `origem` aponta uma origem que ja esta no `config.json`, e e o caminho
    /// preferido: a credencial nao sai do servidor e nao viaja de novo. Quem
    /// esta montando uma ligacao NOVA manda host, porta, token e usuario com
    /// `senha_hash` -- o mesmo hash do cadastro, nunca a senha. Nada disso
    /// volta na resposta.
    pub(super) fn op_replicacao_testar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let host_solto = p.texto_ou("origem", "").trim().is_empty();
        let origem = match p.texto_ou("origem", "").trim() {
            // Uma origem ja configurada: a credencial nao viaja.
            nome if !nome.is_empty() => {
                // O laco desta origem ESTACIONOU por credencial recusada:
                // testar agora e ligar na origem com a mesma credencial, e
                // cada teste conta como tentativa leve no bloqueio de la. O
                // dialogo da tela sondava a cada 3 s -- medido em 09/09/2026,
                // cinco tentativas em 12 s de dialogo aberto, e o master
                // bloqueou o IP pela SONDA depois de o laco ter parado direito. A tela
                // deixou de sondar; este portao e para todo outro cliente,
                // porque portao e um so.
                self.recusar_se_estacionada(nome)?;
                self.config
                    .replicacao
                    .origens
                    .iter()
                    .find(|o| o.nome == nome)
                    .cloned()
                    .ok_or_else(|| {
                        PhxError::NaoEncontrado(format!(
                            "nao ha origem {nome:?} em replicacao.origens"
                        ))
                    })?
            }
            _ => {
                let host = p.texto_ou("host", "").trim().to_string();
                if host.is_empty() {
                    return Err(PhxError::Esquema(
                        "informe \"origem\" (uma ja configurada) ou \"host\"".into(),
                    ));
                }
                origem_da_sonda(p, host)
            }
        };

        match self.sondar_origem(&origem, p) {
            Ok(r) => Ok(r),
            // A falha de REDE volta classificada, nunca com o texto do sistema
            // operacional (revisao SEC de 17/09/2026, A5): «Connection refused»
            // distingue porta fechada de filtrada, e a sonda virava um scanner
            // com o texto de cada `errno`. O `ErrorKind` sobrevive ao embrulho
            // em `Cliente::conectar_com_prazo` justamente para se classificar
            // por tipo e nao por frase. E o host FORA da configuracao que nao
            // responde conta como tentativa leve: a origem nomeada ja tem o
            // freio do `recusar_se_estacionada`; o host solto nao tinha nenhum.
            Err(PhxError::Io(io)) => {
                if host_solto && !sessao.ip.is_empty() {
                    self.violacao_leve(
                        &sessao.ip,
                        "replicacao_testar",
                        "sonda para host fora da configuracao sem resposta",
                    );
                }
                let alvo = format!("{}:{}", origem.host, origem.porta);
                Err(PhxError::Io(std::io::Error::new(
                    io.kind(),
                    self.msg(chave_da_falha_de_rede(io.kind()), &[("alvo", &alvo)]),
                )))
            }
            Err(e) => Err(e),
        }
    }

    /// A conversa da sonda com o outro servidor, separada para a falha de rede
    /// ter UM lugar de classificacao -- ver [`Servidor::op_replicacao_testar`].
    fn sondar_origem(&self, origem: &crate::config::Origem, p: &Json) -> Result<Json> {
        let mut cliente = crate::replica::ligar(origem)?;
        // O IRMAO da descoberta do laco -- as mesmas cinco linhas --, e aqui
        // `so_os_databases_desta_origem` NAO entra, de proposito. Duas razoes,
        // e as duas sao decisao: a sonda RELATA, nao aplica evento nenhum, e
        // uma sonda que esconde o database em disputa mente justamente para
        // quem esta diagnosticando a disputa; e ela roda com a origem que veio
        // no PEDIDO, que pode nem estar no `config.json` -- reivindicar um
        // nome aqui entregaria a um host solto o database de uma origem de
        // verdade, e a guarda passaria a criar o estrago que existe para
        // impedir.
        let databases = if origem.databases.is_empty() {
            cliente.databases()?
        } else {
            origem.databases.clone()
        };
        let alvo = p.texto_ou("database", "").trim().to_string();

        let mut id_servidor = String::new();
        let mut imagem = false;
        let mut papel_de_la = String::new();
        let mut tabelas = Vec::new();
        let mut sem_chave: Vec<String> = Vec::new();
        for db in databases.iter().filter(|d| alvo.is_empty() || **d == alvo) {
            let r = cliente.pedir(vec![
                ("op", Json::texto_de("posicao")),
                ("database", Json::texto_de(db)),
            ])?;
            id_servidor = r.texto_ou("id_servidor", "").to_string();
            papel_de_la = r.texto_ou("papel", "").to_string();
            imagem = r.booleano_ou("imagem_da_linha", false);
            if let Some(Json::Objeto(pares)) = r.campo("tabelas") {
                for (nome, v) in pares {
                    let chave = v.texto_ou("chave", "").to_string();
                    if chave.is_empty() {
                        sem_chave.push(format!("{db}.{nome}"));
                    }
                    tabelas.push(Json::objeto(vec![
                        ("database", Json::texto_de(db)),
                        ("tabela", Json::texto_de(nome)),
                        (
                            "eventos",
                            Json::de_u64(v.inteiro_ou("eventos", 0).max(0) as u64),
                        ),
                        (
                            "registros",
                            Json::de_u64(v.inteiro_ou("registros", 0).max(0) as u64),
                        ),
                        // Vazio = sem identidade replicavel: serve para os
                        // modos A, C e D, e NAO serve para o bidirecional.
                        (
                            "chave",
                            if chave.is_empty() {
                                Json::Nulo
                            } else {
                                Json::texto_de(chave)
                            },
                        ),
                    ]));
                }
            }
        }

        // O que IMPEDE cada modo, dito antes de alguem configurar -- e nao
        // depois, pela replica parada.
        let mut impedimentos = Vec::new();
        if !imagem {
            impedimentos.push(Json::texto_de(
                "o outro servidor esta com replicacao.imagem_da_linha desligada: \
                 o diario dele registra que a linha mudou sem registrar a linha, \
                 e nao ha o que aplicar (vale para TODOS os modos)",
            ));
        }
        if id_servidor.trim().is_empty() {
            impedimentos.push(Json::texto_de(
                "o outro servidor nao tem replicacao.id_servidor: sem ele nao ha \
                 origem nos eventos, e o modo bidirecional nao tem como impedir \
                 que a alteracao volte para quem a mandou",
            ));
        }
        if !sem_chave.is_empty() {
            impedimentos.push(Json::texto_de(format!(
                "{} tabela(s) sem chave unica de colunas obrigatorias: elas \
                 replicam nos modos A, C e D, e o bidirecional as recusa, porque \
                 la a identidade entre servidores e a chave -- e chave que aceita \
                 nulo nao identifica ninguem",
                sem_chave.len()
            )));
        }

        Ok(Json::objeto(vec![
            ("alcancavel", Json::Bool(true)),
            ("host", Json::texto_de(&origem.host)),
            ("porta", Json::de_u64(origem.porta as u64)),
            // Nunca token, nunca senha, nunca hash: a resposta do protocolo
            // nao carrega credencial, nem a que quem perguntou acabou de
            // mandar.
            ("papel", Json::texto_de(papel_de_la)),
            ("id_servidor", Json::texto_de(id_servidor)),
            ("imagem_da_linha", Json::Bool(imagem)),
            (
                "databases",
                Json::Lista(databases.iter().map(Json::texto_de).collect()),
            ),
            ("tabelas", Json::Lista(tabelas)),
            (
                "sem_chave_unica",
                Json::Lista(sem_chave.iter().map(Json::texto_de).collect()),
            ),
            ("impedimentos", Json::Lista(impedimentos)),
        ]))
    }

    /// O estado do laco de replicacao DESTE servidor, origem por origem.
    ///
    /// E o que um assistente mostra no passo final: a posicao consumida de
    /// cada tabela, a ultima rodada, o ultimo erro, e as tabelas recusadas
    /// com o motivo (ex.: sem chave unica no modo bidirecional).
    pub(super) fn op_replicacao_estado(&self) -> Result<Json> {
        let origens = {
            let e = self.estado_replicacao.tomar("estado_replicacao")?;
            let mut pares: Vec<(String, Json)> =
                e.iter().map(|(k, v)| (k.clone(), v.para_json())).collect();
            pares.sort_by(|a, b| a.0.cmp(&b.0));
            Json::Objeto(pares)
        };
        // Colisoes de Sequence do modo multi (defeito (a)): so as tabelas com
        // count > 0 aparecem, para o campo ficar vazio no caso comum e gritar
        // quando ha estrago. Duas faixas iguais numerando a mesma chave perdem
        // linha em silencio; este e o numero que tira o silencio.
        let (colisoes, carimbos_do_futuro, recusas_por_unicidade) = match self.toques_bidi.lock() {
            Ok(g) => {
                let pares: Vec<(String, Json)> = g
                    .iter()
                    .filter(|(_, m)| m.colisoes > 0)
                    .map(|(k, m)| (k.clone(), Json::de_u64(m.colisoes)))
                    .collect();
                // Mesmo desenho: so a tabela com contagem aparece. E o
                // numero que tira o silencio do carimbo mentido (A9).
                let futuros: Vec<(String, Json)> = g
                    .iter()
                    .filter(|(_, m)| m.carimbos_do_futuro > 0)
                    .map(|(k, m)| (k.clone(), Json::de_u64(m.carimbos_do_futuro)))
                    .collect();
                // Mesmo desenho de novo: e o numero que tira o silencio da
                // linha que o indice unico secundario recusou (pedido 292).
                let unicidade: Vec<(String, Json)> = g
                    .iter()
                    .filter(|(_, m)| m.recusas_por_unicidade > 0)
                    .map(|(k, m)| (k.clone(), Json::de_u64(m.recusas_por_unicidade)))
                    .collect();
                (
                    Json::Objeto(pares),
                    Json::Objeto(futuros),
                    Json::Objeto(unicidade),
                )
            }
            Err(_) => (
                Json::Objeto(vec![]),
                Json::Objeto(vec![]),
                Json::Objeto(vec![]),
            ),
        };
        Ok(Json::objeto(vec![
            ("papel", Json::texto_de(self.papel_atual().nome())),
            (
                "papel_configurado",
                Json::texto_de(self.config.replicacao.papel.nome()),
            ),
            (
                "id_servidor",
                Json::texto_de(&self.config.replicacao.id_servidor),
            ),
            (
                "somente_leitura",
                Json::Bool(self.somente_leitura_vivo.load(Ordering::Relaxed)),
            ),
            ("origens", origens),
            ("colisoes_de_sequencia", colisoes),
            ("carimbos_do_futuro", carimbos_do_futuro),
            ("recusas_por_unicidade", recusas_por_unicidade),
            // Pedido 330: uma entrada por chave DISTINTA do diario local,
            // 86-118 bytes cada (medido, 01/10/2026), ate o teto por tabela
            // (`replicacao.teto_de_toques`, parte b). O numero nao e calado:
            // chaves, teto, esquecidas, e o que ja passou com a trava
            // exclusiva na mao.
            ("toques_no_mapa", self.toques_no_mapa()),
            // Pedido 300 §2.7: as linhas que a replicacao gravou aqui sem a
            // mae. So a tabela com contagem aparece, como os irmaos acima: o
            // campo vazio no caso comum, e o numero que tira o silencio do
            // invariante «so existe filho se o pai existir primeiro».
            ("orfas_na_replica", self.orfas_na_replica()),
            // Pedido 300 (4): os pedidos de escrita LOCAL que esta replica
            // fiel aceitou, por tabela. Vazio no caso comum (e sempre, com
            // `somente_leitura`).
            ("escritas_locais", self.escritas_locais_na_replica()),
            (
                "ledger_marcado_recebido",
                Json::de_u64(self.ledger_marcado_recebido.load(Ordering::Relaxed)),
            ),
            // Pedido 207: o estado do quorum de escrita. `Nulo` sem cluster.
            (
                "quorum",
                self.quorum.as_ref().map_or(Json::Nulo, |q| q.para_json()),
            ),
            // Pedido 684: os dois jeitos de o id de transacao sumir calado
            // NESTE processo, como origem. `recuos_do_relogio`: um `.log`
            // aberto trouxe id a frente do relogio, e o piso do disco o
            // segurou. `commits_mistos`: uma tomada gravou em volume sem id
            // (2/3) e em volume com id (4) -- a replica recebe esse commit
            // partido, e so a virada do volume velho fecha isso. E o teto
            // da transacao vigente (685), o mesmo dos dois lados.
            (
                "id_de_transacao",
                Json::objeto(vec![
                    (
                        "recuos_do_relogio",
                        Json::de_u64(phxsql_store::log::recuos_do_relogio()),
                    ),
                    (
                        "commits_mistos",
                        Json::de_u64(phxsql_store::log::commits_mistos()),
                    ),
                    (
                        "teto_da_transacao",
                        Json::de_u64(phxsql_store::log::teto_da_transacao() as u64),
                    ),
                ]),
            ),
        ]))
    }

    /// `{"db/tabela": N}` -- pedido 300 (4).
    fn escritas_locais_na_replica(&self) -> Json {
        let Ok(m) = self.escritas_locais_na_replica.lock() else {
            return Json::Objeto(vec![]);
        };
        let mut pares: Vec<(String, Json)> = m
            .iter()
            .map(|(k, n)| (k.clone(), Json::de_u64(*n)))
            .collect();
        pares.sort_by(|a, b| a.0.cmp(&b.0));
        Json::Objeto(pares)
    }

    /// `{"db/tabela": {"orfas": N, "sem_conferir": N}}` -- pedido 300 §2.7.
    fn orfas_na_replica(&self) -> Json {
        let Ok(m) = self.orfas_na_replica.lock() else {
            return Json::Objeto(vec![]);
        };
        let mut pares: Vec<(String, Json)> = m
            .iter()
            .map(|(k, (o, s))| {
                (
                    k.clone(),
                    Json::objeto(vec![
                        ("orfas", Json::de_u64(*o)),
                        ("sem_conferir", Json::de_u64(*s)),
                    ]),
                )
            })
            .collect();
        pares.sort_by(|a, b| a.0.cmp(&b.0));
        Json::Objeto(pares)
    }

    /// `{"db/tabela": {"chaves": N, "vistos": N, "sob_a_exclusiva": N,
    /// "escritores_entre_fatias": N, "fatias_que_furaram_a_fila": N, "teto": N,
    /// "esquecidas": N, "esquecido_ate": N, "vidas_novas": N}}` de
    /// toda tabela com mapa de toques -- pedido 330. Trava envenenada vira
    /// objeto vazio, como os contadores irmaos acima.
    fn toques_no_mapa(&self) -> Json {
        let Ok(g) = self.toques_bidi.lock() else {
            return Json::Objeto(vec![]);
        };
        let mut pares: Vec<(String, Json)> = g
            .iter()
            .map(|(k, m)| {
                (
                    k.clone(),
                    Json::objeto(vec![
                        ("chaves", Json::de_u64(m.toques.len() as u64)),
                        // Pedido 330 (b): o teto, quantas o teto ja esqueceu e
                        // ate onde -- o mapa que esquece diz que esqueceu.
                        ("teto", Json::de_u64(self.config.replicacao.teto_de_toques)),
                        ("esquecidas", Json::de_u64(m.esquecidas)),
                        (
                            "esquecido_ate",
                            m.piso.map_or(Json::Nulo, |(c, _)| Json::de_i64(c)),
                        ),
                        ("vistos", Json::de_u64(m.vistos)),
                        // Pedido 620: quantas vezes o mapa recomecou porque
                        // a tabela daqui era de outra vida.
                        ("vidas_novas", Json::de_u64(m.vidas_novas)),
                        (
                            "sob_a_exclusiva",
                            Json::de_u64(m.absorvidos_sob_a_exclusiva),
                        ),
                        (
                            "escritores_entre_fatias",
                            Json::de_u64(m.escritores_entre_as_fatias),
                        ),
                        (
                            "fatias_que_furaram_a_fila",
                            Json::de_u64(m.fatias_que_furaram_a_fila),
                        ),
                    ]),
                )
            })
            .collect();
        pares.sort_by(|a, b| a.0.cmp(&b.0));
        Json::Objeto(pares)
    }

    /// A origem esta com o laco estacionado por credencial recusada?
    /// Recusa nomeando o caminho de volta, em vez de gastar mais uma
    /// tentativa contra o IP no bloqueio da origem.
    fn recusar_se_estacionada(&self, origem: &str) -> Result<()> {
        let parada = self
            .estado_replicacao
            .tomar("estado_replicacao")?
            .get(origem)
            .map(|e| e.parada.clone())
            .unwrap_or_default();
        if parada.is_empty() {
            return Ok(());
        }
        Err(PhxError::Esquema(format!(
            "o laco da origem {origem:?} esta parado ({parada}): testar agora seria mais \
             uma tentativa com a credencial que a origem recusou, contra este IP no \
             bloqueio de la. Corrija a configuracao e religue com replicacao_ligar"
        )))
    }

    /// `replicacao_ligar`: manda o laco de uma origem tentar de novo AGORA.
    ///
    /// E o unico caminho de volta de um laco que estacionou por credencial
    /// recusada (pedido 203) -- o outro e reiniciar com a configuracao
    /// corrigida. Por tempo, nunca: um laco que voltasse sozinho depois de
    /// uma hora seria o mesmo defeito em camera lenta. Num laco que so esta
    /// dormindo (recuo de rede, ou o intervalo entre rodadas em vao) o pedido
    /// vale como «tente ja».
    ///
    /// Quem consome o pedido e o LACO, no passo seguinte dele -- ate 1 s.
    pub(super) fn op_replicacao_ligar(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("origem", "").trim().to_string();
        if nome.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"origem\": um nome de replicacao.origens, ou cluster:<id>".into(),
            ));
        }
        let mut estados = self.estado_replicacao.tomar("estado_replicacao")?;
        let Some(estado) = estados.get_mut(&nome) else {
            let mut conhecidas: Vec<&String> = estados.keys().collect();
            conhecidas.sort();
            return Err(PhxError::NaoEncontrado(format!(
                "nao ha laco de replicacao para a origem {nome:?}; as que este \
                 servidor conhece: {conhecidas:?}"
            )));
        };
        let estava_parada = estado.parada.clone();
        estado.religar_pedido = true;
        Ok(Json::objeto(vec![
            ("origem", Json::texto_de(&nome)),
            ("pedido", Json::Bool(true)),
            (
                "estava_parada",
                if estava_parada.is_empty() {
                    Json::Nulo
                } else {
                    Json::texto_de(&estava_parada)
                },
            ),
            (
                "aviso",
                Json::texto_de("o laco atende no passo seguinte dele, em ate 1 s"),
            ),
        ]))
    }

    /// `replicacao_pular`: solta o par que um conflito parou, pulando o
    /// evento pela POSICAO e mandando a origem adiante.
    ///
    /// E o `ALTER SUBSCRIPTION ... SKIP (lsn)` somado ao
    /// `pg_replication_origin_advance()` do PostgreSQL, na forma que o nosso
    /// laco permite: a posicao consumida vai para `posicao + 1`, e a marca de
    /// parada sai. Da rodada seguinte em diante o par volta a andar.
    ///
    /// # Por que ela EXIGE uma parada, em vez de aceitar qualquer posicao
    ///
    /// O `pg_replication_origin_advance()` aceita qualquer LSN e o manual
    /// dele diz, com todas as letras, que usar errado leva a inconsistencia.
    /// Aqui a restricao que causa a divergencia e outra: no nosso laco
    /// **reaplicar e inofensivo** -- o casamento e por chave e a regra e
    /// "mais recente vence" --, entao andar a posicao NUNCA conserta nada; so
    /// pode pular evento que ninguem olhou. Uma posicao livre seria um botao
    /// que so tem como errar. Com a parada exigida, o unico evento que se
    /// pula e aquele que um ser humano leu no `detalhe` e decidiu descartar.
    ///
    /// # O portao
    ///
    /// Administrar, e a tabela vai no campo `"tabela"` -- o campo que o
    /// portao unico do `despachar` le. Sem isso a operacao seria a setima a
    /// esconder tabela do portao, e quem tivesse o direito negado naquela
    /// tabela mexeria no laco dela assim mesmo.
    pub(super) fn op_replicacao_pular(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("origem", "").trim().to_string();
        let database = p.texto_ou("database", "").trim().to_string();
        let tabela = p.texto_ou("tabela", "").trim().to_string();
        if nome.is_empty() || database.is_empty() || tabela.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"origem\", \"database\" e \"tabela\": o pulo e de UMA \
                 tabela em UM par"
                    .into(),
            ));
        }
        let mut estados = self.estado_replicacao.tomar("estado_replicacao")?;
        let Some(estado) = estados.get_mut(&nome) else {
            let mut conhecidas: Vec<&String> = estados.keys().collect();
            conhecidas.sort();
            return Err(PhxError::NaoEncontrado(format!(
                "nao ha laco de replicacao para a origem {nome:?}; as que este \
                 servidor conhece: {conhecidas:?}"
            )));
        };
        // A chave e procurada SEM distinguir maiuscula, porque o nome da
        // tabela atravessa o protocolo como o cliente o escreveu e a chave
        // nasceu como o source a nomeou.
        let procurada = format!("{database}/{tabela}");
        let Some(chave_tab) = estado
            .paradas
            .keys()
            .find(|k| k.eq_ignore_ascii_case(&procurada))
            .cloned()
        else {
            let mut paradas: Vec<&String> = estado.paradas.keys().collect();
            paradas.sort();
            return Err(PhxError::NaoEncontrado(format!(
                "a replicacao de {procurada:?} na origem {nome:?} nao esta parada: \
                 nao ha evento para pular. Paradas nesta origem agora: {paradas:?}"
            )));
        };
        let parada = estado.paradas[&chave_tab].clone();
        if parada.posicao == crate::replica::POSICAO_DESCONHECIDA {
            return Err(PhxError::Esquema(format!(
                "a origem {nome:?} nao informou a posicao do evento que parou \
                 {chave_tab:?} -- ela e de uma versao que ainda nao manda o campo \
                 \"posicao\" no `replicar`. Pular exigiria adivinhar a posicao, e \
                 adivinhar aqui descarta evento que ninguem olhou: resolva o \
                 conflito no dado ({}) e atualize o outro lado",
                parada.detalhe
            )));
        }
        let nova = parada.posicao + 1;
        drop(estados);

        // Pedido 597: a posicao vai ao DISCO antes de a parada sair, e a falha
        // de grava-la vira a resposta. Era um `let _ =` depois de a parada ja
        // ter saido da memoria: o cliente ouvia «pulou» por uma posicao que um
        // reinicio devolvia ao evento descartado -- e o par parava de novo no
        // mesmo conflito sem ninguem saber por que. Enquanto a parada esta de
        // pe o laco nem chega a ler a posicao (`esta_parada` e o portao dele),
        // entao grava-la antes nao corre com ninguem.
        {
            let mut pos = self
                .posicoes_bidi
                .lock()
                .map_err(|_| PhxError::Io(std::io::Error::other("posicoes_bidi envenenado")))?;
            let anterior = pos.insert(parada.chave_da_posicao.clone(), nova);
            if let Err(e) = bidirecional::gravar_posicoes(
                &self.config.base.join("replicacao-posicoes.json"),
                &pos,
            ) {
                // O mapa volta ao que estava: ele e gravado INTEIRO pelo
                // proximo alcance de qualquer origem, que levaria ao disco o
                // pulo que esta resposta diz que nao aconteceu.
                match anterior {
                    Some(v) => pos.insert(parada.chave_da_posicao.clone(), v),
                    None => pos.remove(&parada.chave_da_posicao),
                };
                return Err(PhxError::Io(std::io::Error::other(format!(
                    "o evento {} de {chave_tab:?} NAO foi pulado: a posicao nova \
                     ({nova}) nao foi ao disco ({e}). A parada continua de pe; \
                     pule de novo depois de resolver o disco",
                    parada.posicao
                ))));
            }
        }
        self.anotar_estado(&nome, |est| {
            est.paradas.remove(&chave_tab);
            est.posicoes.insert(chave_tab.clone(), nova);
        });
        eprintln!(
            "replicacao [{nome}]: {chave_tab} SOLTA por replicacao_pular -- o evento \
             {} foi descartado e a posicao foi para {nova}. O que ele trazia: {}",
            parada.posicao, parada.detalhe
        );
        Ok(Json::objeto(vec![
            ("origem", Json::texto_de(&nome)),
            ("database", Json::texto_de(&database)),
            ("tabela", Json::texto_de(&tabela)),
            ("motivo", Json::texto_de(&parada.motivo)),
            ("pulou", Json::de_u64(parada.posicao)),
            ("posicao", Json::de_u64(nova)),
            ("detalhe", Json::texto_de(&parada.detalhe)),
            (
                "aviso",
                Json::texto_de(
                    "o evento pulado NAO entra mais: o outro lado continua com a \
                     linha dele, e os dois so voltam a ser iguais se alguem os \
                     igualar",
                ),
            ),
        ]))
    }
}

/// A recusa da replica FIEL quando a tabela daqui e de outra historia que a do
/// source -- pedido 601. `None` = segue.
///
/// O bloco de esquema chega no `posicao` com a linhagem do source (v11), e a
/// replica nascida dele tem a mesma, byte a byte. Outra linhagem e uma de duas
/// coisas: a tabela foi apagada e recriada la, ou foi criada por conta aqui.
/// Nas duas, rowid e carimbo coincidem por acaso de arranque, e aplicar
/// apagaria ou sobrescreveria a linha de outra historia. Sem linhagem de um
/// dos lados (esquema de antes da v11), fica como era.
///
/// So na replica fiel: no bidirecional a identidade e a chave, e os caixas de
/// um central divergem na linhagem legitimamente -- por isso a conferencia
/// NAO mora no `garantir_tabela_da_replica`, que o bidi tambem chama.
pub(super) fn recusa_da_linhagem(
    aqui: &Schema,
    database: &str,
    no: &crate::replica::NoSource,
) -> Option<String> {
    let la = no.esquema.as_ref()?.linhagem();
    aqui.conferir_linhagem(la, &format!("replicacao de {database}.{}", no.nome))
        .err()
        .map(|e| {
            format!(
                "{e}. A tabela foi apagada e recriada no source, ou criada por conta \
                 nesta replica. Esta tabela ficou como estava; para segui-la, \
                 apague-a nesta replica e ela renasce do esquema do source"
            )
        })
}

/// A `Origem` de uma sonda `replicacao_testar` montada com o que veio no
/// pedido -- o caminho de quem esta ligando um source NOVO, que ainda nao esta
/// no `config.json`.
///
/// Fora do laco, e pura, pelo mesmo motivo da [`origem_do_master`]: o irmao do
/// padrao de saida mora aqui. Esta origem cai no MESMO `replica::ligar` que o
/// laco vai usar depois, entao sondar em claro o que a replicacao vai pedir
/// cifrado devolveria "ligacao boa" para uma configuracao que nao sobe -- e o
/// inverso, um "nao liga" para uma que sobe. O padrao e um so
/// ([`crate::config::CIFRA_DE_SAIDA_PADRAO`]) porque padrao que mora em dois
/// lugares diverge nos dois.
pub(super) fn origem_da_sonda(p: &Json, host: String) -> crate::config::Origem {
    crate::config::Origem {
        nome: p.texto_ou("nome", "teste").trim().to_string(),
        host,
        porta: p
            .inteiro_ou("porta", crate::config::PORTA_PADRAO as i64)
            .clamp(1, 65_535) as u16,
        // `token_remoto` primeiro, e `token` so como resto: no `/api` da tela o
        // campo `token` JA e o de quem pede aqui, entao mandar o do outro
        // servidor com o mesmo nome faz um sobrescrever o outro dentro do
        // mesmo objeto.
        token: {
            let remoto = p.texto_ou("token_remoto", "");
            if remoto.is_empty() {
                p.texto_ou("token", "").to_string()
            } else {
                remoto.to_string()
            }
        },
        databases: p.textos("databases"),
        reconectar_em: 10,
        usuario: p.texto_ou("usuario", "").trim().to_string(),
        senha_hash: p.texto_ou("senha_hash", "").trim().to_string(),
        senha: p.texto_ou("senha", "").to_string(),
        cada_minutos: 0,
        hora: String::new(),
        cifra: p.booleano_ou("cifra", crate::config::CIFRA_DE_SAIDA_PADRAO),
        chave_do_fio: p.texto_ou("chave_do_fio", "").trim().to_string(),
        espelho: false,
    }
}
