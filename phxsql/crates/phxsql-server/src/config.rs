//! Configuracao do servidor, lida do `config.json`.
//!
//! O arquivo e JSON puro, lido pelo leitor do proprio projeto -- nenhuma
//! dependencia externa entra so por causa da configuracao.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

use crate::blacklist::Politica;
use crate::usuarios::Cadastro;

/// Porta padrao do PhxSql.
pub const PORTA_PADRAO: u16 = 5000;

/// Porta padrao da interface web. Outra porta de proposito: quem fala HTTP
/// nao e quem fala JSON Lines, e separar deixa o firewall escolher.
pub const PORTA_WEB_PADRAO: u16 = 5001;

/// `"IP:porta"` em endereco, com a mensagem de erro escrita para gente.
///
/// Extraida de `Config::endereco` porque a troca de porta pela tela precisa
/// resolver um endereco que ainda nao esta em `Config` -- e resolve-lo em
/// outro lugar significaria duas ideias diferentes do que e um endereco
/// valido.
pub fn endereco_de(bind: &str) -> Result<SocketAddr> {
    use std::net::ToSocketAddrs;
    let bind = bind.trim();
    if bind.is_empty() {
        return Err(PhxError::Esquema("endereco vazio".into()));
    }
    bind.to_socket_addrs()
        .map_err(|e| PhxError::Esquema(format!("bind invalido {bind:?}: {e}")))?
        .next()
        .ok_or_else(|| PhxError::Esquema(format!("bind sem endereco: {bind:?}")))
}

/// Papel do servidor na replicacao.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Papel {
    /// Servidor sozinho, sem replicacao.
    Isolado,
    /// Origem: mantem o diario e atende as replicas que se conectam.
    Source,
    /// Replica: conecta no source, le os eventos e aplica localmente.
    Replica,
    /// Replica de leitura: como a replica, e com o contrato EXPLICITO --
    /// leitura de cliente e bem-vinda, escrita de cliente e recusada com um
    /// erro que aponta o primario. Serve relatorio e balanceamento de leitura.
    ReadReplica,
    /// Reserva de contingencia: replica que NAO atende cliente nenhum, nem de
    /// leitura. So administracao e monitoramento enxergam, ate a operacao
    /// `spare_promover` transforma-la em primario.
    Spare,
    /// Bidirecional (multi-master): recebe escrita de cliente E puxa as
    /// alteracoes do outro servidor, casando as linhas pela CHAVE UNICA.
    /// Exige `id_servidor`, e o conflito e resolvido pelo carimbo mais
    /// recente -- ver docs/REPLICACAO.md.
    Multi,
}

impl Papel {
    fn de_texto(s: &str) -> Result<Papel> {
        Ok(match s.trim().to_lowercase().as_str() {
            "" | "isolado" | "standalone" => Papel::Isolado,
            "source" | "master" | "origem" => Papel::Source,
            "replica" | "slave" => Papel::Replica,
            "read_replica" | "read-replica" | "leitura" => Papel::ReadReplica,
            "spare" | "standby" => Papel::Spare,
            "multi" | "multimaster" | "multi-master" | "bidirecional" => Papel::Multi,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "papel de replicacao desconhecido: {outro}"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Papel::Isolado => "isolado",
            Papel::Source => "source",
            Papel::Replica => "replica",
            Papel::ReadReplica => "read_replica",
            Papel::Spare => "spare",
            Papel::Multi => "multi",
        }
    }

    /// Este papel roda o laco que puxa eventos de uma origem?
    pub fn puxa_de_origem(self) -> bool {
        matches!(
            self,
            Papel::Replica | Papel::ReadReplica | Papel::Spare | Papel::Multi
        )
    }

    /// Este papel PRECISA da imagem da linha no diario para cumprir o que
    /// promete? O source porque replica para fora; o multi porque alem disso
    /// casa conflito pela chave, que mora dentro da imagem.
    pub fn exige_imagem(self) -> bool {
        matches!(self, Papel::Source | Papel::Multi)
    }
}

/// De onde a replica puxa os eventos.
#[derive(Debug, Clone)]
pub struct Origem {
    pub nome: String,
    pub host: String,
    pub porta: u16,
    pub token: String,
    /// Databases a replicar. Vazio = todos.
    pub databases: Vec<String>,
    /// Segundos entre tentativas quando a conexao cai.
    pub reconectar_em: u64,
    /// Login com que a replica entra no source.
    pub usuario: String,
    /// Hash da senha desse login -- o MESMO texto do cadastro de usuarios.
    ///
    /// Dele sai a chave derivada do desafio-resposta, entao a replica se
    /// autentica sem que exista senha em claro em lugar nenhum.
    pub senha_hash: String,
    /// Senha em claro. Existe so para quem ainda nao trocou o `config.json`,
    /// e o arranque avisa em voz alta.
    pub senha: String,
    /// Replicacao AGENDADA: puxar a cada tantos minutos. Zero = streaming,
    /// que e o comportamento de sempre -- o laco puxa continuamente.
    pub cada_minutos: u64,
    /// Replicacao DIARIA a uma hora marcada, "HH:MM" (ex.: "02:30", a noite).
    /// Vazia = nao ha hora marcada. Com as duas vazias, vale o streaming.
    pub hora: String,
    /// Puxar por dentro do tunel cifrado. Ver `docs/CIFRA-DO-FIO.md`.
    ///
    /// `false` (o padrao) e como sempre foi: JSON em claro. Ligar exige que o
    /// SOURCE atenda o aperto -- e um source de versao anterior nao atende,
    /// entao ligar isto e uma decisao dos dois lados, nao de um.
    pub cifra: bool,
    /// A chave publica que se ESPERA do source, em hexadecimal -- o pino.
    ///
    /// Vazia com `cifra` ligada e tunel SEM pino: protege da escuta passiva e
    /// nao protege de quem esta no meio, porque o atacante apresenta a chave
    /// dele e nao ha com o que comparar. O arranque avisa exatamente isso.
    pub chave_do_fio: String,
}

impl Origem {
    /// A replicacao desta origem e agendada (em vez de streaming)?
    pub fn agendada(&self) -> bool {
        self.cada_minutos > 0 || !self.hora.is_empty()
    }

    /// O pino do source, ja em bytes -- ou o erro que diz o que corrigir.
    ///
    /// `None` significa "sem pino", e nao "qualquer chave serve por engano":
    /// hexadecimal torto vira ERRO em vez de virar `None`, senao um pino
    /// escrito errado viraria silenciosamente um tunel sem pino, que e
    /// exatamente o estrago que o pino existe para impedir.
    pub fn pino_do_fio(&self) -> Result<Option<[u8; 32]>> {
        if self.chave_do_fio.is_empty() {
            return Ok(None);
        }
        Ok(Some(chave_de_hex(
            &self.chave_do_fio,
            &format!("origens[{}].chave_do_fio", self.nome),
        )?))
    }
}

#[derive(Debug, Clone)]
pub struct Replicacao {
    pub papel: Papel,
    /// Socket por onde o SOURCE ENVIA os eventos para as replicas.
    ///
    /// Porta propria, separada da 5000, pelo mesmo motivo da interface web:
    /// quem fala replicacao nao e quem fala consulta, e o firewall precisa
    /// poder tratar as duas de forma diferente. Vazia = usa a porta de dados.
    pub envio: String,
    /// Socket por onde o SOURCE RECEBE o retorno das replicas.
    ///
    /// O retorno e o "apliquei ate aqui" de cada replica, mais os pedidos de
    /// reenvio. Separado do envio a pedido: com dois soquetes, uma replica
    /// lenta lendo devagar nao segura o canal por onde as confirmacoes das
    /// outras chegam, e o firewall pode abrir so um sentido.
    ///
    /// Vazio = a volta usa a MESMA conexao do envio, que e o desenho mais
    /// simples e o que o MySQL(R) faz.
    pub retorno: String,
    /// Identidade deste servidor, usada na numeracao global dos eventos.
    pub id_servidor: String,
    /// IPs autorizados a pedir o fluxo de replicacao (so no source).
    pub replicas_autorizadas: Vec<String>,
    /// Origens de onde puxar (so na replica). Varias = multi-source.
    pub origens: Vec<Origem>,
    /// Gravar a imagem da linha no `.log`? So com ela da para REPLICAR.
    ///
    /// Sem ela o evento diz que o rowid 42 mudou e nao diz para que -- basta
    /// para auditoria, nao basta para uma replica aplicar. Custa: um registro
    /// de 200 bytes gasta ~244 bytes de diario por alteracao em vez de 44.
    ///
    /// Liga sozinha quando o papel e `source`, que e quando ela e obrigatoria:
    /// um source sem imagem no diario e um source que nao replica, e descobrir
    /// isso pela replica parada seria o pior jeito de descobrir.
    pub imagem_da_linha: bool,
}

impl Replicacao {
    /// Resolve um dos enderecos de replicacao.
    fn resolver(rotulo: &str, texto: &str) -> Result<SocketAddr> {
        use std::net::ToSocketAddrs;
        texto
            .to_socket_addrs()
            .map_err(|e| PhxError::Esquema(format!("replicacao.{rotulo} invalida {texto:?}: {e}")))?
            .next()
            .ok_or_else(|| {
                PhxError::Esquema(format!("replicacao.{rotulo} sem endereco: {texto:?}"))
            })
    }

    /// Por onde o source ENVIA os eventos.
    pub fn endereco_envio(&self) -> Result<SocketAddr> {
        Replicacao::resolver("envio", &self.envio)
    }

    /// Por onde o source RECEBE o retorno das replicas.
    pub fn endereco_retorno(&self) -> Result<SocketAddr> {
        Replicacao::resolver("retorno", &self.retorno)
    }

    /// As portas configuradas, em ordem, para o arranque e para o `config`.
    pub fn portas(&self) -> Vec<(&'static str, &str)> {
        let mut v = Vec::new();
        if !self.envio.is_empty() {
            v.push(("envio", self.envio.as_str()));
        }
        if !self.retorno.is_empty() {
            v.push(("retorno", self.retorno.as_str()));
        }
        v
    }
}

impl Default for Replicacao {
    fn default() -> Self {
        Replicacao {
            papel: Papel::Isolado,
            envio: String::new(),
            retorno: String::new(),
            id_servidor: String::new(),
            replicas_autorizadas: Vec::new(),
            origens: Vec::new(),
            imagem_da_linha: false,
        }
    }
}

/// Um no do cluster, como os OUTROS o alcancam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoCluster {
    pub id: String,
    pub endereco: String,
    pub porta: u16,
}

impl NoCluster {
    /// `host:porta`, do jeito que um cliente redirecionado usa.
    pub fn alvo(&self) -> String {
        format!("{}:{}", self.endereco, self.porta)
    }
}

/// Cluster com eleicao e promocao automatica -- pedido 126.
///
/// # Pedida, nao imposta
///
/// Sem o bloco `cluster` no `config.json`, NADA disto existe: nenhuma thread
/// sobe, nenhum portao muda, e a replicacao continua exatamente como era. O
/// teste que trava isso e o do comportamento velho.
///
/// # O que o bloco liga
///
/// Cada no manda um pulso (`cluster_pulso`) aos outros pela porta de dados,
/// autenticado como a replica ja se autentica. Master calado alem da
/// `janela_inatividade_s` abre eleicao; so promove quem enxerga a MAIORIA dos
/// nos configurados, e entre os elegiveis vence a maior posicao do diario,
/// com empate por prioridade e depois pelo menor id. Nao e Raft -- as
/// garantias reais e as nao-garantias estao em `docs/CLUSTER.md`.
#[derive(Debug, Clone)]
pub struct Cluster {
    /// Todos os nos, ESTE incluido. A maioria e contada sobre esta lista.
    pub nos: Vec<NoCluster>,
    /// Qual no da lista e este servidor. Cai no `replicacao.id_servidor`.
    pub id: String,
    /// Desempate da eleicao (maior ganha). Viaja no pulso, entao cada no so
    /// precisa declarar a PROPRIA -- a dos mortos nao entra em eleicao nenhuma.
    pub prioridade: i64,
    /// Master sem pulso por tanto tempo = master caido.
    pub janela_s: u64,
    /// Intervalo do pulso. Zero no arquivo = um terco da janela.
    pub pulso_s: u64,
    /// Aviso por e-mail a cada X minutos enquanto degradado. Aceita fracao
    /// (0.1 = 6 s), porque a bancada precisa provar a repeticao sem esperar
    /// minutos de relogio.
    pub avisar_cada_min: f64,
    /// Por onde o aviso sai. Sem e-mail configurado, sem e-mail -- e nada
    /// mais muda.
    pub email: Email,
    /// Databases replicados no cluster. Vazio = todos os do master.
    pub databases: Vec<String>,
    /// Credenciais com que ESTE no fala com os outros -- as mesmas tres
    /// pecas da origem de replicacao, e pela mesma razao: a senha nunca
    /// aparece em claro, so o hash de onde sai a chave do desafio-resposta.
    pub token: String,
    pub usuario: String,
    pub senha_hash: String,
}

impl Cluster {
    fn de_json(j: &Json, id_servidor: &str) -> Result<Option<Cluster>> {
        let Some(c) = j.campo("cluster") else {
            return Ok(None);
        };
        let nos: Vec<NoCluster> = c
            .campo("nos")
            .and_then(Json::lista)
            .map(|l| {
                l.iter()
                    .map(|n| NoCluster {
                        id: n.texto_ou("id", "").trim().to_string(),
                        endereco: n.texto_ou("endereco", "127.0.0.1").trim().to_string(),
                        porta: n.inteiro_ou("porta", PORTA_PADRAO as i64).clamp(1, 65_535) as u16,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let janela_s = c.inteiro_ou("janela_inatividade_s", 10).max(1) as u64;
        let pulso_s = match c.inteiro_ou("pulso_s", 0).max(0) as u64 {
            0 => (janela_s / 3).max(1),
            p => p,
        };
        let avisar = c
            .campo("avisar_cada_min")
            .and_then(Json::numero)
            .unwrap_or(5.0);
        Ok(Some(Cluster {
            nos,
            id: c.texto_ou("id", id_servidor).trim().to_string(),
            prioridade: c.inteiro_ou("prioridade", 0),
            janela_s,
            pulso_s,
            // Abaixo de tres segundos o aviso viraria enxurrada ate em teste.
            avisar_cada_min: if avisar > 0.0 { avisar.max(0.05) } else { 5.0 },
            email: Email::de_json(c)?,
            databases: c.textos("databases"),
            token: c.texto_ou("token", "").to_string(),
            usuario: c.texto_ou("usuario", "").trim().to_string(),
            senha_hash: c.texto_ou("senha_hash", "").trim().to_string(),
        }))
    }

    /// O no desta lista com este id.
    pub fn no(&self, id: &str) -> Option<&NoCluster> {
        self.nos.iter().find(|n| n.id == id)
    }

    /// Os OUTROS nos -- os que este servidor pulsa.
    pub fn outros(&self) -> impl Iterator<Item = &NoCluster> {
        self.nos.iter().filter(move |n| n.id != self.id)
    }

    /// `vivos` enxergam a maioria dos nos CONFIGURADOS? Metade nao basta:
    /// dois lados de uma particao com metade cada um seriam dois masters.
    pub fn e_maioria(&self, vivos: usize) -> bool {
        vivos * 2 > self.nos.len()
    }

    /// Janela e pulso em milissegundos, para quem compara com carimbo.
    pub fn janela_ms(&self) -> i64 {
        self.janela_s as i64 * 1_000
    }

    pub fn avisar_cada_ms(&self) -> i64 {
        (self.avisar_cada_min * 60_000.0) as i64
    }

    fn validar(&self, replicacao: &Replicacao) -> Result<()> {
        if self.nos.len() < 2 {
            return Err(PhxError::Esquema(
                "cluster com menos de dois nos: nao ha o que eleger \
                 (preencha cluster.nos com todos os nos, este incluido)"
                    .into(),
            ));
        }
        if self.id.is_empty() {
            return Err(PhxError::Esquema(
                "cluster sem \"id\": diga qual no da lista e este servidor \
                 (ou preencha replicacao.id_servidor)"
                    .into(),
            ));
        }
        if self.no(&self.id).is_none() {
            return Err(PhxError::Esquema(format!(
                "cluster.id {:?} nao esta em cluster.nos -- este servidor \
                 precisa constar da propria lista",
                self.id
            )));
        }
        let mut ids: Vec<&str> = self.nos.iter().map(|n| n.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != self.nos.len() {
            return Err(PhxError::Esquema(
                "cluster.nos com id repetido ou vazio".into(),
            ));
        }
        if self.nos.iter().any(|n| n.id.is_empty()) {
            return Err(PhxError::Esquema("cluster.nos com no sem \"id\"".into()));
        }
        if replicacao.papel == Papel::Isolado {
            return Err(PhxError::Esquema(
                "cluster pede papel \"source\" (o master inicial) ou \"replica\" \
                 em replicacao.papel"
                    .into(),
            ));
        }
        if self.email.ligado {
            self.email.validar()?;
        }
        Ok(())
    }

    /// O resumo que a op `config` mostra. Sem token e sem hash: resposta de
    /// protocolo nao carrega credencial.
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("id", Json::texto_de(&self.id)),
            ("prioridade", Json::de_i64(self.prioridade)),
            ("janela_inatividade_s", Json::de_u64(self.janela_s)),
            ("pulso_s", Json::de_u64(self.pulso_s)),
            (
                "avisar_cada_min",
                Json::texto_de(format!("{:.2}", self.avisar_cada_min)),
            ),
            ("email", Json::Bool(self.email.ligado)),
            (
                "nos",
                Json::Lista(
                    self.nos
                        .iter()
                        .map(|n| {
                            Json::objeto(vec![
                                ("id", Json::texto_de(&n.id)),
                                ("endereco", Json::texto_de(&n.endereco)),
                                ("porta", Json::de_u64(n.porta as u64)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}

/// Backup agendado.
///
/// Vem desligado. Backup que roda sozinho num destino que ninguem conferiu e
/// backup que enche o disco e para -- ligar e uma decisao, com um destino
/// escolhido de proposito.
#[derive(Debug, Clone)]
pub struct Backup {
    pub agendado: bool,
    /// Pasta onde os arquivos caem.
    pub destino: PathBuf,
    /// Hora do dia, "HH:MM". Vazia = usa `cada_horas`.
    pub hora: String,
    /// Intervalo em horas, quando nao ha hora marcada.
    pub cada_horas: u64,
    /// Um ZIP unico (padrao) ou a arvore de diretorios.
    pub zip: bool,
    /// Qual database copiar. Vazio = todos.
    pub database: String,
    /// Nome que entra no arquivo, no lugar do usuario.
    pub admin: String,
    /// Quantos arquivos guardar. Zero = nao apaga nada.
    pub manter: usize,
}

impl Default for Backup {
    fn default() -> Self {
        Backup {
            agendado: false,
            destino: PathBuf::from("backups"),
            hora: String::new(),
            cada_horas: 24,
            zip: true,
            database: String::new(),
            admin: "agendado".into(),
            manter: 14,
        }
    }
}

impl Backup {
    fn de_json(j: &Json) -> Result<Backup> {
        let padrao = Backup::default();
        let Some(b) = j.campo("backup") else {
            return Ok(padrao);
        };
        let hora = b.texto_ou("hora", "").trim().to_string();
        if !hora.is_empty() && Backup::minuto_do_dia(&hora).is_none() {
            return Err(PhxError::Esquema(format!(
                "backup.hora invalida: {hora:?} (use \"HH:MM\", 24 horas)"
            )));
        }
        Ok(Backup {
            agendado: b.booleano_ou("agendado", false),
            destino: PathBuf::from(b.texto_ou("destino", "backups")),
            hora,
            cada_horas: b.inteiro_ou("cada_horas", padrao.cada_horas as i64).max(1) as u64,
            zip: b.booleano_ou("zip", true),
            database: b.texto_ou("database", "").trim().to_string(),
            admin: b.texto_ou("admin", "agendado").trim().to_string(),
            manter: b.inteiro_ou("manter", padrao.manter as i64).max(0) as usize,
        })
    }

    /// "HH:MM" em minutos desde a meia-noite. `None` se nao for hora.
    pub fn minuto_do_dia(hora: &str) -> Option<u64> {
        let (h, m) = hora.split_once(':')?;
        let h: u64 = h.trim().parse().ok()?;
        let m: u64 = m.trim().parse().ok()?;
        if h > 23 || m > 59 {
            return None;
        }
        Some(h * 60 + m)
    }

    /// Ja passou da hora de rodar de novo?
    ///
    /// `ultimo_ms` e zero quando nunca rodou. Com hora marcada, dispara quando
    /// o minuto do dia chega e ainda nao rodou hoje -- e nao a cada minuto
    /// depois disso.
    pub fn hora_de_rodar(&self, agora_ms: i64, ultimo_ms: i64) -> bool {
        if !self.agendado {
            return false;
        }
        match Backup::minuto_do_dia(&self.hora) {
            Some(alvo) => {
                let minuto_agora = (agora_ms.rem_euclid(86_400_000) / 60_000) as u64;
                let dia_agora = agora_ms.div_euclid(86_400_000);
                let dia_ultimo = ultimo_ms.div_euclid(86_400_000);
                minuto_agora >= alvo && (ultimo_ms == 0 || dia_agora > dia_ultimo)
            }
            None => {
                let intervalo = self.cada_horas as i64 * 3_600_000;
                ultimo_ms == 0 || agora_ms - ultimo_ms >= intervalo
            }
        }
    }
}

/// Aviso de espaco em disco, e por onde o aviso sai.
///
/// Vem desligado. Um alerta que dispara sozinho para um destinatario que
/// ninguem conferiu vira caixa de entrada cheia, e caixa de entrada cheia e
/// como um alerta de verdade passa despercebido.
///
/// # Os dois limites
///
/// Percentual e piso em MB valem JUNTOS, no OU: o que chegar primeiro
/// dispara. Sozinho, cada um erra de um lado -- 10% de um disco de 8 TB sao
/// 800 GB, que nao e aperto nenhum; e 1 GB livre num disco de 20 GB e aperto
/// de verdade sem chegar perto de 10%.
#[derive(Debug, Clone)]
pub struct Alertas {
    pub ligado: bool,
    /// Percentual livre abaixo do qual o disco vira alerta. Zero desliga.
    pub livre_minimo_percentual: f64,
    /// Piso absoluto de espaco livre, em MB. Zero desliga.
    pub livre_minimo_mb: u64,
    /// De quanto em quanto tempo o relogio confere os discos.
    pub checar_minutos: u64,
    /// Silencio entre dois avisos do MESMO caminho.
    ///
    /// Sem isto o alerta vira enxurrada: um disco cheio continua cheio, e
    /// avisar a cada conferencia manda dezenas de mensagens por hora ate
    /// alguem liberar espaco.
    pub repetir_horas: u64,
    /// Caminhos extras a vigiar, alem do `base` e do destino do backup.
    pub caminhos: Vec<PathBuf>,
    pub email: Email,
}

impl Default for Alertas {
    fn default() -> Self {
        Alertas {
            ligado: false,
            livre_minimo_percentual: 10.0,
            livre_minimo_mb: 1_024,
            checar_minutos: 15,
            repetir_horas: 6,
            caminhos: Vec::new(),
            email: Email::default(),
        }
    }
}

impl Alertas {
    fn de_json(j: &Json) -> Result<Alertas> {
        let padrao = Alertas::default();
        let Some(a) = j.campo("alertas") else {
            return Ok(padrao);
        };
        let numero = |campo: &str, padrao: f64| {
            a.campo(campo)
                .and_then(Json::numero)
                .unwrap_or(padrao)
                .max(0.0)
        };
        let alertas = Alertas {
            ligado: a.booleano_ou("ligado", false),
            livre_minimo_percentual: numero(
                "livre_minimo_percentual",
                padrao.livre_minimo_percentual,
            )
            .min(100.0),
            livre_minimo_mb: numero("livre_minimo_mb", padrao.livre_minimo_mb as f64) as u64,
            checar_minutos: a
                .inteiro_ou("checar_minutos", padrao.checar_minutos as i64)
                .max(1) as u64,
            repetir_horas: a
                .inteiro_ou("repetir_horas", padrao.repetir_horas as i64)
                .max(0) as u64,
            caminhos: a
                .textos("caminhos")
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            email: Email::de_json(a)?,
        };
        if alertas.ligado && alertas.livre_minimo_percentual <= 0.0 && alertas.livre_minimo_mb == 0
        {
            return Err(PhxError::Esquema(
                "alertas ligado com os dois limites em zero: nada dispararia nunca \
                 (preencha livre_minimo_percentual ou livre_minimo_mb)"
                    .into(),
            ));
        }
        // O aviso de jobs anda por fora do vigia de disco: `avisar_jobs` com
        // `alertas.ligado` falso ainda manda e-mail -- entao o endereco tem de
        // estar certo nos dois caminhos, e a recusa vem no arranque, nao as
        // tres da manha quando o primeiro job falhar.
        if alertas.email.ligado && (alertas.ligado || alertas.email.avisar_jobs) {
            alertas.email.validar()?;
        }
        Ok(alertas)
    }

    /// Este disco esta apertado?
    pub fn apertado(&self, livre_percentual: f64, livre_kb: u64) -> bool {
        (self.livre_minimo_percentual > 0.0 && livre_percentual < self.livre_minimo_percentual)
            || (self.livre_minimo_mb > 0 && livre_kb / 1_024 < self.livre_minimo_mb)
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("ligado", Json::Bool(self.ligado)),
            // NUMERO, e nao texto formatado. Como "10.00" ele mentia sobre o
            // tipo: a tela desenha um campo numerico, o arquivo guarda um
            // numero, e so a resposta trazia uma string -- o bastante para
            // "o arquivo diz 10 e o servidor diz 10.00" virar divergencia.
            (
                "livre_minimo_percentual",
                Json::Numero(self.livre_minimo_percentual),
            ),
            ("livre_minimo_mb", Json::de_u64(self.livre_minimo_mb)),
            ("checar_minutos", Json::de_u64(self.checar_minutos)),
            ("repetir_horas", Json::de_u64(self.repetir_horas)),
            (
                "caminhos",
                Json::Lista(
                    self.caminhos
                        .iter()
                        .map(|c| Json::texto_de(c.display().to_string()))
                        .collect(),
                ),
            ),
            ("email", self.email.para_json()),
        ])
    }
}

/// Para onde o alerta vai, e com que credencial.
///
/// # O que este cliente NAO faz
///
/// Nao fala TLS. A `std` nao traz TLS e o projeto nao aceita crate, entao a
/// conversa com o servidor de e-mail e em texto claro. Na pratica isso
/// significa RELE INTERNO -- um `postfix` na propria maquina ou na rede local,
/// que aceita a mensagem na porta 25 e cuida do TLS para fora. Nao serve para
/// entregar direto em provedor publico, que exige TLS na porta 465 ou 587.
///
/// Consequencia direta: se `usuario` e `senha` forem preenchidos, eles viajam
/// em base64 pela rede, e base64 nao esconde nada. Preencha so para um rele
/// que voce controla, e prefira liberar o IP no rele a mandar senha.
#[derive(Debug, Clone, Default)]
pub struct Email {
    pub ligado: bool,
    /// Avisar tambem sobre JOBS: quando um falha, e quando um esta parado.
    ///
    /// Opt-in de proposito, e separado do `ligado`: quem configurou e-mail so
    /// para o disco apertado nao pode comecar a receber aviso de job por
    /// causa de uma versao nova. Guarda nova entra pedida, nao imposta.
    pub avisar_jobs: bool,
    pub servidor: String,
    pub porta: u16,
    pub de: String,
    pub para: Vec<String>,
    pub usuario: String,
    /// PRIVADO de proposito: quem quiser ler passa por [`Email::senha`], e o
    /// `para_json` nunca a inclui. E a mesma regra da senha do usuario -- a
    /// diferenca e que esta o servidor precisa apresentar ao rele, entao nao
    /// da para guardar so o hash.
    senha: String,
    pub assunto: String,
    pub timeout_s: u64,
}

impl Email {
    fn de_json(j: &Json) -> Result<Email> {
        let Some(e) = j.campo("email") else {
            return Ok(Email::default());
        };
        // A senha pode vir de variavel de ambiente. E o caminho recomendado:
        // config.json costuma ir para o controle de versao, e variavel de
        // ambiente nao.
        let senha = match e.texto_ou("senha_env", "").trim() {
            "" => e.texto_ou("senha", "").to_string(),
            var => std::env::var(var).unwrap_or_default(),
        };
        Ok(Email {
            ligado: e.booleano_ou("ligado", false),
            avisar_jobs: e.booleano_ou("avisar_jobs", false),
            servidor: e.texto_ou("servidor", "127.0.0.1").trim().to_string(),
            porta: e.inteiro_ou("porta", 25).clamp(1, 65_535) as u16,
            de: e.texto_ou("de", "").trim().to_string(),
            para: e.textos("para"),
            usuario: e.texto_ou("usuario", "").trim().to_string(),
            senha,
            assunto: e
                .texto_ou("assunto", "PhxSql: espaco em disco")
                .trim()
                .to_string(),
            timeout_s: e.inteiro_ou("timeout_s", 10).max(1) as u64,
        })
    }

    /// A senha do rele. O unico caminho de leitura -- e nao aparece em JSON.
    pub fn senha(&self) -> &str {
        &self.senha
    }

    fn validar(&self) -> Result<()> {
        if self.servidor.is_empty() {
            return Err(PhxError::Esquema(
                "alertas.email ligado sem \"servidor\"".into(),
            ));
        }
        if self.de.is_empty() {
            return Err(PhxError::Esquema(
                "alertas.email ligado sem \"de\": o rele recusa mensagem sem remetente".into(),
            ));
        }
        if self.para.is_empty() {
            return Err(PhxError::Esquema(
                "alertas.email ligado sem \"para\": nao ha para quem mandar".into(),
            ));
        }
        // Cabecalho de e-mail termina em CRLF; um endereco com quebra de linha
        // deixaria quem escreve o config.json injetar cabecalho na mensagem.
        for campo in std::iter::once(&self.de).chain(self.para.iter()) {
            if campo.contains(['\r', '\n']) {
                return Err(PhxError::Esquema(format!(
                    "endereco de e-mail com quebra de linha: {campo:?}"
                )));
            }
            if !campo.contains('@') {
                return Err(PhxError::Esquema(format!(
                    "endereco de e-mail sem arroba: {campo:?}"
                )));
            }
        }
        Ok(())
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("ligado", Json::Bool(self.ligado)),
            ("avisar_jobs", Json::Bool(self.avisar_jobs)),
            ("servidor", Json::texto_de(&self.servidor)),
            ("porta", Json::de_u64(self.porta as u64)),
            ("de", Json::texto_de(&self.de)),
            (
                "para",
                Json::Lista(self.para.iter().map(Json::texto_de).collect()),
            ),
            ("usuario", Json::texto_de(&self.usuario)),
            // Nunca a senha. Nem mascarada com asteriscos do tamanho certo --
            // o tamanho ja e informacao.
            (
                "senha",
                Json::texto_de(if self.senha.is_empty() {
                    "(vazia)"
                } else {
                    "(oculta)"
                }),
            ),
            ("assunto", Json::texto_de(&self.assunto)),
            ("tls", Json::Bool(false)),
        ])
    }
}

/// A cifra dos diarios em repouso (`.log`, `.trash`, `.reason`).
///
/// # Pedida, nao imposta
///
/// Nasce DESLIGADA, e ligar vale para os volumes criados DAQUI PARA A FRENTE.
/// Um diario que ja existe em claro continua em claro e continua abrindo: um
/// arquivo append-only nao se reescreve, e nao ha como cifrar para tras sem
/// reescrever. Isso esta dito aqui porque a surpresa seria pior que a
/// limitacao -- quem liga a cifra precisa saber que o dado velho nao mudou de
/// lugar.
///
/// # O que ela protege
///
/// O ARQUIVO COPIADO: disco levado, backup vazado, copia numa maquina que nao
/// e esta. **Nao** protege contra quem le o `config.json` desta maquina, porque
/// e nele que a senha esta -- pela mesma razao da senha do rele de e-mail e da
/// do DbLink, o servidor precisa APRESENTAR a chave, entao nao da para guardar
/// so o hash.
#[derive(Clone, Default)]
pub struct Cifra {
    pub ligada: bool,
    /// PRIVADA de proposito: quem quiser ler passa por [`Cifra::senha`], e o
    /// `para_json` nunca a inclui.
    senha: String,
    /// Nome da variavel de ambiente de onde a senha veio, quando veio de la.
    pub senha_env: String,
    /// Iteracoes do PBKDF2. Zero cai no padrao do cofre.
    pub iteracoes: u32,
    /// Como o valor da coluna marcada e selado: `aead` (padrao) ou
    /// `frogcript`.
    ///
    /// O padrao NAO e o FrogCript, e a razao esta no documento do proprio
    /// autor (secao 9): a transposicao e a direcao nao acrescentam forca
    /// criptografica. O que ele acrescenta e formato, e custa 167 bytes por
    /// valor. Quem quiser o formato pede por ele.
    pub modo: String,
    /// De quantas em quantas casas o FrogCript extrai. Padrao 5.
    pub salto: usize,
    /// O separador entre os dois lados do pacote FrogCript. Padrao `|`.
    pub separador: String,
}

/// `Debug` escrito a mao: o derivado imprimiria a senha, e um diagnostico
/// apressado com `{:?}` num `Config` a jogaria no log. Segredo que aparece em
/// `Debug` vaza no dia em que alguem acrescentar um `dbg!`.
impl std::fmt::Debug for Cifra {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cifra")
            .field("ligada", &self.ligada)
            .field("senha", &"(oculta)")
            .field("senha_env", &self.senha_env)
            .field("iteracoes", &self.iteracoes)
            .field("modo", &self.modo)
            .field("salto", &self.salto)
            .field("separador", &self.separador)
            .finish()
    }
}

impl Cifra {
    fn de_json(j: &Json) -> Cifra {
        let Some(c) = j.campo("cifra") else {
            return Cifra::default();
        };
        // A senha pode vir do ambiente, e esse e o caminho recomendado:
        // `config.json` costuma ir para o controle de versao, e variavel de
        // ambiente nao.
        let senha_env = c.texto_ou("senha_env", "").trim().to_string();
        let senha = if senha_env.is_empty() {
            c.texto_ou("senha", "").to_string()
        } else {
            std::env::var(&senha_env).unwrap_or_default()
        };
        Cifra {
            ligada: c.booleano_ou("ligada", false),
            senha,
            senha_env,
            iteracoes: c
                .inteiro_ou("iteracoes", phxsql_store::cofre::ITERACOES_PADRAO as i64)
                .clamp(0, u32::MAX as i64) as u32,
            modo: c.texto_ou("modo", "aead").trim().to_string(),
            salto: c
                .inteiro_ou("salto", phxsql_core::frogcript::SALTO_PADRAO as i64)
                .clamp(0, 4096) as usize,
            separador: {
                let s = c.texto_ou("separador", "").to_string();
                if s.is_empty() {
                    (phxsql_core::frogcript::SEPARADOR_PADRAO as char).to_string()
                } else {
                    s
                }
            },
        }
    }

    /// O modo e o ajuste ja validados, ou o erro que diz o que corrigir.
    ///
    /// Validar AQUI, e nao no cofre, e o que faz um `config.json` errado
    /// impedir o servidor de subir em vez de derrubar a primeira gravacao --
    /// que e a mesma regra do `validar()` do arranque.
    pub fn modo_e_ajuste(
        &self,
    ) -> Result<(phxsql_store::cofre::Modo, phxsql_core::frogcript::Ajuste)> {
        let modo = phxsql_store::cofre::Modo::de_nome(&self.modo)?;
        // O separador e UM byte: e assim que ele vai ao pacote, e um caractere
        // de dois bytes gravaria metade dele. Recusar aqui e melhor que
        // truncar em silencio um valor que o dono escolheu.
        let sep = self.separador.as_bytes();
        if sep.len() != 1 {
            return Err(PhxError::Esquema(format!(
                "cifra.separador {:?} tem {} bytes: use um unico caractere ASCII",
                self.separador,
                sep.len()
            )));
        }
        Ok((
            modo,
            phxsql_core::frogcript::Ajuste::novo(self.salto, sep[0])?,
        ))
    }

    /// A senha do cofre. O unico caminho de leitura -- e nao aparece em JSON.
    pub fn senha(&self) -> &str {
        &self.senha
    }

    /// Liga o cofre do processo, se a configuracao pediu.
    ///
    /// # Por que so LIGA, e nunca desliga
    ///
    /// Desligar aqui seria uma decisao sobre um estado que este `Config` pode
    /// nao ter posto -- e um `config.json` lido por engano derrubaria a chave
    /// de um servidor que ja estava lendo diario cifrado. Quem desliga e quem
    /// para o processo.
    pub fn aplicar(&self) -> Result<()> {
        if !self.ligada {
            return Ok(());
        }
        let (modo, ajuste) = self.modo_e_ajuste()?;
        phxsql_store::cofre::definir_com(&self.senha, self.iteracoes, modo, ajuste)
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("ligada", Json::Bool(self.ligada)),
            ("iteracoes", Json::de_u64(self.iteracoes as u64)),
            ("modo", Json::texto_de(&self.modo)),
            ("salto", Json::de_u64(self.salto as u64)),
            // O salto e o separador personalizados sao parte do segredo (secao
            // 10 do documento do FrogCript), mas so quando SAEM do padrao --
            // e o padrao esta publicado. Mostrar o de fabrica ajuda quem
            // configura; mostrar um personalizado o entregaria.
            (
                "separador",
                Json::texto_de(
                    if self.separador.as_bytes()
                        == [phxsql_core::frogcript::SEPARADOR_PADRAO].as_slice()
                        && self.salto == phxsql_core::frogcript::SALTO_PADRAO
                    {
                        self.separador.as_str()
                    } else {
                        "(oculto)"
                    },
                ),
            ),
            ("senha_env", Json::texto_de(&self.senha_env)),
            // Nunca a senha. Nem mascarada com asteriscos do tamanho certo --
            // o tamanho ja e informacao.
            (
                "senha",
                Json::texto_de(if self.senha.is_empty() {
                    "(vazia)"
                } else if self.senha_env.is_empty() {
                    "(oculta)"
                } else {
                    "(do ambiente)"
                }),
            ),
        ])
    }
}

// ---------------------------------------------------------------------------
// A cifra do FIO -- que nao e a cifra dos arquivos acima
// ---------------------------------------------------------------------------

/// O aperto de mao estilo Noise da porta de dados. Ver `docs/CIFRA-DO-FIO.md`.
///
/// # Pedida, nao imposta -- e o motivo de `exigir` existir
///
/// `ligada` nasce LIGADA e isso nao muda nada para ninguem: o aperto so
/// acontece se o CLIENTE pedir, e cliente que nunca ouviu falar dele nunca
/// pede. `exigir` nasce DESLIGADA, e e ela que carrega a decisao dificil.
///
/// Cifra pedida e cifra que o atacante ativo apaga do pedido: ele corta o
/// `cifrar` do fio, o cliente rebaixa para claro, e a protecao vira zero.
/// Contra ele so vale `exigir: true` -- que quebra todo cliente velho, e por
/// isso e uma decisao de quem implanta, e nao um padrao herdado.
///
/// **Com `exigir` desligado, o tunel protege contra escuta PASSIVA e nada
/// mais.** Esta frase esta aqui, no `docs/SEGURANCA.md` e na tela pelo mesmo
/// motivo: e a que o leitor nao pode ter de adivinhar.
#[derive(Clone)]
pub struct CifraFio {
    /// O servidor ATENDE o aperto. `false` recusa -- e a unica maneira de um
    /// servidor dizer "aqui nao tem".
    pub ligada: bool,
    /// Recusa qualquer pedido fora do tunel.
    pub exigir: bool,
    /// PRIVADA de proposito: quem quiser ler passa por [`CifraFio::estatica`],
    /// e o `para_json` nunca a inclui.
    chave_privada: String,
    /// Nome da variavel de ambiente de onde a privada veio, quando veio de la.
    pub chave_privada_env: String,
    /// Onde a estatica e lida, ou criada na primeira vez que alguem pedir o
    /// aperto. Relativo ao `config.json`, quando ele tem caminho.
    pub arquivo: PathBuf,
}

/// `Debug` escrito a mao, pelo mesmo motivo do da [`Cifra`]: o derivado
/// imprimiria a chave privada, e um `{:?}` num diagnostico a jogaria no log.
impl std::fmt::Debug for CifraFio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CifraFio")
            .field("ligada", &self.ligada)
            .field("exigir", &self.exigir)
            .field("chave_privada", &"(oculta)")
            .field("chave_privada_env", &self.chave_privada_env)
            .field("arquivo", &self.arquivo)
            .finish()
    }
}

impl Default for CifraFio {
    fn default() -> Self {
        CifraFio {
            ligada: true,
            exigir: false,
            chave_privada: String::new(),
            chave_privada_env: String::new(),
            arquivo: PathBuf::from("chave-do-fio.hex"),
        }
    }
}

impl CifraFio {
    fn de_json(j: &Json) -> CifraFio {
        let padrao = CifraFio::default();
        let Some(c) = j.campo("cifra_fio") else {
            return padrao;
        };
        let chave_privada_env = c.texto_ou("chave_privada_env", "").trim().to_string();
        let chave_privada = if chave_privada_env.is_empty() {
            c.texto_ou("chave_privada", "").trim().to_string()
        } else {
            std::env::var(&chave_privada_env).unwrap_or_default()
        };
        CifraFio {
            ligada: c.booleano_ou("ligada", padrao.ligada),
            exigir: c.booleano_ou("exigir", padrao.exigir),
            chave_privada,
            chave_privada_env,
            arquivo: {
                let a = c.texto_ou("arquivo", "").trim().to_string();
                if a.is_empty() {
                    padrao.arquivo
                } else {
                    PathBuf::from(a)
                }
            },
        }
    }

    /// O caminho do arquivo da estatica, resolvido ao lado do `config.json`.
    ///
    /// Um caminho relativo escrito no `config.json` significa "ao lado dele", e
    /// nao "ao lado de onde o processo por acaso subiu": um servico iniciado do
    /// `/` criaria a chave na raiz, e o pino de todo cliente quebraria na
    /// primeira vez que alguem o subisse de outro diretorio.
    pub fn caminho_da_chave(&self, config_em: Option<&Path>) -> PathBuf {
        if self.arquivo.is_absolute() {
            return self.arquivo.clone();
        }
        match config_em.and_then(|c| c.parent()) {
            Some(pasta) if !pasta.as_os_str().is_empty() => pasta.join(&self.arquivo),
            _ => self.arquivo.clone(),
        }
    }

    /// A privada estatica do servidor, e os avisos que a busca gerou.
    ///
    /// Ordem: variavel de ambiente, `config.json`, arquivo proprio. O arquivo
    /// e CRIADO na primeira vez -- e so na primeira vez que alguem de fato
    /// pede o aperto, para um servidor com quem ninguem faz aperto nao passar
    /// a escrever arquivo que antes nao escrevia.
    pub fn estatica(&self, config_em: Option<&Path>) -> Result<([u8; 32], Vec<String>)> {
        let mut avisos = Vec::new();
        if !self.chave_privada.is_empty() {
            let de_onde = if self.chave_privada_env.is_empty() {
                "cifra_fio.chave_privada".to_string()
            } else {
                format!("a variavel {}", self.chave_privada_env)
            };
            return Ok((chave_de_hex(&self.chave_privada, &de_onde)?, avisos));
        }

        let caminho = self.caminho_da_chave(config_em);
        if caminho.exists() {
            let texto = std::fs::read_to_string(&caminho).map_err(|e| {
                PhxError::Esquema(format!("nao consegui ler {}: {e}", caminho.display()))
            })?;
            return Ok((
                chave_de_hex(&texto, &caminho.display().to_string())?,
                avisos,
            ));
        }

        let nova = phxsql_core::x25519::gerar_privada();
        if let Err(e) = gravar_chave(&caminho, &nova) {
            // Nao derruba o servidor: ele estava funcionando antes disto
            // existir. Mas AVISA alto, porque uma estatica que muda a cada
            // arranque quebra o pino de todo cliente -- e quebra em silencio.
            avisos.push(format!(
                "cifra_fio: nao consegui gravar {} ({e}). A chave do fio vale \
                 so enquanto este processo viver, entao o pino de todo cliente \
                 quebra no proximo arranque",
                caminho.display()
            ));
        }
        Ok((nova, avisos))
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("ligada", Json::Bool(self.ligada)),
            ("exigir", Json::Bool(self.exigir)),
            (
                "arquivo",
                Json::texto_de(self.arquivo.display().to_string()),
            ),
            ("chave_privada_env", Json::texto_de(&self.chave_privada_env)),
            // Nunca a privada -- nem mascarada, que o tamanho ja e informacao.
            (
                "chave_privada",
                Json::texto_de(if self.chave_privada.is_empty() {
                    "(do arquivo)"
                } else if self.chave_privada_env.is_empty() {
                    "(oculta)"
                } else {
                    "(do ambiente)"
                }),
            ),
        ])
    }
}

/// Le 32 bytes em hexadecimal, dizendo de onde vieram quando estao errados.
fn chave_de_hex(texto: &str, de_onde: &str) -> Result<[u8; 32]> {
    let limpo: String = texto.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = phxsql_core::hash::de_hex(&limpo).ok_or_else(|| {
        PhxError::Esquema(format!(
            "a chave do fio em {de_onde} nao e hexadecimal valido"
        ))
    })?;
    if bytes.len() != 32 {
        return Err(PhxError::Esquema(format!(
            "a chave do fio em {de_onde} tem {} bytes, e a X25519 tem 32",
            bytes.len()
        )));
    }
    let mut k = [0u8; 32];
    k.copy_from_slice(&bytes);
    Ok(k)
}

/// Grava a estatica com permissao 0600 no Unix.
///
/// A permissao e posta na CRIACAO, e nao depois: entre criar aberto e apertar
/// ha uma janela em que qualquer um le a chave, e essa janela e a unica coisa
/// que este arquivo existe para nao ter.
fn gravar_chave(caminho: &Path, chave: &[u8; 32]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut opcoes = std::fs::OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opcoes.mode(0o600);
    }
    let mut arq = opcoes.open(caminho)?;
    writeln!(arq, "{}", phxsql_core::hash::para_hex(chave))?;
    arq.sync_all()
}

/// Interface web: um servidor HTTP separado, que serve a pagina do Centro de
/// Controle e traduz o clique do navegador no mesmo protocolo da porta 5000.
///
/// Vem DESLIGADA e presa ao proprio computador. Ligar abre uma porta a mais, e
/// isso e uma decisao de quem administra -- nao um padrao herdado.
#[derive(Debug, Clone)]
pub struct Web {
    pub ligado: bool,
    /// Endereco de escuta da interface. Padrao: so o proprio computador.
    pub bind: String,
    /// Minutos que uma sessao do navegador vale sem uso. Cada clique renova.
    pub sessao_minutos: u64,
    /// Servidores PhxSql que esta interface pode alcancar, como "host:porta".
    ///
    /// VAZIO = so este servidor. E o padrao, e e o padrao certo: uma interface
    /// que fala com qualquer endereco e um proxy aberto de saida, e quem
    /// invadir a porta da web ganha a rede inteira junto.
    pub servidores: Vec<String>,
}

impl Default for Web {
    fn default() -> Self {
        Web {
            ligado: false,
            bind: format!("127.0.0.1:{PORTA_WEB_PADRAO}"),
            sessao_minutos: 60,
            servidores: Vec::new(),
        }
    }
}

impl Web {
    fn de_json(j: &Json) -> Web {
        let padrao = Web::default();
        match j.campo("web") {
            None => padrao,
            Some(w) => Web {
                ligado: w.booleano_ou("ligado", false),
                bind: w.texto_ou("bind", &padrao.bind).to_string(),
                sessao_minutos: w
                    .inteiro_ou("sessao_minutos", padrao.sessao_minutos as i64)
                    .max(1) as u64,
                servidores: w.textos("servidores"),
            },
        }
    }

    pub fn endereco(&self) -> Result<SocketAddr> {
        use std::net::ToSocketAddrs;
        self.bind
            .to_socket_addrs()
            .map_err(|e| PhxError::Esquema(format!("web.bind invalido {:?}: {e}", self.bind)))?
            .next()
            .ok_or_else(|| PhxError::Esquema(format!("web.bind sem endereco: {:?}", self.bind)))
    }

    /// Prazo da sessao em milissegundos.
    pub fn sessao_ms(&self) -> i64 {
        self.sessao_minutos as i64 * 60_000
    }

    /// A interface pode abrir conexao para este endereco?
    ///
    /// Compara o texto exato do `config.json`. Nada de resolver nome e
    /// comparar IP: quem controla o DNS decidiria o que a lista permite.
    /// Ha algum servidor configurado? Sem isso a interface so fala consigo.
    pub fn alcanca_outro_servidor(&self) -> bool {
        !self.servidores.is_empty()
    }

    pub fn servidor_permitido(&self, alvo: &str) -> bool {
        let d = alvo.trim();
        !d.is_empty() && self.servidores.iter().any(|p| p.trim() == d)
    }
}

/// Quando o dado gravado vai de fato para o disco.
///
/// # O numero que decide isto
///
/// Medido com 20.000 linhas na mesma tabela, mesmos dados, mesma maquina:
///
/// ```text
/// sincroniza a cada linha ......  1.289 linhas/s   (o que o servidor fazia)
/// a cada 100 ................... 18.264 linhas/s   14,2x
/// a cada 1.000 ................. 24.858 linhas/s   19,3x
/// so no fim .................... 26.301 linhas/s   20,4x
/// ```
///
/// Ou seja: **95% do tempo de uma insercao pelo servidor era `fsync`**, e nao
/// o heap nem o indice. Depois de tirar o `fsync` a insercao custa 37,5 us, dos
/// quais 65% sao os dois indices -- que e o gargalo seguinte, nao este.
///
/// # O que se arrisca
///
/// Os bytes vao para o sistema operacional em toda gravacao, sempre: um
/// `write` direto, sem buffer nosso. Entao **outro processo que abrir o arquivo
/// ve o dado na hora**, sincronizado ou nao. O `fsync` protege de UMA coisa: o
/// computador perder energia antes de o sistema descarregar a pagina.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Durabilidade {
    /// `fsync` depois de cada gravacao. Nao perde nada nem numa queda de
    /// energia, e custa 20x.
    PorOperacao,
    /// `fsync` a cada N gravacoes ou T milissegundos, o que vier primeiro.
    ///
    /// Uma queda de energia perde, no pior caso, o que entrou na janela. E o
    /// padrao porque a janela e curta e o ganho e grande.
    #[default]
    PorLote,
    /// Nunca chama `fsync`; deixa o sistema operacional decidir quando
    /// descarregar. O mais rapido, e o que mais perde numa queda.
    Sistema,
}

impl Durabilidade {
    pub fn de_texto(t: &str) -> Result<Durabilidade> {
        Ok(match t.trim().to_ascii_lowercase().as_str() {
            "" | "por_lote" | "lote" => Durabilidade::PorLote,
            "por_operacao" | "operacao" | "sempre" => Durabilidade::PorOperacao,
            "sistema" | "nunca" => Durabilidade::Sistema,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "durabilidade desconhecida: {outro:?} \
                     (use por_operacao, por_lote ou sistema)"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Durabilidade::PorOperacao => "por_operacao",
            Durabilidade::PorLote => "por_lote",
            Durabilidade::Sistema => "sistema",
        }
    }
}

/// O que o servidor pode consumir da maquina.
///
/// Todos os tetos aceitam zero, e zero quer dizer **sem teto imposto por
/// aqui** -- nao "desligado". Um teto de memoria em zero nao faz o servidor
/// rodar sem memoria; faz ele nao se limitar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recursos {
    pub durabilidade: Durabilidade,
    /// A exclusao FISICA tambem entra na janela de durabilidade?
    ///
    /// # Nasce DESLIGADO, e e por isso que ele existe
    ///
    /// Hoje um `excluir` que responde OK **ja esta no disco**: o `fsync` mora
    /// dentro de `LixeiraFile::guardar`, e ele acontece por exclusao. Poe-lo
    /// na janela por padrao mudaria o significado da resposta para todo
    /// cliente que ja existe, sem ninguem ter pedido -- e retirar garantia sem
    /// pedido e o mesmo estrago que impor guarda nova, pelo outro lado.
    ///
    /// Ligado, a exclusao passa a fechar com o resto da tabela: 3,6 s -> 0,84 s
    /// em 20.000 exclusoes, medido em `docs/DESEMPENHO.md` §4.12. O que se
    /// arrisca esta escrito la, e cabe numa linha: numa QUEDA DE ENERGIA
    /// dentro da janela, uma linha ja liberada do `.reg` pode nao ter chegado
    /// ao `.trash`. Queda do PROCESSO nao perde nada.
    pub exclusao_na_janela: bool,
    /// Quantas gravacoes cabem numa janela de sincronizacao.
    pub lote_operacoes: u64,
    /// Quantos milissegundos uma janela dura, no maximo.
    pub lote_milissegundos: u64,
    /// Paginas do `.ndx` mantidas em memoria, por arquivo aberto. Cada uma tem
    /// 4 KiB, entao 2.048 dao 8 MiB por tabela aberta.
    ///
    /// O padrao saiu de uma varredura de quatro tamanhos, em
    /// `docs/DESEMPENHO.md` §2.1: 2.048 e o joelho da curva.
    pub cache_paginas: usize,
    /// Teto de memoria para as tabelas residentes (`SelectMemory`), em MiB.
    /// Zero = sem teto.
    pub memoria_max_mb: u64,
    /// Threads de trabalho. Zero = quantos nucleos a maquina tiver.
    pub threads: usize,
    /// Percentual de CPU que o trabalho dividido pode usar, de 1 a 100.
    ///
    /// Nao e uma cota do sistema operacional -- ele nao tem como impor isso a
    /// um processo. E quantos nucleos o trabalho dividido usa: 50 em oito
    /// nucleos usa quatro. Cortar pela metade a divisao e o unico jeito
    /// honesto de "usar menos CPU" sem mentir sobre o mecanismo.
    pub cpu_percentual: u8,
    /// Conexoes simultaneas aceitas.
    pub conexoes_max: usize,
    /// Minutos que uma reserva de carga (`BULKINSERT`) dura sem ser renovada.
    ///
    /// E a SEGUNDA rede de protecao contra reserva orfa. A primeira e a queda
    /// da conexao, que solta na hora; esta pega o caso em que o soquete fica
    /// pendurado vivo com o cliente morto do outro lado.
    ///
    /// Zero nao desliga: cairia no padrao, porque reserva sem prazo nenhum e
    /// exatamente a que trava a tabela para sempre.
    pub carga_prazo_min: u64,
    /// Minutos que uma transacao aberta dura sem ser confirmada.
    ///
    /// E a SEGUNDA rede contra transacao orfa, exatamente como no
    /// `carga_prazo_min`: a primeira e a queda da conexao, que desfaz na hora;
    /// esta pega o soquete pendurado vivo com o cliente morto do outro lado.
    ///
    /// Curto de proposito -- uma transacao segura tabelas contra a escrita de
    /// todo mundo, e ninguem digita por dez minutos com uma transacao aberta.
    /// Zero nao desliga: cairia no padrao, porque transacao sem prazo nenhum e
    /// exatamente a que trava a tabela para sempre.
    pub transacao_prazo_min: u64,
    /// Teto de linhas empilhadas numa transacao. Zero = sem teto.
    ///
    /// O conjunto de escrita inteiro fica em RAM ate o `COMMIT` -- e o preco
    /// declarado do desenho de «nada vai a disco antes do COMMIT». Estourado,
    /// a operacao e RECUSADA com erro nomeado e a transacao vai para
    /// `ABORT_ONLY`: nunca engolida, e nunca vazada para disco pelas costas.
    pub transacao_max_linhas: u64,
    /// Quanto uma transacao aceita ESPERAR por uma trava de outra, em ms.
    ///
    /// # Por que tres prazos, e nao um
    ///
    /// Sao problemas diferentes. O `transacao_prazo_min` limita a transacao
    /// INTEIRA -- ela nao pode segurar tabela a tarde toda. Este limita a
    /// espera por OUTRO, que e o que transforma a possibilidade de abraco
    /// mortal entre linhas num erro nomeado em vez de numa thread pendurada. E
    /// o `transacao_statement_ms` limita UMA operacao, que pode demorar sem
    /// que nem a transacao nem a espera tenham estourado.
    ///
    /// Zero quer dizer **nao espere**: recusa na hora, que era o desenho
    /// anterior desta frente e continua sendo uma escolha legitima.
    pub transacao_lock_timeout_ms: u64,
    /// Quanto UMA operacao dentro de uma transacao pode levar, em ms.
    ///
    /// **Zero = sem prazo**, e esse e o padrao: por o relogio em cima de toda
    /// operacao mudaria o comportamento de quem nunca pediu isso.
    ///
    /// Vale nos PONTOS DE CANCELAMENTO que existem -- os lacos longos que ja
    /// chamam `Atividade::siga`, como a conversao de uma carga. Uma insercao
    /// de uma linha nao tem ponto de cancelamento no meio e nao poderia ter:
    /// parar entre o slot e o indice deixaria a tabela e o indice discordando.
    /// O `docs/TRANSACOES.md` diz exatamente onde ele morde.
    pub transacao_statement_ms: u64,
    /// Usuarios DIFERENTES conectados ao mesmo tempo. Zero = sem teto.
    ///
    /// Nao e o mesmo que conexoes: um usuario pode ter varias. Este teto conta
    /// logins distintos, que e o que uma licenca por posto quer contar.
    pub usuarios_max: usize,
    /// Onde o volume do `.log`, da `.trash` e do `.reason` corta, em MiB.
    ///
    /// **Zero = nao mexe**, e esse e o padrao: vale o `bytes_por_arquivo` do
    /// esquema, que e 1 GiB. Existe porque 1 GiB e um numero razoavel para um
    /// anexo e nao para um diario de eventos de 44 bytes -- 1 GiB de `.log` sao
    /// 24 milhoes de eventos, e na pratica o primeiro volume de uma tabela de
    /// um milhao de linhas nunca fecha.
    ///
    /// Isso importa porque volume FECHADO e a unidade de tudo que se faz com
    /// diario velho: compactar, arquivar, mover para disco barato. Um arquivo
    /// que nunca fecha volume nao oferece nenhuma dessas.
    ///
    /// O preco de cortar pequeno esta medido em `docs/DESEMPENHO.md`: mais
    /// volumes e um TETO menor, porque `max_arquivos` continua valendo.
    pub diario_volume_mib: u64,
}

impl Default for Recursos {
    fn default() -> Self {
        Recursos {
            durabilidade: Durabilidade::PorLote,
            // Ver o campo: desligado e o comportamento de sempre.
            exclusao_na_janela: false,
            lote_operacoes: 200,
            lote_milissegundos: 200,
            cache_paginas: 2_048,
            memoria_max_mb: 0,
            threads: 0,
            cpu_percentual: 100,
            conexoes_max: 64,
            carga_prazo_min: 30,
            transacao_prazo_min: 5,
            // 100.000 linhas de umas duas centenas de bytes dao ~20 MiB de
            // conjunto de escrita -- folga larga para a carga que motivou esta
            // frente (2.500 linhas) e teto baixo o bastante para uma transacao
            // esquecida nao comer a memoria do servidor.
            transacao_max_linhas: 100_000,
            // Meio segundo e o numero do proprio exemplo do desenho, e ele e
            // curto de proposito: quem espera mais que isso por uma linha esta
            // disputando de verdade, e a resposta certa e o erro nomeado.
            transacao_lock_timeout_ms: 500,
            transacao_statement_ms: 0,
            usuarios_max: 0,
            diario_volume_mib: 0,
        }
    }
}

/// O rodizio do arquivo `.txt` do Profiler.
///
/// # Por que ele NASCE ligado, e a regra da casa
///
/// «Guarda nova entra pedida, nao imposta» existe para nao quebrar quem ja
/// escreveu cliente contra o comportamento de antes. Aqui a pergunta e quem
/// esta sendo protegido de que: o `.txt` mede **345 bytes por pedido** e nao
/// parava nunca -- **1,2 GB por hora** a mil pedidos por segundo, o que enche
/// a particao do servidor inteiro, e nao so o log.
///
/// E o arquivo nunca prometeu ser completo: com o disco cheio ele ja perdia
/// linha em silencio (medido: 400 pedidos, 223 linhas), e o conserto de
/// entao foi CONTAR a perda, nao evita-la. Trocar um arquivo sem teto que
/// morre junto com a particao por um arquivo com teto que avisa quando vira e
/// estritamente melhor -- e o Profiler e ferramenta de diagnostico, ligada
/// por minutos, nao diario de auditoria.
///
/// Quem quiser o comportamento exato de antes escreve `arquivo_mib: 0`, e
/// esta escrito no MANUAL ao lado do campo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerfilEmDisco {
    /// Teto de cada arquivo, em MiB. **Zero = sem rodizio**, como era antes.
    pub arquivo_mib: u64,
    /// Quantos arquivos ANTIGOS guardar, alem do corrente.
    ///
    /// Zero e legitimo e quer dizer «nao guarde historico»: cheio o teto, o
    /// arquivo recomeca. O gasto maximo e `arquivo_mib x (arquivos + 1)`.
    pub arquivos: usize,
}

impl Default for PerfilEmDisco {
    fn default() -> PerfilEmDisco {
        // 64 x (4 + 1) = 320 MiB de teto, que a 345 bytes por pedido sao
        // ~970.000 pedidos -- muito alem de qualquer sessao que alguem leia, e
        // ainda assim um numero que cabe em qualquer particao de servidor.
        PerfilEmDisco {
            arquivo_mib: 64,
            arquivos: 4,
        }
    }
}

impl PerfilEmDisco {
    fn de_json(j: &Json) -> PerfilEmDisco {
        let padrao = PerfilEmDisco::default();
        let Some(c) = j.campo("profiler") else {
            return padrao;
        };
        PerfilEmDisco {
            arquivo_mib: c
                .inteiro_ou("arquivo_mib", padrao.arquivo_mib as i64)
                .max(0) as u64,
            arquivos: (c.inteiro_ou("arquivos", padrao.arquivos as i64).max(0) as usize)
                .min(crate::profiler::MAX_ARQUIVOS_ANTIGOS),
        }
    }

    /// O teto por arquivo em BYTES, que e a unidade do profiler.
    pub fn teto_do_arquivo(&self) -> u64 {
        self.arquivo_mib.saturating_mul(1024 * 1024)
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("arquivo_mib", Json::de_u64(self.arquivo_mib)),
            ("arquivos", Json::de_u64(self.arquivos as u64)),
            // O produto sai daqui, e nao da tela: e o numero que o operador
            // compara com o `df`, e uma segunda multiplicacao escrita no
            // JavaScript envelheceria no dia em que a regra mudasse.
            (
                "teto_em_disco_mib",
                Json::de_u64(self.arquivo_mib.saturating_mul(self.arquivos as u64 + 1)),
            ),
        ])
    }
}

/// O interruptor da trilha de dado pessoal (`.lgpd`).
///
/// # Nasce LIGADA, e por que isso nao quebra a regra da casa
///
/// «Guarda nova entra pedida, nao imposta» existe para que uma protecao nova
/// nao pare quem escreveu o cliente antes dela. Aqui nada para: a trilha so
/// acontece em tabela que tem coluna marcada como dado pessoal, e marcar e um
/// ato deliberado de quem cadastrou o campo. **Tabela sem marca nao muda de
/// comportamento** -- nao ganha arquivo, nao paga custo, nao responde
/// diferente. Quem marcou ja declarou que ali ha dado pessoal; a trilha e a
/// consequencia legal dessa declaracao.
///
/// Os dois lados sao separados porque respondem a perguntas diferentes e
/// custam diferente: a alteracao e barata e e a que a lei pede primeiro; o
/// acesso e o que uma base muito lida gera em volume. Quem precisa apertar o
/// tamanho desliga o acesso e mantem a alteracao.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lgpd {
    /// Registrar ANTES e DEPOIS de coluna marcada que mudou.
    pub alteracoes: bool,
    /// Registrar quem LEU coluna marcada, um registro por operacao.
    pub acessos: bool,
}

impl Default for Lgpd {
    fn default() -> Lgpd {
        Lgpd {
            alteracoes: true,
            acessos: true,
        }
    }
}

impl Lgpd {
    fn de_json(j: &Json) -> Lgpd {
        let padrao = Lgpd::default();
        // Bloco ausente = os padroes, que sao ligados. Um `config.json`
        // escrito antes desta versao continua valendo, e o que ele descreve
        // nao muda: tabela sem coluna marcada nao tem trilha de qualquer jeito.
        let Some(c) = j.campo("lgpd") else {
            return padrao;
        };
        Lgpd {
            alteracoes: c.booleano_ou("alteracoes", padrao.alteracoes),
            acessos: c.booleano_ou("acessos", padrao.acessos),
        }
    }

    /// Leva a decisao ao processo.
    pub fn aplicar(&self) {
        phxsql_store::trilha::definir(self.alteracoes, self.acessos);
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("alteracoes", Json::Bool(self.alteracoes)),
            ("acessos", Json::Bool(self.acessos)),
        ])
    }
}

/// As cores das bolhas do painel de telemetria, e os limiares que decidem
/// qual delas cada atividade recebe.
///
/// # Cor VAZIA quer dizer *de fabrica*, e nao preto
///
/// A cor de fabrica nao e um hexadecimal: e a variavel do tema (`var(--reg)`,
/// `var(--ambar)`, `var(--vermelho)`, `var(--acao-marcar)`), que escurece
/// sozinha no tema claro pelo mesmo motivo do vermelhao da marca. Congelar
/// aqui o hexadecimal do tema escuro como "padrao" tiraria isso de quem nunca
/// pediu nada. Por isso o campo vazio nao viaja na resposta: o retrato de quem
/// nao configurou cor nenhuma e o mesmo de antes deste bloco existir, e e isso
/// que `sem_cor_configurada_nada_muda` trava.
///
/// # Por que os limiares moram no mesmo bloco
///
/// Porque sao a MESMA regra vista dos dois lados: o limiar decide o nivel no
/// servidor, e a legenda da tela escreve o numero que decidiu. Eles ja saiam
/// daqui para a resposta (campo `limiares`) justamente para nao existirem em
/// dois lugares; o que este bloco acrescenta e poder mudar o numero sem
/// recompilar. Quem nao escrever nada continua com os 2 s e os 5 s de fabrica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Painel {
    pub cor_normal: String,
    pub cor_alto: String,
    pub cor_stress: String,
    pub cor_encerrando: String,
    /// A partir de quantos milissegundos a operacao corrente pinta de amarelo.
    pub alto_uso_ms: u64,
    /// A partir de quantos milissegundos trabalhando a atividade fica vermelha.
    pub stress_ms: u64,
}

impl Default for Painel {
    fn default() -> Painel {
        Painel {
            cor_normal: String::new(),
            cor_alto: String::new(),
            cor_stress: String::new(),
            cor_encerrando: String::new(),
            // O padrao sai da MESMA constante que o servidor usa para decidir
            // o nivel. Repetir o 2000 aqui seria abrir a porta para os dois
            // numeros discordarem no dia em que um deles mudasse.
            alto_uso_ms: crate::telemetria::ALTO_USO_MS,
            stress_ms: crate::telemetria::STRESS_MS,
        }
    }
}

/// `#rrggbb`, e so isso -- ou vazio, que e o pedido de voltar a de fabrica.
///
/// A tela escolhe a cor num `<input type="color">`, que produz exatamente esta
/// forma. Aceitar nome do CSS ou `rgb()` alargaria o que entra sem alargar o
/// que sai, e alargaria tambem o que a conferencia de contraste precisa
/// entender antes de avisar que a escolha ficou ilegivel.
pub fn cor_valida(t: &str) -> bool {
    t.is_empty()
        || (t.len() == 7 && t.starts_with('#') && t[1..].bytes().all(|b| b.is_ascii_hexdigit()))
}

impl Painel {
    fn de_json(j: &Json, avisos: &mut Vec<String>) -> Painel {
        let padrao = Painel::default();
        // Bloco ausente = tudo de fabrica. Config escrito antes desta versao
        // continua pintando o painel exatamente como pintava.
        let Some(c) = j.campo("telemetria") else {
            return padrao;
        };
        // Cor torta nao derruba o servidor: vira aviso e cai na de fabrica,
        // igual ao idioma que nao existe. Derrubar o arranque por causa de uma
        // cor seria a guarda cobrando mais caro do que aquilo que ela protege.
        // Quem RECUSA a cor torta e o portao da gravacao pela tela
        // (`TipoDoCampo::Cor`), onde ela ainda da para corrigir na hora.
        let mut cor = |campo: &str| {
            let t = c.texto_ou(campo, "").trim().to_lowercase();
            if cor_valida(&t) {
                return t;
            }
            avisos.push(format!(
                "telemetria.{campo}: {t:?} nao e uma cor #rrggbb; \
                 a cor de fabrica continua valendo"
            ));
            String::new()
        };
        Painel {
            cor_normal: cor("cor_normal"),
            cor_alto: cor("cor_alto"),
            cor_stress: cor("cor_stress"),
            cor_encerrando: cor("cor_encerrando"),
            // Zero apagaria o nivel inteiro -- com limiar zero TODA operacao em
            // curso ja nasce amarela, e cor que pinta todo mundo nao separa
            // ninguem. O piso de 1 ms mantem o campo util e o painel honesto.
            alto_uso_ms: c
                .inteiro_ou("alto_uso_ms", padrao.alto_uso_ms as i64)
                .max(1) as u64,
            stress_ms: c.inteiro_ou("stress_ms", padrao.stress_ms as i64).max(1) as u64,
        }
    }

    /// As cores CONFIGURADAS, para a resposta da telemetria.
    ///
    /// `None` quando nenhuma foi escolhida; e, quando alguma foi, as vazias
    /// continuam de fora -- o que nao viaja e o que a tela pinta de fabrica,
    /// um nivel de cada vez.
    pub fn cores_json(&self) -> Option<Json> {
        let pares: Vec<(&str, Json)> = [
            ("normal", &self.cor_normal),
            ("alto", &self.cor_alto),
            ("stress", &self.cor_stress),
            ("encerrando", &self.cor_encerrando),
        ]
        .into_iter()
        .filter(|(_, c)| !c.is_empty())
        .map(|(n, c)| (n, Json::texto_de(c.as_str())))
        .collect();
        (!pares.is_empty()).then(|| Json::objeto(pares))
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("cor_normal", Json::texto_de(&self.cor_normal)),
            ("cor_alto", Json::texto_de(&self.cor_alto)),
            ("cor_stress", Json::texto_de(&self.cor_stress)),
            ("cor_encerrando", Json::texto_de(&self.cor_encerrando)),
            ("alto_uso_ms", Json::de_u64(self.alto_uso_ms)),
            ("stress_ms", Json::de_u64(self.stress_ms)),
        ])
    }
}

impl Recursos {
    /// Leva ao processo os tetos que nao sao parametro de ninguem.
    ///
    /// Dois hoje: onde o volume do diario corta, e quantos nucleos o trabalho
    /// dividido usa. O teto do cache de paginas continua sendo aplicado pelo
    /// servidor, onde ja estava -- mover os tres para o mesmo lugar e limpeza,
    /// e limpeza em arquivo de outro agente e conflito.
    ///
    /// O teto de nucleos e o LEITOR de `threads` e `cpu_percentual`: antes
    /// dele os dois campos estavam no config.json, no MANUAL e na tela, e o
    /// `paralelo::nucleos()` perguntava direto a maquina -- a mesma armadilha
    /// do `cache_paginas` sem cache.
    pub fn aplicar(&self) {
        phxsql_store::diario::definir_bytes_por_volume(self.diario_volume_mib * 1024 * 1024);
        phxsql_core::paralelo::definir_teto(self.nucleos());
        // O leitor do `exclusao_na_janela`. Sem esta linha o campo estaria no
        // config.json, no MANUAL e na tela sem nada o ler -- que e exatamente
        // a armadilha do `cache_paginas` prometendo um cache que nao existia.
        phxsql_store::lixeira::definir_na_janela(self.exclusao_na_janela);
    }

    fn de_json(j: &Json, conexoes_no_topo: usize) -> Result<Recursos> {
        let padrao = Recursos::default();
        let r = match j.campo("recursos") {
            None => {
                return Ok(Recursos {
                    conexoes_max: conexoes_no_topo,
                    ..padrao
                })
            }
            Some(r) => r,
        };
        Ok(Recursos {
            durabilidade: Durabilidade::de_texto(r.texto_ou("durabilidade", ""))?,
            exclusao_na_janela: r.booleano_ou("exclusao_na_janela", padrao.exclusao_na_janela),
            lote_operacoes: r
                .inteiro_ou("lote_operacoes", padrao.lote_operacoes as i64)
                .max(1) as u64,
            lote_milissegundos: r
                .inteiro_ou("lote_milissegundos", padrao.lote_milissegundos as i64)
                .max(1) as u64,
            carga_prazo_min: {
                let m = r.inteiro_ou("carga_prazo_min", padrao.carga_prazo_min as i64);
                if m > 0 {
                    m as u64
                } else {
                    padrao.carga_prazo_min
                }
            },
            transacao_prazo_min: {
                let m = r.inteiro_ou("transacao_prazo_min", padrao.transacao_prazo_min as i64);
                if m > 0 {
                    m as u64
                } else {
                    padrao.transacao_prazo_min
                }
            },
            transacao_max_linhas: r
                .inteiro_ou("transacao_max_linhas", padrao.transacao_max_linhas as i64)
                .max(0) as u64,
            transacao_lock_timeout_ms: r
                .inteiro_ou(
                    "transacao_lock_timeout_ms",
                    padrao.transacao_lock_timeout_ms as i64,
                )
                .max(0) as u64,
            transacao_statement_ms: r
                .inteiro_ou(
                    "transacao_statement_ms",
                    padrao.transacao_statement_ms as i64,
                )
                .max(0) as u64,
            diario_volume_mib: r.inteiro_ou("diario_volume_mib", 0).max(0) as u64,
            cache_paginas: r
                .inteiro_ou("cache_paginas", padrao.cache_paginas as i64)
                .max(0) as usize,
            memoria_max_mb: r.inteiro_ou("memoria_max_mb", 0).max(0) as u64,
            threads: r.inteiro_ou("threads", 0).max(0) as usize,
            // Fora de 1..=100 nao ha o que fazer de sensato, entao vale o
            // limite mais proximo em vez de recusar o arranque inteiro.
            cpu_percentual: r.inteiro_ou("cpu_percentual", 100).clamp(1, 100) as u8,
            // `conexoes_max` no topo continua valendo, para config antigo nao
            // quebrar. Dentro de `recursos` ele ganha.
            conexoes_max: r.inteiro_ou("conexoes_max", conexoes_no_topo as i64).max(1) as usize,
            usuarios_max: r.inteiro_ou("usuarios_max", 0).max(0) as usize,
        })
    }

    /// Quantos nucleos o trabalho dividido pode usar.
    pub fn nucleos(&self) -> usize {
        let disponiveis = if self.threads > 0 {
            self.threads
        } else {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        };
        // O percentual corta a divisao, e nunca abaixo de um: metade de um
        // nucleo continua sendo um nucleo.
        ((disponiveis * self.cpu_percentual as usize) / 100).max(1)
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("durabilidade", Json::texto_de(self.durabilidade.nome())),
            ("exclusao_na_janela", Json::Bool(self.exclusao_na_janela)),
            ("lote_operacoes", Json::de_u64(self.lote_operacoes)),
            ("lote_milissegundos", Json::de_u64(self.lote_milissegundos)),
            ("cache_paginas", Json::de_u64(self.cache_paginas as u64)),
            ("diario_volume_mib", Json::de_u64(self.diario_volume_mib)),
            ("carga_prazo_min", Json::de_u64(self.carga_prazo_min)),
            (
                "transacao_prazo_min",
                Json::de_u64(self.transacao_prazo_min),
            ),
            (
                "transacao_max_linhas",
                Json::de_u64(self.transacao_max_linhas),
            ),
            (
                "transacao_lock_timeout_ms",
                Json::de_u64(self.transacao_lock_timeout_ms),
            ),
            (
                "transacao_statement_ms",
                Json::de_u64(self.transacao_statement_ms),
            ),
            ("memoria_max_mb", Json::de_u64(self.memoria_max_mb)),
            ("threads", Json::de_u64(self.threads as u64)),
            ("cpu_percentual", Json::de_u64(self.cpu_percentual as u64)),
            ("nucleos_efetivos", Json::de_u64(self.nucleos() as u64)),
            ("conexoes_max", Json::de_u64(self.conexoes_max as u64)),
            ("usuarios_max", Json::de_u64(self.usuarios_max as u64)),
        ])
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    /// Endereco e porta de escuta.
    pub bind: String,
    /// Raiz onde ficam os databases.
    pub base: PathBuf,
    /// Segredo exigido em todo pedido.
    pub token: String,
    /// Teto de linhas devolvidas numa varredura.
    pub max_linhas: u64,
    /// Arquivo do log de acessos.
    pub log_acessos: PathBuf,
    /// IPs autorizados. Vazio = qualquer origem (so use atras de firewall).
    pub ips_permitidos: Vec<String>,
    /// Conexoes simultaneas aceitas.
    ///
    /// Espelha `recursos.conexoes_max`; fica aqui porque `config.json` antigo
    /// traz o campo no topo e nao pode parar de subir.
    pub conexoes_max: usize,
    /// O que o servidor pode consumir da maquina, e quando grava de verdade.
    pub recursos: Recursos,
    /// Segundos de espera por um pedido antes de encerrar a conexao.
    pub timeout_s: u64,
    /// Recusa qualquer operacao de escrita.
    pub somente_leitura: bool,
    /// Espelha todo `.reg` num `.bkp` irmao -- a segunda chance.
    ///
    /// Custa uma escrita a mais por gravacao e o dobro de espaco do `.reg`.
    /// Protege contra o dado ficar RUIM, nao contra o disco morrer: os dois
    /// arquivos moram no mesmo lugar.
    pub espelho: bool,
    pub replicacao: Replicacao,
    /// Cluster com eleicao e promocao automatica. `None` = tudo como sempre.
    pub cluster: Option<Cluster>,
    /// Usuarios e o poder de cada um sobre cada base.
    pub cadastro: Cadastro,
    /// Comandos e bases proibidos, e a politica de bloqueio.
    pub politica: Politica,
    /// Arquivo da lista de bloqueio.
    pub blacklist: PathBuf,
    /// Interface web.
    pub web: Web,
    /// Backup agendado.
    pub backup: Backup,
    /// Aviso de disco apertado, e o e-mail por onde ele sai.
    pub alertas: Alertas,
    /// Arquivo com as ligacoes de DbLink.
    ///
    /// Separado do `config.json` de proposito: o cadastro de ligacoes muda
    /// pela tela, e reescrever o `config.json` inteiro a cada ligacao nova
    /// arriscaria os comentarios e o resto da configuracao a cada gravacao.
    pub dblink: PathBuf,
    /// Arquivo com os jobs de execucao.
    ///
    /// Separado pelo mesmo motivo do DbLink: o cadastro muda pela tela. E as
    /// corridas vao para o `.log` de mesmo nome, ao lado.
    pub jobs: PathBuf,
    /// A cifra dos diarios em repouso. Desligada por padrao.
    pub cifra: Cifra,
    /// A cifra do FIO -- o aperto de mao da porta de dados. Ver [`CifraFio`].
    pub cifra_fio: CifraFio,
    /// A trilha de dado pessoal. Ver [`Lgpd`].
    pub lgpd: Lgpd,
    /// As cores e os limiares do painel de bolhas. Ver [`Painel`].
    pub telemetria: Painel,
    /// O rodizio do `.txt` do Profiler. Ver [`PerfilEmDisco`].
    pub profiler: PerfilEmDisco,
    /// O idioma das mensagens do servidor: o nome de uma das seis colunas da
    /// tabela `phxsys.mensagens`. Vazio ou ausente = `Portugues`, que e o
    /// texto de fabrica -- e por isso config antigo nao muda nada.
    pub idioma: String,
    /// Campos do arquivo que o servidor nao reconhece.
    ///
    /// Nao e erro -- config antigo continua subindo. E aviso: campo escrito
    /// errado e silencioso, e silencio aqui custa caro. Quem escreve
    /// `"porta": 5001` esperando trocar a porta (o campo e `bind`) descobria
    /// so quando ninguem conseguia conectar.
    pub estranhas: Vec<String>,
    /// Avisos de leitura que nao impedem o servidor de subir -- valor que foi
    /// ignorado, idioma que nao existe. O `main` os imprime no arranque, pelo
    /// mesmo motivo das `estranhas`: silencio aqui custa caro.
    pub avisos: Vec<String>,
    /// De onde este `Config` foi lido. `None` quando veio de JSON avulso.
    ///
    /// E o que permite a tela GRAVAR de volta no mesmo arquivo: sem o caminho,
    /// `gravar_campos` nao tem onde escrever e recusa com a explicacao.
    pub caminho: Option<PathBuf>,
}

/// Campos de primeiro nivel que o `config.json` pode trazer.
///
/// Os que comecam com `_` sao comentario -- o JSON nao tem comentario, e os
/// exemplos usam `_web`, `_backup` e afins para explicar a secao seguinte.
// `lgpd` entrou aqui junto com `telemetria`, e nao por capricho: a secao ja
// existia em `SECOES_CONHECIDAS` e era lida por `Lgpd::de_json`, mas faltava
// no primeiro nivel -- quem a escrevesse no arquivo levava um "campo que este
// servidor nao conhece" sobre um campo que ele le e obedece. Aviso falso gasta
// a confianca do aviso verdadeiro.
const CAMPOS_CONHECIDOS: [&str; 27] = [
    "bind",
    "base",
    "token",
    "max_linhas",
    "log_acessos",
    "ips_permitidos",
    "conexoes_max",
    "recursos",
    "timeout_s",
    "somente_leitura",
    "espelho",
    "replicacao",
    "cluster",
    "root",
    "usuarios",
    "seguranca",
    "web",
    "backup",
    "alertas",
    "dblink",
    "jobs",
    "cifra",
    "cifra_fio",
    "idioma",
    "lgpd",
    "telemetria",
    "profiler",
];

/// O que cada secao conhecida aceita por dentro.
///
/// O aviso de campo estranho so olhava o primeiro nivel: um
/// `recursos.cache_pagina` (sem o `s`) passava calado, e o campo escrito
/// errado dentro de secao e exatamente o mais provavel -- e o mais dificil de
/// achar depois. Ficam FORA `seguranca`, `replicacao`, `root` e `usuarios`:
/// as duas primeiras estao ganhando campos novos por outras frentes nesta
/// rodada, e um aviso falso de "campo desconhecido" seria pior que a lacuna;
/// as duas ultimas tem chaves livres (bases, tabelas).
const SECOES_CONHECIDAS: [(&str, &[&str]); 10] = [
    (
        "recursos",
        &[
            "durabilidade",
            "exclusao_na_janela",
            "lote_operacoes",
            "lote_milissegundos",
            "cache_paginas",
            "memoria_max_mb",
            "threads",
            "cpu_percentual",
            "conexoes_max",
            "carga_prazo_min",
            "transacao_prazo_min",
            "transacao_max_linhas",
            "transacao_lock_timeout_ms",
            "transacao_statement_ms",
            "usuarios_max",
            "diario_volume_mib",
        ],
    ),
    ("web", &["ligado", "bind", "sessao_minutos", "servidores"]),
    (
        "backup",
        &[
            "agendado",
            "hora",
            "cada_horas",
            "destino",
            "zip",
            "database",
            "admin",
            "manter",
        ],
    ),
    (
        "alertas",
        &[
            "ligado",
            "livre_minimo_percentual",
            "livre_minimo_mb",
            "checar_minutos",
            "repetir_horas",
            "caminhos",
            "email",
        ],
    ),
    (
        "alertas.email",
        &[
            "ligado",
            // Da frente dos jobs: liga o aviso por e-mail de job que falhou ou
            // parou. Sem ele na lista, o verificador novo -- que passou a olhar
            // o INTERIOR das secoes -- acusava campo estranho num exemplo que
            // esta certo.
            "avisar_jobs",
            "servidor",
            "porta",
            "de",
            "para",
            "usuario",
            "senha",
            "senha_env",
            "assunto",
            "timeout_s",
        ],
    ),
    (
        "cifra",
        &[
            "ligada",
            "senha",
            "senha_env",
            "iteracoes",
            "modo",
            "salto",
            "separador",
        ],
    ),
    (
        "cifra_fio",
        &[
            "ligada",
            "exigir",
            "chave_privada",
            "chave_privada_env",
            "arquivo",
        ],
    ),
    ("lgpd", &["alteracoes", "acessos"]),
    (
        "telemetria",
        &[
            "cor_normal",
            "cor_alto",
            "cor_stress",
            "cor_encerrando",
            "alto_uso_ms",
            "stress_ms",
        ],
    ),
    ("profiler", &["arquivo_mib", "arquivos"]),
];

/// O que o arquivo trouxe e o servidor nao sabe ler.
fn chaves_estranhas(j: &Json) -> Vec<String> {
    let mut fora: Vec<String> = j
        .chaves()
        .into_iter()
        .filter(|k| !k.starts_with('_') && !CAMPOS_CONHECIDOS.contains(k))
        .map(str::to_string)
        .collect();
    for (secao, conhecidos) in SECOES_CONHECIDAS {
        let Some(s) = secao.split('.').try_fold(j, |o, parte| o.campo(parte)) else {
            continue;
        };
        for k in s.chaves() {
            if !k.starts_with('_') && !conhecidos.contains(&k) {
                fora.push(format!("{secao}.{k}"));
            }
        }
    }
    fora
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bind: format!("0.0.0.0:{PORTA_PADRAO}"),
            base: PathBuf::from("dados"),
            token: String::new(),
            max_linhas: 1_000,
            log_acessos: PathBuf::from("acessos.log"),
            ips_permitidos: Vec::new(),
            conexoes_max: 64,
            recursos: Recursos::default(),
            timeout_s: 30,
            somente_leitura: false,
            espelho: false,
            replicacao: Replicacao::default(),
            cluster: None,
            cadastro: Cadastro::default(),
            politica: Politica::default(),
            blacklist: PathBuf::from("blacklist.json"),
            web: Web::default(),
            backup: Backup::default(),
            alertas: Alertas::default(),
            dblink: PathBuf::from("dblink.json"),
            jobs: PathBuf::from("jobs.json"),
            cifra: Cifra::default(),
            cifra_fio: CifraFio::default(),
            lgpd: Lgpd::default(),
            telemetria: Painel::default(),
            profiler: PerfilEmDisco::default(),
            idioma: String::new(),
            estranhas: Vec::new(),
            avisos: Vec::new(),
            caminho: None,
        }
    }
}

impl Config {
    /// Le o `config.json` do caminho informado.
    pub fn ler(caminho: impl AsRef<Path>) -> Result<Config> {
        let caminho = caminho.as_ref();
        let texto = std::fs::read_to_string(caminho).map_err(|e| {
            PhxError::NaoEncontrado(format!("nao consegui ler {}: {e}", caminho.display()))
        })?;
        let json = Json::analisar(&texto)?;
        let mut c = Config::de_json(&json)?;
        c.caminho = Some(caminho.to_path_buf());
        // Caminhos relativos valem a partir do diretorio do config.json.
        if let Some(dir) = caminho.parent().filter(|d| !d.as_os_str().is_empty()) {
            if c.base.is_relative() {
                c.base = dir.join(&c.base);
            }
            if c.log_acessos.is_relative() {
                c.log_acessos = dir.join(&c.log_acessos);
            }
            if c.blacklist.is_relative() {
                c.blacklist = dir.join(&c.blacklist);
            }
            if c.backup.destino.is_relative() {
                c.backup.destino = dir.join(&c.backup.destino);
            }
        }
        c.validar()?;
        // A chave do cofre entra AQUI, e nao la no servidor, por uma razao
        // pratica: `ler` e o unico caminho por onde um `config.json` vira
        // configuracao viva, e um campo que so o servidor aplicasse nao valeria
        // para a CLI, que le o mesmo arquivo e precisa da mesma chave para
        // abrir o mesmo diario. Campo de configuracao que so metade do
        // programa le e a mesma armadilha do campo que ninguem le.
        c.cifra.aplicar()?;
        c.recursos.aplicar();
        c.lgpd.aplicar();
        Ok(c)
    }

    pub fn de_json(j: &Json) -> Result<Config> {
        let padrao = Config::default();
        let mut avisos: Vec<String> = Vec::new();
        let rep = match j.campo("replicacao") {
            None => Replicacao::default(),
            Some(r) => Replicacao {
                papel: Papel::de_texto(r.texto_ou("papel", "isolado"))?,
                // "escuta" e o nome antigo de "envio". Continua valendo:
                // config que ja existe nao pode parar de subir por renomeacao.
                envio: r
                    .texto_ou("envio", r.texto_ou("escuta", ""))
                    .trim()
                    .to_string(),
                retorno: r.texto_ou("retorno", "").trim().to_string(),
                id_servidor: r.texto_ou("id_servidor", "").to_string(),
                replicas_autorizadas: r.textos("replicas_autorizadas"),
                origens: r
                    .campo("origens")
                    .and_then(Json::lista)
                    .map(|l| {
                        l.iter()
                            .map(|o| Origem {
                                nome: o.texto_ou("nome", "origem").to_string(),
                                host: o.texto_ou("host", "127.0.0.1").to_string(),
                                porta: o.inteiro_ou("porta", PORTA_PADRAO as i64) as u16,
                                token: o.texto_ou("token", "").to_string(),
                                databases: o.textos("databases"),
                                reconectar_em: o.inteiro_ou("reconectar_em", 10).max(1) as u64,
                                usuario: o.texto_ou("usuario", "").trim().to_string(),
                                senha_hash: o.texto_ou("senha_hash", "").trim().to_string(),
                                senha: o.texto_ou("senha", "").to_string(),
                                cada_minutos: o.inteiro_ou("cada_minutos", 0).max(0) as u64,
                                hora: o.texto_ou("hora", "").trim().to_string(),
                                cifra: o.booleano_ou("cifra", false),
                                chave_do_fio: o.texto_ou("chave_do_fio", "").trim().to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                imagem_da_linha: r.booleano_ou(
                    "imagem_da_linha",
                    // O padrao segue o papel: quem PRECISA dela (source e
                    // multi) liga, o resto nao.
                    Papel::de_texto(r.texto_ou("papel", "isolado"))?.exige_imagem(),
                ),
            },
        };
        let mut rep = rep;
        let cluster = Cluster::de_json(j, &rep.id_servidor)?;
        if cluster.is_some() {
            // Num cluster, QUALQUER no pode ser promovido -- entao todo no
            // precisa da imagem no diario, e nao so o source. O padrao vira
            // ligado; quem desligar de proposito esta pedindo um no que nao
            // pode assumir, e isso e contradicao, nao configuracao.
            match j
                .campo("replicacao")
                .and_then(|r| r.campo("imagem_da_linha"))
            {
                Some(Json::Bool(false)) => {
                    return Err(PhxError::Esquema(
                        "cluster com replicacao.imagem_da_linha desligada: um no \
                         promovido precisa da imagem no diario para as replicas \
                         continuarem (apague o campo ou ligue-o)"
                            .into(),
                    ))
                }
                _ => rep.imagem_da_linha = true,
            }
        }

        let recursos = Recursos::de_json(
            j,
            j.inteiro_ou("conexoes_max", padrao.conexoes_max as i64)
                .max(1) as usize,
        )?;
        Ok(Config {
            bind: j.texto_ou("bind", &padrao.bind).to_string(),
            base: PathBuf::from(j.texto_ou("base", "dados")),
            token: j.texto_ou("token", "").to_string(),
            max_linhas: j.inteiro_ou("max_linhas", padrao.max_linhas as i64).max(1) as u64,
            log_acessos: PathBuf::from(j.texto_ou("log_acessos", "acessos.log")),
            ips_permitidos: j.textos("ips_permitidos"),
            // O ESPELHO do valor de `recursos`, e nao uma segunda leitura do
            // topo. Era uma segunda leitura, e a promessa "dentro de recursos
            // ele ganha" mentia: o laco de aceitacao le este campo, e um
            // `recursos.conexoes_max: 99` ficava so na tela.
            conexoes_max: recursos.conexoes_max,
            recursos,
            timeout_s: j.inteiro_ou("timeout_s", padrao.timeout_s as i64).max(1) as u64,
            somente_leitura: j.booleano_ou("somente_leitura", false),
            espelho: j.booleano_ou("espelho", false),
            replicacao: rep,
            cluster,
            cadastro: Cadastro::de_json(j)?,
            politica: match j.campo("seguranca") {
                Some(seg) => Politica::de_json(seg),
                None => Politica::default(),
            },
            blacklist: PathBuf::from(
                j.campo("seguranca")
                    .map(|seg| seg.texto_ou("blacklist", "blacklist.json"))
                    .unwrap_or("blacklist.json"),
            ),
            web: Web::de_json(j),
            backup: Backup::de_json(j)?,
            alertas: Alertas::de_json(j)?,
            dblink: PathBuf::from(j.texto_ou("dblink", "dblink.json")),
            jobs: PathBuf::from(j.texto_ou("jobs", "jobs.json")),
            cifra: Cifra::de_json(j),
            cifra_fio: CifraFio::de_json(j),
            lgpd: Lgpd::de_json(j),
            telemetria: Painel::de_json(j, &mut avisos),
            profiler: PerfilEmDisco::de_json(j),
            idioma: {
                // O valor aceito e o NOME de uma coluna da tabela de
                // mensagens. Desconhecido nao derruba o servidor -- vira
                // aviso e cai no portugues, que e o texto de fabrica.
                let pedido = j.texto_ou("idioma", "").trim().to_string();
                if pedido.is_empty() || crate::mensagens::IDIOMAS.contains(&pedido.as_str()) {
                    pedido
                } else {
                    avisos.push(format!(
                        "idioma {pedido:?} nao existe; use {}. Ficou Portugues.",
                        crate::mensagens::IDIOMAS.join(", ")
                    ));
                    String::new()
                }
            },
            estranhas: chaves_estranhas(j),
            avisos,
            caminho: None,
        })
    }

    fn validar(&self) -> Result<()> {
        if self.token.trim().is_empty() {
            return Err(PhxError::Esquema(
                "config.json sem token: preencha o campo \"token\" antes de subir o servidor"
                    .into(),
            ));
        }
        self.endereco()?;
        if self.web.ligado {
            let web = self.web.endereco()?;
            if web == self.endereco()? {
                return Err(PhxError::Esquema(format!(
                    "web.bind e bind apontam para o mesmo endereco ({web}): a interface precisa de uma porta so dela"
                )));
            }
        }
        // Cada porta de replicacao contra a de dados, a da web e a outra.
        // Duas portas no mesmo endereco nao sobem, e descobrir isso no
        // arranque e melhor do que descobrir com uma delas calada.
        let mut ocupadas = vec![("bind", self.endereco()?)];
        if self.web.ligado {
            ocupadas.push(("web.bind", self.web.endereco()?));
        }
        for (rotulo, texto) in self.replicacao.portas() {
            let alvo = Replicacao::resolver(rotulo, texto)?;
            if let Some((quem, _)) = ocupadas.iter().find(|(_, e)| *e == alvo) {
                return Err(PhxError::Esquema(format!(
                    "replicacao.{rotulo} e {quem} apontam para o mesmo endereco ({alvo})"
                )));
            }
            ocupadas.push((rotulo, alvo));
        }
        // Duas regras que se somam: a lista de origens e exigida de TODO papel
        // que puxa (replica, read replica, spare, multi) -- e nao so do
        // `replica`, como era antes de os papeis novos existirem --, MENOS
        // quando ha cluster, porque ai a origem e o master CORRENTE descoberto
        // pelo pulso, e uma lista fixa apontaria para o master de ontem.
        if self.cluster.is_none()
            && self.replicacao.papel.puxa_de_origem()
            && self.replicacao.origens.is_empty()
        {
            return Err(PhxError::Esquema(format!(
                "papel {} exige ao menos uma origem em replicacao.origens",
                self.replicacao.papel.nome()
            )));
        }
        if self.replicacao.papel == Papel::Multi {
            // O id e a identidade dos eventos: sem ele nao ha como marcar a
            // origem, e sem a origem o evento que A aplicou de B voltaria
            // para B num laco infinito.
            if self.replicacao.id_servidor.trim().is_empty() {
                return Err(PhxError::Esquema(
                    "papel multi exige replicacao.id_servidor: e ele que marca a \
                     origem de cada evento e impede o laco infinito"
                        .into(),
                ));
            }
            if self.somente_leitura {
                return Err(PhxError::Esquema(
                    "papel multi com somente_leitura e contradicao: multi existe \
                     para receber escrita nos dois servidores"
                        .into(),
                ));
            }
            if !self.replicacao.imagem_da_linha {
                return Err(PhxError::Esquema(
                    "papel multi exige replicacao.imagem_da_linha: a identidade \
                     entre servidores e a chave, e a chave mora dentro da imagem"
                        .into(),
                ));
            }
        }
        for o in &self.replicacao.origens {
            if !o.hora.is_empty() && Backup::minuto_do_dia(&o.hora).is_none() {
                return Err(PhxError::Esquema(format!(
                    "origem {}: hora invalida {:?} (use \"HH:MM\", 24 horas)",
                    o.nome, o.hora
                )));
            }
        }
        if let Some(c) = &self.cluster {
            c.validar(&self.replicacao)?;
        }
        Ok(())
    }

    pub fn endereco(&self) -> Result<SocketAddr> {
        endereco_de(&self.bind)
    }

    /// O IP tem permissao de conectar? Lista vazia libera todos.
    pub fn ip_permitido(&self, ip: &str) -> bool {
        self.ips_permitidos.is_empty() || self.ips_permitidos.iter().any(|p| p == ip)
    }

    /// Comparacao de token em tempo constante, para nao vazar o segredo pelo
    /// tempo de resposta.
    pub fn token_confere(&self, oferecido: &str) -> bool {
        let a = self.token.as_bytes();
        let b = oferecido.as_bytes();
        if a.len() != b.len() {
            return false;
        }
        let mut diferenca = 0u8;
        for (x, y) in a.iter().zip(b.iter()) {
            diferenca |= x ^ y;
        }
        diferenca == 0
    }

    /// A configuracao como resposta de protocolo, SEM segredo nenhum dentro.
    ///
    /// # O formato espelha o arquivo
    ///
    /// As secoes saem com os MESMOS nomes do `config.json` -- `seguranca`,
    /// `web`, `backup`, `replicacao` --, e nao num achatado proprio da
    /// resposta. A versao achatada mentia por omissao: a tela lia
    /// `c.seguranca.bases_proibidas`, o campo nao vinha, e uma base proibida
    /// aparecia como "nao" -- que e pior que nao mostrar nada.
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("bind", Json::texto_de(&self.bind)),
            ("base", Json::texto_de(self.base.display().to_string())),
            // Onde os dados moram DE VERDADE. O campo "base" pode ser
            // relativo, e relativo a que depende de onde o servidor foi
            // iniciado -- que e a duvida que a tela precisa tirar.
            (
                "base_absoluta",
                Json::texto_de(
                    std::fs::canonicalize(&self.base)
                        .unwrap_or_else(|_| self.base.clone())
                        .display()
                        .to_string(),
                ),
            ),
            ("token", Json::texto_de("(oculto)")),
            ("max_linhas", Json::de_u64(self.max_linhas)),
            ("timeout_s", Json::de_u64(self.timeout_s)),
            (
                "log_acessos",
                Json::texto_de(self.log_acessos.display().to_string()),
            ),
            (
                "ips_permitidos",
                Json::Lista(self.ips_permitidos.iter().map(Json::texto_de).collect()),
            ),
            ("conexoes_max", Json::de_u64(self.conexoes_max as u64)),
            ("recursos", self.recursos.para_json()),
            ("somente_leitura", Json::Bool(self.somente_leitura)),
            ("espelho", Json::Bool(self.espelho)),
            (
                "replicacao",
                Json::objeto(vec![
                    ("papel", Json::texto_de(self.replicacao.papel.nome())),
                    ("envio", Json::texto_de(&self.replicacao.envio)),
                    ("retorno", Json::texto_de(&self.replicacao.retorno)),
                    ("id_servidor", Json::texto_de(&self.replicacao.id_servidor)),
                    // O que a tela da replicacao precisa para dizer a verdade:
                    // sem a imagem no diario o servidor tem papel de source e
                    // nao replica, e a tela diria que esta tudo pronto.
                    (
                        "imagem_da_linha",
                        Json::Bool(self.replicacao.imagem_da_linha),
                    ),
                    (
                        "replicas_autorizadas",
                        Json::Lista(
                            self.replicacao
                                .replicas_autorizadas
                                .iter()
                                .map(Json::texto_de)
                                .collect(),
                        ),
                    ),
                    (
                        "origens",
                        Json::Lista(
                            self.replicacao
                                .origens
                                .iter()
                                .map(|o| {
                                    Json::objeto(vec![
                                        ("nome", Json::texto_de(&o.nome)),
                                        ("host", Json::texto_de(&o.host)),
                                        ("porta", Json::de_u64(o.porta as u64)),
                                        ("usuario", Json::texto_de(&o.usuario)),
                                        ("reconectar_em", Json::de_u64(o.reconectar_em)),
                                        (
                                            "databases",
                                            Json::Lista(
                                                o.databases.iter().map(Json::texto_de).collect(),
                                            ),
                                        ),
                                        // A senha NAO sai daqui, nem o hash: a
                                        // tela nao precisa dela e a resposta do
                                        // protocolo nunca carrega credencial.
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ]),
            ),
            (
                "seguranca",
                Json::objeto(vec![
                    (
                        "comandos_proibidos",
                        Json::Lista(
                            self.politica
                                .comandos_proibidos
                                .iter()
                                .map(Json::texto_de)
                                .collect(),
                        ),
                    ),
                    (
                        "bases_proibidas",
                        Json::Lista(
                            self.politica
                                .bases_proibidas
                                .iter()
                                .map(Json::texto_de)
                                .collect(),
                        ),
                    ),
                    (
                        "tentativas_ate_bloquear",
                        Json::de_u64(self.politica.tentativas_ate_bloquear as u64),
                    ),
                    ("janela_minutos", Json::de_u64(self.politica.janela_minutos)),
                    (
                        "bloqueio_minutos",
                        Json::de_u64(self.politica.bloqueio_minutos),
                    ),
                    (
                        "blacklist",
                        Json::texto_de(self.blacklist.display().to_string()),
                    ),
                    (
                        "firewall",
                        Json::Bool(
                            self.politica
                                .firewall
                                .as_ref()
                                .map(|f| f.ligado)
                                .unwrap_or(false),
                        ),
                    ),
                ]),
            ),
            (
                "web",
                Json::objeto(vec![
                    ("ligado", Json::Bool(self.web.ligado)),
                    ("bind", Json::texto_de(&self.web.bind)),
                    ("sessao_minutos", Json::de_u64(self.web.sessao_minutos)),
                    (
                        "servidores",
                        Json::Lista(self.web.servidores.iter().map(Json::texto_de).collect()),
                    ),
                ]),
            ),
            (
                "backup",
                Json::objeto(vec![
                    ("agendado", Json::Bool(self.backup.agendado)),
                    ("hora", Json::texto_de(&self.backup.hora)),
                    ("cada_horas", Json::de_u64(self.backup.cada_horas)),
                    (
                        "destino",
                        Json::texto_de(self.backup.destino.display().to_string()),
                    ),
                    ("zip", Json::Bool(self.backup.zip)),
                    ("database", Json::texto_de(&self.backup.database)),
                    ("admin", Json::texto_de(&self.backup.admin)),
                    ("manter", Json::de_u64(self.backup.manter as u64)),
                ]),
            ),
            (
                "usuarios",
                Json::de_u64(
                    (self.cadastro.usuarios.len() + usize::from(self.cadastro.root.is_some()))
                        as u64,
                ),
            ),
            ("alertas", self.alertas.para_json()),
            ("dblink", Json::texto_de(self.dblink.display().to_string())),
            ("jobs", Json::texto_de(self.jobs.display().to_string())),
            ("cifra", self.cifra.para_json()),
            ("cifra_fio", self.cifra_fio.para_json()),
            ("lgpd", self.lgpd.para_json()),
            // As cores VAO para a tela por aqui -- o mesmo caminho de todo o
            // resto da configuracao. A tela de configuracao nao le arquivo, e
            // o painel de bolhas as recebe na propria resposta da telemetria,
            // ao lado dos limiares que decidiram o nivel.
            ("telemetria", self.telemetria.para_json()),
            ("profiler", self.profiler.para_json()),
            // O idioma EM USO, ja resolvido: vazio no arquivo vira Portugues
            // aqui, para a tela nao ter de repetir a regra do fallback.
            (
                "idioma",
                Json::texto_de(if self.idioma.is_empty() {
                    crate::mensagens::IDIOMAS[0]
                } else {
                    &self.idioma
                }),
            ),
            // O aviso de campo desconhecido tem de chegar na TELA, e nao so no
            // stderr do arranque: quem edita pela interface nunca ve o
            // terminal do servidor.
            (
                "estranhas",
                Json::Lista(self.estranhas.iter().map(Json::texto_de).collect()),
            ),
            // O que a tela pode gravar, dito pelo SERVIDOR.
            //
            // A tela monta o formulario desta lista em vez de trazer a sua --
            // duas listas divergem no primeiro campo que alguem acrescentar de
            // um lado so, e a que envelhece e sempre a da tela, que ninguem
            // compila. E a mesma regra do catalogo das operacoes.
            ("editaveis", editaveis_json()),
        ])
    }
}

/// Os campos editaveis como dado, para a tela montar o formulario.
pub fn editaveis_json() -> Json {
    Json::Lista(
        CAMPOS_EDITAVEIS
            .iter()
            .map(|(campo, tipo, quente)| {
                Json::objeto(vec![
                    ("campo", Json::texto_de(*campo)),
                    ("tipo", Json::texto_de(tipo.nome())),
                    ("a_quente", Json::Bool(*quente)),
                ])
            })
            .collect(),
    )
}

/// O tipo que um campo editavel aceita. Existe porque os leitores usam
/// `inteiro_ou`/`texto_ou`, que caem no PADRAO quando o tipo nao bate: sem
/// esta conferencia, gravar `"max_linhas": "abc"` passaria na validacao (o
/// leitor ignora e usa 1.000) e deixaria no arquivo um valor que nunca vale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoDoCampo {
    Inteiro,
    Numero,
    Booleano,
    Texto,
    /// `#rrggbb` ou vazio. Ver [`cor_valida`].
    ///
    /// E um tipo proprio, e nao um `Texto` com conferencia solta, porque o
    /// tipo e o que a TELA le para saber que ali vai um seletor de cor com a
    /// amostra da bolha ao lado. Uma lista de "estes campos sao cores" escrita
    /// no JavaScript envelheceria calada no dia em que entrasse a quinta cor
    /// -- e a lista de campos ja vem do servidor justamente por isso.
    Cor,
}

impl TipoDoCampo {
    fn confere(self, v: &Json) -> bool {
        match self {
            TipoDoCampo::Inteiro => v.inteiro().is_some(),
            TipoDoCampo::Numero => v.numero().is_some(),
            TipoDoCampo::Booleano => v.booleano().is_some(),
            TipoDoCampo::Texto => v.texto().is_some(),
            TipoDoCampo::Cor => v.texto().is_some_and(cor_valida),
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            TipoDoCampo::Inteiro => "inteiro",
            TipoDoCampo::Numero => "numero",
            TipoDoCampo::Booleano => "booleano",
            TipoDoCampo::Texto => "texto",
            TipoDoCampo::Cor => "cor",
        }
    }
}

/// Os campos que a tela pode gravar: (campo, tipo, aplica a quente?).
///
/// `true` no fim = o servidor aplica sem reiniciar; `false` = fica gravado e
/// vale no proximo arranque -- e a tela diz isso AO LADO do campo, em vez de
/// prometer efeito que nao vem.
///
/// O que NAO esta aqui nao se grava pela porta web, e a ausencia e decisao,
/// nao esquecimento: `token`, `seguranca.*`, `usuarios`/`root`, `cifra.*`,
/// `alertas.email.*` e `replicacao.*` continuam sendo edicao do arquivo --
/// uma sessao roubada nao abre o firewall, nao cria supervisor, nao vira a
/// replica para outro source e nao mexe em campo que carrega credencial. As
/// listas (`ips_permitidos`, `web.servidores`, `alertas.caminhos`) tambem
/// ficam de fora por serem politica de rede da mesma familia.
pub const CAMPOS_EDITAVEIS: &[(&str, TipoDoCampo, bool)] = &[
    // O efeito a quente do bind quem da e a op `servico_subir`; gravar aqui e
    // o que faz a troca sobreviver ao arranque.
    ("bind", TipoDoCampo::Texto, false),
    ("max_linhas", TipoDoCampo::Inteiro, true),
    ("timeout_s", TipoDoCampo::Inteiro, false),
    ("somente_leitura", TipoDoCampo::Booleano, true),
    ("espelho", TipoDoCampo::Booleano, true),
    ("recursos.durabilidade", TipoDoCampo::Texto, false),
    // A quente: quem grava e `Recursos::aplicar`, e o global vale para a
    // exclusao seguinte -- inclusive nesta mesma conexao.
    ("recursos.exclusao_na_janela", TipoDoCampo::Booleano, true),
    ("recursos.lote_operacoes", TipoDoCampo::Inteiro, false),
    ("recursos.lote_milissegundos", TipoDoCampo::Inteiro, false),
    // O cache vale para o proximo abrir de tabela -- e a tabela abre e fecha
    // a cada operacao, entao na pratica e imediato.
    ("recursos.cache_paginas", TipoDoCampo::Inteiro, true),
    ("recursos.diario_volume_mib", TipoDoCampo::Inteiro, true),
    ("recursos.threads", TipoDoCampo::Inteiro, true),
    ("recursos.cpu_percentual", TipoDoCampo::Inteiro, true),
    ("recursos.conexoes_max", TipoDoCampo::Inteiro, false),
    ("recursos.carga_prazo_min", TipoDoCampo::Inteiro, false),
    // Os dois da transacao NAO valem a quente, e a razao e a mesma dos outros
    // tetos de recurso: mudar o teto no meio de uma transacao aberta mudaria a
    // regra debaixo de quem ja empilhou metade do trabalho.
    ("recursos.transacao_prazo_min", TipoDoCampo::Inteiro, false),
    ("recursos.transacao_max_linhas", TipoDoCampo::Inteiro, false),
    (
        "recursos.transacao_lock_timeout_ms",
        TipoDoCampo::Inteiro,
        false,
    ),
    (
        "recursos.transacao_statement_ms",
        TipoDoCampo::Inteiro,
        false,
    ),
    ("recursos.memoria_max_mb", TipoDoCampo::Inteiro, false),
    ("recursos.usuarios_max", TipoDoCampo::Inteiro, false),
    ("web.sessao_minutos", TipoDoCampo::Inteiro, false),
    ("backup.agendado", TipoDoCampo::Booleano, false),
    ("backup.hora", TipoDoCampo::Texto, false),
    ("backup.cada_horas", TipoDoCampo::Inteiro, false),
    ("backup.destino", TipoDoCampo::Texto, false),
    ("backup.zip", TipoDoCampo::Booleano, false),
    ("backup.manter", TipoDoCampo::Inteiro, false),
    ("alertas.ligado", TipoDoCampo::Booleano, false),
    (
        "alertas.livre_minimo_percentual",
        TipoDoCampo::Numero,
        false,
    ),
    ("alertas.livre_minimo_mb", TipoDoCampo::Inteiro, false),
    ("alertas.checar_minutos", TipoDoCampo::Inteiro, false),
    ("alertas.repetir_horas", TipoDoCampo::Inteiro, false),
    // As cores do painel de bolhas valem A QUENTE, e isso e o ponto: cor se
    // escolhe VENDO, e uma cor que so aparecesse depois de reiniciar o
    // servidor seria escolhida no escuro. O painel a recebe na resposta
    // seguinte da telemetria, dois segundos depois de salvar.
    ("telemetria.cor_normal", TipoDoCampo::Cor, true),
    ("telemetria.cor_alto", TipoDoCampo::Cor, true),
    ("telemetria.cor_stress", TipoDoCampo::Cor, true),
    ("telemetria.cor_encerrando", TipoDoCampo::Cor, true),
    // A quente: `gravar_campos` leva os dois ao profiler vivo, e eles valem
    // para o arquivo CORRENTE -- quem viu o arquivo crescendo na tela quer o
    // teto agora, e nao no proximo `profiler_ligar`.
    ("profiler.arquivo_mib", TipoDoCampo::Inteiro, true),
    ("profiler.arquivos", TipoDoCampo::Inteiro, true),
    ("telemetria.alto_uso_ms", TipoDoCampo::Inteiro, true),
    ("telemetria.stress_ms", TipoDoCampo::Inteiro, true),
];

/// O valor de `"secao.campo"` dentro de um JSON, ou `None` se nao existe.
pub fn valor_em(j: &Json, campo: &str) -> Option<Json> {
    campo
        .split('.')
        .try_fold(j, |o, parte| o.campo(parte))
        .cloned()
}

/// O que o ARQUIVO diz e ainda nao esta valendo neste processo.
///
/// # Por que a tela precisa disto
///
/// Gravar `timeout_s` pela tela grava no arquivo e NAO muda o servidor -- esse
/// campo so vale no proximo arranque, e a tela ja diz isso ao lado dele. Mas
/// no redesenho seguinte a tela lia a configuracao VIVA, e o campo voltava com
/// o valor velho, calado: quem acabou de digitar 90 via 45 de novo e nao tinha
/// como saber que o 90 estava gravado. Achado exercitando a tela, que e a
/// unica forma de achar isso.
///
/// Com este mapa a tela mostra o que esta no arquivo e avisa o que ainda vale.
pub fn divergencias_do_arquivo(caminho: &Path, vivo: &Json) -> Vec<(String, Json)> {
    let Ok(texto) = std::fs::read_to_string(caminho) else {
        return Vec::new();
    };
    let Ok(arquivo) = Json::analisar(&texto) else {
        return Vec::new();
    };
    let mut fora = Vec::new();
    for (campo, _, _) in CAMPOS_EDITAVEIS {
        let no_arquivo = valor_em(&arquivo, campo);
        let (Some(a), Some(v)) = (no_arquivo, valor_em(vivo, campo)) else {
            continue;
        };
        // Numero e numero: o arquivo pode trazer `10` onde o vivo traz `10.0`,
        // e isso nao e divergencia nenhuma.
        let igual = match (&a, &v) {
            (Json::Numero(x), Json::Numero(y)) => x == y,
            _ => a == v,
        };
        if !igual {
            fora.push((campo.to_string(), a));
        }
    }
    fora
}

/// Este campo se grava pela tela? Devolve (tipo, aplica a quente).
pub fn campo_editavel(nome: &str) -> Option<(TipoDoCampo, bool)> {
    CAMPOS_EDITAVEIS
        .iter()
        .find(|(c, _, _)| *c == nome)
        .map(|(_, t, q)| (*t, *q))
}

impl Config {
    /// Grava campos escolhidos no `config.json`, atomicamente, e devolve o
    /// `Config` que o arquivo novo produz.
    ///
    /// # Le o arquivo de novo, em vez de reserializar o `Config` vivo
    ///
    /// E o que preserva os comentarios `_...`, a ordem das chaves e as secoes
    /// que este processo nao conhece -- inclusive blocos que outras frentes
    /// acrescentarem. So muda o que foi pedido; o resto sai byte a byte do que
    /// o leitor de JSON devolveu.
    ///
    /// # A validacao vem ANTES da gravacao
    ///
    /// A arvore alterada passa por `de_json` + `validar` primeiro: valor que
    /// nao subiria o servidor nao entra no arquivo. E cada valor e conferido
    /// contra o TIPO do campo, porque os leitores usam `inteiro_ou`, que cai
    /// no padrao em silencio quando o tipo nao bate -- e um campo gravado que
    /// nunca vale e a mentira desta tela.
    pub fn gravar_campos(caminho: &Path, mudancas: &[(String, Json)]) -> Result<Config> {
        if mudancas.is_empty() {
            return Err(PhxError::Esquema(
                "nada a gravar: mande \"campos\" com ao menos um".into(),
            ));
        }
        for (campo, valor) in mudancas {
            let Some((tipo, _)) = campo_editavel(campo) else {
                return Err(PhxError::Autorizacao(format!(
                    "o campo {campo:?} nao se grava pela tela; edite o config.json"
                )));
            };
            if !tipo.confere(valor) {
                return Err(PhxError::Esquema(format!(
                    "{campo:?} espera {}, veio {}",
                    tipo.nome(),
                    valor.escrever()
                )));
            }
        }

        let texto = std::fs::read_to_string(caminho).map_err(|e| {
            PhxError::NaoEncontrado(format!("nao consegui ler {}: {e}", caminho.display()))
        })?;
        let mut arvore = Json::analisar(&texto)?;
        for (campo, valor) in mudancas {
            match campo.split_once('.') {
                None => arvore.definir(campo, valor.clone()),
                Some((secao, resto)) => {
                    let mut s = match arvore.campo(secao) {
                        None => Json::Objeto(Vec::new()),
                        Some(Json::Objeto(pares)) => Json::Objeto(pares.clone()),
                        Some(outro) => {
                            return Err(PhxError::Esquema(format!(
                                "{secao:?} no arquivo nao e um objeto: {}",
                                outro.escrever()
                            )))
                        }
                    };
                    s.definir(resto, valor.clone());
                    arvore.definir(secao, s);
                }
            }
        }

        let mut novo = Config::de_json(&arvore)?;
        novo.caminho = Some(caminho.to_path_buf());
        novo.validar()?;

        // O TEXTO a gravar: a troca cirurgica primeiro.
        //
        // Reserializar a arvore preserva valor, ordem e comentario, e perde a
        // FORMA -- linhas em branco entre secoes somem e `["a","b"]` vira tres
        // linhas. Num arquivo escrito a mao isso e devolver o trabalho de
        // alguem reformatado, e num controle de versao e um diff ilegivel.
        // Entao cada campo e trocado NO TEXTO, e o resto sai byte a byte.
        //
        // A reserializacao continua como reserva para o caso que a troca
        // cirurgica se recusa a fazer: campo (ou secao) que ainda nao esta no
        // arquivo. Inserir texto exigiria adivinhar a indentacao de quem
        // escreveu, e adivinhar errado e o mesmo estrago que reformatar.
        let mut corpo = texto.clone();
        for (campo, valor) in mudancas {
            let partes: Vec<&str> = campo.split('.').collect();
            match Json::texto_trocar(&corpo, &partes, valor) {
                Some(t) => corpo = t,
                None => {
                    corpo = arvore.escrever_identado();
                    corpo.push('\n');
                    break;
                }
            }
        }
        // Cinto: o texto que vai para o disco tem de dizer o mesmo que a
        // arvore que passou pela validacao. Se a edicao no texto divergir por
        // qualquer motivo, vale o reserializado -- que esta provado.
        match Json::analisar(&corpo) {
            Ok(conferido) if conferido == arvore => {}
            _ => {
                corpo = arvore.escrever_identado();
                corpo.push('\n');
            }
        }

        // O mesmo padrao do cadastro do DbLink: escreve inteiro num arquivo
        // temporario e troca com rename. Um corte de energia no meio deixa o
        // config.json antigo inteiro, e nao um pela metade -- que derrubaria o
        // proximo arranque.
        let temporario = caminho.with_extension("tmp");
        std::fs::write(&temporario, corpo)
            .map_err(|e| PhxError::Esquema(format!("nao gravei {}: {e}", temporario.display())))?;
        // O arquivo carrega o token e os hashes: o temporario herda as
        // permissoes do original em vez de nascer com as largas do umask.
        if let Ok(meta) = std::fs::metadata(caminho) {
            let _ = std::fs::set_permissions(&temporario, meta.permissions());
        }
        std::fs::rename(&temporario, caminho)
            .map_err(|e| PhxError::Esquema(format!("nao troquei {}: {e}", caminho.display())))?;
        Ok(novo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /* --------------------------------------------------------------- a cifra

    O que estes testes NAO fazem: ligar o cofre do processo. Ligar e o
    trabalho do `Cifra::aplicar`, e ele mexe num global que vale para o
    binario de teste inteiro -- provar isso pertence a
    `tests/cifra-pelo-config.rs`, que roda em outro processo. Aqui se prova a
    leitura do campo e o que ele NAO deixa sair. */

    /// Sem a secao `cifra`, nada muda -- e este e o teste que mais importa.
    #[test]
    fn sem_a_secao_cifra_nada_muda() {
        let j = Json::analisar(r#"{"token":"t"}"#).unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(!c.cifra.ligada, "a cifra nao pode nascer ligada");
        assert!(c.cifra.senha().is_empty());
        assert!(c.estranhas.is_empty());
        // E aplicar uma cifra desligada nao liga cofre nenhum.
        c.cifra.aplicar().unwrap();
        assert!(!phxsql_store::cofre::ligado());
    }

    /// **O comportamento velho.** Um `config.json` que nunca ouviu falar da
    /// cifra do fio nao exige nada de ninguem.
    ///
    /// E o teste que mais importa nesta frente: guarda nova entra PEDIDA.
    #[test]
    fn sem_a_secao_cifra_fio_nada_e_exigido() {
        let j = Json::analisar(r#"{"token":"t"}"#).unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(
            !c.cifra_fio.exigir,
            "sem a secao, o servidor passou a EXIGIR o tunel: todo cliente \
             velho para de funcionar na atualizacao"
        );
        // `ligada` NASCE ligada, e isso nao muda nada para ninguem: o aperto
        // so acontece se o cliente pedir, e cliente velho nunca pede.
        assert!(c.cifra_fio.ligada);
        assert!(c.estranhas.is_empty());
    }

    #[test]
    fn a_secao_cifra_fio_e_lida_e_nao_vira_campo_estranho() {
        let j = Json::analisar(
            r#"{"token":"t","cifra_fio":{"ligada":false,"exigir":true,
                 "arquivo":"/tmp/uma-chave.hex"}}"#,
        )
        .unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(!c.cifra_fio.ligada);
        assert!(c.cifra_fio.exigir);
        assert_eq!(c.cifra_fio.arquivo, PathBuf::from("/tmp/uma-chave.hex"));
        assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);

        // Campo escrito errado DENTRO da secao vira aviso, e nao silencio.
        let j = Json::analisar(r#"{"token":"t","cifra_fio":{"exigirr":true}}"#).unwrap();
        let c = Config::de_json(&j).unwrap();
        assert_eq!(c.estranhas, vec!["cifra_fio.exigirr".to_string()]);
    }

    /// A privada do fio nao sai pelo `para_json` (que a tela le) nem pelo
    /// `Debug` (que um `dbg!` apressado jogaria no log).
    #[test]
    fn a_privada_do_fio_nunca_sai() {
        let segredo = "1122334455667788112233445566778811223344556677881122334455667788";
        let j = Json::analisar(&format!(
            r#"{{"token":"t","cifra_fio":{{"chave_privada":"{segredo}"}}}}"#
        ))
        .unwrap();
        let c = Config::de_json(&j).unwrap();
        let texto = c.para_json().escrever();
        assert!(!texto.contains(segredo), "a privada vazou no para_json");
        assert!(
            texto.contains("cifra_fio"),
            "a secao sumiu da tela: {texto}"
        );
        assert!(!format!("{:?}", c.cifra_fio).contains(segredo));
        assert!(!format!("{c:?}").contains(segredo));

        // E ela e mesmo LIDA -- campo de configuracao sem leitor mente.
        let (privada, avisos) = c.cifra_fio.estatica(None).unwrap();
        assert!(avisos.is_empty());
        assert_eq!(phxsql_core::hash::para_hex(&privada), segredo);
    }

    /// A estatica nasce no arquivo, ao lado do `config.json`, e a SEGUNDA
    /// leitura devolve a mesma -- senao o pino de todo cliente quebraria a
    /// cada arranque.
    #[test]
    fn a_estatica_do_fio_nasce_no_arquivo_e_nao_muda() {
        let d = std::env::temp_dir().join(format!("phxsql-chave-do-fio-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let config = d.join("config.json");

        let cf = CifraFio::default();
        assert!(!d.join("chave-do-fio.hex").exists());
        let (primeira, avisos) = cf.estatica(Some(&config)).unwrap();
        assert!(avisos.is_empty(), "{avisos:?}");
        assert!(
            d.join("chave-do-fio.hex").exists(),
            "a estatica nao foi gravada ao lado do config"
        );
        let (segunda, _) = cf.estatica(Some(&config)).unwrap();
        assert_eq!(primeira, segunda, "a estatica mudou entre duas leituras");

        // Permissao 0600 na criacao: entre criar aberto e apertar ha uma
        // janela em que qualquer um le a chave.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let modo = std::fs::metadata(d.join("chave-do-fio.hex"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(modo, 0o600, "a chave do fio nasceu legivel por outros");
        }

        // Hexadecimal torto e ERRO com o caminho dentro, e nao chave sorteada
        // em silencio -- senao o pino do cliente pararia de bater sem motivo.
        std::fs::write(d.join("chave-do-fio.hex"), "nao sou hexadecimal").unwrap();
        let e = cf.estatica(Some(&config)).unwrap_err().to_string();
        assert!(e.contains("hexadecimal"), "{e}");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// O pino da origem: ausente e `None`, e torto e ERRO.
    ///
    /// Nunca `None` por engano -- um pino escrito errado que virasse "sem
    /// pino" desligaria em silencio exatamente a protecao que ele existe para
    /// dar.
    #[test]
    fn pino_torto_na_origem_e_erro_e_nao_ausencia() {
        let mut o = Origem {
            nome: "matriz".into(),
            host: "10.0.0.1".into(),
            porta: 5000,
            token: String::new(),
            databases: Vec::new(),
            reconectar_em: 10,
            usuario: String::new(),
            senha_hash: String::new(),
            senha: String::new(),
            cada_minutos: 0,
            hora: String::new(),
            cifra: true,
            chave_do_fio: String::new(),
        };
        assert!(o.pino_do_fio().unwrap().is_none());
        o.chave_do_fio = "abacaxi".into();
        assert!(o.pino_do_fio().is_err());
        o.chave_do_fio = "aa".repeat(31);
        assert!(o.pino_do_fio().is_err(), "31 bytes passaram por 32");
        o.chave_do_fio = "aa".repeat(32);
        assert_eq!(o.pino_do_fio().unwrap(), Some([0xaau8; 32]));
    }

    #[test]
    fn a_secao_cifra_e_lida_e_nao_vira_campo_estranho() {
        let j = Json::analisar(
            r#"{"token":"t","cifra":{"ligada":true,"senha":"abre-te sesamo","iteracoes":300000}}"#,
        )
        .unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(c.cifra.ligada);
        assert_eq!(c.cifra.senha(), "abre-te sesamo");
        assert_eq!(c.cifra.iteracoes, 300_000);
        assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    }

    #[test]
    fn a_senha_da_cifra_nunca_sai_em_json() {
        let j = Json::analisar(r#"{"token":"t","cifra":{"ligada":true,"senha":"abre-te sesamo"}}"#)
            .unwrap();
        let c = Config::de_json(&j).unwrap();
        let texto = c.para_json().escrever();
        assert!(!texto.contains("abre-te sesamo"), "a senha vazou: {texto}");
        assert!(texto.contains("(oculta)"));
    }

    /// UM teste para TODAS as credenciais do `config.json`, e nao um por
    /// campo.
    ///
    /// Ja ha tres testes especificos aqui -- a senha da cifra, a do rele e a
    /// do cluster --, e nenhum deles pega o campo que ALGUEM ACRESCENTAR
    /// AMANHA: cada um confere o segredo que ja conhece. E o mesmo desenho do
    /// portao de permissao, que e um so justamente para nao existir a
    /// operacao que ficou de fora.
    ///
    /// Aqui todo campo que carrega segredo recebe uma marca DISTINTA, e a
    /// asercao e sobre o JSON inteiro: se um campo novo entrar no
    /// `para_json`, ele so passa se a marca dele nao aparecer. O teste tambem
    /// diz QUAL marca vazou, para o conserto nao comecar por procurar.
    #[test]
    fn nenhuma_credencial_do_config_sai_pela_op_config() {
        let segredos = [
            ("token do servidor", "MARCA-TOKEN-SERVIDOR"),
            ("hash do root", "pbkdf2-sha256$1000$a1$dead0001"),
            ("hash de um usuario", "pbkdf2-sha256$1000$a2$dead0002"),
            ("senha da cifra", "MARCA-SENHA-CIFRA"),
            ("senha do rele de e-mail", "MARCA-SENHA-RELE"),
            ("token da origem de replicacao", "MARCA-TOKEN-ORIGEM"),
            ("senha em claro da origem", "MARCA-SENHA-ORIGEM"),
            ("hash da origem", "pbkdf2-sha256$1000$a3$dead0003"),
            ("token do cluster", "MARCA-TOKEN-CLUSTER"),
            ("hash do cluster", "pbkdf2-sha256$1000$a4$dead0004"),
        ];
        let bruto = r#"{
          "token":"MARCA-TOKEN-SERVIDOR",
          "bind":"127.0.0.1:5000",
          "root":{"id":1,"login":"root",
                  "senha_hash":"pbkdf2-sha256$1000$a1$dead0001"},
          "usuarios":[{"id":2,"login":"ana",
                       "senha_hash":"pbkdf2-sha256$1000$a2$dead0002"}],
          "cifra":{"ligada":true,"senha":"MARCA-SENHA-CIFRA"},
          "alertas":{"ligado":true,"email":{"ligado":true,"servidor":"rele",
                     "de":"phx@x.com","para":["a@x.com"],
                     "usuario":"phx","senha":"MARCA-SENHA-RELE"}},
          "replicacao":{"papel":"replica","id_servidor":"r1",
            "origens":[{"nome":"m","host":"10.0.0.1","porta":5000,
                        "token":"MARCA-TOKEN-ORIGEM",
                        "usuario":"rep","senha":"MARCA-SENHA-ORIGEM",
                        "senha_hash":"pbkdf2-sha256$1000$a3$dead0003"}]},
          "cluster":{"id":"r1","token":"MARCA-TOKEN-CLUSTER","usuario":"rep",
            "senha_hash":"pbkdf2-sha256$1000$a4$dead0004",
            "nos":[{"id":"r1","endereco":"127.0.0.1","porta":5000},
                   {"id":"r2","endereco":"127.0.0.2","porta":5000}]}
        }"#;
        let c = Config::de_json(&Json::analisar(bruto).unwrap()).unwrap();

        // O caminho de leitura continua funcionando -- um `para_json` que
        // esconde tudo porque nao leu nada passaria neste teste sem valer.
        assert_eq!(c.token, "MARCA-TOKEN-SERVIDOR");
        assert_eq!(c.cifra.senha(), "MARCA-SENHA-CIFRA");
        assert_eq!(c.alertas.email.senha(), "MARCA-SENHA-RELE");
        assert_eq!(c.replicacao.origens[0].token, "MARCA-TOKEN-ORIGEM");
        assert_eq!(c.cluster.as_ref().unwrap().token, "MARCA-TOKEN-CLUSTER");

        let texto = c.para_json().escrever();
        for (qual, marca) in segredos {
            assert!(
                !texto.contains(marca),
                "vazou pela op `config`: {qual} ({marca})\n{texto}"
            );
        }
        // E nem o prefixo do PBKDF2 sozinho: ele denuncia o formato e o
        // numero de voltas de todo hash deste servidor.
        assert!(
            !texto.contains("pbkdf2-sha256$"),
            "o formato do hash vazou: {texto}"
        );
    }

    #[test]
    fn a_senha_da_cifra_pode_vir_do_ambiente() {
        std::env::set_var("PHXSQL_TESTE_CIFRA", "vinda do ambiente");
        let j = Json::analisar(
            r#"{"token":"t","cifra":{"ligada":true,"senha_env":"PHXSQL_TESTE_CIFRA"}}"#,
        )
        .unwrap();
        let c = Config::de_json(&j).unwrap();
        assert_eq!(c.cifra.senha(), "vinda do ambiente");
        let texto = c.para_json().escrever();
        assert!(!texto.contains("vinda do ambiente"), "{texto}");
        assert!(texto.contains("(do ambiente)"));
        std::env::remove_var("PHXSQL_TESTE_CIFRA");
    }

    #[test]
    fn padroes_quando_o_json_e_minimo() {
        let j = Json::analisar(r#"{"token":"segredo"}"#).unwrap();
        let c = Config::de_json(&j).unwrap();
        assert_eq!(c.bind, "0.0.0.0:5000");
        assert_eq!(c.max_linhas, 1_000);
        assert_eq!(c.replicacao.papel, Papel::Isolado);
        assert!(c.ips_permitidos.is_empty());
        c.validar().unwrap();
    }

    #[test]
    fn campo_com_nome_errado_e_apontado() {
        // O caso real: quem quer trocar a porta escreve "porta", que nao
        // existe -- o campo e "bind". Sem aviso, o servidor sobe na 5000 e
        // parece obedecer.
        let j = Json::analisar(
            r#"{"token":"t","porta":5001,"_comentario":"isto e comentario","bind":"0.0.0.0:5000"}"#,
        )
        .unwrap();
        let c = Config::de_json(&j).unwrap();
        assert_eq!(c.estranhas, vec!["porta".to_string()]);
        assert_eq!(c.bind, "0.0.0.0:5000");
    }

    #[test]
    fn config_so_com_campos_conhecidos_nao_avisa() {
        let j = Json::analisar(r#"{"token":"t","bind":"0.0.0.0:5000","espelho":true}"#).unwrap();
        assert!(Config::de_json(&j).unwrap().estranhas.is_empty());
    }

    /// O aviso tem de cobrir DENTRO das secoes: o erro de digitacao mais
    /// provavel e `recursos.cache_pagina` sem o `s`, e ele passava calado.
    #[test]
    fn campo_estranho_dentro_de_secao_e_apontado() {
        let j = Json::analisar(
            r#"{"token":"t","recursos":{"cache_pagina":4096,"threads":2},
                "web":{"sesao_minutos":5},
                "alertas":{"ligado":false,"email":{"servido":"x"}}}"#,
        )
        .unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(
            c.estranhas.contains(&"recursos.cache_pagina".to_string()),
            "{:?}",
            c.estranhas
        );
        assert!(c.estranhas.contains(&"web.sesao_minutos".to_string()));
        assert!(c.estranhas.contains(&"alertas.email.servido".to_string()));
        assert!(
            !c.estranhas.iter().any(|x| x == "recursos.threads"),
            "campo certo apontado como estranho: {:?}",
            c.estranhas
        );
    }

    /// `seguranca` e `replicacao` ganham campos novos por OUTRAS frentes nesta
    /// rodada: apontar o que este processo ainda nao conhece la dentro seria
    /// um aviso falso para todo mundo que atualizar primeiro o config.
    #[test]
    fn secoes_de_outras_frentes_nao_geram_aviso_por_dentro() {
        let j = Json::analisar(
            r#"{"token":"t","seguranca":{"campo_novo":1},
                "replicacao":{"papel":"isolado","agendamento_novo":{}}}"#,
        )
        .unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    }

    /// A resposta da op `config` espelha o ARQUIVO: as secoes saem com os
    /// nomes do config.json. A versao achatada mentia por omissao -- a tela
    /// lia `c.seguranca.bases_proibidas`, o campo nao vinha, e uma base
    /// proibida aparecia como "nao".
    #[test]
    fn a_resposta_de_config_espelha_o_arquivo() {
        let j = Json::analisar(
            r#"{"token":"t","timeout_s":45,
                "seguranca":{"bases_proibidas":["financeiro"],"bloqueio_minutos":120},
                "web":{"ligado":true,"bind":"127.0.0.1:5001","sessao_minutos":15},
                "backup":{"agendado":true,"hora":"03:00","destino":"copias"},
                "replicacao":{"papel":"source"},
                "isto_nao_existe":1}"#,
        )
        .unwrap();
        let r = Config::de_json(&j).unwrap().para_json();
        assert_eq!(r.inteiro_ou("timeout_s", 0), 45);
        let seg = r.campo("seguranca").expect("sem a secao seguranca");
        assert_eq!(seg.textos("bases_proibidas"), vec!["financeiro"]);
        assert_eq!(seg.inteiro_ou("bloqueio_minutos", 0), 120);
        let web = r.campo("web").expect("sem a secao web");
        assert!(web.booleano_ou("ligado", false));
        assert_eq!(web.inteiro_ou("sessao_minutos", 0), 15);
        let bkp = r.campo("backup").expect("sem a secao backup");
        assert_eq!(bkp.texto_ou("hora", ""), "03:00");
        let rep = r.campo("replicacao").expect("sem a secao replicacao");
        assert_eq!(rep.texto_ou("papel", ""), "source");
        assert!(rep.booleano_ou("imagem_da_linha", false));
        // E o aviso de campo desconhecido chega na tela, nao so no stderr.
        let estranhas = r.textos("estranhas");
        assert_eq!(estranhas, vec!["isto_nao_existe"]);
    }

    #[test]
    fn os_exemplos_nao_tem_campo_estranho() {
        // Se um exemplo trouxesse campo que o servidor ignora, o aviso
        // apareceria para todo mundo que comeca por ele.
        for (n, texto) in [
            (1, crate::CONFIG_EXEMPLO_01),
            (2, crate::CONFIG_EXEMPLO_02),
            (3, crate::CONFIG_EXEMPLO_03),
        ] {
            let j = Json::analisar(texto).unwrap();
            let c = Config::de_json(&j).unwrap();
            assert!(c.estranhas.is_empty(), "exemplo {n}: {:?}", c.estranhas);
        }
    }

    #[test]
    fn sem_token_nao_sobe() {
        let j = Json::analisar("{}").unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(c.validar().is_err());
    }

    #[test]
    fn replica_sem_origem_nao_sobe() {
        let j = Json::analisar(r#"{"token":"x","replicacao":{"papel":"replica"}}"#).unwrap();
        assert!(Config::de_json(&j).unwrap().validar().is_err());
    }

    #[test]
    fn le_origens_de_replicacao() {
        let txt = r#"{
          "token":"x",
          "replicacao":{
            "papel":"replica",
            "id_servidor":"belgica-01",
            "origens":[
              {"nome":"curitiba","host":"10.1.1.102","porta":5000,"token":"t1","databases":["Z"]},
              {"nome":"saopaulo","host":"10.2.1.10","porta":5000,"token":"t2"}
            ]
          }
        }"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert_eq!(c.replicacao.papel, Papel::Replica);
        assert_eq!(c.replicacao.origens.len(), 2);
        assert_eq!(c.replicacao.origens[0].host, "10.1.1.102");
        assert_eq!(c.replicacao.origens[0].databases, vec!["Z"]);
        assert_eq!(c.replicacao.origens[1].porta, 5000);
        assert_eq!(c.replicacao.origens[1].reconectar_em, 10);
        c.validar().unwrap();
    }

    #[test]
    fn le_a_secao_de_seguranca() {
        let txt = r#"{
          "token":"x",
          "seguranca":{
            "comandos_proibidos":["excluir","reindexar"],
            "bases_proibidas":["financeiro"],
            "tentativas_ate_bloquear":3,
            "tentativas_para_bloqueio":4,
            "janela_minutos":5,
            "bloqueio_minutos":120,
            "whitelist":["127.0.0.1","192.168.50.0/24"],
            "blacklist":"bl.json",
            "firewall":{"ligado":true,"bloquear":["/sbin/iptables","-s","{ip}"]}
          }
        }"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.politica.comando_proibido("excluir"));
        assert!(c.politica.comando_proibido("REINDEXAR"));
        assert!(!c.politica.comando_proibido("ler"));
        assert!(c.politica.base_proibida("financeiro"));
        assert_eq!(c.politica.tentativas_ate_bloquear, 3);
        assert_eq!(c.politica.tentativas_para_bloqueio, 4);
        assert_eq!(c.politica.bloqueio_minutos, 120);
        assert!(c.politica.na_whitelist("127.0.0.1"));
        assert!(c.politica.na_whitelist("192.168.50.77"));
        assert!(!c.politica.na_whitelist("10.0.0.1"));
        assert!(c.politica.firewall.as_ref().unwrap().ligado);
        assert_eq!(c.blacklist, PathBuf::from("bl.json"));
    }

    /// **O teste que mais importa da guarda nova**: sem o bloco `seguranca`,
    /// a politica e a de sempre -- nada proibido, whitelist vazia, e o grave
    /// bloqueia na primeira, como desde que a blacklist existe.
    #[test]
    fn sem_secao_de_seguranca_nada_e_proibido() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(!c.politica.comando_proibido("excluir"));
        assert!(c.politica.firewall.is_none());
        assert_eq!(c.politica.tentativas_ate_bloquear, 5);
        assert_eq!(c.politica.tentativas_para_bloqueio, 1);
        assert!(c.politica.whitelist.is_empty());
        assert!(!c.politica.na_whitelist("127.0.0.1"));
    }

    #[test]
    fn idioma_ausente_e_portugues_e_desconhecido_avisa() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert_eq!(c.idioma, "");
        assert!(c.avisos.is_empty());

        let c = Config::de_json(&Json::analisar(r#"{"token":"x","idioma":"Ingles"}"#).unwrap())
            .unwrap();
        assert_eq!(c.idioma, "Ingles");
        assert!(c.avisos.is_empty());

        // Desconhecido nao derruba: avisa no arranque e cai no portugues --
        // o mesmo padrao do campo com nome errado.
        let c = Config::de_json(&Json::analisar(r#"{"token":"x","idioma":"Klingon"}"#).unwrap())
            .unwrap();
        assert_eq!(c.idioma, "");
        assert_eq!(c.avisos.len(), 1);
        assert!(c.avisos[0].contains("Klingon"), "{:?}", c.avisos);
    }

    /// **O comportamento velho.** Sem o bloco, tudo de fabrica.
    ///
    /// Cor vazia, e nao o hexadecimal do tema escuro: a de fabrica e a
    /// variavel do tema, que escurece sozinha no tema claro. Congelar o
    /// hexadecimal aqui tiraria isso de quem nunca pediu nada.
    #[test]
    fn sem_bloco_de_telemetria_tudo_de_fabrica() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert_eq!(c.telemetria, Painel::default());
        assert!(c.telemetria.cor_alto.is_empty());
        assert!(c.telemetria.cores_json().is_none());
        assert_eq!(c.telemetria.alto_uso_ms, crate::telemetria::ALTO_USO_MS);
        assert_eq!(c.telemetria.stress_ms, crate::telemetria::STRESS_MS);
        assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    }

    #[test]
    fn le_as_cores_e_os_limiares_do_painel() {
        let txt = r##"{
          "token":"x",
          "telemetria":{
            "cor_normal":"#3355FF",
            "cor_alto":"#00c2a8",
            "alto_uso_ms":700,
            "stress_ms":1500
          }
        }"##;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        // Guardada em minusculas: a tela compara com o que o `<input
        // type=color>` devolve, que e sempre minusculo -- sem isso um
        // "#3355FF" gravado a mao apareceria como mudanca a cada abrir da tela.
        assert_eq!(c.telemetria.cor_normal, "#3355ff");
        assert_eq!(c.telemetria.cor_alto, "#00c2a8");
        assert!(c.telemetria.cor_stress.is_empty());
        assert_eq!(c.telemetria.alto_uso_ms, 700);
        assert_eq!(c.telemetria.stress_ms, 1_500);
        assert!(c.avisos.is_empty(), "{:?}", c.avisos);
        assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
        // So o que foi escolhido viaja.
        let cores = c.telemetria.cores_json().unwrap().escrever();
        assert_eq!(cores, r##"{"normal":"#3355ff","alto":"#00c2a8"}"##);
    }

    /// Cor torta avisa e cai na de fabrica -- nao derruba o servidor.
    ///
    /// O mesmo padrao do idioma que nao existe: um arranque que morre por
    /// causa de uma cor cobraria mais caro do que aquilo que a guarda protege.
    #[test]
    fn cor_torta_vira_aviso_e_cai_na_de_fabrica() {
        let txt = r##"{"token":"x","telemetria":{"cor_alto":"amarelo","cor_stress":"#ff0000"}}"##;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.telemetria.cor_alto.is_empty());
        assert_eq!(c.telemetria.cor_stress, "#ff0000");
        assert_eq!(c.avisos.len(), 1);
        assert!(c.avisos[0].contains("cor_alto"), "{:?}", c.avisos);
        c.validar().unwrap();
    }

    /// Limiar zero apagaria o nivel inteiro -- toda operacao nasceria amarela.
    #[test]
    fn limiar_zero_vira_um() {
        let txt = r#"{"token":"x","telemetria":{"alto_uso_ms":0,"stress_ms":-5}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert_eq!(c.telemetria.alto_uso_ms, 1);
        assert_eq!(c.telemetria.stress_ms, 1);
    }

    #[test]
    fn lista_de_ips_filtra() {
        let j = Json::analisar(r#"{"token":"x","ips_permitidos":["192.168.50.20"]}"#).unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(c.ip_permitido("192.168.50.20"));
        assert!(!c.ip_permitido("10.0.0.1"));

        let livre = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(livre.ip_permitido("qualquer"));
    }

    #[test]
    fn token_em_tempo_constante() {
        let j = Json::analisar(r#"{"token":"abc123"}"#).unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(c.token_confere("abc123"));
        assert!(!c.token_confere("abc124"));
        assert!(!c.token_confere("abc"));
        assert!(!c.token_confere(""));
    }

    /* -------------------------------------------------------------- cluster */

    /// Sem o bloco `cluster`, NADA muda -- e este e o teste que mais importa:
    /// todo config que ja existe tem de continuar subindo exatamente como
    /// antes, sem ganhar thread, portao nem aviso novo.
    #[test]
    fn sem_o_bloco_cluster_nada_muda() {
        let j = Json::analisar(r#"{"token":"t"}"#).unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(c.cluster.is_none());
        assert!(c.estranhas.is_empty());
        c.validar().unwrap();

        // E a regra velha da replica sem origem continua valendo sem cluster.
        let j = Json::analisar(r#"{"token":"x","replicacao":{"papel":"replica"}}"#).unwrap();
        assert!(Config::de_json(&j).unwrap().validar().is_err());
    }

    fn cluster_minimo(extra: &str) -> String {
        format!(
            r#"{{"token":"t","replicacao":{{"papel":"source"}},
                "cluster":{{"id":"no1","janela_inatividade_s":6,{extra}
                  "nos":[
                    {{"id":"no1","endereco":"127.0.0.1","porta":5310}},
                    {{"id":"no2","endereco":"127.0.0.1","porta":5311}},
                    {{"id":"no3","endereco":"127.0.0.1","porta":5312}}]}}}}"#
        )
    }

    #[test]
    fn le_o_bloco_cluster() {
        let txt = cluster_minimo(r#""prioridade":7,"avisar_cada_min":0.5,"#);
        let c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let cl = c.cluster.as_ref().unwrap();
        assert_eq!(cl.id, "no1");
        assert_eq!(cl.nos.len(), 3);
        assert_eq!(cl.prioridade, 7);
        assert_eq!(cl.janela_s, 6);
        // Sem pulso_s, um terco da janela.
        assert_eq!(cl.pulso_s, 2);
        assert_eq!(cl.avisar_cada_ms(), 30_000);
        assert_eq!(cl.no("no2").unwrap().alvo(), "127.0.0.1:5311");
        assert_eq!(cl.outros().count(), 2);
        assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
        c.validar().unwrap();
    }

    /// Num cluster qualquer no pode ser promovido, entao a imagem da linha
    /// liga em todo papel -- e desliga-la de proposito e contradicao.
    #[test]
    fn cluster_liga_a_imagem_da_linha_em_toda_replica() {
        let txt = cluster_minimo("").replace(
            r#""replicacao":{"papel":"source"}"#,
            r#""replicacao":{"papel":"replica"}"#,
        );
        let c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        assert!(c.replicacao.imagem_da_linha);
        // Replica de cluster sobe SEM origens: a origem e o master corrente.
        c.validar().unwrap();

        let desligada = txt.replace(
            r#""replicacao":{"papel":"replica"}"#,
            r#""replicacao":{"papel":"replica","imagem_da_linha":false}"#,
        );
        assert!(Config::de_json(&Json::analisar(&desligada).unwrap()).is_err());
    }

    #[test]
    fn cluster_recusa_lista_torta() {
        // Este servidor fora da propria lista.
        let fora = cluster_minimo("").replace(r#""id":"no1","janela"#, r#""id":"no9","janela"#);
        let c = Config::de_json(&Json::analisar(&fora).unwrap()).unwrap();
        assert!(c.validar().is_err());

        // Menos de dois nos.
        let um = r#"{"token":"t","replicacao":{"papel":"source"},
            "cluster":{"id":"no1","nos":[{"id":"no1","endereco":"127.0.0.1","porta":5310}]}}"#;
        assert!(Config::de_json(&Json::analisar(um).unwrap())
            .unwrap()
            .validar()
            .is_err());

        // Papel isolado nao tem lugar num cluster.
        let isolado = cluster_minimo("").replace(r#""replicacao":{"papel":"source"},"#, "");
        assert!(Config::de_json(&Json::analisar(&isolado).unwrap())
            .unwrap()
            .validar()
            .is_err());
    }

    #[test]
    fn maioria_e_mais_da_metade_dos_configurados() {
        let txt = cluster_minimo("");
        let c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let cl = c.cluster.unwrap();
        assert!(!cl.e_maioria(1), "1 de 3 nao e maioria");
        assert!(cl.e_maioria(2));
        assert!(cl.e_maioria(3));
    }

    /// A credencial do cluster nao sai pela op `config`, pela mesma regra da
    /// senha do rele: resposta de protocolo nao carrega segredo.
    #[test]
    fn a_credencial_do_cluster_nao_sai_em_json() {
        let txt = cluster_minimo(
            r#""token":"segredo-entre-nos","usuario":"replicador",
               "senha_hash":"pbkdf2-sha256$210000$aa$bb","#,
        );
        let c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let texto = c.para_json().escrever();
        assert!(!texto.contains("segredo-entre-nos"), "{texto}");
        assert!(!texto.contains("pbkdf2-sha256$210000"), "{texto}");
    }

    #[test]
    fn papel_aceita_nomenclatura_antiga_e_nova() {
        assert_eq!(Papel::de_texto("master").unwrap(), Papel::Source);
        assert_eq!(Papel::de_texto("source").unwrap(), Papel::Source);
        assert_eq!(Papel::de_texto("slave").unwrap(), Papel::Replica);
        assert_eq!(Papel::de_texto("replica").unwrap(), Papel::Replica);
        assert_eq!(Papel::de_texto("read_replica").unwrap(), Papel::ReadReplica);
        assert_eq!(Papel::de_texto("leitura").unwrap(), Papel::ReadReplica);
        assert_eq!(Papel::de_texto("spare").unwrap(), Papel::Spare);
        assert_eq!(Papel::de_texto("standby").unwrap(), Papel::Spare);
        assert_eq!(Papel::de_texto("multi").unwrap(), Papel::Multi);
        assert_eq!(Papel::de_texto("bidirecional").unwrap(), Papel::Multi);
        assert!(Papel::de_texto("banana").is_err());
    }

    /// Os papeis novos que puxam de origem tambem exigem uma origem -- e o
    /// multi exige a identidade e recusa a contradicao com somente_leitura.
    #[test]
    fn os_papeis_novos_validam_o_que_lhes_falta() {
        for papel in ["read_replica", "spare", "multi"] {
            let txt = format!(r#"{{"token":"x","replicacao":{{"papel":"{papel}"}}}}"#);
            assert!(
                Config::de_json(&Json::analisar(&txt).unwrap())
                    .unwrap()
                    .validar()
                    .is_err(),
                "{papel} sem origem subiu"
            );
        }

        let origem = r#""origens":[{"nome":"a","host":"127.0.0.1","porta":5000,"token":"t"}]"#;

        // Multi sem id_servidor: e o id que marca a origem dos eventos.
        let sem_id = format!(r#"{{"token":"x","replicacao":{{"papel":"multi",{origem}}}}}"#);
        let e = Config::de_json(&Json::analisar(&sem_id).unwrap())
            .unwrap()
            .validar()
            .unwrap_err()
            .to_string();
        assert!(e.contains("id_servidor"), "{e}");

        // Multi com somente_leitura e contradicao.
        let contradicao = format!(
            r#"{{"token":"x","somente_leitura":true,
                 "replicacao":{{"papel":"multi","id_servidor":"a",{origem}}}}}"#
        );
        assert!(Config::de_json(&Json::analisar(&contradicao).unwrap())
            .unwrap()
            .validar()
            .is_err());

        // Multi completo sobe, e a imagem ja vem ligada por padrao.
        let ok = format!(
            r#"{{"token":"x","replicacao":{{"papel":"multi","id_servidor":"a",{origem}}}}}"#
        );
        let c = Config::de_json(&Json::analisar(&ok).unwrap()).unwrap();
        c.validar().unwrap();
        assert!(c.replicacao.imagem_da_linha, "multi liga a imagem sozinho");
    }

    /// O comportamento VELHO: origem sem os campos de agendamento continua
    /// streaming, byte a byte como sempre foi. E o teste que mais importa.
    #[test]
    fn origem_sem_agendamento_continua_streaming() {
        let txt = r#"{"token":"x","replicacao":{"papel":"replica",
            "origens":[{"nome":"a","host":"127.0.0.1","porta":5000,"token":"t"}]}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        let o = &c.replicacao.origens[0];
        assert_eq!(o.cada_minutos, 0);
        assert!(o.hora.is_empty());
        assert!(!o.agendada(), "sem os campos, e streaming como sempre");
        c.validar().unwrap();
    }

    #[test]
    fn origem_agendada_le_os_dois_jeitos_e_recusa_hora_invalida() {
        let cada = r#"{"token":"x","replicacao":{"papel":"replica",
            "origens":[{"nome":"a","host":"h","porta":5000,"token":"t","cada_minutos":15}]}}"#;
        let c = Config::de_json(&Json::analisar(cada).unwrap()).unwrap();
        assert_eq!(c.replicacao.origens[0].cada_minutos, 15);
        assert!(c.replicacao.origens[0].agendada());
        c.validar().unwrap();

        let diaria = r#"{"token":"x","replicacao":{"papel":"replica",
            "origens":[{"nome":"a","host":"h","porta":5000,"token":"t","hora":"02:30"}]}}"#;
        let c = Config::de_json(&Json::analisar(diaria).unwrap()).unwrap();
        assert_eq!(c.replicacao.origens[0].hora, "02:30");
        assert!(c.replicacao.origens[0].agendada());
        c.validar().unwrap();

        let torta = r#"{"token":"x","replicacao":{"papel":"replica",
            "origens":[{"nome":"a","host":"h","porta":5000,"token":"t","hora":"25:99"}]}}"#;
        assert!(Config::de_json(&Json::analisar(torta).unwrap())
            .unwrap()
            .validar()
            .is_err());
    }
    #[test]
    fn a_interface_web_vem_desligada_e_presa_ao_proprio_computador() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(!c.web.ligado);
        assert_eq!(c.web.bind, "127.0.0.1:5001");
        assert_eq!(c.web.sessao_minutos, 60);
        assert_eq!(c.web.sessao_ms(), 3_600_000);
    }

    #[test]
    fn le_a_secao_web() {
        let txt =
            r#"{"token":"x","web":{"ligado":true,"bind":"0.0.0.0:8080","sessao_minutos":15}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.web.ligado);
        assert_eq!(c.web.bind, "0.0.0.0:8080");
        assert_eq!(c.web.sessao_ms(), 900_000);
        c.validar().unwrap();
    }

    #[test]
    fn a_web_nao_pode_roubar_a_porta_de_dados() {
        let txt = r#"{"token":"x","bind":"127.0.0.1:5000","web":{"ligado":true,"bind":"127.0.0.1:5000"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.validar().is_err());
    }

    #[test]
    fn web_desligada_nao_valida_o_endereco() {
        // Um bind ruim numa interface desligada nao impede o servidor de subir.
        let txt = r#"{"token":"x","web":{"bind":"isso nao e endereco"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        c.validar().unwrap();
    }
    #[test]
    fn le_a_porta_de_replicacao() {
        // Nome novo: envio e retorno separados.
        let txt = r#"{"token":"x","bind":"0.0.0.0:5000",
          "replicacao":{"papel":"source","envio":"0.0.0.0:5010","retorno":"0.0.0.0:5011"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert_eq!(c.replicacao.endereco_envio().unwrap().port(), 5010);
        assert_eq!(c.replicacao.endereco_retorno().unwrap().port(), 5011);
        assert_eq!(c.replicacao.portas().len(), 2);
        c.validar().unwrap();

        // Nome antigo "escuta" continua valendo como envio: config que ja
        // existe nao pode parar de subir so porque o campo foi renomeado.
        let velho = r#"{"token":"x","bind":"0.0.0.0:5000",
          "replicacao":{"papel":"source","escuta":"0.0.0.0:5010"}}"#;
        let c = Config::de_json(&Json::analisar(velho).unwrap()).unwrap();
        assert_eq!(c.replicacao.envio, "0.0.0.0:5010");
        assert!(
            c.replicacao.retorno.is_empty(),
            "sem retorno = volta pelo envio"
        );
        c.validar().unwrap();
    }

    #[test]
    fn a_replicacao_nao_pode_roubar_a_porta_de_dados_nem_a_da_web() {
        let mesma = r#"{"token":"x","bind":"127.0.0.1:5000",
          "replicacao":{"papel":"source","envio":"127.0.0.1:5000"}}"#;
        assert!(Config::de_json(&Json::analisar(mesma).unwrap())
            .unwrap()
            .validar()
            .is_err());

        let contra_web = r#"{"token":"x","bind":"127.0.0.1:5000",
          "web":{"ligado":true,"bind":"127.0.0.1:5001"},
          "replicacao":{"papel":"source","envio":"127.0.0.1:5001"}}"#;
        assert!(Config::de_json(&Json::analisar(contra_web).unwrap())
            .unwrap()
            .validar()
            .is_err());

        // E o envio contra o proprio retorno.
        let uma_contra_outra = r#"{"token":"x","bind":"127.0.0.1:5000",
          "replicacao":{"papel":"source","envio":"127.0.0.1:5010","retorno":"127.0.0.1:5010"}}"#;
        assert!(Config::de_json(&Json::analisar(uma_contra_outra).unwrap())
            .unwrap()
            .validar()
            .is_err());
    }

    #[test]
    fn sem_escuta_a_replicacao_usa_a_porta_de_dados() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(c.replicacao.envio.is_empty());
        assert!(c.replicacao.retorno.is_empty());
        assert!(c.replicacao.portas().is_empty());
        c.validar().unwrap();
    }

    #[test]
    fn a_lista_de_servidores_da_web_e_exata() {
        let txt = r#"{"token":"x","web":{"servidores":["10.1.1.5:5000","curitiba:5000"]}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.web.alcanca_outro_servidor());
        assert!(c.web.servidor_permitido("10.1.1.5:5000"));
        assert!(c.web.servidor_permitido(" curitiba:5000 "));
        // Sem porta, com outra porta, ou vazio: nao entra.
        assert!(!c.web.servidor_permitido("10.1.1.5"));
        assert!(!c.web.servidor_permitido("10.1.1.5:5001"));
        assert!(!c.web.servidor_permitido(""));

        let fechado = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(!fechado.web.alcanca_outro_servidor());
        assert!(!fechado.web.servidor_permitido("qualquer:5000"));
    }
    #[test]
    fn o_backup_vem_desligado() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(!c.backup.agendado);
        assert!(c.backup.zip, "zip e o padrao quando ligarem");
        assert_eq!(c.backup.manter, 14);
        assert!(
            !c.backup.hora_de_rodar(1_000_000, 0),
            "desligado nunca roda"
        );
    }

    #[test]
    fn hora_marcada_dispara_uma_vez_por_dia() {
        let txt = r#"{"token":"x","backup":{"agendado":true,"hora":"03:00"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        let dia = 20_000i64 * 86_400_000;

        // 02:59 ainda nao.
        assert!(!c.backup.hora_de_rodar(dia + 2 * 3_600_000 + 59 * 60_000, 0));
        // 03:00 sim, porque nunca rodou.
        let as_tres = dia + 3 * 3_600_000;
        assert!(c.backup.hora_de_rodar(as_tres, 0));
        // 03:01, ja tendo rodado as 03:00: NAO de novo.
        assert!(!c.backup.hora_de_rodar(as_tres + 60_000, as_tres));
        // 23:59 do mesmo dia: ainda nao.
        assert!(!c.backup.hora_de_rodar(dia + 86_340_000, as_tres));
        // 03:00 do dia seguinte: sim.
        assert!(c.backup.hora_de_rodar(as_tres + 86_400_000, as_tres));
    }

    #[test]
    fn sem_hora_marcada_vale_o_intervalo() {
        let txt = r#"{"token":"x","backup":{"agendado":true,"cada_horas":6}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(
            c.backup.hora_de_rodar(1_000_000_000, 0),
            "nunca rodou, roda"
        );
        let t = 1_000_000_000i64;
        assert!(!c.backup.hora_de_rodar(t + 5 * 3_600_000, t));
        assert!(c.backup.hora_de_rodar(t + 6 * 3_600_000, t));
    }

    #[test]
    fn hora_invalida_nao_sobe() {
        for h in ["25:00", "12:60", "meia-noite", "3", "03;00"] {
            let txt = format!(r#"{{"token":"x","backup":{{"agendado":true,"hora":"{h}"}}}}"#);
            assert!(
                Config::de_json(&Json::analisar(&txt).unwrap()).is_err(),
                "{h:?} passou"
            );
        }
        assert_eq!(Backup::minuto_do_dia("03:00"), Some(180));
        assert_eq!(Backup::minuto_do_dia("23:59"), Some(1439));
        assert_eq!(Backup::minuto_do_dia("00:00"), Some(0));
    }
}

#[cfg(test)]
mod testes_recursos {
    use super::*;

    fn cfg(t: &str) -> Config {
        Config::de_json(&Json::analisar(t).unwrap()).unwrap()
    }

    #[test]
    fn sem_a_secao_recursos_valem_os_padroes() {
        let c = cfg(r#"{"bind":"127.0.0.1:5000","token":"t"}"#);
        assert_eq!(c.recursos.durabilidade, Durabilidade::PorLote);
        assert_eq!(c.recursos.lote_operacoes, 200);
        assert_eq!(c.recursos.cpu_percentual, 100);
        assert_eq!(c.recursos.usuarios_max, 0, "zero = sem teto");
    }

    /// `conexoes_max` morava no topo antes de existir a secao `recursos`.
    /// Config antigo nao pode parar de subir por causa disso.
    #[test]
    fn conexoes_max_no_topo_continua_valendo() {
        let c = cfg(r#"{"token":"t","conexoes_max":7}"#);
        assert_eq!(c.conexoes_max, 7);
        assert_eq!(c.recursos.conexoes_max, 7, "a secao herda o do topo");

        // E dentro de `recursos` ele ganha, porque e o lugar novo -- inclusive
        // no campo do TOPO, que e o que o laco de aceitacao le de verdade.
        // Antes deste espelho a promessa era mentira: o 99 ficava so na tela e
        // o servidor seguia recusando na 7a conexao.
        let c = cfg(r#"{"token":"t","conexoes_max":7,"recursos":{"conexoes_max":99}}"#);
        assert_eq!(c.recursos.conexoes_max, 99);
        assert_eq!(c.conexoes_max, 99, "o leitor real nao veria o 99");
    }

    /// `threads` e `cpu_percentual` tem leitor de verdade: o teto global do
    /// trabalho dividido. Antes disto os dois campos existiam no config.json,
    /// no MANUAL e na tela, e `paralelo::nucleos()` perguntava direto a
    /// maquina -- a mesma armadilha do `cache_paginas` sem cache.
    #[test]
    fn threads_e_cpu_viram_o_teto_do_paralelo() {
        // 4 threads a 25% = teto de UM nucleo -- e um e o resultado em
        // qualquer maquina, porque o teto so corta, nunca inventa nucleo.
        let c = cfg(r#"{"token":"t","recursos":{"threads":4,"cpu_percentual":25}}"#);
        c.recursos.aplicar();
        assert_eq!(phxsql_core::paralelo::nucleos(), 1, "o teto nao valeu");
        // Devolve o processo ao estado sem teto, para nao morder os vizinhos.
        phxsql_core::paralelo::definir_teto(0);
    }

    #[test]
    fn durabilidade_le_os_tres_modos_e_recusa_o_resto() {
        for (texto, esperado) in [
            ("por_operacao", Durabilidade::PorOperacao),
            ("por_lote", Durabilidade::PorLote),
            ("sistema", Durabilidade::Sistema),
            ("SEMPRE", Durabilidade::PorOperacao),
        ] {
            let c = cfg(&format!(
                r#"{{"token":"t","recursos":{{"durabilidade":"{texto}"}}}}"#
            ));
            assert_eq!(c.recursos.durabilidade, esperado, "{texto}");
        }
        let erro = Config::de_json(
            &Json::analisar(r#"{"token":"t","recursos":{"durabilidade":"talvez"}}"#).unwrap(),
        )
        .unwrap_err()
        .to_string();
        assert!(erro.contains("durabilidade desconhecida"), "{erro}");
    }

    /// O percentual de CPU vira numero de nucleos, e nunca zero: metade de um
    /// nucleo continua sendo um nucleo.
    #[test]
    fn o_percentual_de_cpu_vira_nucleos() {
        let com = |t: usize, p: u8| {
            Recursos {
                threads: t,
                cpu_percentual: p,
                ..Recursos::default()
            }
            .nucleos()
        };
        assert_eq!(com(8, 100), 8);
        assert_eq!(com(8, 50), 4);
        assert_eq!(com(8, 25), 2);
        assert_eq!(com(1, 50), 1, "nunca zero");
        assert_eq!(com(3, 1), 1);
    }

    #[test]
    fn os_tetos_aceitam_zero_como_sem_teto() {
        let c =
            cfg(r#"{"token":"t","recursos":{"memoria_max_mb":0,"usuarios_max":0,"threads":0}}"#);
        assert_eq!(c.recursos.memoria_max_mb, 0);
        assert_eq!(c.recursos.usuarios_max, 0);
        // threads zero quer dizer "quantos nucleos a maquina tiver", e o
        // resultado tem de ser pelo menos um.
        assert!(c.recursos.nucleos() >= 1);
    }

    #[test]
    fn numero_fora_da_faixa_e_ajustado_em_vez_de_derrubar_o_arranque() {
        let c = cfg(r#"{"token":"t","recursos":{"cpu_percentual":500,"lote_operacoes":0}}"#);
        assert_eq!(c.recursos.cpu_percentual, 100, "acima de 100 vira 100");
        assert_eq!(c.recursos.lote_operacoes, 1, "zero vira um");
    }

    #[test]
    fn a_secao_recursos_e_um_campo_conhecido() {
        // Campo desconhecido no config avisa no arranque; `recursos` nao pode
        // cair nessa lista.
        let c = cfg(r#"{"token":"t","recursos":{"threads":2}}"#);
        assert!(!c.estranhas.iter().any(|x| x == "recursos"));
    }
}

#[cfg(test)]
mod testes_alertas {
    use super::*;

    fn de(txt: &str) -> Config {
        Config::de_json(&Json::analisar(txt).unwrap()).unwrap()
    }

    #[test]
    fn sem_a_secao_o_alerta_vem_desligado() {
        let c = de(r#"{"token":"x"}"#);
        assert!(!c.alertas.ligado);
        assert!(!c.alertas.email.ligado);
    }

    #[test]
    fn o_que_dispara_primeiro_manda() {
        let c = de(r#"{"token":"x","alertas":{"ligado":true,
                "livre_minimo_percentual":10,"livre_minimo_mb":1024}}"#);
        let a = &c.alertas;
        // Disco grande com 12% livre e 900 GB: o percentual nao aperta e o
        // piso muito menos.
        assert!(!a.apertado(12.0, 900 * 1024 * 1024));
        // Mesmo disco a 8%: o percentual aperta, mesmo com 600 GB livres.
        assert!(a.apertado(8.0, 600 * 1024 * 1024));
        // Disco pequeno com 20% livre, mas so 500 MB: o piso aperta, mesmo com
        // o percentual folgado. E o caso que o percentual sozinho perderia.
        assert!(a.apertado(20.0, 500 * 1024));
    }

    #[test]
    fn limite_em_zero_desliga_aquele_lado() {
        let c = de(r#"{"token":"x","alertas":{"ligado":true,
                "livre_minimo_percentual":0,"livre_minimo_mb":512}}"#);
        // So o piso vale: 1% livre com 2 GB nao aperta.
        assert!(!c.alertas.apertado(1.0, 2 * 1024 * 1024));
        assert!(c.alertas.apertado(90.0, 100 * 1024));
    }

    #[test]
    fn ligado_sem_limite_nenhum_e_erro() {
        // Um alerta que nunca dispara e pior do que nenhum: quem configurou
        // acha que esta protegido.
        let j = Json::analisar(
            r#"{"token":"x","alertas":{"ligado":true,
                "livre_minimo_percentual":0,"livre_minimo_mb":0}}"#,
        )
        .unwrap();
        assert!(Config::de_json(&j).is_err());
    }

    #[test]
    fn email_ligado_sem_destinatario_nao_sobe() {
        let j = Json::analisar(
            r#"{"token":"x","alertas":{"ligado":true,
                "email":{"ligado":true,"servidor":"rele","de":"phx@x.com"}}}"#,
        )
        .unwrap();
        let e = Config::de_json(&j).unwrap_err().to_string();
        assert!(e.contains("para"), "{e}");
    }

    #[test]
    fn endereco_com_quebra_de_linha_e_recusado() {
        // Injecao de cabecalho pelo config.json: o "para" carrega um Bcc.
        let j = Json::analisar(
            r#"{"token":"x","alertas":{"ligado":true,"email":{"ligado":true,
                "servidor":"rele","de":"phx@x.com",
                "para":["a@x.com\r\nBcc: ladrao@fora.com"]}}}"#,
        )
        .unwrap();
        assert!(Config::de_json(&j).is_err());
    }

    #[test]
    fn a_senha_do_rele_nunca_aparece_no_json() {
        let c = de(
            r#"{"token":"x","alertas":{"ligado":true,"email":{"ligado":true,
                "servidor":"rele","de":"phx@x.com","para":["a@x.com"],
                "usuario":"phx","senha":"segredo-do-rele"}}}"#,
        );
        assert_eq!(c.alertas.email.senha(), "segredo-do-rele");
        let texto = c.para_json().escrever();
        assert!(
            !texto.contains("segredo-do-rele"),
            "a senha do rele vazou no config: {texto}"
        );
        assert!(texto.contains("(oculta)"), "{texto}");
    }

    /// O teste que mais importa numa guarda nova: o comportamento VELHO.
    /// Quem ja tinha e-mail configurado para o disco nao pode comecar a
    /// receber aviso de job por causa de uma versao nova.
    #[test]
    fn sem_avisar_jobs_nada_muda() {
        let c = de(
            r#"{"token":"x","alertas":{"ligado":true,"email":{"ligado":true,
                "servidor":"rele","de":"phx@x.com","para":["a@x.com"]}}}"#,
        );
        assert!(c.alertas.email.ligado, "o aviso de disco continua como era");
        assert!(
            !c.alertas.email.avisar_jobs,
            "aviso de job e opt-in: sem pedir, nao existe"
        );
    }

    #[test]
    fn avisar_jobs_vale_mesmo_com_o_vigia_de_disco_desligado() {
        // O aviso de jobs anda por fora do vigia de disco -- entao o endereco
        // e conferido no arranque tambem quando so ele esta ligado.
        let c = de(
            r#"{"token":"x","alertas":{"email":{"ligado":true,"avisar_jobs":true,
                "servidor":"rele","de":"phx@x.com","para":["a@x.com"]}}}"#,
        );
        assert!(!c.alertas.ligado);
        assert!(c.alertas.email.avisar_jobs);

        let j = Json::analisar(
            r#"{"token":"x","alertas":{"email":{"ligado":true,"avisar_jobs":true,
                "servidor":"rele","de":"phx@x.com"}}}"#,
        )
        .unwrap();
        let e = Config::de_json(&j).unwrap_err().to_string();
        assert!(
            e.contains("para"),
            "sem destinatario a recusa vem no arranque, nao quando o job falhar: {e}"
        );
    }

    #[test]
    fn a_senha_pode_vir_do_ambiente() {
        // O caminho recomendado: config.json costuma ir para o controle de
        // versao, variavel de ambiente nao.
        std::env::set_var("PHXSQL_TESTE_SMTP", "vinda-do-ambiente");
        let c = de(
            r#"{"token":"x","alertas":{"ligado":true,"email":{"ligado":true,
                "servidor":"rele","de":"phx@x.com","para":["a@x.com"],
                "senha_env":"PHXSQL_TESTE_SMTP"}}}"#,
        );
        assert_eq!(c.alertas.email.senha(), "vinda-do-ambiente");
    }
}

/// A gravacao pela tela: `gravar_campos` e a whitelist.
#[cfg(test)]
mod testes_gravacao {
    use super::*;

    /// Um config.json de verdade no disco, com comentario, ordem propria e um
    /// bloco que este processo nao conhece -- exatamente o que a gravacao nao
    /// pode estragar.
    fn arquivo(nome: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("phx-gravar-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let caminho = dir.join("config.json");
        std::fs::write(
            &caminho,
            r#"{
  "_comentario": "explicacao que o Adriano escreveu",
  "token": "t",
  "bind": "127.0.0.1:5399",
  "max_linhas": 1000,
  "bloco_de_outra_frente": { "campo": 1 },
  "backup": { "agendado": false, "hora": "03:00" },
  "alertas": { "ligado": false, "livre_minimo_percentual": 10 }
}
"#,
        )
        .unwrap();
        caminho
    }

    fn muda(campo: &str, valor: Json) -> Vec<(String, Json)> {
        vec![(campo.to_string(), valor)]
    }

    #[test]
    fn grava_o_pedido_e_preserva_o_resto() {
        let caminho = arquivo("preserva");
        let novo = Config::gravar_campos(&caminho, &muda("max_linhas", Json::de_i64(50))).unwrap();
        assert_eq!(novo.max_linhas, 50);

        let texto = std::fs::read_to_string(&caminho).unwrap();
        assert!(
            texto.contains("explicacao que o Adriano escreveu"),
            "{texto}"
        );
        assert!(texto.contains("bloco_de_outra_frente"), "{texto}");
        assert!(texto.contains("\"max_linhas\": 50"), "{texto}");
        // A ordem das chaves e a do arquivo original, nao a do struct.
        let pos = |t: &str| texto.find(t).unwrap_or(usize::MAX);
        assert!(pos("_comentario") < pos("token"), "{texto}");
        assert!(pos("max_linhas") < pos("bloco_de_outra_frente"), "{texto}");
        // E o arquivo continua sendo um config que sobe.
        Config::ler(&caminho).unwrap();
    }

    /// O arquivo sai byte a byte igual, MENOS o valor trocado.
    ///
    /// Reserializar a arvore preservava valor, ordem e comentario -- e perdia
    /// a forma: as linhas em branco entre as secoes sumiam e `["a","b"]`
    /// virava tres linhas. Num arquivo escrito a mao isso e devolver o
    /// trabalho de alguem reformatado, e no controle de versao e um diff
    /// ilegivel. Este teste e o que trava a troca cirurgica.
    #[test]
    fn o_arquivo_sai_igual_menos_o_valor_trocado() {
        let dir = std::env::temp_dir().join(format!("phx-bytes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let caminho = dir.join("config.json");
        let original = "{\n  \"_nota\": \"escrito a mao\",\n\n  \"token\": \"t\",\n  \
             \"max_linhas\": 1000,\n\n  \"ips_permitidos\": [\"10.0.0.1\", \"10.0.0.2\"],\n\n  \
             \"recursos\": { \"threads\": 0, \"cpu_percentual\": 100 }\n}\n";
        std::fs::write(&caminho, original).unwrap();

        Config::gravar_campos(&caminho, &muda("max_linhas", Json::de_i64(50))).unwrap();
        let depois = std::fs::read_to_string(&caminho).unwrap();
        assert_eq!(
            depois,
            original.replace("\"max_linhas\": 1000", "\"max_linhas\": 50")
        );

        // Inclusive dentro de uma secao escrita numa linha so.
        Config::gravar_campos(&caminho, &muda("recursos.threads", Json::de_i64(4))).unwrap();
        let depois = std::fs::read_to_string(&caminho).unwrap();
        assert!(
            depois.contains("\"recursos\": { \"threads\": 4, \"cpu_percentual\": 100 }"),
            "{depois}"
        );
        // A lista numa linha so e as linhas em branco continuam onde estavam.
        assert!(depois.contains("[\"10.0.0.1\", \"10.0.0.2\"]"), "{depois}");
        assert!(depois.contains("\n\n  \"token\""), "{depois}");
    }

    /// Campo que ainda nao esta no arquivo entra pelo caminho reserializado --
    /// e o arquivo continua valido e com os comentarios.
    #[test]
    fn campo_ausente_entra_pelo_reserializado() {
        let caminho = arquivo("ausente");
        // `backup.manter` nao existe no arquivo de origem.
        let novo =
            Config::gravar_campos(&caminho, &muda("backup.manter", Json::de_i64(3))).unwrap();
        assert_eq!(novo.backup.manter, 3);
        let texto = std::fs::read_to_string(&caminho).unwrap();
        assert!(
            texto.contains("explicacao que o Adriano escreveu"),
            "{texto}"
        );
        assert!(texto.contains("bloco_de_outra_frente"), "{texto}");
        Config::ler(&caminho).unwrap();
    }

    #[test]
    fn campo_dentro_de_secao_muda_so_ele() {
        let caminho = arquivo("secao");
        let novo =
            Config::gravar_campos(&caminho, &muda("backup.agendado", Json::Bool(true))).unwrap();
        assert!(novo.backup.agendado);
        assert_eq!(novo.backup.hora, "03:00", "a hora nao podia mudar");
        // E secao ausente e criada, em vez de recusada.
        let novo =
            Config::gravar_campos(&caminho, &muda("recursos.cache_paginas", Json::de_i64(64)))
                .unwrap();
        assert_eq!(novo.recursos.cache_paginas, 64);
    }

    /// O portao da whitelist: o que nao esta na lista NAO se grava por aqui,
    /// e o arquivo fica intocado -- token e o exemplo que mais importa.
    #[test]
    fn campo_fora_da_lista_e_recusado_sem_tocar_o_arquivo() {
        let caminho = arquivo("whitelist");
        let antes = std::fs::read_to_string(&caminho).unwrap();
        for campo in ["token", "seguranca.firewall", "usuarios", "cifra.senha"] {
            let e = Config::gravar_campos(&caminho, &muda(campo, Json::texto_de("x")))
                .unwrap_err()
                .to_string();
            assert!(e.contains("nao se grava pela tela"), "{campo}: {e}");
        }
        assert_eq!(std::fs::read_to_string(&caminho).unwrap(), antes);
    }

    /// Tipo errado nao entra: o leitor usa `inteiro_ou`, que cai no padrao em
    /// silencio -- um `"max_linhas": "abc"` gravado seria um campo que nunca
    /// vale, e ninguem descobriria pela tela.
    #[test]
    fn tipo_errado_e_recusado_antes_de_gravar() {
        let caminho = arquivo("tipo");
        let antes = std::fs::read_to_string(&caminho).unwrap();
        let e = Config::gravar_campos(&caminho, &muda("max_linhas", Json::texto_de("abc")))
            .unwrap_err()
            .to_string();
        assert!(e.contains("espera inteiro"), "{e}");
        assert_eq!(std::fs::read_to_string(&caminho).unwrap(), antes);
    }

    /// Valor que nao subiria o servidor nao entra no arquivo: a validacao
    /// roda ANTES do rename.
    #[test]
    fn valor_que_nao_valida_nao_entra_no_arquivo() {
        let caminho = arquivo("valida");
        let antes = std::fs::read_to_string(&caminho).unwrap();
        // "25:00" passa no tipo (e texto) e cai na validacao do Backup.
        let e = Config::gravar_campos(&caminho, &muda("backup.hora", Json::texto_de("25:00")))
            .unwrap_err()
            .to_string();
        assert!(e.contains("backup.hora"), "{e}");
        assert_eq!(std::fs::read_to_string(&caminho).unwrap(), antes);
        // O mesmo para um bind que nao e endereco.
        assert!(
            Config::gravar_campos(&caminho, &muda("bind", Json::texto_de("nao e endereco")))
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&caminho).unwrap(), antes);
    }

    /// O que esta GRAVADO e ainda nao vale tem de aparecer -- e o que ja vale
    /// nao pode aparecer como divergencia.
    ///
    /// Achado exercitando a tela: gravar `timeout_s` gravava certo e o campo
    /// voltava com o valor velho, calado. E o primeiro conserto trouxe um
    /// falso positivo junto, porque `livre_minimo_percentual` saia da resposta
    /// como o TEXTO "10.00" contra o numero 10 do arquivo.
    #[test]
    fn o_que_esta_gravado_e_ainda_nao_vale_aparece() {
        let caminho = arquivo("divergencia");
        let c = Config::ler(&caminho).unwrap();
        // Nada gravado ainda: o arquivo e a memoria concordam.
        assert!(divergencias_do_arquivo(&caminho, &c.para_json()).is_empty());

        // Grava um campo de arranque. O `Config` vivo continua o de antes --
        // e e exatamente essa a situacao que a tela precisa mostrar.
        Config::gravar_campos(&caminho, &muda("timeout_s", Json::de_i64(90))).unwrap();
        let fora = divergencias_do_arquivo(&caminho, &c.para_json());
        assert_eq!(fora.len(), 1, "{fora:?}");
        assert_eq!(fora[0].0, "timeout_s");
        assert_eq!(fora[0].1.inteiro(), Some(90));

        // E o percentual do alerta nao pode virar divergencia por causa de
        // formatacao: 10 no arquivo e 10 na resposta sao o mesmo numero.
        let com_alerta = Config::ler(&caminho).unwrap();
        let fora = divergencias_do_arquivo(&caminho, &com_alerta.para_json());
        assert!(
            !fora.iter().any(|(c, _)| c.starts_with("alertas.")),
            "falso positivo de formatacao: {fora:?}"
        );
    }

    /// O comportamento VELHO: um arquivo que ninguem gravou pela tela abre
    /// exatamente como antes -- gravar_campos nao roda no arranque e nao
    /// reescreve nada sozinho.
    #[test]
    fn arquivo_antigo_abre_byte_a_byte_como_antes() {
        let caminho = arquivo("velho");
        let antes = std::fs::read_to_string(&caminho).unwrap();
        let c = Config::ler(&caminho).unwrap();
        assert_eq!(c.max_linhas, 1000);
        assert_eq!(std::fs::read_to_string(&caminho).unwrap(), antes);
    }

    /// A tela grava a cor, e o vazio volta para a de fabrica.
    ///
    /// O vazio e o botao «voltar as cores de fabrica»: ele nao apaga o campo
    /// do arquivo (isso exigiria adivinhar a indentacao de quem escreveu), ele
    /// grava a string vazia -- que e como o leitor escreve «de fabrica».
    #[test]
    fn a_tela_grava_a_cor_e_o_vazio_volta_a_de_fabrica() {
        let caminho = arquivo("cores");
        let novo = Config::gravar_campos(
            &caminho,
            &muda("telemetria.cor_alto", Json::texto_de("#00c2a8")),
        )
        .unwrap();
        assert_eq!(novo.telemetria.cor_alto, "#00c2a8");
        assert!(std::fs::read_to_string(&caminho)
            .unwrap()
            .contains("#00c2a8"));

        let novo =
            Config::gravar_campos(&caminho, &muda("telemetria.cor_alto", Json::texto_de("")))
                .unwrap();
        assert!(novo.telemetria.cor_alto.is_empty());
        assert!(novo.telemetria.cores_json().is_none());
    }

    /// O portao da gravacao recusa o que nao e `#rrggbb`.
    ///
    /// A recusa mora no TIPO do campo, e nao numa conferencia solta no meio da
    /// gravacao: e o mesmo portao que ja recusa `"max_linhas":"abc"`, e por
    /// isso nao ha como uma cor nova entrar por fora dele.
    #[test]
    fn a_tela_recusa_cor_que_nao_e_rrggbb() {
        let caminho = arquivo("cor-torta");
        let antes = std::fs::read_to_string(&caminho).unwrap();
        for torta in ["amarelo", "#12345", "rgb(1,2,3)", "#gggggg", "#00c2a8 "] {
            let e = Config::gravar_campos(
                &caminho,
                &muda("telemetria.cor_alto", Json::texto_de(torta)),
            )
            .unwrap_err()
            .to_string();
            assert!(e.contains("cor"), "{torta:?} passou: {e}");
            assert_eq!(std::fs::read_to_string(&caminho).unwrap(), antes);
        }
    }
}
